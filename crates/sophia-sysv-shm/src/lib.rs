//! Narrow safe adapters for the shared memory an X11 client hands the server.
//!
//! Two backings, one surface. MIT-SHM 1.1 names a SysV segment by `shmid`;
//! 1.2 passes a file descriptor instead, which is what modern toolkits reach
//! for because it needs no SysV namespace and survives sandboxes that
//! `shmget` does not. Callers copy through the same shapes either way.
//!
//! This crate is the workspace's only exemption from `unsafe_code = "forbid"`,
//! so raw shared memory is audited in one place. It stays small deliberately:
//! everything here is bounds-checking around a handful of mapping calls, and
//! anything that does not need a raw pointer belongs somewhere else.

use core::ptr::NonNull;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};

/// The largest segment this adapter will map.
///
/// `CreateSegment` takes a `CARD32`, so a client can ask for four gigabytes;
/// honouring that would let any client exhaust the server's address space for
/// the cost of one request. The bound is generous against what a segment is
/// for -- a single image transfer -- since even an 8K RGBA frame is about
/// 132 MB.
pub const MAX_SEGMENT_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessError {
    InvalidId,
    MissingSegment,
    RangeOverflow,
    OutOfBounds,
    AttachFailed,
    DetachFailed,
    /// The descriptor could not be mapped, or is not shareable memory.
    MapFailed,
    /// A write was asked of a segment the client attached read-only.
    ReadOnlySegment,
    /// Larger than `MAX_SEGMENT_BYTES`.
    TooLarge,
}

impl core::fmt::Display for AccessError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AccessError {}

pub type ReadError = AccessError;
pub type WriteError = AccessError;

#[derive(Debug)]
pub struct ReadOnlyMapping {
    address: NonNull<u8>,
    len: usize,
}

// SysV mappings belong to the process rather than the attaching thread. This
// wrapper exposes only immutable copies and owns the matching `shmdt`.
unsafe impl Send for ReadOnlyMapping {}
// Concurrent readers do not mutate the mapping or its lifetime. Client-side
// writes are synchronized by the X11 Present request before the authority
// snapshots bytes from this read-only view.
unsafe impl Sync for ReadOnlyMapping {}

