//! The exporter as the scanout pipeline sees it.
//!
//! Split from `discovery.rs`, which owns the exporter's state and the long
//! export procedure itself. What lives here is the trait boundary: the
//! episode bookkeeping the pipeline drives, and the timing wrapper that
//! files each export under the path it actually took.
//!
//! Kept apart because the procedure and the boundary change for different
//! reasons -- one when the fallback ladder does, the other when the pipeline
//! asks the exporter something new.

use super::discovery::NativeGbmRenderedScanoutBufferDiscoveryExporter;
use super::{LiveRenderedScanoutBufferExport, LiveRenderedScanoutBufferExporter};
use crate::api::*;

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
impl<R> LiveRenderedScanoutBufferExporter for NativeGbmRenderedScanoutBufferDiscoveryExporter<R>
where
    R: RenderDeviceDiscoveryBackend,
{
    type Owner = NativeGbmRenderedScanoutOwner;

    fn direct_scanout_test_required(&self) -> bool {
        !self.direct_scanout_tested
    }

    fn record_direct_scanout_test(&mut self, accepted: bool) {
        self.direct_scanout_tests = self.direct_scanout_tests.saturating_add(1);
        self.record_direct_scanout_episode(
            if accepted {
                "test_passed"
            } else {
                "test_rejected"
            },
            self.outstanding_direct_generation(),
            "none",
        );
        if accepted {
            self.direct_scanout_tested = true;
        } else {
            self.direct_scanout_test_rejections =
                self.direct_scanout_test_rejections.saturating_add(1);
            self.direct_scanout_tested = false;
        }
    }

    fn commit_direct_scanout(&mut self) {
        // Read before committing: committing drops the composed form the
        // identity is read from.
        let generation = self.outstanding_direct_generation();
        Self::commit_direct_scanout(self);
        self.record_direct_scanout_episode("flipped", generation, "none");
    }

    fn fall_back_from_direct(&mut self) -> bool {
        // A refusal ends the episode: whatever the driver objected to, the
        // next direct frame is a fresh question and gets a fresh test.
        self.direct_scanout_tested = false;
        let generation = self.outstanding_direct_generation();
        let fell_back = Self::fall_back_from_direct(self);
        if fell_back {
            self.record_direct_scanout_episode("fell_back", generation, "none");
        }
        fell_back
    }

    /// Times the export, and files the result under the path it took.
    ///
    /// This is the whole of what direct scanout skips: a composed frame
    /// waits for the renderer and pays for the draw between the moment its
    /// content was offered and the moment a buffer is ready to submit, while
    /// a direct frame pays only the export path. Which population an export
    /// belongs to is not known until it finishes -- the direct attempt can
    /// fall back -- so it is read from the flip counter afterwards rather
    /// than predicted.
    fn export_rendered_scanout_buffer(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
    ) -> LiveRenderedScanoutBufferExport<Self::Owner> {
        let offered = self.frame_offered_at.take();
        let direct_flips_before = self.direct_scanout_flips();
        let export = self.export_rendered_scanout_buffer_measured(target);
        if let Some(offered) = offered {
            let direct = self.direct_scanout_flips() > direct_flips_before;
            self.cost.record_offer_to_submit(direct, offered.elapsed());
        }
        export
    }
}
