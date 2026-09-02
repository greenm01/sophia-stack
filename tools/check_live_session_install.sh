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
    sophia-xterm-proof
    sophia-truecolor-proof
    sophia-recovery-proof
    sophia-native-chrome-proof
    sophia-status
    sophia-stop
    sophia-soak-progress
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
    sophia-verify-xterm-runs
    sophia-verify-truecolor-runs
    sophia-verify-xmobar-work-area
    sophia-verify-soak
)

make_artifact() {
    local release_id="$1" artifact command
    artifact="$TEMP_DIR/artifact-$release_id"
    install -d -m 755 \
        "$artifact/bin" \
        "$artifact/share/doc/sophia" \
        "$artifact/share/sophia-policy/xmonad" \
        "$artifact/share/wayland-sessions" \
        "$artifact/target/release" \
        "$artifact/tools/fixtures" \
        "$artifact/tools/lib"
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
    install -m 644 "$ROOT_DIR/tools/lib/live_session_surface.sh" \
        "$artifact/tools/lib/live_session_surface.sh"
    install -m 755 "$ROOT_DIR/tools/verify_packaged_policy.sh" \
        "$artifact/tools/verify_packaged_policy.sh"
    printf '#!/usr/bin/env bash\necho xmonad_0.18.1\n' \
        >"$artifact/target/release/xmonad"
    printf '#!/usr/bin/env bash\necho xmobar_0.51.1\n' \
        >"$artifact/target/release/xmobar"
    chmod 755 "$artifact/target/release/xmonad" \
        "$artifact/target/release/xmobar"
    printf 'main = pure ()\n' >"$artifact/share/sophia-policy/xmonad/Main.hs"
    printf 'name: sophia-xmonad\n' \
        >"$artifact/share/sophia-policy/xmonad/sophia-xmonad.cabal"
    printf 'offline: True\n' >"$artifact/share/sophia-policy/xmonad/cabal.project"
    printf '/- kdl-version 2\nschema 2\n' \
        >"$artifact/share/sophia-policy/xmonad/core.kdl"
    printf '/- kdl-version 2\nschema 1\nshell enabled=#false\n' \
        >"$artifact/share/sophia-policy/xmonad/desktop.kdl"
    printf 'Config {}\n' >"$artifact/tools/fixtures/xmobar_sophia.config"
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
    xmonad_digest="$(sha256sum "$artifact/target/release/xmonad" | awk '{print $1}')"
    xmobar_digest="$(sha256sum "$artifact/target/release/xmobar" | awk '{print $1}')"
    xmonad_config_digest="$(sha256sum "$artifact/share/sophia-policy/xmonad/Main.hs" | awk '{print $1}')"
    xmonad_cabal_digest="$(sha256sum "$artifact/share/sophia-policy/xmonad/sophia-xmonad.cabal" | awk '{print $1}')"
    xmonad_project_digest="$(sha256sum "$artifact/share/sophia-policy/xmonad/cabal.project" | awk '{print $1}')"
    xmonad_core_config_digest="$(sha256sum "$artifact/share/sophia-policy/xmonad/core.kdl" | awk '{print $1}')"
    xmonad_desktop_profile_digest="$(sha256sum "$artifact/share/sophia-policy/xmonad/desktop.kdl" | awk '{print $1}')"
    xmobar_config_digest="$(sha256sum "$artifact/tools/fixtures/xmobar_sophia.config" | awk '{print $1}')"
    printf 'schema=4\nversion=0.1.0\ncommit=%040d\nrelease_id=%s\nxmonad_version=xmonad_0.18.1\nxmonad_source_version=0.18.1\nxmonad_contrib_source_version=0.18.2\nxmonad_config_sha256=%s\nxmonad_cabal_sha256=%s\nxmonad_project_sha256=%s\nxmonad_core_config_sha256=%s\nxmonad_desktop_profile_sha256=%s\nxmonad_binary_sha256=%s\nxmobar_version=xmobar_0.51.1\nxmobar_source_commit=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nxmobar_config_sha256=%s\nxmobar_binary_sha256=%s\nhagia_included=false\n' \
        "$release_id" "$release_id" "$xmonad_config_digest" "$xmonad_cabal_digest" \
        "$xmonad_project_digest" "$xmonad_core_config_digest" \
        "$xmonad_desktop_profile_digest" "$xmonad_digest" \
        "$xmobar_config_digest" "$xmobar_digest" >"$artifact/manifest"
    (
        cd "$artifact"
        find bin share target tools -type f -print0 | sort -z | xargs -0 sha256sum >SHA256SUMS
    )
    printf '%s\n' "$artifact"
}

