use super::prelude::*;

use sophia_backend_live::{
    ClassicHardwareCursorUpdate, LiveProductionAuthorityBatch, LiveProductionCpuScene,
    LiveProductionCursorPresentation, LiveProductionCycleRequest, LiveProductionDmaBufRegistration,
    LiveProductionFenceRegistration, LiveProductionNativeScanout, LiveProductionRetiredPresent,
    LiveProductionVisualRuntime,
};
use sophia_cli::emergency_input::{EmergencyChordAction, EmergencyChordState};
use sophia_cli::input_proof::{PhysicalTextProof, PhysicalTextProofEvent};
use sophia_cli::resize_transaction::{
    PendingLayoutGeometryAuthority, ResizeVisualCommit, ResizeVisualCommitTracker,
    merge_unrequested_layout_observation, project_authority_batch_onto_layout,
};
use sophia_cli::session_actions::{
    SessionLaunchIntent, SessionLaunchQueue, SessionLaunchQueueOutcome,
};
use sophia_cli::session_control::{SESSION_CONTROL_CAPACITY, SessionControlQueue};
use sophia_cli::session_keyboard::{
    PhysicalKeyboardCoverage, SESSION_CLIENT_PRESSED_KEY_CAPACITY, SessionClientKeyState,
    SessionClientPressedKey, VirtualTerminalChordAction, VirtualTerminalChordState,
};
use sophia_cli::session_shutdown::{
    SessionLogoutDrainDecision, SessionLogoutDrainState, session_logout_drain_decision,
};
use sophia_cli::session_startup::{
    SessionStartupEvent, SessionStartupReadiness, reduce_session_startup,
};
use sophia_engine::{
    FocusedInputRoute, InputFocusDecision, InputFocusState, KeyRepeatConfig, KeyRepeatState,
    KeyRepeatTarget, LayoutEpochCoordinator, NonBlockingInputPoller, OutputFrameServiceRequest,
    OutputNativeFramePhase, PointerFocusHandoffState, WmPolicyApplyOutcome, WmShortcutRegistry,
    WmShortcutRouter, WmWorkspaceState,
};
use sophia_protocol::{
    ClientAdmissionContext, DeviceId, NamespaceCapabilities, NamespaceId, NamespaceProfile, Point,
    SeatId, SessionApplicationId, WM_DEFAULT_WORKSPACES, WmActionActivation, WmActionId,
    WmManageSurface, WmPolicyAckOutcome, WmPolicyUpdate, WmResponsePacket, WmSessionAction,
};
use sophia_runtime::NamespaceRegistry;
use sophia_x_authority::{
    XAuthorityClientControlAck, XAuthorityClientControlCommand, XAuthorityClientInputDelivery,
    XAuthorityClientSurfaceRoutes, XAuthorityControlCommand, XAuthorityControlKind,
    XAuthorityInputDeliveryId, XAuthorityInputDeliveryOutcome, XAuthorityRoutedInput,
    XAuthorityRoutedInputMode, XCoreKeyboardMapper, XPresentCompletionMode,
    XServerFrontendAdmissionError, XServerFrontendAdmissionPolicy, XServerFrontendAdmissionRequest,
    XServerFrontendConfig, XServerFrontendControlRouter, XServerFrontendProtocolRouter,
    XServerFrontendRenderDeviceError, XServerFrontendRenderDeviceProvider,
    XServerFrontendRouteBroker, XServerFrontendRouteCapacities, XServerFrontendServiceCommand,
    XServerFrontendSetupAuthorization, XkbKeymapSnapshot,
    run_x_server_frontend_routed_until_stopped,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::{Read, Write};
use std::num::NonZeroUsize;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod authority_file;
pub(super) mod input_guard;
mod native_retirement;
mod process_supervision;
mod proof_artifacts;
mod startup_readiness;
mod wm_transport_worker;
mod x_frontend;

use authority_file::{LiveXAuthorityFile, fill_session_random};
use native_retirement::{
    NativePresentRetirementObservation, correlate_physical_input_page_flip,
    record_native_present_retirement, record_native_software_present_retirement,
};
use process_supervision::{
    ManagedSessionChild, SessionProcessGuard, managed_child_exit_is_nonfatal,
    terminate_session_child,
};
use proof_artifacts::{LiveClientStdoutCapture, LiveInputProofResult};
use startup_readiness::{
    StartupSurfacePresentationEvidence, all_startup_outputs_presented,
    independent_native_output_presented, rects_intersect, startup_native_recovery_reason,
    startup_output_evidence, startup_submission_requirement, startup_surface_visual_detail,
    synchronous_modeset_record,
};
use wm_transport_worker::{WmTransportPolicyEvent, WmTransportSubmitError, WmTransportWorker};
use x_frontend::{LiveXAdmissionPolicy, LiveXRenderDeviceProvider};

include!("live_session/config.rs");
include!("live_session/input.rs");
include!("live_session/policy.rs");
include!("live_session/presentation.rs");
include!("live_session/startup.rs");
include!("live_session/wm.rs");

const SESSION_AUTHORITY_CAPACITY: usize = 256;
const SESSION_KEY_CAPACITY: usize = 64;
// One accepted Present can emit independent Complete and Idle records. Size
// protocol transport from authority work, not from the smaller input queue.
const SESSION_PRESENT_PROTOCOL_CAPACITY: usize = SESSION_AUTHORITY_CAPACITY * 2;
const SESSION_INPUT_QUIET_MSEC: u64 = 500;
const SESSION_PHYSICAL_SEQUENCE_TIMEOUT_MSEC: u64 = 15_000;
const SESSION_PHYSICAL_PIXEL_TIMEOUT_MSEC: u64 = 5_000;
const SESSION_COMPLETION_TIMEOUT_MSEC: u64 = 5_000;
const SESSION_WM_TRANSPORT_RESPONSE_TIMEOUT_MSEC: u64 = 4_000;
const SESSION_WM_TRANSACTION_TIMEOUT_MAX_MSEC: u32 = 10_000;
const SESSION_APP_ADMISSION_TIMEOUT_MSEC: u64 = 12_000;
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

    fn drain_event_timings(&mut self) -> Vec<sophia_backend_live::ThreadedNativeInputEventTiming> {
        match self {
            Self::Threaded(poller) => poller.drain_event_timings(),
        }
    }
}

