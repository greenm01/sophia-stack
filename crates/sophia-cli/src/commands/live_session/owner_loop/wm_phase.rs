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
        if let Some(effect) = wm.take_output_topology_effect() {
            let native = native_scanout
                .as_ref()
                .ok_or("output authority effect has no native scanout owner")?;
            let plan = native.plan_output_topology(&effect.resolved);
            match &plan {
                Ok(plan) => tracing::info!(
                    "sophia_live_output_authority schema=1 status=native_plan_ready transaction={} base_epoch={} candidate_epoch={} heads={} outputs={} preserved_topology=true",
                    effect.transaction.raw(),
                    effect.base_topology_epoch,
                    effect.candidate_topology_epoch,
                    plan.heads.len(),
                    plan.outputs.len(),
                ),
                Err(error) => tracing::warn!(
                    "sophia_live_output_authority schema=1 status=native_plan_failed transaction={} error={error} preserved_topology=true",
                    effect.transaction.raw(),
                ),
            }
            // The proposal now crosses into the visual/session owner before it
            // can settle. Until the native replacement transaction below is
            // wired, fail at this owner boundary without mutating KMS or
            // publishing the provisional topology.
            wm.reject_output_topology_effect(
                effect.transaction,
                sophia_engine::OutputTopologyTransactionFailure::Preparation,
            )?;
            tracing::info!(
                "sophia_live_output_authority schema=1 status=effect_refused transaction={} phase={} preserved_topology=true",
                effect.transaction.raw(),
                if plan.is_ok() { "target_preparation" } else { "native_plan" },
            );
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
