#[path = "config/chrome.rs"]
mod chrome;
#[path = "config/firefox_stage.rs"]
mod firefox_stage;
#[path = "config/output.rs"]
mod output;
#[path = "config/session.rs"]
mod session;
use firefox_stage::FirefoxM8StageProof;
use output::{
    output_topology_from_engine_outputs, output_topology_from_engine_outputs_at_generation,
    wm_output_bounds,
};
use session::{SessionApplicationConfig, SessionApplicationSpec};

const TERMINAL_APPLICATION_ID: SessionApplicationId = SessionApplicationId::from_raw(1);
const LAUNCHER_APPLICATION_ID: SessionApplicationId = SessionApplicationId::from_raw(2);
const BROWSER_APPLICATION_ID: SessionApplicationId = SessionApplicationId::from_raw(3);

fn session_action_evidence_name(action: WmSessionAction) -> &'static str {
    match action {
        WmSessionAction::LaunchApplication { application }
            if application == TERMINAL_APPLICATION_ID =>
        {
            "LaunchTerminal"
        }
        WmSessionAction::LaunchApplication { application }
            if application == LAUNCHER_APPLICATION_ID =>
        {
            "LaunchApplicationMenu"
        }
        WmSessionAction::LaunchApplication { application }
            if application == BROWSER_APPLICATION_ID =>
        {
            "LaunchFirefox"
        }
        WmSessionAction::LaunchApplication { .. } => "LaunchApplication",
        WmSessionAction::CloseFocused => "CloseFocused",
        WmSessionAction::Logout => "Logout",
    }
}

#[derive(Clone, Debug)]
struct PersistentXtermSessionConfig {
    display: String,

    socket_path: std::path::PathBuf,
    terminal: String,
    terminal_exec: Option<String>,
    terminal_exec_args: Vec<String>,
    session_launcher: Option<String>,
    session_firefox: Option<String>,
    client: Option<String>,
    client_args: Vec<String>,
    expect_client_stdout: Option<String>,
    require_client_normal_exit: bool,
    normal_session: bool,
    exit_when_startup_exits: bool,
    startup_ready_timeout: Option<Duration>,
    applications: SessionApplicationConfig,
    secondary_terminal: bool,
    max_runtime: Option<Duration>,
    max_ticks: Option<usize>,
    inject_text: Option<String>,
    expect_physical_text: Option<String>,
    physical_sequence_timeout_msec: u64,
    expect_physical_pointer: bool,
    exit_after_input_proof: bool,
    input_devices: Vec<std::path::PathBuf>,
    input_seat: Option<String>,
    native_scanout: bool,
    software_client_rendering: bool,
    wm_process: Option<String>,
    wm_process_args: Vec<String>,
    wm_interface: sophia_config::ExternalWmInterface,
    wm_public_fault_after: Option<PublicPolicyFaultPoint>,
    wm_socket_path: std::path::PathBuf,
    input_quiet_msec: u64,
    namespace_profile: NamespaceProfile,
    namespace_capabilities: NamespaceCapabilities,
    xkb_config: sophia_x_authority::XkbRmlvoConfig,
    key_repeat_config: sophia_config::RepeatConfig,
    initial_caps_lock: bool,
    initial_num_lock: bool,
    pointer_candidate: Option<sophia_config::DesktopPointerCandidate>,
    desktop_profile: sophia_config::DesktopProfileGeneration,
    core_config_source: sophia_config::ConfigSource,
    core_config_state: sophia_config::CoreConfigState,
    surface_chrome_style: sophia_engine::SurfaceChromeStyle,
    verbose_diagnostics: bool,
    inject_output_size: Option<Size>,
    inject_surface_resize: Option<Size>,
    inject_surface_resize_sequence: Vec<Size>,
    m4_first_acquire_delay: Option<Duration>,
    m4_reject_first_present: bool,
    m4_diagnose_first_mixed_export: bool,
    firefox_m8_proof: bool,
    firefox_m10_proof: bool,
    firefox_m10_rendering_proof: bool,
    firefox_m10_dialog_proof: bool,
    firefox_m10_primary_proof: bool,
    firefox_m10_selection_proof: bool,
    firefox_m10_lifecycle_proof: bool,
}

impl PersistentXtermSessionConfig {
    pub(super) fn keyboard_mapper(&self) -> XCoreKeyboardMapper {
        XCoreKeyboardMapper::with_locks(self.initial_caps_lock, self.initial_num_lock)
    }

    pub(super) fn native_pointer_policy(&self) -> sophia_backend_live::NativeLibinputPointerPolicy {
        let Some(candidate) = self.pointer_candidate else {
            return sophia_backend_live::NativeLibinputPointerPolicy::default();
        };
        sophia_backend_live::NativeLibinputPointerPolicy {
            natural_scroll: candidate.natural_scroll,
            accel_profile: candidate.accel_profile.map(|profile| match profile {
                sophia_config::DesktopPointerAccelProfile::Flat => {
                    sophia_backend_live::NativeLibinputAccelProfile::Flat
                }
                sophia_config::DesktopPointerAccelProfile::Adaptive => {
                    sophia_backend_live::NativeLibinputAccelProfile::Adaptive
                }
            }),
            accel_speed: candidate.accel_speed,
            left_handed: candidate.left_handed,
            middle_emulation: candidate.middle_emulation,
            scroll_factor: candidate.scroll_factor.unwrap_or(1.0),
        }
    }

