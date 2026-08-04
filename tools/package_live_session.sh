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
xmonad_bin="$(tools/resolve_sophia_xmonad.sh)"

install -d -m 755 \
    "$artifact/bin" \
    "$artifact/target/release" \
    "$artifact/tools/fixtures" \
    "$artifact/tools/lib" \
    "$artifact/share/wayland-sessions"
install -m 755 target/release/sophia "$artifact/target/release/sophia"
install -m 755 target/release/sophia-x11-wm-bridge \
    "$artifact/target/release/sophia-x11-wm-bridge"
install -m 755 "$xmonad_bin" "$artifact/target/release/xmonad"
install -m 755 tools/installed/sophia-session "$artifact/bin/sophia-session"
install -m 755 tools/installed/sophia-kitty-session \
    "$artifact/bin/sophia-kitty-session"
install -m 755 tools/installed/sophia-firefox-proof \
    "$artifact/bin/sophia-firefox-proof"
install -m 755 tools/installed/capture-runtime-identity.sh \
    "$artifact/bin/capture-runtime-identity"
install -m 755 tools/status_live_session.sh "$artifact/bin/sophia-status"
install -m 755 tools/stop_sophia_xmonad_session.sh "$artifact/bin/sophia-stop"
install -m 755 tools/rollback_live_session.sh "$artifact/bin/sophia-rollback"
install -m 755 tools/record_installed_session_run.sh \
    "$artifact/bin/sophia-record-run"
install -m 755 tools/record_installed_emergency_run.sh \
    "$artifact/bin/sophia-record-emergency-run"
install -m 755 tools/verify_installed_session_cycles.sh \
    "$artifact/bin/sophia-verify-cycles"
install -m 755 tools/verify_installed_session_soak.sh \
    "$artifact/bin/sophia-verify-soak"
install -m 755 tools/verify_sophia_xmonad_tty3.sh \
    "$artifact/bin/sophia-verify-xmonad-run"
install -m 755 tools/verify_sophia_xmonad_emergency_tty3.sh \
    "$artifact/bin/sophia-verify-emergency-run"
install -m 755 tools/verify_installed_runtime_identity.sh \
    "$artifact/bin/sophia-verify-runtime-identity"
install -m 755 tools/verify_installed_session_lifecycle.sh \
    "$artifact/bin/sophia-verify-lifecycle"
install -m 755 tools/verify_sophia_firefox_physical.sh \
    "$artifact/bin/sophia-verify-firefox-run"
install -m 755 tools/record_sophia_firefox_physical_run.sh \
    "$artifact/bin/sophia-record-firefox-run"
install -m 755 tools/verify_sophia_firefox_physical_runs.sh \
    "$artifact/bin/sophia-verify-firefox-runs"
install -m 755 tools/run_sophia_xmonad_session.sh \
    tools/resolve_sophia_xmonad.sh \
    tools/resolve_sophia_xmobar.sh "$artifact/tools/"
install -m 644 tools/lib/session_lifecycle.sh \
    "$artifact/tools/lib/session_lifecycle.sh"
install -m 755 tools/sophia_tty_mode.py "$artifact/tools/sophia_tty_mode.py"
install -m 644 tools/fixtures/firefox_m8_local_page.html \
    "$artifact/tools/fixtures/firefox_m8_local_page.html"
install -m 755 tools/fixtures/firefox_m10_kitty_probe.sh \
    "$artifact/tools/fixtures/firefox_m10_kitty_probe.sh"
install -m 755 tools/fixtures/firefox_m10_selection_kitty_probe.sh \
    "$artifact/tools/fixtures/firefox_m10_selection_kitty_probe.sh"
install -m 644 tools/fixtures/xmobar_sophia.config \
    "$artifact/tools/fixtures/xmobar_sophia.config"

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
printf 'schema=1\nversion=%s\ncommit=%s\nrelease_id=%s\nbuilt_at_utc=%s\n' \
    "$version" "$commit" "$release_id" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    >"$artifact/manifest"
(
    cd "$artifact"
    find bin target tools share -type f -print0 |
        sort -z |
        xargs -0 sha256sum >SHA256SUMS
)

echo "Packaged immutable Sophia release: $artifact"
echo "Install with: tools/install_live_session.sh"
