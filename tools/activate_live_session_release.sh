#!/usr/bin/env bash
set -euo pipefail

(( $# == 1 )) || {
    echo "usage: tools/activate_live_session_release.sh INSTALLED_RELEASE_DIR" >&2
    exit 1
}

release_input="$1"
PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"
SESSION_DIR="${SOPHIA_SESSION_DIR:-/usr/share/wayland-sessions}"
COMMAND_DIR="${SOPHIA_COMMAND_DIR:-/usr/local/bin}"

require_writable_parent() {
    local path="$1" ancestor="$1" parent
    while [[ ! -e "$ancestor" ]]; do
        parent="$(dirname "$ancestor")"
        [[ "$parent" != "$ancestor" ]] || break
        ancestor="$parent"
    done
    [[ -d "$ancestor" && -w "$ancestor" ]] || {
        echo "Activation requires root for $path; run with sudo." >&2
        exit 1
    }
}

if [[ "$(id -u)" != 0 ]]; then
    require_writable_parent "$PREFIX"
    require_writable_parent "$SESSION_DIR"
    require_writable_parent "$COMMAND_DIR"
fi

[[ -d "$PREFIX" ]] || {
    echo "Sophia install prefix does not exist: $PREFIX" >&2
    exit 1
}
[[ -d "$release_input" && ! -L "$release_input" ]] || {
    echo "Installed release is not a real directory: $release_input" >&2
    exit 1
}

PREFIX="$(cd "$PREFIX" && pwd -P)"
release="$(cd "$release_input" && pwd -P)"
releases="$PREFIX/releases"
release_id="$(awk -F= '$1 == "release_id" { print $2; exit }' "$release/manifest")"
[[ "$release_id" =~ ^[0-9A-Za-z._-]+$ ]] || {
    echo "Installed release has an invalid release_id." >&2
    exit 1
}
[[ "$release" == "$releases/$release_id" ]] || {
    echo "Refusing to activate a directory outside its immutable release path." >&2
    echo "Expected: $releases/$release_id" >&2
    echo "Observed: $release" >&2
    exit 1
}

hagia_included="$(awk -F= '$1 == "hagia_included" { print $2; exit }' "$release/manifest")"
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
    cd "$release"
    sha256sum -c SHA256SUMS
)
"$release/tools/verify_packaged_policy.sh" "$release"

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

desktops=(
    sophia sophia-kitty sophia-firefox-proof sophia-recovery-proof
    sophia-native-chrome-proof sophia-cycle-proof
)
if [[ "$hagia_included" == true ]]; then
    desktops+=(sophia-hagia sophia-hagia-promotion)
fi

# Finish every release-specific check before changing the active installation.
for command in "${commands[@]}"; do
    [[ -x "$release/bin/$command" ]] || {
        echo "Release is missing operator command: $command" >&2
        exit 1
    }
done
for desktop in "${desktops[@]}"; do
    [[ -f "$release/share/wayland-sessions/$desktop.desktop" ]] || {
        echo "Release is missing session entry: $desktop.desktop" >&2
        exit 1
    }
done

install -d -m 755 "$SESSION_DIR" "$COMMAND_DIR"
for command in "${commands[@]}"; do
    ln -sfn "$PREFIX/current/bin/$command" "$COMMAND_DIR/$command"
done

sed_prefix="${PREFIX//\\/\\\\}"
sed_prefix="${sed_prefix//&/\\&}"
sed_prefix="${sed_prefix//|/\\|}"
desktop_temps=()
current_temp=""
previous_temp=""
cleanup() {
    local path
    for path in "${desktop_temps[@]}" "$current_temp" "$previous_temp"; do
        [[ -z "$path" ]] || rm -f -- "$path"
    done
}
trap cleanup EXIT
for desktop in "${desktops[@]}"; do
    desktop_temp="$SESSION_DIR/.$desktop.desktop.$$"
    desktop_temps+=("$desktop_temp")
    sed "s|@SOPHIA_INSTALL_PREFIX@|$sed_prefix|g" \
        "$release/share/wayland-sessions/$desktop.desktop" >"$desktop_temp"
    chmod 644 "$desktop_temp"
    mv -f "$desktop_temp" "$SESSION_DIR/$desktop.desktop"
done

old_current="$(readlink "$PREFIX/current" 2>/dev/null || true)"
old_current_path="$(readlink -f "$PREFIX/current" 2>/dev/null || true)"
if [[ "$old_current_path" != "$release" ]]; then
    if [[ -n "$old_current" && -d "$old_current_path" && "$old_current_path" == "$releases/"* ]]; then
        previous_temp="$PREFIX/.previous.$$"
        ln -s "$old_current" "$previous_temp"
        mv -Tf "$previous_temp" "$PREFIX/previous"
    elif [[ -n "$old_current" ]]; then
        echo "Warning: not retaining invalid current release as previous: $old_current" >&2
    fi
    current_temp="$PREFIX/.current.$$"
    ln -s "releases/$release_id" "$current_temp"
    mv -Tf "$current_temp" "$PREFIX/current"
fi
trap - EXIT

echo "Activated Sophia release: $release_id"
echo "Current: $PREFIX/current"
echo "Session entries:"
for desktop in "${desktops[@]}"; do
    echo "  $SESSION_DIR/$desktop.desktop"
done
echo "Operator commands: ${commands[*]}"
