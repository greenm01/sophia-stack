#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR="$(mktemp -d)"
stop_pid=""
cleanup() {
    [[ -z "$stop_pid" ]] || kill "$stop_pid" 2>/dev/null || true
    rm -rf -- "$TEMP_DIR"
}
trap cleanup EXIT

# shellcheck source=tools/lib/live_session_surface.sh
source "$ROOT_DIR/tools/lib/live_session_surface.sh"

make_executable() {
    local path="$1"
    printf '#!/usr/bin/env bash\nexit 0\n' >"$path"
    chmod 755 "$path"
}

make_artifact() {
    local release_id="$1" with_hagia="$2" artifact command desktop
    local hagia_digest narthex_digest profile_digest
    artifact="$TEMP_DIR/artifact-$release_id"
    install -d -m 755 \
        "$artifact/bin" \
        "$artifact/share/doc/sophia" \
        "$artifact/share/wayland-sessions" \
        "$artifact/target/release" \
        "$artifact/tools/lib"
    install -m 644 "$ROOT_DIR/docs/operations.md" \
        "$artifact/share/doc/sophia/operations.md"
    install -m 644 "$ROOT_DIR/tools/lib/live_session_surface.sh" \
        "$artifact/tools/lib/live_session_surface.sh"
    install -m 755 "$ROOT_DIR/tools/verify_packaged_policy.sh" \
        "$artifact/tools/verify_packaged_policy.sh"

    make_executable "$artifact/target/release/sophia"
    make_executable "$artifact/target/release/sophia-wm-demo"
    if [[ "$with_hagia" == true ]]; then
        install -d -m 755 "$artifact/share/sophia-policy/hagia"
        make_executable "$artifact/target/release/hagia"
        make_executable "$artifact/target/release/narthex"
        printf 'schema 1\nshell { enabled #true; panel 28; }\n' \
            >"$artifact/share/sophia-policy/hagia/default.kdl"
        hagia_digest="$(sha256sum "$artifact/target/release/hagia" | awk '{print $1}')"
        narthex_digest="$(sha256sum "$artifact/target/release/narthex" | awk '{print $1}')"
        profile_digest="$(sha256sum "$artifact/share/sophia-policy/hagia/default.kdl" | awk '{print $1}')"
        printf 'schema=6\nversion=0.1.0\ncommit=%040d\nrelease_id=%s\nbuilt_at_utc=2026-09-04T00:00:00Z\nhagia_included=true\nhagia_source_commit=%040d\nhagia_default_profile_sha256=%s\nhagia_binary_sha256=%s\nhagia_shell_binary_sha256=%s\n' \
            "$release_id" "$release_id" 1 "$profile_digest" "$hagia_digest" \
            "$narthex_digest" >"$artifact/manifest"
    else
        printf 'schema=6\nversion=0.1.0\ncommit=%040d\nrelease_id=%s\nbuilt_at_utc=2026-09-04T00:00:00Z\nhagia_included=false\n' \
            "$release_id" "$release_id" >"$artifact/manifest"
    fi

    sophia_surface_select_entries "$artifact"
    for command in "${SOPHIA_SURFACE_COMMANDS[@]}"; do
        case "$command" in
            sophia-status)
                install -m 755 "$ROOT_DIR/tools/status_live_session.sh" \
                    "$artifact/bin/$command"
                ;;
            sophia-rollback)
                install -m 755 "$ROOT_DIR/tools/rollback_live_session.sh" \
                    "$artifact/bin/$command"
                ;;
            sophia-stop)
                install -m 755 "$ROOT_DIR/tools/installed/sophia-stop" \
                    "$artifact/bin/$command"
                ;;
            *) make_executable "$artifact/bin/$command" ;;
        esac
    done
    install -m 755 "$ROOT_DIR/tools/stop_sophia_session.sh" \
        "$artifact/tools/stop_sophia_session.sh"
    for desktop in "${SOPHIA_SURFACE_DESKTOPS[@]}"; do
        case "$desktop" in
            sophia-hagia) command=sophia-hagia-session ;;
            sophia-hagia-promotion) command=sophia-hagia-promotion-session ;;
            sophia-kitty) command=sophia-kitty-session ;;
            sophia-firefox-proof) command=sophia-firefox-proof ;;
            sophia-recovery-proof) command=sophia-recovery-proof ;;
            sophia-native-chrome-proof) command=sophia-native-chrome-proof ;;
            *) echo "test does not map desktop $desktop" >&2; return 1 ;;
        esac
        printf '[Desktop Entry]\nExec=@SOPHIA_INSTALL_PREFIX@/current/bin/%s\n' \
            "$command" >"$artifact/share/wayland-sessions/$desktop.desktop"
    done
    (
        cd "$artifact"
        find bin share target tools -type f -print0 | sort -z | \
            xargs -0 sha256sum >SHA256SUMS
    )
    "$ROOT_DIR/tools/verify_packaged_policy.sh" "$artifact" >/dev/null
    printf '%s\n' "$artifact"
}

