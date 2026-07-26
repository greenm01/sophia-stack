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
        focused_surface_display_list(
            output,
            presentation_order,
            committed_surfaces,
            self.focused_surface,
            FocusedSurfaceBorderStyle::default(),
        )
    }

    pub(super) fn record_focused_border_observation(
        &mut self,
        committed_surfaces: &[CommittedSurfaceState],
        force: bool,
    ) -> Result<(), CompositorDisplayListError> {
        let display_list = self.display_list(committed_surfaces, &self.presentation_order)?;
        let Some(surface) = self.focused_surface else {
            self.last_focused_border_observation = None;
            return Ok(());
        };
        let Some(border) = display_list.solid_rects().find(|border| {
            matches!(
                border.node,
                CompositorNodeId::FocusedSurfaceBorder {
                    surface: border_surface,
                    ..
                } if border_surface == surface
            )
        }) else {
            self.last_focused_border_observation = None;
            return Ok(());
        };
        let observation = LiveFocusedBorderObservation {
            surface,
            generation: border.generation,
            primitives: display_list.solid_rects().count(),
        };
        if observation.primitives > 0
            && (force || self.last_focused_border_observation != Some(observation))
        {
            self.last_focused_border_observation = Some(observation);
            self.pending_focused_border_observation = Some(observation);
        }
        Ok(())
    }

    pub fn take_focused_border_observation(&mut self) -> Option<LiveFocusedBorderObservation> {
        self.pending_focused_border_observation.take()
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
                CompositorDisplayCommand::SolidRect(rect) => {
                    layers.push(LiveOwnedMixedCompositionLayer::Solid {
                        geometry: rect.geometry,
                        color: rect.color,
                    });
                }
            }
        }
        Ok(transaction.map(|transaction| (transaction, LiveOwnedMixedCompositionFrame { layers })))
    }
}
