#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TEMP_DIR"' EXIT
PREFIX="$TEMP_DIR/nested/install/prefix"
SESSION_DIR="$TEMP_DIR/nested/share/wayland-sessions"
COMMAND_DIR="$TEMP_DIR/nested/commands"
OPERATOR_COMMANDS=(
    sophia-session
    sophia-kitty-session
    sophia-firefox-proof
    sophia-recovery-proof
    sophia-status
    sophia-stop
    sophia-rollback
    sophia-record-run
    sophia-record-emergency-run
    sophia-record-watchdog-run
    sophia-record-firefox-run
    sophia-verify-login-cycle
    sophia-verify-cycles
    sophia-verify-firefox-runs
    sophia-verify-soak
)

make_artifact() {
    local release_id="$1" artifact command
    artifact="$TEMP_DIR/artifact-$release_id"
    install -d -m 755 "$artifact/bin" "$artifact/share/wayland-sessions"
    for command in "${OPERATOR_COMMANDS[@]}"; do
        case "$command" in
            sophia-status)
                cp "$ROOT_DIR/tools/status_live_session.sh" "$artifact/bin/$command"
                ;;
            sophia-rollback)
                cp "$ROOT_DIR/tools/rollback_live_session.sh" "$artifact/bin/$command"
                ;;
            *)
                printf '#!/usr/bin/env bash\nexit 0\n' >"$artifact/bin/$command"
                ;;
        esac
        chmod 755 "$artifact/bin/$command"
    done
    printf '[Desktop Entry]\nExec=@SOPHIA_INSTALL_PREFIX@/current/bin/sophia-session\n' \
        >"$artifact/share/wayland-sessions/sophia.desktop"
    printf '[Desktop Entry]\nExec=@SOPHIA_INSTALL_PREFIX@/current/bin/sophia-kitty-session\n' \
        >"$artifact/share/wayland-sessions/sophia-kitty.desktop"
    printf '[Desktop Entry]\nExec=@SOPHIA_INSTALL_PREFIX@/current/bin/sophia-firefox-proof\n' \
        >"$artifact/share/wayland-sessions/sophia-firefox-proof.desktop"
    printf '[Desktop Entry]\nExec=@SOPHIA_INSTALL_PREFIX@/current/bin/sophia-recovery-proof\n' \
        >"$artifact/share/wayland-sessions/sophia-recovery-proof.desktop"
    printf 'schema=1\nversion=0.1.0\ncommit=%040d\nrelease_id=%s\n' \
        "$release_id" "$release_id" >"$artifact/manifest"
    (
        cd "$artifact"
        find bin share -type f -print0 | sort -z | xargs -0 sha256sum >SHA256SUMS
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
for command in "${OPERATOR_COMMANDS[@]}"; do
    [[ "$(readlink "$COMMAND_DIR/$command")" == "$PREFIX/current/bin/$command" ]]
done

env "${install_env[@]}" "$ROOT_DIR/tools/install_live_session.sh" "$second"
[[ "$(readlink "$PREFIX/current")" == releases/0002 ]]
[[ "$(readlink "$PREFIX/previous")" == releases/0001 ]]
env SOPHIA_INSTALL_PREFIX="$PREFIX" "$COMMAND_DIR/sophia-rollback"
[[ "$(readlink "$PREFIX/current")" == releases/0001 ]]
[[ "$(readlink "$PREFIX/previous")" == releases/0002 ]]
status_output="$(env SOPHIA_INSTALL_PREFIX="$PREFIX" "$COMMAND_DIR/sophia-status")"
grep -Fq 'release_id=0001' <<<"$status_output"

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
    find bin share -type f -print0 | sort -z | xargs -0 sha256sum >SHA256SUMS
)
env \
    SOPHIA_ARTIFACT_ROOT="$current_artifact_root" \
    SOPHIA_INSTALL_PREFIX="$current_prefix" \
    SOPHIA_SESSION_DIR="$current_session_dir" \
    SOPHIA_COMMAND_DIR="$current_command_dir" \
    "$ROOT_DIR/tools/install_live_session.sh"
[[ "$(readlink "$current_prefix/current")" == "releases/$current_release" ]]
grep -Fxq "commit=$current_commit" "$current_prefix/current/manifest"

echo "live-session staged install and rollback checks passed"
