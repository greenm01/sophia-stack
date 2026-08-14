use super::*;

#[derive(Debug)]
pub struct LiveProductionRendererImageHandoff {
    output: OutputId,
    expected: Vec<sophia_renderer_live::LiveRendererImageId>,
    snapshots: Vec<sophia_renderer_live::LiveRendererImageSnapshot>,
}

impl LiveProductionRendererImageHandoff {
    pub const fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    pub fn image_ids(&self) -> &[sophia_renderer_live::LiveRendererImageId] {
        &self.expected
    }
}

fn validate_renderer_image_handoff_ids(
    expected: &[sophia_renderer_live::LiveRendererImageId],
    actual: &[sophia_renderer_live::LiveRendererImageId],
) -> Result<(), &'static str> {
    match crate::reduce_live_renderer_image_handoff_admission(expected, Some(actual)) {
        crate::LiveRendererImageHandoffAdmission::Ready => Ok(()),
        crate::LiveRendererImageHandoffAdmission::InvalidIdentity => {
            Err("renderer-image handoff contains an invalid image identity")
        }
        crate::LiveRendererImageHandoffAdmission::DuplicateIdentity => {
            Err("renderer-image handoff contains a duplicate image identity")
        }
        crate::LiveRendererImageHandoffAdmission::CoverageMismatch => {
            Err("renderer-image handoff does not cover the retained scene")
        }
        crate::LiveRendererImageHandoffAdmission::Missing => {
            Err("renderer-image handoff is unexpectedly missing")
        }
    }
}

