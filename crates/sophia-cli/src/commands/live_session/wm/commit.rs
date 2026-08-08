impl LiveWmSession {
    const fn max_request(&self) -> Duration {
        self.max_request
    }

    fn pending_request_count(&self) -> usize {
        self.queued_requests
            .len()
            .saturating_add(usize::from(self.in_flight_request.is_some()))
    }

    fn surface_visible_on_output(
        &self,
        surface: SurfaceId,
        output: sophia_protocol::OutputId,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        self.workspace_state
            .surface_visible_on_output(surface, output)
            .map_err(Into::into)
    }

    fn apply_commit_result(
        &mut self,
        mut result: LiveWmCommitResult,
        previous_focus: Option<SurfaceId>,
        output: sophia_protocol::OutputId,
    ) -> Result<LiveWmOwnerCommit, Box<dyn std::error::Error>> {
        let committed = result.update.commit.outcome == TransactionOutcome::Committed;
        let restart_speculative_transport = wm_transport_requires_reseed(&result);
        let physical_action = committed
            .then_some(result.source)
            .flatten()
            .and_then(|source| match source {
                LiveWmProposalSource::Action(action) => Some(action),
                LiveWmProposalSource::Focus(_)
                | LiveWmProposalSource::Manage(_)
                | LiveWmProposalSource::PointerGesture { .. }
                | LiveWmProposalSource::Relayout => None,
            });
        let pointer_gesture = committed
            .then_some(result.source)
            .flatten()
            .and_then(|source| match source {
                LiveWmProposalSource::PointerGesture { mode, .. } => Some(mode),
                _ => None,
            });
        let mut session_action = None;
        let mut workspace_projection = None;
        let mut clear_focus = None;
        if committed && let Some(effects) = result.effects.take() {
            let mut candidate = effects.workspace_state;
            let retained_newer_bounds = candidate.copy_output_bounds_from(&self.workspace_state)?;
            let transaction = effects.transaction;
            let output_state = candidate
                .output(output)
                .ok_or("committed WM state lost its output projection")?;
            let policy_focus = output_state.focus;
            workspace_projection = Some(LiveWmWorkspaceProjection {
                transaction,
                output,
                workspace: output_state.workspace,
                visible_surfaces: candidate.visible_surfaces(output)?.len(),
                focus_present: policy_focus.is_some(),
            });
            self.workspace_state = candidate;
            if retained_newer_bounds {
                self.work_area_relayout_required = true;
            } else if matches!(result.source, Some(LiveWmProposalSource::Relayout)) {
                self.work_area_relayout_required = false;
                if let Some(chrome) = self.pending_visual_chrome.take() {
                    self.visual_chrome = chrome;
                }
            }
            session_action = effects
                .session_action
                .map(|action| (transaction, action.0, action.1));
            clear_focus = hidden_wm_focus_to_clear(transaction, previous_focus, policy_focus);
            self.mark_committed();
        }
        let owner_commit = LiveWmOwnerCommit {
            update: result.update,
            physical_action,
            pointer_gesture,
            session_action,
            workspace_projection,
            clear_focus,
        };
        if restart_speculative_transport {
            // A legacy WM mutates its private model before Sophia can prove
            // and commit the proposed layout. If that proof fails, restart
            // the bridge and seed it from the preserved committed layout;
            // otherwise later responses can name speculative surfaces which
            // do not exist in the Engine's workspace state.
            self.request_transport_restart("uncommitted_proposal", None);
        }
        Ok(owner_commit)
    }
}

fn hidden_wm_focus_to_clear(
    transaction: TransactionId,
    previous_focus: Option<SurfaceId>,
    policy_focus: Option<SurfaceId>,
) -> Option<(TransactionId, SurfaceId)> {
    // Positive focus has one Engine-owned handoff in PersistentLiveLayout.
    // That path can wait for a presented admission to retire; this projection
    // adapter only clears an old focus when policy leaves no visible target.
    policy_focus
        .is_none()
        .then_some(previous_focus)
        .flatten()
        .map(|surface| (transaction, surface))
}

fn wm_transport_requires_reseed(result: &LiveWmCommitResult) -> bool {
    result.update.commit.outcome != TransactionOutcome::Committed && result.source.is_some()
}

impl Drop for LiveWmSession {
    fn drop(&mut self) {
        let _ = self.supervisor.terminate();
        self.transport.take();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}
