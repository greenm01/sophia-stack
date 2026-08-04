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
    live_transaction_pixel_size(transaction.target_buffer, dma_buf_sizes, cpu_buffer_sizes)
        .unwrap_or(Size {
            width: transaction.target_geometry.width,
            height: transaction.target_geometry.height,
        })
}

fn live_transaction_visual_evidence(
    transaction: &SurfaceTransaction,
    batch: &sophia_x_authority::XAuthorityObservedTransactionBatch,
) -> sophia_engine::SurfaceVisualEvidence {
    let presented = match transaction.target_buffer {
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
                                candidate.target_buffer,
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
