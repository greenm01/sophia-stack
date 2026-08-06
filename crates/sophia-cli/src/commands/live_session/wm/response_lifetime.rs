#[derive(Clone, Debug, Eq, PartialEq)]
struct LiveWmLayoutFingerprint(Vec<SurfaceId>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiveWmResponseLifetime {
    Current,
    RestartAndReseed {
        removed_registered_surfaces: usize,
    },
}

impl LiveWmLayoutFingerprint {
    fn capture(layout: &PersistentLiveLayout, state: &WmWorkspaceState) -> Self {
        Self(
            layout
                .layers
                .keys()
                .chain(layout.planning_surfaces.keys())
                .copied()
                .filter(|surface| layout.is_policy_managed(*surface))
                .filter(|surface| state.surface_workspace(*surface).is_some())
                .collect(),
        )
    }

    fn still_matches(&self, layout: &PersistentLiveLayout) -> bool {
        self.0
            .iter()
            .all(|surface| layout.knows_surface(*surface))
    }

    fn reconcile_response_lifetime(
        &self,
        layout: &PersistentLiveLayout,
        workspace_state: &mut WmWorkspaceState,
    ) -> LiveWmResponseLifetime {
        if self.still_matches(layout) {
            return LiveWmResponseLifetime::Current;
        }
        let removed_registered_surfaces = self
            .0
            .iter()
            .filter(|surface| !layout.knows_surface(**surface))
            .filter(|surface| workspace_state.remove_surface(**surface))
            .count();
        LiveWmResponseLifetime::RestartAndReseed {
            removed_registered_surfaces,
        }
    }
}
