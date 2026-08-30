use super::*;

pub fn project_native_cursor_logical_viewport(
    position: sophia_protocol::Point,
    logical_viewports: &[(OutputId, sophia_protocol::Rect)],
) -> Result<Option<(OutputId, i32, i32, sophia_protocol::Size)>, &'static str> {
    if !position.x.is_finite() || !position.y.is_finite() {
        return Ok(None);
    }
    let mut seen = BTreeSet::new();
    if logical_viewports.is_empty()
        || logical_viewports.iter().any(|(output, logical)| {
            !output.is_valid() || logical.is_empty() || !seen.insert(*output)
        })
    {
        return Err("hardware cursor projection has invalid logical viewports");
    }
    let x = position.x.floor() as i32;
    let y = position.y.floor() as i32;
    Ok(logical_viewports.iter().find_map(|(output, logical)| {
        (x >= logical.x
            && x < logical.x.saturating_add(logical.width)
            && y >= logical.y
            && y < logical.y.saturating_add(logical.height))
        .then_some((
            *output,
            x.saturating_sub(logical.x),
            y.saturating_sub(logical.y),
            sophia_protocol::Size {
                width: logical.width,
                height: logical.height,
            },
        ))
    }))
}

impl LiveProductionNativeScanout {
    pub fn update_classic_hardware_cursor(
        &mut self,
        position: sophia_protocol::Point,
        logical_viewports: &[(OutputId, sophia_protocol::Rect)],
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
            // Only the atomic path can produce this: an ioctl never waits for
            // a flip. Stated rather than folded into the arm below so that
            // routing this caller through the atomic path later is a compile
            // error here instead of a cursor that silently stops moving.
            crate::LegacyHardwareCursorAdmission::DeferredUpdate => {
                return Err("the legacy cursor path was told to defer an update".into());
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
        let native_outputs = self.outputs();
        let expected = native_outputs
            .iter()
            .map(|output| output.id)
            .collect::<BTreeSet<_>>();
        let actual = logical_viewports
            .iter()
            .map(|(output, _)| *output)
            .collect::<BTreeSet<_>>();
        if expected != actual
            || actual.len() != logical_viewports.len()
            || logical_viewports
                .iter()
                .any(|(_, logical)| logical.is_empty())
        {
            return Err("hardware cursor projection has stale logical viewports".into());
        }
        let mut targets =
            BTreeMap::<usize, Vec<(crate::LibdrmNativePrimaryPlaneSelection, i32, i32)>>::new();
        if let Some((output, logical_x, logical_y, logical_size)) =
            project_native_cursor_logical_viewport(position, logical_viewports)?
        {
            for head_index in self.head_indices(output) {
                let head = &self.heads[head_index];
                let Some((head_x, head_y)) = crate::project_mirror_coordinates(
                    logical_x,
                    logical_y,
                    logical_size,
                    head.output.size,
                    head.mapping,
                ) else {
                    continue;
                };
                targets
                    .entry(head.group)
                    .or_default()
                    .push((head.selection, head_x, head_y));
            }
        }

        let mut visible = false;
        for (group_index, group) in self.groups.iter_mut().enumerate() {
            let group_targets = targets.get(&group_index).map_or(&[][..], Vec::as_slice);
            match group.session.update_classic_hardware_cursors(group_targets) {
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
