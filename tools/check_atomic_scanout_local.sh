#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$ROOT_DIR"

cargo fmt --check
tools/audit_source_layout.sh
tools/check_github_language_visibility.sh
cargo check --offline -p sophia-cli --features atomic-scanout-smoke-live --quiet
cargo test --offline -p sophia-cli --features atomic-scanout-smoke-live --test backend_evidence --quiet
cargo test --offline -p sophia-cli --features atomic-scanout-smoke-live --test input_proof --quiet
cargo test --offline -p sophia-renderer-native-egl --features gbm-platform --quiet
cargo test --offline -p sophia-renderer-live --features "gbm-probe egl-probe" --quiet
cargo test --offline -p sophia-backend-live --features "libdrm-events libinput-events gbm-probe egl-probe" --quiet
bash -n tools/qemu_xmonad_m7_acceptance.sh
bash -n tools/check_qemu_xmonad_m7_verifier.sh
tools/check_qemu_xmonad_m7_verifier.sh
bash -n tools/qemu_xmonad_launch_burst_acceptance.sh \
    tools/verify_qemu_xmonad_launch_burst_evidence.sh \
    tools/check_qemu_xmonad_launch_burst_verifier.sh
tools/check_qemu_xmonad_launch_burst_verifier.sh
bash -n tools/fixtures/qemu_resize_storm_client.sh \
    tools/qemu_xmonad_resize_storm_acceptance.sh \
    tools/verify_qemu_xmonad_resize_storm_evidence.sh \
    tools/check_qemu_xmonad_resize_storm_verifier.sh
tools/check_qemu_xmonad_resize_storm_verifier.sh
bash -n tools/fixtures/qemu_idle_glxgears_client.sh \
    tools/qemu_xmonad_idle_efficiency_acceptance.sh \
    tools/verify_qemu_xmonad_idle_efficiency_evidence.sh \
    tools/check_qemu_xmonad_idle_efficiency_verifier.sh
tools/check_qemu_xmonad_idle_efficiency_verifier.sh
bash -n tools/qemu_xmonad_producer_overload_acceptance.sh \
    tools/verify_qemu_xmonad_producer_overload_evidence.sh \
    tools/check_qemu_xmonad_producer_overload_verifier.sh
tools/check_qemu_xmonad_producer_overload_verifier.sh
bash -n tools/qemu_xmonad_render_contention_acceptance.sh \
    tools/verify_qemu_xmonad_render_contention_evidence.sh \
    tools/check_qemu_xmonad_render_contention_verifier.sh
tools/check_qemu_xmonad_render_contention_verifier.sh
bash -n tools/qemu_xmonad_stale_response_acceptance.sh \
    tools/verify_qemu_xmonad_stale_response_evidence.sh \
    tools/check_qemu_xmonad_stale_response_verifier.sh
tools/check_qemu_xmonad_stale_response_verifier.sh
bash -n tools/qemu_xmonad_interactive.sh \
    tools/reduce_qemu_interactive_trace.sh \
    tools/verify_qemu_xmonad_interactive_evidence.sh \
    tools/check_qemu_xmonad_interactive_verifier.sh
python3 tools/check_qemu_rfb_input.py
tools/check_qemu_xmonad_interactive_verifier.sh
bash -n tools/qemu_xmonad_m8_mix_acceptance.sh
bash -n tools/qemu_xmonad_m8_soak_acceptance.sh
bash -n tools/verify_qemu_xmonad_m8_mix_evidence.sh
bash -n tools/verify_qemu_xmonad_m8_soak_evidence.sh
tools/check_qemu_xmonad_m8_verifiers.sh
tools/check_atomic_scanout_verifiers.sh
bash -n tools/audit_xcentric_runtime.sh
tools/audit_xcentric_runtime.sh
bash -n tools/atomic_scanout_preflight.sh
bash -n tools/atomic_scanout_smoke.sh
bash -n tools/runtime_rendered_scanout_evidence.sh
bash -n tools/atomic_scanout_hardware_proof.sh
bash -n tools/atomic_scanout_hardware_status.sh
bash -n tools/operator_keyboard_hardware_proof.sh
bash -n tools/finish_milestones_1_2.sh
bash -n tools/run_sophia_xmonad_session.sh
bash -n tools/resolve_sophia_xmobar.sh
bash -n tools/start_sophia_tty3.sh tools/start_sophia_kitty_tty3.sh \
    tools/start_sophia_native_hot_reload_tty3.sh tools/start_sophia_xmonad_tty3.sh
