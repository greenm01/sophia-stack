use sophia_engine::{
    HeadCompositionPlan, HeadCompositorCommand, compositor_border_bands,
    head_output_damage_snapshot,
};
use sophia_protocol::{BufferSource, Size, SurfaceId, Transform};

use crate::{
    LiveCompositionPlacement, LiveCpuPresentationLayer, LiveOwnedMixedCompositionFrame,
    LiveOwnedMixedCompositionLayer, LiveOwnedMultiPlaneDmaBufFrame, LiveRendererImageId,
    LiveSharedCpuBufferSource,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveHeadCompositionLoweringError {
    MissingCpuSource(u64),
    UnsupportedSource(BufferSource),
    /// The plan measured this buffer at one size and the source holds another.
    ///
    /// It carries both sizes and the surface because the handle alone cannot
    /// say which record is stale: a plan is written from committed content,
    /// while a retained projection holds the pixels of whatever last reached a
    /// screen. When a surface commits a new buffer that no head has composed
    /// yet, those two disagree, and the session ends over a number nobody can
    /// trace without seeing both.
    SourceSizeMismatch {
        surface: SurfaceId,
        handle: u64,
        planned: Size,
        held: Size,
    },
    DuplicateSurface,
    MissingPlannedSurface,
    MissingSource(BufferSource),
    SourceKindMismatch(BufferSource),
    DmaBufCloneFailed(u64),
}

impl core::fmt::Display for LiveHeadCompositionLoweringError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LiveHeadCompositionLoweringError {}

#[derive(Debug)]
pub enum LiveOwnedHeadCompositionSourceKind {
    Cpu(LiveSharedCpuBufferSource),
    DmaBuf {
        image_id: LiveRendererImageId,
        frame: LiveOwnedMultiPlaneDmaBufFrame,
    },
    RendererImage {
        image_id: LiveRendererImageId,
        size: Size,
        format: u32,
    },
}

/// One authority-owned source realization available to the per-head lowerer.
///
/// The logical source identity stays the committed `BufferSource`. DMA-BUF file
/// descriptors are duplicated for each head plan, while retained renderer-image
/// identities refer to independently initialized per-head exporter stores.
#[derive(Debug)]
pub struct LiveOwnedHeadCompositionSource {
    pub surface: SurfaceId,
    pub source: BufferSource,
    pub kind: LiveOwnedHeadCompositionSourceKind,
}

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
    let sources = sources
        .iter()
        .map(|source| LiveOwnedHeadCompositionSource {
            surface: source.surface,
            source: BufferSource::CpuBuffer {
                handle: source.buffer.handle,
            },
            kind: LiveOwnedHeadCompositionSourceKind::Cpu(source.buffer.clone().into()),
        })
        .collect::<Vec<_>>();
    lower_head_composition_plan(plan, &sources)
}

/// Lowers a complete immutable Engine plan using authority-owned source
/// realizations. No geometry is inherited from a primary head: every placement,
/// clip, compositor primitive, and damage snapshot comes from `plan`.
pub fn lower_head_composition_plan(
    plan: &HeadCompositionPlan,
    sources: &[LiveOwnedHeadCompositionSource],
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
                let source = sources
                    .iter()
                    .find(|source| source.surface == *surface && source.source == binding.source)
                    .ok_or_else(|| match binding.source {
                        BufferSource::CpuBuffer { handle } => {
                            LiveHeadCompositionLoweringError::MissingCpuSource(handle)
                        }
                        source => LiveHeadCompositionLoweringError::MissingSource(source),
                    })?;
                layers.push(lower_surface_source(binding, source)?);
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
        let source = sources
            .iter()
            .find(|source| source.source == cursor.source)
            .ok_or_else(|| match cursor.source {
                BufferSource::CpuBuffer { handle } => {
                    LiveHeadCompositionLoweringError::MissingCpuSource(handle)
                }
                source => LiveHeadCompositionLoweringError::MissingSource(source),
            })?;
        let placement = LiveCompositionPlacement {
            target: cursor.geometry,
            clip: Some(cursor.geometry),
            transform: Transform::IDENTITY,
            alpha: 1.0,
        };
        layers.push(lower_owned_source(
            cursor.source,
            source,
            placement,
            source_size(source)?,
        )?);
    }
    Ok(LiveOwnedMixedCompositionFrame {
        layers,
        output_damage_snapshot: Some(head_output_damage_snapshot(plan)),
    })
}

