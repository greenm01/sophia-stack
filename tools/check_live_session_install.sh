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
PREFIX="$TEMP_DIR/nested/install/prefix"
SESSION_DIR="$TEMP_DIR/nested/share/wayland-sessions"
COMMAND_DIR="$TEMP_DIR/nested/commands"
OPERATOR_COMMANDS=(
    sophia-session
    sophia-kitty-session
    sophia-firefox-proof
    sophia-recovery-proof
    sophia-native-chrome-proof
    sophia-status
    sophia-stop
    sophia-rollback
    sophia-run-cycles
    sophia-setup-uinput
    sophia-record-run
    sophia-record-fallback-run
    sophia-record-emergency-run
    sophia-record-watchdog-run
    sophia-record-native-chrome-run
    sophia-record-firefox-run
    sophia-verify-login-cycle
    sophia-verify-cycles
    sophia-verify-emergency
    sophia-verify-fallback
    sophia-verify-watchdog
    sophia-verify-native-chrome
    sophia-verify-firefox-runs
    sophia-verify-soak
)

make_artifact() {
    local release_id="$1" artifact command
    artifact="$TEMP_DIR/artifact-$release_id"
    install -d -m 755 \
        "$artifact/bin" \
        "$artifact/share/doc/sophia" \
        "$artifact/share/wayland-sessions" \
        "$artifact/tools"
    install -m 644 "$ROOT_DIR/docs/operations.md" \
        "$artifact/share/doc/sophia/operations.md"
    for command in "${OPERATOR_COMMANDS[@]}"; do
        case "$command" in
            sophia-status)
                cp "$ROOT_DIR/tools/status_live_session.sh" "$artifact/bin/$command"
                ;;
            sophia-rollback)
                cp "$ROOT_DIR/tools/rollback_live_session.sh" "$artifact/bin/$command"
                ;;
            sophia-stop)
                cp "$ROOT_DIR/tools/installed/sophia-stop" \
                    "$artifact/bin/$command"
                ;;
            *)
                printf '#!/usr/bin/env bash\nexit 0\n' >"$artifact/bin/$command"
                ;;
        esac
        chmod 755 "$artifact/bin/$command"
    done
    install -m 755 "$ROOT_DIR/tools/stop_sophia_session.sh" \
        "$artifact/tools/stop_sophia_session.sh"
    printf '[Desktop Entry]\nExec=@SOPHIA_INSTALL_PREFIX@/current/bin/sophia-session\n' \
        >"$artifact/share/wayland-sessions/sophia.desktop"
    printf '[Desktop Entry]\nExec=@SOPHIA_INSTALL_PREFIX@/current/bin/sophia-kitty-session\n' \
        >"$artifact/share/wayland-sessions/sophia-kitty.desktop"
    printf '[Desktop Entry]\nExec=@SOPHIA_INSTALL_PREFIX@/current/bin/sophia-firefox-proof\n' \
        >"$artifact/share/wayland-sessions/sophia-firefox-proof.desktop"
    printf '[Desktop Entry]\nExec=@SOPHIA_INSTALL_PREFIX@/current/bin/sophia-recovery-proof\n' \
        >"$artifact/share/wayland-sessions/sophia-recovery-proof.desktop"
    printf '[Desktop Entry]\nExec=@SOPHIA_INSTALL_PREFIX@/current/bin/sophia-native-chrome-proof\n' \
        >"$artifact/share/wayland-sessions/sophia-native-chrome-proof.desktop"
    printf '[Desktop Entry]\nExec=@SOPHIA_INSTALL_PREFIX@/current/bin/sophia-run-cycles\n' \
        >"$artifact/share/wayland-sessions/sophia-cycle-proof.desktop"
    printf 'schema=1\nversion=0.1.0\ncommit=%040d\nrelease_id=%s\n' \
        "$release_id" "$release_id" >"$artifact/manifest"
    (
        cd "$artifact"
        find bin share tools -type f -print0 | sort -z | xargs -0 sha256sum >SHA256SUMS
    )
    printf '%s\n' "$artifact"
}

