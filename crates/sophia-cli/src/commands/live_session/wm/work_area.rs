impl LiveWmSession {
    fn has_current_relayout_request(&self, layout: &PersistentLiveLayout) -> bool {
        let fingerprint = LiveWmLayoutFingerprint::capture(layout, &self.workspace_state);
        self.in_flight_request
            .iter()
            .chain(self.queued_requests.iter())
            .any(|request| {
                matches!(
                    &request.kind,
                    LiveWmQueuedKind::Proposal {
                        base_state,
                        fingerprint: pending_fingerprint,
                        source: LiveWmProposalSource::Relayout,
                        ..
                    } if *base_state == self.workspace_state
                        && *pending_fingerprint == fingerprint
                )
            })
    }

    fn update_output_work_areas(
        &mut self,
        layout: &PersistentLiveLayout,
        outputs: &[sophia_engine::HeadlessOutput],
        primary: sophia_engine::HeadlessOutput,
    ) -> Result<LiveWmRequestAdmission, Box<dyn std::error::Error>> {
        let full_bounds = wm_output_bounds(outputs);
        let root = full_bounds.iter().try_fold(
            Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            |root, (_, bounds)| {
                Some(Rect {
                    x: 0,
                    y: 0,
                    width: root.width.max(bounds.x.checked_add(bounds.width)?),
                    height: root.height.max(bounds.y.checked_add(bounds.height)?),
                })
            },
        );
        let Some(root) = root.filter(|root| !root.is_empty()) else {
            return Err("live WM output topology has no valid root bounds".into());
        };
        let work_areas = sophia_engine::reduce_output_work_areas(
            root,
            full_bounds,
            &layout.active_output_reservations(),
        );
        let mut changed = 0usize;
        let mut rejected = 0usize;
        for area in &work_areas {
            let Some(work) = area.work else {
                rejected = rejected.saturating_add(1);
                println!(
                    "sophia_live_work_area schema=1 status=preserved output={} reason=invalid_reduction full={}x{}_{}_{}",
                    area.output.raw(),
                    area.full.width,
                    area.full.height,
                    area.full.x,
                    area.full.y,
                );
                continue;
            };
            println!(
                "sophia_live_work_area schema=1 status=applied output={} full={}x{}_{}_{} work={}x{}_{}_{}",
                area.output.raw(),
                area.full.width,
                area.full.height,
                area.full.x,
                area.full.y,
                work.width,
                work.height,
                work.x,
                work.y,
            );
            changed = changed.saturating_add(usize::from(
                self.workspace_state
                    .update_output_bounds(area.output, work)?,
            ));
        }
        println!(
            "sophia_live_work_area schema=1 status=reduced outputs={} changed={} rejected={} active_reservations={}",
            work_areas.len(),
            changed,
            rejected,
            layout.active_output_reservations().len(),
        );
        if changed == 0 {
            return Ok(LiveWmRequestAdmission::Duplicate);
        }
        let managed_surfaces_visible = work_areas.iter().any(|area| {
            self.workspace_state
                .visible_surfaces(area.output)
                .is_ok_and(|surfaces| !surfaces.is_empty())
        });
        self.work_area_relayout_required = managed_surfaces_visible;
        if !managed_surfaces_visible {
            return Ok(LiveWmRequestAdmission::Duplicate);
        }
        self.enqueue_relayout(layout, primary)
    }
}