bash -n tools/config/proof_helpers.sh tools/config/write_core_chrome_config.sh
bash -n tools/verify_sophia_native_chrome.sh \
    tools/check_sophia_native_chrome_verifier.sh
tools/check_sophia_native_chrome_verifier.sh
grep -Fq 'SOPHIA_NATIVE_CHROME_STAGE_DWELL_SECONDS:-3' \
    tools/start_sophia_native_hot_reload_tty3.sh
[[ "$(grep -Fc 'sleep "$stage_dwell_seconds"' tools/start_sophia_native_hot_reload_tty3.sh)" == 5 ]]
bash -n tools/start_sophia_xmonad_config_reload_tty3.sh \
    tools/verify_sophia_xmonad_config_reload.sh \
    tools/check_sophia_xmonad_config_reload_verifier.sh
tools/check_sophia_xmonad_config_reload_verifier.sh
bash -n tools/start_sophia_xmonad_keyboard_vt_tty3.sh \
    tools/verify_sophia_xmonad_keyboard_vt.sh \
    tools/check_sophia_xmonad_keyboard_vt_verifier.sh
tools/check_sophia_xmonad_keyboard_vt_verifier.sh
bash -n tools/run_sophia_m9_semantic_gate.sh \
    tools/verify_sophia_m9_semantic_gate.sh \
    tools/check_sophia_m9_semantic_verifier.sh \
    tools/sophia_m9_promotion.sh \
    tools/check_sophia_m9_promotion.sh
tools/check_sophia_m9_semantic_verifier.sh
tools/check_sophia_m9_promotion.sh
bash -n tools/start_sophia_xmonad_clipboard_tty3.sh
bash -n tools/start_sophia_xmonad_input_proof_tty3.sh
bash -n tools/start_sophia_xmonad_emergency_tty3.sh
bash -n tools/start_sophia_xmonad_hardware_smoke_tty3.sh \
    tools/verify_sophia_xmonad_hardware_smoke.sh \
    tools/check_sophia_xmonad_hardware_smoke_verifier.sh
tools/check_sophia_xmonad_hardware_smoke_verifier.sh
bash -n tools/start_sophia_xmonad_xmobar_tty3.sh \
    tools/verify_sophia_xmonad_xmobar_hardware_smoke.sh \
    tools/check_sophia_xmonad_xmobar_hardware_smoke_verifier.sh
tools/check_sophia_xmonad_xmobar_hardware_smoke_verifier.sh
bash -n tools/start_sophia_xmonad_vkcube_recovery_tty3.sh \
    tools/verify_sophia_xmonad_vkcube_recovery.sh \
    tools/check_sophia_xmonad_vkcube_recovery_verifier.sh
tools/check_sophia_xmonad_vkcube_recovery_verifier.sh
bash -n tools/start_sophia_vkcube_standalone_tty3.sh \
    tools/benchmark_sophia_vkcube_tty3.sh \
    tools/benchmark_sophia_glxgears_tty3.sh \
    tools/benchmark_xserver_graphics.sh \
    tools/probes/run_bounded_glxgears.sh \
    tools/report_sophia_glxgears_performance.sh \
    tools/report_sophia_rendering_performance.sh \
    tools/report_xserver_rendering_performance.sh \
    tools/compare_sophia_xserver_rendering.sh \
    tools/verify_sophia_standalone_vkcube.sh \
    tools/check_sophia_standalone_vkcube_verifier.sh \
    tools/check_sophia_glxgears_performance_reporter.sh \
    tools/check_sophia_rendering_performance_reporter.sh \
    tools/check_xserver_rendering_performance_reporter.sh
bash -n tools/run_sophia_input_latency_tty3.sh \
    tools/report_sophia_input_latency.sh \
    tools/check_sophia_input_latency_reporter.sh \
    tools/setup_sophia_uinput.sh
