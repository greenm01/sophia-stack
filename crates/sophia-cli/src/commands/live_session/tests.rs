#![cfg(test)]
use super::{
    BufferSource, CommittedSurfaceState, LayerSnapshot, LiveClientStdoutCapture,
    LiveProductionCpuScene, LiveProductionVisualRuntime, LiveXAuthorityFile,
    PRIMARY_INPUT_PROOF_SCRIPT, PersistentXtermSessionConfig, PhysicalInputRoutingMode,
    PhysicalTextProof, Rect, Region, ResizeSyncCapability, SECONDARY_POINTER_WITNESS_SCRIPT,
    SessionPointerPlacement, SessionProcessGuard, Size, Transform, authority_transaction_count,
    center_geometry_without_scaling, global_runtime_deadline_ends_session,
    input_baseline_is_presented, pending_wm_focus_after_engine_decision,
    physical_input_pixels_already_changed, physical_input_routing_mode,
    place_pointer_event_for_routing, pointer_offset_for_geometry, record_runtime_commits,
    route_input_events, session_protocol_errors_are_fatal,
    stable_gpu_frame_proves_post_input_pixels, successful_primary_exit_ends_session,
    take_settled_input_delivery_wait,
};
use sophia_engine::{InputFocusState, WmShortcutRegistry, WmShortcutRouter};
use sophia_protocol::{
    AuthorityKind, DeviceId, InputEventKind, InputEventPacket, NamespaceCapabilities,
    NamespaceProfile, Point, SeatId, SurfaceId, SurfaceTransaction, SurfaceTransactionReadiness,
    WM_API_VERSION, WmActionId, WmBindingRegistration, WmCapabilities, WmHello, WmModifierMask,
    WmSessionAction,
};
use sophia_x_authority::X_AUTHORITY_CPU_BUFFER_FORMAT_XRGB8888;
use sophia_x_authority::{XAuthorityClientSurfaceRoutes, XCoreKeyboardMapper};
use std::io::Write;
use std::sync::mpsc::sync_channel;
use std::time::{Duration, Instant};

mod input_policy_tests;
mod presentation_tests;

#[test]
fn blank_normal_session_process_guard_has_no_primary_child() {
    let mut guard = SessionProcessGuard {
        child: None,
        secondary_children: Vec::new(),
        socket_path: None,
        grouped: true,
    };
    let (primary, secondary) = guard.children_mut();
    assert!(primary.is_none());
    assert!(secondary.is_empty());
    guard.terminate().unwrap();
}

#[test]
fn client_stdout_capture_reads_without_waiting_for_inherited_writer_close() {
    let (capture, mut writer) = LiveClientStdoutCapture::create(181).unwrap();
    writer.write_all(b"sophia\n").unwrap();
    writer.flush().unwrap();

    assert_eq!(capture.read_bounded().unwrap(), b"sophia\n");

    writer.write_all(b"still-open").unwrap();
}

#[test]
fn settled_input_delivery_wait_is_consumed_once() {
    let started = Instant::now();
    let mut wait = Some(started);

    assert_eq!(take_settled_input_delivery_wait(&mut wait, false), None);
    assert_eq!(wait, Some(started));
    assert_eq!(
        take_settled_input_delivery_wait(&mut wait, true),
        Some(started)
    );
    assert_eq!(wait, None);
}

#[test]
fn successful_primary_exit_keeps_requested_input_proof_alive() {
    assert!(successful_primary_exit_ends_session(false));
    assert!(!successful_primary_exit_ends_session(true));
}

#[test]
fn global_runtime_deadline_does_not_strand_an_active_input_proof() {
    assert!(global_runtime_deadline_ends_session(false));
    assert!(!global_runtime_deadline_ends_session(true));
}

#[test]
fn normal_sessions_fail_on_any_protocol_error() {
    assert!(session_protocol_errors_are_fatal(true, false, 1));
    assert!(session_protocol_errors_are_fatal(false, true, 1));
    assert!(!session_protocol_errors_are_fatal(false, false, 1));
    assert!(!session_protocol_errors_are_fatal(true, true, 0));
}

