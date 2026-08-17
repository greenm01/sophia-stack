use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sophia_protocol::{
    DisplayHeadId, DisplayModeId, OutputAuthoritySnapshot, OutputGroupMember, OutputHeadDescriptor,
    OutputHeadMapping, OutputLogicalGroupState, OutputModeDescriptor, OutputTransformSet,
    OutputV1Outcome, OutputV1OutcomeKind, OutputV1ServerWelcome, OutputV1Snapshot, OutputVrrPolicy,
    Rect, SOPHIA_IPC_HEADER_LEN, SOPHIA_OUTPUT_CAPABILITY_CONFIGURE,
    SOPHIA_OUTPUT_CAPABILITY_OBSERVE, SOPHIA_OUTPUT_INTERFACE_REVISION, Size, TransactionId,
    decode_output_v1_client_hello_frame, decode_output_v1_proposal_frame,
    encode_output_v1_outcome_frame, encode_output_v1_server_welcome_frame,
    encode_output_v1_snapshot_frame,
};
use sophia_wm_demo::{OutputV1Client, mixed_mirror_extended_candidate};

static NEXT_SOCKET: AtomicU64 = AtomicU64::new(1);

#[test]
fn mixed_candidate_keeps_modes_and_forms_one_mirror_plus_one_extended_group() {
    let snapshot = three_head_snapshot();
    let candidate =
        mixed_mirror_extended_candidate(&snapshot, "Display 1", "Display 2", "Display 3").unwrap();

    assert_eq!(candidate.heads.len(), 3);
    assert_eq!(candidate.groups.len(), 2);
    assert_eq!(candidate.groups[0].output.raw(), 1);
    assert_eq!(candidate.groups[0].members.len(), 2);
    assert_eq!(
        candidate.groups[0].members[0].mapping,
        OutputHeadMapping::Exact
    );
    assert_eq!(
        candidate.groups[0].members[1].mapping,
        OutputHeadMapping::Fit
    );
    assert_eq!(candidate.groups[1].output.raw(), 3);
    assert_eq!(
        candidate.groups[1].members[0].mapping,
        OutputHeadMapping::Exact
    );
    assert_eq!(candidate.groups[1].logical.x, 2560);
    assert_eq!(candidate.groups[1].logical.width, 1920);
    assert_eq!(candidate.heads[0].mode.raw(), 11);
    assert_eq!(candidate.heads[1].mode.raw(), 12);
    assert_eq!(candidate.heads[2].mode.raw(), 13);
    assert!(
        candidate
            .heads
            .iter()
            .all(|head| head.vrr == OutputVrrPolicy::Disabled)
    );
    candidate.validate_against(&snapshot).unwrap();
}

#[test]
fn mixed_candidate_refuses_to_disable_an_unmentioned_connected_head() {
    let mut snapshot = three_head_snapshot();
    snapshot.heads.push(head(4, 14, "Display 4", 1280, 1024));
    snapshot.groups.push(group(4, 4, 4480, 1280, 1024));

    let error = mixed_mirror_extended_candidate(&snapshot, "Display 1", "Display 2", "Display 3")
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("proof requires exactly three connected heads")
    );
}

