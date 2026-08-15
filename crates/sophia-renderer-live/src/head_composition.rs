use sophia_engine::{
    HeadCompositionPlan, HeadCompositorCommand, compositor_border_bands,
    head_output_damage_snapshot,
};
use sophia_protocol::{BufferSource, Transform};

use crate::{
    LiveCompositionPlacement, LiveCpuPresentationLayer, LiveOwnedMixedCompositionFrame,
    LiveOwnedMixedCompositionLayer,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveHeadCompositionLoweringError {
    MissingCpuSource(u64),
    UnsupportedSource(BufferSource),
    SourceSizeMismatch(u64),
    DuplicateSurface,
    MissingPlannedSurface,
}

impl core::fmt::Display for LiveHeadCompositionLoweringError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LiveHeadCompositionLoweringError {}

/// Lowers one immutable Engine plan into an owned, head-native renderer frame.
///
/// This first production consumer admits CPU-authority variants and Engine
/// solids. DMA-BUF and retained renderer-image variants remain unavailable
/// here until their lease resolver can return an independently owned source for
/// every head; they are rejected rather than flattened or silently omitted.
pub fn lower_cpu_head_composition_plan(
    plan: &HeadCompositionPlan,
    sources: &[LiveCpuPresentationLayer],
) -> Result<LiveOwnedMixedCompositionFrame, LiveHeadCompositionLoweringError> {
    let mut layers = Vec::with_capacity(plan.compositor.len().saturating_mul(4));
    let mut emitted_surfaces = std::collections::BTreeSet::new();
    for command in &plan.compositor {
        match command {
            HeadCompositorCommand::Background(background) => {
                layers.push(LiveOwnedMixedCompositionLayer::Solid {
                    geometry: background.geometry,
                    color: background.color,
                });
            }
            HeadCompositorCommand::Surface { surface } => {
                if !emitted_surfaces.insert(*surface) {
                    return Err(LiveHeadCompositionLoweringError::DuplicateSurface);
                }
                let binding = plan
                    .layers
                    .iter()
                    .find(|binding| binding.surface == *surface)
                    .ok_or(LiveHeadCompositionLoweringError::MissingPlannedSurface)?;
                let BufferSource::CpuBuffer { handle } = binding.source else {
                    return Err(LiveHeadCompositionLoweringError::UnsupportedSource(
                        binding.source,
                    ));
                };
                let source = sources
                    .iter()
                    .find(|source| source.buffer.handle == handle)
                    .ok_or(LiveHeadCompositionLoweringError::MissingCpuSource(handle))?;
                if source.buffer.size != binding.source_pixel_size {
                    return Err(LiveHeadCompositionLoweringError::SourceSizeMismatch(handle));
                }
                layers.push(LiveOwnedMixedCompositionLayer::Cpu {
                    buffer: source.buffer.clone().into(),
                    placement: LiveCompositionPlacement {
                        target: binding.native_geometry,
                        clip: Some(binding.native_clip),
                        transform: Transform::IDENTITY,
                        alpha: f32::from(binding.opacity_millis) / 1_000.0,
                    },
                });
            }
            HeadCompositorCommand::Border(border) => {
                for band in compositor_border_bands(sophia_engine::CompositorBorder {
                    node: border.node,
                    generation: border.generation,
                    outer: border.outer,
                    inner: border.inner,
                    color: border.color,
                }) {
                    if !band.geometry.is_empty() {
                        layers.push(LiveOwnedMixedCompositionLayer::Solid {
                            geometry: band.geometry,
                            color: band.color,
                        });
                    }
                }
            }
        }
    }
    if let Some(cursor) = plan.cursor {
        let BufferSource::CpuBuffer { handle } = cursor.source else {
            return Err(LiveHeadCompositionLoweringError::UnsupportedSource(
                cursor.source,
            ));
        };
        let source = sources
            .iter()
            .find(|source| source.buffer.handle == handle)
            .ok_or(LiveHeadCompositionLoweringError::MissingCpuSource(handle))?;
        layers.push(LiveOwnedMixedCompositionLayer::Cpu {
            buffer: source.buffer.clone().into(),
            placement: LiveCompositionPlacement {
                target: cursor.geometry,
                clip: Some(cursor.geometry),
                transform: Transform::IDENTITY,
                alpha: 1.0,
            },
        });
    }
    Ok(LiveOwnedMixedCompositionFrame {
        layers,
        output_damage_snapshot: Some(head_output_damage_snapshot(plan)),
    })
}