#[test]
fn physical_input_preserves_shortcuts_without_an_application_surface() {
    let proof = SurfaceId::new(1, 1);
    let survivor = SurfaceId::new(2, 1);
    assert_eq!(
        physical_input_routing_mode(false, Some(proof), Some(proof), false),
        PhysicalInputRoutingMode::Full
    );
    assert_eq!(
        physical_input_routing_mode(true, Some(proof), Some(proof), false),
        PhysicalInputRoutingMode::Suppressed
    );
    assert_eq!(
        physical_input_routing_mode(true, Some(survivor), Some(proof), false),
        PhysicalInputRoutingMode::Full
    );
    assert_eq!(
        physical_input_routing_mode(true, None, None, true),
        PhysicalInputRoutingMode::ShortcutsOnly
    );
    assert_eq!(
        physical_input_routing_mode(true, Some(proof), Some(proof), true),
        PhysicalInputRoutingMode::ShortcutsOnly
    );
}

#[test]
fn shortcut_only_input_activates_super_enter_without_routing_unfocused_keys() {
    let action = WmActionId::from_raw(7);
    let registry = WmShortcutRegistry::from_hello(&WmHello {
        api_version: WM_API_VERSION,
        capabilities: WmCapabilities::all_supported(),
        bindings: vec![WmBindingRegistration {
            action,
            keycode: 28,
            modifiers: WmModifierMask {
                bits: WmModifierMask::SUPER,
            },
        }],
    })
    .unwrap();
    let mut shortcuts = WmShortcutRouter::new(registry);
    let events = [125, 28]
        .into_iter()
        .enumerate()
        .map(|(index, keycode)| InputEventPacket {
            serial: u64::try_from(index + 1).unwrap(),
            seat: SeatId::from_raw(1),
            device: DeviceId::from_raw(1),
            time_msec: u64::try_from(index + 1).unwrap(),
            kind: InputEventKind::Key {
                keycode,
                pressed: true,
            },
            global_position: None,
            target_surface: None,
            local_position: None,
        })
        .collect();
    let (input_sender, input_receiver) = sync_channel(4);
    let mut modifiers = XCoreKeyboardMapper::new();
    let mut emergency = super::EmergencyChordState::awaiting_arm();
    let mut virtual_terminal = sophia_cli::session_keyboard::VirtualTerminalChordState::default();
    let mut pointer = SessionPointerPlacement::default();
    let mut next_delivery = 1;

    let report = route_input_events(
        events,
        &InputFocusState::new(),
        &[],
        &[],
        &XAuthorityClientSurfaceRoutes::default(),
        &input_sender,
        &mut modifiers,
        &mut emergency,
        &mut virtual_terminal,
        Some(&mut shortcuts),
        &mut pointer,
        false,
        false,
        false,
        PhysicalInputRoutingMode::ShortcutsOnly,
        &mut next_delivery,
        None,
    )
    .unwrap();

    assert_eq!(report.wm_actions, [action]);
    assert_eq!(report.keys_observed, 2);
    assert_eq!(report.keys_routed, 0);
    assert!(input_receiver.try_recv().is_err());
}

#[test]
fn pending_physical_proof_moves_cursor_without_routing_application_input() {
    let events = vec![
        InputEventPacket {
            serial: 1,
            seat: SeatId::from_raw(1),
            device: DeviceId::from_raw(2),
            time_msec: 1,
            kind: InputEventKind::PointerMotion,
            global_position: Some(Point { x: 12.0, y: -8.0 }),
            target_surface: None,
            local_position: None,
        },
        InputEventPacket {
            serial: 2,
            seat: SeatId::from_raw(1),
            device: DeviceId::from_raw(1),
            time_msec: 2,
            kind: InputEventKind::Key {
                keycode: 31,
                pressed: true,
            },
            global_position: None,
            target_surface: None,
            local_position: None,
        },
    ];
    let (input_sender, input_receiver) = sync_channel(2);
    let mut modifiers = XCoreKeyboardMapper::new();
    let mut emergency = super::EmergencyChordState::awaiting_arm();
    let mut virtual_terminal = sophia_cli::session_keyboard::VirtualTerminalChordState::default();
    let mut pointer = SessionPointerPlacement::default();
    pointer.center_on_primary_output(Size {
        width: 2560,
        height: 1440,
    });
    let mut proof = PhysicalTextProof::new("sophia").unwrap();
    let mut next_delivery = 1;

    let report = route_input_events(
        events,
        &InputFocusState::new(),
        &[],
        &[],
        &XAuthorityClientSurfaceRoutes::default(),
        &input_sender,
        &mut modifiers,
        &mut emergency,
        &mut virtual_terminal,
        None,
        &mut pointer,
        true,
        false,
        false,
        PhysicalInputRoutingMode::CursorOnly,
        &mut next_delivery,
        Some(&mut proof),
    )
    .unwrap();

    assert_eq!(report.pointer_events, 1);
    assert_eq!(report.pointer_routed, 0);
    assert_eq!(report.keys_observed, 1);
    assert_eq!(report.keys_routed, 0);
    assert_eq!(proof.matched_events(), 0);
    assert!(input_receiver.try_recv().is_err());
    assert_ne!(
        pointer.position,
        Some(Point {
            x: 1280.0,
            y: 720.0
        })
    );
}