impl ReadOnlyMapping {
    pub fn attach(shmid: u32) -> Result<Self, AccessError> {
        let shmid = libc::c_int::try_from(shmid).map_err(|_| AccessError::InvalidId)?;
        let len = segment_len(shmid)?;
        // SAFETY: a null address lets the kernel choose the mapping; SHM_RDONLY
        // prevents this adapter from mutating client-owned memory.
        let address = unsafe { libc::shmat(shmid, core::ptr::null(), libc::SHM_RDONLY) };
        let address = NonNull::new(address.cast::<u8>())
            .filter(|address| address.as_ptr() != (-1_isize) as *mut u8)
            .ok_or(AccessError::AttachFailed)?;
        Ok(Self { address, len })
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn copy_bytes(&self, offset: usize, len: usize) -> Result<Vec<u8>, AccessError> {
        let end = checked_end(offset, len, self.len)?;
        // SAFETY: checked_end established that offset..end lies inside the
        // live read-only mapping owned by self.
        Ok(unsafe {
            core::slice::from_raw_parts(self.address.as_ptr().add(offset), end - offset).to_vec()
        })
    }

    pub fn copy_rows(
        &self,
        offset: usize,
        stride: usize,
        row_offset: usize,
        row_bytes: usize,
        row_count: usize,
    ) -> Result<Vec<u8>, AccessError> {
        if row_bytes == 0 || row_count == 0 || row_offset.saturating_add(row_bytes) > stride {
            return Err(AccessError::OutOfBounds);
        }
        let byte_len = row_bytes
            .checked_mul(row_count)
            .ok_or(AccessError::RangeOverflow)?;
        let mut output = Vec::with_capacity(byte_len);
        for row in 0..row_count {
            let source = row
                .checked_mul(stride)
                .and_then(|value| value.checked_add(row_offset))
                .and_then(|value| value.checked_add(offset))
                .ok_or(AccessError::RangeOverflow)?;
            let end = checked_end(source, row_bytes, self.len)?;
            // SAFETY: checked_end established that source..end lies inside the
            // live read-only mapping owned by self.
            output.extend_from_slice(unsafe {
                core::slice::from_raw_parts(self.address.as_ptr().add(source), end - source)
            });
        }
        Ok(output)
    }
}

impl Drop for ReadOnlyMapping {
    fn drop(&mut self) {
        // SAFETY: address is exactly the live mapping returned by shmat and is
        // detached once when this owner is dropped.
        let _ = unsafe { libc::shmdt(self.address.as_ptr().cast()) };
    }
}

/// Shared memory named by a file descriptor, as MIT-SHM 1.2 passes it.
///
/// The length is taken from the descriptor when it is mapped and every access
/// is bounded by it, so a client that lies about a segment's size is refused
/// rather than obeyed. That check is the whole point: reading past the end of
/// a mapping is not an error the process can catch, it is a `SIGBUS`, and a
/// display server that dies because a client shortened a file it owns is a
/// denial of service with extra steps.
#[derive(Debug)]
pub struct DescriptorMapping {
    address: NonNull<u8>,
    len: usize,
    writable: bool,
}

// The mapping belongs to the process, not the mapping thread, and this wrapper
// hands out only copies. Same reasoning as `ReadOnlyMapping` above.
unsafe impl Send for DescriptorMapping {}
unsafe impl Sync for DescriptorMapping {}

impl DescriptorMapping {
    /// Maps a descriptor a client attached.
    ///
    /// `read_only` is the client's own declaration, and mapping `PROT_READ`
    /// when it is set means a later write cannot reach memory the client asked
    /// us not to touch even if some caller forgets to ask.
    pub fn map(descriptor: BorrowedFd<'_>, read_only: bool) -> Result<Self, AccessError> {
        let len = descriptor_len(descriptor)?;
        if len == 0 {
            return Err(AccessError::OutOfBounds);
        }
        let protection = if read_only {
            libc::PROT_READ
        } else {
            libc::PROT_READ | libc::PROT_WRITE
        };
        // SAFETY: a null address lets the kernel choose the placement, and the
        // length is the one `fstat` just reported for this descriptor.
        let address = unsafe {
            libc::mmap(
                core::ptr::null_mut(),
                len,
                protection,
                libc::MAP_SHARED,
                descriptor.as_raw_fd(),
                0,
            )
        };
        let address = NonNull::new(address.cast::<u8>())
            .filter(|address| address.as_ptr() != libc::MAP_FAILED.cast::<u8>())
            .ok_or(AccessError::MapFailed)?;
        Ok(Self {
            address,
            len,
            writable: !read_only,
        })
    }

    /// Allocates a segment the server owns and seals it against resizing.
    ///
    /// Sealing is what separates a segment that can be read safely from one a
    /// client can truncate underneath the read. It is available because the
    /// descriptor is ours: for a descriptor a client attached, the same
    /// guarantee cannot be assumed and the length check on every access is
    /// what stands in for it.
    ///
    /// Returns the mapping and the descriptor to hand back to the client.
    pub fn create_sealed(len: usize) -> Result<(Self, OwnedFd), AccessError> {
        if len == 0 || len > MAX_SEGMENT_BYTES {
            return Err(AccessError::TooLarge);
        }
        let name = c"sophia-shm";
        // SAFETY: `name` is a live NUL-terminated C string for this call.
        let raw = unsafe {
            libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING)
        };
        if raw < 0 {
            return Err(AccessError::MapFailed);
        }
        // SAFETY: memfd_create returned a fresh descriptor this owner now owns.
        let descriptor = unsafe { OwnedFd::from_raw_fd(raw) };
        let length = libc::off_t::try_from(len).map_err(|_| AccessError::TooLarge)?;
        // SAFETY: the descriptor is live and owned here.
        if unsafe { libc::ftruncate(descriptor.as_raw_fd(), length) } != 0 {
            return Err(AccessError::MapFailed);
        }
        // Neither direction may move again, so the length observed below is
        // the length for as long as the segment exists.
        // SAFETY: the descriptor is live and owned here.
        if unsafe {
            libc::fcntl(
                descriptor.as_raw_fd(),
                libc::F_ADD_SEALS,
                libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_SEAL,
            )
        } != 0
        {
            return Err(AccessError::MapFailed);
        }
        let mapping = Self::map(descriptor.as_fd(), false)?;
        Ok((mapping, descriptor))
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn is_writable(&self) -> bool {
        self.writable
    }

