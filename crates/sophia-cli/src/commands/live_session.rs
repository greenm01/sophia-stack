use super::prelude::*;

use sophia_backend_live::{
    ClassicHardwareCursorUpdate, LiveProductionAuthorityBatch, LiveProductionCpuScene,
    LiveProductionCursorPresentation, LiveProductionDmaBufRegistration,
    LiveProductionFenceRegistration, LiveProductionNativeScanout, LiveProductionPresentSubmission,
    LiveProductionVisualRuntime,
};
use sophia_cli::emergency_input::{EmergencyChordAction, EmergencyChordState};
use sophia_cli::input_proof::{PhysicalTextProof, PhysicalTextProofEvent};
use sophia_cli::resize_transaction::{
    ResizeRollbackCoordinator, project_authority_batch_onto_layout,
};
use sophia_engine::{
    FocusedInputRoute, InputFocusDecision, InputFocusState, NonBlockingInputPoller, WmPolicyError,
    WmShortcutRouter, WmWorkspaceState,
};
use sophia_protocol::{
    ClientAdmissionContext, DeviceId, NamespaceCapabilities, NamespaceId, NamespaceProfile, Point,
    SeatId, SessionApplicationId, WM_DEFAULT_WORKSPACES, WmActionActivation, WmActionId,
    WmManageSurface, WmSessionAction,
};
use sophia_runtime::NamespaceRegistry;
use sophia_x_authority::{
    XAuthorityClientControlAck, XAuthorityClientControlCommand, XAuthorityClientInputDelivery,
    XAuthorityClientSurfaceRoutes, XAuthorityControlCommand, XAuthorityControlOutcome,
    XAuthorityInputDeliveryId, XAuthorityInputDeliveryOutcome, XAuthorityRoutedInput,
    XCoreKeyboardMapper, XPresentCompletionMode, XServerFrontendAdmissionError,
    XServerFrontendAdmissionPolicy, XServerFrontendAdmissionRequest, XServerFrontendConfig,
    XServerFrontendProtocolRouter, XServerFrontendRenderDeviceError,
    XServerFrontendRenderDeviceProvider, XServerFrontendRouteBroker, XServerFrontendServiceCommand,
    XServerFrontendSetupAuthorization, run_x_server_frontend_routed_until_stopped,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::{Read, Write};
use std::num::NonZeroUsize;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod authority_file;
pub(super) mod input_guard;
mod process_supervision;
mod proof_artifacts;
mod x_frontend;

use authority_file::{LiveXAuthorityFile, fill_session_random};
use process_supervision::{ManagedSessionChild, SessionProcessGuard, terminate_session_child};
use proof_artifacts::{LiveClientStdoutCapture, LiveInputProofResult};
use x_frontend::{LiveXAdmissionPolicy, LiveXRenderDeviceProvider};

include!("live_session/config.rs");
include!("live_session/input.rs");
include!("live_session/policy.rs");
include!("live_session/presentation.rs");
include!("live_session/startup.rs");
include!("live_session/wm.rs");

const SESSION_AUTHORITY_CAPACITY: usize = 256;
const SESSION_KEY_CAPACITY: usize = 64;
const SESSION_CONTROL_CAPACITY: usize = 32;
const SESSION_INPUT_QUIET_MSEC: u64 = 500;
const SESSION_PHYSICAL_SEQUENCE_TIMEOUT_MSEC: u64 = 15_000;
const SESSION_PHYSICAL_PIXEL_TIMEOUT_MSEC: u64 = 5_000;
const SESSION_COMPLETION_TIMEOUT_MSEC: u64 = 5_000;
const SESSION_INPUT_DELIVERY_TIMEOUT_MSEC: u64 = 1_000;
const SESSION_SEAT_RAW: u64 = 1;
const SESSION_KEYBOARD_DEVICE_RAW: u64 = 1;
const SESSION_POINTER_DEVICE_RAW: u64 = 2;
const PRIMARY_INPUT_PROOF_SCRIPT: &str = r#"printf 'type %s then Return: ' "$1"; IFS= read -r line; umask 077; printf '%s' "$line" > "$2"; printf '\nreceived:%s\n' "$line"; sleep 300"#;
const SECONDARY_POINTER_WITNESS_SCRIPT: &str = r#"saved=$(stty -g); stty raw -echo; printf '\033[?1000h\033[?1006hPointer witness: click here\r\n'; dd bs=1 count=1 >/dev/null 2>&1; printf '\033[?1000l\033[?1006l'; stty "$saved"; printf 'Pointer input received\n'; sleep 300"#;
static NEXT_SESSION_GENERATION: AtomicU64 = AtomicU64::new(1);

enum SessionPhysicalInput {
    Threaded(sophia_backend_live::ThreadedNativeLibinputEventPoller),
}

impl NonBlockingInputPoller for SessionPhysicalInput {
    fn poll_ready(&mut self) -> std::io::Result<Vec<sophia_protocol::InputEventPacket>> {
        match self {
            Self::Threaded(poller) => poller.poll_ready(),
        }
    }
}

impl SessionPhysicalInput {
    fn stats(&self) -> sophia_backend_live::ThreadedNativeInputStats {
        match self {
            Self::Threaded(poller) => poller.stats(),
        }
    }

    fn policy_report(&self) -> sophia_backend_live::NativeLibinputPolicyReport {
        match self {
            Self::Threaded(poller) => poller.policy_report(),
        }
    }
}

pub(crate) fn run_persistent_xterm_session(
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let config = PersistentXtermSessionConfig::from_args(args)?;
    let terminal = if config.client.is_none() {
        Some(super::x_authority::resolve_external_probe_binary(
            "xterm",
            &config.terminal,
        )?)
    } else {
        None
    };
    prepare_display_socket(&config.socket_path)?;
    let display_number = parse_display_number(&config.display)?;
    let (mut xauthority, xauthority_cookie) = LiveXAuthorityFile::create(display_number)?;
    let mut native_scanout = config
        .native_scanout
        .then(LiveProductionNativeScanout::new)
        .transpose()?;
    let device_map =
        sophia_backend_live::NativeLibinputDeviceMap::new(SeatId::from_raw(SESSION_SEAT_RAW))
            .with_keyboard_device(DeviceId::from_raw(SESSION_KEYBOARD_DEVICE_RAW))
            .with_pointer_device(DeviceId::from_raw(SESSION_POINTER_DEVICE_RAW));
    let mut physical_input = if !config.input_devices.is_empty() {
        Some(SessionPhysicalInput::Threaded(
            sophia_backend_live::open_threaded_native_libinput_path_poller(
                &config.input_devices,
                device_map,
                64,
                256,
            )?,
        ))
    } else if let Some(seat_name) = config.input_seat.as_deref() {
        Some(SessionPhysicalInput::Threaded(
            sophia_backend_live::open_threaded_native_libinput_udev_poller(
                seat_name, device_map, 64, 256,
            )?,
        ))
    } else {
        None
    };
    if physical_input.is_some() {
        let policy = physical_input
            .as_ref()
            .expect("configured input devices create a poller")
            .policy_report();
        println!(
            "sophia_live_session_input_pipeline schema=3 status=poller_ready source={} seat={} devices={} active={} keyboards={} pointers={} touch={} tap_capable={} tap_enabled={}",
            if policy.udev_managed { "udev" } else { "paths" },
            config.input_seat.as_deref().unwrap_or("explicit"),
            policy.devices_added,
            policy.active_devices,
            policy.keyboards,
            policy.pointers,
            policy.touch_devices,
            policy.tap_capable,
            policy.tap_enabled
        );
        std::io::stdout().flush()?;
    }
    let initial_outputs = native_scanout
        .as_ref()
        .map(LiveProductionNativeScanout::outputs)
        .unwrap_or_else(|| vec![sophia_engine::HeadlessOutput::deterministic()]);
    let mut wm_session = LiveWmSession::from_config(&config, &initial_outputs)?;
    let output_topology = output_topology_from_engine_outputs(&initial_outputs)?;

    let server_path = config.socket_path.clone();
    let session_generation = NEXT_SESSION_GENERATION
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |generation| {
            generation.checked_add(1)
        })
        .map_err(|_| "Sophia session generation exhausted")?;
    let namespace_registry = Arc::new(Mutex::new(NamespaceRegistry::new(session_generation)?));
    let x_namespace = namespace_registry
        .lock()
        .map_err(|_| "Sophia namespace registry lock was poisoned")?
        .create_namespace(config.namespace_profile, config.namespace_capabilities);
    let session_user_id = rustix::process::geteuid().as_raw();
    let admission_policy = Arc::new(LiveXAdmissionPolicy {
        registry: namespace_registry.clone(),
        namespace: x_namespace.id,
        session_user_id,
    });
    let mut frontend_config =
        XServerFrontendConfig::new_with_namespace_context(&server_path, x_namespace)?
            .with_output_topology(output_topology.clone())?
            .with_xkb_config(config.xkb_config.clone())?
            .with_setup_authorization(XServerFrontendSetupAuthorization::MitMagicCookie(
                xauthority_cookie,
            ))
            .with_admission_policy(admission_policy);
    if !config.software_client_rendering
        && let Some(native_scanout) = native_scanout.as_ref()
    {
        frontend_config =
            frontend_config.with_render_device_provider(Arc::new(LiveXRenderDeviceProvider {
                device: native_scanout.clone_render_device_file()?,
            }));
    }
    let (authority_sender, authority_receiver) = sync_channel(SESSION_AUTHORITY_CAPACITY);
    let (control_ack_sender, control_ack_receiver) = sync_channel(SESSION_CONTROL_CAPACITY);
    let (input_delivery_sender, input_delivery_receiver) = sync_channel(SESSION_KEY_CAPACITY);
    let broker =
        XServerFrontendRouteBroker::with_control_and_input_delivery_senders_and_xkb_config(
            NonZeroUsize::new(SESSION_KEY_CAPACITY).expect("session route capacity is nonzero"),
            control_ack_sender,
            input_delivery_sender,
            config.xkb_config.clone(),
        )?;
    let input_sender = broker.routed_input_sender();
    let control_sender = broker.control_sender();
    let protocol_router = broker.protocol_router();
    let (service_command_sender, service_command_receiver) = sync_channel(1);
    let mut server = Some(std::thread::spawn(move || {
        run_x_server_frontend_routed_until_stopped(
            frontend_config,
            authority_sender,
            broker,
            service_command_receiver,
        )
    }));
    wait_for_x_server_socket(&config.socket_path, &mut server)?;

    let input_proof_result = (config.input_proof_requested() && config.client.is_none())
        .then(|| LiveInputProofResult::create(display_number))
        .transpose()?;
    let normal_primary = config
        .normal_session
        .then(|| {
            config.applications.startup.first().map(|id| {
                config
                    .applications
                    .applications
                    .get(id)
                    .expect("normal session startup application was validated")
            })
        })
        .flatten();
    let mut terminal_command = match (normal_primary, config.client.as_deref()) {
        (Some(app), _) => Some(std::process::Command::new(&app.executable)),
        (None, _) if config.normal_session => None,
        (None, Some(client)) => Some(application_client_command(client)),
        (None, None) => Some(std::process::Command::new(
            terminal.as_deref().expect("xterm executable is resolved"),
        )),
    };
    let (client_stdout_capture, client_stdout_file) = if config.client.is_some() {
        let (capture, file) = LiveClientStdoutCapture::create(display_number)?;
        (Some(capture), Some(file))
    } else {
        (None, None)
    };
    if let Some(terminal_command) = terminal_command.as_mut() {
        terminal_command
            .env("DISPLAY", &config.display)
            .env("XAUTHORITY", xauthority.path())
            .env_remove("ENV")
            .env_remove("BASH_ENV")
            .stdin(Stdio::null())
            .stderr(Stdio::inherit());
        if let Some(app) = normal_primary {
            terminal_command
                .args(&app.arguments)
                .process_group(0)
                .stdout(Stdio::inherit());
        } else if config.client.is_some() {
            terminal_command
                .env("GDK_BACKEND", "x11")
                .env("GTK_USE_PORTAL", "0")
                .env_remove("WAYLAND_DISPLAY")
                .args(&config.client_args)
                .stdout(Stdio::from(
                    client_stdout_file.expect("application stdout file was created"),
                ));
        } else {
            terminal_command
                .args([
                    "-cm",
                    "-dc",
                    "-geometry",
                    "120x36+80+60",
                    "-title",
                    "Sophia Terminal",
                ])
                .stdout(Stdio::inherit());
        }
        if config.client.is_none()
            && let Some(proof_text) = config
                .inject_text
                .as_deref()
                .or(config.expect_physical_text.as_deref())
        {
            terminal_command
                .args([
                    "-e",
                    "sh",
                    "-c",
                    PRIMARY_INPUT_PROOF_SCRIPT,
                    "sophia-input-proof",
                ])
                .arg(proof_text)
                .arg(
                    input_proof_result
                        .as_ref()
                        .expect("input proof result exists with proof text")
                        .path(),
                );
        } else if let Some(program) = config.terminal_exec.as_deref() {
            terminal_command
                .env_remove("ENV")
                .env_remove("BASH_ENV")
                .arg("-e")
                .arg(program)
                .args(&config.terminal_exec_args);
        }
    }
    let child = terminal_command
        .map(|mut command| command.spawn())
        .transpose()?;
    if child.is_some()
        && let Some(app) = normal_primary
    {
        println!(
            "sophia_session_app schema=1 status=started id={} source=startup",
            app.id
        );
    }
    let mut process = SessionProcessGuard::new(
        child,
        Vec::new(),
        config.socket_path.clone(),
        config.normal_session,
    );
    // Admit one primary-client transaction before launching the secondary
    // proof client. Otherwise optimized startup lets both xterms race for the
    // first committed surface, making initial focus nondeterministic.
    let initial_authority_batch =
        if config.secondary_terminal || config.applications.startup.len() > 1 {
            Some(
                authority_receiver
                    .recv_timeout(Duration::from_secs(5))
                    .map_err(|error| {
                        format!("primary xterm did not publish a startup frame: {error}")
                    })?,
            )
        } else {
            None
        };
    if config.secondary_terminal {
        process.add_secondary_child(
            None,
            spawn_secondary_xterm(
                terminal
                    .as_deref()
                    .expect("secondary terminal requires xterm"),
                &config.display,
                xauthority.path(),
                config
                    .inject_text
                    .as_deref()
                    .or(config.expect_physical_text.as_deref()),
            )?,
        );
    }
    for id in config.applications.startup.iter().skip(1) {
        let app = config
            .applications
            .applications
            .get(id)
            .expect("normal session startup application was validated");
        process.add_secondary_child(
            Some(app.id.clone()),
            PersistentXtermSessionConfig::spawn_session_application(
                app,
                &config.display,
                xauthority.path(),
            )?,
        );
        println!(
            "sophia_session_app schema=1 status=started id={} source=startup",
            app.id
        );
    }

    let mut randr_witness = config
        .inject_output_size
        .map(|_| open_randr_update_witness(&config.socket_path, xauthority_cookie))
        .transpose()?;
    let mut output_notifications = 0usize;
    if let Some(size) = config.inject_output_size {
        let mut snapshot = output_topology.clone();
        snapshot.generation = snapshot.generation.saturating_add(1);
        let primary_id = snapshot.primary;
        let primary = snapshot
            .outputs
            .iter_mut()
            .find(|entry| entry.output == primary_id)
            .ok_or("live output injection lost the primary output")?;
        primary.logical.width = size.width;
        primary.logical.height = size.height;
        primary.pixel_size = size;
        snapshot
            .validate()
            .map_err(|error| format!("invalid --inject-output-size topology: {error:?}"))?;
        let (ack_sender, ack_receiver) = sync_channel(1);
        service_command_sender.send(XServerFrontendServiceCommand::UpdateOutputTopology {
            snapshot,
            acknowledgement: ack_sender,
        })?;
        let outcome = ack_receiver.recv_timeout(Duration::from_secs(1))?;
        let notifications = match outcome {
            sophia_x_authority::XAuthorityOutputUpdateOutcome::Applied {
                notifications, ..
            } => notifications,
            outcome => {
                return Err(format!("live output injection was rejected: {outcome:?}").into());
            }
        };
        output_notifications = notifications;
        let witness = randr_witness
            .as_mut()
            .ok_or("live output injection lost its RandR witness")?;
        confirm_randr_update_witness(witness, size)?;
        println!(
            "sophia_live_output_update schema=3 status=applied width={} height={} notifications={} witness=true",
            size.width, size.height, notifications
        );
    }

    println!(
        "sophia_live_session_mode schema=1 mode={} configured_apps={} startup_apps={}",
        if config.normal_session {
            "normal"
        } else {
            "proof"
        },
        config.applications.applications.len(),
        config.applications.startup.len(),
    );

    println!(
        "sophia_live_session schema=7 status=running display={} terminal=xterm runtime=persistent authority_capacity={} input_capacity={} control_capacity={} native_presentation={} physical_input={} pointer_proof={} secondary_terminal={} wm_policy={} namespace_profile={} namespace_request_capabilities={} namespace_publish_capabilities={}",
        config.display,
        SESSION_AUTHORITY_CAPACITY,
        SESSION_KEY_CAPACITY,
        SESSION_CONTROL_CAPACITY,
        if native_scanout.is_some() {
            "enabled"
        } else {
            "disabled"
        },
        if physical_input.is_some() {
            "enabled"
        } else {
            "disabled"
        },
        if config.expect_physical_pointer {
            "enabled"
        } else {
            "disabled"
        },
        if config.secondary_terminal {
            "enabled"
        } else {
            "disabled"
        },
        if wm_session.is_some() {
            "external"
        } else {
            "disabled"
        },
        match config.namespace_profile {
            NamespaceProfile::ClassicShared => "classic_shared",
            NamespaceProfile::Confined => "confined",
        },
        config.namespace_capabilities.request_bits(),
        config.namespace_capabilities.publish_bits(),
    );
    if config.normal_session && config.applications.startup.is_empty() {
        println!("sophia_live_session schema=1 status=desktop_ready startup_apps=0");
    }
    if let Some(native_scanout) = native_scanout.as_ref() {
        println!(
            "sophia_live_outputs schema=2 status=ready discovered={} presentation={} native_owned={} multi_output_scanout=enabled layout=extended_horizontal",
            native_scanout.discovered_outputs,
            native_scanout.presentation_outputs,
            native_scanout.heads.len(),
        );
    }

    let (primary_child, secondary_children) = process.children_mut();
    let result = run_session_loop(
        &config,
        SessionLoopChannels {
            authority: &authority_receiver,
            input: &input_sender,
            control: &control_sender,
            control_acknowledgements: &control_ack_receiver,
            input_deliveries: &input_delivery_receiver,
        },
        SessionLoopResources {
            child: primary_child,
            secondary_children,
            physical_input: &mut physical_input,
            native_scanout: &mut native_scanout,
            wm_session: &mut wm_session,
        },
        SessionLoopStartup {
            xauthority: xauthority.path(),
            protocol_router,
            input_proof_result: input_proof_result.as_ref(),
            client_stdout_capture: client_stdout_capture.as_ref(),
            require_startup_focus: false,
            initial_authority_batch,
            output_notifications,
        },
    );
    drop(randr_witness);
    // Stop frontend routing before terminating its clients. Pointer motion can
    // leave a bounded burst in the Engine ingress queue; killing xterm first
    // turns that normal shutdown backlog into a client-queue disconnect.
    let _ = service_command_sender.send(XServerFrontendServiceCommand::StopAccepting);
    drop(input_sender);
    drop(control_sender);
    process.terminate()?;
    let server_result = server
        .take()
        .expect("X Server Frontend handle is retained after startup")
        .join()
        .map_err(|_| "persistent X authority server thread panicked")?;
    server_result.map_err(|error| format!("persistent X authority server failed: {error}"))?;
    namespace_registry
        .lock()
        .map_err(|_| "Sophia namespace registry lock was poisoned")?
        .revoke_namespace(x_namespace.id)?;
    let xauthority_cleanup = xauthority.remove();
    result?;
    xauthority_cleanup?;
    println!(
        "sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 namespace=revoked xauthority=removed"
    );
    Ok(())
}