#[test]
fn authority_transaction_accounting_excludes_surface_removals() {
    assert_eq!(authority_transaction_count(&[]), 0);
}

#[test]
fn runtime_commit_accounting_records_only_accepted_batches() {
    assert_eq!(record_runtime_commits(166, 1), 167);
    assert_eq!(record_runtime_commits(167, 0), 167);
}

#[test]
fn completed_physical_input_reconciles_pixels_that_arrived_before_return() {
    assert!(physical_input_pixels_already_changed(
        Some(10),
        Some(20),
        true
    ));
    assert!(!physical_input_pixels_already_changed(
        Some(10),
        Some(20),
        false
    ));
    assert!(!physical_input_pixels_already_changed(
        Some(10),
        Some(10),
        true
    ));
}

#[test]
fn stable_focused_gpu_content_arms_input_without_cpu_scene_pixels() {
    assert!(input_baseline_is_presented(true, false));
    assert!(input_baseline_is_presented(false, true));
    assert!(!input_baseline_is_presented(false, false));
}

#[test]
fn physical_pointer_starts_at_focused_surface_center() {
    let raw = Point { x: -4.0, y: 6.0 };
    let offset = pointer_offset_for_geometry(
        raw,
        Rect {
            x: 80,
            y: 60,
            width: 960,
            height: 640,
        },
    );
    assert_eq!(raw.x + offset.x, 560.0);
    assert_eq!(raw.y + offset.y, 380.0);
}

#[test]
fn physical_pointer_can_move_before_an_application_surface_exists() {
    let mut pointer = SessionPointerPlacement::default();
    assert_eq!(
        pointer.center_on_primary_output(Size {
            width: 2560,
            height: 1440,
        }),
        Point {
            x: 1280.0,
            y: 720.0,
        }
    );
    let events = vec![InputEventPacket {
        serial: 1,
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        time_msec: 1,
        kind: InputEventKind::PointerMotion,
        global_position: Some(Point { x: 12.0, y: -8.0 }),
        target_surface: None,
        local_position: None,
    }];
    let (input_sender, input_receiver) = sync_channel(1);
    let mut modifiers = XCoreKeyboardMapper::new();
    let mut emergency = super::EmergencyChordState::awaiting_arm();
    let mut virtual_terminal = sophia_cli::session_keyboard::VirtualTerminalChordState::default();
    let mut next_delivery = 1;
    let report = route_input_events(
        events,
        &InputFocusState::new(),
        &[],
        &[],
        &XAuthorityClientSurfaceRoutes::default(),
        &input_sender,
        &mut modifiers,
        &mut emergency,
        &mut virtual_terminal,
        None,
        &mut pointer,
        true,
        false,
        false,
        PhysicalInputRoutingMode::Full,
        &mut next_delivery,
        None,
    )
    .unwrap();

    assert_eq!(report.pointer_events, 1);
    assert_eq!(report.pointer_routed, 0);
    assert_eq!(
        pointer.position,
        Some(Point {
            x: 1292.0,
            y: 712.0,
        })
    );
    assert!(input_receiver.try_recv().is_err());
}

