fn live_transaction_pixel_size(
    source: sophia_protocol::BufferSource,
    dma_buf_sizes: &BTreeMap<sophia_protocol::BufferHandle, Size>,
    cpu_buffer_sizes: &BTreeMap<u64, Size>,
) -> Option<Size> {
    match source {
        sophia_protocol::BufferSource::DmaBuf { handle } => dma_buf_sizes
            .get(&sophia_protocol::BufferHandle::from_raw(handle))
            .copied(),
        sophia_protocol::BufferSource::CpuBuffer { handle } => {
            cpu_buffer_sizes.get(&handle).copied()
        }
        sophia_protocol::BufferSource::None
        | sophia_protocol::BufferSource::XPixmap { .. } => None,
    }
}

fn live_transaction_observed_size(
    transaction: &SurfaceTransaction,
    dma_buf_sizes: &BTreeMap<sophia_protocol::BufferHandle, Size>,
    cpu_buffer_sizes: &BTreeMap<u64, Size>,
) -> Size {
    let logical = Size {
        width: transaction.target_geometry.width,
        height: transaction.target_geometry.height,
    };
    let Some(source) = live_transaction_pixel_size(
        transaction.target_buffer(),
        dma_buf_sizes,
        cpu_buffer_sizes,
    ) else {
        return logical;
    };
    if source == transaction.target_content_size() {
        logical
    } else {
        // An old or partial buffer cannot satisfy a new logical extent. Keep
        // reporting its physical size so exact resize and admission gates
        // remain closed until the authority publishes matching content.
        source
    }
}

fn live_transaction_visual_evidence(
    transaction: &SurfaceTransaction,
    batch: &sophia_x_authority::XAuthorityObservedTransactionBatch,
) -> sophia_engine::SurfaceVisualEvidence {
    let presented = match transaction.target_buffer() {
        sophia_protocol::BufferSource::DmaBuf { handle } => batch
            .present_submissions
            .iter()
            .filter(|submission| {
                submission.transaction == transaction.transaction
                    && submission.surface == transaction.surface
                    && submission.buffer.raw() == handle
            })
            .count()
            == 1,
        sophia_protocol::BufferSource::CpuBuffer { .. } => {
            batch
                .software_present_submissions
                .iter()
                .filter(|submission| {
                    submission.transaction == transaction.transaction
                        && submission.surface == transaction.surface
                })
                .count()
                == 1
                && batch
                    .transactions
                    .iter()
                    .filter(|candidate| {
                        candidate.transaction == transaction.transaction
                            && candidate.surface == transaction.surface
                            && matches!(
                                candidate.target_buffer(),
                                sophia_protocol::BufferSource::CpuBuffer { .. }
                            )
                    })
                    .count()
                    == 1
        }
        sophia_protocol::BufferSource::XPixmap { .. }
        | sophia_protocol::BufferSource::None => false,
    };
    if presented {
        sophia_engine::SurfaceVisualEvidence::PresentedBuffer
    } else {
        sophia_engine::SurfaceVisualEvidence::BackingSnapshot
    }
}

impl PersistentLiveLayout {
    fn arm_standing_recovery_candidate(
        &mut self,
        transaction: &SurfaceTransaction,
        layout_size: Size,
        evidence: sophia_engine::SurfaceVisualEvidence,
        candidate_selected: bool,
    ) -> bool {
        if !candidate_selected
            || evidence != sophia_engine::SurfaceVisualEvidence::PresentedBuffer
            || self.pending.is_some()
            || self.layout_epochs.pending_target(transaction.surface) != Some(layout_size)
            || self
                .awaiting_visual_commits
                .surface_layout_awaiting(transaction.surface, layout_size)
        {
            return false;
        }
        let Some(source_size) = live_transaction_pixel_size(
            transaction.target_buffer(),
            &self.dma_buf_sizes,
            &self.cpu_buffer_sizes,
        ) else {
            return false;
        };

        // A client can answer the standing target before or after its fallback
        // retires. Keep one exact successor beside the fallback; normal layout
        // still controls geometry, and only native retirement commits pixels.
        self.awaiting_visual_commits
            .arm(ResizeVisualCommit {
                candidate: transaction.key(),
                size: source_size,
                layout_size,
            })
            .expect("one standing recovery target owns one visual candidate");
        println!(
            "sophia_live_resize_epoch schema=3 status=visual_armed epoch={} transaction={} surface={} width={} height={} source=standing_target_recovery",
            transaction.transaction.raw(),
            transaction.transaction.raw(),
            transaction.surface.index(),
            layout_size.width,
            layout_size.height,
        );
        true
    }

    fn selected_pre_admission_transaction(
        &self,
        surface: SurfaceId,
        extent: Size,
    ) -> Option<&SurfaceTransaction> {
        let selected = self.layout_epochs.safe_observation(surface)?;
        if selected.extent != extent {
            return None;
        }
        self.pre_admission_groups
            .iter()
            .flat_map(|group| group.transactions.iter())
            .find(|transaction| {
                selected.candidate == Some(transaction.key())
                    && live_transaction_observed_size(
                        transaction,
                        &self.dma_buf_sizes,
                        &self.cpu_buffer_sizes,
                    ) == extent
            })
    }
}
