//! Protocol-neutral live presentation resources.
//!
//! Protocol frontends translate their local IDs into these typed records. The
//! backend retains renderer-private FDs, polls acquire fences, builds mixed
//! composition input, and retires each presentation by transaction identity.

use std::os::fd::OwnedFd;

use sophia_protocol::{
    BufferHandle, DmaBufDescriptor, FenceHandle, Rect, Size, TransactionId, Transform,
};
use sophia_renderer_live::{
    LiveBufferRegistryError, LiveBufferState, LiveCompositionPlacement, LiveCpuBufferSource,
    LiveCpuComposedFrame, LiveDmaBufPresentationRegistry, LiveOwnedDmaBufPlane,
    LiveOwnedMixedCompositionFrame, LiveOwnedMixedCompositionLayer, LiveOwnedMultiPlaneDmaBufFrame,
    LivePresentationDisconnectReport, LivePresentationRegistryLimits, LivePresentationRetirement,
    LiveResourceReleaseStatus,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivePresentationSubmission {
    pub transaction: TransactionId,
    pub buffer: BufferHandle,
    pub acquire_fence: Option<FenceHandle>,
    pub idle_fence: Option<FenceHandle>,
}

#[derive(Debug)]
pub struct LiveRetainedDmaBufLayer {
    pub image_id: sophia_renderer_live::LiveRendererImageId,
    pub frame: LiveOwnedMultiPlaneDmaBufFrame,
    pub placement: LiveCompositionPlacement,
}

impl LiveRetainedDmaBufLayer {
    pub fn try_clone(&self) -> std::io::Result<Self> {
        Ok(Self {
            image_id: self.image_id,
            frame: self.frame.try_clone()?,
            placement: self.placement,
        })
    }

    pub fn has_unit_scale(&self) -> bool {
        let logical = self.placement.clip.unwrap_or(self.placement.target);
        logical.width == i32::try_from(self.frame.width).unwrap_or(i32::MAX)
            && logical.height == i32::try_from(self.frame.height).unwrap_or(i32::MAX)
    }

    pub fn reproject(&mut self, surface: Rect) {
        self.placement = pixel_aligned_dma_buf_placement(
            Size {
                width: i32::try_from(self.frame.width).unwrap_or(i32::MAX),
                height: i32::try_from(self.frame.height).unwrap_or(i32::MAX),
            },
            surface,
            None,
            self.placement.alpha,
        );
    }
}

fn pixel_aligned_dma_buf_placement(
    frame_size: Size,
    surface: Rect,
    clip: Option<Rect>,
    alpha: f32,
) -> LiveCompositionPlacement {
    let target = Rect {
        width: frame_size.width,
        height: frame_size.height,
        ..surface
    };
    let clip = if frame_size.width == surface.width && frame_size.height == surface.height {
        clip
    } else {
        Some(intersect_rects(surface, clip.unwrap_or(surface)))
    };
    LiveCompositionPlacement {
        target,
        clip,
        transform: Transform::IDENTITY,
        alpha,
    }
}

fn intersect_rects(left: Rect, right: Rect) -> Rect {
    let x = left.x.max(right.x);
    let y = left.y.max(right.y);
    let right_edge = left
        .x
        .saturating_add(left.width)
        .min(right.x.saturating_add(right.width));
    let bottom_edge = left
        .y
        .saturating_add(left.height)
        .min(right.y.saturating_add(right.height));
    Rect {
        x,
        y,
        width: right_edge.saturating_sub(x).max(0),
        height: bottom_edge.saturating_sub(y).max(0),
    }
}

pub fn compose_full_state_mixed_frame(
    mut current: LiveOwnedMixedCompositionFrame,
    retained: Vec<LiveRetainedDmaBufLayer>,
) -> LiveOwnedMixedCompositionFrame {
    let current_layer = current.layers.pop();
    current.layers.extend(retained.into_iter().map(
        |LiveRetainedDmaBufLayer {
             image_id,
             frame,
             placement,
         }| LiveOwnedMixedCompositionLayer::DmaBuf {
            image_id,
            frame,
            placement,
        },
    ));
    if let Some(current_layer) = current_layer {
        current.layers.push(current_layer);
    }
    current
}

pub fn try_clone_mixed_frame(
    frame: &LiveOwnedMixedCompositionFrame,
) -> std::io::Result<LiveOwnedMixedCompositionFrame> {
    let layers = frame
        .layers
        .iter()
        .map(|layer| match layer {
            LiveOwnedMixedCompositionLayer::Cpu { buffer, placement } => {
                Ok(LiveOwnedMixedCompositionLayer::Cpu {
                    buffer: buffer.clone(),
                    placement: *placement,
                })
            }
            LiveOwnedMixedCompositionLayer::DmaBuf {
                image_id,
                frame,
                placement,
            } => Ok(LiveOwnedMixedCompositionLayer::DmaBuf {
                image_id: *image_id,
                frame: frame.try_clone()?,
                placement: *placement,
            }),
            LiveOwnedMixedCompositionLayer::Solid { geometry, color } => {
                Ok(LiveOwnedMixedCompositionLayer::Solid {
                    geometry: *geometry,
                    color: *color,
                })
            }
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    Ok(LiveOwnedMixedCompositionFrame {
        layers,
        output_damage_snapshot: frame.output_damage_snapshot.clone(),
    })
}

#[derive(Debug, Default)]
pub struct LivePresentationResourceSession {
    registry: LiveDmaBufPresentationRegistry,
}

impl LivePresentationResourceSession {
    pub fn with_limits(limits: LivePresentationRegistryLimits) -> Self {
        Self {
            registry: LiveDmaBufPresentationRegistry::with_limits(limits),
        }
    }

    pub fn register_source(
        &mut self,
        descriptor: DmaBufDescriptor,
        plane_fds: Vec<OwnedFd>,
    ) -> Result<(), LiveBufferRegistryError> {
        self.registry.register_source(descriptor, plane_fds)
    }

    pub fn register_fence(
        &mut self,
        handle: FenceHandle,
        initially_triggered: bool,
        fd: OwnedFd,
    ) -> Result<(), LiveBufferRegistryError> {
        self.registry
            .register_fence(handle, initially_triggered, fd)
    }

    pub fn begin(
        &mut self,
        submission: LivePresentationSubmission,
    ) -> Result<(), LiveBufferRegistryError> {
        self.registry.begin_present(
            submission.transaction,
            submission.buffer,
            submission.acquire_fence,
            submission.idle_fence,
        )
    }

    pub fn begin_software(
        &mut self,
        transaction: TransactionId,
        acquire_fence: Option<FenceHandle>,
        idle_fence: Option<FenceHandle>,
    ) -> Result<(), LiveBufferRegistryError> {
        self.registry
            .begin_software_present(transaction, acquire_fence, idle_fence)
    }

    pub fn poll_acquire_fence(
        &mut self,
        transaction: TransactionId,
    ) -> Result<bool, LiveBufferRegistryError> {
        self.registry.poll_acquire_fence(transaction)
    }

    pub fn state(&self, transaction: TransactionId) -> Option<LiveBufferState> {
        self.registry.state(transaction)
    }

    pub fn mark_submitted(
        &mut self,
        transaction: TransactionId,
    ) -> Result<(), LiveBufferRegistryError> {
        self.registry.submit(transaction)
    }

    pub fn build_mixed_frame(
        &self,
        transaction: TransactionId,
        cpu_background: Option<LiveCpuComposedFrame>,
        target: Rect,
        clip: Option<Rect>,
        alpha: f32,
    ) -> Result<LiveOwnedMixedCompositionFrame, LiveBufferRegistryError> {
        if self.registry.state(transaction) != Some(LiveBufferState::Ready) {
            return Err(LiveBufferRegistryError::AcquireFencePending);
        }
        let handle = self
            .registry
            .source_for_presentation(transaction)
            .ok_or(LiveBufferRegistryError::UnknownPresentation)?;
        let descriptor = self
            .registry
            .descriptor(handle)
            .ok_or(LiveBufferRegistryError::UnknownHandle)?;
        let mut planes: [Option<LiveOwnedDmaBufPlane>; 4] = std::array::from_fn(|_| None);
        for (index, target_plane) in planes
            .iter_mut()
            .enumerate()
            .take(usize::from(descriptor.plane_count))
        {
            let plane =
                descriptor.planes[index].ok_or(LiveBufferRegistryError::PlaneFdCountMismatch)?;
            *target_plane = Some(LiveOwnedDmaBufPlane {
                fd: self
                    .registry
                    .try_clone_presentation_plane_fd(transaction, index)?,
                offset: plane.offset,
                stride: plane.stride,
            });
        }

        let mut layers = Vec::with_capacity(usize::from(cpu_background.is_some()) + 1);
        if let Some(background) = cpu_background {
            let size = background.size;
            layers.push(LiveOwnedMixedCompositionLayer::Cpu {
                buffer: LiveCpuBufferSource {
                    handle: 0,
                    size,
                    stride: background.stride,
                    format: background.format,
                    generation: 0,
                    bytes: std::sync::Arc::try_unwrap(background.bytes)
                        .unwrap_or_else(|bytes| bytes.as_ref().clone()),
                },
                placement: LiveCompositionPlacement {
                    target: Rect {
                        x: 0,
                        y: 0,
                        width: size.width,
                        height: size.height,
                    },
                    clip: None,
                    transform: Transform::IDENTITY,
                    alpha: 1.0,
                },
            });
        }
        layers.push(LiveOwnedMixedCompositionLayer::DmaBuf {
            image_id: sophia_renderer_live::LiveRendererImageId::from_raw(transaction.raw()),
            frame: LiveOwnedMultiPlaneDmaBufFrame {
                width: descriptor.size.width as u32,
                height: descriptor.size.height as u32,
                format: descriptor.format,
                modifier: descriptor.modifier,
                plane_count: descriptor.plane_count,
                planes,
            },
            placement: pixel_aligned_dma_buf_placement(descriptor.size, target, clip, alpha),
        });
        Ok(LiveOwnedMixedCompositionFrame {
            layers,
            output_damage_snapshot: None,
        })
    }

    pub fn release_source(&mut self, handle: BufferHandle) -> LiveResourceReleaseStatus {
        self.registry.remove_source(handle)
    }

    pub fn release_fence(&mut self, handle: FenceHandle) -> LiveResourceReleaseStatus {
        self.registry.remove_fence(handle)
    }

    pub fn retire_page_flip(
        &mut self,
        transaction: TransactionId,
    ) -> Option<LivePresentationRetirement> {
        self.registry.retire_page_flip(transaction)
    }

    pub fn reject(&mut self, transaction: TransactionId) -> Option<LivePresentationRetirement> {
        self.registry.reject(transaction)
    }

    pub fn disconnect(&mut self) -> LivePresentationDisconnectReport {
        self.registry.disconnect()
    }

    pub fn source_count(&self) -> usize {
        self.registry.source_count()
    }

    pub fn fence_count(&self) -> usize {
        self.registry.fence_count()
    }

    pub fn presentation_count(&self) -> usize {
        self.registry.presentation_count()
    }
}
