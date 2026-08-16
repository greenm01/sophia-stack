#![cfg(target_os = "linux")]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sophia_protocol::*;
use sophia_runtime::*;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

#[test]
fn output_transport_exchanges_snapshot_proposal_and_terminal_outcome() {
    let directory = std::env::temp_dir().join(format!(
        "sophia-output-transport-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ));
    let peer = PolicyPeerIdentity {
        uid: rustix::process::geteuid().as_raw(),
        pid: std::process::id(),
    };
    let mut transport = OutputSessionTransport::bind(&directory, peer).unwrap();
    let socket_path = transport.socket_path().to_owned();
    let snapshot = snapshot();
    let expected_snapshot = snapshot.clone();
    let client = std::thread::spawn(move || {
        let mut stream = UnixStream::connect(socket_path).unwrap();
        stream
            .write_all(
                &encode_output_v1_client_hello_frame(OutputV1ClientHello {
                    minimum_revision: 1,
                    maximum_revision: 1,
                    capabilities: SOPHIA_OUTPUT_CAPABILITY_OBSERVE
                        | SOPHIA_OUTPUT_CAPABILITY_CONFIGURE,
                })
                .unwrap(),
            )
            .unwrap();
        let welcome = decode_output_v1_server_welcome_frame(&read_frame(&mut stream)).unwrap();
        assert_eq!(welcome.connection_epoch, 11);

        let (_, observed) = decode_output_v1_snapshot_frame(&read_frame(&mut stream)).unwrap();
        assert_eq!(observed.snapshot, expected_snapshot);
        let proposal = proposal(welcome.connection_epoch);
        stream
            .write_all(
                &encode_output_v1_proposal_frame(TransactionId::from_raw(8), &proposal).unwrap(),
            )
            .unwrap();
        decode_output_v1_outcome_frame(&read_frame(&mut stream)).unwrap()
    });

    transport
        .accept_and_negotiate(11, Duration::from_secs(1))
        .unwrap();
    transport
        .send_snapshot(TransactionId::from_raw(7), &snapshot)
        .unwrap();
    let (transaction, admission) = transport.receive_proposal(&snapshot).unwrap();
    assert_eq!(transaction, TransactionId::from_raw(8));
    assert_eq!(admission, OutputProposalAdmission::Active);
    transport
        .send_outcome(
            transaction,
            OutputV1Outcome {
                connection_epoch: 11,
                topology_epoch: 12,
                kind: OutputV1OutcomeKind::Committed,
                reason: 0,
            },
        )
        .unwrap();
    transport.settle_active(transaction).unwrap();
    let (received_transaction, outcome) = client.join().unwrap();
    assert_eq!(received_transaction, transaction);
    assert_eq!(outcome.kind, OutputV1OutcomeKind::Committed);
    assert_eq!(outcome.topology_epoch, 12);
    transport.disconnect().unwrap();
}

#[test]
fn output_transport_buffers_partial_proposal_without_blocking_owner() {
    let directory = std::env::temp_dir().join(format!(
        "sophia-output-transport-partial-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ));
    let peer = PolicyPeerIdentity {
        uid: rustix::process::geteuid().as_raw(),
        pid: std::process::id(),
    };
    let mut transport = OutputSessionTransport::bind(&directory, peer).unwrap();
    let socket_path = transport.socket_path().to_owned();
    let snapshot = snapshot();
    let expected = proposal(12);
    let sent = expected.clone();
    let (header_sent, header_observed) = std::sync::mpsc::sync_channel(1);
    let (continue_sender, continue_receiver) = std::sync::mpsc::sync_channel(1);
    let client = std::thread::spawn(move || {
        let mut stream = UnixStream::connect(socket_path).unwrap();
        stream
            .write_all(
                &encode_output_v1_client_hello_frame(OutputV1ClientHello {
                    minimum_revision: 1,
                    maximum_revision: 1,
                    capabilities: SOPHIA_OUTPUT_CAPABILITY_OBSERVE
                        | SOPHIA_OUTPUT_CAPABILITY_CONFIGURE,
                })
                .unwrap(),
            )
            .unwrap();
        decode_output_v1_server_welcome_frame(&read_frame(&mut stream)).unwrap();
        decode_output_v1_snapshot_frame(&read_frame(&mut stream)).unwrap();
        let frame = encode_output_v1_proposal_frame(TransactionId::from_raw(17), &sent).unwrap();
        stream.write_all(&frame[..SOPHIA_IPC_HEADER_LEN]).unwrap();
        header_sent.send(()).unwrap();
        continue_receiver.recv().unwrap();
        stream.write_all(&frame[SOPHIA_IPC_HEADER_LEN..]).unwrap();
    });

    transport
        .accept_and_negotiate(12, Duration::from_secs(1))
        .unwrap();
    transport
        .send_snapshot(TransactionId::from_raw(16), &snapshot)
        .unwrap();
    header_observed
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    assert_eq!(
        transport.try_receive_admitted_proposal(&snapshot).unwrap(),
        None
    );
    continue_sender.send(()).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    let (admitted, admission) = loop {
        if let Some(proposal) = transport.try_receive_admitted_proposal(&snapshot).unwrap() {
            break proposal;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    };
    assert_eq!(admitted.transaction, TransactionId::from_raw(17));
    assert_eq!(admitted.message, expected);
    assert_eq!(admission, OutputProposalAdmission::Active);
    client.join().unwrap();
    transport.disconnect().unwrap();
}

fn snapshot() -> OutputAuthoritySnapshot {
    OutputAuthoritySnapshot {
        topology_epoch: 4,
        primary_output: OutputId::from_raw(1),
        heads: vec![OutputHeadDescriptor {
            head: DisplayHeadId::from_raw(1),
            generation: 2,
            label: "DP-1".into(),
            connected: true,
            enabled: true,
            current_mode: Some(DisplayModeId::from_raw(10)),
            transforms: OutputTransformSet::NORMAL,
            vrr_capable: false,
            modes: vec![OutputModeDescriptor {
                mode: DisplayModeId::from_raw(10),
                pixel_size: Size {
                    width: 1920,
                    height: 1080,
                },
                refresh_millihz: 60_000,
                preferred: true,
            }],
        }],
        groups: vec![OutputLogicalGroupState {
            output: OutputId::from_raw(1),
            generation: 3,
            logical: Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            members: vec![OutputGroupMember {
                head: DisplayHeadId::from_raw(1),
                mapping: OutputHeadMapping::Exact,
            }],
        }],
    }
}

fn proposal(connection_epoch: u64) -> OutputV1Proposal {
    OutputV1Proposal {
        connection_epoch,
        candidate: OutputTopologyCandidate {
            base_topology_epoch: 4,
            intent: OutputTopologyIntent::Apply,
            primary_group_index: 0,
            heads: vec![OutputHeadTargetProposal {
                head: DisplayHeadId::from_raw(1),
                head_generation: 2,
                mode: DisplayModeId::from_raw(10),
                transform: OutputTransform::Normal,
                vrr: OutputVrrPolicy::Disabled,
            }],
            groups: vec![OutputLogicalGroupProposal {
                output: OutputId::from_raw(1),
                logical: Rect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                members: vec![OutputGroupMember {
                    head: DisplayHeadId::from_raw(1),
                    mapping: OutputHeadMapping::Exact,
                }],
            }],
        },
    }
}

fn read_frame(stream: &mut UnixStream) -> Vec<u8> {
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    stream.read_exact(&mut header).unwrap();
    let payload_len = u32::from_le_bytes(header[16..20].try_into().unwrap()) as usize;
    let mut frame = header.to_vec();
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    stream
        .read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])
        .unwrap();
    frame
}