tools/run_sophia_input_latency_tty3.sh --self-test
tools/check_sophia_input_latency_reporter.sh
tools/check_sophia_standalone_vkcube_verifier.sh
tools/check_sophia_glxgears_performance_reporter.sh
tools/check_sophia_rendering_performance_reporter.sh
tools/check_xserver_rendering_performance_reporter.sh
bash -n tools/verify_sophia_xmonad_xmobar.sh
bash -n tools/check_sophia_xmonad_xmobar_verifier.sh
tools/check_sophia_xmonad_xmobar_verifier.sh
bash -n tools/start_sophia_xmonad_four_kitty_tty3.sh
bash -n tools/verify_sophia_xmonad_four_kitty.sh
bash -n tools/check_sophia_xmonad_four_kitty_verifier.sh
tools/check_sophia_xmonad_four_kitty_verifier.sh
bash -n tools/install_and_run_sophia_xmonad_input_proof_tty3.sh
bash -n tools/verify_sophia_xmonad_input_proof_tty3.sh
bash -n tools/check_sophia_xmonad_input_proof_verifier.sh
tools/check_sophia_xmonad_input_proof_verifier.sh
bash -n tools/verify_sophia_xmonad_pointer_focus.sh
bash -n tools/check_sophia_xmonad_pointer_focus_verifier.sh
tools/check_sophia_xmonad_pointer_focus_verifier.sh
bash -n tools/start_sophia_xmonad_pointer_focus_tty3.sh
bash -n tools/verify_sophia_xmonad_pointer_focus_pair.sh
bash -n tools/check_sophia_xmonad_pointer_focus_pair_verifier.sh
tools/check_sophia_xmonad_pointer_focus_pair_verifier.sh
bash -n tools/start_sophia_xmonad_pointer_edges_tty3.sh
bash -n tools/verify_sophia_xmonad_pointer_edges.sh
bash -n tools/check_sophia_xmonad_pointer_edges_verifier.sh
tools/check_sophia_xmonad_pointer_edges_verifier.sh
bash -n tools/verify_sophia_xmonad_focused_border.sh
bash -n tools/check_sophia_xmonad_focused_border_verifier.sh
tools/check_sophia_xmonad_focused_border_verifier.sh
bash -n tools/start_sophia_xmonad_firefox_proof_tty3.sh
bash -n tools/start_sophia_xmonad_firefox_rendering_tty3.sh \
    tools/start_sophia_xmonad_firefox_dialog_tty3.sh \
    tools/start_sophia_xmonad_firefox_primary_tty3.sh \
    tools/start_sophia_xmonad_firefox_selection_tty3.sh \
    tools/start_sophia_xmonad_firefox_lifecycle_tty3.sh
bash -n tools/fixtures/firefox_m10_kitty_probe.sh \
    tools/fixtures/firefox_m10_primary_kitty_probe.sh \
    tools/fixtures/firefox_m10_selection_kitty_probe.sh
bash -n tools/check_firefox_m10_kitty_probe.sh
tools/check_firefox_m10_kitty_probe.sh
bash -n tools/check_firefox_m10_selection_kitty_probe.sh
tools/check_firefox_m10_selection_kitty_probe.sh
bash -n tools/check_firefox_m10_primary_kitty_probe.sh
tools/check_firefox_m10_primary_kitty_probe.sh
bash -n tools/check_firefox_m10_selection_page.sh
tools/check_firefox_m10_selection_page.sh
bash -n tools/check_firefox_m10_promotion_page.sh
tools/check_firefox_m10_promotion_page.sh
bash -n tools/check_firefox_m10_primary_page.sh
tools/check_firefox_m10_primary_page.sh
bash -n tools/check_firefox_m10_rendering_page.sh
tools/check_firefox_m10_rendering_page.sh
bash -n tools/check_firefox_m10_dialog_page.sh
tools/check_firefox_m10_dialog_page.sh
bash -n tools/run_sophia_kitty_session.sh tools/stop_sophia_kitty_session.sh \
    tools/stop_sophia_native_session.sh tools/stop_sophia_standalone_session.sh \
    tools/stop_sophia_session.sh
