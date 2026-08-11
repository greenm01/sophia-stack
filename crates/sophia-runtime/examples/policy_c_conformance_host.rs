#![cfg(target_os = "linux")]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use sophia_engine::PolicyProjectionReducer;
use sophia_protocol::{
    OutputId, PolicyProjectionOutcome, SOPHIA_WM_V1_BEHAVIOR_SCENARIOS, TransactionId,
    decode_wm_v1_policy_projection, encode_wm_v1_policy_snapshot, sophia_wm_v1_behavior_cause,
    sophia_wm_v1_behavior_scene,
};
use sophia_runtime::{PolicyWmSessionTransport, QueuedPolicyProjection};

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
    if scenario != "all" && !SOPHIA_WM_V1_BEHAVIOR_SCENARIOS.contains(&scenario.as_str()) {
        return Err(format!("unknown policy behavior scenario: {scenario}").into());
    }
    // Bind before launch so the session owns the endpoint for its entire
    // lifetime, then narrow admission to the exact child it supervised.
    let mut transport = PolicyWmSessionTransport::bind_for_supervised_uid(
        &directory,
        rustix::process::geteuid().as_raw(),
    )?;
    let socket_path = transport.socket_path().to_path_buf();
    let mut command = Command::new(client);
    if let Some(argument) = client_argument {
        command.arg(argument);
    }
    command.arg(&socket_path);
    if scenario == "all" {
        command.arg(SOPHIA_WM_V1_BEHAVIOR_SCENARIOS.len().to_string());
    }
    let mut child = command.spawn()?;
    transport.authorize_supervised_pid(child.id())?;
    let result = run_host(&mut transport, &scenario);
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

fn run_host(
    transport: &mut PolicyWmSessionTransport,
    scenario: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let scenarios = if scenario == "all" {
        SOPHIA_WM_V1_BEHAVIOR_SCENARIOS.as_slice()
    } else {
        std::slice::from_ref(&scenario)
    };
    let mut reducer = PolicyProjectionReducer::new(scene(scenarios[0])?)?;
    reducer.connect(1)?;
    transport.accept_and_negotiate(1, Duration::from_secs(2))?;
    for (index, scenario) in scenarios.iter().copied().enumerate() {
        let scene = scene(scenario)?;
        if index != 0 && reducer.scene().generation != scene.generation {
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
        let transaction = 29 + u64::try_from(index)? * 2;
        // Gate on what the client actually negotiated, so the corpus exercises the
        // real outbound path rather than assuming every capability is present.
        let snapshot = encode_wm_v1_policy_snapshot(
            TransactionId::from_raw(transaction),
            1,
            &scene,
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
                let successor_name = scenarios
                    .get(index + 1)
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
            return Err(format!("behavior scenario {scenario} had outcome {outcome:?}").into());
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
    transport.disconnect()?;
    Ok(())
}

fn scene(
    scenario: &str,
) -> Result<sophia_protocol::PolicySceneSnapshot, Box<dyn std::error::Error>> {
    sophia_wm_v1_behavior_scene(scenario)
        .ok_or_else(|| format!("unknown policy behavior scenario: {scenario}").into())
}
