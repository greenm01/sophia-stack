//! Narrow safe adapter for DRM's `OUT_FENCE_PTR` ownership ABI.

use drm::control::Device;
use std::io;
use std::os::fd::{FromRawFd, OwnedFd};

/// Submit one atomic request and capture the sync-file descriptor written by
/// the kernel into the CRTC's `OUT_FENCE_PTR` property.
///
/// `request` must not already contain `out_fence_property`. The slot remains
/// alive for the entire synchronous ioctl and closes a descriptor even if a
/// driver writes one before returning an error.
pub fn atomic_commit_with_out_fence<D>(
    device: &D,
    flags: drm::control::AtomicCommitFlags,
    mut request: drm::control::atomic::AtomicModeReq,
    crtc: drm::control::crtc::Handle,
    out_fence_property: drm::control::property::Handle,
) -> io::Result<Option<OwnedFd>>
where
    D: Device,
{
    let mut slot = KernelOwnedFdSlot::new();
    request.add_property(
        crtc,
        out_fence_property,
        drm::control::property::Value::Unknown(slot.pointer_value()),
    );
    device.atomic_commit(flags, request)?;
    Ok(slot.take())
}

#[derive(Debug)]
struct KernelOwnedFdSlot {
    raw: i32,
}

impl KernelOwnedFdSlot {
    const fn new() -> Self {
        Self { raw: -1 }
    }

    fn pointer_value(&mut self) -> u64 {
        std::ptr::from_mut(&mut self.raw).addr() as u64
    }

    fn take(&mut self) -> Option<OwnedFd> {
        let raw = std::mem::replace(&mut self.raw, -1);
        if raw < 0 {
            return None;
        }
        // SAFETY: OUT_FENCE_PTR returns a new descriptor owned by the caller.
        Some(unsafe { OwnedFd::from_raw_fd(raw) })
    }
}

impl Drop for KernelOwnedFdSlot {
    fn drop(&mut self) {
        // If the ioctl failed after writing a descriptor, it is still closed.
        drop(self.take());
    }
}