expect_policy_rejection() {
    local artifact="$1" label="$2"
    if "$ROOT_DIR/tools/verify_packaged_policy.sh" "$artifact" >/dev/null 2>&1; then
        echo "packaged policy verifier accepted $label" >&2
        exit 1
    fi
}

first="$(make_artifact 0001 false)"
second="$(make_artifact 0002 false)"
hagia_artifact="$(make_artifact 0003 true)"

invalid_bridge="$TEMP_DIR/invalid-bridge"
cp -a "$first" "$invalid_bridge"
make_executable "$invalid_bridge/target/release/sophia-x11-wm-bridge"
expect_policy_rejection "$invalid_bridge" "a removed legacy-WM bridge"

invalid_legacy_field="$TEMP_DIR/invalid-legacy-field"
cp -a "$first" "$invalid_legacy_field"
printf 'xmonad_binary_sha256=%064d\n' 0 >>"$invalid_legacy_field/manifest"
expect_policy_rejection "$invalid_legacy_field" "a legacy policy manifest field"

invalid_narthex="$TEMP_DIR/invalid-narthex"
cp -a "$hagia_artifact" "$invalid_narthex"
chmod 644 "$invalid_narthex/target/release/narthex"
expect_policy_rejection "$invalid_narthex" "a non-executable Narthex binary"

PREFIX="$TEMP_DIR/install/prefix"
SESSION_DIR="$TEMP_DIR/share/wayland-sessions"
COMMAND_DIR="$TEMP_DIR/commands"
install_env=(
    SOPHIA_INSTALL_PREFIX="$PREFIX"
    SOPHIA_SESSION_DIR="$SESSION_DIR"
    SOPHIA_COMMAND_DIR="$COMMAND_DIR"
)

env "${install_env[@]}" "$ROOT_DIR/tools/install_live_session.sh" "$first"
[[ "$(readlink "$PREFIX/current")" == releases/0001 ]]
[[ ! -e "$PREFIX/previous" ]]
for desktop in sophia-kitty sophia-native-chrome-proof; do
    [[ -f "$SESSION_DIR/$desktop.desktop" ]]
done
[[ ! -e "$SESSION_DIR/sophia.desktop" ]]
[[ ! -e "$COMMAND_DIR/sophia-verify-xmonad-run" ]]

# Activation removes only stale Sophia-owned compatibility entries. Foreign
# files with the same names remain operator-owned.
printf '[Desktop Entry]\nExec=%s/current/bin/sophia-session\n' "$PREFIX" \
    >"$SESSION_DIR/sophia.desktop"
ln -s "$PREFIX/current/bin/sophia-verify-xmonad-run" \
    "$COMMAND_DIR/sophia-verify-xmonad-run"
printf '[Desktop Entry]\nExec=/foreign-hagia-session\n' \
    >"$SESSION_DIR/sophia-hagia.desktop"
