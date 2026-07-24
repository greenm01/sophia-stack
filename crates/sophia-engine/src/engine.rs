mod core;
mod layout;
mod rendering;
mod session_tick;
mod wm_transaction;

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
            surface: transaction.surface,
            authority_local_id: None,
            namespace: transaction.namespace,
            stack_rank: u32::try_from(index).unwrap_or(u32::MAX),
            geometry: transaction.target_geometry,
            source: BufferSource::None,
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