    fn from_args(args: &[String]) -> Result<Self, Box<dyn std::error::Error>> {
        let no_config = args.iter().any(|argument| argument == "--no-config");
        let explicit_config = arg_value(args, "--config")
            .map(std::path::PathBuf::from);
        if no_config && explicit_config.is_some() {
            return Err("--no-config and --config are mutually exclusive".into());
        }
        if explicit_config
            .as_ref()
            .is_some_and(|path| !path.is_absolute() || path.as_os_str().is_empty())
        {
            return Err("--config requires an absolute path".into());
        }
        let core_config_source = if no_config {
            sophia_config::ConfigSource {
                class: sophia_config::ConfigSourceClass::CompiledDefault,
                path: None,
            }
        } else {
            sophia_config::discover_default_config_source(
                sophia_config::ConfigDomain::Core,
                explicit_config.as_deref(),
            )
        };
        let core_config_state = sophia_config::CoreConfigState::load(&core_config_source)?;
        let core_snapshot = core_config_state.active();
        let explicit_desktop_profile = arg_value(args, "--desktop-profile").map(Into::into);
        if no_config && explicit_desktop_profile.is_some() {
            return Err("--no-config and --desktop-profile are mutually exclusive".into());
        }
        if explicit_desktop_profile
            .as_ref()
            .is_some_and(|path: &std::path::PathBuf| !path.is_absolute())
        {
            return Err("--desktop-profile requires an absolute path".into());
        }
        let user_config_root = sophia_config::default_user_config_root();
        let desktop_profile_source = if no_config {
            None
        } else {
            sophia_config::discover_desktop_profile_source(
                explicit_desktop_profile.as_deref(),
                user_config_root.as_deref(),
            )
        };
        let desktop_profile = sophia_config::load_desktop_profile(
            desktop_profile_source.as_deref(),
            sophia_config::ConfigGeneration::INITIAL,
        )?;
        let desktop_input = sophia_config::prepare_desktop_input_candidate(
            desktop_profile
                .candidates
                .get(&sophia_config::DesktopAuthority::Input)
                .expect("desktop profile partition creates an input candidate"),
        )?;
        let display = arg_value(args, "--display").unwrap_or_else(|| ":77".to_owned());
        let display_number = parse_display_number(&display)?;
        let normal_session = args.iter().any(|arg| arg == "--session-mode=normal")
            || !core_snapshot.session.applications.is_empty();
        let exit_when_startup_exits = args.iter().any(|arg| arg == "--exit-when-startup-exits");
        let startup_ready_timeout = arg_value(args, "--startup-ready-timeout-ms")
            .as_deref()
            .map(parse_u64)
            .transpose()?
            .map(Duration::from_millis);
        if startup_ready_timeout.is_some_and(|timeout| {
            timeout < Duration::from_millis(100) || timeout > Duration::from_secs(60)
        }) {
            return Err("--startup-ready-timeout-ms accepts 100-60000 milliseconds".into());
        }
        let mut applications = Self::applications_from_core(core_snapshot)?;
        for value in args
            .iter()
            .filter_map(|arg| arg.strip_prefix("--session-app="))
        {
            let (id, executable) = value
                .split_once('=')
                .ok_or("--session-app expects ID=/absolute/executable")?;
            Self::validate_session_app_id(id)?;
            let executable = std::path::PathBuf::from(executable);
            if !executable.is_absolute() || executable.as_os_str().is_empty() {
                return Err("--session-app executable must be an absolute path".into());
            }
            if applications.applications.len() >= 32 {
                return Err("--session-app accepts at most 32 applications".into());
            }
            if applications
                .applications
                .insert(
                    id.to_owned(),
                    SessionApplicationSpec {
                        id: id.to_owned(),
                        executable,
                        arguments: Vec::new(),
                    },
                )
                .is_some()
            {
                return Err(format!("duplicate --session-app ID {id:?}").into());
            }
        }
        for value in args
            .iter()
            .filter_map(|arg| arg.strip_prefix("--session-app-arg="))
        {
            let (id, argument) = value
                .split_once('=')
                .ok_or("--session-app-arg expects ID=ARG")?;
            if argument.len() > 4_096 {
                return Err("--session-app-arg accepts at most 4096 bytes".into());
            }
            let app = applications
                .applications
                .get_mut(id)
                .ok_or_else(|| format!("--session-app-arg references unknown app {id:?}"))?;
            if app.arguments.len() >= 32 {
                return Err(format!("session app {id:?} accepts at most 32 arguments").into());
            }
            app.arguments.push(argument.to_owned());
        }
        let desktop_session = sophia_config::prepare_desktop_session_candidate(
            desktop_profile
                .candidates
                .get(&sophia_config::DesktopAuthority::Session)
                .expect("desktop profile partition creates a session candidate"),
        )?;
        let terminal_overridden = args.iter().any(|argument| {
            argument
                .strip_prefix("--session-action-app=")
                .is_some_and(|value| value.starts_with("terminal="))
        });
        let browser_overridden = args.iter().any(|argument| {
            argument
                .strip_prefix("--session-action-app=")
                .is_some_and(|value| value.starts_with("firefox="))
        });
        let startup_overridden = args
            .iter()
            .any(|argument| argument.starts_with("--session-start="));
        applications.apply_desktop_candidate(
            &desktop_session,
            terminal_overridden,
            browser_overridden,
            startup_overridden,
        )?;
        if startup_overridden {
            applications.startup.clear();
        }
        for id in args
            .iter()
            .filter_map(|arg| arg.strip_prefix("--session-start="))
        {
            Self::validate_session_app_id(id)?;
            if !applications.applications.contains_key(id) {
                return Err(format!("--session-start references unknown app {id:?}").into());
            }
            if applications.startup.iter().any(|entry| entry == id) {
                return Err(format!("duplicate --session-start ID {id:?}").into());
            }
            applications.startup.push(id.to_owned());
        }
        let mut explicit_session_actions = BTreeSet::new();
        for value in args
            .iter()
            .filter_map(|arg| arg.strip_prefix("--session-action-app="))
        {
            let (action, id) = value
                .split_once('=')
                .ok_or("--session-action-app expects terminal|launcher|firefox=ID")?;
            if !applications.applications.contains_key(id) {
                return Err(format!("--session-action-app references unknown app {id:?}").into());
            }
            if !explicit_session_actions.insert(action) {
                return Err(format!("duplicate session action mapping {action:?}").into());
            }
            let slot = match action {
                "terminal" => &mut applications.terminal,
                "launcher" => &mut applications.launcher,
                "firefox" => &mut applications.firefox,
                _ => {
                    return Err(
                        "--session-action-app expects terminal, launcher, or firefox".into(),
                    );
                }
            };
            *slot = Some(id.to_owned());
        }
        if normal_session {
            let terminal_proof = args.iter().any(|arg| {
                arg.starts_with("--inject-text=") || arg.starts_with("--expect-physical-text=")
            });
            let startup_terminal = applications.startup.len() == 1
                && applications.terminal.as_ref() == applications.startup.first();
            if applications.startup.is_empty()
                && applications.terminal.is_none()
                && applications.launcher.is_none()
                && applications.firefox.is_none()
            {
                return Err(
                    "--session-mode=normal requires a startup app or session action mapping".into(),
                );
            }
            let proof_only = args.iter().any(|arg| {
                arg == "--secondary-terminal"
                    || arg == "--proof"
                    || arg.starts_with("--client=")
                    || arg.starts_with("--terminal=")
                    || arg.starts_with("--terminal-exec=")
                    || ((arg.starts_with("--inject-text=")
                        || arg.starts_with("--expect-physical-text="))
                        && !startup_terminal)
            });
            if proof_only {
                return Err(
                    "--session-mode=normal cannot be combined with proof or terminal-specific options"
                        .into(),
                );
            }
            if exit_when_startup_exits && applications.startup.len() != 1 {
                return Err(
                    "--exit-when-startup-exits requires exactly one normal-session startup app"
                        .into(),
                );
            }
            if startup_ready_timeout.is_some() && applications.startup.is_empty() {
                return Err(
                    "--startup-ready-timeout-ms requires at least one startup application".into(),
                );
            }
            if terminal_proof && !startup_terminal {
                return Err(
                    "normal-session input proof requires one startup app mapped to the terminal action"
                        .into(),
                );
            }
        } else if !applications.applications.is_empty()
            || !applications.startup.is_empty()
            || applications.terminal.is_some()
            || applications.launcher.is_some()
            || applications.firefox.is_some()
        {
            return Err("session application options require --session-mode=normal".into());
        } else if exit_when_startup_exits {
            return Err("--exit-when-startup-exits requires --session-mode=normal".into());
        } else if startup_ready_timeout.is_some() {
            return Err("--startup-ready-timeout-ms requires --session-mode=normal".into());
        }
        let max_runtime = arg_value(args, "--max-runtime-ms")
            .as_deref()
            .map(parse_u64)
            .transpose()?
            .map(Duration::from_millis);
        let max_ticks = arg_value(args, "--max-ticks")
            .as_deref()
            .map(parse_usize)
            .transpose()?;
        if max_ticks.is_some_and(|ticks| ticks == 0 || ticks > 1_000_000) {
            return Err("--max-ticks accepts a value from 1 through 1000000".into());
        }
        let inject_text = arg_value(args, "--inject-text");
        let expect_physical_text = arg_value(args, "--expect-physical-text");
        let physical_sequence_timeout = arg_value(args, "--physical-sequence-timeout-ms")
            .as_deref()
            .map(parse_u64)
            .transpose()?;
        if physical_sequence_timeout
            .is_some_and(|timeout| !(1_000..=600_000).contains(&timeout))
        {
            return Err(
                "--physical-sequence-timeout-ms accepts a value from 1000 through 600000"
                    .into(),
            );
        }
        if physical_sequence_timeout.is_some() && expect_physical_text.is_none() {
            return Err(
                "--physical-sequence-timeout-ms requires --expect-physical-text".into(),
            );
        }
        let terminal_exec = arg_value(args, "--terminal-exec");
        let terminal_exec_args = args
            .iter()
            .filter_map(|arg| arg.strip_prefix("--terminal-exec-arg="))
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let session_launcher = arg_value(args, "--session-launcher");
        let session_firefox = arg_value(args, "--session-firefox");
        if session_launcher
            .iter()
            .chain(session_firefox.iter())
            .any(|path| path.is_empty() || path.len() > 4_096)
        {
            return Err("approved session executable paths accept 1-4096 bytes".into());
        }
        let client = arg_value(args, "--client");
        let software_client_rendering = args.iter().any(|arg| arg == "--software-client-rendering");
        let client_args = args
            .iter()
            .filter_map(|arg| arg.strip_prefix("--client-arg="))
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let expect_client_stdout = arg_value(args, "--expect-client-stdout");
        let require_client_normal_exit =
            args.iter().any(|arg| arg == "--require-client-normal-exit");
        let proof_mode = args.iter().any(|arg| arg == "--proof");
        if software_client_rendering && client.is_none() {
            return Err("--software-client-rendering requires --client".into());
        }
        if client.is_none()
            && (!client_args.is_empty()
                || expect_client_stdout.is_some()
                || require_client_normal_exit)
        {
            return Err("client proof options require --client".into());
        }
        if client_args.len() > 64 || client_args.iter().any(|argument| argument.len() > 4_096) {
            return Err("--client accepts at most 64 bounded arguments".into());
        }
        if expect_client_stdout
            .as_ref()
            .is_some_and(|text| text.len() > 4_096)
        {
            return Err("--expect-client-stdout accepts at most 4096 bytes".into());
        }
        if terminal_exec.is_none() && !terminal_exec_args.is_empty() {
            return Err("--terminal-exec-arg requires --terminal-exec".into());
        }
        if terminal_exec_args.len() > 32
            || terminal_exec_args
                .iter()
                .any(|argument| argument.len() > 4_096)
        {
            return Err("--terminal-exec accepts at most 32 bounded arguments".into());
        }
        let expect_physical_pointer = args.iter().any(|arg| arg == "--expect-physical-pointer");
        let secondary_terminal = args.iter().any(|arg| arg == "--secondary-terminal");
        let exit_after_input_proof = args.iter().any(|arg| arg == "--exit-after-input-proof");
        let native_scanout = args.iter().any(|arg| arg == "--native-scanout");
        let namespace_profile_name = arg_value(args, "--namespace-profile")
            .unwrap_or_else(|| core_snapshot.namespace_profile.clone());
        let namespace_profile = match namespace_profile_name.as_str() {
            "classic" | "classic-shared" => NamespaceProfile::ClassicShared,
            "confined" => NamespaceProfile::Confined,
            profile => {
                return Err(format!(
                    "unsupported namespace profile {profile:?}; expected classic or confined"
                )
                .into());
            }
        };
        let mut effective_xkb = if desktop_input.inherit_sophia {
            core_snapshot.input.xkb.clone()
        } else {
            sophia_config::XkbConfig::default()
        };
        let mut key_repeat_config = if desktop_input.inherit_sophia {
            core_snapshot.input.repeat
        } else {
            sophia_config::RepeatConfig::default()
        };
        let mut initial_caps_lock = false;
        let mut initial_num_lock = false;
        if let Some(keyboard) = desktop_input.keyboard.as_ref() {
            if let Some(xkb) = keyboard.xkb.as_ref() {
                if let Some(value) = xkb.rules.as_ref() {
                    effective_xkb.rules.clone_from(value);
                }
                if let Some(value) = xkb.model.as_ref() {
                    effective_xkb.model.clone_from(value);
                }
                if let Some(value) = xkb.layout.as_ref() {
                    effective_xkb.layout.clone_from(value);
                }
                if let Some(value) = xkb.variant.as_ref() {
                    effective_xkb.variant.clone_from(value);
                }
                if let Some(value) = xkb.options.as_ref() {
                    effective_xkb.options.clone_from(value);
                }
            }
            if let Some(rate) = keyboard.repeat_rate {
                key_repeat_config.interval_msec = u64::from(1_000_u32.div_ceil(rate));
            }
            if let Some(delay) = keyboard.repeat_delay_msec {
                key_repeat_config.delay_msec = delay;
            }
            initial_caps_lock = keyboard.caps_lock.unwrap_or(false);
            initial_num_lock = keyboard.num_lock.unwrap_or(false);
        }
        let xkb_config = sophia_x_authority::XkbRmlvoConfig {
            rules: arg_value(args, "--xkb-rules").unwrap_or(effective_xkb.rules),
            model: arg_value(args, "--xkb-model").unwrap_or(effective_xkb.model),
            layout: arg_value(args, "--xkb-layout").unwrap_or(effective_xkb.layout),
            variant: arg_value(args, "--xkb-variant").unwrap_or(effective_xkb.variant),
            options: arg_value(args, "--xkb-options").unwrap_or(effective_xkb.options),
        };
        xkb_config.validate()?;
        let inject_output_size = arg_value(args, "--inject-output-size")
            .as_deref()
            .map(parse_output_size)
            .transpose()?;
        let inject_surface_resize = arg_value(args, "--inject-surface-resize")
            .as_deref()
            .map(parse_output_size)
            .transpose()?;
        let inject_surface_resize_sequence =
            arg_value(args, "--inject-surface-resize-sequence")
                .as_deref()
                .map(parse_surface_resize_sequence)
                .transpose()?
                .unwrap_or_default();
        if inject_surface_resize.is_some() && !inject_surface_resize_sequence.is_empty() {
            return Err(
                "--inject-surface-resize and --inject-surface-resize-sequence are mutually exclusive"
                    .into(),
            );
        }
        let m4_first_acquire_delay = arg_value(args, "--m4-first-acquire-delay-ms")
            .as_deref()
            .map(parse_u64)
            .transpose()?
            .map(Duration::from_millis);
        if m4_first_acquire_delay.is_some_and(|delay| delay.is_zero() || delay.as_millis() > 2_000)
        {
            return Err("--m4-first-acquire-delay-ms accepts 1-2000 milliseconds".into());
        }
        let m4_reject_first_present = args.iter().any(|arg| arg == "--m4-reject-first-present");
        let m4_diagnose_first_mixed_export = args
            .iter()
            .any(|arg| arg == "--m4-diagnose-first-mixed-export");
        let firefox_m8_proof = args.iter().any(|arg| arg == "--firefox-m8-proof");
        let firefox_m10_proof = args.iter().any(|arg| arg == "--firefox-m10-proof");
        let firefox_m10_rendering_proof = args
            .iter()
            .any(|arg| arg == "--firefox-m10-rendering-proof");
        let firefox_m10_dialog_proof = args
            .iter()
            .any(|arg| arg == "--firefox-m10-dialog-proof");
        let firefox_m10_primary_proof = args
            .iter()
            .any(|arg| arg == "--firefox-m10-primary-proof");
        let firefox_m10_selection_proof = args
            .iter()
            .any(|arg| arg == "--firefox-m10-selection-proof");
        let firefox_m10_lifecycle_proof = args
            .iter()
            .any(|arg| arg == "--firefox-m10-lifecycle-proof");
        let firefox_proof_count = [
            firefox_m8_proof,
            firefox_m10_proof,
            firefox_m10_rendering_proof,
            firefox_m10_dialog_proof,
            firefox_m10_primary_proof,
            firefox_m10_selection_proof,
            firefox_m10_lifecycle_proof,
        ]
        .into_iter()
        .filter(|selected| *selected)
        .count();
        if firefox_proof_count > 1 {
            return Err("select only one Firefox proof mode".into());
        }
        if firefox_proof_count == 1
            && (!normal_session || applications.firefox.is_none())
        {
            return Err(
                "Firefox proof mode requires normal session mode and a Firefox action mapping"
                    .into(),
            );
        }
        let configured_wm = core_snapshot.external_wm.as_ref();
        let explicit_wm_process = arg_value(args, "--wm-process");
        let wm_process = explicit_wm_process.clone().or_else(|| {
                configured_wm
                    .and_then(|wm| wm.executable.to_str())
                    .map(ToOwned::to_owned)
            });
        let mut wm_process_args = if explicit_wm_process.is_some() {
            Vec::new()
        } else {
            configured_wm
                .map(|wm| wm.arguments.clone())
                .unwrap_or_default()
        };
        wm_process_args.extend(args
            .iter()
            .filter_map(|arg| arg.strip_prefix("--wm-process-arg="))
            .map(ToOwned::to_owned)
        );
        if wm_process.is_none() && !wm_process_args.is_empty() {
            return Err("--wm-process-arg requires --wm-process".into());
        }
        let wm_interface = match arg_value(args, "--wm-interface").as_deref() {
            Some("api_v7") => sophia_config::ExternalWmInterface::ApiV7,
            Some("sophia_wm_v1") => sophia_config::ExternalWmInterface::SophiaWmV1,
            Some(other) => {
                return Err(format!(
                    "--wm-interface expects api_v7 or sophia_wm_v1, got {other:?}"
                )
                .into());
            }
            None => configured_wm.map_or(
                sophia_config::ExternalWmInterface::ApiV7,
                |wm| wm.interface,
            ),
        };
        if wm_process.is_none()
            && wm_interface != sophia_config::ExternalWmInterface::ApiV7
        {
            return Err("--wm-interface=sophia_wm_v1 requires --wm-process".into());
        }
        if normal_session && wm_interface == sophia_config::ExternalWmInterface::SophiaWmV1 {
            let shortcuts = sophia_config::prepare_desktop_shortcut_candidate(
                desktop_profile
                    .candidates
                    .get(&sophia_config::DesktopAuthority::Shortcut)
                    .expect("desktop profile partition creates a shortcut candidate"),
            )?;
            applications.validate_shortcuts(&shortcuts)?;
        }
        let wm_public_fault_after = arg_value(args, "--wm-proof-fault-after")
            .as_deref()
            .map(PublicPolicyFaultPoint::parse)
            .transpose()?;
        if wm_public_fault_after.is_some()
            && (wm_interface != sophia_config::ExternalWmInterface::SophiaWmV1
                || max_runtime.is_none())
        {
            return Err(
                "--wm-proof-fault-after requires --wm-interface=sophia_wm_v1 and --max-runtime-ms"
                    .into(),
            );
        }
        if native_scanout && std::env::var_os("SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE").is_none() {
            return Err(
                "set SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 to run persistent native scanout"
                    .into(),
            );
        }
        let input_devices_argument = arg_value(args, "--input-devices");
        let input_seat_argument = arg_value(args, "--input-seat");
        if input_devices_argument.is_some() && input_seat_argument.is_some() {
            return Err("--input-seat and --input-devices are mutually exclusive".into());
        }
        let input_devices = input_devices_argument
            .map(|paths| {
                paths
                    .split(',')
                    .map(std::path::PathBuf::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                if input_seat_argument.is_some() {
                    Vec::new()
                } else {
                    match &core_snapshot.input.source {
                        sophia_config::InputSourceConfig::Devices(devices) => devices.clone(),
                        sophia_config::InputSourceConfig::Seat(_) => Vec::new(),
                    }
                }
            });
        if input_devices.len() > 16
            || input_devices
                .iter()
                .any(|path| !path.is_absolute() || path.as_os_str().is_empty())
        {
            return Err("--input-devices accepts 1-16 comma-separated absolute paths".into());
        }
        let input_seat = input_seat_argument.or_else(|| {
            if input_devices.is_empty() {
                match &core_snapshot.input.source {
                    sophia_config::InputSourceConfig::Seat(seat) => Some(seat.clone()),
                    sophia_config::InputSourceConfig::Devices(_) => None,
                }
            } else {
                None
            }
        });
        if input_seat.as_ref().is_some_and(|seat| {
            seat.is_empty()
                || seat.len() > 64
                || !seat
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        }) {
            return Err("--input-seat accepts a 1-64 byte ASCII seat name".into());
        }
        if input_seat.is_some() && !input_devices.is_empty() {
            return Err("--input-seat and --input-devices are mutually exclusive".into());
        }
        if inject_text.is_some() && expect_physical_text.is_some() {
            return Err("--inject-text and --expect-physical-text are mutually exclusive".into());
        }
        if terminal_exec.is_some() && (inject_text.is_some() || expect_physical_text.is_some()) {
            return Err("--terminal-exec cannot be combined with input-proof commands".into());
        }
        if client.is_some()
            && (terminal_exec.is_some()
                || secondary_terminal
                || args.iter().any(|arg| arg.starts_with("--terminal=")))
        {
            return Err(
                "--client cannot be combined with terminal-specific session options".into(),
            );
        }
        if client.is_some() && inject_text.is_some() && !proof_mode {
            return Err("--client with --inject-text requires explicit --proof mode".into());
        }
        if client.is_some() && inject_text.is_some() && expect_client_stdout.is_none() {
            return Err("--client with --inject-text requires --expect-client-stdout".into());
        }
        if (m4_first_acquire_delay.is_some()
            || m4_reject_first_present
            || m4_diagnose_first_mixed_export)
            && (!native_scanout || terminal_exec.is_none())
        {
            return Err(
                "M4 Present proof controls require --native-scanout and --terminal-exec".into(),
            );
        }
        if (inject_text.is_some() || expect_physical_text.is_some())
            && max_runtime.is_none()
            && max_ticks.is_none()
        {
            return Err(
                "input proof flags require --max-runtime-ms or --max-ticks for a bounded proof"
                    .into(),
            );
        }
        if expect_physical_text.is_some() && input_devices.is_empty() && input_seat.is_none() {
            return Err("--expect-physical-text requires --input-seat or --input-devices".into());
        }
        if expect_physical_pointer && expect_physical_text.is_none() {
            return Err(
                "--expect-physical-pointer requires --expect-physical-text for visible content"
                    .into(),
            );
        }
        if exit_after_input_proof && inject_text.is_none() && expect_physical_text.is_none() {
            return Err("--exit-after-input-proof requires an input proof".into());
        }
        if let Some(text) = inject_text.as_ref().or(expect_physical_text.as_ref())
            && (text.is_empty()
                || text.len() > 24
                || !text.bytes().all(|byte| byte.is_ascii_lowercase()))
        {
            return Err("input proof text accepts 1-24 lowercase ASCII letters".into());
        }
        Ok(Self {
            display,
            socket_path: std::path::PathBuf::from(format!("/tmp/.X11-unix/X{display_number}")),
            terminal: arg_value(args, "--terminal").unwrap_or_else(|| "xterm".to_owned()),
            terminal_exec,
            terminal_exec_args,
            session_launcher,
            session_firefox,
            client,
            client_args,
            expect_client_stdout,
            require_client_normal_exit,
            secondary_terminal,
            max_runtime,
            normal_session,
            exit_when_startup_exits,
            startup_ready_timeout,
            applications,
            max_ticks,
            inject_text,
            expect_physical_text,
            physical_sequence_timeout_msec: physical_sequence_timeout
                .unwrap_or(SESSION_PHYSICAL_SEQUENCE_TIMEOUT_MSEC),
            expect_physical_pointer,
            exit_after_input_proof,
            input_devices,
            input_seat,
            native_scanout,
            software_client_rendering,
            wm_process,
            wm_process_args,
            wm_interface,
            wm_public_fault_after,
            wm_socket_path: std::env::temp_dir().join(format!(
                "sophia-live-wm-{}-{display_number}.sock",
                std::process::id()
            )),
            input_quiet_msec: SESSION_INPUT_QUIET_MSEC,
            namespace_profile,

            namespace_capabilities: NamespaceCapabilities::NONE,
            xkb_config,
            key_repeat_config,
            initial_caps_lock,
            initial_num_lock,
            pointer_candidate: desktop_input.pointer,
            desktop_profile,
            surface_chrome_style: Self::surface_chrome_style(core_snapshot.fallback_chrome),
            verbose_diagnostics: core_snapshot.verbose_diagnostics,
            core_config_source,
            core_config_state,
            inject_output_size,
            inject_surface_resize,
            inject_surface_resize_sequence,
            m4_first_acquire_delay,
            m4_reject_first_present,
            m4_diagnose_first_mixed_export,
            firefox_m8_proof,
            firefox_m10_proof,
            firefox_m10_rendering_proof,
            firefox_m10_dialog_proof,
            firefox_m10_primary_proof,
            firefox_m10_selection_proof,
            firefox_m10_lifecycle_proof,
        })
    }

    fn applications_from_core(
        snapshot: &sophia_config::CoreConfigSnapshot,
    ) -> Result<SessionApplicationConfig, Box<dyn std::error::Error>> {
        let mut applications = SessionApplicationConfig::default();
        let mut names_by_id = BTreeMap::new();
        for app in &snapshot.session.applications {
            names_by_id.insert(app.id, app.name.clone());
            applications.applications.insert(
                app.name.clone(),
                SessionApplicationSpec {
                    id: app.name.clone(),
                    executable: app.executable.clone(),
                    arguments: app.arguments.clone(),
                },
            );
            match app.id {
                1 => applications.terminal = Some(app.name.clone()),
                2 => applications.launcher = Some(app.name.clone()),
                3 => applications.firefox = Some(app.name.clone()),
                _ => {}
            }
        }
        for id in &snapshot.session.startup {
            applications.startup.push(
                names_by_id
                    .get(id)
                    .ok_or_else(|| format!("core config startup references unknown app {id}"))?
                    .clone(),
            );
        }
        Ok(applications)
    }

    fn validate_session_app_id(id: &str) -> Result<(), Box<dyn std::error::Error>> {
        if id.is_empty()
            || id.len() > 32
            || !id.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
            })
        {
            return Err(
                "session application IDs accept 1-32 lowercase ASCII letters, digits, '-' or '_'"
                    .into(),
            );
        }
        Ok(())
    }

    fn input_proof_requested(&self) -> bool {
        self.inject_text.is_some() || self.expect_physical_text.is_some()
    }

    fn surface_resize_requested(&self) -> bool {
        self.inject_surface_resize.is_some() || !self.inject_surface_resize_sequence.is_empty()
    }

    fn surface_resize_targets(&self) -> Vec<Size> {
        self.inject_surface_resize
            .iter()
            .copied()
            .chain(self.inject_surface_resize_sequence.iter().copied())
            .collect()
    }

    fn application_proof_requested(&self) -> bool {
        self.client.is_some()
    }

    fn application_for_action(&self, action: WmSessionAction) -> Option<&SessionApplicationSpec> {
        let id = match action {
            WmSessionAction::LaunchApplication { application }
                if application == TERMINAL_APPLICATION_ID =>
            {
                self.applications.terminal.as_ref()
            }
            WmSessionAction::LaunchApplication { application }
                if application == LAUNCHER_APPLICATION_ID =>
            {
                self.applications.launcher.as_ref()
            }
            WmSessionAction::LaunchApplication { application }
                if application == BROWSER_APPLICATION_ID =>
            {
                self.applications.firefox.as_ref()
            }
            WmSessionAction::LaunchApplication { .. } => None,
            WmSessionAction::CloseFocused | WmSessionAction::Logout => None,
        }?;
        self.applications.applications.get(id)
    }
    fn spawn_session_application(
        app: &SessionApplicationSpec,
        display: &str,
        xauthority: &std::path::Path,
    ) -> Result<Child, Box<dyn std::error::Error>> {
        let mut command = std::process::Command::new(&app.executable);
        command
            .args(&app.arguments)
            .env("DISPLAY", display)
            .env("XAUTHORITY", xauthority)
            .env_remove("ENV")
            .env_remove("BASH_ENV")
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        Ok(command.spawn()?)
    }

    fn firefox_proof_requested(&self) -> bool {
        self.firefox_m8_proof
            || self.firefox_m10_proof
            || self.firefox_m10_rendering_proof
            || self.firefox_m10_dialog_proof
            || self.firefox_m10_primary_proof
            || self.firefox_m10_selection_proof
            || self.firefox_m10_lifecycle_proof
    }

    fn firefox_full_proof_requested(&self) -> bool {
        self.firefox_m8_proof || self.firefox_m10_proof
    }
}