fn open_session_physical_input(
    config: &PersistentXtermSessionConfig,
    device_map: sophia_backend_live::NativeLibinputDeviceMap,
    seat_opener: Option<sophia_backend_live::LiveSeatDeviceOpener>,
) -> Result<Option<SessionPhysicalInput>, Box<dyn std::error::Error>> {
    if !config.input_devices.is_empty() {
        return Ok(Some(SessionPhysicalInput::Threaded(
            sophia_backend_live::open_threaded_native_libinput_path_poller(
                &config.input_devices,
                device_map,
                64,
                256,
            )?,
        )));
    }
    config
        .input_seat
        .as_deref()
        .map(|seat_name| {
            if let Some(opener) = seat_opener {
                sophia_backend_live::open_threaded_native_libinput_udev_poller_with_seat(
                    seat_name, device_map, 64, 256, opener,
                )
            } else {
                sophia_backend_live::open_threaded_native_libinput_udev_poller(
                    seat_name, device_map, 64, 256,
                )
            }
            .map(SessionPhysicalInput::Threaded)
            .map_err(|error| error.into())
        })
        .transpose()
}

pub(crate) fn run_persistent_xterm_session(
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = PersistentXtermSessionConfig::from_args(args)?;
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
    let mut seat_controller = config
        .native_scanout
        .then(sophia_backend_live::LiveSeatController::open)
        .transpose()?;
    if let Some(controller) = seat_controller.as_mut() {
        let _ = controller.dispatch()?;
        println!(
            "sophia_live_seat schema=1 status=active seat={}",
            controller.name()
        );
    }
    let mut native_scanout = seat_controller
        .as_ref()
        .map(|controller| LiveProductionNativeScanout::new_with_seat(&controller.device_opener()))
        .transpose()?;
    let device_map =
        sophia_backend_live::NativeLibinputDeviceMap::new(SeatId::from_raw(SESSION_SEAT_RAW))
            .with_keyboard_device(DeviceId::from_raw(SESSION_KEYBOARD_DEVICE_RAW))
            .with_pointer_device(DeviceId::from_raw(SESSION_POINTER_DEVICE_RAW));
    let mut physical_input = open_session_physical_input(
        &mut config,
        device_map,
        seat_controller
            .as_ref()
            .map(sophia_backend_live::LiveSeatController::device_opener),
    )?;
    if let Some(physical_input) = physical_input.as_ref() {
        let policy = physical_input.policy_report();
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
    let policy_map_mode = LivePolicyMapMode::from_external_wm(wm_session.is_some());
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
            // XLibre maps immediately unless a redirecting policy owner is
            // present. Deferring without a WM strands the client's toplevel
            // before MapNotify, VisibilityNotify, and Expose.
            .with_policy_map_deferred(policy_map_mode.frontend_deferred())
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
    // Completion notifications must never kill an X11 writer merely because
    // another client filled a shared acknowledgement queue while the owner was
    // committing a WM transaction. Routed input itself remains bounded.
    let (input_delivery_sender, input_delivery_receiver) = channel();
    let broker = XServerFrontendRouteBroker::with_route_capacities_and_xkb_config(
        XServerFrontendRouteCapacities::new(
            NonZeroUsize::new(SESSION_KEY_CAPACITY)
                .expect("session input route capacity is nonzero"),
            NonZeroUsize::new(SESSION_CONTROL_CAPACITY)
                .expect("session control route capacity is nonzero"),
            NonZeroUsize::new(SESSION_PRESENT_PROTOCOL_CAPACITY)
                .expect("session protocol route capacity is nonzero"),
            NonZeroUsize::new(SESSION_KEY_CAPACITY)
                .expect("session presentation route capacity is nonzero"),
        ),
        control_ack_sender,
        input_delivery_sender,
        config.xkb_config.clone(),
    )?;
    println!(
        "sophia_live_x11_route_capacity schema=1 input={} control={} protocol={} presentations={}",
        SESSION_KEY_CAPACITY,
        SESSION_CONTROL_CAPACITY,
        SESSION_PRESENT_PROTOCOL_CAPACITY,
        SESSION_KEY_CAPACITY,
    );
    let input_sender = broker.routed_input_sender();
    let control_sender = broker.control_router();
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
        &mut config,
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
            seat_controller: &mut seat_controller,
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
    println!("sophia_live_session_lifecycle schema=1 status=stopping_frontend");
    // Stop frontend routing before terminating its clients. Pointer motion can
    // leave a bounded burst in the Engine ingress queue; killing xterm first
    // turns that normal shutdown backlog into a client-queue disconnect.
    let _ = service_command_sender.send(XServerFrontendServiceCommand::StopAccepting);
    drop(input_sender);
    drop(control_sender);
    println!("sophia_live_session_lifecycle schema=1 status=stopping_clients");
    process.terminate()?;
    let _ = service_command_sender.send(XServerFrontendServiceCommand::StopAndDisconnect);
    println!("sophia_live_session_lifecycle schema=1 status=joining_frontend");
    let server_result = server
        .take()
        .expect("X Server Frontend handle is retained after startup")
        .join()
        .map_err(|_| "persistent X authority server thread panicked")?;
    println!("sophia_live_session_lifecycle schema=1 status=frontend_joined");
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

include!("live_session/owner_loop_state.rs");
include!("live_session/owner_loop.rs");

mod tests;