fn lower_surface_source(
    binding: &sophia_engine::HeadLayerBinding,
    source: &LiveOwnedHeadCompositionSource,
) -> Result<LiveOwnedMixedCompositionLayer, LiveHeadCompositionLoweringError> {
    lower_owned_source(
        binding.source,
        source,
        LiveCompositionPlacement {
            target: binding.native_geometry,
            clip: Some(binding.native_clip),
            transform: Transform::IDENTITY,
            alpha: f32::from(binding.opacity_millis) / 1_000.0,
        },
        binding.source_pixel_size,
    )
}

fn source_size(
    source: &LiveOwnedHeadCompositionSource,
) -> Result<Size, LiveHeadCompositionLoweringError> {
    match &source.kind {
        LiveOwnedHeadCompositionSourceKind::Cpu(buffer) => Ok(buffer.size),
        LiveOwnedHeadCompositionSourceKind::DmaBuf { frame, .. } => Ok(Size {
            width: i32::try_from(frame.width)
                .map_err(|_| LiveHeadCompositionLoweringError::SourceKindMismatch(source.source))?,
            height: i32::try_from(frame.height)
                .map_err(|_| LiveHeadCompositionLoweringError::SourceKindMismatch(source.source))?,
        }),
        LiveOwnedHeadCompositionSourceKind::RendererImage { size, .. } => Ok(*size),
    }
}

/// Whether this source is the buffer a plan measured, or a copy of one.
pub const fn requires_exact_source_size(kind: &LiveOwnedHeadCompositionSourceKind) -> bool {
    match kind {
        LiveOwnedHeadCompositionSourceKind::Cpu(_)
        | LiveOwnedHeadCompositionSourceKind::DmaBuf { .. } => true,
        LiveOwnedHeadCompositionSourceKind::RendererImage { .. } => false,
    }
}

fn lower_owned_source(
    expected: BufferSource,
    source: &LiveOwnedHeadCompositionSource,
    placement: LiveCompositionPlacement,
    expected_size: Size,
) -> Result<LiveOwnedMixedCompositionLayer, LiveHeadCompositionLoweringError> {
    if source.source != expected {
        return Err(LiveHeadCompositionLoweringError::MissingSource(expected));
    }
    let actual_size = source_size(source)?;
    // A renderer image is the compositor's own copy of an earlier generation,
    // carried under the identity of the surface's committed buffer. Its size is
    // its own fact, not a measurement of that buffer, so comparing the two asks
    // a question with no answer: they agree only while the surface has not
    // resized, and a mirror member already draws every retained image at a size
    // of its own. Placement carries it to the head either way. The comparison
    // stays for CPU and DMA-BUF sources, where the plan measured the very
    // buffer being handed over and a difference means the wrong one arrived.
    if requires_exact_source_size(&source.kind) && actual_size != expected_size {
        return Err(LiveHeadCompositionLoweringError::SourceSizeMismatch {
            surface: source.surface,
            handle: match expected {
                BufferSource::CpuBuffer { handle } | BufferSource::DmaBuf { handle } => handle,
                BufferSource::None | BufferSource::XPixmap { .. } => 0,
            },
            planned: expected_size,
            held: actual_size,
        });
    }
    match (&source.kind, expected) {
        (LiveOwnedHeadCompositionSourceKind::Cpu(buffer), BufferSource::CpuBuffer { .. }) => {
            Ok(LiveOwnedMixedCompositionLayer::Cpu {
                buffer: buffer.clone(),
                placement,
            })
        }
        (
            LiveOwnedHeadCompositionSourceKind::DmaBuf { image_id, frame },
            BufferSource::DmaBuf { handle },
        ) => Ok(LiveOwnedMixedCompositionLayer::DmaBuf {
            image_id: *image_id,
            frame: frame
                .try_clone()
                .map_err(|_| LiveHeadCompositionLoweringError::DmaBufCloneFailed(handle))?,
            placement,
        }),
        (
            LiveOwnedHeadCompositionSourceKind::RendererImage {
                image_id,
                size,
                format,
            },
            BufferSource::DmaBuf { .. },
        ) => Ok(LiveOwnedMixedCompositionLayer::RendererImage {
            image_id: *image_id,
            size: *size,
            format: *format,
            placement,
        }),
        _ => Err(LiveHeadCompositionLoweringError::SourceKindMismatch(
            expected,
        )),
    }
}
