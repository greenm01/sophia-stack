use super::*;

impl LiveProductionNativeScanout {
    pub fn update_classic_hardware_cursor(
        &mut self,
        position: sophia_protocol::Point,
    ) -> Result<crate::ClassicHardwareCursorUpdate, Box<dyn std::error::Error>> {
        if !position.x.is_finite() || !position.y.is_finite() {
            return Ok(crate::ClassicHardwareCursorUpdate::Hidden);
        }
        let primary_in_flight = self.heads.iter().any(|head| head.submitted_at.is_some());
        let initialized = self
            .groups
            .iter()
            .all(|group| group.session.classic_hardware_cursor_initialized());
        match crate::legacy_hardware_cursor_admission(initialized, primary_in_flight) {
            crate::LegacyHardwareCursorAdmission::DeferredInitialization => {
                self.cursor_initialization_deferrals =
                    self.cursor_initialization_deferrals.saturating_add(1);
                return Ok(crate::ClassicHardwareCursorUpdate::Deferred);
            }
            crate::LegacyHardwareCursorAdmission::InitializeThenUpdate => {
                let initialization_started = Instant::now();
                for group in &mut self.groups {
                    if let Err(error) = group.session.initialize_classic_hardware_cursor() {
                        self.max_cursor_initialization = self
                            .max_cursor_initialization
                            .max(initialization_started.elapsed());
                        self.cursor_update_failures = self.cursor_update_failures.saturating_add(1);
                        return Err(
                            format!("hardware cursor initialization failed: {error}").into()
                        );
                    }
                }
                self.max_cursor_initialization = self
                    .max_cursor_initialization
                    .max(initialization_started.elapsed());
            }
            crate::LegacyHardwareCursorAdmission::Update => {}
        }

        let update_started = Instant::now();
        let mut offset_x = 0_i32;
        let mut target = None;
        for head in &self.heads {
            let width = head.output.size.width;
            let height = head.output.size.height;
            let x = position.x.floor() as i32;
            let y = position.y.floor() as i32;
            if x >= offset_x && x < offset_x.saturating_add(width) && y >= 0 && y < height {
                target = Some((head.group, head.selection, x.saturating_sub(offset_x), y));
                break;
            }
            offset_x = offset_x.saturating_add(width);
        }

        let mut visible = false;
        for (group_index, group) in self.groups.iter_mut().enumerate() {
            let group_target = target
                .filter(|(target_group, _, _, _)| *target_group == group_index)
                .map(|(_, selection, x, y)| (selection, x, y));
            match group.session.update_classic_hardware_cursor(group_target) {
                Ok(crate::ClassicHardwareCursorUpdate::Visible) => visible = true,
                Ok(crate::ClassicHardwareCursorUpdate::Hidden) => {}
                Ok(crate::ClassicHardwareCursorUpdate::Deferred) => {
                    self.max_cursor_update = self.max_cursor_update.max(update_started.elapsed());
                    self.cursor_update_failures = self.cursor_update_failures.saturating_add(1);
                    return Err("initialized legacy cursor update was unexpectedly deferred".into());
                }
                Err(error) => {
                    self.max_cursor_update = self.max_cursor_update.max(update_started.elapsed());
                    self.cursor_update_failures = self.cursor_update_failures.saturating_add(1);
                    return Err(format!("hardware cursor update failed: {error}").into());
                }
            }
        }
        self.max_cursor_update = self.max_cursor_update.max(update_started.elapsed());
        if primary_in_flight {
            self.cursor_updates_primary_in_flight =
                self.cursor_updates_primary_in_flight.saturating_add(1);
        }
        if visible {
            self.cursor_updates = self.cursor_updates.saturating_add(1);
            Ok(crate::ClassicHardwareCursorUpdate::Visible)
        } else {
            self.cursor_hidden_updates = self.cursor_hidden_updates.saturating_add(1);
            Ok(crate::ClassicHardwareCursorUpdate::Hidden)
        }
    }
}