first="$(make_artifact 0001)"
second="$(make_artifact 0002)"
install_env=(
    SOPHIA_INSTALL_PREFIX="$PREFIX"
    SOPHIA_SESSION_DIR="$SESSION_DIR"
    SOPHIA_COMMAND_DIR="$COMMAND_DIR"
)
env "${install_env[@]}" "$ROOT_DIR/tools/install_live_session.sh" "$first"
[[ "$(readlink "$PREFIX/current")" == releases/0001 ]]
[[ ! -e "$PREFIX/previous" ]]
grep -Fq "Exec=$PREFIX/current/bin/sophia-session" "$SESSION_DIR/sophia.desktop"
grep -Fq "Exec=$PREFIX/current/bin/sophia-kitty-session" \
    "$SESSION_DIR/sophia-kitty.desktop"
grep -Fq "Exec=$PREFIX/current/bin/sophia-firefox-proof" \
    "$SESSION_DIR/sophia-firefox-proof.desktop"
grep -Fq "Exec=$PREFIX/current/bin/sophia-recovery-proof" \
    "$SESSION_DIR/sophia-recovery-proof.desktop"
grep -Fq "Exec=$PREFIX/current/bin/sophia-native-chrome-proof" \
    "$SESSION_DIR/sophia-native-chrome-proof.desktop"
grep -Fq "Exec=$PREFIX/current/bin/sophia-run-cycles" \
    "$SESSION_DIR/sophia-cycle-proof.desktop"
for command in "${OPERATOR_COMMANDS[@]}"; do
    [[ "$(readlink "$COMMAND_DIR/$command")" == "$PREFIX/current/bin/$command" ]]
done

env "${install_env[@]}" "$ROOT_DIR/tools/install_live_session.sh" "$second"
[[ "$(readlink "$PREFIX/current")" == releases/0002 ]]
[[ "$(readlink "$PREFIX/previous")" == releases/0001 ]]
env SOPHIA_INSTALL_PREFIX="$PREFIX" "$COMMAND_DIR/sophia-rollback"
[[ "$(readlink "$PREFIX/current")" == releases/0001 ]]
[[ "$(readlink "$PREFIX/previous")" == releases/0002 ]]
[[ -f "$PREFIX/current/share/doc/sophia/operations.md" ]]
operator_state="$TEMP_DIR/operator-state"
install -d -m 700 "$operator_state/sophia/promotion/runs/0001"
install -d -m 700 "$operator_state/sophia/promotion/fallback-runs/0001"
install -d -m 700 "$operator_state/sophia/promotion/emergency-runs/0001"
install -d -m 700 "$operator_state/sophia/promotion/watchdog-runs/0001"
install -d -m 700 "$operator_state/sophia/promotion/native-chrome-runs/0001"
printf 'sophia_installed_cycle schema=1 status=passed exit_status=0\n' \
    >"$operator_state/sophia/promotion/runs/0001/result.kdl"
printf 'sophia_installed_fallback schema=1 status=passed exit_status=0\n' \
    >"$operator_state/sophia/promotion/fallback-runs/0001/result.kdl"
printf 'sophia_installed_emergency schema=1 status=passed exit_status=130\n' \
    >"$operator_state/sophia/promotion/emergency-runs/0001/result.kdl"
printf 'sophia_installed_watchdog schema=1 status=passed exit_status=124\n' \
    >"$operator_state/sophia/promotion/watchdog-runs/0001/result.kdl"
printf 'sophia_installed_native_chrome schema=1 status=passed exit_status=0\n' \
    >"$operator_state/sophia/promotion/native-chrome-runs/0001/result.kdl"
status_output="$(env \
    SOPHIA_INSTALL_PREFIX="$PREFIX" \
    XDG_STATE_HOME="$operator_state" \
    "$COMMAND_DIR/sophia-status")"
