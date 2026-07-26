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
        surface_chrome_display_list_for_surfaces(
            output,
            presentation_order,
            &self.chrome_surfaces,
            committed_surfaces,
            self.focused_surface,
            self.surface_chrome_style,
        )
    }

    pub(super) fn record_focus_ring_observation(
        &mut self,
        committed_surfaces: &[CommittedSurfaceState],
        force: bool,
    ) -> Result<(), CompositorDisplayListError> {
        let display_list = self.display_list(committed_surfaces, &self.presentation_order)?;
        let Some(surface) = self.focused_surface else {
            self.last_focus_ring_observation = None;
            return Ok(());
        };
        let Some(border) = display_list.borders().find(|border| {
            matches!(
                border.node,
                CompositorNodeId::SurfaceChrome {
                    surface: border_surface,
                    role: SurfaceChromeRole::FocusRing,
                } if border_surface == surface
            )
        }) else {
            self.last_focus_ring_observation = None;
            return Ok(());
        };
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
        Ok(())
    }

    pub fn take_focus_ring_observation(&mut self) -> Option<LiveFocusRingObservation> {
        self.pending_focus_ring_observation.take()
    }

    pub(super) fn retained_mixed_frame(
        &self,
        cpu_layers: &[LiveCpuPresentationLayer],
    ) -> Result<Option<(TransactionId, LiveOwnedMixedCompositionFrame)>, std::io::Error> {
        let mut transaction = None;
        let mut layers = Vec::with_capacity(self.displayed_surfaces.len().saturating_add(4));
        let committed = self.production.committed_surfaces();
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
        for command in display_list.commands {
            match command {
                CompositorDisplayCommand::Surface { surface } => {
                    if let Some(displayed) = self.displayed_surfaces.get(&surface) {
                        transaction = transaction.or(displayed.retained_transaction);
                        layers.push(LiveOwnedMixedCompositionLayer::DmaBuf {
                            frame: displayed.layer.frame.try_clone()?,
                            placement: displayed.layer.placement,
                        });
                    } else if let Some(layer) =
                        cpu_layers.iter().find(|layer| layer.surface == surface)
                    {
                        layers.push(LiveOwnedMixedCompositionLayer::Cpu {
                            buffer: layer.buffer.clone(),
                            placement: LiveCompositionPlacement {
                                target: layer.geometry,
                                clip: None,
                                transform: Transform::IDENTITY,
                                alpha: 1.0,
                            },
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
        Ok(transaction.map(|transaction| {
            (
                transaction,
                LiveOwnedMixedCompositionFrame {
                    layers,
                    output_damage_snapshot,
                },
            )
        }))
    }
}