include!("live_session/owner_loop.rs");

#[cfg(test)]
mod tests {
    use super::{
        BufferSource, CommittedSurfaceState, LiveClientStdoutCapture, LiveProductionCpuScene,
        LiveProductionVisualRuntime, LiveXAuthorityFile, PRIMARY_INPUT_PROOF_SCRIPT,
        PersistentXtermSessionConfig, PhysicalInputRoutingMode, Rect, Region,
        SECONDARY_POINTER_WITNESS_SCRIPT, SessionPointerPlacement, SessionProcessGuard, Size,
        authority_transaction_count, center_geometry_without_scaling,
        global_runtime_deadline_ends_session, layer_snapshots_from_committed,
        physical_input_pixels_already_changed, physical_input_routing_mode,
        place_pointer_event_for_routing, pointer_offset_for_geometry, record_runtime_commits,
        route_input_events, session_protocol_errors_are_fatal,
        successful_primary_exit_ends_session, take_settled_input_delivery_wait,
    };
    use sophia_engine::{InputFocusState, WmShortcutRegistry, WmShortcutRouter};
    use sophia_protocol::{
        AuthorityKind, DeviceId, InputEventKind, InputEventPacket, NamespaceCapabilities,
        NamespaceProfile, Point, SeatId, SurfaceId, SurfaceTransaction,
        SurfaceTransactionReadiness, WM_API_VERSION, WmActionId, WmBindingRegistration,
        WmCapabilities, WmHello, WmModifierMask, WmSessionAction,
    };
    use sophia_x_authority::X_AUTHORITY_CPU_BUFFER_FORMAT_XRGB8888;
    use sophia_x_authority::{XAuthorityClientSurfaceRoutes, XCoreKeyboardMapper};
    use std::io::Write;
    use std::sync::mpsc::sync_channel;
    use std::time::{Duration, Instant};

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
            PersistentXtermSessionConfig::from_args(&["--input-seat=../../seat0".to_owned()])
                .is_err()
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