bash -n tools/verify_sophia_xmonad_tty3.sh
bash -n tools/check_sophia_xmonad_tty3_verifier.sh
bash -n tools/verify_sophia_xmonad_emergency_tty3.sh
bash -n tools/check_sophia_xmonad_emergency_verifier.sh
bash -n tools/verify_installed_emergency_archive.sh
bash -n tools/verify_installed_session_lifecycle.sh
bash -n tools/check_installed_session_lifecycle_verifier.sh
bash -n tools/verify_installed_login_cycle.sh \
    tools/check_installed_login_cycle_verifier.sh \
    tools/verify_installed_xterm_session.sh \
    tools/verify_installed_xterm_runs.sh \
    tools/check_installed_xterm_verifier.sh \
    tools/verify_installed_truecolor_session.sh \
    tools/verify_installed_truecolor_runs.sh \
    tools/check_truecolor_verifier.sh \
    tools/verify_sophia_xmobar_work_area_session.sh \
    tools/verify_installed_xmobar_work_area_archive.sh \
    tools/check_xmobar_work_area_verifier.sh \
    tools/verify_installed_fallback_session.sh \
    tools/check_installed_fallback_verifier.sh \
    tools/verify_installed_native_chrome_session.sh \
    tools/verify_installed_native_chrome_archive.sh \
    tools/check_installed_native_chrome_verifier.sh \
    tools/check_installed_cycle_ledger.sh \
    tools/check_installed_session_auto_record.sh
tools/check_installed_login_cycle_verifier.sh
tools/check_installed_xterm_verifier.sh
tools/check_truecolor_verifier.sh
tools/check_xmobar_work_area_verifier.sh
tools/check_installed_fallback_verifier.sh
tools/check_installed_native_chrome_verifier.sh
tools/check_installed_cycle_ledger.sh
tools/check_installed_session_auto_record.sh
bash -n tools/check_sophia_session_watchdog.sh
bash -n tools/verify_installed_watchdog_recovery.sh
bash -n tools/check_installed_watchdog_recovery.sh
bash -n tools/verify_installed_watchdog_archive.sh
bash -n tools/verify_sophia_firefox_physical.sh
bash -n tools/record_sophia_firefox_physical_run.sh
bash -n tools/verify_sophia_firefox_physical_runs.sh
bash -n tools/check_sophia_firefox_physical_verifier.sh
tools/check_sophia_firefox_physical_verifier.sh
bash -n tools/verify_sophia_firefox_selection_physical.sh \
    tools/verify_sophia_firefox_primary_physical.sh \
    tools/verify_sophia_firefox_rendering_physical.sh \
    tools/verify_sophia_firefox_dialog_physical.sh \
    tools/check_sophia_firefox_rendering_verifier.sh \
    tools/check_sophia_firefox_dialog_verifier.sh \
    tools/check_sophia_firefox_primary_verifier.sh \
    tools/check_sophia_firefox_selection_verifier.sh \
    tools/verify_sophia_firefox_lifecycle_physical.sh \
    tools/check_sophia_firefox_lifecycle_verifier.sh
tools/check_sophia_firefox_selection_verifier.sh
tools/check_sophia_firefox_primary_verifier.sh
tools/check_sophia_firefox_rendering_verifier.sh
tools/check_sophia_firefox_dialog_verifier.sh
tools/check_sophia_firefox_lifecycle_verifier.sh
tools/check_sophia_xmonad_tty3_verifier.sh
tools/check_sophia_xmonad_emergency_verifier.sh
tools/check_installed_session_lifecycle_verifier.sh
tools/check_sophia_session_watchdog.sh
tools/check_installed_watchdog_recovery.sh
grep -Fq 'sophia-session-input-guard' tools/run_sophia_xmonad_session.sh
grep -Fq 'python3 "$TTY_MODE_HELPER" "$kd_mode"' tools/run_sophia_xmonad_session.sh
grep -Fq -- '--session-start=terminal' tools/lib/session_terminal.sh
grep -Fq -- '--session-start=statusbar' tools/run_sophia_xmonad_session.sh
grep -Fq 'SOPHIA_TTY_PROFILE=kitty' tools/run_sophia_kitty_session.sh
grep -Fq 'DBUS_SESSION_BUS_ADDRESS=unix:path=/dev/null' tools/run_sophia_xmonad_session.sh
bash -n tools/lib/session_terminal.sh tools/check_session_terminal_arguments.sh
tools/check_session_terminal_arguments.sh
grep -Fq -- '--session-app-arg=terminal=NONE' tools/lib/session_terminal.sh
grep -Fq 'linux_display_server=x11' tools/lib/session_terminal.sh
grep -Fq 'background_opacity=1' tools/lib/session_terminal.sh
grep -Fq 'SOPHIA_LIVE_SESSION_DIAGNOSTIC=1' tools/run_sophia_xmonad_session.sh
grep -Fq -- '--startup-ready-timeout-ms=8000' tools/run_sophia_xmonad_session.sh
grep -Fq -- '--session-app=standalone=' tools/run_sophia_xmonad_session.sh
grep -Fq 'layout "natural"' tools/fixtures/standalone_sophia_wm.kdl
grep -Fq 'behavior="logout"' tools/fixtures/standalone_sophia_wm.kdl
grep -Fq 'install -m 600 "$standalone_wm_template" "$standalone_wm_config"' \
    tools/run_sophia_xmonad_session.sh
