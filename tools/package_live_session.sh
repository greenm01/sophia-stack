#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACT_ROOT="${SOPHIA_ARTIFACT_ROOT:-$ROOT_DIR/.artifacts}"

cd "$ROOT_DIR"
[[ -z "$(git status --short)" ]] || {
    echo "Refusing to package a dirty worktree; commit the exact release first." >&2
    exit 1
}
commit="$(git rev-parse HEAD)"
version="$(awk -F'"' '$1 ~ /^version = / { print $2; exit }' Cargo.toml)"
[[ -n "$version" ]] || {
    echo "Could not resolve workspace version." >&2
    exit 1
}
release_id="${version}-${commit:0:12}"
artifact="$ARTIFACT_ROOT/sophia-$release_id"
[[ ! -e "$artifact" ]] || {
    echo "Release artifact already exists: $artifact" >&2
    exit 1
}

cargo build --offline --release -p sophia-cli --features atomic-scanout-live
cargo build --offline --release -p sophia-x11-wm-bridge
cargo build --offline --release -p sophia-wm-demo
xmonad_bin="$(tools/build_sophia_xmonad.sh)"
xmobar_bin="$(tools/build_sophia_xmobar.sh)"
xmobar_source="${SOPHIA_XMOBAR_SOURCE:-$HOME/src/xmobar}"
xmobar_source_commit="$(git -C "$xmobar_source" rev-parse HEAD)"
xmonad_version="$($xmonad_bin --version 2>&1 | head -n 1 | tr ' ' '_')"
xmobar_version="$($xmobar_bin --version 2>&1 | head -n 1 | tr ' ' '_')"
xmonad_config_sha256="$(sha256sum tools/config/sophia-xmonad/Main.hs | awk '{print $1}')"
xmonad_cabal_sha256="$(sha256sum tools/config/sophia-xmonad/sophia-xmonad.cabal | awk '{print $1}')"
xmonad_project_sha256="$(sha256sum tools/config/sophia-xmonad/cabal.project | awk '{print $1}')"
xmobar_config_sha256="$(sha256sum tools/fixtures/xmobar_sophia.config | awk '{print $1}')"

install -d -m 755 \
    "$artifact/bin" \
    "$artifact/target/release" \
    "$artifact/tools/fixtures" \
    "$artifact/tools/lib" \
    "$artifact/tools/probes" \
    "$artifact/share/doc/sophia" \
    "$artifact/share/sophia-policy/xmonad" \
    "$artifact/share/wayland-sessions"
install -m 755 target/release/sophia "$artifact/target/release/sophia"
install -m 755 target/release/sophia-x11-wm-bridge \
    "$artifact/target/release/sophia-x11-wm-bridge"
install -m 755 target/release/sophia-wm-demo \
    "$artifact/target/release/sophia-wm-demo"
install -m 755 "$xmonad_bin" "$artifact/target/release/xmonad"
install -m 755 "$xmobar_bin" "$artifact/target/release/xmobar"
install -m 755 tools/installed/sophia-session "$artifact/bin/sophia-session"
install -m 755 tools/installed/sophia-kitty-session \
    "$artifact/bin/sophia-kitty-session"
install -m 755 tools/installed/sophia-firefox-proof \
    "$artifact/bin/sophia-firefox-proof"
install -m 755 tools/installed/sophia-xterm-proof \
    "$artifact/bin/sophia-xterm-proof"
install -m 755 tools/installed/sophia-recovery-proof \
    "$artifact/bin/sophia-recovery-proof"
install -m 755 tools/installed/sophia-native-chrome-proof \
    "$artifact/bin/sophia-native-chrome-proof"
install -m 755 tools/installed/capture-runtime-identity.sh \
    "$artifact/bin/capture-runtime-identity"
install -m 755 tools/run_installed_cycle_gate_tty3.sh \
    "$artifact/bin/sophia-run-cycles"
install -m 755 tools/setup_sophia_uinput.sh \
    "$artifact/bin/sophia-setup-uinput"
install -m 755 tools/status_live_session.sh "$artifact/bin/sophia-status"
install -m 755 tools/installed/sophia-stop "$artifact/bin/sophia-stop"
install -m 755 tools/rollback_live_session.sh "$artifact/bin/sophia-rollback"
install -m 755 tools/record_installed_session_run.sh \
    "$artifact/bin/sophia-record-run"
install -m 755 tools/record_installed_firefox_attempt.sh \
    "$artifact/bin/sophia-record-firefox-attempt"
install -m 755 tools/record_installed_xterm_run.sh \
    "$artifact/bin/sophia-record-xterm-run"
install -m 755 tools/record_installed_fallback_run.sh \
    "$artifact/bin/sophia-record-fallback-run"
