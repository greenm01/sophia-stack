#![cfg(target_os = "linux")]

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sophia_protocol::{
    LayoutNodeCapabilities, OutputId, PolicyOutputSnapshot, PolicyPresentationState,
    PolicyProjectionOutcome, PolicyProjectionRequest, PolicyRequestCause, PolicySceneSnapshot,
    PolicySurfaceKind, PolicySurfaceSnapshot, Rect, Size, SurfaceConstraints, SurfaceId,
    TransactionId, decode_wm_v1_policy_projection, encode_wm_v1_policy_snapshot,
};
use sophia_runtime::{PolicyPeerIdentity, PolicyWmSessionTransport, QueuedPolicyProjection};
use sophia_wm_demo::PolicyV1Client;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

#[test]
fn session_and_reference_client_exchange_one_complete_policy_cycle() {
    let directory = std::env::temp_dir().join(format!(
        "sophia-policy-transport-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ));
    let peer = PolicyPeerIdentity {
        uid: rustix::process::geteuid().as_raw(),
        pid: std::process::id(),
    };
    let mut transport = PolicyWmSessionTransport::bind(&directory, peer).unwrap();
    let socket_path = transport.socket_path().to_path_buf();
    let client = std::thread::spawn(move || {
        let mut client = PolicyV1Client::connect(socket_path, Duration::from_secs(1)).unwrap();
        let snapshot = client.receive_snapshot().unwrap();
        let request = client.receive_projection_request().unwrap();
        let proposal = client.tile_once(&snapshot.scene, &request).unwrap();
        client.send_projection(&proposal).unwrap();
        assert_eq!(
            client.receive_projection_outcome(&proposal).unwrap(),
            PolicyProjectionOutcome::Committed
        );
        proposal
    });

    transport
        .accept_and_negotiate(1, Duration::from_secs(1))
        .unwrap();
    let scene = scene();
    let snapshot =
        encode_wm_v1_policy_snapshot(TransactionId::from_raw(29), 1, &scene, &[]).unwrap();
    transport
        .send_snapshot(
            snapshot.transaction,
            &snapshot.begin,
            &snapshot.chunks,
            &snapshot.end,
        )
        .unwrap();
    transport
        .send_projection_request(
            TransactionId::from_raw(30),
            &PolicyProjectionRequest {
                connection_epoch: 1,
                request_id: 1,
                scene_generation: 1,
                affected_outputs: vec![OutputId::from_raw(1)],
                cause: PolicyRequestCause::SceneChanged,
            },
        )
        .unwrap();
    assert_eq!(transport.receive_projection_part().unwrap(), None);
    assert_eq!(transport.receive_projection_part().unwrap(), None);
    assert_eq!(transport.receive_projection_part().unwrap(), None);
    let assembled = match transport.receive_projection_part().unwrap() {
        Some(QueuedPolicyProjection::Admitted(projection)) => projection,
        other => panic!("expected admitted projection, got {other:?}"),
    };
    transport
        .send_projection_outcome(
            assembled.transaction,
            assembled.request_id,
            assembled.base_generation,
            PolicyProjectionOutcome::Committed,
        )
        .unwrap();
    let expected = client.join().unwrap();
    assert_eq!(
        decode_wm_v1_policy_projection(&assembled.into_wire_transfer()),
        Ok(expected)
    );
    transport.disconnect().unwrap();
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
                width: 100,
                height: 100,
            },
            work_area: Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            },
        }],
        surfaces: vec![PolicySurfaceSnapshot {
            surface: SurfaceId::new(3, 1),
            generation: 1,
            current_output: Some(output),
            kind: PolicySurfaceKind::Toplevel,
            capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
            constraints: SurfaceConstraints {
                min_size: Some(Size {
                    width: 20,
                    height: 20,
                }),
                max_size: None,
            },
            exact_size: None,
            requested_state: PolicyPresentationState::default(),
            current_state: PolicyPresentationState::default(),
            transient_owner: None,
            geometry: Rect {
                x: 10,
                y: 10,
                width: 40,
                height: 40,
            },
        }],
        session_operations: Vec::new(),
    }
}
