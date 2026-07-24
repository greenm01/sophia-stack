#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TEMP_DIR"' EXIT
PREFIX="$TEMP_DIR/prefix"
SESSION_DIR="$TEMP_DIR/sessions"
COMMAND_DIR="$TEMP_DIR/commands"

make_artifact() {
    local release_id="$1" artifact command
    artifact="$TEMP_DIR/artifact-$release_id"
    install -d -m 755 "$artifact/bin" "$artifact/share/wayland-sessions"
    for command in \
        sophia-session sophia-status sophia-rollback sophia-record-run \
        sophia-verify-cycles sophia-verify-soak; do
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
for command in \
    sophia-session sophia-status sophia-rollback sophia-record-run \
    sophia-verify-cycles sophia-verify-soak; do
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

echo "live-session staged install and rollback checks passed"