grep -Fq 'standalone) tools/run_sophia_xmonad_session.sh' tools/start_sophia_tty3.sh
grep -Fq 'tools/start_sophia_tty3.sh' tools/start_sophia_kitty_tty3.sh
grep -Fq 'tools/start_sophia_tty3.sh' tools/start_sophia_xmonad_tty3.sh
grep -Fq 'native) tools/run_sophia_xmonad_session.sh' tools/start_sophia_tty3.sh
grep -Fq "status=ready ' 180" tools/start_sophia_native_hot_reload_tty3.sh
grep -Fq -- '--session-app=terminal-secondary=' tools/start_sophia_native_hot_reload_tty3.sh
grep -Fq -- '--session-start=terminal-secondary' tools/start_sophia_native_hot_reload_tty3.sh
if grep -Fq -- '--secondary-terminal' tools/start_sophia_native_hot_reload_tty3.sh; then
    echo "Native chrome proof must use normal-session startup applications." >&2
    exit 1
fi
grep -Fq -- '--exit-when-startup-exits' tools/run_sophia_xmonad_session.sh
grep -Fq -- '--input-seat=$input_seat' tools/run_sophia_xmonad_session.sh
if grep -Fq -- '*-event-kbd' tools/run_sophia_xmonad_session.sh; then
    echo "TTY launcher must use production libinput seat discovery, not path heuristics" >&2
    exit 1
fi
bash -n tools/stop_sophia_xmonad_session.sh
bash -n tools/build_sophia_xmonad.sh tools/build_sophia_xmobar.sh \
    tools/resolve_sophia_xmonad.sh tools/xmonad_live_session_smoke.sh \
    tools/verify_packaged_policy.sh tools/check_sophia_desktop_policy.sh
tools/check_sophia_desktop_policy.sh
bash -n tools/package_live_session.sh tools/install_live_session.sh \
    tools/install_current_live_session.sh tools/run_installed_cycle_gate_tty3.sh
tools/run_installed_cycle_gate_tty3.sh --self-test
grep -Fq 'exec {SESSION_INPUT_FD}<&0' tools/run_installed_cycle_gate_tty3.sh
grep -Fq 'wm_helpers_are_drained' tools/run_installed_cycle_gate_tty3.sh
if grep -Fq 'exec {SESSION_INPUT_FD}<>/dev/tty' \
    tools/run_installed_cycle_gate_tty3.sh; then
    echo "Installed cycle runner must preserve the concrete greetd VT descriptor." >&2
    exit 1
fi
grep -Fq 'rustix::io::fcntl_dupfd_cloexec' crates/sophia-backend-live/src/seat.rs
if grep -Fq 'rustix::io::dup(' crates/sophia-backend-live/src/seat.rs; then
    echo "Seat authority descriptors must be close-on-exec." >&2
    exit 1
fi
bash -n tools/rollback_live_session.sh tools/status_live_session.sh
bash -n tools/check_live_session_install.sh
tools/check_live_session_install.sh
bash -n tools/lib/session_lifecycle.sh tools/check_session_lifecycle_diagnostics.sh
tools/check_session_lifecycle_diagnostics.sh
bash -n tools/installed/sophia-session
bash -n tools/installed/sophia-kitty-session
bash -n tools/installed/sophia-firefox-proof
bash -n tools/installed/sophia-xterm-proof
bash -n tools/installed/sophia-recovery-proof
bash -n tools/installed/sophia-native-chrome-proof
bash -n tools/installed/sophia-stop
bash -n tools/installed/capture-runtime-identity.sh
grep -Fq -- '--firefox-m10-proof' tools/installed/sophia-firefox-proof
grep -Fq 'SOPHIA_INSTALLED_ATTEMPT_MODE=firefox' \
    tools/installed/sophia-firefox-proof