    #[test]
    fn compatibility_surface_is_centered_without_resizing() {
        let geometry = center_geometry_without_scaling(
            Rect {
                x: 19,
                y: 27,
                width: 800,
                height: 600,
            },
            Size {
                width: 1280,
                height: 720,
            },
        );
        assert_eq!(geometry.x, 240);
        assert_eq!(geometry.y, 60);
        assert_eq!(geometry.width, 800);
        assert_eq!(geometry.height, 600);
    }

    #[test]
    fn oversized_compatibility_surface_keeps_size_and_anchors_at_origin() {
        let geometry = center_geometry_without_scaling(
            Rect {
                x: 19,
                y: 27,
                width: 1920,
                height: 1080,
            },
            Size {
                width: 1280,
                height: 720,
            },
        );
        assert_eq!(geometry.x, 0);
        assert_eq!(geometry.y, 0);
        assert_eq!(geometry.width, 1920);
        assert_eq!(geometry.height, 1080);
    }

    #[test]
    fn terminal_readiness_is_scoped_to_the_focused_surface() {
        let focused = SurfaceId::new(21, 1);
        let secondary = SurfaceId::new(22, 1);
        let mut scene = LiveProductionCpuScene::new(Size {
            width: 4,
            height: 1,
        });
        let committed = vec![
            test_committed_cpu_surface(
                focused,
                Rect {
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 1,
                },
                1,
            ),
            test_committed_cpu_surface(
                secondary,
                Rect {
                    x: 2,
                    y: 0,
                    width: 2,
                    height: 1,
                },
                2,
            ),
        ];
        scene
            .apply_updates(
                [
                    sophia_backend_live::LiveCpuBufferUpdate::Replace(test_cpu_buffer(
                        1, [0xff; 8],
                    )),
                    sophia_backend_live::LiveCpuBufferUpdate::Replace(test_cpu_buffer(
                        2,
                        [0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0xff],
                    )),
                ],
                &committed,
            )
            .unwrap();

        assert!(!scene.surface_has_visual_detail(&committed, focused));
        assert!(scene.surface_has_visual_detail(&committed, secondary));

        scene
            .apply_updates(
                [sophia_backend_live::LiveCpuBufferUpdate::Replace(
                    test_cpu_buffer(1, [0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0xff]),
                )],
                &committed,
            )
            .unwrap();
        assert!(scene.surface_has_visual_detail(&committed, focused));
    }

