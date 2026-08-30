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
    /// What the card said about a cursor plane, once any group has asked.
    ///
    /// One answer per card, and the groups agree by construction: the buffer
    /// and its format belong to the card, not the head.
    /// Drive the cursor atomically, if the card will take it.
    ///
    /// Asked for rather than assumed: the probe says what the card would
    /// accept, and this says what the session chose. A card that refused
    /// keeps the legacy ioctl, which archive `0004` proved works over
    /// directly scanned frames, so there is nothing to gain by insisting.
    pub fn use_atomic_cursor_plane(&mut self) -> crate::HardwareCursorPath {
        self.cursor_path = match self.cursor_plane_probe() {
            Some(probe) => crate::cursor_path_for_probe(probe),
            None => crate::HardwareCursorPath::LegacyIoctl,
        };
        self.cursor_path
    }

    pub fn cursor_plane_probe(&self) -> Option<crate::CursorPlaneProbe> {
        self.groups
            .iter()
            .find_map(|group| group.session.cursor_plane_probe())
    }

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
        // Where the cursor lands on each head, by head index. Heads absent
        // from this map are heads the pointer is not on, and they are told to
        // hide rather than left alone -- a head with nothing said to it is
        // how a cursor ends up showing on two monitors at once.
        let mut head_positions = BTreeMap::<usize, (i32, i32)>::new();
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
                head_positions.insert(head_index, (head_x, head_y));
                targets
                    .entry(head.group)
                    .or_default()
                    .push((head.selection, head_x, head_y));
            }
        }
        if self.cursor_path == crate::HardwareCursorPath::AtomicPlane {
            return self.commit_atomic_cursor(&head_positions);
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

impl LiveProductionNativeScanout {
    /// Put the cursor where it belongs on every head, atomically.
    ///
    /// One decision per head, taken by `plan_cursor_commit` so the rule lives
    /// in one testable place rather than in this loop. A head the pointer is
    /// not on is told to hide, in its own commit -- the frame path commits
    /// per head, so the model's single group request is not available here
    /// and heads agree across commits rather than within one.
    fn commit_atomic_cursor(
        &mut self,
        head_positions: &BTreeMap<usize, (i32, i32)>,
    ) -> Result<crate::ClassicHardwareCursorUpdate, Box<dyn std::error::Error>> {
        let mut visible = false;
        for index in 0..self.heads.len() {
            let Some((framebuffer, width, height)) = self.groups[self.heads[index].group]
                .session
                .atomic_cursor_resources()
            else {
                continue;
            };
            let placement =
                head_positions
                    .get(&index)
                    .map(|(x, y)| crate::LibdrmNativeCursorPlacement {
                        framebuffer,
                        x: *x,
                        y: *y,
                        width,
                        height,
                    });
            visible |= placement.is_some();
            self.heads[index].pending_cursor = Some(placement);

            let Some(plane) = self.heads[index].selection.cursor_plane() else {
                continue;
            };
            if self.heads[index].cursor_properties.is_none() {
                self.heads[index].cursor_properties = crate::discover_cursor_plane_properties(
                    self.groups[self.heads[index].group]
                        .session
                        .cursor_commit_device(),
                    plane,
                );
            }
            let Some(properties) = self.heads[index].cursor_properties else {
                continue;
            };

            // The CRTC is busy exactly when a frame of ours is in flight on
            // it, which is the same signal the primary submit ladder defers
            // on.
            let admission = crate::hardware_cursor_admission(
                crate::HardwareCursorPath::AtomicPlane,
                true,
                self.heads[index].submitted_at.is_some(),
            );
            match crate::plan_cursor_commit(
                crate::HardwareCursorPath::AtomicPlane,
                admission,
                self.heads[index].pending_cursor,
                self.heads[index].committed_cursor,
                false,
            ) {
                crate::CursorCommitPlan::CommitCursorOnly => {
                    let request = crate::build_native_cursor_only_atomic_request(
                        plane,
                        self.heads[index].selection.crtc_handle(),
                        properties,
                        placement,
                    );
                    let device = self.groups[self.heads[index].group]
                        .session
                        .cursor_commit_device();
                    match crate::submit_native_cursor_only_commit(device, request) {
                        crate::LibdrmNativeAtomicCommitSubmitStatus::Submitted => {
                            // Blocking, so the CRTC is free now and the plane
                            // shows this. Clearing the pending cell is what
                            // stops the next tick paying for the same commit.
                            self.heads[index].committed_cursor = placement;
                            self.heads[index].pending_cursor = None;
                            self.cursor_updates = self.cursor_updates.saturating_add(1);
                        }
                        _ => {
                            // The position stays pending for a later commit.
                            // A cursor that stutters is not a session that
                            // failed.
                            self.cursor_update_failures =
                                self.cursor_update_failures.saturating_add(1);
                        }
                    }
                }
                // Waiting or riding a frame: the position stays pending and
                // some later commit carries it.
                crate::CursorCommitPlan::Wait
                | crate::CursorCommitPlan::RideNextPrimary
                | crate::CursorCommitPlan::Idle => {}
            }
        }
        if visible {
            Ok(crate::ClassicHardwareCursorUpdate::Visible)
        } else {
            self.cursor_hidden_updates = self.cursor_hidden_updates.saturating_add(1);
            Ok(crate::ClassicHardwareCursorUpdate::Hidden)
        }
    }
}