first="$(make_artifact 0001)"
second="$(make_artifact 0002)"
hagia_artifact="$TEMP_DIR/artifact-hagia"
cp -a "$first" "$hagia_artifact"
for command in sophia-hagia-session sophia-hagia-promotion-session \
    sophia-record-hagia-run sophia-verify-hagia sophia-verify-hagia-promotion; do
    printf '#!/usr/bin/env bash\nexit 0\n' >"$hagia_artifact/bin/$command"
    chmod 755 "$hagia_artifact/bin/$command"
done
printf '#!/usr/bin/env bash\nexit 0\n' >"$hagia_artifact/target/release/hagia"
printf '#!/usr/bin/env bash\nexit 0\n' >"$hagia_artifact/target/release/hagia-shell"
printf '#!/usr/bin/env bash\nexit 0\n' >"$hagia_artifact/target/release/sophia"
chmod 755 "$hagia_artifact/target/release/hagia" \
    "$hagia_artifact/target/release/hagia-shell" \
    "$hagia_artifact/target/release/sophia"
install -d -m 755 "$hagia_artifact/share/sophia-policy/hagia"
printf 'schema 1\nshell { enabled #true; panel 28; }\n' \
    >"$hagia_artifact/share/sophia-policy/hagia/default.kdl"
printf '[Desktop Entry]\nExec=@SOPHIA_INSTALL_PREFIX@/current/bin/sophia-hagia-session\n' \
    >"$hagia_artifact/share/wayland-sessions/sophia-hagia.desktop"
printf '[Desktop Entry]\nExec=@SOPHIA_INSTALL_PREFIX@/current/bin/sophia-hagia-promotion-session\n' \
    >"$hagia_artifact/share/wayland-sessions/sophia-hagia-promotion.desktop"
hagia_digest="$(sha256sum "$hagia_artifact/target/release/hagia" | awk '{print $1}')"
hagia_shell_digest="$(sha256sum "$hagia_artifact/target/release/hagia-shell" | awk '{print $1}')"
hagia_profile_digest="$(sha256sum "$hagia_artifact/share/sophia-policy/hagia/default.kdl" | awk '{print $1}')"
sed -i 's/^schema=4$/schema=5/' "$hagia_artifact/manifest"
sed -i 's/^release_id=0001$/release_id=0003/' "$hagia_artifact/manifest"
sed -i 's/^hagia_included=false$/hagia_included=true/' "$hagia_artifact/manifest"
printf 'hagia_source_commit=%040d\nhagia_default_profile_sha256=%s\nhagia_binary_sha256=%s\nhagia_shell_binary_sha256=%s\n' \
    1 "$hagia_profile_digest" "$hagia_digest" "$hagia_shell_digest" \
    >>"$hagia_artifact/manifest"
(
    cd "$hagia_artifact"
    find bin share target tools -type f -print0 | sort -z | xargs -0 sha256sum >SHA256SUMS
)
expect_policy_rejection() {
    local artifact="$1" label="$2"
    if "$ROOT_DIR/tools/verify_packaged_policy.sh" "$artifact" >/dev/null 2>&1; then
        echo "packaged policy verifier accepted $label" >&2
        exit 1
    fi
}

invalid_hagia_shell="$TEMP_DIR/invalid-hagia-shell"
cp -a "$hagia_artifact" "$invalid_hagia_shell"
chmod 644 "$invalid_hagia_shell/target/release/hagia-shell"
expect_policy_rejection "$invalid_hagia_shell" "a missing executable Hagia Shell"

invalid_hagia_profile="$TEMP_DIR/invalid-hagia-profile"
cp -a "$hagia_artifact" "$invalid_hagia_profile"
printf 'policy { view-count 3; }\n' \
    >>"$invalid_hagia_profile/share/sophia-policy/hagia/default.kdl"
expect_policy_rejection "$invalid_hagia_profile" "a mutated Hagia default profile"

invalid_cabal="$TEMP_DIR/invalid-cabal"
cp -a "$first" "$invalid_cabal"
printf 'build-depends: unexpected\n' \
    >>"$invalid_cabal/share/sophia-policy/xmonad/sophia-xmonad.cabal"