    #[test]
    fn focused_surface_is_composed_above_an_overlapping_client() {
        let focused = SurfaceId::new(31, 1);
        let secondary = SurfaceId::new(32, 1);
        let geometry = Rect {
            x: 0,
            y: 0,
            width: 2,
            height: 1,
        };
        let mut scene = LiveProductionCpuScene::new(Size {
            width: 2,
            height: 1,
        });
        let committed = vec![
            test_committed_cpu_surface(focused, geometry, 1),
            test_committed_cpu_surface(secondary, geometry, 2),
        ];
        let focused_pixels = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
        let secondary_pixels = [0, 0, 0, 0xff, 0, 0, 0, 0xff];
        scene
            .apply_updates(
                [
                    sophia_backend_live::LiveCpuBufferUpdate::Replace(test_cpu_buffer(
                        1,
                        focused_pixels,
                    )),
                    sophia_backend_live::LiveCpuBufferUpdate::Replace(test_cpu_buffer(
                        2,
                        secondary_pixels,
                    )),
                ],
                &committed,
            )
            .unwrap();

        assert_eq!(
            scene.compose(&committed, None, None).unwrap().frame.bytes,
            secondary_pixels.to_vec()
        );
        assert_eq!(
            scene
                .compose(&committed, Some(focused), None)
                .unwrap()
                .frame
                .bytes,
            focused_pixels.to_vec()
        );
    }

