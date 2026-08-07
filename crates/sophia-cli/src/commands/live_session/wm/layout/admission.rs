impl PersistentLiveLayout {
    fn surface_requires_admission(&self, surface: SurfaceId) -> bool {
        !self.bypass_policy_admission
            && self.presentation_roles.get(&surface)
                == Some(&sophia_protocol::SurfacePresentationRole::PolicyManaged)
            && !matches!(
                self.admissions.state(surface),
                sophia_engine::SurfacePresentationAdmissionState::Inactive
                    | sophia_engine::SurfacePresentationAdmissionState::Managed
            )
    }

    fn prime_admission_extent(&mut self, surface: SurfaceId) {
        if !self.surface_requires_admission(surface)
            || self.layout_epochs.recovery_extent(surface).is_some()
        {
            return;
        }
        let Some(extent) = self.layout_epochs.safe_size(surface) else {
            return;
        };
        // Admit the complete pixels the client has already produced before
        // driving it toward the blind WM's final geometry.
        self.layout_epochs.set_recovery_extent(surface, extent);
        self.layout_epochs.set_admission(
            surface,
            sophia_engine::SurfaceAdmissionState::PendingLayout,
        );
        println!(
            "sophia_live_resize_epoch schema=3 status=admission_extent_primed surface={} width={} height={}",
            surface.index(),
            extent.width,
            extent.height,
        );
    }
}