    pub fn copy_bytes(&self, offset: usize, len: usize) -> Result<Vec<u8>, AccessError> {
        // SAFETY: self.address is the live mapping of self.len bytes this
        // owner created and drops.
        unsafe { copy_region(self.address, self.len, offset, len) }
    }

    pub fn copy_rows(
        &self,
        offset: usize,
        stride: usize,
        row_offset: usize,
        row_bytes: usize,
        row_count: usize,
    ) -> Result<Vec<u8>, AccessError> {
        // SAFETY: as `copy_bytes`.
        unsafe {
            copy_region_rows(
                self.address,
                self.len,
                offset,
                stride,
                row_offset,
                row_bytes,
                row_count,
            )
        }
    }

    pub fn write_bytes(&self, offset: usize, bytes: &[u8]) -> Result<(), AccessError> {
        if !self.writable {
            return Err(AccessError::ReadOnlySegment);
        }
        checked_end(offset, bytes.len(), self.len)?;
        // SAFETY: checked_end established that the destination lies inside the
        // live writable mapping this owner created, and the source is a slice
        // that cannot overlap memory this process shares with the client.
        unsafe {
            core::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                self.address.as_ptr().add(offset),
                bytes.len(),
            );
        }
        Ok(())
    }
}

impl Drop for DescriptorMapping {
    fn drop(&mut self) {
        // SAFETY: address and len are exactly the mapping returned by mmap,
        // unmapped once when this owner is dropped.
        let _ = unsafe { libc::munmap(self.address.as_ptr().cast(), self.len) };
    }
}

/// Shared memory a client made available, however the client named it.
///
/// The two backings differ only in how they were obtained, so callers that
/// copy pixels should not have to care which one they hold. Keeping the choice
/// here rather than in the X frontend is what makes MIT-SHM 1.2 a second
/// variant of an existing path instead of a second path.
#[derive(Debug)]
pub enum ClientMapping {
    /// 1.1: named by SysV id, which `write_bytes` still needs to reattach for
    /// a write, since the read mapping is `SHM_RDONLY`.
    Sysv {
        shmid: u32,
        mapping: ReadOnlyMapping,
    },
    /// 1.2: named by descriptor.
    Descriptor(DescriptorMapping),
}

impl ClientMapping {
    pub fn attach_sysv(shmid: u32) -> Result<Self, AccessError> {
        Ok(Self::Sysv {
            shmid,
            mapping: ReadOnlyMapping::attach(shmid)?,
        })
    }

