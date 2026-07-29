//! Narrow safe adapter for copying bytes from an existing SysV SHM segment.

use core::ptr::NonNull;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessError {
    InvalidId,
    MissingSegment,
    RangeOverflow,
    OutOfBounds,
    AttachFailed,
    DetachFailed,
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