grep -Fq 'release_id=0001' <<<"$status_output"
grep -Fq "operator_guide=$PREFIX/current/share/doc/sophia/operations.md" \
    <<<"$status_output"
grep -Fq "latest_installed_cycle=$operator_state/sophia/promotion/runs/0001" \
    <<<"$status_output"
grep -Fq 'sophia_installed_cycle schema=1 status=passed exit_status=0' \
    <<<"$status_output"
grep -Fq \
    "latest_installed_fallback=$operator_state/sophia/promotion/fallback-runs/0001" \
    <<<"$status_output"
grep -Fq 'sophia_installed_fallback schema=1 status=passed exit_status=0' \
    <<<"$status_output"
grep -Fq \
    "latest_installed_emergency=$operator_state/sophia/promotion/emergency-runs/0001" \
    <<<"$status_output"
grep -Fq 'sophia_installed_emergency schema=1 status=passed exit_status=130' \
    <<<"$status_output"
grep -Fq \
    "latest_installed_watchdog=$operator_state/sophia/promotion/watchdog-runs/0001" \
    <<<"$status_output"
grep -Fq 'sophia_installed_watchdog schema=1 status=passed exit_status=124' \
    <<<"$status_output"
grep -Fq \
    "latest_installed_native_chrome=$operator_state/sophia/promotion/native-chrome-runs/0001" \
    <<<"$status_output"
grep -Fq 'sophia_installed_native_chrome schema=1 status=passed exit_status=0' \
    <<<"$status_output"

stop_runtime="$TEMP_DIR/stop-runtime"
stop_state="$stop_runtime/sophia-xmonad-session-$UID"
install -d -m 700 "$stop_state"
sleep 60 &
stop_pid=$!
printf '%s\n' "$stop_pid" >"$stop_state/wrapper.pid"
env XDG_RUNTIME_DIR="$stop_runtime" "$COMMAND_DIR/sophia-stop"
if kill -0 "$stop_pid" 2>/dev/null; then
    echo "installed Sophia stop command left the wrapper running" >&2
    exit 1
fi
wait "$stop_pid" 2>/dev/null || true
stop_pid=""

current_commit="$(git -C "$ROOT_DIR" rev-parse HEAD)"
current_version="$(awk -F'"' '$1 ~ /^version = / { print $2; exit }' "$ROOT_DIR/Cargo.toml")"
current_release="$current_version-${current_commit:0:12}"
current_artifact_root="$TEMP_DIR/current-artifacts"
current_artifact="$current_artifact_root/sophia-$current_release"
current_prefix="$TEMP_DIR/current/install/prefix"
current_session_dir="$TEMP_DIR/current/share/wayland-sessions"
current_command_dir="$TEMP_DIR/current/commands"
install -d -m 755 "$current_artifact_root"
cp -a "$first" "$current_artifact"
sed -i \
    -e "s/^commit=.*/commit=$current_commit/" \
    -e "s/^release_id=.*/release_id=$current_release/" \
    "$current_artifact/manifest"
(
    cd "$current_artifact"
    find bin share tools -type f -print0 | sort -z | xargs -0 sha256sum >SHA256SUMS
)
env \
    SOPHIA_ARTIFACT_ROOT="$current_artifact_root" \
    SOPHIA_INSTALL_PREFIX="$current_prefix" \
    SOPHIA_SESSION_DIR="$current_session_dir" \
    SOPHIA_COMMAND_DIR="$current_command_dir" \
    "$ROOT_DIR/tools/install_live_session.sh"
[[ "$(readlink "$current_prefix/current")" == "releases/$current_release" ]]
grep -Fxq "commit=$current_commit" "$current_prefix/current/manifest"
[[ -f "$current_prefix/current/share/doc/sophia/operations.md" ]]

echo "live-session staged install and rollback checks passed"