    pub const fn len(&self) -> usize {
        match self {
            Self::Sysv { mapping, .. } => mapping.len(),
            Self::Descriptor(mapping) => mapping.len(),
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn copy_bytes(&self, offset: usize, len: usize) -> Result<Vec<u8>, AccessError> {
        match self {
            Self::Sysv { mapping, .. } => mapping.copy_bytes(offset, len),
            Self::Descriptor(mapping) => mapping.copy_bytes(offset, len),
        }
    }

    pub fn copy_rows(
        &self,
        offset: usize,
        stride: usize,
        row_offset: usize,
        row_bytes: usize,
        row_count: usize,
    ) -> Result<Vec<u8>, AccessError> {
        match self {
            Self::Sysv { mapping, .. } => {
                mapping.copy_rows(offset, stride, row_offset, row_bytes, row_count)
            }
            Self::Descriptor(mapping) => {
                mapping.copy_rows(offset, stride, row_offset, row_bytes, row_count)
            }
        }
    }

    pub fn write_bytes(&self, offset: usize, bytes: &[u8]) -> Result<(), AccessError> {
        match self {
            Self::Sysv { shmid, .. } => write_bytes(*shmid, offset, bytes),
            Self::Descriptor(mapping) => mapping.write_bytes(offset, bytes),
        }
    }
}

fn descriptor_len(descriptor: BorrowedFd<'_>) -> Result<usize, AccessError> {
    let mut status = core::mem::MaybeUninit::<libc::stat>::zeroed();
    // SAFETY: status points to writable storage for `fstat`.
    if unsafe { libc::fstat(descriptor.as_raw_fd(), status.as_mut_ptr()) } != 0 {
        return Err(AccessError::MissingSegment);
    }
    // SAFETY: a successful fstat initialized the complete `stat` value.
    let status = unsafe { status.assume_init() };
    let len = usize::try_from(status.st_size).map_err(|_| AccessError::TooLarge)?;
    if len > MAX_SEGMENT_BYTES {
        return Err(AccessError::TooLarge);
    }
    Ok(len)
}

pub fn copy_bytes(shmid: u32, offset: usize, len: usize) -> Result<Vec<u8>, ReadError> {
    ReadOnlyMapping::attach(shmid)?.copy_bytes(offset, len)
}

pub fn write_bytes(shmid: u32, offset: usize, bytes: &[u8]) -> Result<(), WriteError> {
    let shmid = libc::c_int::try_from(shmid).map_err(|_| AccessError::InvalidId)?;
    checked_end(offset, bytes.len(), segment_len(shmid)?)?;

    // SAFETY: a null address lets the kernel choose a writable mapping.
    let address = unsafe { libc::shmat(shmid, core::ptr::null(), 0) };
    if address == (-1_isize) as *mut libc::c_void {
        return Err(AccessError::AttachFailed);
    }
    // SAFETY: IPC_STAT established that offset..end lies inside the writable
    // mapping, which remains attached for the duration of the copy.
    unsafe {
        core::ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            (address as *mut u8).add(offset),
            bytes.len(),
        );
    }
    // SAFETY: address is exactly the live mapping returned by shmat above.
    if unsafe { libc::shmdt(address) } != 0 {
        return Err(AccessError::DetachFailed);
    }
    Ok(())
}

/// Copies `len` bytes at `offset` out of a live mapping.
///
/// # Safety
///
/// `address` must be the base of a readable mapping of at least `available`
/// bytes, live for this call.
unsafe fn copy_region(
    address: NonNull<u8>,
    available: usize,
    offset: usize,
    len: usize,
) -> Result<Vec<u8>, AccessError> {
    let end = checked_end(offset, len, available)?;
    // SAFETY: checked_end established that offset..end lies inside the mapping
    // the caller promised is live and at least `available` bytes long.
    Ok(unsafe { core::slice::from_raw_parts(address.as_ptr().add(offset), end - offset).to_vec() })
}

/// Copies `row_count` rows of `row_bytes` out of a live mapping.
///
/// # Safety
///
/// As `copy_region`.
unsafe fn copy_region_rows(
    address: NonNull<u8>,
    available: usize,
    offset: usize,
    stride: usize,
    row_offset: usize,
    row_bytes: usize,
    row_count: usize,
) -> Result<Vec<u8>, AccessError> {
    if row_bytes == 0 || row_count == 0 || row_offset.saturating_add(row_bytes) > stride {
        return Err(AccessError::OutOfBounds);
    }
    let byte_len = row_bytes
        .checked_mul(row_count)
        .ok_or(AccessError::RangeOverflow)?;
    let mut output = Vec::with_capacity(byte_len);
    for row in 0..row_count {
        let source = row
            .checked_mul(stride)
            .and_then(|value| value.checked_add(row_offset))
            .and_then(|value| value.checked_add(offset))
            .ok_or(AccessError::RangeOverflow)?;
        let end = checked_end(source, row_bytes, available)?;
        // SAFETY: checked_end established that source..end lies inside the
        // mapping the caller promised is live.
        output.extend_from_slice(unsafe {
            core::slice::from_raw_parts(address.as_ptr().add(source), end - source)
        });
    }
    Ok(output)
}

fn segment_len(shmid: libc::c_int) -> Result<usize, AccessError> {
    let mut metadata = core::mem::MaybeUninit::<libc::shmid_ds>::zeroed();
    // SAFETY: metadata points to writable storage for `shmctl(IPC_STAT)`.
    if unsafe { libc::shmctl(shmid, libc::IPC_STAT, metadata.as_mut_ptr()) } != 0 {
        return Err(AccessError::MissingSegment);
    }
    // SAFETY: successful IPC_STAT initialized the complete `shmid_ds` value.
    Ok(unsafe { metadata.assume_init() }.shm_segsz)
}

fn checked_end(offset: usize, len: usize, available: usize) -> Result<usize, AccessError> {
    let end = offset.checked_add(len).ok_or(AccessError::RangeOverflow)?;
    (end <= available)
        .then_some(end)
        .ok_or(AccessError::OutOfBounds)
}
