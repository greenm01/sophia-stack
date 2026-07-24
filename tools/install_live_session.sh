#!/usr/bin/env bash
set -euo pipefail

artifact="${1:-}"
[[ -n "$artifact" && -d "$artifact" ]] || {
    echo "usage: tools/install_live_session.sh ARTIFACT_DIR" >&2
    exit 1
}
artifact="$(cd "$artifact" && pwd)"
PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"
SESSION_DIR="${SOPHIA_SESSION_DIR:-/usr/local/share/wayland-sessions}"
COMMAND_DIR="${SOPHIA_COMMAND_DIR:-/usr/local/bin}"

[[ "$(id -u)" == 0 || -w "$(dirname "$PREFIX")" ]] || {
    echo "Installation requires root for $PREFIX; run with sudo." >&2
    exit 1
}
release_id="$(sed -n 's/^release_id=//p' "$artifact/manifest" | head -n 1)"
[[ "$release_id" =~ ^[0-9A-Za-z._-]+$ ]] || {
    echo "Artifact has an invalid release_id." >&2
    exit 1
}
(
    cd "$artifact"
    sha256sum -c SHA256SUMS
)

releases="$PREFIX/releases"
target="$releases/$release_id"
install -d -m 755 "$releases" "$SESSION_DIR" "$COMMAND_DIR"
[[ ! -e "$target" ]] || {
    echo "Release is already installed: $target" >&2
    exit 1
}
staging="$releases/.install-$release_id-$$"
trap '[[ ! -d "$staging" ]] || mv "$staging" "$staging.failed"' EXIT
cp -a "$artifact" "$staging"
mv "$staging" "$target"

old_current="$(readlink "$PREFIX/current" 2>/dev/null || true)"
if [[ -n "$old_current" ]]; then
    ln -sfn "$old_current" "$PREFIX/previous"
fi
ln -sfn "releases/$release_id" "$PREFIX/current"
commands=(
    sophia-session
    sophia-status
    sophia-rollback
    sophia-record-run
    sophia-verify-cycles
    sophia-verify-soak
)
for command in "${commands[@]}"; do
    [[ -x "$target/bin/$command" ]] || {
        echo "Release is missing operator command: $command" >&2
        exit 1
    }
    ln -sfn "$PREFIX/current/bin/$command" "$COMMAND_DIR/$command"
done
install -m 644 "$target/share/wayland-sessions/sophia.desktop" \
    "$SESSION_DIR/sophia.desktop.template"
sed "s|@SOPHIA_INSTALL_PREFIX@|$PREFIX|g" \
    "$SESSION_DIR/sophia.desktop.template" >"$SESSION_DIR/sophia.desktop"
chmod 644 "$SESSION_DIR/sophia.desktop"
rm -f "$SESSION_DIR/sophia.desktop.template"
trap - EXIT

echo "Installed Sophia release: $release_id"
echo "Current: $PREFIX/current"
echo "Session entry: $SESSION_DIR/sophia.desktop"
echo "Operator commands: ${commands[*]}"