grep -Fq 'SOPHIA_INSTALLED_ATTEMPT_MODE=xterm' \
    tools/installed/sophia-xterm-proof
grep -Fq 'SOPHIA_TERMINAL_KIND=xterm' tools/installed/sophia-xterm-proof
grep -Fq 'firefox_m10_kitty_probe.sh' tools/package_live_session.sh
grep -Fq 'tools/record_installed_firefox_attempt.sh' \
    tools/package_live_session.sh
grep -Fq 'tools/record_installed_xterm_run.sh' tools/package_live_session.sh
grep -Fq 'tools/verify_installed_xterm_session.sh' tools/package_live_session.sh
grep -Fq 'tools/verify_installed_xterm_runs.sh' tools/package_live_session.sh
grep -Fq 'tools/installed/sophia-recovery-proof' tools/package_live_session.sh
grep -Fq 'tools/verify_installed_emergency_archive.sh' tools/package_live_session.sh
grep -Fq 'tools/record_installed_watchdog_run.sh' tools/package_live_session.sh
grep -Fq 'tools/verify_installed_watchdog_recovery.sh' tools/package_live_session.sh
grep -Fq 'tools/verify_installed_watchdog_archive.sh' tools/package_live_session.sh
grep -Fq 'tools/record_installed_fallback_run.sh' tools/package_live_session.sh
grep -Fq 'tools/verify_installed_fallback_session.sh' tools/package_live_session.sh
grep -Fq 'tools/record_installed_native_chrome_run.sh' tools/package_live_session.sh
grep -Fq 'tools/verify_installed_native_chrome_archive.sh' tools/package_live_session.sh
grep -Fq 'target/release/sophia-wm-demo' tools/package_live_session.sh
grep -Fq 'tools/lib/installed_attempt_ledger.sh' tools/package_live_session.sh
grep -Fq 'tools/lib/session_terminal.sh' tools/package_live_session.sh
grep -Fq 'tools/verify_installed_soak_archive.sh' tools/package_live_session.sh
grep -Fq 'sophia-verify-soak-session' tools/package_live_session.sh
bash -n tools/verify_installed_runtime_identity.sh
bash -n tools/record_installed_session_run.sh \
    tools/record_installed_firefox_attempt.sh \
    tools/record_installed_xterm_run.sh \
    tools/record_installed_emergency_run.sh \
    tools/record_installed_watchdog_run.sh tools/record_installed_fallback_run.sh \
    tools/record_installed_native_chrome_run.sh
bash -n tools/lib/installed_attempt_ledger.sh \
    tools/verify_installed_session_cycles.sh \
    tools/verify_installed_fallback_run.sh
bash -n tools/verify_installed_session_soak.sh tools/check_installed_session_verifiers.sh
tools/check_installed_session_verifiers.sh
bash -n tools/verify_installed_soak_archive.sh tools/check_installed_soak_archive.sh
tools/check_installed_soak_archive.sh
bash -n tools/live_session_persistent_hardware_proof.sh
bash -n tools/live_session_milestone5_gtk_hardware_proof.sh
if ! grep -Fq -- '--namespace-profile="$profile" --software-client-rendering' tools/live_session_milestone5_gtk_hardware_proof.sh; then
    echo "Milestone 5 GTK proof must retain the selected namespace profile in software-rendering mode." >&2
    exit 1
fi
bash -n tools/verify_live_session_milestone5_tty_recovery.sh
bash -n tools/check_live_session_milestone5_verifier.sh
tools/check_live_session_milestone5_verifier.sh
python3 -c 'compile(open("tools/sophia_tty_mode.py", encoding="utf-8").read(), "tools/sophia_tty_mode.py", "exec")'
python3 -c 'compile(open("tools/probes/uinput_text_injector.py", encoding="utf-8").read(), "tools/probes/uinput_text_injector.py", "exec")'
tools/probes/uinput_text_injector.py --self-test
tools/probes/uinput_text_injector.py --chord=logout --self-test
tools/probes/uinput_text_injector.py \
    --chord=recovery --followup-chord=logout --self-test

echo "atomic scanout local checks passed"
