#![cfg(target_os = "linux")]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::Duration;

use sophia_engine::PolicyProjectionReducer;
use sophia_protocol::{
    OutputId, PolicyProjectionOutcome, SOPHIA_WM_V1_BEHAVIOR_SCENARIOS, TransactionId,
    WmV1ProfileIdentity, decode_wm_v1_policy_projection, encode_wm_v1_policy_snapshot,
    sophia_wm_v1_behavior_cause, sophia_wm_v1_behavior_scene,
};
use sophia_runtime::{PolicyClientEvent, PolicyWmSessionTransport, QueuedPolicyProjection};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let client = PathBuf::from(arguments.next().ok_or("missing policy client path")?);
    let directory = PathBuf::from(arguments.next().ok_or("missing session directory")?);
    let scenario = arguments
        .next()
        .map(|value| value.into_string().map_err(|_| "scenario is not UTF-8"))
        .transpose()?
        .unwrap_or_else(|| SOPHIA_WM_V1_BEHAVIOR_SCENARIOS[0].to_owned());
    let client_argument = arguments
        .next()
        .map(|value| {
            value
                .into_string()
                .map_err(|_| "client argument is not UTF-8")
        })
        .transpose()?;
    if arguments.next().is_some() {
        return Err("unexpected argument".into());
    }
    if scenario != "all"
        && scenario != "restart"
        && scenario != "configured-all"
        && scenario != "configured-restart"
        && !SOPHIA_WM_V1_BEHAVIOR_SCENARIOS.contains(&scenario.as_str())
    {
        return Err(format!("unknown policy behavior scenario: {scenario}").into());
    }
    // Bind before launch so the session owns the endpoint for its entire
    // lifetime, then narrow admission to the exact child it supervised.
    let configured = scenario.starts_with("configured-");
    let mut transport = if configured {
        PolicyWmSessionTransport::bind_for_supervised_uid_profile_activation(
            &directory,
            rustix::process::geteuid().as_raw(),
        )?
    } else {
        PolicyWmSessionTransport::bind_for_supervised_uid(
            &directory,
            rustix::process::geteuid().as_raw(),
        )?
    };
    let socket_path = transport.socket_path().to_path_buf();
    if scenario.ends_with("restart") {
        return run_restart_host(
            &mut transport,
            &client,
            client_argument.as_deref(),
            &socket_path,
            configured,
        );
    }
    let cycles = if scenario.ends_with("all") {
        SOPHIA_WM_V1_BEHAVIOR_SCENARIOS.len()
    } else {
        1
    };
    let mut child = spawn_client(&client, client_argument.as_deref(), &socket_path, cycles)?;
    transport.authorize_supervised_pid(child.id())?;
    let normalized_scenario = if scenario == "configured-all" {
        "all"
    } else {
        &scenario
    };
    let result = run_host(&mut transport, normalized_scenario, configured);
    if result.is_err() {
        let _ = child.kill();
    }
    let status = child.wait()?;
    result?;
    if !status.success() {
        return Err(format!("policy client exited with {status}").into());
    }
    Ok(())
}

fn spawn_client(
    client: &std::path::Path,
    client_argument: Option<&str>,
    socket_path: &std::path::Path,
    cycles: usize,
) -> Result<Child, std::io::Error> {
    let mut command = Command::new(client);
    if let Some(argument) = client_argument {
        command.arg(argument);
    }
    command.arg(socket_path).arg(cycles.to_string()).spawn()
}

