use sophia_engine::{
    CompositorDisplayCommand, CompositorDisplayList, HeadlessOutput, OutputFrameDamageSnapshot,
    output_frame_damage_snapshot,
};
use sophia_protocol::{BufferSource, CommittedSurfaceState, Point, Rect, Size, SurfaceId};

use crate::{
    LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, LiveCpuBufferRegistry, LiveCpuBufferSource,
    LiveCpuBufferSourceRef, LiveCpuBufferUpdate, LiveCpuComposedFrame,
    LiveCpuCompositionElementRef, LiveCpuCompositionLayer, LiveCpuCompositionLayerRef,
    LiveCpuCompositionReport, LiveCpuFrameMetricsMode,
    compose_live_cpu_display_list_frame_with_metrics_reusing, compose_live_cpu_frame,
    compose_live_cpu_frame_ref_with_cursor,
};

#[derive(Clone)]
pub struct LiveProductionComposedFrame {
    pub frame: LiveCpuComposedFrame,
    pub checksum: u64,
    pub nonzero_pixel_bytes: usize,
    pub output_damage_snapshot: Option<OutputFrameDamageSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveCpuPresentationLayer {
    pub surface: SurfaceId,
    pub geometry: Rect,
    pub buffer: LiveCpuBufferSource,
}

pub struct LiveProductionCpuScene {
    output_size: Size,
    buffers: LiveCpuBufferRegistry,
    last_report: Option<LiveCpuCompositionReport>,
    last_output_damage_snapshot: Option<OutputFrameDamageSnapshot>,
    max_nonzero_pixel_bytes: usize,
    nonzero_frames: usize,
    exact_pixel_proofs_remaining: usize,
    exact_pixel_metric_frames: usize,
    damage_scoped_metric_frames: usize,
}

impl LiveProductionCpuScene {
    pub fn new(output_size: Size) -> Self {
        Self {
            output_size,
            buffers: LiveCpuBufferRegistry::new(),
            last_report: None,
            last_output_damage_snapshot: None,
            max_nonzero_pixel_bytes: 0,
            nonzero_frames: 0,
            exact_pixel_proofs_remaining: 3,
            exact_pixel_metric_frames: 0,
            damage_scoped_metric_frames: 0,
        }
    }

    pub fn apply_updates(
        &mut self,
        updates: impl IntoIterator<Item = LiveCpuBufferUpdate>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for update in updates {
            self.buffers
                .apply(update)
                .map_err(|error| format!("renderer CPU buffer update failed: {error:?}"))?;
        }
        Ok(())
    }

    pub fn reconcile_buffer_residency(&mut self, retained_handles: &[u64]) {
        self.buffers
            .retain_handles(|handle| retained_handles.binary_search(&handle).is_ok());
    }

    pub fn resident_buffer_count(&self) -> usize {
        self.buffers.len()
    }

    pub fn resident_buffer_bytes(&self) -> usize {
        self.buffers.total_bytes()
    }

    pub fn missing_committed_buffer_count(
        &self,
        committed_surfaces: &[CommittedSurfaceState],
    ) -> usize {
        committed_surfaces
            .iter()
            .filter_map(|surface| match surface.buffer {
                BufferSource::CpuBuffer { handle } => Some(handle),
                _ => None,
            })
            .filter(|handle| !self.buffers.contains(*handle))
            .count()
    }

    pub fn compose(
        &mut self,
        committed_surfaces: &[CommittedSurfaceState],
        raised_surface: Option<SurfaceId>,
        cursor_position: Option<Point>,
    ) -> Result<&LiveCpuCompositionReport, Box<dyn std::error::Error>> {
        self.compose_ordered(committed_surfaces, None, raised_surface, cursor_position)
    }

    pub fn compose_visible(
        &mut self,
        committed_surfaces: &[CommittedSurfaceState],
        presentation_order: &[SurfaceId],
        raised_surface: Option<SurfaceId>,
        cursor_position: Option<Point>,
    ) -> Result<&LiveCpuCompositionReport, Box<dyn std::error::Error>> {
        self.compose_ordered(
            committed_surfaces,
            Some(presentation_order),
            raised_surface,
            cursor_position,
        )
    }