#[derive(Default)]
struct FirefoxM10KittyProof {
    observed: [bool; Self::CHECKPOINTS.len()],
}

#[derive(Default)]
struct FirefoxM10SelectionKittyProof {
    observed: [bool; Self::CHECKPOINTS.len()],
}

#[derive(Default)]
struct FirefoxM10DialogProof {
    completed: usize,
}

#[derive(Default)]
struct FirefoxM10PrimaryProof {
    completed: usize,
}

impl FirefoxM10PrimaryProof {
    // Page initialization can be coalesced before the metadata observer sees
    // it. A trusted full-field selection is the first causal proof boundary.
    const CHECKPOINTS: [(usize, &'static str); 3] = [
        (251, "source_armed"),
        (253, "kitty_received"),
        (252, "confirmed"),
    ];

    fn observe(&mut self, property_name: &str, byte_len: usize) -> Option<&'static str> {
        if property_name != "_NET_WM_NAME" {
            return None;
        }
        let (expected, checkpoint) = Self::CHECKPOINTS.get(self.completed)?;
        if byte_len != *expected {
            return None;
        }
        self.completed += 1;
        Some(*checkpoint)
    }

    fn complete(&self) -> bool {
        self.completed == Self::CHECKPOINTS.len()
    }
}

impl FirefoxM10DialogProof {
    const CHECKPOINTS: [(usize, &'static str); 3] = [
        (245, "page_ready"),
        (246, "modal_ready"),
        (247, "confirmed"),
    ];

    fn observe(&mut self, property_name: &str, byte_len: usize) -> Option<&'static str> {
        if property_name != "_NET_WM_NAME" {
            return None;
        }
        let (expected, checkpoint) = Self::CHECKPOINTS.get(self.completed)?;
        if byte_len != *expected {
            return None;
        }
        self.completed += 1;
        Some(*checkpoint)
    }

