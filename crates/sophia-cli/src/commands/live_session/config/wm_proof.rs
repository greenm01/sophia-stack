use super::*;

pub(super) fn parse_wm_proof_controls(
    args: &[String],
    wm_process_configured: bool,
    max_runtime: Option<Duration>,
) -> Result<(Option<PublicPolicyFaultPoint>, Option<WmActionId>), Box<dyn std::error::Error>> {
    let fault_after = arg_value(args, "--wm-proof-fault-after")
        .as_deref()
        .map(PublicPolicyFaultPoint::parse)
        .transpose()?;
    let restart_after_action = arg_value(args, "--wm-proof-restart-after-action")
        .as_deref()
        .map(parse_u64)
        .transpose()?
        .map(WmActionId::from_raw);
    if restart_after_action.is_some_and(|action| !action.is_valid()) {
        return Err("--wm-proof-restart-after-action requires a nonzero action".into());
    }
    if fault_after.is_some() && restart_after_action.is_some() {
        return Err(
            "--wm-proof-fault-after and --wm-proof-restart-after-action are mutually exclusive"
                .into(),
        );
    }
    if (fault_after.is_some() || restart_after_action.is_some())
        && (!wm_process_configured || max_runtime.is_none())
    {
        let flag = if restart_after_action.is_some() {
            "--wm-proof-restart-after-action"
        } else {
            "--wm-proof-fault-after"
        };
        return Err(format!(
            "{flag} requires a configured sophia_wm_v1 --wm-process and --max-runtime-ms"
        )
        .into());
    }
    Ok((fault_after, restart_after_action))
}