    pub fn presentation_layers(
        &self,
        committed_surfaces: &[CommittedSurfaceState],
        presentation_order: &[SurfaceId],
    ) -> Vec<LiveCpuPresentationLayer> {
        presentation_order
            .iter()
            .filter_map(|surface| {
                let committed = committed_surfaces
                    .iter()
                    .find(|committed| committed.surface == *surface)?;
                let BufferSource::CpuBuffer { handle } = committed.buffer else {
                    return None;
                };
                Some(LiveCpuPresentationLayer {
                    surface: *surface,
                    geometry: committed.geometry,
                    buffer: self.buffers.get(handle)?.clone(),
                })
            })
            .collect()
    }

    pub fn compose_display_list(
        &mut self,
        output: HeadlessOutput,
        committed_surfaces: &[CommittedSurfaceState],
        display_list: &CompositorDisplayList,
        cursor_position: Option<Point>,
    ) -> Result<&LiveCpuCompositionReport, Box<dyn std::error::Error>> {
        if output.size != self.output_size {
            return Err("CPU scene output descriptor has a mismatched size".into());
        }
        let mut elements = Vec::with_capacity(display_list.commands.len().saturating_mul(4));
        for command in &display_list.commands {
            match command {
                CompositorDisplayCommand::Surface { surface } => {
                    let committed = committed_surfaces
                        .iter()
                        .find(|committed| committed.surface == *surface);
                    let Some(committed) = committed else {
                        continue;
                    };
                    let BufferSource::CpuBuffer { handle } = committed.buffer else {
                        continue;
                    };
                    let Some(buffer) = self.buffers.get(handle) else {
                        continue;
                    };
                    elements.push(LiveCpuCompositionElementRef::Layer(
                        LiveCpuCompositionLayerRef {
                            geometry: committed.geometry,
                            buffer: LiveCpuBufferSourceRef {
                                handle: buffer.handle,
                                size: buffer.size,
                                stride: buffer.stride,
                                format: buffer.format,
                                generation: buffer.generation,
                                bytes: &buffer.bytes,
                            },
                        },
                    ));
                }
                CompositorDisplayCommand::Border(border) => {
                    for band in sophia_engine::compositor_border_bands(*border) {
                        if !band.geometry.is_empty() {
                            elements.push(LiveCpuCompositionElementRef::Solid {
                                geometry: band.geometry,
                                color: band.color,
                            });
                        }
                    }
                }
            }
        }
        let metrics_mode = if self.exact_pixel_proofs_remaining == 0 {
            self.damage_scoped_metric_frames = self.damage_scoped_metric_frames.saturating_add(1);
            LiveCpuFrameMetricsMode::DamageScopedEvidence
        } else {
            self.exact_pixel_proofs_remaining = self.exact_pixel_proofs_remaining.saturating_sub(1);
            self.exact_pixel_metric_frames = self.exact_pixel_metric_frames.saturating_add(1);
            LiveCpuFrameMetricsMode::ExactPixels
        };
        let reusable_bytes = self.last_report.take().map(|report| report.frame.bytes);
        self.last_report = Some(
            compose_live_cpu_display_list_frame_with_metrics_reusing(
                self.output_size,
                &elements,
                cursor_position,
                metrics_mode,
                reusable_bytes,
            )
            .map_err(|error| {
                format!("persistent CPU display-list composition failed: {error:?}")
            })?,
        );
        let cursor_geometry = cursor_position.map(|position| Rect {
            x: position.x.floor() as i32,
            y: position.y.floor() as i32,
            width: i32::try_from(crate::DEFAULT_CURSOR_EDGE).unwrap_or(i32::MAX),
            height: i32::try_from(crate::DEFAULT_CURSOR_EDGE).unwrap_or(i32::MAX),
        });
        self.last_output_damage_snapshot = Some(output_frame_damage_snapshot(
            output,
            display_list.clone(),
            committed_surfaces,
            cursor_geometry,
        )?);
        self.record_last_report();
        Ok(self.last_report.as_ref().expect("assigned above"))
    }

