{
    if layout.constraint_relayout_required()
        && layout.pending.is_none()
        && let Some(wm) = wm_session.as_mut()
    {
        match wm.enqueue_relayout(&layout, output)? {
            LiveWmRequestAdmission::Admitted | LiveWmRequestAdmission::Duplicate => {
                layout.acknowledge_constraint_relayout();
            }
            LiveWmRequestAdmission::RejectedCapacity => {
                return Err("WM recovery-constraint relayout exceeded owner capacity".into());
            }
        }
    }
    if let Some(wm) = wm_session.as_mut() {
        let _ = wm.poll_restart(&mut layout, output)?;
        if let Some(transaction) = active_output_topology_preparation {
            let native = native_scanout
                .as_mut()
                .ok_or("output topology preparation lost its native owner")?;
            let report = native.service_output_topology_preparation()?;
            tracing::info!(
                "sophia_live_output_authority schema=1 status=resource_progress transaction={} phase={:?} candidate_prepared={} rollback_prepared={} heads={} kms_submits=0 preserved_topology=true",
                transaction.raw(),
                report.phase,
                report.candidate_prepared,
                report.rollback_prepared,
                report.affected_heads,
            );
            if report.phase
                == sophia_backend_live::LiveProductionNativeTopologyPreparationPhase::Prepared
            {
                let plan = native.cancel_prepared_output_topology()?;
                wm.reject_output_topology_effect(
                    transaction,
                    sophia_engine::OutputTopologyTransactionFailure::Preparation,
                )?;
                active_output_topology_preparation = None;
                tracing::info!(
                    "sophia_live_output_authority schema=1 status=effect_refused transaction={} phase=apply_not_yet_wired heads={} candidate_and_rollback_prepared=true kms_submits=0 preserved_topology=true",
                    transaction.raw(),
                    plan.heads.len(),
                );
            } else if report.phase
                == sophia_backend_live::LiveProductionNativeTopologyPreparationPhase::Failed
            {
                let (plan, error) = native.finish_failed_output_topology_preparation()?;
                wm.reject_output_topology_effect(
                    transaction,
                    sophia_engine::OutputTopologyTransactionFailure::Preparation,
                )?;
                active_output_topology_preparation = None;
                tracing::warn!(
                    "sophia_live_output_authority schema=1 status=resource_preparation_rejected transaction={} heads={} error={error} kms_submits=0 preserved_topology=true",
                    transaction.raw(),
                    plan.heads.len(),
                );
            }
        }
        if active_output_topology_preparation.is_none()
            && let Some(effect) = wm.take_output_topology_effect()
        {
            let preparation = (|| -> Result<_, Box<dyn std::error::Error>> {
                let native = native_scanout
                    .as_mut()
                    .ok_or("output authority effect has no native scanout owner")?;
                let plan = native.plan_output_topology(&effect.resolved)?;
                let rollback = native.published_output_topology(&effect.published_snapshot)?;
                let runtime = runtime
                    .as_ref()
                    .ok_or("output authority effect has no visual runtime")?;
                let candidate_frames = runtime.compose_output_topology_head_frames(
                    &scene,
                    &effect.resolved,
                    effect.candidate_topology_epoch,
                )?;
                let rollback_frames = runtime.compose_output_topology_head_frames(
                    &scene,
                    &rollback,
                    effect.candidate_topology_epoch,
                )?;
                let heads = plan.heads.len();
                let outputs = plan.outputs.len();
                let report = native.begin_output_topology_preparation(
                    plan,
                    candidate_frames,
                    rollback_frames,
                )?;
                Ok((heads, outputs, report))
            })();
            match preparation {
                Ok((heads, outputs, report)) => {
                    active_output_topology_preparation = Some(effect.transaction);
                    tracing::info!(
                        "sophia_live_output_authority schema=1 status=resource_preparation_started transaction={} base_epoch={} candidate_epoch={} heads={} outputs={} phase={:?} candidate_prepared={} rollback_prepared={} kms_submits=0 preserved_topology=true",
                        effect.transaction.raw(),
                        effect.base_topology_epoch,
                        effect.candidate_topology_epoch,
                        heads,
                        outputs,
                        report.phase,
                        report.candidate_prepared,
                        report.rollback_prepared,
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        "sophia_live_output_authority schema=1 status=resource_preparation_failed transaction={} error={error} kms_submits=0 preserved_topology=true",
                        effect.transaction.raw(),
                    );
                    wm.reject_output_topology_effect(
                        effect.transaction,
                        sophia_engine::OutputTopologyTransactionFailure::Preparation,
                    )?;
                }
            }
        }
    }
    synchronize_wm_pointer_epoch!();
    if let Some(runtime) = runtime.as_mut() {
        let style = wm_session
            .as_ref()
            .and_then(|wm| wm.surface_chrome_style())
            .unwrap_or(config.surface_chrome_style);
        synchronize_runtime_surface_chrome_style(runtime, style);
    }
    if pending_wm_update.is_none()
        && layout.pending.is_none()
        && let Some(wm) = wm_session.as_mut()
        && let Some(proposal) = wm.poll_request(&mut layout, output)?
    {
        let public_projection = proposal
            .policy_settlement
            .is_some_and(|settlement| !settlement.session_operation);
        if public_projection
            && wm.trigger_public_proof_fault(PublicPolicyFaultPoint::ProposalStaged)
        {
            let _ = wm.poll_restart(&mut layout, output)?;
        } else {
            let previous_focus = focus.focused_surface(seat);
            if let Some(result) = layout.stage(proposal, &mut session_controls)? {
                pending_wm_update = Some(apply_wm_commit_result!(result, previous_focus));
            } else if public_projection
                && wm.trigger_public_proof_fault(PublicPolicyFaultPoint::FrontendPending)
            {
                let _ = wm.poll_restart(&mut layout, output)?;
            }
        }
    }
    service_layout_progress!("wm_stage");
}
