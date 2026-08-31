use crate::prelude::*;
use std::os::fd::OwnedFd;

#[derive(Debug)]
pub struct LibdrmNativePrimaryPlaneScanoutSubmission {
    pub(crate) resources: LibdrmNativePrimaryPlaneResourceBundle,
    pub(crate) completion_fence: Option<OwnedFd>,
}

impl LibdrmNativePrimaryPlaneScanoutSubmission {
    pub fn completion_fence_status(&self) -> io::Result<LibdrmNativeCompletionFenceStatus> {
        let Some(fence) = self.completion_fence.as_ref() else {
            return Ok(LibdrmNativeCompletionFenceStatus::Unsupported);
        };
        let mut poll_fds = [rustix::event::PollFd::new(
            fence,
            rustix::event::PollFlags::IN,
        )];
        let no_wait = rustix::event::Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        rustix::event::poll(&mut poll_fds, Some(&no_wait))?;
        let ready = poll_fds[0].revents();
        if ready.contains(rustix::event::PollFlags::NVAL) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "DRM completion fence descriptor is invalid",
            ));
        }
        if ready.intersects(
            rustix::event::PollFlags::IN
                | rustix::event::PollFlags::ERR
                | rustix::event::PollFlags::HUP,
        ) {
            Ok(LibdrmNativeCompletionFenceStatus::Signaled)
        } else {
            Ok(LibdrmNativeCompletionFenceStatus::Pending)
        }
    }

    pub(crate) fn clear_completion_fence(&mut self) {
        self.completion_fence = None;
    }

    pub fn retire<D>(self, device: &D) -> LibdrmNativePrimaryPlaneResourceDestroyReport
    where
        D: LibdrmNativePrimaryPlaneResourceDevice,
    {
        destroy_native_primary_plane_resources(device, self.resources)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeCompletionFenceStatus {
    Unsupported,
    Pending,
    Signaled,
}