expect_policy_rejection "$invalid_cabal" "a mutated xmonad build configuration"

invalid_desktop_profile="$TEMP_DIR/invalid-desktop-profile"
cp -a "$first" "$invalid_desktop_profile"
printf 'session-action id="untrusted"\n' \
    >>"$invalid_desktop_profile/share/sophia-policy/xmonad/desktop.kdl"
expect_policy_rejection "$invalid_desktop_profile" "a mutated xmonad desktop profile"

invalid_version="$TEMP_DIR/invalid-version"
cp -a "$first" "$invalid_version"
printf '#!/usr/bin/env bash\necho xmonad 9.0.0\n' \
    >"$invalid_version/target/release/xmonad"
invalid_version_digest="$(sha256sum "$invalid_version/target/release/xmonad" | awk '{print $1}')"
sed -i "s/^xmonad_binary_sha256=.*/xmonad_binary_sha256=$invalid_version_digest/" \
    "$invalid_version/manifest"
expect_policy_rejection "$invalid_version" "a wrong xmonad executable version"

invalid_source="$TEMP_DIR/invalid-source"
cp -a "$first" "$invalid_source"
sed -i 's/^xmonad_source_version=.*/xmonad_source_version=0.19.0/' \
    "$invalid_source/manifest"
expect_policy_rejection "$invalid_source" "an unapproved xmonad source version"

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
printf '[Desktop Entry]\nExec=/foreign-hagia-session\n' \
    >"$SESSION_DIR/sophia-hagia.desktop"
ln -s /foreign-hagia-command "$COMMAND_DIR/sophia-hagia-session"

env "${install_env[@]}" "$ROOT_DIR/tools/install_live_session.sh" "$second"
[[ "$(readlink "$PREFIX/current")" == releases/0002 ]]
[[ "$(readlink "$PREFIX/previous")" == releases/0001 ]]
env "${install_env[@]}" "$COMMAND_DIR/sophia-rollback"
[[ "$(readlink "$PREFIX/current")" == releases/0001 ]]
[[ "$(readlink "$PREFIX/previous")" == releases/0002 ]]
grep -Fqx 'Exec=/foreign-hagia-session' "$SESSION_DIR/sophia-hagia.desktop"
[[ "$(readlink "$COMMAND_DIR/sophia-hagia-session")" == /foreign-hagia-command ]]
[[ -f "$PREFIX/current/share/doc/sophia/operations.md" ]]
hagia_prefix="$TEMP_DIR/hagia/prefix"
hagia_sessions="$TEMP_DIR/hagia/sessions"
hagia_commands="$TEMP_DIR/hagia/commands"
env SOPHIA_INSTALL_PREFIX="$hagia_prefix" SOPHIA_SESSION_DIR="$hagia_sessions" \
    SOPHIA_COMMAND_DIR="$hagia_commands" \
    "$ROOT_DIR/tools/install_live_session.sh" "$first"
env SOPHIA_INSTALL_PREFIX="$hagia_prefix" SOPHIA_SESSION_DIR="$hagia_sessions" \
    SOPHIA_COMMAND_DIR="$hagia_commands" \
    "$ROOT_DIR/tools/install_live_session.sh" "$hagia_artifact"
[[ "$(readlink "$hagia_prefix/current")" == releases/0003 ]]
grep -Fq "Exec=$hagia_prefix/current/bin/sophia-hagia-session" \
    "$hagia_sessions/sophia-hagia.desktop"
grep -Fq "Exec=$hagia_prefix/current/bin/sophia-hagia-promotion-session" \
    "$hagia_sessions/sophia-hagia-promotion.desktop"
for command in sophia-hagia-session sophia-hagia-promotion-session \
    sophia-record-hagia-run sophia-verify-hagia sophia-verify-hagia-promotion; do
    [[ "$(readlink "$hagia_commands/$command")" == "$hagia_prefix/current/bin/$command" ]]
done
env SOPHIA_INSTALL_PREFIX="$hagia_prefix" SOPHIA_SESSION_DIR="$hagia_sessions" \
    SOPHIA_COMMAND_DIR="$hagia_commands" "$hagia_commands/sophia-rollback"