#[test]
fn vt_chord_releases_application_modifiers_before_suspension() {
    let seat = SeatId::from_raw(1);
    let surface = SurfaceId::new(1, 1);
    let committed = [CommittedSurfaceState {
        surface,
        committed_generation: 1,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        },
        buffer: BufferSource::CpuBuffer { handle: 1 },
        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        }),
    }];
    let mut focus = InputFocusState::new();
    assert_eq!(
        focus.focus_surface(seat, surface, &committed),
        sophia_engine::InputFocusDecision::Focused
    );
    let events = [29, 56, 60]
        .into_iter()
        .enumerate()
        .map(|(index, keycode)| InputEventPacket {
            serial: u64::try_from(index + 1).unwrap(),
            seat,
            device: DeviceId::from_raw(1),
            time_msec: u64::try_from(index + 1).unwrap(),
            kind: InputEventKind::Key {
                keycode,
                pressed: true,
            },
            global_position: None,
            target_surface: None,
            local_position: None,
        })
        .collect();
    let (input_sender, input_receiver) = sync_channel(8);
    let mut modifiers = XCoreKeyboardMapper::new();
    let mut emergency = super::EmergencyChordState::awaiting_arm();
    let mut virtual_terminal = sophia_cli::session_keyboard::VirtualTerminalChordState::default();
    let mut pointer = SessionPointerPlacement::default();
    let mut next_delivery = 1;

    let report = route_input_events(
        events,
        &focus,
        &committed,
        &[],
        &XAuthorityClientSurfaceRoutes::default(),
        &input_sender,
        &mut modifiers,
        &mut emergency,
        &mut virtual_terminal,
        None,
        &mut pointer,
        false,
        false,
        false,
        PhysicalInputRoutingMode::Full,
        &mut next_delivery,
        None,
    )
    .unwrap();
    let routed = input_receiver
        .try_iter()
        .map(|input| input.request.kind)
        .collect::<Vec<_>>();

    assert_eq!(report.virtual_terminal, Some(2));
    assert_eq!(report.virtual_terminal_modifier_releases, 2);
    assert_eq!(
        routed,
        [
            InputEventKind::Key {
                keycode: 29,
                pressed: true,
            },
            InputEventKind::Key {
                keycode: 56,
                pressed: true,
            },
            InputEventKind::Key {
                keycode: 29,
                pressed: false,
            },
            InputEventKind::Key {
                keycode: 56,
                pressed: false,
            },
        ]
    );
    assert_eq!(modifiers.modifier_mask(), 0);
}

#[test]
fn interactive_pointer_proof_routes_motion_after_placement() {
    let mut pointer = SessionPointerPlacement {
        raw_position: None,
        offset: Some(Point { x: 10.0, y: 20.0 }),
        position: None,
    };
    let mut motion = InputEventPacket {
        serial: 1,
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        time_msec: 1,
        kind: InputEventKind::PointerMotion,
        global_position: Some(Point { x: 30.0, y: 40.0 }),
        target_surface: None,
        local_position: None,
    };

    assert!(place_pointer_event_for_routing(
        &mut motion,
        None,
        &[],
        &mut pointer,
        false,
    ));
    assert_eq!(motion.global_position, Some(Point { x: 40.0, y: 60.0 }));
}

#[test]
fn secondary_terminal_is_a_pointer_witness_without_a_text_prompt() {
    assert!(SECONDARY_POINTER_WITNESS_SCRIPT.contains("?1000h"));
    assert!(SECONDARY_POINTER_WITNESS_SCRIPT.contains("stty raw -echo"));
    assert!(SECONDARY_POINTER_WITNESS_SCRIPT.contains("Pointer input received"));
    assert!(!SECONDARY_POINTER_WITNESS_SCRIPT.contains("read -r line"));
    assert!(!SECONDARY_POINTER_WITNESS_SCRIPT.contains('\0'));
}

#[test]
fn primary_input_proof_remains_visible_until_session_completion() {
    assert!(PRIMARY_INPUT_PROOF_SCRIPT.contains("sleep 300"));
    assert!(!PRIMARY_INPUT_PROOF_SCRIPT.contains("sleep 5"));
}

#[test]
fn live_x_session_profiles_are_explicit_and_fail_closed() {
    let classic = PersistentXtermSessionConfig::from_args(&[]).unwrap();
    assert_eq!(classic.namespace_profile, NamespaceProfile::ClassicShared);
    assert_eq!(classic.namespace_capabilities, NamespaceCapabilities::NONE);

    let confined =
        PersistentXtermSessionConfig::from_args(&["--namespace-profile=confined".to_owned()])
            .unwrap();
    assert_eq!(confined.namespace_profile, NamespaceProfile::Confined);
    assert_eq!(confined.namespace_capabilities, NamespaceCapabilities::NONE);

    assert!(
        PersistentXtermSessionConfig::from_args(&["--namespace-profile=unknown".to_owned()])
            .unwrap_err()
            .to_string()
            .contains("expected classic or confined")
    );
}