ln -s /foreign-hagia-command "$COMMAND_DIR/sophia-hagia-session"

env "${install_env[@]}" "$ROOT_DIR/tools/install_live_session.sh" "$second"
[[ "$(readlink "$PREFIX/current")" == releases/0002 ]]
[[ "$(readlink "$PREFIX/previous")" == releases/0001 ]]
[[ ! -e "$SESSION_DIR/sophia.desktop" ]]
[[ ! -e "$COMMAND_DIR/sophia-verify-xmonad-run" ]]
grep -Fqx 'Exec=/foreign-hagia-session' "$SESSION_DIR/sophia-hagia.desktop"
[[ "$(readlink "$COMMAND_DIR/sophia-hagia-session")" == /foreign-hagia-command ]]

env "${install_env[@]}" "$COMMAND_DIR/sophia-rollback"
[[ "$(readlink "$PREFIX/current")" == releases/0001 ]]
[[ "$(readlink "$PREFIX/previous")" == releases/0002 ]]

hagia_prefix="$TEMP_DIR/hagia/prefix"
hagia_sessions="$TEMP_DIR/hagia/sessions"
hagia_commands="$TEMP_DIR/hagia/commands"
hagia_env=(
    SOPHIA_INSTALL_PREFIX="$hagia_prefix"
    SOPHIA_SESSION_DIR="$hagia_sessions"
    SOPHIA_COMMAND_DIR="$hagia_commands"
)
env "${hagia_env[@]}" "$ROOT_DIR/tools/install_live_session.sh" "$first"
env "${hagia_env[@]}" "$ROOT_DIR/tools/install_live_session.sh" "$hagia_artifact"
[[ "$(readlink "$hagia_prefix/current")" == releases/0003 ]]
for desktop in sophia-hagia sophia-hagia-promotion sophia-firefox-proof sophia-recovery-proof; do
    [[ -f "$hagia_sessions/$desktop.desktop" ]]
done
for command in sophia-hagia-session sophia-hagia-promotion-session \
    sophia-record-hagia-run sophia-verify-hagia sophia-verify-hagia-promotion; do
    [[ "$(readlink "$hagia_commands/$command")" == \
        "$hagia_prefix/current/bin/$command" ]]
done

env "${hagia_env[@]}" "$hagia_commands/sophia-rollback"
[[ "$(readlink "$hagia_prefix/current")" == releases/0001 ]]
for desktop in sophia-hagia sophia-hagia-promotion sophia-firefox-proof sophia-recovery-proof; do
    [[ ! -e "$hagia_sessions/$desktop.desktop" ]]
done
[[ ! -e "$hagia_commands/sophia-hagia-session" ]]

env "${hagia_env[@]}" "$hagia_commands/sophia-rollback"
[[ "$(readlink "$hagia_prefix/current")" == releases/0003 ]]
[[ -f "$hagia_sessions/sophia-hagia.desktop" ]]

if env "${hagia_env[@]}" "$ROOT_DIR/tools/activate_live_session_release.sh" \
    "$hagia_artifact" >/dev/null 2>&1; then
    echo "activation accepted an artifact outside the immutable install prefix" >&2
    exit 1
fi

stop_runtime="$TEMP_DIR/stop-runtime"
stop_state="$stop_runtime/sophia-hagia-session-$UID"
install -d -m 700 "$stop_state"
sleep 60 &
stop_pid=$!
printf '%s\n' "$stop_pid" >"$stop_state/wrapper.pid"
env XDG_RUNTIME_DIR="$stop_runtime" "$hagia_commands/sophia-stop"
if kill -0 "$stop_pid" 2>/dev/null; then
    echo "installed Sophia stop command left the Hagia wrapper running" >&2
    exit 1
fi
wait "$stop_pid" 2>/dev/null || true
stop_pid=""

echo "native-only live-session install, activation, and rollback checks passed"
