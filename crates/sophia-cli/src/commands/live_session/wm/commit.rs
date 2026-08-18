impl LiveWmSession {
    const fn max_request(&self) -> Duration {
        self.max_request
    }

    /// Requests already sent to the policy peer and not yet settled.
    ///
    /// Distinct from `pending_request_count`, which also counts causes still
    /// queued locally. At a runtime deadline only this matters: a queued cause
    /// was promised to nobody and can be dropped when the session stops, while
    /// an issued request is owed an answer. Counting the queue there never
    /// converges anyway, because a moving pointer keeps raising fresh causes
    /// for as long as the drain waits.
    fn in_flight_request_count(&self) -> usize {
        if let Some(public) = self.public.as_ref() {
            return usize::from(public.in_flight_request.is_some());
        }
        usize::from(self.in_flight_request.is_some())
    }

    fn pending_request_count(&self) -> usize {
        if let Some(public) = self.public.as_ref() {
            return public
                .queue
                .len()
                .saturating_add(usize::from(public.in_flight_request.is_some()));
        }
        self.queued_requests
            .len()
            .saturating_add(usize::from(self.in_flight_request.is_some()))
    }

    fn surface_visible_on_output(
        &self,
        surface: SurfaceId,
        output: sophia_protocol::OutputId,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        if let Some(public) = self.public.as_ref() {
            return Ok(public.reducer.committed().into_iter().any(|projection| {
                projection.output == output
                    && projection
                        .placements
                        .iter()
                        .any(|placement| placement.surface == surface)
            }));
        }
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
        if let Some(settlement) = result.policy_settlement.take() {
            return self.apply_public_commit_result(result, settlement);
        }
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

impl LiveWmSession {
    fn topology_policy_commit_serial(&self) -> u64 {
        self.public
            .as_ref()
            .map_or_else(
                || u64::try_from(self.committed).unwrap_or(u64::MAX),
                |public| public.reducer.commit_serial(),
            )
    }

    fn apply_public_commit_result(
        &mut self,
        result: LiveWmCommitResult,
        settlement: LivePolicySettlementIdentity,
    ) -> Result<LiveWmOwnerCommit, Box<dyn std::error::Error>> {
        let public = self.public.as_mut().ok_or("public WM settlement lost its session")?;
        let layout_committed = result.update.commit.outcome == TransactionOutcome::Committed;
        if settlement.session_operation {
            let (transaction, request) = public
                .pending_operation
                .take()
                .ok_or("public session operation settled without a pending request")?;
            if transaction != settlement.transaction
                || request.connection_epoch != settlement.connection_epoch
                || request.request_id != settlement.request_id
            {
                return Err("public session-operation settlement identity changed".into());
            }
            let action = public.operation_actions.get(&request.operation).copied();
            let operation = public
                .session_operations
                .iter()
                .find(|operation| operation.token == request.operation);
            let valid_target = match (operation, request.target) {
                (Some(operation), Some(_)) => operation.permits_surface_target,
                (Some(_), None) => true,
                (None, _) => false,
            };
            let outcome = if layout_committed && action.is_some() && valid_target {
                sophia_protocol::PolicyProjectionOutcome::Committed
            } else {
                sophia_protocol::PolicyProjectionOutcome::RejectedInvalid
            };
            public.submit_or_defer(PolicyTransportCommand::SessionOperationOutcome {
                    transaction,
                    request_id: request.request_id,
                    outcome,
                })?;
            self.trigger_public_proof_fault(PublicPolicyFaultPoint::TerminalOutcomeQueued);
            return Ok(LiveWmOwnerCommit {
                update: result.update,
                physical_action: None,
                pointer_gesture: None,
                session_action: if outcome == sophia_protocol::PolicyProjectionOutcome::Committed {
                    action.map(|action| (transaction, action, request.target))
                } else {
                    None
                },
                workspace_projection: None,
                clear_focus: None,
            });
        }

        let prepared = public.prepared.take();
        let outcome = if layout_committed && prepared == Some(settlement) {
            let staged = public
                .staged
                .take()
                .ok_or("public projection committed without its staged reducer successor")?;
            public.reducer.commit_staged(staged)
        } else if layout_committed {
            let staged = public
                .staged
                .take()
                .ok_or("public projection settled without its staged reducer successor")?;
            public.reducer.commit_staged(staged)
        } else {
            public.staged = None;
            public.reducer.timeout(settlement.request_id)
        };
        public.submit_or_defer(PolicyTransportCommand::ProjectionOutcome {
                transaction: settlement.transaction,
                request_id: settlement.request_id,
                scene_generation: public.reducer.scene().generation,
                outcome,
                expect_session_operation: settlement.expect_session_operation,
            })?;
        public.settle_public_projection(outcome);
        if outcome != sophia_protocol::PolicyProjectionOutcome::Committed
            || !settlement.expect_session_operation
        {
            public.expected_operation_slot = None;
        }
        if outcome == sophia_protocol::PolicyProjectionOutcome::Committed {
            public.active_output = public.reducer.scene().active_output;
            self.mark_committed();
        } else {
            self.stale_responses = self.stale_responses.saturating_add(1);
        }
        let physical_action = if outcome == sophia_protocol::PolicyProjectionOutcome::Committed {
            match result.source {
                Some(LiveWmProposalSource::Action(action)) => Some(action),
                _ => None,
            }
        } else {
            None
        };
        self.trigger_public_proof_fault(PublicPolicyFaultPoint::TerminalOutcomeQueued);
        Ok(LiveWmOwnerCommit {
            update: result.update,
            physical_action,
            pointer_gesture: None,
            session_action: None,
            workspace_projection: None,
            clear_focus: None,
        })
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