fn run_restart_host(
    transport: &mut PolicyWmSessionTransport,
    client: &std::path::Path,
    client_argument: Option<&str>,
    socket_path: &std::path::Path,
    configured: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    const RESTART_AFTER: usize = 5;
    let phases = if configured {
        vec![
            &SOPHIA_WM_V1_BEHAVIOR_SCENARIOS[..RESTART_AFTER],
            &SOPHIA_WM_V1_BEHAVIOR_SCENARIOS[RESTART_AFTER..6],
            &SOPHIA_WM_V1_BEHAVIOR_SCENARIOS[6..8],
            &SOPHIA_WM_V1_BEHAVIOR_SCENARIOS[8..10],
            &SOPHIA_WM_V1_BEHAVIOR_SCENARIOS[10..],
        ]
    } else {
        vec![
            &SOPHIA_WM_V1_BEHAVIOR_SCENARIOS[..RESTART_AFTER],
            &SOPHIA_WM_V1_BEHAVIOR_SCENARIOS[RESTART_AFTER..],
        ]
    };
    let process_count = phases.len();
    let mut reducer = PolicyProjectionReducer::new(scene(phases[0][0])?)?;
    let mut preserved: Option<Vec<sophia_protocol::PolicyOutputProjection>> = None;
    let mut scenario_offset = 0;
    for (phase_index, scenarios) in phases.into_iter().enumerate() {
        let epoch = u64::try_from(phase_index + 1)?;
        let mut child = spawn_client(client, client_argument, socket_path, scenarios.len())?;
        transport.authorize_supervised_pid(child.id())?;
        reducer.connect(epoch)?;
        connect_client(transport, epoch, configured)?;
        if let Some(expected) = &preserved
            && reducer.committed() != expected.as_slice()
        {
            return Err("policy restart changed the last committed projection".into());
        }
        let result = run_scenario_cycles(transport, &mut reducer, scenarios, scenario_offset);
        if result.is_err() {
            let _ = child.kill();
        }
        let status = child.wait()?;
        result?;
        let expected_success = !configured
            || !matches!(
                scenarios.last().copied(),
                Some("timeout-discard" | "stale-discard" | "invalid-discard")
            );
        if status.success() != expected_success {
            return Err(format!("policy client restart phase exited with {status}").into());
        }
        transport.disconnect()?;
        if reducer.disconnect(epoch) != PolicyProjectionOutcome::Disconnected {
            return Err("policy reducer rejected an active disconnect".into());
        }
        preserved = Some(reducer.committed().to_vec());
        scenario_offset += scenarios.len();
    }
    println!(
        "sophia_policy_restart_corpus schema=1 status=complete revision=3 processes={process_count} connection_epochs={process_count} scenarios={} preserved_commit=true configured={configured}",
        SOPHIA_WM_V1_BEHAVIOR_SCENARIOS.len(),
    );
    Ok(())
}

fn run_host(
    transport: &mut PolicyWmSessionTransport,
    scenario: &str,
    configured: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let scenarios = if scenario == "all" {
        SOPHIA_WM_V1_BEHAVIOR_SCENARIOS.as_slice()
    } else {
        std::slice::from_ref(&scenario)
    };
    let mut reducer = PolicyProjectionReducer::new(scene(scenarios[0])?)?;
    reducer.connect(1)?;
    connect_client(transport, 1, configured)?;
    run_scenario_cycles(transport, &mut reducer, scenarios, 0)?;
    transport.disconnect()?;
    Ok(())
}

fn connect_client(
    transport: &mut PolicyWmSessionTransport,
    connection_epoch: u64,
    configured: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    transport.accept_and_negotiate(connection_epoch, Duration::from_secs(8))?;
    if !configured {
        return Ok(());
    }
    let identity = WmV1ProfileIdentity::new(connection_epoch, 1, [0x71; 32])
        .map_err(|error| format!("invalid configured profile identity: {error:?}"))?;
    transport.activate_profile_handoff(
        identity,
        TransactionId::from_raw(1),
        TransactionId::from_raw(2),
    )?;
    let PolicyClientEvent::Configuration {
        transaction,
        configuration,
    } = transport.receive_client_event()?
    else {
        return Err("configured policy did not send its action catalog".into());
    };
    if configuration.connection_epoch != connection_epoch
        || configuration.generation != 1
        || configuration.actions.is_empty()
    {
        return Err("configured policy sent an invalid action catalog".into());
    }
    transport.send_configuration_outcome(
        transaction,
        configuration.generation,
        PolicyProjectionOutcome::Committed,
    )?;
    Ok(())
}