    fn test_committed_cpu_surface(
        surface: SurfaceId,
        geometry: Rect,
        handle: u64,
    ) -> CommittedSurfaceState {
        CommittedSurfaceState {
            surface,
            committed_generation: 1,
            geometry,
            buffer: BufferSource::CpuBuffer { handle },
            damage: Region::single(geometry),
        }
    }

    fn test_cpu_buffer(handle: u64, bytes: [u8; 8]) -> sophia_backend_live::LiveCpuBufferSource {
        sophia_backend_live::LiveCpuBufferSource {
            handle,
            size: Size {
                width: 2,
                height: 1,
            },
            stride: 8,
            format: X_AUTHORITY_CPU_BUFFER_FORMAT_XRGB8888,
            generation: 1,
            bytes: bytes.to_vec(),
        }
    }

    #[test]
    fn committed_snapshot_preserves_surface_generation_in_render_layers() {
        let layers = layer_snapshots_from_committed(&[CommittedSurfaceState {
            surface: sophia_protocol::SurfaceId::new(9, 1),
            committed_generation: 4,
            geometry: Rect {
                x: 10,
                y: 20,
                width: 300,
                height: 200,
            },
            buffer: BufferSource::CpuBuffer { handle: 99 },
            damage: Region::single(Rect {
                x: 0,
                y: 0,
                width: 300,
                height: 200,
            }),
        }]);

        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].generation, 4);
        assert_eq!(layers[0].source, BufferSource::CpuBuffer { handle: 99 });
    }

    #[test]
    fn authority_batch_commits_once_and_fans_out_one_snapshot() {
        let outputs = [17u64, 18]
            .into_iter()
            .map(|id| sophia_engine::HeadlessOutput {
                id: sophia_protocol::OutputId::from_raw(id),
                size: Size {
                    width: 640,
                    height: 480,
                },
                scale: 1,
            })
            .collect::<Vec<_>>();
        let surface = sophia_protocol::SurfaceId::new(17, 1);
        let mut runtime = LiveProductionVisualRuntime::new(&outputs, None, None).unwrap();
        let transaction = SurfaceTransaction {
            transaction: sophia_protocol::TransactionId::from_raw(90),
            authority: AuthorityKind::SophiaX,
            surface,
            namespace: None,
            target_geometry: Rect {
                x: 4,
                y: 8,
                width: 632,
                height: 464,
            },
            target_buffer: BufferSource::CpuBuffer { handle: 18 },
            damage: Region::single(Rect {
                x: 0,
                y: 0,
                width: 632,
                height: 464,
            }),
            readiness: SurfaceTransactionReadiness::Ready,
            timeout_msec: 250,
            previous_committed_generation: 0,
        };

        let report = runtime
            .run_authority_transactions(
                sophia_protocol::TransactionId::from_raw(90),
                std::slice::from_ref(&transaction),
                &[],
                1,
                None,
                None,
                None,
            )
            .unwrap();

        assert_eq!(
            report
                .engine
                .runtime
                .runtime_state
                .authority_transactions_committed,
            1
        );
        assert_eq!(runtime.committed_surfaces().len(), 1);
        assert_eq!(runtime.committed_surfaces()[0].committed_generation, 1);
        for index in 0..runtime.output_count() {
            let committed = runtime.output_committed(index).unwrap();
            assert_eq!(committed.len(), 1);
            assert_eq!(committed[0].committed_generation, 1);
        }
    }
}