#[test]
fn output_client_negotiates_snapshot_and_committed_candidate() {
    let socket = std::env::temp_dir().join(format!(
        "sophia-wm-demo-output-v1-{}-{}",
        std::process::id(),
        NEXT_SOCKET.fetch_add(1, Ordering::Relaxed)
    ));
    let listener = UnixListener::bind(&socket).unwrap();
    let expected_snapshot = three_head_snapshot();
    let served_snapshot = expected_snapshot.clone();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let hello = decode_output_v1_client_hello_frame(&read_frame(&mut stream)).unwrap();
        assert_eq!(hello.minimum_revision, SOPHIA_OUTPUT_INTERFACE_REVISION);
        assert_eq!(
            hello.capabilities,
            SOPHIA_OUTPUT_CAPABILITY_OBSERVE | SOPHIA_OUTPUT_CAPABILITY_CONFIGURE
        );
        stream
            .write_all(
                &encode_output_v1_server_welcome_frame(OutputV1ServerWelcome {
                    selected_revision: SOPHIA_OUTPUT_INTERFACE_REVISION,
                    capabilities: SOPHIA_OUTPUT_CAPABILITY_OBSERVE
                        | SOPHIA_OUTPUT_CAPABILITY_CONFIGURE,
                    connection_epoch: 9,
                    max_heads: 16,
                    max_groups: 16,
                    max_modes_per_head: 128,
                    max_heads_per_group: 4,
                })
                .unwrap(),
            )
            .unwrap();
        stream
            .write_all(
                &encode_output_v1_snapshot_frame(
                    TransactionId::from_raw(7),
                    &OutputV1Snapshot {
                        connection_epoch: 9,
                        snapshot: served_snapshot.clone(),
                    },
                )
                .unwrap(),
            )
            .unwrap();
        let (transaction, proposal) =
            decode_output_v1_proposal_frame(&read_frame(&mut stream)).unwrap();
        proposal
            .candidate
            .validate_against(&served_snapshot)
            .unwrap();
        assert_eq!(proposal.candidate.groups.len(), 2);
        stream
            .write_all(
                &encode_output_v1_outcome_frame(
                    transaction,
                    OutputV1Outcome {
                        connection_epoch: 9,
                        topology_epoch: 5,
                        kind: OutputV1OutcomeKind::Committed,
                        reason: 0,
                    },
                )
                .unwrap(),
            )
            .unwrap();
    });

    let mut client = OutputV1Client::connect(&socket, Duration::from_secs(1)).unwrap();
    let (snapshot_transaction, snapshot) = client.receive_snapshot().unwrap();
    assert_eq!(snapshot_transaction.raw(), 7);
    assert_eq!(snapshot, expected_snapshot);
    let candidate =
        mixed_mirror_extended_candidate(&snapshot, "Display 1", "Display 2", "Display 3").unwrap();
    let outcome = client.submit(candidate, &snapshot).unwrap();
    assert_eq!(outcome.kind, OutputV1OutcomeKind::Committed);
    assert_eq!(outcome.topology_epoch, 5);

    server.join().unwrap();
    std::fs::remove_file(socket).unwrap();
}

fn three_head_snapshot() -> OutputAuthoritySnapshot {
    OutputAuthoritySnapshot {
        topology_epoch: 4,
        primary_output: sophia_protocol::OutputId::from_raw(1),
        heads: vec![
            head(1, 11, "Display 1", 2560, 1440),
            head(2, 12, "Display 2", 1920, 1080),
            head(3, 13, "Display 3", 1920, 1200),
        ],
        groups: vec![
            group(1, 1, 0, 2560, 1440),
            group(2, 2, 2560, 1920, 1080),
            group(3, 3, 4480, 1920, 1200),
        ],
    }
}

fn head(raw: u64, mode_raw: u64, label: &str, width: i32, height: i32) -> OutputHeadDescriptor {
    OutputHeadDescriptor {
        head: DisplayHeadId::from_raw(raw),
        generation: 2,
        label: label.to_owned(),
        connected: true,
        enabled: true,
        current_mode: Some(DisplayModeId::from_raw(mode_raw)),
        transforms: OutputTransformSet::NORMAL,
        vrr_capable: false,
        modes: vec![OutputModeDescriptor {
            mode: DisplayModeId::from_raw(mode_raw),
            pixel_size: Size { width, height },
            refresh_millihz: 60_000,
            preferred: true,
        }],
    }
}

fn group(
    output_raw: u64,
    head_raw: u64,
    x: i32,
    width: i32,
    height: i32,
) -> OutputLogicalGroupState {
    OutputLogicalGroupState {
        output: sophia_protocol::OutputId::from_raw(output_raw),
        generation: 2,
        logical: Rect {
            x,
            y: 0,
            width,
            height,
        },
        members: vec![OutputGroupMember {
            head: DisplayHeadId::from_raw(head_raw),
            mapping: OutputHeadMapping::Exact,
        }],
    }
}

fn read_frame(stream: &mut impl Read) -> Vec<u8> {
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
