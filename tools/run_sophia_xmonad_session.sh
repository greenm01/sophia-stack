#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOPHIA_BIN="${SOPHIA_BIN:-$ROOT_DIR/target/release/sophia}"
SOPHIA_WM_BRIDGE_BIN="${SOPHIA_X11_WM_BRIDGE_BIN:-$ROOT_DIR/target/release/sophia-x11-wm-bridge}"
TTY_MODE_HELPER="${SOPHIA_TTY_MODE_HELPER:-$ROOT_DIR/tools/sophia_tty_mode.py}"
BUILD_SESSION="${SOPHIA_BUILD_SESSION:-true}"
MANAGE_KEYD="${SOPHIA_MANAGE_KEYD:-true}"
INSTALLED_SESSION="${SOPHIA_INSTALLED_SESSION:-false}"
REQUIRE_RUNTIME_DIR="${SOPHIA_REQUIRE_RUNTIME_DIR:-false}"
REQUIRE_LOCAL_VT="${SOPHIA_REQUIRE_LOCAL_VT:-false}"
DISPLAY_NAME="${SOPHIA_LIVE_SESSION_DISPLAY:-:77}"
SESSION_PROFILE="${SOPHIA_TTY_PROFILE:-xmonad}"
if [[ "$SESSION_PROFILE" != xmonad && "$SESSION_PROFILE" != kitty ]]; then
    echo "SOPHIA_TTY_PROFILE must be xmonad or kitty." >&2
    exit 1
fi
SESSION_LABEL="Sophia $SESSION_PROFILE session"
runtime_root="${XDG_RUNTIME_DIR:-/tmp}"
if [[ ! -t 0 ]]; then
    echo "Run this interactively from a dedicated local TTY." >&2
    exit 1