#[test]
fn normal_session_application_registry_is_bounded_and_explicit() {
    let config = PersistentXtermSessionConfig::from_args(&[
        "--session-mode=normal".to_owned(),
        "--session-app=terminal=/usr/bin/xterm".to_owned(),
        "--session-app-arg=terminal=-cm".to_owned(),
        "--session-start=terminal".to_owned(),
        "--session-action-app=terminal=terminal".to_owned(),
    ])
    .unwrap();
    assert!(config.normal_session);
    assert_eq!(config.applications.startup, ["terminal"]);
    assert_eq!(
        config
            .application_for_action(WmSessionAction::LaunchApplication {
                application: super::TERMINAL_APPLICATION_ID,
            })
            .unwrap()
            .arguments,
        ["-cm"]
    );

    let blank = PersistentXtermSessionConfig::from_args(&[
        "--session-mode=normal".to_owned(),
        "--session-app=terminal=/usr/bin/kitty".to_owned(),
        "--session-action-app=terminal=terminal".to_owned(),
    ])
    .unwrap();
    assert!(blank.applications.startup.is_empty());
    assert!(
        blank
            .application_for_action(WmSessionAction::LaunchApplication {
                application: super::TERMINAL_APPLICATION_ID,
            })
            .is_some()
    );

    for args in [
        vec![
            "--session-mode=normal".to_owned(),
            "--session-app=terminal=xterm".to_owned(),
            "--session-start=terminal".to_owned(),
        ],
        vec![
            "--session-mode=normal".to_owned(),
            "--session-app=terminal=/usr/bin/xterm".to_owned(),
            "--session-start=missing".to_owned(),
        ],
        vec![
            "--session-app=terminal=/usr/bin/xterm".to_owned(),
            "--session-start=terminal".to_owned(),
        ],
        vec![
            "--session-mode=normal".to_owned(),
            "--session-app=terminal=/usr/bin/xterm".to_owned(),
            "--session-app=terminal=/usr/bin/xterm".to_owned(),
            "--session-start=terminal".to_owned(),
        ],
    ] {
        assert!(PersistentXtermSessionConfig::from_args(&args).is_err());
    }
}

#[test]
fn normal_session_rejects_proof_only_options() {
    let result = PersistentXtermSessionConfig::from_args(&[
        "--session-mode=normal".to_owned(),
        "--session-app=terminal=/usr/bin/xterm".to_owned(),
        "--session-start=terminal".to_owned(),
        "--proof".to_owned(),
    ]);
    assert!(result.is_err());
}

#[test]
fn kitty_only_session_can_exit_with_its_single_startup_app() {
    let config = PersistentXtermSessionConfig::from_args(&[
        "--session-mode=normal".to_owned(),
        "--session-app=terminal=/usr/bin/kitty".to_owned(),
        "--session-start=terminal".to_owned(),
        "--exit-when-startup-exits".to_owned(),
    ])
    .unwrap();
    assert!(config.exit_when_startup_exits);

    for args in [
        vec!["--exit-when-startup-exits".to_owned()],
        vec![
            "--session-mode=normal".to_owned(),
            "--session-app=terminal=/usr/bin/kitty".to_owned(),
            "--session-action-app=terminal=terminal".to_owned(),
            "--exit-when-startup-exits".to_owned(),
        ],
    ] {
        assert!(PersistentXtermSessionConfig::from_args(&args).is_err());
    }
}

#[test]
fn startup_readiness_timeout_is_bounded_and_requires_a_startup_app() {
    let config = PersistentXtermSessionConfig::from_args(&[
        "--session-mode=normal".to_owned(),
        "--session-app=terminal=/usr/bin/kitty".to_owned(),
        "--session-start=terminal".to_owned(),
        "--startup-ready-timeout-ms=8000".to_owned(),
    ])
    .unwrap();
    assert_eq!(
        config.startup_ready_timeout,
        Some(Duration::from_millis(8_000))
    );

    for args in [
        vec!["--startup-ready-timeout-ms=8000".to_owned()],
        vec![
            "--session-mode=normal".to_owned(),
            "--session-app=terminal=/usr/bin/kitty".to_owned(),
            "--session-action-app=terminal=terminal".to_owned(),
            "--startup-ready-timeout-ms=8000".to_owned(),
        ],
        vec![
            "--session-mode=normal".to_owned(),
            "--session-app=terminal=/usr/bin/kitty".to_owned(),
            "--session-start=terminal".to_owned(),
            "--startup-ready-timeout-ms=99".to_owned(),
        ],
    ] {
        assert!(PersistentXtermSessionConfig::from_args(&args).is_err());
    }
}

