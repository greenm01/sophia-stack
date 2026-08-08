#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use sophia_engine::PolicyProjectionReducer;
use sophia_protocol::{
    LayoutNodeCapabilities, OutputId, PolicyOutputSnapshot, PolicyPresentationState,
    PolicyProjectionOutcome, PolicySceneSnapshot, PolicySurfaceKind, PolicySurfaceSnapshot, Rect,
    Size, SurfaceConstraints, SurfaceId, TransactionId, decode_wm_v1_policy_projection,
    encode_wm_v1_policy_snapshot,
};
use sophia_runtime::{PolicyPeerIdentity, PolicyWmSessionTransport, QueuedPolicyProjection};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let client = PathBuf::from(arguments.next().ok_or("missing C client path")?);
    let directory = PathBuf::from(arguments.next().ok_or("missing session directory")?);
    if arguments.next().is_some() {
        return Err("unexpected argument".into());
    }
    let socket_path = directory.join("wm.sock");
    // The client waits for the socket, allowing the session to learn its exact
    // PID before creating the credential-checked endpoint.
    let mut child = Command::new(client).arg(&socket_path).spawn()?;
    let peer = PolicyPeerIdentity {
        uid: rustix::process::geteuid().as_raw(),
        pid: child.id(),
    };
    let result = run_host(directory, peer);
    if result.is_err() {
        let _ = child.kill();
    }
    let status = child.wait()?;
    result?;
    if !status.success() {
        return Err(format!("C client exited with {status}").into());
    }
    Ok(())
}

fn run_host(
    directory: PathBuf,
    peer: PolicyPeerIdentity,
) -> Result<(), Box<dyn std::error::Error>> {
    let scene = scene();
    let mut reducer = PolicyProjectionReducer::new(scene.clone())?;
    reducer.connect(1)?;
    let request = reducer.issue_request(vec![OutputId::from_raw(1)])?;
    let mut transport = PolicyWmSessionTransport::bind(directory, peer)?;
    transport.accept_and_negotiate(1, Duration::from_secs(2))?;
    let snapshot = encode_wm_v1_policy_snapshot(TransactionId::from_raw(29), 1, &scene, &[])
        .map_err(|error| format!("snapshot encode failed: {error:?}"))?;
    transport.send_snapshot(
        snapshot.transaction,
        &snapshot.begin,
        &snapshot.chunks,
        &snapshot.end,
    )?;
    transport.send_projection_request(TransactionId::from_raw(30), &request)?;

    let mut admitted = None;
    for _ in 0..=sophia_runtime::POLICY_MAX_TRANSFER_CHUNKS + 1 {
        if let Some(projection) = transport.receive_projection_part()? {
            admitted = Some(projection);
            break;
        }
    }
    let QueuedPolicyProjection::Admitted(projection) = admitted.ok_or("incomplete projection")?
    else {
        return Err("projection was discarded".into());
    };
    let proposal = decode_wm_v1_policy_projection(&projection.into_wire_transfer())
        .map_err(|error| format!("projection decode failed: {error:?}"))?;
    let outcome = reducer.apply_proposal(&proposal);
    if outcome != PolicyProjectionOutcome::Committed {
        return Err("canonical reducer rejected C projection".into());
    }
    transport.send_projection_outcome(
        proposal.transaction,
        proposal.request_id,
        reducer.scene().generation,
        outcome,
    )?;
    let committed = reducer.committed();
    if committed.len() != 1
        || committed[0].placements.len() != 2
        || committed[0].placements[1].geometry.x != 600
        || committed[0].focus != Some(SurfaceId::new(3, 1))
    {
        return Err("C projection did not preserve the expected semantics".into());
    }
    transport.disconnect()?;
    Ok(())
}

fn scene() -> PolicySceneSnapshot {
    let output = OutputId::from_raw(1);
    PolicySceneSnapshot {
        generation: 1,
        outputs: vec![PolicyOutputSnapshot {
            output,
            generation: 1,
            focus: Some(SurfaceId::new(3, 1)),
            bounds: Rect {
                x: 0,
                y: 0,
                width: 1200,
                height: 800,
            },
            work_area: Rect {
                x: 0,
                y: 0,
                width: 1200,
                height: 800,
            },
        }],
        surfaces: vec![surface(3, output), surface(4, output)],
        session_operations: Vec::new(),
    }
}

fn surface(index: u32, output: OutputId) -> PolicySurfaceSnapshot {
    PolicySurfaceSnapshot {
        surface: SurfaceId::new(index, 1),
        generation: 1,
        current_output: Some(output),
        kind: PolicySurfaceKind::Toplevel,
        capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        constraints: SurfaceConstraints {
            min_size: Some(Size {
                width: 100,
                height: 80,
            }),
            max_size: None,
        },
        exact_size: None,
        requested_state: PolicyPresentationState::default(),
        current_state: PolicyPresentationState::default(),
        transient_owner: None,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 600,
            height: 800,
        },
    }
}