fi
tty_name="$(tty 2>/dev/null || true)"
if [[ "$REQUIRE_RUNTIME_DIR" == true ]]; then
    [[ -n "${XDG_RUNTIME_DIR:-}" && -d "$XDG_RUNTIME_DIR" ]] || {
        echo "Installed Sophia requires an existing XDG_RUNTIME_DIR." >&2
        exit 1
    }
    [[ "$XDG_RUNTIME_DIR" == /* && "$(stat -c %u "$XDG_RUNTIME_DIR")" == "$UID" ]] || {
        echo "Installed Sophia requires an absolute, user-owned XDG_RUNTIME_DIR." >&2
        exit 1
    }
fi
if [[ "$REQUIRE_LOCAL_VT" == true && ! "$tty_name" =~ ^/dev/tty[0-9]+$ ]]; then
    echo "Installed Sophia requires a local Linux VT; observed: $tty_name" >&2
    exit 1
fi
if [[ "$INSTALLED_SESSION" == true
    && ( "$BUILD_SESSION" != false || "$MANAGE_KEYD" != false ) ]]; then
    echo "Installed Sophia forbids source builds and manual service control." >&2
    exit 1
fi
STATE_DIR="$runtime_root/sophia-${SESSION_PROFILE}-session-${UID}"
PID_FILE="$STATE_DIR/wrapper.pid"
LOG_DIR="${XDG_STATE_HOME:-${HOME}/.local/state}/sophia/${SESSION_PROFILE}-session"
GUARD_LOG="$LOG_DIR/input-guard.log"
RECOVERY_LOG="$LOG_DIR/recovery.log"
SESSION_LOG="$LOG_DIR/session.log"
LIFECYCLE_LOG="$LOG_DIR/lifecycle.log"
GUARD_ARMED_FILE="$STATE_DIR/input-guard.armed"
GUARD_TRIGGERED_FILE="$STATE_DIR/input-guard.triggered"

mkdir -p "$STATE_DIR"
chmod 700 "$STATE_DIR"
mkdir -p "$LOG_DIR"
chmod 700 "$LOG_DIR"
[[ ! -f "$LIFECYCLE_LOG" ]] || mv -f "$LIFECYCLE_LOG" "$LIFECYCLE_LOG.previous"
: >"$LIFECYCLE_LOG"
chmod 600 "$LIFECYCLE_LOG"
lifecycle_phase() {
    printf 'sophia_session_lifecycle schema=1 status=%s phase=%s installed=%s build=%s manual_service=%s runtime=%s vt=%s\n' \
        "$1" "$2" "$INSTALLED_SESSION" "$BUILD_SESSION" "$MANAGE_KEYD" \
        "$([[ "$runtime_root" == /tmp ]] && echo temporary || echo owner)" \
        "$([[ "$tty_name" =~ ^/dev/tty[0-9]+$ ]] && echo local || echo other)" \
        >>"$LIFECYCLE_LOG"
}
lifecycle_phase entering preflight
if [[ -s "$PID_FILE" ]]; then
    previous_pid="$(<"$PID_FILE")"
    if [[ "$previous_pid" =~ ^[0-9]+$ ]] && kill -0 "$previous_pid" 2>/dev/null; then
        echo "A $SESSION_LABEL is already running (wrapper PID $previous_pid)." >&2
        echo "Stop it with: tools/stop_sophia_${SESSION_PROFILE}_session.sh" >&2
        exit 1
    fi
    rm -f "$PID_FILE"
fi

live_named_processes() {
    local name pid state
    for name in "$@"; do
        while read -r pid; do
            [[ -n "$pid" ]] || continue
            state="$(ps -o stat= -p "$pid" 2>/dev/null || true)"
            [[ "$state" == Z* ]] || printf '%s:%s\n' "$name" "$pid"
        done < <(pgrep -x "$name" 2>/dev/null || true)
    done
}
active_sessions=()
for process in river niri sway Hyprland kwin_wayland Xorg; do
    while read -r active; do
        [[ -n "$active" ]] && active_sessions+=("$active")
    done < <(live_named_processes "$process")
done
if (( ${#active_sessions[@]} > 0 )); then
    echo "Refusing to take over a TTY while a graphical session is active." >&2
    echo "Still active (process:pid): ${active_sessions[*]}" >&2
    exit 1
fi

input_seat="${SOPHIA_OPERATOR_INPUT_SEAT:-seat0}"
input_devices="${SOPHIA_OPERATOR_INPUT_DEVICES:-}"
input_source_args=()
if [[ -n "$input_devices" ]]; then
    input_source_args+=("--input-devices=$input_devices")
else
    input_source_args+=("--input-seat=$input_seat")
fi

xmonad_bin=""
xmobar_bin=""
if [[ "$SESSION_PROFILE" == xmonad ]]; then
    xmonad_bin="$("$ROOT_DIR/tools/resolve_sophia_xmonad.sh")"
    if [[ "${SOPHIA_ENABLE_XMOBAR:-auto}" != false ]]; then
        xmobar_bin="$("$ROOT_DIR/tools/resolve_sophia_xmobar.sh" || true)"
        if [[ "${SOPHIA_ENABLE_XMOBAR:-auto}" == true && -z "$xmobar_bin" ]]; then
            echo "Xmobar was requested but no executable was found." >&2
            echo "Build ~/src/xmobar or set SOPHIA_XMOBAR_BIN." >&2
            exit 1
        fi
    fi
fi

cd "$ROOT_DIR"
if [[ "$BUILD_SESSION" == true ]]; then
    cargo build --offline --release -p sophia-cli --features atomic-scanout-live
    if [[ "$SESSION_PROFILE" == xmonad ]]; then
        cargo build --offline --release -p sophia-x11-wm-bridge
    fi
    tools/atomic_scanout_preflight.sh
fi
[[ -x "$SOPHIA_BIN" ]] || {
    echo "Sophia session binary is not executable: $SOPHIA_BIN" >&2
    exit 1
}
if [[ "$SESSION_PROFILE" == xmonad && ! -x "$SOPHIA_WM_BRIDGE_BIN" ]]; then
    echo "Sophia WM bridge is not executable: $SOPHIA_WM_BRIDGE_BIN" >&2
    exit 1
fi
lifecycle_phase complete preflight

keyd_was_running=false
tty_state=""
kd_mode=""
keyboard_mode=""
guard_pid=""
session_pid=""
cleanup_done=false
emergency_session_shutdown=not_requested
emergency_session_exit_status=none
terminate_bounded() {
    local target="$1" label="$2"
    if ! kill -0 -- "$target" 2>/dev/null; then
        return 0
    fi
    kill -TERM -- "$target" 2>/dev/null || true
    for _ in {1..40}; do
        if ! kill -0 -- "$target" 2>/dev/null; then
            wait "${target#-}" 2>/dev/null || true
            return 0
        fi
        sleep 0.05
    done
    echo "WARNING: $label did not stop after TERM; sending KILL." >&2
    kill -KILL -- "$target" 2>/dev/null || true
    wait "${target#-}" 2>/dev/null || true
}
cleanup() {
    local status=$?
    if [[ "$cleanup_done" == true ]]; then
        return "$status"
    fi
    cleanup_done=true
    local emergency=false
    [[ ! -s "$GUARD_TRIGGERED_FILE" ]] || emergency=true
    [[ -z "$session_pid" ]] || terminate_bounded "-$session_pid" "$SESSION_LABEL"
    session_pid=""
    [[ -z "$guard_pid" ]] || terminate_bounded "$guard_pid" "Sophia input guard"
    guard_pid=""
    rm -f "$PID_FILE"
    if [[ -n "$kd_mode" ]]; then
        python3 "$TTY_MODE_HELPER" "$kd_mode" 2>/dev/null || status=1
    fi
    if [[ -n "$keyboard_mode" ]]; then
        python3 "$TTY_MODE_HELPER" "keyboard-$keyboard_mode" 2>/dev/null || status=1
    fi
    if [[ -n "$tty_state" ]]; then
        stty "$tty_state" 2>/dev/null || status=1
    fi
    if [[ "$keyd_was_running" == true ]]; then
        echo
        echo "Restoring keyd..."
        if ! sudo sv up keyd; then
            echo "WARNING: keyd could not be restored; run: sudo sv up keyd" >&2
            status=1
        fi
    fi
    rm -f "$GUARD_ARMED_FILE" "$GUARD_TRIGGERED_FILE"
    if [[ -n "$kd_mode" && -n "$tty_state" ]]; then
        local restored_kd restored_termios
        restored_kd="$(python3 "$TTY_MODE_HELPER" get 2>/dev/null || echo unavailable)"
        restored_termios="$(stty -g 2>/dev/null || echo unavailable)"
        printf 'sophia_tty_recovery schema=3 profile=%s kd_mode_before=%s kd_mode_after=%s termios_restored=%s emergency=%s session_shutdown=%s session_exit_status=%s\n' \
            "$SESSION_PROFILE" \
            "$kd_mode" "$restored_kd" \
            "$([[ "$restored_termios" == "$tty_state" ]] && echo true || echo false)" \
            "$emergency" \
            "$emergency_session_shutdown" \
            "$emergency_session_exit_status" >>"$RECOVERY_LOG"
        if [[ "$restored_kd" != "$kd_mode" || "$restored_termios" != "$tty_state" ]]; then
            status=1
        fi
    fi
    printf 'sophia_session_lifecycle schema=1 status=returned phase=handoff installed=%s exit_status=%s emergency=%s handoff=display_manager\n' \
        "$INSTALLED_SESSION" "$status" "$emergency" >>"$LIFECYCLE_LOG"
    return "$status"
}
stop_from_signal() {
    local status="$1"
    exit "$status"
}
trap cleanup EXIT
trap 'stop_from_signal 130' INT
trap 'stop_from_signal 143' TERM
printf '%s\n' "$$" >"$PID_FILE"

tty_state="$(stty -g)"
kd_mode="$(python3 "$TTY_MODE_HELPER" get)"
keyboard_mode="$(python3 "$TTY_MODE_HELPER" get-keyboard)"

if [[ "$MANAGE_KEYD" == true ]] && pgrep -x keyd >/dev/null 2>&1; then
    echo "Temporarily stopping keyd so Sophia can own the keyboard..."
    sudo -v
    sudo sv down keyd
    keyd_was_running=true
fi

[[ ! -f "$GUARD_LOG" ]] || mv -f "$GUARD_LOG" "$GUARD_LOG.previous"
: >"$GUARD_LOG"
chmod 600 "$GUARD_LOG"
rm -f "$GUARD_ARMED_FILE" "$GUARD_TRIGGERED_FILE"
lifecycle_phase entering input_guard
"$SOPHIA_BIN" sophia-session-input-guard \
    "${input_source_args[@]}" \
    --armed-file="$GUARD_ARMED_FILE" \
    --triggered-file="$GUARD_TRIGGERED_FILE" \
    --owner-pid="$$" >>"$GUARD_LOG" 2>&1 &
guard_pid=$!
echo "Safety check: press and release Ctrl-Alt-Backspace once to arm recovery."
echo "During Sophia, press Ctrl-Alt-Backspace again for emergency recovery."
for _ in {1..600}; do
    [[ ! -s "$GUARD_ARMED_FILE" ]] || break
    kill -0 "$guard_pid" 2>/dev/null || {
        echo "Input guard exited before arming; see $GUARD_LOG" >&2
        exit 1
    }
    sleep 0.05
done
[[ -s "$GUARD_ARMED_FILE" ]] || {
    echo "Input guard was not armed within 30 seconds; refusing graphics takeover." >&2
    exit 1
}
echo "Emergency input guard armed."
lifecycle_phase complete input_guard

if [[ "$SESSION_PROFILE" == xmonad ]]; then
    echo "Starting Sophia with experimental xmonad layout policy on $DISPLAY_NAME."
    echo "Use Super+Enter for Kitty or Super+Shift+Q to log out."
else
    echo "Starting the supported Kitty-only Sophia input session on $DISPLAY_NAME."
    echo "xmonad and Super+Enter are intentionally disabled for this input gate."
    echo "Exit Kitty normally to return to tty3."
fi
echo "Press Ctrl-Alt-Backspace for local emergency recovery."
echo "The outside control plane may also run tools/stop_sophia_${SESSION_PROFILE}_session.sh."
terminal_bin="${SOPHIA_TERMINAL_BIN:-$(command -v kitty || true)}"
if [[ -z "$terminal_bin" || ! -x "$terminal_bin" ]]; then
    echo "The graphical session requires Kitty; set SOPHIA_TERMINAL_BIN if it is installed elsewhere." >&2
    exit 1
fi
[[ ! -f "$SESSION_LOG" ]] || mv -f "$SESSION_LOG" "$SESSION_LOG.previous"
: >"$SESSION_LOG"
chmod 600 "$SESSION_LOG"
session_args=(
    sophia-live-session
    --session-mode=normal
    "--session-app=terminal=$terminal_bin"
    --session-start=terminal
    --display="$DISPLAY_NAME"
    --native-scanout
    "${input_source_args[@]}"
    --session-app-arg=terminal=--config
    --session-app-arg=terminal=NONE
    --session-app-arg=terminal=--override
    --session-app-arg=terminal=linux_display_server=x11
    --session-app-arg=terminal=--override
    --session-app-arg=terminal=background_opacity=1
    --session-app-arg=terminal=--title
    "--session-app-arg=terminal=Sophia ${SESSION_PROFILE^} TTY3"
    --startup-ready-timeout-ms=8000
)
if [[ "$SESSION_PROFILE" == xmonad ]]; then
    session_args+=(
        --session-action-app=terminal=terminal
        --wm-process="$SOPHIA_WM_BRIDGE_BIN"
        --wm-process-arg="--wm=$xmonad_bin"
        --wm-process-arg=--profile=xmonad
        --wm-process-arg=--wm-private-alias=xmonad/xmonad-x86_64-linux
    )
    if [[ -n "$xmobar_bin" ]]; then
        xmobar_config="${SOPHIA_XMOBAR_CONFIG:-$ROOT_DIR/tools/fixtures/xmobar_sophia.config}"
        [[ -f "$xmobar_config" ]] || {
            echo "Xmobar configuration does not exist: $xmobar_config" >&2
            exit 1
        }
        session_args+=(
            "--session-app=statusbar=$xmobar_bin"
            "--session-app-arg=statusbar=$xmobar_config"
            --session-start=statusbar
        )
        echo "Xmobar status bar enabled: $xmobar_bin"
    fi
    firefox_bin="${SOPHIA_FIREFOX_BIN:-$(command -v firefox || true)}"
    if [[ -n "$firefox_bin" && -x "$firefox_bin" ]]; then
        session_args+=(
            "--session-app=firefox=$firefox_bin"
            --session-action-app=firefox=firefox
            --session-app-arg=firefox=--no-remote
            --session-app-arg=firefox=--new-instance
            "--session-app-arg=firefox=file://$ROOT_DIR/tools/fixtures/firefox_m8_local_page.html"
        )
    fi
else
    session_args+=(
        --exit-when-startup-exits
    )
fi
session_args+=("$@")
session_environment=(
    SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1
    DBUS_SESSION_BUS_ADDRESS=unix:path=/dev/null
    "SOPHIA_SESSION_TTY=$tty_name"
)
if [[ "${SOPHIA_SESSION_VERBOSE_TRACE:-false}" == true ]]; then
    session_environment+=(
        SOPHIA_LIVE_SESSION_DIAGNOSTIC=1
        SOPHIA_NATIVE_COMPOSITION_PIXEL_TRACE=1
        SOPHIA_X11_AUTHORITY_TRACE=1
    )
fi
session_command=(
    env
    -u WAYLAND_DISPLAY
    -u WAYLAND_SOCKET
    "${session_environment[@]}"
    "$SOPHIA_BIN"
    "${session_args[@]}"
)
python3 "$TTY_MODE_HELPER" graphics
python3 "$TTY_MODE_HELPER" keyboard-off
stty raw -echo
lifecycle_phase entering graphics_takeover
setsid "${session_command[@]}" > >(tee "$SESSION_LOG") 2>&1 &
session_pid=$!
lifecycle_phase complete graphics_takeover
lifecycle_phase entering session
set +e
wait -n "$session_pid" "$guard_pid"
status=$?
set -e
if [[ -s "$GUARD_TRIGGERED_FILE" ]]; then
    echo "Emergency recovery requested."
    emergency_session_shutdown=fallback_term
    for _ in {1..100}; do
        session_state="$(ps -o stat= -p "$session_pid" 2>/dev/null || true)"
        if [[ -z "$session_state" || "$session_state" == Z* ]]; then
            set +e
            wait "$session_pid"
            emergency_session_exit_status=$?
            set -e
            session_pid=""
            emergency_session_shutdown=graceful
            break
        fi
        sleep 0.05
    done
    exit 130
fi
if ! kill -0 "$session_pid" 2>/dev/null; then
    set +e
    wait "$session_pid"
    status=$?
    set -e
    session_pid=""
else
    echo "Input guard exited unexpectedly; see $GUARD_LOG" >&2
    status=1
fi
exit "$status"
