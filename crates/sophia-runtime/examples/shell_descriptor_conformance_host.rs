use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sophia_engine::{
    CHROME_PRIMARY_BUTTON, ChromeDescriptorTable, DescriptorOverlayCandidate,
    DescriptorOverlayEntry, PresentedChromeCaptureState, PresentedChromePointerDisposition,
    descriptor_overlay_projection, resolve_presented_chrome_pointer_event,
};
use sophia_protocol::{
    AttentionState, ChromeDescriptor, DeviceId, DisplayLabel, IconTokenId, InputEventKind,
    OutputId, Point, Rect, SeatId, ShellV1Activation, ShellV1ActivationDisposition,
    ShellV1Candidate, ShellV1CandidateOutcome, ShellV1CandidateOutcomeKind, ShellV1Descriptor,
    ShellV1DescriptorSnapshot, SurfaceId, ToplevelActionCapabilityRef, TransactionId, TrustLevel,
};
use sophia_runtime::{
    ProcessLaunchSpec, ProcessSupervisor, ProtectionDomainRole, ProtectionDomainSpec,
    ProtectionPath, ShellSessionTransport, SupervisedProcessKind, SupervisorCommand,
};

const OUTPUT: OutputId = OutputId::from_raw(1);
const FIRST: SurfaceId = SurfaceId::new(10, 1);
const SECOND: SurfaceId = SurfaceId::new(11, 1);

fn main() {
    if let Err(error) = run() {
        eprintln!("shell-descriptor-conformance-host: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let client = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: shell_descriptor_conformance_host CLIENT [--proof|--serve]")?;
    let client_mode = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "--proof".to_owned());
    if !matches!(client_mode.as_str(), "--proof" | "--serve") {
        return Err("shell client mode must be --proof or --serve".into());
    }
    if !client.is_absolute() || !client.is_file() {
        return Err("shell client must be an absolute executable path".into());
    }
    let directory = std::env::temp_dir().join(format!(
        "sophia-shell-conformance-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let mut transport = ShellSessionTransport::bind_for_supervised_uid(
        &directory,
        rustix::process::geteuid().as_raw(),
    )?;
    let socket = transport.socket_path().to_path_buf();
    let domain = ProtectionDomainSpec::bubblewrap([ProtectionDomainRole::MetadataShell])?.path(
        ProtectionPath::read_only(socket.parent().ok_or("shell socket lacks a parent")?),
    )?;
    let spec = ProcessLaunchSpec::new(client)
        .arg(client_mode)
        .env(sophia_runtime::SOPHIA_SHELL_SOCKET_ENV, &socket)
        .process_group()
        .protection_domain(domain);
    let mut supervisor = ProcessSupervisor::new(SupervisedProcessKind::Shell, spec);
    supervisor.apply(SupervisorCommand::StartProcess {
        process: SupervisedProcessKind::Shell,
        delay: Duration::ZERO,
    })?;
    let evidence = supervisor
        .protection_evidence()
        .ok_or("shell process has no protection evidence")?
        .clone();
    transport.authorize_protected_peer(&evidence)?;
    transport.accept_and_negotiate(1, Duration::from_secs(5))?;

    let (table, snapshot, surfaces) = fixture();
    let candidate_transaction = TransactionId::from_raw(1);
    let candidate = transport.request_candidate(candidate_transaction, &snapshot)?;
    let projection_candidate = resolve_candidate(&candidate, &snapshot, &surfaces)?;
    let projection = descriptor_overlay_projection(
        &projection_candidate,
        &table,
        Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 720,
        },
    )?;
    transport.send_candidate_outcome(
        candidate_transaction,
        ShellV1CandidateOutcome {
            connection_epoch: 1,
            candidate_generation: candidate.candidate_generation,
            presentation_epoch: 0,
            kind: ShellV1CandidateOutcomeKind::Prepared,
        },
    )?;
    let presentation_epoch = 12;
    transport.send_candidate_outcome(
        candidate_transaction,
        ShellV1CandidateOutcome {
            connection_epoch: 1,
            candidate_generation: candidate.candidate_generation,
            presentation_epoch,
            kind: ShellV1CandidateOutcomeKind::Presented,
        },
    )?;

    let target = projection
        .targets
        .iter()
        .find(|target| Some(target.id.slot) == candidate.selected_slot)
        .ok_or("presented selection has no target")?;
    let point = Point {
        x: f64::from(target.geometry.x + 4),
        y: f64::from(target.geometry.y + 4),
    };
    let mut capture = PresentedChromeCaptureState::default();
    let press = resolve_presented_chrome_pointer_event(
        &mut capture,
        SeatId::from_raw(1),
        DeviceId::from_raw(1),
        InputEventKind::PointerButton {
            button: CHROME_PRIMARY_BUTTON,
            pressed: true,
        },
        Some(point),
        Some(OUTPUT),
        presentation_epoch,
        &projection.targets,
        Some(projection.geometry),
        false,
    )
    .map_err(|error| format!("Engine rejected shell pointer press: {error:?}"))?;
    if press != PresentedChromePointerDisposition::Captured {
        return Err("Engine did not capture the presented shell target".into());
    }
    let release = resolve_presented_chrome_pointer_event(
        &mut capture,
        SeatId::from_raw(1),
        DeviceId::from_raw(1),
        InputEventKind::PointerButton {
            button: CHROME_PRIMARY_BUTTON,
            pressed: false,
        },
        Some(point),
        Some(OUTPUT),
        presentation_epoch,
        &projection.targets,
        Some(projection.geometry),
        false,
    )
    .map_err(|error| format!("Engine rejected shell pointer release: {error:?}"))?;
    let PresentedChromePointerDisposition::Activated { action, activation } = release else {
        return Err("Engine did not activate the exact presented shell target".into());
    };
    transport.queue_activation(
        TransactionId::from_raw(2),
        ShellV1Activation {
            connection_epoch: 1,
            candidate_generation: candidate.candidate_generation,
            presentation_epoch,
            activation,
            action,
        },
    )?;
    let ack = transport.receive_activation_ack()?;
    if ack.disposition != ShellV1ActivationDisposition::Consumed {
        return Err("shell rejected an exact activation".into());
    }

    let mut withdrawal_snapshot = snapshot;
    withdrawal_snapshot.snapshot_generation += 1;
    let withdrawal_transaction = TransactionId::from_raw(3);
    let withdrawal = transport.request_candidate(withdrawal_transaction, &withdrawal_snapshot)?;
    if withdrawal.visible || !withdrawal.entries.is_empty() || withdrawal.selected_slot.is_some() {
        return Err("shell withdrawal retained visible targets".into());
    }
    transport.send_candidate_outcome(
        withdrawal_transaction,
        ShellV1CandidateOutcome {
            connection_epoch: 1,
            candidate_generation: withdrawal.candidate_generation,
            presentation_epoch: 0,
            kind: ShellV1CandidateOutcomeKind::Prepared,
        },
    )?;
    transport.send_candidate_outcome(
        withdrawal_transaction,
        ShellV1CandidateOutcome {
            connection_epoch: 1,
            candidate_generation: withdrawal.candidate_generation,
            presentation_epoch: presentation_epoch + 1,
            kind: ShellV1CandidateOutcomeKind::Presented,
        },
    )?;
    transport.disconnect()?;
    supervisor.terminate()?;
    println!(
        "sophia_shell_descriptor_corpus schema=1 status=complete protected=true descriptors=2 activations=1 withdrawn=true surface_ids_disclosed=0 coordinates_disclosed=0 icons_disclosed=0"
    );
    Ok(())
}

