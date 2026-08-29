#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
wm=xmonad
if [[ "${1:-}" == --wm=* ]]; then
    wm="${1#--wm=}"
    shift
fi
case "$wm" in
    xmonad|native|hagia|none) ;;
    *) echo "--wm expects xmonad, native, hagia, or none" >&2; exit 1 ;;
esac

resolve_program() {
    local configured=$1
    local fallback=$2
    if [[ -n "$configured" && -x "$configured" ]]; then
        printf '%s\n' "$configured"
        return
    fi
    command -v "$fallback" || true
}

sophia_bin="$(resolve_program "${SOPHIA_BIN:-$ROOT_DIR/target/debug/sophia}" sophia)"
terminal_bin="$(resolve_program "${SOPHIA_TERMINAL_BIN:-}" kitty)"
if [[ -z "$sophia_bin" || -z "$terminal_bin" ]]; then
    echo "Sophia and kitty must be built or installed before starting a normal session." >&2
    exit 1
fi

wm_args=()
case "$wm" in
    xmonad)
        bridge="$(resolve_program "${SOPHIA_X11_WM_BRIDGE_BIN:-$ROOT_DIR/target/debug/sophia-x11-wm-bridge}" sophia-x11-wm-bridge)"
        xmonad="$(resolve_program "${SOPHIA_XMONAD_BIN:-}" xmonad)"
        if [[ -z "$bridge" || -z "$xmonad" ]]; then
            echo "The xmonad selection requires sophia-x11-wm-bridge and xmonad." >&2
            exit 1
        fi
        wm_args=(
            "--wm-process=$bridge"
            "--wm-process-arg=--wm=$xmonad"
            "--wm-process-arg=--profile=xmonad"
            "--wm-process-arg=--wm-private-alias=xmonad/xmonad-x86_64-linux"
        )
        ;;
    native)
        # No window manager. `sophia-wm-demo` lost its serving mode in
        # 83596bfc with the experimental WM API v7, so naming it here made
        # every native session exit on a usage string. Hagia is Sophia's
        # native WM; this selection proves the session and its action
        # mappings, which are session-level and need no policy client.
        wm_args=()
        ;;
    hagia)
        hagia="$(resolve_program "${SOPHIA_HAGIA_BIN:-}" hagia)"
        if [[ -z "$hagia" ]]; then
            echo "The Hagia selection requires a hagia executable." >&2
            exit 1
        fi
        wm_args=(
            "--wm-process=$hagia"
            --wm-interface=sophia_wm_v1
        )
        ;;
esac

session_args=(
    session
    run
    --session-mode=normal
    "--session-app=terminal=$terminal_bin"
    --session-start=terminal
)
if [[ "$wm" != hagia ]]; then
    session_args+=(--session-action-app=terminal=terminal)
fi
firefox_bin="$(resolve_program "${SOPHIA_FIREFOX_BIN:-}" firefox)"
if [[ -n "$firefox_bin" ]]; then
    if [[ "$wm" == hagia ]]; then
        session_args+=("--session-app=browser=$firefox_bin")
    else
        session_args+=(
            "--session-app=firefox=$firefox_bin"
            --session-action-app=browser=firefox
        )
    fi
fi
session_args+=("${wm_args[@]}")
session_args+=("$@")
exec "$sophia_bin" "${session_args[@]}"