    fn compose_ordered(
        &mut self,
        committed_surfaces: &[CommittedSurfaceState],
        presentation_order: Option<&[SurfaceId]>,
        raised_surface: Option<SurfaceId>,
        cursor_position: Option<Point>,
    ) -> Result<&LiveCpuCompositionReport, Box<dyn std::error::Error>> {
        let mut surface_order = match presentation_order {
            Some(order) => order
                .iter()
                .filter_map(|surface| {
                    committed_surfaces
                        .iter()
                        .find(|committed| committed.surface == *surface)
                })
                .collect::<Vec<_>>(),
            None => committed_surfaces.iter().collect::<Vec<_>>(),
        };
        surface_order.retain(|surface| Some(surface.surface) != raised_surface);
        if let Some(raised) = raised_surface
            && let Some(surface) = committed_surfaces
                .iter()
                .find(|surface| surface.surface == raised)
            && presentation_order.is_none_or(|order| order.contains(&raised))
        {
            surface_order.push(surface);
        }
        let layers = surface_order
            .iter()
            .filter_map(|surface| {
                let BufferSource::CpuBuffer { handle } = surface.buffer else {
                    return None;
                };
                let buffer = self.buffers.get(handle)?;
                Some(LiveCpuCompositionLayerRef {
                    geometry: surface.geometry,
                    buffer: LiveCpuBufferSourceRef {
                        handle: buffer.handle,
                        size: buffer.size,
                        stride: buffer.stride,
                        format: buffer.format,
                        generation: buffer.generation,
                        bytes: &buffer.bytes,
                    },
                })
            })
            .collect::<Vec<_>>();
        self.last_report = Some(
            compose_live_cpu_frame_ref_with_cursor(self.output_size, &layers, cursor_position)
                .map_err(|error| format!("persistent CPU composition failed: {error:?}"))?,
        );
        self.last_output_damage_snapshot = None;
        self.record_last_report();
        Ok(self.last_report.as_ref().expect("assigned above"))
    }

    fn record_last_report(&mut self) {
        let nonzero_pixel_bytes = self
            .last_report
            .as_ref()
            .expect("assigned above")
            .nonzero_pixel_bytes;
        self.max_nonzero_pixel_bytes = self.max_nonzero_pixel_bytes.max(nonzero_pixel_bytes);
        self.nonzero_frames = self
            .nonzero_frames
            .saturating_add(usize::from(nonzero_pixel_bytes > 0));
    }

    pub fn last_report(&self) -> Option<&LiveCpuCompositionReport> {
        self.last_report.as_ref()
    }

    pub fn max_nonzero_pixel_bytes(&self) -> usize {
        self.max_nonzero_pixel_bytes
    }

    pub fn nonzero_frames(&self) -> usize {
        self.nonzero_frames
    }

    pub fn exact_pixel_metric_frames(&self) -> usize {
        self.exact_pixel_metric_frames
    }

    pub fn damage_scoped_metric_frames(&self) -> usize {
        self.damage_scoped_metric_frames
    }

    pub fn buffer_checksum(&self) -> u64 {
        self.buffers.checksum()
    }

    pub fn surface_buffer_generation(
        &self,
        committed_surfaces: &[CommittedSurfaceState],
        surface: SurfaceId,
    ) -> Option<u64> {
        let committed = committed_surfaces
            .iter()
            .find(|committed| committed.surface == surface)?;
        let BufferSource::CpuBuffer { handle } = committed.buffer else {
            return None;
        };
        Some(self.buffers.get(handle)?.generation)
    }

