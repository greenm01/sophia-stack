#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if (( $# == 0 )); then
    exec "$ROOT_DIR/tools/install_current_live_session.sh"
fi
(( $# == 1 )) || {
    echo "usage: tools/install_live_session.sh [ARTIFACT_DIR]" >&2
    exit 1
}
artifact="$1"
[[ -d "$artifact" ]] || {
    echo "Artifact directory does not exist: $artifact" >&2
    exit 1
}
artifact="$(cd "$artifact" && pwd)"
PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"
SESSION_DIR="${SOPHIA_SESSION_DIR:-/usr/share/wayland-sessions}"
COMMAND_DIR="${SOPHIA_COMMAND_DIR:-/usr/local/bin}"

if [[ "$(id -u)" != 0 ]]; then
    writable_ancestor="$PREFIX"
    while [[ ! -e "$writable_ancestor" ]]; do
        parent="$(dirname "$writable_ancestor")"
        [[ "$parent" != "$writable_ancestor" ]] || break
        writable_ancestor="$parent"
    done
    [[ -d "$writable_ancestor" && -w "$writable_ancestor" ]] || {
        echo "Installation requires root for $PREFIX; run with sudo." >&2
        exit 1
    }
fi
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
    sophia-kitty-session
    sophia-firefox-proof
    sophia-status
    sophia-stop
    sophia-rollback
    sophia-record-run
    sophia-record-emergency-run
    sophia-record-firefox-run
    sophia-verify-cycles
    sophia-verify-firefox-runs
    sophia-verify-soak
)
for command in "${commands[@]}"; do
    [[ -x "$target/bin/$command" ]] || {
        echo "Release is missing operator command: $command" >&2
        exit 1
    }
    ln -sfn "$PREFIX/current/bin/$command" "$COMMAND_DIR/$command"
done
for desktop in sophia sophia-kitty sophia-firefox-proof; do
    install -m 644 "$target/share/wayland-sessions/$desktop.desktop" \
        "$SESSION_DIR/$desktop.desktop.template"
    sed "s|@SOPHIA_INSTALL_PREFIX@|$PREFIX|g" \
        "$SESSION_DIR/$desktop.desktop.template" >"$SESSION_DIR/$desktop.desktop"
    chmod 644 "$SESSION_DIR/$desktop.desktop"
    rm -f "$SESSION_DIR/$desktop.desktop.template"
done
trap - EXIT

echo "Installed Sophia release: $release_id"
echo "Current: $PREFIX/current"
echo "Session entries: $SESSION_DIR/sophia.desktop $SESSION_DIR/sophia-kitty.desktop $SESSION_DIR/sophia-firefox-proof.desktop"
echo "Operator commands: ${commands[*]}"
