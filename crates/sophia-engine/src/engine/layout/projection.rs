use super::*;

impl HeadlessEngine {
    pub fn committed_state_from_layer(&self, layer: &LayerSnapshot) -> CommittedSurfaceState {
        CommittedSurfaceState::from_layer_snapshot(layer)
    }

    pub fn project_committed_surface_state(
        &self,
        committed: &CommittedSurfaceState,
        template: &LayerSnapshot,
    ) -> Result<LayerSnapshot, EngineError> {
        if !committed.surface.is_valid() || !template.surface.is_valid() {
            warn!(
                committed_index = committed.surface.index(),
                committed_generation = committed.surface.generation(),
                template_index = template.surface.index(),
                template_generation = template.surface.generation(),
                "rejected projection of an invalid surface id"
            );
            return Err(EngineError::InvalidSurface);
        }
        if committed.surface != template.surface {
            warn!(
                committed_index = committed.surface.index(),
                committed_generation = committed.surface.generation(),
                template_index = template.surface.index(),
                template_generation = template.surface.generation(),
                "rejected projection pairing two different surfaces"
            );
            return Err(EngineError::InvalidSurface);
        }

        let mut layer = template.clone();
        layer.geometry = committed.geometry;
        layer.source = committed.buffer();
        layer.damage = committed.damage.clone();
        layer.generation = committed.committed_generation;
        Ok(layer)
    }

    pub fn project_committed_surface_states(
        &self,
        committed: &[CommittedSurfaceState],
        templates: &[LayerSnapshot],
    ) -> Result<Vec<LayerSnapshot>, EngineError> {
        let templates_by_surface = templates
            .iter()
            .map(|template| (template.surface, template))
            .collect::<BTreeMap<_, _>>();

        committed
            .iter()
            .map(|state| {
                let Some(template) = templates_by_surface.get(&state.surface) else {
                    // A committed surface with no template means the tick's two
                    // views of the scene came from different moments or different
                    // coordinators. Failing closed is right; failing silently cost
                    // three hardware round-trips to name, so the rejection now
                    // names its subject like every other rejection here.
                    warn!(
                        surface_index = state.surface.index(),
                        surface_generation = state.surface.generation(),
                        committed_surfaces = committed.len(),
                        templates = templates.len(),
                        "rejected projection of a committed surface with no template"
                    );
                    return Err(EngineError::InvalidSurface);
                };
                self.project_committed_surface_state(state, template)
            })
            .collect()
    }
}