[[ "$(readlink "$hagia_prefix/current")" == releases/0001 ]]
[[ "$(readlink "$hagia_prefix/previous")" == releases/0003 ]]
for desktop in sophia-hagia sophia-hagia-promotion; do
    [[ ! -e "$hagia_sessions/$desktop.desktop" ]]
done
for command in sophia-hagia-session sophia-hagia-promotion-session \
    sophia-record-hagia-run sophia-verify-hagia sophia-verify-hagia-promotion; do
    [[ ! -e "$hagia_commands/$command" && ! -L "$hagia_commands/$command" ]]
done
env SOPHIA_INSTALL_PREFIX="$hagia_prefix" SOPHIA_SESSION_DIR="$hagia_sessions" \
    SOPHIA_COMMAND_DIR="$hagia_commands" "$hagia_commands/sophia-rollback"
[[ "$(readlink "$hagia_prefix/current")" == releases/0003 ]]
[[ "$(readlink "$hagia_prefix/previous")" == releases/0001 ]]
grep -Fq "Exec=$hagia_prefix/current/bin/sophia-hagia-session" \
    "$hagia_sessions/sophia-hagia.desktop"
grep -Fq "Exec=$hagia_prefix/current/bin/sophia-hagia-promotion-session" \
    "$hagia_sessions/sophia-hagia-promotion.desktop"
for command in sophia-hagia-session sophia-hagia-promotion-session \
    sophia-record-hagia-run sophia-verify-hagia sophia-verify-hagia-promotion; do
    [[ "$(readlink "$hagia_commands/$command")" == "$hagia_prefix/current/bin/$command" ]]
done
rm -f "$hagia_sessions/sophia-hagia-promotion.desktop"
ln -sfn /missing "$hagia_commands/sophia-hagia-promotion-session"
env SOPHIA_INSTALL_PREFIX="$hagia_prefix" SOPHIA_SESSION_DIR="$hagia_sessions" \
    SOPHIA_COMMAND_DIR="$hagia_commands" \
    "$ROOT_DIR/tools/activate_live_session_release.sh" \
    "$hagia_prefix/releases/0003"
[[ "$(readlink "$hagia_prefix/current")" == releases/0003 ]]
[[ "$(readlink "$hagia_prefix/previous")" == releases/0001 ]]
grep -Fq "Exec=$hagia_prefix/current/bin/sophia-hagia-promotion-session" \
    "$hagia_sessions/sophia-hagia-promotion.desktop"
[[ "$(readlink "$hagia_commands/sophia-hagia-promotion-session")" == \
    "$hagia_prefix/current/bin/sophia-hagia-promotion-session" ]]
env SOPHIA_INSTALL_PREFIX="$hagia_prefix" SOPHIA_SESSION_DIR="$hagia_sessions" \
    SOPHIA_COMMAND_DIR="$hagia_commands" \
    "$ROOT_DIR/tools/activate_live_session_release.sh" \
    "$hagia_prefix/releases/0003"
[[ "$(readlink "$hagia_prefix/current")" == releases/0003 ]]
[[ "$(readlink "$hagia_prefix/previous")" == releases/0001 ]]
if env SOPHIA_INSTALL_PREFIX="$hagia_prefix" \
    SOPHIA_SESSION_DIR="$hagia_sessions" SOPHIA_COMMAND_DIR="$hagia_commands" \
    "$ROOT_DIR/tools/activate_live_session_release.sh" "$hagia_artifact" \
    >/dev/null 2>&1; then
    echo "activation accepted an artifact outside the immutable install prefix" >&2
    exit 1
fi
operator_state="$TEMP_DIR/operator-state"
install -d -m 700 "$operator_state/sophia/promotion/runs/0001"
install -d -m 700 "$operator_state/sophia/promotion/xterm-runs/0001"
install -d -m 700 "$operator_state/sophia/promotion/truecolor-runs/0001"
install -d -m 700 "$operator_state/sophia/promotion/fallback-runs/0001"
install -d -m 700 "$operator_state/sophia/promotion/emergency-runs/0001"
install -d -m 700 "$operator_state/sophia/promotion/watchdog-runs/0001"
install -d -m 700 "$operator_state/sophia/promotion/native-chrome-runs/0001"
install -d -m 700 "$operator_state/sophia/promotion/hagia-runs/0001"
printf 'sophia_installed_cycle schema=1 status=passed exit_status=0\n' \
    >"$operator_state/sophia/promotion/runs/0001/result.kdl"
