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
hagia_included="$(sed -n 's/^hagia_included=//p' "$artifact/manifest" | head -n 1)"
if [[ "$hagia_included" == true ]]; then
    [[ -x /usr/bin/bwrap ]] || {
        echo "Hagia requires Bubblewrap at /usr/bin/bwrap (minimum 0.11.2)." >&2
        exit 1
    }
    bwrap_version="$(/usr/bin/bwrap --version 2>/dev/null || true)"
    [[ "$bwrap_version" =~ ^bubblewrap\ ([0-9]+)\.([0-9]+)\.([0-9]+)$ ]] || {
        echo "Could not verify the installed Bubblewrap version: $bwrap_version" >&2
        exit 1
    }
    bwrap_major="${BASH_REMATCH[1]}"
    bwrap_minor="${BASH_REMATCH[2]}"
    bwrap_patch="${BASH_REMATCH[3]}"
    if (( bwrap_major == 0 && (bwrap_minor < 11 || (bwrap_minor == 11 && bwrap_patch < 2)) )); then
        echo "Hagia requires Bubblewrap 0.11.2 or newer; found $bwrap_version." >&2
        exit 1
    fi
fi
(
    cd "$artifact"
    sha256sum -c SHA256SUMS
)
"$artifact/tools/verify_packaged_policy.sh" "$artifact"

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
if [[ "$hagia_included" == true ]]; then
    commands+=(
        sophia-hagia-session
        sophia-hagia-promotion-session
        sophia-record-hagia-run
        sophia-verify-hagia
        sophia-verify-hagia-promotion
    )
fi
for command in "${commands[@]}"; do
    [[ -x "$target/bin/$command" ]] || {
        echo "Release is missing operator command: $command" >&2
        exit 1
    }
    ln -sfn "$PREFIX/current/bin/$command" "$COMMAND_DIR/$command"
done
desktops=(
    sophia sophia-kitty sophia-firefox-proof sophia-recovery-proof
    sophia-native-chrome-proof sophia-cycle-proof
)
if [[ "$hagia_included" == true ]]; then
    desktops+=(sophia-hagia sophia-hagia-promotion)
fi
for desktop in "${desktops[@]}"; do
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
echo "Operator guide: $PREFIX/current/share/doc/sophia/operations.md"
echo "Session entries:"
for desktop in "${desktops[@]}"; do
    echo "  $SESSION_DIR/$desktop.desktop"
done
echo "Operator commands: ${commands[*]}"