    /// Returns true only when the focused surface contains at least two
    /// visible XRGB pixel values. A newly mapped xterm initially publishes a
    /// uniform background buffer; its prompt or cursor introduces visual
    /// detail once the terminal side is ready for input. Inspecting the
    /// focused surface avoids treating another client's draw as readiness.
    pub fn surface_has_visual_detail(
        &self,
        committed_surfaces: &[CommittedSurfaceState],
        surface: SurfaceId,
    ) -> bool {
        let Some(committed) = committed_surfaces
            .iter()
            .find(|committed| committed.surface == surface)
        else {
            return false;
        };
        let BufferSource::CpuBuffer { handle } = committed.buffer else {
            return false;
        };
        let Some(buffer) = self.buffers.get(handle) else {
            return false;
        };
        let Ok(width) = usize::try_from(buffer.size.width) else {
            return false;
        };
        let Ok(height) = usize::try_from(buffer.size.height) else {
            return false;
        };
        let Ok(stride) = usize::try_from(buffer.stride) else {
            return false;
        };
        let Some(row_bytes) = width.checked_mul(4) else {
            return false;
        };
        if width == 0 || height == 0 || stride < row_bytes || buffer.bytes.len() < 4 {
            return false;
        }
        let first = &buffer.bytes[..4];
        (0..height).any(|row| {
            let Some(start) = row.checked_mul(stride) else {
                return false;
            };
            let Some(end) = start.checked_add(row_bytes) else {
                return false;
            };
            buffer
                .bytes
                .get(start..end)
                .is_some_and(|bytes| bytes.chunks_exact(4).any(|pixel| pixel != first))
        })
    }

    pub fn frames_for_outputs(
        &self,
        outputs: &[HeadlessOutput],
    ) -> Result<Vec<LiveProductionComposedFrame>, Box<dyn std::error::Error>> {
        let primary = self
            .last_report
            .as_ref()
            .ok_or("persistent CPU scene has no composed primary frame")?;
        let mut frames = Vec::with_capacity(outputs.len());
        for (index, output) in outputs.iter().enumerate() {
            if index == 0 && output.size == primary.frame.size {
                frames.push(LiveProductionComposedFrame {
                    frame: primary.frame.clone(),
                    checksum: primary.checksum,
                    nonzero_pixel_bytes: primary.nonzero_pixel_bytes,
                    output_damage_snapshot: self
                        .last_output_damage_snapshot
                        .as_ref()
                        .filter(|snapshot| snapshot.output == *output)
                        .cloned(),
                });
                continue;
            }
            let marker_size = Size {
                width: output.size.width.clamp(1, 64),
                height: output.size.height.clamp(1, 64),
            };
            let marker_width = usize::try_from(marker_size.width)?;
            let marker_height = usize::try_from(marker_size.height)?;
            let marker_stride = marker_width
                .checked_mul(4)
                .ok_or("marker stride overflow")?;
            let marker_byte = u8::try_from((index + 1).min(255)).unwrap_or(255);
            let marker = LiveCpuCompositionLayer {
                geometry: Rect {
                    x: 0,
                    y: 0,
                    width: marker_size.width,
                    height: marker_size.height,
                },
                buffer: LiveCpuBufferSource {
                    handle: 0x5350_4800u64.saturating_add(index as u64),
                    size: marker_size,
                    stride: u32::try_from(marker_stride)?,
                    format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                    generation: 1,
                    bytes: vec![marker_byte; marker_stride.saturating_mul(marker_height)],
                },
            };
            let report = compose_live_cpu_frame(output.size, &[marker])
                .map_err(|error| format!("secondary output composition failed: {error:?}"))?;
            frames.push(LiveProductionComposedFrame {
                frame: report.frame,
                checksum: report.checksum,
                nonzero_pixel_bytes: report.nonzero_pixel_bytes,
                output_damage_snapshot: Some(output_frame_damage_snapshot(
                    *output,
                    CompositorDisplayList::empty(output.id),
                    &[],
                    None,
                )?),
            });
        }
        Ok(frames)
    }
}