impl LiveProductionNativeScanout {
    pub fn queue_present_cpu_frame(
        &mut self,
        output: OutputId,
        frame: LiveProductionComposedFrame,
    ) -> Result<LiveProductionNativeFrameId, &'static str> {
        let index = self
            .primary_head_index(output)
            .ok_or("native output has no head")?;
        if self
            .exporter(output)
            .is_some_and(crate::NativeGbmRenderedScanoutBufferDiscoveryExporter::pending_frame)
        {
            return Err("native output already has pending frame work");
        }
        let frame_id = self.allocate_frame_id();
        let (head, exporter) = self.head_and_exporter(index, output);
        head.pending_nonzero_pixel_bytes = frame.nonzero_pixel_bytes;
        head.last_checksum = frame.checksum;
        head.queue_output_damage_snapshot(frame.output_damage_snapshot.clone());
        head.pending_content = Some(LiveProductionScanoutContent::Cpu {
            frame: frame_id,
            checksum: frame.checksum,
        });
        exporter.set_pending_cpu_frame_with_damage(
            frame.frame,
            frame.checksum,
            frame.output_damage_snapshot,
        );
        Ok(frame_id)
    }

    pub fn queue_mixed_frame(
        &mut self,
        output: OutputId,
        transaction: TransactionId,
        frame: crate::LiveOwnedMixedCompositionFrame,
    ) -> LiveProductionNativeFrameId {
        let index = self
            .primary_head_index(output)
            .expect("native mixed frame targets a registered output");
        let frame_id = self.allocate_frame_id();
        let (head, exporter) = self.head_and_exporter(index, output);
        let pending_before = exporter.pending_frame();
        let worker_in_flight = exporter.worker_in_flight();
        if let Some(superseded) = head.pending_content {
            tracing::warn!(
                "sophia_live_native_scanout schema=1 status=superseded output={} old={superseded:?} new=Mixed({})",
                head.output.id.raw(),
                transaction.raw(),
            );
        }
        head.pending_content = Some(LiveProductionScanoutContent::MixedPresent {
            frame: frame_id,
            transaction,
            nonzero_rgb_pixels: 0,
        });
        head.queue_output_damage_snapshot(frame.output_damage_snapshot.clone());
        exporter.set_pending_mixed_frame(frame);
        tracing::debug!(
            "sophia_live_retained_projection schema=1 status=native_queued output={} frame={} pending_before={} worker_in_flight={}",
            head.output.id.raw(),
            frame_id.raw(),
            pending_before,
            worker_in_flight,
        );
        frame_id
    }

    /// Queues one composed frame onto every head of a logical output, each at the
    /// rect that frame lands on for that head.
    ///
    /// This is mirroring's whole visual behaviour. A group's heads show one
    /// *scene*, not one buffer: the same composed pixels are placed into each
    /// head's own buffer at its own mode, scaled and centred by `fit`. Nothing is
    /// captured and nothing is composed twice -- the frame is already a single
    /// flat buffer, and the placement is what differs per head.
    ///
    /// It goes through the mixed door rather than the CPU one deliberately. The
    /// pure-CPU path carries no destination rect and would upload the frame at its
    /// own size, which is right for a head whose mode matches the scene and wrong
    /// for every other head of a group.
    ///
    /// Returns how many heads took the frame.
    pub fn queue_projected_frame(
        &mut self,
        output: OutputId,
        frame: &sophia_renderer_live::LiveCpuComposedFrame,
        fit: crate::NativeMirrorFit,
    ) -> usize {
        let heads = self.head_indices(output);
        let mut queued = 0usize;
        for head_index in heads {
            let destination = self.heads[head_index].output.size;
            let target = crate::project_mirror_rect(frame.size, destination, fit);
            if target.width <= 0 || target.height <= 0 {
                continue;
            }
            let layer = sophia_renderer_live::LiveOwnedMixedCompositionLayer::Cpu {
                buffer: sophia_renderer_live::LiveCpuBufferSource {
                    handle: 0,
                    size: frame.size,
                    stride: frame.stride,
                    format: frame.format,
                    generation: 0,
                    bytes: frame.bytes.as_ref().clone(),
                },
                placement: sophia_renderer_live::LiveCompositionPlacement {
                    target,
                    clip: None,
                    transform: sophia_protocol::Transform::IDENTITY,
                    alpha: 1.0,
                },
            };
            let frame_id = self.allocate_frame_id();
            let (head, exporter) = self.head_and_exporter(head_index, output);
            head.pending_content = Some(LiveProductionScanoutContent::Cpu {
                frame: frame_id,
                checksum: 0,
            });
            exporter.set_pending_mixed_frame(
                sophia_renderer_live::LiveOwnedMixedCompositionFrame {
                    layers: vec![layer],
                    output_damage_snapshot: None,
                },
            );
            queued = queued.saturating_add(1);
        }
        queued
    }

    pub fn queue_retained_mixed_frame(
        &mut self,
        output: OutputId,
        frame: crate::LiveOwnedMixedCompositionFrame,
    ) -> LiveProductionNativeFrameId {
        let index = self
            .primary_head_index(output)
            .expect("native retained frame targets a registered output");
        let frame_id = self.allocate_frame_id();
        let (head, exporter) = self.head_and_exporter(index, output);
        if let Some(superseded) = head.pending_content {
            tracing::warn!(
                "sophia_live_native_scanout schema=1 status=superseded output={} old={superseded:?} new=RetainedMixed",
                head.output.id.raw(),
            );
        }
        head.pending_content = Some(LiveProductionScanoutContent::RetainedMixed {
            frame: frame_id,
            nonzero_rgb_pixels: 0,
        });
        head.queue_output_damage_snapshot(frame.output_damage_snapshot.clone());
        exporter.set_pending_mixed_frame(frame);
        frame_id
    }

    pub fn diagnose_mixed_frame(
        &mut self,
        output: OutputId,
        frame: crate::LiveOwnedMixedCompositionFrame,
    ) -> (
        crate::LiveRendererScanoutBufferExportStatus,
        crate::LiveRendererScanoutBufferExportDetail,
    ) {
        use crate::LiveRenderedScanoutBufferExporter as _;

        let index = self
            .primary_head_index(output)
            .expect("native mixed-frame diagnosis targets a registered output");
        let (head, exporter) = self.head_and_exporter(index, output);
        exporter.set_pending_mixed_frame(frame);
        let size = head.output.size;
        let export =
            exporter.export_rendered_scanout_buffer(crate::LiveGbmEglFrameTargetRecord::new(size));
        let status = export.status;
        let detail = export.detail;
        drop(export);
        (status, detail)
    }

    pub fn evict_renderer_image(
        &mut self,
        image_id: sophia_renderer_live::LiveRendererImageId,
    ) -> Result<usize, crate::LiveRendererScanoutBufferExportDetail> {
        let mut evicted = 0usize;
        for exporter in self.exporters.iter_mut() {
            evicted = evicted.saturating_add(usize::from(exporter.evict_renderer_image(image_id)?));
        }
        Ok(evicted)
    }

    pub fn promote_renderer_image(
        &mut self,
        image_id: sophia_renderer_live::LiveRendererImageId,
    ) -> Result<usize, crate::LiveRendererScanoutBufferExportDetail> {
        let mut promoted = 0usize;
        for exporter in self.exporters.iter_mut() {
            promoted =
                promoted.saturating_add(usize::from(exporter.promote_renderer_image(image_id)?));
        }
        Ok(promoted)
    }

    pub fn rollback_renderer_image(
        &mut self,
        image_id: sophia_renderer_live::LiveRendererImageId,
    ) -> Result<usize, crate::LiveRendererScanoutBufferExportDetail> {
        let mut rolled_back = 0usize;
        for exporter in self.exporters.iter_mut() {
            rolled_back = rolled_back
                .saturating_add(usize::from(exporter.rollback_renderer_image(image_id)?));
        }
        Ok(rolled_back)
    }

    pub fn export_renderer_image_handoff(
        &mut self,
        output: OutputId,
        expected: &[sophia_renderer_live::LiveRendererImageId],
    ) -> Result<LiveProductionRendererImageHandoff, Box<dyn std::error::Error>> {
        validate_renderer_image_handoff_ids(expected, expected)?;
        let mut snapshots = Vec::with_capacity(expected.len());
        for image_id in expected {
            let snapshot = self
                .exporter_mut(output)?
                .export_promoted_renderer_image(*image_id)?
                .ok_or("retained scene refers to an unavailable promoted renderer image")?;
            snapshots.push(snapshot);
        }
        let actual = snapshots
            .iter()
            .map(sophia_renderer_live::LiveRendererImageSnapshot::image_id)
            .collect::<Vec<_>>();
        validate_renderer_image_handoff_ids(expected, &actual)?;
        Ok(LiveProductionRendererImageHandoff {
            output,
            expected: expected.to_vec(),
            snapshots,
        })
    }

    pub fn restore_renderer_image_handoff(
        &mut self,
        handoff: LiveProductionRendererImageHandoff,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        if !self
            .exporter_mut(handoff.output)?
            .renderer_image_owner_initialized()
        {
            return Err("replacement renderer image owner is not initialized".into());
        }
        let actual = handoff
            .snapshots
            .iter()
            .map(sophia_renderer_live::LiveRendererImageSnapshot::image_id)
            .collect::<Vec<_>>();
        validate_renderer_image_handoff_ids(&handoff.expected, &actual)?;
        let expected_count = handoff.expected.len();
        for snapshot in handoff.snapshots {
            if !self
                .exporter_mut(handoff.output)?
                .restore_promoted_renderer_image(snapshot)?
            {
                return Err("replacement renderer rejected a retained image snapshot".into());
            }
        }
        Ok(expected_count)
    }

    /// Every logical output's exporter has an initialized renderer-image owner.
    ///
    /// Counted over exporters rather than heads: a mirror group has one exporter
    /// and several heads, so counting heads would ask the same exporter twice and
    /// call an empty desktop initialized.
    pub fn renderer_image_owners_initialized(&self) -> bool {
        !self.exporters.is_empty()
            && self
                .exporters
                .iter()
                .all(crate::NativeGbmRenderedScanoutBufferDiscoveryExporter::renderer_image_owner_initialized)
    }

    pub fn clear_renderer_images(
        &mut self,
    ) -> Result<usize, crate::LiveRendererScanoutBufferExportDetail> {
        let mut evicted = 0usize;
        for exporter in self.exporters.iter_mut() {
            evicted = evicted.saturating_add(exporter.clear_renderer_images()?);
        }
        Ok(evicted)
    }

    pub fn export_attempts(&self) -> usize {
        self.exporters
            .iter()
            .map(crate::NativeGbmRenderedScanoutBufferDiscoveryExporter::cpu_frame_export_attempts)
            .chain(self.exporters.iter().map(
                crate::NativeGbmRenderedScanoutBufferDiscoveryExporter::mixed_frame_export_attempts,
            ))
            .sum()
    }

    pub fn mixed_exports(&self) -> usize {
        self.exporters
            .iter()
            .map(crate::NativeGbmRenderedScanoutBufferDiscoveryExporter::mixed_frame_exports)
            .sum()
    }

    pub fn persistent_render_metrics(&self) -> LivePersistentRenderMetrics {
        self.exporters.iter().fold(
            LivePersistentRenderMetrics::default(),
            |mut metrics, exporter| {
                let stats = exporter.persistent_render_stats();
                metrics.target_creations = metrics
                    .target_creations
                    .saturating_add(stats.target_creations);
                metrics.target_recreations = metrics
                    .target_recreations
                    .saturating_add(stats.target_recreations);
                metrics.pipeline_creations = metrics
                    .pipeline_creations
                    .saturating_add(stats.gl_pipeline_creations);
                metrics.frame_surface_creations = metrics
                    .frame_surface_creations
                    .saturating_add(stats.frame_surface_creations);
                metrics.cpu_target_creations = metrics
                    .cpu_target_creations
                    .saturating_add(stats.cpu_target_creations);
                metrics.dmabuf_target_creations = metrics
                    .dmabuf_target_creations
                    .saturating_add(stats.dmabuf_target_creations);
                metrics.composition_target_creations = metrics
                    .composition_target_creations
                    .saturating_add(stats.composition_target_creations);
                metrics.composition_target_reuses = metrics
                    .composition_target_reuses
                    .saturating_add(stats.composition_target_reuses);
                metrics.generation_replacements = metrics
                    .generation_replacements
                    .saturating_add(stats.generation_replacements);
                metrics.recovery_replacements = metrics
                    .recovery_replacements
                    .saturating_add(stats.recovery_replacements);
                metrics.uploads = metrics.uploads.saturating_add(stats.frame_uploads);
                metrics.snapshot_captures = metrics
                    .snapshot_captures
                    .saturating_add(stats.snapshot_captures);
                metrics.snapshot_promotions = metrics
                    .snapshot_promotions
                    .saturating_add(stats.snapshot_promotions);
                metrics.snapshot_rollbacks = metrics
                    .snapshot_rollbacks
                    .saturating_add(stats.snapshot_rollbacks);
                metrics.snapshot_evictions = metrics
                    .snapshot_evictions
                    .saturating_add(stats.snapshot_evictions);
                metrics.snapshot_live_entries = metrics
                    .snapshot_live_entries
                    .saturating_add(stats.snapshot_live_entries);
                metrics.snapshot_live_bytes = metrics
                    .snapshot_live_bytes
                    .saturating_add(stats.snapshot_live_bytes);
                metrics.import_cache_imports = metrics
                    .import_cache_imports
                    .saturating_add(stats.import_cache.imports);
                metrics.import_cache_hits = metrics
                    .import_cache_hits
                    .saturating_add(stats.import_cache.hits);
                metrics.import_cache_evictions = metrics
                    .import_cache_evictions
                    .saturating_add(stats.import_cache.evictions);
                metrics.import_cache_live_entries = metrics
                    .import_cache_live_entries
                    .saturating_add(stats.import_cache.live_entries);
                metrics.import_cache_descriptor_mismatches = metrics
                    .import_cache_descriptor_mismatches
                    .saturating_add(stats.import_cache.descriptor_mismatches);
                metrics.import_cache_capacity_rejections = metrics
                    .import_cache_capacity_rejections
                    .saturating_add(stats.import_cache.capacity_rejections);
                if let Some(worker) = exporter.worker_metrics() {
                    metrics.worker_requests =
                        metrics.worker_requests.saturating_add(worker.requests);
                    metrics.worker_completions = metrics
                        .worker_completions
                        .saturating_add(worker.completions);
                    metrics.worker_failures =
                        metrics.worker_failures.saturating_add(worker.failures);
                    metrics.worker_soft_stalls = metrics
                        .worker_soft_stalls
                        .saturating_add(worker.soft_stalls);
                    metrics.worker_hard_stalls = metrics
                        .worker_hard_stalls
                        .saturating_add(worker.hard_stalls);
                    metrics.worker_release_enqueue_failures = metrics
                        .worker_release_enqueue_failures
                        .saturating_add(worker.release_enqueue_failures);
                    metrics.max_worker_request =
                        metrics.max_worker_request.max(worker.max_request_age);
                }
                metrics.max_target_create = metrics.max_target_create.max(stats.max_target_create);
                metrics.max_frame_surface_create = metrics
                    .max_frame_surface_create
                    .max(stats.max_frame_surface_create);
                metrics.max_render = metrics.max_render.max(stats.max_render);
                metrics.max_upload = metrics.max_upload.max(stats.max_upload);
                metrics
            },
        )
    }
}