fn run_scenario_cycles(
    transport: &mut PolicyWmSessionTransport,
    reducer: &mut PolicyProjectionReducer,
    scenarios: &[&str],
    scenario_offset: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    for (index, scenario) in scenarios.iter().copied().enumerate() {
        let scene = scene(scenario)?;
        if reducer.scene().generation != scene.generation {
            reducer.observe_scene(scene.clone())?;
        }
        let mut affected_outputs = scene
            .outputs
            .iter()
            .map(|output| output.output)
            .collect::<Vec<_>>();
        affected_outputs.sort_by_key(|output| (*output != scene.active_output, output.raw()));
        let cause = sophia_wm_v1_behavior_cause(scenario)
            .ok_or_else(|| format!("behavior scenario {scenario} has no cause"))?;
        let request = reducer.issue_request_with_cause(affected_outputs, cause)?;
        let transaction = 29 + u64::try_from(scenario_offset + index)? * 2;
        // Gate on what the client actually negotiated, so the corpus exercises the
        // real outbound path rather than assuming every capability is present.
        let snapshot = encode_wm_v1_policy_snapshot(
            TransactionId::from_raw(transaction),
            request.connection_epoch,
            &scene,
            &[],
            &[],
            transport.selected_capabilities(),
        )
        .map_err(|error| format!("snapshot encode failed: {error:?}"))?;
        transport.send_snapshot(
            snapshot.transaction,
            &snapshot.begin,
            &snapshot.chunks,
            &snapshot.end,
        )?;
        transport.send_projection_request(TransactionId::from_raw(transaction + 1), &request)?;

        let mut admitted = None;
        for _ in 0..=sophia_runtime::POLICY_MAX_TRANSFER_CHUNKS + 1 {
            if let Some(projection) = transport.receive_projection_part()? {
                admitted = Some(projection);
                break;
            }
        }
        let QueuedPolicyProjection::Admitted(projection) =
            admitted.ok_or("incomplete projection")?
        else {
            return Err("projection was discarded".into());
        };
        let proposal = decode_wm_v1_policy_projection(&projection.into_wire_transfer())
            .map_err(|error| format!("projection decode failed: {error:?}"))?;
        let outcome = match scenario {
            "timeout-discard" => reducer.timeout(proposal.request_id),
            "stale-discard" => {
                let successor_name = SOPHIA_WM_V1_BEHAVIOR_SCENARIOS
                    .get(scenario_offset + index + 1)
                    .copied()
                    .ok_or("stale scenario has no successor")?;
                let successor = sophia_wm_v1_behavior_scene(successor_name)
                    .ok_or("stale scenario successor is unknown")?;
                reducer.observe_scene(successor)?;
                reducer.apply_proposal(&proposal)
            }
            "invalid-discard" => {
                let mut invalid = proposal.clone();
                invalid.active_output = OutputId::from_raw(0);
                reducer.apply_proposal(&invalid)
            }
            _ => reducer.apply_proposal(&proposal),
        };
        let expected_outcome = match scenario {
            "timeout-discard" => PolicyProjectionOutcome::TimedOut,
            "stale-discard" => PolicyProjectionOutcome::RejectedStale,
            "invalid-discard" => PolicyProjectionOutcome::RejectedInvalid,
            _ => PolicyProjectionOutcome::Committed,
        };
        if outcome != expected_outcome {
            return Err(format!(
                "behavior scenario {scenario} had outcome {outcome:?}: proposal={proposal:?}"
            )
            .into());
        }
        transport.send_projection_outcome(
            proposal.transaction,
            proposal.request_id,
            reducer.scene().generation,
            outcome,
        )?;
        let expected_surfaces = scene
            .surfaces
            .iter()
            .filter(|surface| surface.current_output.is_some())
            .map(|surface| surface.surface)
            .collect::<BTreeSet<_>>();
        if outcome == PolicyProjectionOutcome::Committed {
            let committed = reducer.committed();
            let committed_surfaces = committed
                .iter()
                .flat_map(|output| output.placements.iter())
                .map(|placement| placement.surface)
                .collect::<BTreeSet<_>>();
            if committed.len() != scene.outputs.len()
                || committed_surfaces != expected_surfaces
                || reducer.scene().active_output != scene.active_output
            {
                return Err(format!(
                    "behavior scenario {scenario} lost canonical semantics: outputs={}/{} surfaces={committed_surfaces:?}/{expected_surfaces:?} active={}/{}",
                    committed.len(),
                    scene.outputs.len(),
                    reducer.scene().active_output.raw(),
                    scene.active_output.raw(),
                )
                .into());
            }
        }
        println!(
            "sophia_policy_behavior schema=1 scenario={scenario} status=complete outcome={outcome:?} outputs={} surfaces={}",
            scene.outputs.len(),
            expected_surfaces.len(),
        );
    }
    Ok(())
}

fn scene(
    scenario: &str,
) -> Result<sophia_protocol::PolicySceneSnapshot, Box<dyn std::error::Error>> {
    sophia_wm_v1_behavior_scene(scenario)
        .ok_or_else(|| format!("unknown policy behavior scenario: {scenario}").into())
}
