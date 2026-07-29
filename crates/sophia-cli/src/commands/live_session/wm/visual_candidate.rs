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
    explicitly_presented: bool,
) -> sophia_engine::SurfaceVisualEvidence {
    match (transaction.target_buffer, explicitly_presented) {
        (_, true) | (sophia_protocol::BufferSource::DmaBuf { .. }, false) => {
            sophia_engine::SurfaceVisualEvidence::PresentedBuffer
        }
        (
            sophia_protocol::BufferSource::XPixmap { .. }
            | sophia_protocol::BufferSource::CpuBuffer { .. }
            | sophia_protocol::BufferSource::None,
            false,
        ) => {
            sophia_engine::SurfaceVisualEvidence::BackingSnapshot
        }
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
                transaction.surface == surface
                    && selected.transaction == Some(transaction.transaction)
                    && live_transaction_observed_size(
                        transaction,
                        &self.dma_buf_sizes,
                        &self.cpu_buffer_sizes,
                    ) == extent
            })
    }
}
