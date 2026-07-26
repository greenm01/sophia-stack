impl LiveWmSession {
    fn accept_negotiated_chrome(&mut self, registry: &WmShortcutRegistry) {
        self.wm_chrome_supported = registry.supports_chrome_policy();
        self.chrome = registry.chrome();
        self.stage_visual_chrome(self.candidate_chrome_style());
        println!(
            "sophia_live_wm_chrome schema=1 status=negotiated source={} capability={} clearance={}",
            if self.wm_chrome_supported {
                "wm_policy"
            } else {
                "core_fallback"
            },
            self.wm_chrome_supported,
            self.candidate_chrome_style().clearance(),
        );
    }

    fn surface_chrome_style(&self) -> Option<sophia_engine::SurfaceChromeStyle> {
        Some(self.visual_chrome)
    }

    fn candidate_chrome_style(&self) -> sophia_engine::SurfaceChromeStyle {
        if self.wm_chrome_supported {
            PersistentXtermSessionConfig::wm_surface_chrome_style(self.chrome)
        } else {
            self.fallback_chrome
        }
    }

    fn stage_visual_chrome(&mut self, candidate: sophia_engine::SurfaceChromeStyle) {
        if self.committed == 0 || candidate.clearance() == self.visual_chrome.clearance() {
            self.visual_chrome = candidate;
            self.pending_visual_chrome = None;
        } else {
            self.pending_visual_chrome = Some(candidate);
            self.work_area_relayout_required = true;
        }
    }

    fn set_fallback_chrome(&mut self, style: sophia_engine::SurfaceChromeStyle) {
        self.fallback_chrome = style;
        if !self.wm_chrome_supported {
            self.stage_visual_chrome(style);
        }
    }
}