printf 'sophia_installed_xterm schema=1 status=passed exit_status=0\n' \
    >"$operator_state/sophia/promotion/xterm-runs/0001/result.kdl"
printf 'sophia_installed_truecolor schema=1 status=passed exit_status=0\n' \
    >"$operator_state/sophia/promotion/truecolor-runs/0001/result.kdl"
printf 'sophia_installed_fallback schema=1 status=passed exit_status=0\n' \
    >"$operator_state/sophia/promotion/fallback-runs/0001/result.kdl"
printf 'sophia_installed_emergency schema=1 status=passed exit_status=130\n' \
    >"$operator_state/sophia/promotion/emergency-runs/0001/result.kdl"
printf 'sophia_installed_watchdog schema=1 status=passed exit_status=124\n' \
    >"$operator_state/sophia/promotion/watchdog-runs/0001/result.kdl"
printf 'sophia_installed_native_chrome schema=1 status=passed exit_status=0\n' \
    >"$operator_state/sophia/promotion/native-chrome-runs/0001/result.kdl"
printf 'sophia_installed_hagia schema=1 status=passed exit_status=0\n' \
    >"$operator_state/sophia/promotion/hagia-runs/0001/result.kdl"
printf 'sophia_hagia_coverage schema=1 terminal_starts=1 firefox_starts=0 physical_actions=2 session_actions=1 pointer_moves=0 pointer_resizes=0 checkpoints=0 reconciliations=0 output_changes=0 topology_changes=0\n' \
    >"$operator_state/sophia/promotion/hagia-runs/0001/coverage.kdl"
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
    "latest_installed_xterm=$operator_state/sophia/promotion/xterm-runs/0001" \
    <<<"$status_output"
grep -Fq 'sophia_installed_xterm schema=1 status=passed exit_status=0' \
    <<<"$status_output"
grep -Fq \
    "latest_installed_truecolor=$operator_state/sophia/promotion/truecolor-runs/0001" \
    <<<"$status_output"
grep -Fq 'sophia_installed_truecolor schema=1 status=passed exit_status=0' \
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
grep -Fq \
    "latest_installed_hagia=$operator_state/sophia/promotion/hagia-runs/0001" \
    <<<"$status_output"
grep -Fq 'hagia_passed_runs=1' <<<"$status_output"
grep -Fq 'hagia_scenario_physical_actions_sessions=1' <<<"$status_output"

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
    find bin share target tools -type f -print0 | sort -z | xargs -0 sha256sum >SHA256SUMS
)
env SOPHIA_INSTALL_PREFIX="$current_prefix" \
    SOPHIA_SESSION_DIR="$current_session_dir" \
    SOPHIA_COMMAND_DIR="$current_command_dir" \
    "$ROOT_DIR/tools/install_live_session.sh" "$first"
env \
    SOPHIA_ARTIFACT_ROOT="$current_artifact_root" \
    SOPHIA_INSTALL_PREFIX="$current_prefix" \
    SOPHIA_SESSION_DIR="$current_session_dir" \
    SOPHIA_COMMAND_DIR="$current_command_dir" \
    "$ROOT_DIR/tools/install_live_session.sh"
[[ "$(readlink "$current_prefix/current")" == "releases/$current_release" ]]
grep -Fxq "commit=$current_commit" "$current_prefix/current/manifest"
[[ -f "$current_prefix/current/share/doc/sophia/operations.md" ]]
env SOPHIA_INSTALL_PREFIX="$current_prefix" \
    SOPHIA_SESSION_DIR="$current_session_dir" \
    SOPHIA_COMMAND_DIR="$current_command_dir" \
    "$current_command_dir/sophia-rollback"
[[ "$(readlink "$current_prefix/current")" == releases/0001 ]]
env \
    SOPHIA_ARTIFACT_ROOT="$current_artifact_root" \
    SOPHIA_INSTALL_PREFIX="$current_prefix" \
    SOPHIA_SESSION_DIR="$current_session_dir" \
    SOPHIA_COMMAND_DIR="$current_command_dir" \
    "$ROOT_DIR/tools/install_live_session.sh"
[[ "$(readlink "$current_prefix/current")" == "releases/$current_release" ]]
[[ "$(readlink "$current_prefix/previous")" == releases/0001 ]]

echo "live-session staged install and rollback checks passed"
