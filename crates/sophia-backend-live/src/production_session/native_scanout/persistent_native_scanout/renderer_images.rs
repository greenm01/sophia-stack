use super::*;

pub(super) struct LiveProductionQueuedMirrorGeneration {
    output: OutputId,
    pub(super) frame: LiveProductionNativeFrameId,
    heads: Vec<LiveProductionQueuedMirrorHeadFrame>,
}

struct LiveProductionQueuedMirrorHeadFrame {
    head_index: usize,
    content: LiveProductionScanoutContent,
    frame: crate::LiveOwnedMixedCompositionFrame,
    output_damage_snapshot: Option<sophia_engine::OutputFrameDamageSnapshot>,
    cpu_nonzero_pixel_bytes: usize,
}

impl LiveProductionQueuedMirrorGeneration {
    fn source(&self) -> &'static str {
        match self.heads.first().map(|head| head.content) {
            Some(LiveProductionScanoutContent::Cpu { .. }) => "cpu",
            Some(LiveProductionScanoutContent::MixedPresent { .. }) => "mixed_present",
            Some(LiveProductionScanoutContent::RetainedMixed { .. }) => "retained_mixed",
            None => "empty",
        }
    }

    pub(super) fn cpu_checksum(&self) -> Option<u64> {
        match self.heads.first().map(|head| head.content) {
            Some(LiveProductionScanoutContent::Cpu { checksum, .. }) => Some(checksum),
            _ => None,
        }
    }

    fn mixed_transaction(&self) -> Option<TransactionId> {
        match self.heads.first().map(|head| head.content) {
            Some(LiveProductionScanoutContent::MixedPresent { transaction, .. }) => {
                Some(transaction)
            }
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct LiveProductionRendererImageHandoff {
    output: OutputId,
    expected: Vec<sophia_renderer_live::LiveRendererImageId>,
    heads: Vec<LiveProductionRendererImageHeadHandoff>,
}

#[derive(Debug)]
struct LiveProductionRendererImageHeadHandoff {
    connector_id: u32,
    snapshots: Vec<sophia_renderer_live::LiveRendererImageSnapshot>,
}

impl LiveProductionRendererImageHandoff {
    pub const fn len(&self) -> usize {
        self.expected.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.expected.is_empty()
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

fn project_owned_mixed_frame(
    frame: &crate::LiveOwnedMixedCompositionFrame,
    source: sophia_protocol::Size,
    destination: sophia_engine::HeadlessOutput,
    fit: crate::NativeMirrorFit,
) -> Result<crate::LiveOwnedMixedCompositionFrame, Box<dyn std::error::Error>> {
    if source.width <= 0 || source.height <= 0 {
        return Err("mirror mixed-frame source size is invalid".into());
    }
    let target = crate::project_mirror_rect(source, destination.size, fit);
    if target.width <= 0 || target.height <= 0 {
        return Err("mirror mixed-frame projection is empty".into());
    }
    let mut projected = crate::try_clone_mixed_frame(frame)?;
    for layer in &mut projected.layers {
        match layer {
            sophia_renderer_live::LiveOwnedMixedCompositionLayer::Cpu { placement, .. }
            | sophia_renderer_live::LiveOwnedMixedCompositionLayer::DmaBuf { placement, .. }
            | sophia_renderer_live::LiveOwnedMixedCompositionLayer::RendererImage {
                placement,
                ..
            } => {
                placement.target =
                    crate::project_mirror_child_rect(placement.target, source, target);
                placement.clip = placement
                    .clip
                    .map(|clip| crate::project_mirror_child_rect(clip, source, target));
            }
            sophia_renderer_live::LiveOwnedMixedCompositionLayer::Solid { geometry, .. } => {
                *geometry = crate::project_mirror_child_rect(*geometry, source, target);
            }
        }
    }
    projected.output_damage_snapshot = frame
        .output_damage_snapshot
        .as_ref()
        .map(|snapshot| project_mirror_output_damage_snapshot(snapshot, source, destination, fit))
        .transpose()?;
    Ok(projected)
}

impl LiveProductionNativeScanout {
    fn mirror_mixed_transaction_frame(
        &self,
        output: OutputId,
        transaction: TransactionId,
    ) -> Option<LiveProductionNativeFrameId> {
        if let Some(frame) = self
            .queued_mirror_successors
            .get(&output)
            .filter(|generation| generation.mixed_transaction() == Some(transaction))
            .map(|generation| generation.frame)
        {
            return Some(frame);
        }
        self.head_indices(output)
            .into_iter()
            .find_map(|head_index| {
                let head = &self.heads[head_index];
                [
                    head.pending_content,
                    head.rendering_content,
                    head.submitted_content,
                ]
                .into_iter()
                .flatten()
                .find_map(|content| match content {
                    LiveProductionScanoutContent::MixedPresent {
                        frame,
                        transaction: owned,
                        ..
                    } if owned == transaction => Some(frame),
                    _ => None,
                })
            })
    }

    fn install_mirror_generation(
        &mut self,
        generation: LiveProductionQueuedMirrorGeneration,
        status: &'static str,
    ) -> Result<(), &'static str> {
        if generation.heads.is_empty()
            || generation
                .heads
                .iter()
                .any(|head| head.content.frame() != generation.frame)
        {
            return Err("mirror generation has invalid or mismatched frame identity");
        }
        let expected = self.head_indices(generation.output);
        let actual = generation
            .heads
            .iter()
            .map(|head| head.head_index)
            .collect::<Vec<_>>();
        if expected != actual {
            return Err("mirror generation does not cover every physical head exactly once");
        }
        if generation.heads.iter().any(|queued| {
            self.heads[queued.head_index].pending_content.is_some()
                || self.heads[queued.head_index].rendering_content.is_some()
                || self.exporters[queued.head_index].pending_frame()
        }) {
            return Err("mirror generation promotion found occupied head work");
        }
        let lifecycle = self
            .output_lifecycles
            .get_mut(&generation.output)
            .ok_or("mirror generation targets an unregistered output")?;
        if lifecycle.active_frame().is_some() {
            return Err("mirror successor promoted before the active generation retired");
        }
        if lifecycle.initialized()
            && lifecycle.begin(generation.frame) != LiveProductionMirrorGroupBegin::Started
        {
            return Err("mirror generation could not reserve its lifecycle");
        }
        let source = generation.source();
        let checksum = generation.cpu_checksum();
        for queued in generation.heads {
            let (head, exporter) = self.head_and_exporter(queued.head_index, generation.output);
            if let Some(checksum) = checksum {
                head.last_checksum = checksum;
                head.pending_nonzero_pixel_bytes = queued.cpu_nonzero_pixel_bytes;
            }
            head.pending_content = Some(queued.content);
            head.queue_output_damage_snapshot(queued.output_damage_snapshot);
            exporter.set_pending_mixed_frame(queued.frame);
        }
        tracing::info!(
            "sophia_live_mirror_generation schema=1 status={} output={} frame={} source={} checksum={}",
            status,
            generation.output.raw(),
            generation.frame.raw(),
            source,
            checksum.map_or_else(|| "none".to_owned(), |checksum| checksum.to_string()),
        );
        Ok(())
    }

    fn queue_mirror_generation(
        &mut self,
        generation: LiveProductionQueuedMirrorGeneration,
    ) -> Result<(), &'static str> {
        let output = generation.output;
        let frame = generation.frame;
        let source = generation.source();
        let checksum = generation.cpu_checksum();
        let active = self
            .output_lifecycles
            .get(&output)
            .and_then(LiveProductionMirrorGroupLifecycle::active_frame);
        let successor = self
            .queued_mirror_successors
            .get(&output)
            .map(|generation| generation.frame);
        let target = reduce_live_production_mirror_generation_queue_target(active, successor);
        if target == LiveProductionMirrorGenerationQueueTarget::Active {
            return self.install_mirror_generation(generation, "installed");
        }
        self.queued_mirror_successors.insert(output, generation);
        tracing::info!(
            "sophia_live_mirror_generation schema=1 status={} output={} frame={} source={} checksum={} active={}",
            if matches!(
                target,
                LiveProductionMirrorGenerationQueueTarget::ReplaceSuccessor(_)
            ) {
                "coalesced"
            } else {
                "queued"
            },
            output.raw(),
            frame.raw(),
            source,
            checksum.map_or_else(|| "none".to_owned(), |checksum| checksum.to_string()),
            active.expect("checked above").raw(),
        );
        Ok(())
    }

    pub(super) fn promote_queued_mirror_generation(
        &mut self,
        output: OutputId,
    ) -> Result<bool, &'static str> {
        let Some(generation) = self.queued_mirror_successors.remove(&output) else {
            return Ok(false);
        };
        self.install_mirror_generation(generation, "promoted")?;
        Ok(true)
    }

    pub fn queue_present_cpu_frame(
        &mut self,
        output: OutputId,
        frame: LiveProductionComposedFrame,
    ) -> Result<LiveProductionNativeFrameId, &'static str> {
        let index = self
            .primary_head_index(output)
            .ok_or("native output has no head")?;
        if self.pending_frame(output) {
            return Err("native output already has pending frame work");
        }
        if self.head_indices(output).len() > 1 {
            return self
                .queue_projected_frame(output, &frame, self.mirror_fit)
                .ok_or("native mirror projection produced no head frame");
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
    ) -> Result<LiveProductionNativeFrameId, Box<dyn std::error::Error>> {
        let indices = self.head_indices(output);
        let Some(&index) = indices.first() else {
            return Err("native mixed frame targets an unregistered output".into());
        };
        if indices.len() == 1 {
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
            return Ok(frame_id);
        }
        if let Some(existing) = self.mirror_mixed_transaction_frame(output, transaction) {
            return Ok(existing);
        }
        let source = self.heads[index].output.size;
        let projected = indices
            .iter()
            .map(|head_index| {
                project_owned_mixed_frame(
                    &frame,
                    source,
                    self.heads[*head_index].output,
                    self.mirror_fit,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let frame_id = self.allocate_frame_id();
        let heads = indices
            .into_iter()
            .zip(projected)
            .map(|(head_index, frame)| LiveProductionQueuedMirrorHeadFrame {
                head_index,
                output_damage_snapshot: frame.output_damage_snapshot.clone(),
                content: LiveProductionScanoutContent::MixedPresent {
                    frame: frame_id,
                    transaction,
                    nonzero_rgb_pixels: 0,
                },
                frame,
                cpu_nonzero_pixel_bytes: 0,
            })
            .collect();
        self.queue_mirror_generation(LiveProductionQueuedMirrorGeneration {
            output,
            frame: frame_id,
            heads,
        })?;
        Ok(frame_id)
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
    /// Returns the one logical frame identity shared by every projected head.
    pub fn queue_projected_frame(
        &mut self,
        output: OutputId,
        frame: &LiveProductionComposedFrame,
        fit: crate::NativeMirrorFit,
    ) -> Option<LiveProductionNativeFrameId> {
        if let Some(existing) = self.queued_mirror_successors.get(&output)
            && existing.cpu_checksum() == Some(frame.checksum)
        {
            return Some(existing.frame);
        }
        let heads = self.head_indices(output);
        let targets = heads
            .iter()
            .map(|head_index| {
                crate::project_mirror_rect(
                    frame.frame.size,
                    self.heads[*head_index].output.size,
                    fit,
                )
            })
            .collect::<Vec<_>>();
        if heads.is_empty()
            || targets
                .iter()
                .any(|target| target.width <= 0 || target.height <= 0)
        {
            return None;
        }
        let projected_damage = heads
            .iter()
            .zip(&targets)
            .map(|(head_index, _)| {
                frame
                    .output_damage_snapshot
                    .as_ref()
                    .map(|snapshot| {
                        project_mirror_output_damage_snapshot(
                            snapshot,
                            frame.frame.size,
                            self.heads[*head_index].output,
                            fit,
                        )
                    })
                    .transpose()
            })
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        let frame_id = self.allocate_frame_id();
        let source = sophia_renderer_live::LiveSharedCpuBufferSource {
            handle: 0,
            size: frame.frame.size,
            stride: frame.frame.stride,
            format: frame.frame.format,
            generation: frame_id.raw(),
            bytes: std::sync::Arc::clone(&frame.frame.bytes),
        };
        let heads = heads
            .into_iter()
            .zip(targets)
            .zip(projected_damage)
            .map(|((head_index, target), output_damage_snapshot)| {
                let layer = sophia_renderer_live::LiveOwnedMixedCompositionLayer::Cpu {
                    buffer: source.clone(),
                    placement: sophia_renderer_live::LiveCompositionPlacement {
                        target,
                        clip: None,
                        transform: sophia_protocol::Transform::IDENTITY,
                        alpha: 1.0,
                    },
                };
                LiveProductionQueuedMirrorHeadFrame {
                    head_index,
                    content: LiveProductionScanoutContent::Cpu {
                        frame: frame_id,
                        checksum: frame.checksum,
                    },
                    frame: sophia_renderer_live::LiveOwnedMixedCompositionFrame {
                        layers: vec![layer],
                        output_damage_snapshot: output_damage_snapshot.clone(),
                    },
                    output_damage_snapshot,
                    cpu_nonzero_pixel_bytes: frame.nonzero_pixel_bytes,
                }
            })
            .collect();
        self.queue_mirror_generation(LiveProductionQueuedMirrorGeneration {
            output,
            frame: frame_id,
            heads,
        })
        .ok()?;
        Some(frame_id)
    }

    /// Queues the first mirror generation through direct CPU buffers.
    ///
    /// Startup must synchronously establish KMS owners before renderer workers
    /// exist. Projecting in CPU memory keeps that bootstrap out of inline EGL;
    /// later mixed generations use the worker-backed path above.
    pub fn queue_projected_bootstrap_frame(
        &mut self,
        output: OutputId,
        frame: &LiveProductionComposedFrame,
        fit: crate::NativeMirrorFit,
    ) -> Result<LiveProductionNativeFrameId, Box<dyn std::error::Error>> {
        let heads = self.head_indices(output);
        if heads.is_empty() {
            return Err("native mirror bootstrap has no head".into());
        }
        let targets = heads
            .iter()
            .map(|head_index| {
                crate::project_mirror_rect(
                    frame.frame.size,
                    self.heads[*head_index].output.size,
                    fit,
                )
            })
            .collect::<Vec<_>>();
        let projected = heads
            .iter()
            .zip(&targets)
            .map(|(head_index, target)| {
                sophia_renderer_live::project_live_cpu_composed_frame(
                    &frame.frame,
                    self.heads[*head_index].output.size,
                    *target,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let projected_damage = heads
            .iter()
            .map(|head_index| {
                frame
                    .output_damage_snapshot
                    .as_ref()
                    .map(|snapshot| {
                        project_mirror_output_damage_snapshot(
                            snapshot,
                            frame.frame.size,
                            self.heads[*head_index].output,
                            fit,
                        )
                    })
                    .transpose()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let frame_id = self.allocate_frame_id();
        for ((head_index, projected), output_damage_snapshot) in
            heads.into_iter().zip(projected).zip(projected_damage)
        {
            let (head, exporter) = self.head_and_exporter(head_index, output);
            head.pending_nonzero_pixel_bytes = frame.nonzero_pixel_bytes;
            head.last_checksum = frame.checksum;
            head.queue_output_damage_snapshot(output_damage_snapshot.clone());
            head.pending_content = Some(LiveProductionScanoutContent::Cpu {
                frame: frame_id,
                checksum: frame.checksum,
            });
            exporter.set_pending_cpu_frame_with_damage(
                projected,
                frame.checksum,
                output_damage_snapshot,
            );
        }
        Ok(frame_id)
    }

    pub fn queue_retained_mixed_frame(
        &mut self,
        output: OutputId,
        frame: crate::LiveOwnedMixedCompositionFrame,
    ) -> Result<LiveProductionNativeFrameId, Box<dyn std::error::Error>> {
        let indices = self.head_indices(output);
        let Some(&index) = indices.first() else {
            return Err("native retained frame targets an unregistered output".into());
        };
        if indices.len() == 1 {
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
            return Ok(frame_id);
        }
        let source = self.heads[index].output.size;
        let projected = indices
            .iter()
            .map(|head_index| {
                project_owned_mixed_frame(
                    &frame,
                    source,
                    self.heads[*head_index].output,
                    self.mirror_fit,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let frame_id = self.allocate_frame_id();
        let heads = indices
            .into_iter()
            .zip(projected)
            .map(|(head_index, frame)| LiveProductionQueuedMirrorHeadFrame {
                head_index,
                output_damage_snapshot: frame.output_damage_snapshot.clone(),
                content: LiveProductionScanoutContent::RetainedMixed {
                    frame: frame_id,
                    nonzero_rgb_pixels: 0,
                },
                frame,
                cpu_nonzero_pixel_bytes: 0,
            })
            .collect();
        self.queue_mirror_generation(LiveProductionQueuedMirrorGeneration {
            output,
            frame: frame_id,
            heads,
        })?;
        Ok(frame_id)
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
        let indices = self.head_indices(output);
        if indices.is_empty() {
            return Err("renderer-image handoff targets an unknown output".into());
        }
        let mut heads = Vec::with_capacity(indices.len());
        for head_index in indices {
            let mut snapshots = Vec::with_capacity(expected.len());
            for image_id in expected {
                let snapshot = self.exporters[head_index]
                    .export_promoted_renderer_image(*image_id)?
                    .ok_or("retained scene refers to an unavailable promoted renderer image")?;
                snapshots.push(snapshot);
            }
            let actual = snapshots
                .iter()
                .map(sophia_renderer_live::LiveRendererImageSnapshot::image_id)
                .collect::<Vec<_>>();
            validate_renderer_image_handoff_ids(expected, &actual)?;
            heads.push(LiveProductionRendererImageHeadHandoff {
                connector_id: self.heads[head_index].selection.connector_id(),
                snapshots,
            });
        }
        Ok(LiveProductionRendererImageHandoff {
            output,
            expected: expected.to_vec(),
            heads,
        })
    }

    pub fn restore_renderer_image_handoff(
        &mut self,
        handoff: LiveProductionRendererImageHandoff,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let expected_count = handoff.expected.len();
        let expected_heads = self.head_indices(handoff.output);
        if expected_heads.len() != handoff.heads.len() {
            return Err("renderer-image handoff head coverage changed during replacement".into());
        }
        for head_handoff in handoff.heads {
            let head_index = expected_heads
                .iter()
                .copied()
                .find(|head_index| {
                    self.heads[*head_index].selection.connector_id() == head_handoff.connector_id
                })
                .ok_or("renderer-image handoff names an unavailable connector")?;
            if !self.exporters[head_index].renderer_image_owner_initialized() {
                return Err("replacement renderer image owner is not initialized".into());
            }
            let actual = head_handoff
                .snapshots
                .iter()
                .map(sophia_renderer_live::LiveRendererImageSnapshot::image_id)
                .collect::<Vec<_>>();
            validate_renderer_image_handoff_ids(&handoff.expected, &actual)?;
            for snapshot in head_handoff.snapshots {
                if !self.exporters[head_index].restore_promoted_renderer_image(snapshot)? {
                    return Err("replacement renderer rejected a retained image snapshot".into());
                }
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
