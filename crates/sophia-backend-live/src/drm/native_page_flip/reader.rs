#[cfg(feature = "libdrm-events")]
use super::*;
#[cfg(feature = "libdrm-events")]
use std::{collections::VecDeque, io};

#[cfg(feature = "libdrm-events")]
pub trait LibdrmNativePageFlipReader {
    fn read_ready_page_flip_callbacks(&mut self, max_read: usize)
    -> LibdrmNativePageFlipReadResult;

    fn read_ready_page_flip_callbacks_into(
        &mut self,
        max_read: usize,
        callbacks: &mut Vec<LibdrmNativePageFlipCallback>,
    ) -> LibdrmNativeReadLoopReport {
        let result = self.read_ready_page_flip_callbacks(max_read);
        callbacks.extend(result.callbacks);
        result.report
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibdrmNativePageFlipReadResult {
    pub report: LibdrmNativeReadLoopReport,
    pub callbacks: Vec<LibdrmNativePageFlipCallback>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FakeLibdrmNativePageFlipReader {
    queued: VecDeque<LibdrmNativePageFlipCallback>,
    fail_next_read: bool,
}

#[cfg(feature = "libdrm-events")]
impl FakeLibdrmNativePageFlipReader {
    pub fn new(callbacks: impl IntoIterator<Item = LibdrmNativePageFlipCallback>) -> Self {
        Self {
            queued: callbacks.into_iter().collect(),
            fail_next_read: false,
        }
    }

    pub fn fail_next_read(&mut self) {
        self.fail_next_read = true;
    }

    pub fn queued_len(&self) -> usize {
        self.queued.len()
    }
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePageFlipReader for FakeLibdrmNativePageFlipReader {
    fn read_ready_page_flip_callbacks(
        &mut self,
        max_read: usize,
    ) -> LibdrmNativePageFlipReadResult {
        if self.fail_next_read {
            self.fail_next_read = false;
            return LibdrmNativePageFlipReadResult {
                report: LibdrmNativeReadLoopReport::read_failed(),
                callbacks: Vec::new(),
            };
        }

        let mut callbacks = Vec::new();
        for _ in 0..max_read {
            let Some(callback) = self.queued.pop_front() else {
                break;
            };
            callbacks.push(callback);
        }

        LibdrmNativePageFlipReadResult {
            report: LibdrmNativeReadLoopReport::callbacks_decoded(callbacks.len(), 0)
                .unwrap_or_else(LibdrmNativeReadLoopReport::would_block),
            callbacks,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct NativeLibdrmPageFlipEventReader<D> {
    device: D,
    crtc_routes: Vec<LibdrmNativeCrtcRoute>,
}

#[cfg(feature = "libdrm-events")]
impl<D> NativeLibdrmPageFlipEventReader<D> {
    pub fn new(device: D) -> Self {
        Self {
            device,
            crtc_routes: Vec::new(),
        }
    }

    pub fn with_crtc_routes(
        mut self,
        routes: impl IntoIterator<Item = LibdrmNativeCrtcRoute>,
    ) -> Self {
        self.replace_crtc_routes(routes);
        self
    }

    pub fn replace_crtc_routes(&mut self, routes: impl IntoIterator<Item = LibdrmNativeCrtcRoute>) {
        self.crtc_routes.clear();
        self.crtc_routes.extend(routes);
    }

    pub fn crtc_route_count(&self) -> usize {
        self.crtc_routes.len()
    }
}

#[cfg(feature = "libdrm-events")]
impl<D> LibdrmNativePageFlipReader for NativeLibdrmPageFlipEventReader<D>
where
    D: drm::control::Device,
{
    fn read_ready_page_flip_callbacks(
        &mut self,
        max_read: usize,
    ) -> LibdrmNativePageFlipReadResult {
        let mut callbacks = Vec::with_capacity(max_read);
        let report = self.read_ready_page_flip_callbacks_into(max_read, &mut callbacks);
        LibdrmNativePageFlipReadResult { report, callbacks }
    }

    fn read_ready_page_flip_callbacks_into(
        &mut self,
        max_read: usize,
        callbacks: &mut Vec<LibdrmNativePageFlipCallback>,
    ) -> LibdrmNativeReadLoopReport {
        if max_read == 0 {
            return LibdrmNativeReadLoopReport::would_block();
        }

        let events = match self.device.receive_events() {
            Ok(events) => events,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return LibdrmNativeReadLoopReport::would_block();
            }
            Err(_) => {
                return LibdrmNativeReadLoopReport::read_failed();
            }
        };

        let initial_callback_count = callbacks.len();
        let mut rejected_callbacks = 0usize;

        // `receive_events` has already consumed its complete kernel buffer.
        // Limiting this iterator would silently discard the tail, so the
        // service budget limits read syscalls instead of events in a returned
        // batch. `max_read` remains the zero/read admission gate and reserve
        // hint for generic readers.
        callbacks.reserve(max_read);
        for event in events {
            let drm::control::Event::PageFlip(page_flip) = event else {
                continue;
            };

            match reduce_native_page_flip_event(&page_flip, &mut self.crtc_routes) {
                Some(callback) => callbacks.push(callback),
                None => rejected_callbacks = rejected_callbacks.saturating_add(1),
            }
        }

        LibdrmNativeReadLoopReport::callbacks_decoded(
            callbacks.len().saturating_sub(initial_callback_count),
            rejected_callbacks,
        )
        .unwrap_or_else(LibdrmNativeReadLoopReport::would_block)
    }
}