#[test]
fn production_input_seat_and_explicit_paths_are_distinct_modes() {
    let seat = PersistentXtermSessionConfig::from_args(&[
        "--input-seat=seat0".to_owned(),
        "--max-ticks=1".to_owned(),
    ])
    .unwrap();
    assert_eq!(seat.input_seat.as_deref(), Some("seat0"));
    assert!(seat.input_devices.is_empty());

    assert!(
        PersistentXtermSessionConfig::from_args(&[
            "--input-seat=seat0".to_owned(),
            "--input-devices=/dev/input/event0".to_owned(),
        ])
        .is_err()
    );
    assert!(
        PersistentXtermSessionConfig::from_args(&["--input-seat=../../seat0".to_owned()]).is_err()
    );
}

#[test]
fn live_x_output_injection_is_bounded_and_explicit() {
    let config = PersistentXtermSessionConfig::from_args(&[
        "--inject-output-size=1600x900".to_owned(),
        "--inject-surface-resize=960x640".to_owned(),
    ])
    .unwrap();
    assert_eq!(
        config.inject_output_size,
        Some(Size {
            width: 1600,
            height: 900
        })
    );
    assert_eq!(
        config.inject_surface_resize,
        Some(Size {
            width: 960,
            height: 640
        })
    );
    assert!(
        PersistentXtermSessionConfig::from_args(&["--inject-output-size=0x900".to_owned(),])
            .is_err()
    );
    assert!(
        PersistentXtermSessionConfig::from_args(&["--inject-output-size=wide".to_owned(),])
            .is_err()
    );
}

#[test]
fn live_x_application_client_contract_is_bounded_and_exclusive() {
    let config = PersistentXtermSessionConfig::from_args(&[
        "--client=zenity".to_owned(),
        "--client-arg=--entry".to_owned(),
        "--expect-client-stdout=sophia\n".to_owned(),
        "--require-client-normal-exit".to_owned(),
        "--expect-physical-text=sophia".to_owned(),
        "--expect-physical-pointer".to_owned(),
        "--input-devices=/dev/input/event0,/dev/input/event1".to_owned(),
        "--max-runtime-ms=30000".to_owned(),
    ])
    .unwrap();
    assert_eq!(config.client.as_deref(), Some("zenity"));
    assert_eq!(config.client_args, ["--entry"]);
    assert_eq!(config.expect_client_stdout.as_deref(), Some("sophia\n"));
    assert!(config.require_client_normal_exit);

    assert!(
        PersistentXtermSessionConfig::from_args(&[
            "--client=zenity".to_owned(),
            "--terminal=xterm".to_owned(),
        ])
        .is_err()
    );
    assert!(
        PersistentXtermSessionConfig::from_args(&["--client-arg=--entry".to_owned(),]).is_err()
    );
}

#[test]
fn live_xauthority_file_is_owner_only_valid_and_removed_on_drop() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn field<'a>(record: &'a [u8], offset: &mut usize) -> &'a [u8] {
        let len = usize::from(u16::from_be_bytes([record[*offset], record[*offset + 1]]));
        *offset += 2;
        let value = &record[*offset..*offset + len];
        *offset += len;
        value
    }

    let directory = std::env::temp_dir().join(format!(
        "sophia-live-xauthority-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let (authority, cookie) = LiveXAuthorityFile::create_in(&directory, 77).unwrap();
    let path = authority.path().to_owned();
    let metadata = std::fs::metadata(&path).unwrap();
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);

    let record = std::fs::read(&path).unwrap();
    assert_eq!(u16::from_be_bytes([record[0], record[1]]), 256);
    let mut offset = 2;
    assert_eq!(
        field(&record, &mut offset),
        rustix::system::uname().nodename().to_bytes()
    );
    assert_eq!(field(&record, &mut offset), b"77");
    assert_eq!(field(&record, &mut offset), b"MIT-MAGIC-COOKIE-1");
    assert_eq!(field(&record, &mut offset), cookie);
    assert_eq!(offset, record.len());

    drop(authority);
    assert!(!path.exists());
    std::fs::remove_dir(directory).unwrap();
}