install -m 755 tools/record_installed_emergency_run.sh \
    "$artifact/bin/sophia-record-emergency-run"
install -m 755 tools/record_installed_watchdog_run.sh \
    "$artifact/bin/sophia-record-watchdog-run"
install -m 755 tools/record_installed_native_chrome_run.sh \
    "$artifact/bin/sophia-record-native-chrome-run"
install -m 755 tools/verify_installed_session_cycles.sh \
    "$artifact/bin/sophia-verify-cycles"
install -m 755 tools/verify_installed_soak_archive.sh \
    "$artifact/bin/sophia-verify-soak"
install -m 755 tools/verify_installed_session_soak.sh \
    "$artifact/bin/sophia-verify-soak-session"
install -m 755 tools/verify_sophia_xmonad_tty3.sh \
    "$artifact/bin/sophia-verify-xmonad-run"
install -m 755 tools/verify_installed_login_cycle.sh \
    "$artifact/bin/sophia-verify-login-cycle"
install -m 755 tools/verify_installed_xterm_session.sh \
    "$artifact/bin/sophia-verify-xterm-run"
install -m 755 tools/verify_installed_xterm_runs.sh \
    "$artifact/bin/sophia-verify-xterm-runs"
install -m 755 tools/verify_installed_fallback_session.sh \
    "$artifact/bin/sophia-verify-fallback-session"
install -m 755 tools/verify_installed_fallback_run.sh \
    "$artifact/bin/sophia-verify-fallback"
install -m 755 tools/verify_sophia_xmonad_emergency_tty3.sh \
    "$artifact/bin/sophia-verify-emergency-run"
install -m 755 tools/verify_installed_emergency_archive.sh \
    "$artifact/bin/sophia-verify-emergency"
install -m 755 tools/verify_installed_runtime_identity.sh \
    "$artifact/bin/sophia-verify-runtime-identity"
install -m 755 tools/verify_installed_session_lifecycle.sh \
    "$artifact/bin/sophia-verify-lifecycle"
install -m 755 tools/verify_installed_watchdog_recovery.sh \
    "$artifact/bin/sophia-verify-watchdog-run"
install -m 755 tools/verify_installed_watchdog_archive.sh \
    "$artifact/bin/sophia-verify-watchdog"
install -m 755 tools/verify_sophia_native_chrome.sh \
    "$artifact/bin/sophia-verify-native-chrome-core"
install -m 755 tools/verify_installed_native_chrome_session.sh \
    "$artifact/bin/sophia-verify-native-chrome-session"
install -m 755 tools/verify_installed_native_chrome_archive.sh \
    "$artifact/bin/sophia-verify-native-chrome"
install -m 755 tools/verify_sophia_firefox_physical.sh \
    "$artifact/bin/sophia-verify-firefox-run"
install -m 755 tools/record_sophia_firefox_physical_run.sh \
    "$artifact/bin/sophia-record-firefox-run"
install -m 755 tools/verify_sophia_firefox_physical_runs.sh \
    "$artifact/bin/sophia-verify-firefox-runs"
install -m 755 tools/run_sophia_xmonad_session.sh \
    tools/resolve_sophia_xmonad.sh \
    tools/resolve_sophia_xmobar.sh \
    tools/stop_sophia_session.sh \
    tools/start_sophia_native_hot_reload_tty3.sh "$artifact/tools/"
install -m 755 tools/verify_packaged_policy.sh \
    "$artifact/tools/verify_packaged_policy.sh"
install -d -m 755 "$artifact/tools/config"
install -m 755 tools/probes/uinput_text_injector.py \
    "$artifact/tools/probes/uinput_text_injector.py"
install -m 644 tools/config/proof_helpers.sh \
    "$artifact/tools/config/proof_helpers.sh"
install -m 644 tools/config/99-sophia-uinput.rules \
    "$artifact/tools/config/99-sophia-uinput.rules"
install -m 644 tools/config/sophia-uinput.conf \
    "$artifact/tools/config/sophia-uinput.conf"
install -m 644 tools/lib/session_lifecycle.sh \
    "$artifact/tools/lib/session_lifecycle.sh"
install -m 644 tools/lib/installed_attempt_ledger.sh \
    "$artifact/tools/lib/installed_attempt_ledger.sh"
install -m 755 tools/sophia_tty_mode.py "$artifact/tools/sophia_tty_mode.py"
install -m 644 tools/fixtures/firefox_m8_local_page.html \
    "$artifact/tools/fixtures/firefox_m8_local_page.html"
install -m 755 tools/fixtures/firefox_m10_kitty_probe.sh \
    "$artifact/tools/fixtures/firefox_m10_kitty_probe.sh"
