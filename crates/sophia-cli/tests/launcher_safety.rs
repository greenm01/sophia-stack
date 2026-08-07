const SESSION_LAUNCHER: &str = include_str!("../../../tools/run_sophia_xmonad_session.sh");
const TTY3_LAUNCHER: &str = include_str!("../../../tools/start_sophia_tty3.sh");
const INSTALLED_SESSION: &str = include_str!("../../../tools/installed/sophia-session");
const INSTALLED_RECOVERY: &str = include_str!("../../../tools/installed/sophia-recovery-proof");
const INSTALLED_TRUECOLOR: &str = include_str!("../../../tools/installed/sophia-truecolor-proof");
const INSTALLER: &str = include_str!("../../../tools/install_live_session.sh");
const TTY_MODE_HELPER: &str = include_str!("../../../tools/sophia_tty_mode.py");

fn offset(needle: &str) -> usize {
    SESSION_LAUNCHER
        .find(needle)
        .unwrap_or_else(|| panic!("launcher is missing {needle:?}"))
}

#[test]
fn graphical_takeover_disables_console_rendering_and_input_echo_after_guard_arming() {
    let guard_ready = offset("echo \"Emergency input guard armed.\"");
    let graphics = offset("python3 \"$TTY_MODE_HELPER\" graphics");
    let keyboard_off = offset("python3 \"$TTY_MODE_HELPER\" keyboard-off");
    let raw = offset("stty raw -echo");
    let session = offset("setsid \"${session_command[@]}\"");

    assert!(guard_ready < graphics);
    assert!(graphics < keyboard_off);
    assert!(keyboard_off < raw);
    assert!(raw < session);
}

#[test]
fn graphical_takeover_saves_and_restores_exact_tty_state() {
    let save_termios = offset("tty_state=\"$(stty -g)\"");
    let save_kd = offset("kd_mode=\"$(python3 \"$TTY_MODE_HELPER\" get)\"");
    let save_keyboard = offset("keyboard_mode=\"$(python3 \"$TTY_MODE_HELPER\" get-keyboard)\"");
    let graphics = offset("python3 \"$TTY_MODE_HELPER\" graphics");

    assert!(save_termios < graphics);
    assert!(save_kd < graphics);
    assert!(save_keyboard < graphics);
    assert!(SESSION_LAUNCHER.contains("python3 \"$TTY_MODE_HELPER\" \"$kd_mode\""));
    assert!(SESSION_LAUNCHER.contains("stty \"$tty_state\""));
    assert!(SESSION_LAUNCHER.contains("python3 \"$TTY_MODE_HELPER\" \"keyboard-$keyboard_mode\""));
}

#[test]
fn detached_graphical_owner_does_not_attempt_direct_vt_activation() {
    assert!(!SESSION_LAUNCHER.contains("SOPHIA_SESSION_TTY_FD"));
    assert!(!TTY_MODE_HELPER.contains("VT_ACTIVATE"));
    assert!(!TTY_MODE_HELPER.contains("activate-vt-"));
}

#[test]
fn kitty_gate_always_retains_one_shot_composition_pixel_evidence() {
    assert!(SESSION_LAUNCHER.contains("SOPHIA_NATIVE_COMPOSITION_PIXEL_TRACE=1"));
    assert!(!SESSION_LAUNCHER.contains("SOPHIA_NATIVE_COMPOSITION_PIXEL_TRACE=continuous"));
    assert!(SESSION_LAUNCHER.contains("SOPHIA_SESSION_VERBOSE_TRACE:-false"));
}

#[test]
fn truecolor_wrapper_alone_enables_repeated_final_region_readback() {
    assert!(INSTALLED_TRUECOLOR.contains("SOPHIA_INSTALLED_ATTEMPT_MODE=truecolor"));
    assert!(INSTALLED_TRUECOLOR.contains("SOPHIA_TRUECOLOR_PROOF=true"));
    assert!(INSTALLED_TRUECOLOR.contains("SOPHIA_NATIVE_COMPOSITION_PIXEL_TRACE=final-regions"));
    assert!(!SESSION_LAUNCHER.contains("SOPHIA_NATIVE_COMPOSITION_PIXEL_TRACE=final-regions"));
    assert!(!SESSION_LAUNCHER.contains("SOPHIA_NATIVE_COMPOSITION_PIXEL_TRACE=continuous"));
}

