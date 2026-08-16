#![cfg(target_os = "linux")]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use sophia_protocol::*;
use sophia_runtime::*;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

#[test]
fn optional_output_service_exchanges_candidate_and_settlement() {
    let directory = temporary_directory("exchange");
    let peer = PolicyPeerIdentity {
        uid: rustix::process::geteuid().as_raw(),
        pid: std::process::id(),
    };
    let transport = OutputSessionTransport::bind(&directory, peer).unwrap();
    let socket_path = transport.socket_path().to_owned();
    let snapshot = snapshot();
    let service =
        OutputTransportService::spawn(transport, 1, TransactionId::from_raw(1), snapshot.clone())
            .unwrap();
    let expected = proposal(1);
    let sent = expected.clone();
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
        assert_eq!(welcome.connection_epoch, 1);
        let (_, observed) = decode_output_v1_snapshot_frame(&read_frame(&mut stream)).unwrap();
        assert_eq!(observed.snapshot, snapshot);
        stream
            .write_all(&encode_output_v1_proposal_frame(TransactionId::from_raw(2), &sent).unwrap())
            .unwrap();
        decode_output_v1_outcome_frame(&read_frame(&mut stream)).unwrap()
    });

    assert_eq!(
        service.event_timeout(Duration::from_secs(1)).unwrap(),
        OutputTransportServiceEvent::Connected {
            connection_epoch: 1
        }
    );
    let OutputTransportServiceEvent::Proposal {
        proposal: admitted,
        admission,
    } = service.event_timeout(Duration::from_secs(1)).unwrap()
    else {
        panic!("output service omitted admitted proposal");
    };
    assert_eq!(admitted.transaction, TransactionId::from_raw(2));
    assert_eq!(admitted.message, expected);
    assert_eq!(admission, OutputProposalAdmission::Active);
    service
        .command(OutputTransportServiceCommand::Settle {
            transaction: admitted.transaction,
            outcome: OutputV1Outcome {
                connection_epoch: 1,
                topology_epoch: 1,
                kind: OutputV1OutcomeKind::Validated,
                reason: 0,
            },
        })
        .unwrap();
    let (transaction, outcome) = client.join().unwrap();
    assert_eq!(transaction, TransactionId::from_raw(2));
    assert_eq!(outcome.kind, OutputV1OutcomeKind::Validated);
}

#[test]
fn connected_output_service_publishes_a_replacement_hardware_snapshot() {
    let directory = temporary_directory("publish-snapshot");
    let peer = PolicyPeerIdentity {
        uid: rustix::process::geteuid().as_raw(),
        pid: std::process::id(),
    };
    let transport = OutputSessionTransport::bind(&directory, peer).unwrap();
    let socket_path = transport.socket_path().to_owned();
    let initial = snapshot();
    let service =
        OutputTransportService::spawn(transport, 1, TransactionId::from_raw(1), initial.clone())
            .unwrap();
    let mut client = connect_output_client(&socket_path);
    client
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    decode_output_v1_server_welcome_frame(&read_frame(&mut client)).unwrap();
    let (_, observed) = decode_output_v1_snapshot_frame(&read_frame(&mut client)).unwrap();
    assert_eq!(observed.snapshot, initial);
    assert_eq!(
        service.event_timeout(Duration::from_secs(1)).unwrap(),
        OutputTransportServiceEvent::Connected {
            connection_epoch: 1
        }
    );

    let mut replacement = snapshot();
    replacement.topology_epoch = 2;
    replacement.groups[0].logical.width = 1_280;
    service
        .command(OutputTransportServiceCommand::PublishSnapshot {
            transaction: TransactionId::from_raw(2),
            snapshot: replacement.clone(),
        })
        .unwrap();
    let (transaction, observed) =
        decode_output_v1_snapshot_frame(&read_frame(&mut client)).unwrap();
    assert_eq!(transaction, TransactionId::from_raw(2));
    assert_eq!(observed.connection_epoch, 1);
    assert_eq!(observed.snapshot, replacement);
}