install -m 755 tools/fixtures/firefox_m10_selection_kitty_probe.sh \
    "$artifact/tools/fixtures/firefox_m10_selection_kitty_probe.sh"
install -m 644 tools/fixtures/xmobar_sophia.config \
    "$artifact/tools/fixtures/xmobar_sophia.config"
install -m 644 tools/config/sophia-xmonad/Main.hs \
    tools/config/sophia-xmonad/sophia-xmonad.cabal \
    tools/config/sophia-xmonad/cabal.project \
    "$artifact/share/sophia-policy/xmonad/"
install -m 644 docs/operations.md "$artifact/share/doc/sophia/operations.md"

printf '%s\n' \
    '[Desktop Entry]' \
    'Name=Sophia xmonad (Experimental)' \
    'Comment=Sophia native X11 xmonad promotion candidate' \
    'Exec=@SOPHIA_INSTALL_PREFIX@/current/bin/sophia-session' \
    'Type=Application' \
    'DesktopNames=Sophia' \
    >"$artifact/share/wayland-sessions/sophia.desktop"
printf '%s\n' \
    '[Desktop Entry]' \
    'Name=Sophia Kitty (Baseline)' \
    'Comment=Sophia proven Kitty-only physical input baseline' \
    'Exec=@SOPHIA_INSTALL_PREFIX@/current/bin/sophia-kitty-session' \
    'Type=Application' \
    'DesktopNames=Sophia' \
    >"$artifact/share/wayland-sessions/sophia-kitty.desktop"
printf '%s\n' \
    '[Desktop Entry]' \
    'Name=Sophia Firefox Proof' \
    'Comment=Sophia installed physical Firefox promotion workflow' \
    'Exec=@SOPHIA_INSTALL_PREFIX@/current/bin/sophia-firefox-proof' \
    'Type=Application' \
    'DesktopNames=Sophia' \
    >"$artifact/share/wayland-sessions/sophia-firefox-proof.desktop"
printf '%s\n' \
    '[Desktop Entry]' \
    'Name=Sophia Recovery Proof' \
    'Comment=Bounded installed session and automatic display-manager recovery' \
    'Exec=@SOPHIA_INSTALL_PREFIX@/current/bin/sophia-recovery-proof' \
    'Type=Application' \
    'DesktopNames=Sophia' \
    >"$artifact/share/wayland-sessions/sophia-recovery-proof.desktop"
printf '%s\n' \
    '[Desktop Entry]' \
    'Name=Sophia Native Chrome Proof' \
    'Comment=Installed ring, frame, and combined chrome proof' \
    'Exec=@SOPHIA_INSTALL_PREFIX@/current/bin/sophia-native-chrome-proof' \
    'Type=Application' \
    'DesktopNames=Sophia' \
    >"$artifact/share/wayland-sessions/sophia-native-chrome-proof.desktop"
printf '%s\n' \
    '[Desktop Entry]' \
    'Name=Sophia Cycle Gate (Automated)' \
    'Comment=Ten automatic installed startup and normal-logout cycles' \
    'Exec=@SOPHIA_INSTALL_PREFIX@/current/bin/sophia-run-cycles' \
    'Type=Application' \
    'DesktopNames=Sophia' \
    >"$artifact/share/wayland-sessions/sophia-cycle-proof.desktop"
printf 'schema=2\nversion=%s\ncommit=%s\nrelease_id=%s\nbuilt_at_utc=%s\nxmonad_version=%s\nxmonad_source_version=0.18.1\nxmonad_contrib_source_version=0.18.2\nxmonad_config_sha256=%s\nxmonad_cabal_sha256=%s\nxmonad_project_sha256=%s\nxmonad_binary_sha256=%s\nxmobar_version=%s\nxmobar_source_commit=%s\nxmobar_config_sha256=%s\nxmobar_binary_sha256=%s\n' \
    "$version" "$commit" "$release_id" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    "$xmonad_version" "$xmonad_config_sha256" "$xmonad_cabal_sha256" \
    "$xmonad_project_sha256" "$(sha256sum "$xmonad_bin" | awk '{print $1}')" \
    "$xmobar_version" "$xmobar_source_commit" "$xmobar_config_sha256" \
    "$(sha256sum "$xmobar_bin" | awk '{print $1}')" \
    >"$artifact/manifest"
"$artifact/tools/verify_packaged_policy.sh" "$artifact"
(
    cd "$artifact"
    find bin target tools share -type f -print0 |
        sort -z |
        xargs -0 sha256sum >SHA256SUMS
)

echo "Packaged immutable Sophia release: $artifact"
echo "Install with: tools/install_live_session.sh"
