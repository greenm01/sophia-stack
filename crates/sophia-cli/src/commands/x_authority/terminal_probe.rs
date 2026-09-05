#[cfg(feature = "native-session")]
pub(crate) fn collect_x_authority_xterm_render_authority_batches(
    terminal: &str,
) -> Result<XAuthorityTerminalRenderProof, Box<dyn std::error::Error>> {
    let spec = EXTERNAL_PROBE_SMOKES
        .iter()
        .find(|spec| spec.command_name == "x-authority-xterm-render-smoke")
        .ok_or("xterm render smoke spec is missing")?;
    let command = resolve_external_probe_binary(spec.label, terminal)?;
    let (display, socket_path) = temp_xauthority_display(spec.display_base)?;
    let report = run_x_authority_external_probe_smoke(ExternalProbeInvocation {
        label: spec.label,
        command: &command,
        display_mode: spec.display_mode,
        command_args: spec.args,
        extra_env: &[],
        display,
        socket_path,
        namespace: NamespaceId::from_raw(spec.namespace),
        require_transactions: spec.require_transactions,
        pixel_proof: spec.pixel_proof,
        allow_proof_kill_without_transactions: spec.allow_proof_kill_without_transactions,
        allow_client_failure_without_x_error: spec.allow_client_failure_without_x_error,
        render_device_provider: None,
        pixmap_allocator: None,
        proof_timeout: std::time::Duration::from_secs(spec.proof_timeout_secs),
        isolate_session_bus: false,
    })?;
    let authority_batches =
        authority_intakes_from_observed_transactions(&report.observed_transactions);
    Ok(XAuthorityTerminalRenderProof {
        display: report.display,
        requests: report.requests,
        transactions: report.transactions,
        runtime_committed: report.runtime_committed,
        runtime_surfaces: report.runtime_surfaces,
        cpu_buffers: report.observed_cpu_buffers,
        authority_batches,
    })
}

pub(crate) fn resolve_external_probe_binary(
    label: &str,
    binary: &str,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    sophia_session::support::resolve_external_probe_binary(label, binary)
}