fn resolve_candidate(
    candidate: &ShellV1Candidate,
    snapshot: &ShellV1DescriptorSnapshot,
    surfaces: &BTreeMap<u16, SurfaceId>,
) -> Result<DescriptorOverlayCandidate, Box<dyn std::error::Error>> {
    if candidate.connection_epoch != snapshot.connection_epoch
        || candidate.snapshot_generation != snapshot.snapshot_generation
        || candidate.output != snapshot.output
        || !candidate.visible
        || candidate.entries.is_empty()
    {
        return Err("shell candidate does not match its complete snapshot".into());
    }
    let descriptors = snapshot
        .descriptors
        .iter()
        .map(|descriptor| (descriptor.slot, descriptor))
        .collect::<BTreeMap<_, _>>();
    let entries = candidate
        .entries
        .iter()
        .map(|entry| {
            let descriptor = descriptors
                .get(&entry.slot)
                .ok_or("shell candidate names an unknown descriptor")?;
            if descriptor.generation != entry.generation {
                return Err("shell candidate names a stale descriptor");
            }
            Ok(DescriptorOverlayEntry {
                slot: entry.slot,
                surface: *surfaces
                    .get(&entry.slot)
                    .ok_or("shell descriptor has no private surface mapping")?,
                descriptor_generation: entry.generation,
                action: descriptor.action,
            })
        })
        .collect::<Result<Vec<_>, &str>>()?;
    Ok(DescriptorOverlayCandidate {
        projection: candidate.candidate_generation,
        generation: candidate.candidate_generation,
        output: candidate.output,
        broker_epoch: snapshot.broker_epoch,
        broker_revocation_epoch: snapshot.broker_revocation_epoch,
        shell_session_epoch: candidate.connection_epoch,
        selected_slot: candidate.selected_slot,
        entries,
    })
}

fn fixture() -> (
    ChromeDescriptorTable,
    ShellV1DescriptorSnapshot,
    BTreeMap<u16, SurfaceId>,
) {
    let descriptor = |surface, generation, label: &str, trust_level, attention| ChromeDescriptor {
        surface,
        label: Some(DisplayLabel {
            text: label.into(),
            redacted: false,
        }),
        icon: Some(IconTokenId::from_raw(generation)),
        trust_level,
        attention,
        generation,
    };
    let mut table = ChromeDescriptorTable::default();
    table.upsert(descriptor(
        FIRST,
        8,
        "Terminal",
        TrustLevel::Trusted,
        AttentionState::None,
    ));
    table.upsert(descriptor(
        SECOND,
        9,
        "Browser",
        TrustLevel::Isolated,
        AttentionState::Notice,
    ));
    let action = |slot, generation, token| ToplevelActionCapabilityRef {
        token,
        issuer_epoch: 3,
        issuer_revocation_epoch: 4,
        recipient_epoch: 1,
        target_slot: slot,
        target_generation: generation,
    };
    let snapshot = ShellV1DescriptorSnapshot {
        connection_epoch: 1,
        snapshot_generation: 10,
        output: OUTPUT,
        output_generation: 1,
        broker_epoch: 3,
        broker_revocation_epoch: 4,
        descriptors: vec![
            ShellV1Descriptor {
                slot: 1,
                generation: 8,
                label: Some(DisplayLabel {
                    text: "Terminal".into(),
                    redacted: false,
                }),
                trust_level: TrustLevel::Trusted,
                attention: AttentionState::None,
                action: action(1, 8, 101),
            },
            ShellV1Descriptor {
                slot: 2,
                generation: 9,
                label: Some(DisplayLabel {
                    text: "Browser".into(),
                    redacted: false,
                }),
                trust_level: TrustLevel::Isolated,
                attention: AttentionState::Notice,
                action: action(2, 9, 102),
            },
        ],
    };
    (
        table,
        snapshot,
        [(1, FIRST), (2, SECOND)].into_iter().collect(),
    )
}
