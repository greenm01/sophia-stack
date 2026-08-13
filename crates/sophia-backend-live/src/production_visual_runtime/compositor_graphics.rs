use super::*;

impl LiveProductionVisualRuntime {
    pub(super) fn display_list(
        &self,
        committed_surfaces: &[CommittedSurfaceState],
        presentation_order: &[SurfaceId],
    ) -> Result<CompositorDisplayList, CompositorDisplayListError> {
        let output = self
            .outputs
            .primary_output()
            .ok_or(CompositorDisplayListError::InvalidOutput)?;
        let mut display_list = surface_chrome_display_list_for_surfaces(
            output,
            presentation_order,
            &self.chrome_surfaces,
            committed_surfaces,
            self.focused_surface,
            self.surface_chrome_style,
        )?;
        if let Some(outline) = self.floating_outline {
            if display_list.commands.len() >= MAX_COMPOSITOR_DISPLAY_COMMANDS {
                return Err(CompositorDisplayListError::CapacityExceeded);
            }
            let border = compositor_floating_outline(
                outline.surface,
                outline.geometry,
                self.surface_chrome_style.focus_ring.width.max(2),
                self.surface_chrome_style.focus_ring.color,
            )
            .ok_or(CompositorDisplayListError::InvalidSurface)?;
            display_list
                .commands
                .push(CompositorDisplayCommand::Border(border));
        }
        Ok(display_list)
    }

    pub fn set_floating_outline(
        &mut self,
        outline: Option<LiveFloatingOutline>,
        scene: &LiveProductionCpuScene,
        native_scanout: Option<&mut LiveProductionNativeScanout>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        if self.floating_outline == outline {
            return Ok(false);
        }
        self.floating_outline = outline;
        let cpu_layers = scene.presentation_layers(
            self.production.committed_surfaces(),
            &self.presentation_order,
        );
        if let (Some(native_scanout), Some(frame)) =
            (native_scanout, self.retained_mixed_frame(&cpu_layers)?)
        {
            let primary = self
                .outputs
                .primary_output()
                .ok_or("persistent backend runtime has no primary output")?;
            native_scanout.queue_retained_mixed_frame(primary, frame);
        }
        Ok(true)
    }

    pub(super) fn record_focus_ring_observation(
        &mut self,
        committed_surfaces: &[CommittedSurfaceState],
        force: bool,
    ) -> Result<(), CompositorDisplayListError> {
        let display_list = self.display_list(committed_surfaces, &self.presentation_order)?;
        if let Some(surface) = self.focused_surface {
            if let Some(border) = display_list.borders().find(|border| {
                matches!(
                    border.node,
                    CompositorNodeId::SurfaceChrome {
                        surface: border_surface,
                        role: SurfaceChromeRole::FocusRing,
                    } if border_surface == surface
                )
            }) {
                let observation = LiveFocusRingObservation {
                    surface,
                    generation: border.generation,
                    primitives: compositor_border_bands(border)
                        .into_iter()
                        .filter(|band| !band.geometry.is_empty())
                        .count(),
                };
                if observation.primitives > 0
                    && (force || self.last_focus_ring_observation != Some(observation))
                {
                    self.last_focus_ring_observation = Some(observation);
                    self.pending_focus_ring_observation = Some(observation);
                }
            } else {
                self.last_focus_ring_observation = None;
            }
        } else {
            self.last_focus_ring_observation = None;
        }
        let summary = compositor_chrome_summary(&display_list, self.focused_surface);
        let observation = LiveChromeSetObservation {
            generation: summary.generation,
            eligible_surfaces: self
                .chrome_surfaces
                .iter()
                .filter(|surface| self.presentation_order.contains(surface))
                .count(),
            frames: summary.frames,
            focused_frames: summary.focused_frames,
            unfocused_frames: summary.unfocused_frames,
            focus_rings: summary.focus_rings,
            primitives: summary.primitives,
            clearance: summary.clearance,
        };
        if self.last_chrome_set_observation != Some(observation) {
            self.last_chrome_set_observation = Some(observation);
            self.pending_chrome_set_observation = Some(observation);
        }
        Ok(())
    }

    pub fn take_focus_ring_observation(&mut self) -> Option<LiveFocusRingObservation> {
        self.pending_focus_ring_observation.take()
    }

    pub fn take_chrome_set_observation(&mut self) -> Option<LiveChromeSetObservation> {
        self.pending_chrome_set_observation.take()
    }

    pub(super) fn retained_mixed_frame(
        &self,
        cpu_layers: &[LiveCpuPresentationLayer],
    ) -> Result<Option<LiveOwnedMixedCompositionFrame>, std::io::Error> {
        let mut retained_client_image = false;
        let mut layers = Vec::with_capacity(self.displayed_surfaces.len().saturating_add(4));
        // A serialized software frame may follow a DMA frame that has not yet
        // retired. Its display list must preserve that prepared transaction.
        let committed = self
            .present_scheduler
            .in_flight_candidate()
            .unwrap_or_else(|| self.production.committed_surfaces());
        let display_list = self
            .display_list(committed, &self.presentation_order)
            .map_err(std::io::Error::other)?;
        let output = self
            .outputs
            .output_descriptor(0)
            .ok_or_else(|| std::io::Error::other("mixed composition has no output descriptor"))?;
        let output_damage_snapshot = Some(
            output_frame_damage_snapshot(output, display_list.clone(), committed, None)
                .map_err(std::io::Error::other)?,
        );
        let in_flight = self.present_scheduler.in_flight_displayed_layer();
        for command in display_list.commands {
            match command {
                CompositorDisplayCommand::Surface { surface } => {
                    if let Some((_, displayed)) =
                        in_flight.filter(|(in_flight_surface, _)| *in_flight_surface == surface)
                    {
                        retained_client_image = true;
                        layers.push(LiveOwnedMixedCompositionLayer::RendererImage {
                            image_id: displayed.image_id,
                            size: displayed.size,
                            format: displayed.format,
                            placement: displayed.placement,
                        });
                    } else if let Some(layer) =
                        cpu_layers.iter().find(|layer| layer.surface == surface)
                    {
                        retained_client_image = true;
                        layers.push(LiveOwnedMixedCompositionLayer::Cpu {
                            buffer: layer.buffer.clone(),
                            placement: LiveCompositionPlacement {
                                target: layer.geometry,
                                clip: None,
                                transform: Transform::IDENTITY,
                                alpha: 1.0,
                            },
                        });
                    } else if let Some(displayed) = self.displayed_surfaces.get(&surface) {
                        retained_client_image = true;
                        layers.push(LiveOwnedMixedCompositionLayer::RendererImage {
                            image_id: displayed.layer.image_id,
                            size: displayed.layer.size,
                            format: displayed.layer.format,
                            placement: displayed.layer.placement,
                        });
                    }
                }
                CompositorDisplayCommand::Border(border) => {
                    for band in compositor_border_bands(border) {
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
        Ok(
            retained_client_image.then_some(LiveOwnedMixedCompositionFrame {
                layers,
                output_damage_snapshot,
            }),
        )
    }
}
