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
        if self.heads[index].exporter.pending_frame() {
            return Err("native output already has pending frame work");
        }
        let frame_id = self.allocate_frame_id();
        let head = &mut self.heads[index];
        head.pending_nonzero_pixel_bytes = frame.nonzero_pixel_bytes;
        head.last_checksum = frame.checksum;
        head.queue_output_damage_snapshot(frame.output_damage_snapshot.clone());
        head.pending_content = Some(LiveProductionScanoutContent::Cpu {
            frame: frame_id,
            checksum: frame.checksum,
        });
        head.exporter.set_pending_cpu_frame_with_damage(
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
        let head = &mut self.heads[index];
        let pending_before = head.exporter.pending_frame();
        let worker_in_flight = head.exporter.worker_in_flight();
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
        head.exporter.set_pending_mixed_frame(frame);
        tracing::debug!(
            "sophia_live_retained_projection schema=1 status=native_queued output={} frame={} pending_before={} worker_in_flight={}",
            head.output.id.raw(),
            frame_id.raw(),
            pending_before,
            worker_in_flight,
        );
        frame_id
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
        let head = &mut self.heads[index];
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
        head.exporter.set_pending_mixed_frame(frame);
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
        let head = &mut self.heads[index];
        head.exporter.set_pending_mixed_frame(frame);
        let export =
            head.exporter
                .export_rendered_scanout_buffer(crate::LiveGbmEglFrameTargetRecord::new(
                    head.output.size,
                ));
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
        for head in &mut self.heads {
            evicted =
                evicted.saturating_add(usize::from(head.exporter.evict_renderer_image(image_id)?));
        }
        Ok(evicted)
    }

    pub fn promote_renderer_image(
        &mut self,
        image_id: sophia_renderer_live::LiveRendererImageId,
    ) -> Result<usize, crate::LiveRendererScanoutBufferExportDetail> {
        let mut promoted = 0usize;
        for head in &mut self.heads {
            promoted = promoted
                .saturating_add(usize::from(head.exporter.promote_renderer_image(image_id)?));
        }
        Ok(promoted)
    }

    pub fn rollback_renderer_image(
        &mut self,
        image_id: sophia_renderer_live::LiveRendererImageId,
    ) -> Result<usize, crate::LiveRendererScanoutBufferExportDetail> {
        let mut rolled_back = 0usize;
        for head in &mut self.heads {
            rolled_back = rolled_back.saturating_add(usize::from(
                head.exporter.rollback_renderer_image(image_id)?,
            ));
        }
        Ok(rolled_back)
    }

    pub fn export_renderer_image_handoff(
        &mut self,
        output: OutputId,
        expected: &[sophia_renderer_live::LiveRendererImageId],
    ) -> Result<LiveProductionRendererImageHandoff, Box<dyn std::error::Error>> {
        validate_renderer_image_handoff_ids(expected, expected)?;
        let index = self
            .primary_head_index(output)
            .ok_or("renderer-image handoff selected an unknown output")?;
        let mut snapshots = Vec::with_capacity(expected.len());
        for image_id in expected {
            let snapshot = self.heads[index]
                .exporter
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
        let index = self
            .primary_head_index(handoff.output)
            .ok_or("renderer-image handoff target output is unavailable")?;
        if !self.heads[index]
            .exporter
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
            if !self.heads[index]
                .exporter
                .restore_promoted_renderer_image(snapshot)?
            {
                return Err("replacement renderer rejected a retained image snapshot".into());
            }
        }
        Ok(expected_count)
    }

    pub fn renderer_image_owners_initialized(&self) -> bool {
        !self.heads.is_empty()
            && self
                .heads
                .iter()
                .all(|head| head.exporter.renderer_image_owner_initialized())
    }

    pub fn clear_renderer_images(
        &mut self,
    ) -> Result<usize, crate::LiveRendererScanoutBufferExportDetail> {
        let mut evicted = 0usize;
        for head in &mut self.heads {
            evicted = evicted.saturating_add(head.exporter.clear_renderer_images()?);
        }
        Ok(evicted)
    }

    pub fn export_attempts(&self) -> usize {
        self.heads
            .iter()
            .map(|head| head.exporter.cpu_frame_export_attempts())
            .chain(
                self.heads
                    .iter()
                    .map(|head| head.exporter.mixed_frame_export_attempts()),
            )
            .sum()
    }

    pub fn mixed_exports(&self) -> usize {
        self.heads
            .iter()
            .map(|head| head.exporter.mixed_frame_exports())
            .sum()
    }

    pub fn persistent_render_metrics(&self) -> LivePersistentRenderMetrics {
        self.heads.iter().fold(
            LivePersistentRenderMetrics::default(),
            |mut metrics, head| {
                let stats = head.exporter.persistent_render_stats();
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
                if let Some(worker) = head.exporter.worker_metrics() {
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
