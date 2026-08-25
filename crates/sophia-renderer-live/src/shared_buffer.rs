//! Buffers the compositor originates on a client's behalf.
//!
//! DRI3 has two halves. In one the client allocates and hands over descriptors;
//! in the other it expects the server to own the storage and asks for the
//! descriptors back. This is the allocation the second half needs, kept in the
//! crate that owns the device rather than in the protocol authority, which owns
//! no GPU state and should not begin to.

use sophia_protocol::DmaBufDescriptor;
use std::os::fd::OwnedFd;

/// A buffer allocated for a client, and the descriptors that reach it.
#[derive(Debug)]
pub struct LiveSharedBufferAllocation {
    pub descriptor: DmaBufDescriptor,
    /// One per plane, in plane order, matching the descriptor's plane count.
    pub plane_fds: Vec<OwnedFd>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveSharedBufferError {
    /// The extent or depth is not one this allocator backs.
    UnsupportedTarget,
    /// The device rejected the allocation.
    DeviceRejected,
    /// The buffer exists but its descriptors could not be exported, which is
    /// the same as having no buffer from the client's point of view.
    ExportFailed,
}

/// The formats a depth maps to, matching what the authority's import path
/// accepts so a pixmap cannot be originated at one format and recovered at
/// another.
#[cfg(feature = "gbm-probe")]
const fn format_for_depth(depth: u8) -> Option<u32> {
    match depth {
        24 => Some(sophia_protocol::DRM_FORMAT_XRGB8888),
        32 => Some(sophia_protocol::DRM_FORMAT_ARGB8888),
        _ => None,
    }
}

#[cfg(feature = "gbm-probe")]
pub fn allocate_shared_buffer<T: std::os::fd::AsFd>(
    device: T,
    handle: u64,
    size: sophia_protocol::Size,
    depth: u8,
) -> Result<LiveSharedBufferAllocation, LiveSharedBufferError> {
    let format = format_for_depth(depth).ok_or(LiveSharedBufferError::UnsupportedTarget)?;
    let width = u32::try_from(size.width).map_err(|_| LiveSharedBufferError::UnsupportedTarget)?;
    let height =
        u32::try_from(size.height).map_err(|_| LiveSharedBufferError::UnsupportedTarget)?;
    if width == 0 || height == 0 {
        return Err(LiveSharedBufferError::UnsupportedTarget);
    }
    let gbm_format = if depth == 32 {
        gbm::Format::Argb8888
    } else {
        gbm::Format::Xrgb8888
    };
    // RENDERING because the client draws into it; SCANOUT is deliberately not
    // requested, so a buffer that cannot be scanned out is still allocated and
    // composited rather than refused.
    let usage = gbm::BufferObjectFlags::RENDERING;

    let device = gbm::Device::new(device).map_err(|_| LiveSharedBufferError::DeviceRejected)?;
    let buffer = device
        .create_buffer_object::<()>(width, height, gbm_format, usage)
        .map_err(|_| LiveSharedBufferError::DeviceRejected)?;

    let plane_count = buffer.plane_count();
    if plane_count == 0 || plane_count as usize > sophia_protocol::DMA_BUF_MAX_PLANES {
        return Err(LiveSharedBufferError::UnsupportedTarget);
    }
    let mut planes: [Option<sophia_protocol::DmaBufPlaneDescriptor>;
        sophia_protocol::DMA_BUF_MAX_PLANES] = [None; sophia_protocol::DMA_BUF_MAX_PLANES];
    let mut plane_fds = Vec::with_capacity(plane_count as usize);
    for index in 0..plane_count as i32 {
        plane_fds.push(
            buffer
                .fd_for_plane(index)
                .map_err(|_| LiveSharedBufferError::ExportFailed)?,
        );
        planes[index as usize] = Some(sophia_protocol::DmaBufPlaneDescriptor {
            offset: buffer.offset(index),
            stride: buffer.stride_for_plane(index),
        });
    }

    Ok(LiveSharedBufferAllocation {
        descriptor: DmaBufDescriptor {
            handle: sophia_protocol::BufferHandle::from_raw(handle),
            size,
            format,
            modifier: u64::from(buffer.modifier()),
            plane_count: plane_count as u8,
            planes,
        },
        plane_fds,
    })
}