    fn complete(&self) -> bool {
        self.completed == Self::CHECKPOINTS.len()
    }
}

impl FirefoxM10SelectionKittyProof {
    const CHECKPOINTS: [(usize, &'static str); 3] = [
        (241, "before"),
        (242, "clipboard_peer"),
        (243, "primary_peer"),
    ];

    fn observe(&mut self, property_name: &str, byte_len: usize) -> Option<&'static str> {
        if property_name != "_NET_WM_NAME" {
            return None;
        }
        let (index, (_, checkpoint)) = Self::CHECKPOINTS
            .iter()
            .enumerate()
            .find(|(_, (expected, _))| *expected == byte_len)?;
        if self.observed[index] {
            return None;
        }
        self.observed[index] = true;
        Some(*checkpoint)
    }

    fn complete(&self) -> bool {
        self.observed.iter().all(|observed| *observed)
    }

    fn completed(&self) -> usize {
        self.observed.iter().filter(|observed| **observed).count()
    }
}

impl FirefoxM10KittyProof {
    const CHECKPOINTS: [(usize, &'static str, &'static str); 6] = [
        (193, "a", "before"),
        (194, "b", "before"),
        (211, "a", "after_normal_close"),
        (212, "b", "after_normal_close"),
        (229, "a", "after_forced_close"),
        (230, "b", "after_forced_close"),
    ];

    fn observe(
        &mut self,
        property_name: &str,
        byte_len: usize,
    ) -> Option<(&'static str, &'static str)> {
        if property_name != "_NET_WM_NAME" {
            return None;
        }
        let (index, (_, terminal, checkpoint)) = Self::CHECKPOINTS
            .iter()
            .enumerate()
            .find(|(_, (expected, _, _))| *expected == byte_len)?;
        if self.observed[index] {
            return None;
        }
        self.observed[index] = true;
        Some((*terminal, *checkpoint))
    }

    fn complete(&self) -> bool {
        self.observed.iter().all(|observed| *observed)
    }

    fn lifecycle_complete(&self) -> bool {
        self.complete()
    }

    fn completed(&self) -> usize {
        self.observed.iter().filter(|observed| **observed).count()
    }
}