#[test]
fn optional_output_service_stops_without_a_client() {
    let directory = temporary_directory("idle");
    let peer = PolicyPeerIdentity {
        uid: rustix::process::geteuid().as_raw(),
        pid: std::process::id(),
    };
    let transport = OutputSessionTransport::bind(&directory, peer).unwrap();
    let started = Instant::now();
    let service =
        OutputTransportService::spawn(transport, 1, TransactionId::from_raw(1), snapshot())
            .unwrap();
    drop(service);
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn output_service_reauthorizes_a_restarted_supervised_process() {
    let directory = temporary_directory("restart");
    let uid = rustix::process::geteuid().as_raw();
    let pid = std::process::id();
    let mut transport = OutputSessionTransport::bind_for_supervised_uid(&directory, uid).unwrap();
    transport.authorize_supervised_pid(pid).unwrap();
    let socket_path = transport.socket_path().to_owned();
    let service =
        OutputTransportService::spawn(transport, 1, TransactionId::from_raw(1), snapshot())
            .unwrap();
    let mut first = connect_output_client(&socket_path);
    assert_eq!(
        decode_output_v1_server_welcome_frame(&read_frame(&mut first))
            .unwrap()
            .connection_epoch,
        1
    );
    decode_output_v1_snapshot_frame(&read_frame(&mut first)).unwrap();
    assert_eq!(
        service.event_timeout(Duration::from_secs(1)).unwrap(),
        OutputTransportServiceEvent::Connected {
            connection_epoch: 1
        }
    );

    service
        .command(OutputTransportServiceCommand::ReplaceSupervisedPid { pid })
        .unwrap();
    assert_eq!(
        service.event_timeout(Duration::from_secs(1)).unwrap(),
        OutputTransportServiceEvent::AssigneeReplaced {
            connection_epoch: 2,
            abandoned: Vec::new(),
        }
    );
    first
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let mut closed = [0u8; 1];
    assert_eq!(first.read(&mut closed).unwrap(), 0);

    let mut second = connect_output_client(&socket_path);
    assert_eq!(
        decode_output_v1_server_welcome_frame(&read_frame(&mut second))
            .unwrap()
            .connection_epoch,
        2
    );
    decode_output_v1_snapshot_frame(&read_frame(&mut second)).unwrap();
    assert_eq!(
        service.event_timeout(Duration::from_secs(1)).unwrap(),
        OutputTransportServiceEvent::Connected {
            connection_epoch: 2
        }
    );
}

fn temporary_directory(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "sophia-output-service-{label}-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ))
}

fn snapshot() -> OutputAuthoritySnapshot {
    OutputAuthoritySnapshot {
        topology_epoch: 1,
        primary_output: OutputId::from_raw(1),
        heads: vec![OutputHeadDescriptor {
            head: DisplayHeadId::from_raw(1),
            generation: 1,
            label: "Display 1".into(),
            connected: true,
            enabled: true,
            current_mode: Some(DisplayModeId::from_raw(10)),
            transforms: OutputTransformSet::NORMAL,
            vrr_capable: false,
            modes: vec![OutputModeDescriptor {
                mode: DisplayModeId::from_raw(10),
                pixel_size: Size {
                    width: 1_920,
                    height: 1_080,
                },
                refresh_millihz: 60_000,
                preferred: true,
            }],
        }],
        groups: vec![OutputLogicalGroupState {
            output: OutputId::from_raw(1),
            generation: 1,
            logical: Rect {
                x: 0,
                y: 0,
                width: 1_920,
                height: 1_080,
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
            base_topology_epoch: 1,
            intent: OutputTopologyIntent::ValidateOnly,
            primary_group_index: 0,
            heads: vec![OutputHeadTargetProposal {
                head: DisplayHeadId::from_raw(1),
                head_generation: 1,
                mode: DisplayModeId::from_raw(10),
                transform: OutputTransform::Normal,
                vrr: OutputVrrPolicy::Disabled,
            }],
            groups: vec![OutputLogicalGroupProposal {
                output: OutputId::from_raw(1),
                logical: Rect {
                    x: 0,
                    y: 0,
                    width: 1_920,
                    height: 1_080,
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

fn connect_output_client(path: &std::path::Path) -> UnixStream {
    let mut stream = UnixStream::connect(path).unwrap();
    stream
        .write_all(
            &encode_output_v1_client_hello_frame(OutputV1ClientHello {
                minimum_revision: 1,
                maximum_revision: 1,
                capabilities: SOPHIA_OUTPUT_CAPABILITY_OBSERVE | SOPHIA_OUTPUT_CAPABILITY_CONFIGURE,
            })
            .unwrap(),
        )
        .unwrap();
    stream
}
