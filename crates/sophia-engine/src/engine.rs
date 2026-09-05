mod core;
mod layout;
mod rendering;
mod session_tick;

use crate::{EngineError, FramePlanRequest, HeadlessOutput, ReplayReport};
use sophia_protocol::{
    BufferSource, FrameSnapshot, LayerSnapshot, ResizeSyncCapability, SurfaceTransaction, Transform,
};

pub fn layer_templates_from_surface_transactions(
    transactions: &[SurfaceTransaction],
) -> Vec<LayerSnapshot> {
    transactions
        .iter()
        .enumerate()
        .map(|(index, transaction)| LayerSnapshot {
            translation: None,
            surface: transaction.surface,
            authority_local_id: None,
            // A template describes a transaction, not a placement, so it names
            // no output. The placement path fills this in.
            output: None,
            namespace: transaction.namespace,
            stack_rank: u32::try_from(index).unwrap_or(u32::MAX),
            geometry: transaction.target_geometry,
            source: BufferSource::None,
            // A template names no raster, so its size is only a placeholder.
            source_size: sophia_protocol::Size {
                width: transaction.target_geometry.width,
                height: transaction.target_geometry.height,
            },
            damage: transaction.damage.clone(),
            opacity: 1.0,
            crop: None,
            transform: Transform::IDENTITY,
            generation: transaction.previous_committed_generation,
            resize_sync: ResizeSyncCapability::ImplicitOnly,
        })
        .collect()
}

pub trait EngineBackend {
    fn output(&self) -> HeadlessOutput;

    fn plan_frame(
        &self,
        request: FramePlanRequest,
        layers: Vec<LayerSnapshot>,
    ) -> Result<FrameSnapshot, EngineError>;

    fn replay_frame(&self, frame: &FrameSnapshot) -> Result<ReplayReport, EngineError>;
}

#[derive(Clone, Debug, Default)]
pub struct HeadlessEngine {
    pub(crate) output: HeadlessOutput,
}