#[test]
fn firefox_m10_gate_uses_the_proven_isolated_native_x_configuration() {
    assert!(SESSION_LAUNCHER.contains("firefox_m10_profile_dir=\"\""));
    assert!(SESSION_LAUNCHER.contains("$firefox_m10_profile_dir/user.js"));
    assert!(SESSION_LAUNCHER.contains("browser.tabs.remote.autostart"));
    assert!(SESSION_LAUNCHER.contains("fission.autostart"));
    assert!(SESSION_LAUNCHER.contains("--session-app-arg=firefox=--profile"));
    assert!(SESSION_LAUNCHER.contains("--session-app-arg=firefox=$firefox_m10_profile_dir"));
    assert!(SESSION_LAUNCHER.contains("GDK_BACKEND=x11"));
    assert!(SESSION_LAUNCHER.contains("MOZ_ENABLE_WAYLAND=0"));
    assert!(SESSION_LAUNCHER.contains("MOZ_FORCE_DISABLE_E10S=1"));
    assert!(SESSION_LAUNCHER.contains("MOZ_USE_XINPUT2=1"));
}

#[test]
fn firefox_m10_profiles_are_bounded_by_the_session_lifecycle() {
    let prior_wrapper_check = offset("if [[ -s \"$PID_FILE\" ]]");
    let graphical_session_check = offset("if (( ${#active_sessions[@]} > 0 )); then");
    let child_shutdown = offset(
        "[[ -z \"$session_pid\" ]] || terminate_bounded \"-$session_pid\" \"$SESSION_LABEL\"",
    );
    let current_cleanup = offset("if [[ -n \"$firefox_m10_probe_dir\" ]]; then");
    let trap = offset("trap cleanup EXIT");
    let stale_cleanup =
        offset("find \"$STATE_DIR\" -mindepth 1 -maxdepth 1 -type d -name 'firefox-m10.*'");
    let profile_create = offset("firefox_m10_probe_dir=\"$(mktemp -d");

    assert!(prior_wrapper_check < graphical_session_check);
    assert!(graphical_session_check < stale_cleanup);
    assert!(child_shutdown < current_cleanup);
    assert!(trap < stale_cleanup);
    assert!(stale_cleanup < profile_create);
    assert!(SESSION_LAUNCHER.contains("rm -rf -- \"$firefox_m10_probe_dir\""));
    assert!(SESSION_LAUNCHER.contains("firefox_m10_probe_dir=\"\""));
}

#[test]
fn tty3_gate_reactivates_its_originating_vt_after_display_manager_restore() {
    let restore_manager = TTY3_LAUNCHER
        .find("sudo sv up \"$display_manager\"")
        .unwrap();
    let reactivate_tty = TTY3_LAUNCHER.find("sudo chvt \"$origin_vt\"").unwrap();

    assert!(TTY3_LAUNCHER.contains("origin_tty=\"$(tty)\""));
    assert!(TTY3_LAUNCHER.contains("origin_vt=\"${origin_tty#/dev/tty}\""));
    assert!(restore_manager < reactivate_tty);
    assert!(TTY3_LAUNCHER.contains("active_vt=\"$(fgconsole 2>/dev/null || true)\""));
}

#[test]
fn installed_session_uses_only_versioned_release_artifacts() {
    assert!(INSTALLED_SESSION.contains("SOPHIA_BUILD_SESSION=false"));
    assert!(INSTALLED_SESSION.contains("SOPHIA_MANAGE_KEYD=false"));
    assert!(INSTALLED_SESSION.contains("$RELEASE_DIR/target/release/sophia"));
    assert!(!INSTALLED_SESSION.contains("cargo "));
    assert!(!INSTALLED_SESSION.contains("sudo "));
}

#[test]
fn installed_watchdog_is_fixed_and_opt_in() {
    assert!(INSTALLED_RECOVERY.contains("SOPHIA_SESSION_WATCHDOG_SECONDS=45"));
    assert!(INSTALLED_RECOVERY.contains("$RELEASE_DIR/bin/sophia-session"));
    assert!(!INSTALLED_SESSION.contains("export SOPHIA_SESSION_WATCHDOG_SECONDS="));
}

#[test]
fn installer_preserves_a_rollback_pointer_before_activation() {
    let preserve = INSTALLER
        .find("ln -sfn \"$old_current\" \"$PREFIX/previous\"")
        .unwrap();
    let activate = INSTALLER
        .find("ln -sfn \"releases/$release_id\" \"$PREFIX/current\"")
        .unwrap();
    assert!(preserve < activate);
    assert!(INSTALLER.contains("sha256sum -c SHA256SUMS"));
}
