#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOPHIA_BIN="${SOPHIA_BIN:-$ROOT_DIR/target/release/sophia}"
SOPHIA_WM_BRIDGE_BIN="${SOPHIA_X11_WM_BRIDGE_BIN:-$ROOT_DIR/target/release/sophia-x11-wm-bridge}"
SOPHIA_NATIVE_WM_BIN="${SOPHIA_NATIVE_WM_BIN:-$ROOT_DIR/target/release/sophia-wm-demo}"
TTY_MODE_HELPER="${SOPHIA_TTY_MODE_HELPER:-$ROOT_DIR/tools/sophia_tty_mode.py}"
BUILD_SESSION="${SOPHIA_BUILD_SESSION:-true}"
MANAGE_KEYD="${SOPHIA_MANAGE_KEYD:-true}"
INSTALLED_SESSION="${SOPHIA_INSTALLED_SESSION:-false}"
REQUIRE_RUNTIME_DIR="${SOPHIA_REQUIRE_RUNTIME_DIR:-false}"
REQUIRE_LOCAL_VT="${SOPHIA_REQUIRE_LOCAL_VT:-false}"
DISPLAY_NAME="${SOPHIA_LIVE_SESSION_DISPLAY:-:77}"
SESSION_PROFILE="${SOPHIA_TTY_PROFILE:-xmonad}"
SESSION_WATCHDOG_SECONDS="${SOPHIA_SESSION_WATCHDOG_SECONDS:-}"
FIREFOX_M10_PROOF=false
FIREFOX_M10_RENDERING_PROOF=false
FIREFOX_M10_SELECTION_PROOF=false
FIREFOX_M10_LIFECYCLE_PROOF=false
for argument in "$@"; do
    case "$argument" in
        --firefox-m10-proof) FIREFOX_M10_PROOF=true ;;
        --firefox-m10-rendering-proof) FIREFOX_M10_RENDERING_PROOF=true ;;
        --firefox-m10-selection-proof) FIREFOX_M10_SELECTION_PROOF=true ;;
        --firefox-m10-lifecycle-proof) FIREFOX_M10_LIFECYCLE_PROOF=true ;;
    esac
done
FIREFOX_M10_ANY_PROOF=false
if [[ "$FIREFOX_M10_PROOF" == true
    || "$FIREFOX_M10_RENDERING_PROOF" == true
    || "$FIREFOX_M10_SELECTION_PROOF" == true
    || "$FIREFOX_M10_LIFECYCLE_PROOF" == true ]]; then
    FIREFOX_M10_ANY_PROOF=true
fi
if [[ "$SESSION_PROFILE" != standalone
    && "$SESSION_PROFILE" != xmonad
    && "$SESSION_PROFILE" != native
    && "$SESSION_PROFILE" != kitty ]]; then
    echo "SOPHIA_TTY_PROFILE must be standalone, xmonad, native, or kitty." >&2
    exit 1
fi
if [[ -n "$SESSION_WATCHDOG_SECONDS"
    && ! "$SESSION_WATCHDOG_SECONDS" =~ ^[1-9][0-9]*$ ]]; then
    echo "SOPHIA_SESSION_WATCHDOG_SECONDS must be a positive integer when set." >&2
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
WATCHDOG_TRIGGERED_FILE="$STATE_DIR/session-watchdog.triggered"

mkdir -p "$STATE_DIR"
chmod 700 "$STATE_DIR"
firefox_m10_probe_dir=""
firefox_m10_profile_dir=""
if [[ "$FIREFOX_M10_ANY_PROOF" == true ]]; then
    firefox_m10_probe_dir="$(mktemp -d "$STATE_DIR/firefox-m10.XXXXXX")"
    firefox_m10_profile_dir="$firefox_m10_probe_dir/firefox-profile"
    mkdir -p "$firefox_m10_profile_dir"
    chmod 700 "$firefox_m10_profile_dir"
    printf '%s\n' \
        'user_pref("browser.tabs.remote.autostart", false);' \
        'user_pref("browser.tabs.remote.autostart.2", false);' \
        'user_pref("fission.autostart", false);' \
        'user_pref("middlemouse.paste", true);' \
        'user_pref("middlemouse.contentLoadURL", false);' \
        >"$firefox_m10_profile_dir/user.js"
    chmod 600 "$firefox_m10_profile_dir/user.js"
fi
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
    elif [[ "$SESSION_PROFILE" == native || "$SESSION_PROFILE" == standalone ]]; then
        cargo build --offline --release -p sophia-wm-demo
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
if [[ ( "$SESSION_PROFILE" == native || "$SESSION_PROFILE" == standalone )
    && ! -x "$SOPHIA_NATIVE_WM_BIN" ]]; then
    echo "Sophia native WM is not executable: $SOPHIA_NATIVE_WM_BIN" >&2
    exit 1
fi
lifecycle_phase complete preflight

keyd_was_running=false
tty_state=""
kd_mode=""
keyboard_mode=""
guard_pid=""
watchdog_pid=""
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
    if [[ -s "$GUARD_TRIGGERED_FILE" || -s "$WATCHDOG_TRIGGERED_FILE" ]]; then
        emergency=true
    fi
    [[ -z "$watchdog_pid" ]] || terminate_bounded "$watchdog_pid" "Sophia session watchdog"
    watchdog_pid=""
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
    rm -f "$GUARD_ARMED_FILE" "$GUARD_TRIGGERED_FILE" "$WATCHDOG_TRIGGERED_FILE"
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
rm -f "$GUARD_ARMED_FILE" "$GUARD_TRIGGERED_FILE" "$WATCHDOG_TRIGGERED_FILE"
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

if [[ "$SESSION_PROFILE" == standalone ]]; then
    echo "Starting Sophia's standalone natural-size application proof on $DISPLAY_NAME."
    echo "No terminal, xmonad bridge, or status bar will run."
    if [[ "${SOPHIA_STANDALONE_WORKLOAD:-}" == xterm ]]; then
        echo "Let the bounded xterm exit automatically; do not use the logout shortcut."
    else
        echo "Use Super+Shift+Q to log out after inspecting the application."
    fi
elif [[ "$SESSION_PROFILE" == xmonad ]]; then
    echo "Starting Sophia with experimental xmonad layout policy on $DISPLAY_NAME."
    echo "Use Super+Enter for Kitty or Super+Shift+Q to log out."
elif [[ "$SESSION_PROFILE" == native ]]; then
    echo "Starting Sophia with its native WM policy on $DISPLAY_NAME."
    echo "Use Super+Enter for Kitty or Super+Shift+Q to log out."
else
    echo "Starting the supported Kitty-only Sophia input session on $DISPLAY_NAME."
    echo "xmonad and Super+Enter are intentionally disabled for this input gate."
    echo "Exit Kitty normally to return to tty3."
fi
echo "Press Ctrl-Alt-Backspace for local emergency recovery."
echo "The outside control plane may also run tools/stop_sophia_${SESSION_PROFILE}_session.sh."
terminal_bin=""
standalone_bin=""
standalone_workload=""
glxgears_duration=""
glxgears_width=""
glxgears_height=""
xterm_duration=""
xterm_width=""
xterm_height=""
xterm_lines=""
xterm_interval_msec=""
if [[ "$SESSION_PROFILE" == standalone ]]; then
    standalone_workload="${SOPHIA_STANDALONE_WORKLOAD:-vkcube}"
    case "$standalone_workload" in
        glxgears)
            standalone_default_bin="$(command -v glxgears || true)"
            standalone_requirement=glxgears
            glxgears_duration="${SOPHIA_GLXGEARS_DURATION_SECONDS:-20}"
            glxgears_width="${SOPHIA_GLXGEARS_WIDTH:-500}"
            glxgears_height="${SOPHIA_GLXGEARS_HEIGHT:-500}"
            [[ "$glxgears_duration" =~ ^[1-9][0-9]*$ ]] || {
                echo "SOPHIA_GLXGEARS_DURATION_SECONDS must be a positive integer." >&2
                exit 1
            }
            [[ "$glxgears_width" =~ ^[1-9][0-9]*$
                && "$glxgears_height" =~ ^[1-9][0-9]*$ ]] || {
                echo "SOPHIA_GLXGEARS_WIDTH and SOPHIA_GLXGEARS_HEIGHT must be positive integers." >&2
                exit 1
            }
            ;;
        vkcube)
            standalone_default_bin="$(command -v vkcube || true)"
            standalone_requirement=vkcube
            ;;
        xterm)
            standalone_default_bin="$(command -v xterm || true)"
            standalone_requirement=xterm
            xterm_duration="${SOPHIA_XTERM_DURATION_SECONDS:-20}"
            xterm_width="${SOPHIA_XTERM_WIDTH:-500}"
            xterm_height="${SOPHIA_XTERM_HEIGHT:-500}"
            xterm_lines="${SOPHIA_XTERM_LINES:-8}"
            xterm_interval_msec="${SOPHIA_XTERM_INTERVAL_MSEC:-16}"
            [[ "$xterm_duration" =~ ^[1-9][0-9]*$ ]] || {
                echo "SOPHIA_XTERM_DURATION_SECONDS must be a positive integer." >&2
                exit 1
            }
            [[ "$xterm_width" =~ ^[1-9][0-9]*$
                && "$xterm_height" =~ ^[1-9][0-9]*$ ]] || {
                echo "SOPHIA_XTERM_WIDTH and SOPHIA_XTERM_HEIGHT must be positive integers." >&2
                exit 1
            }
            [[ "$xterm_lines" =~ ^[1-9][0-9]*$ ]] || {
                echo "SOPHIA_XTERM_LINES must be a positive integer." >&2
                exit 1
            }
            [[ "$xterm_interval_msec" =~ ^[1-9][0-9]*$
                && "$xterm_interval_msec" -le 1000 ]] || {
                echo "SOPHIA_XTERM_INTERVAL_MSEC must be an integer from 1 through 1000." >&2
                exit 1
            }
            ;;
        *)
            echo "SOPHIA_STANDALONE_WORKLOAD must be glxgears, vkcube, or xterm." >&2
            exit 1
            ;;
    esac
    standalone_bin="${SOPHIA_STANDALONE_APP_BIN:-$standalone_default_bin}"
    if [[ -z "$standalone_bin" || ! -x "$standalone_bin" ]]; then
        echo "The standalone $standalone_workload proof requires $standalone_requirement; set SOPHIA_STANDALONE_APP_BIN to override it." >&2
        exit 1
    fi
else
    terminal_bin="${SOPHIA_TERMINAL_BIN:-$(command -v kitty || true)}"
    if [[ -z "$terminal_bin" || ! -x "$terminal_bin" ]]; then
        echo "The graphical session requires Kitty; set SOPHIA_TERMINAL_BIN if it is installed elsewhere." >&2
        exit 1
    fi
fi
[[ ! -f "$SESSION_LOG" ]] || mv -f "$SESSION_LOG" "$SESSION_LOG.previous"
: >"$SESSION_LOG"
chmod 600 "$SESSION_LOG"
session_args=(
    sophia-live-session
    --session-mode=normal
    --display="$DISPLAY_NAME"
    --native-scanout
    "${input_source_args[@]}"
    --startup-ready-timeout-ms=8000
)
if [[ "$SESSION_PROFILE" == standalone ]]; then
    standalone_wm_template="$ROOT_DIR/tools/fixtures/standalone_sophia_wm.kdl"
    standalone_wm_config="$STATE_DIR/standalone-wm.kdl"
    if [[ ! -f "$standalone_wm_template" ]]; then
        echo "The standalone WM policy is missing: $standalone_wm_template" >&2
        exit 1
    fi
    install -m 600 "$standalone_wm_template" "$standalone_wm_config"
    session_args+=(
        --no-config
        "--session-app=standalone=$standalone_bin"
        --session-start=standalone
        --exit-when-startup-exits
        --wm-process="$SOPHIA_NATIVE_WM_BIN"
        "--wm-process-arg=--wm-config=$standalone_wm_config"
    )
    if [[ "$standalone_workload" == vkcube ]]; then
        session_args+=(
            --session-app-arg=standalone=--wsi
            --session-app-arg=standalone=xcb
        )
    fi
    if [[ -n "${SOPHIA_STANDALONE_FRAME_COUNT:-}" ]]; then
        [[ "$standalone_workload" == vkcube ]] || {
            echo "SOPHIA_STANDALONE_FRAME_COUNT is valid only for the vkcube workload." >&2
            exit 1
        }
        [[ "$SOPHIA_STANDALONE_FRAME_COUNT" =~ ^[1-9][0-9]*$ ]] || {
            echo "SOPHIA_STANDALONE_FRAME_COUNT must be a positive integer." >&2
            exit 1
        }
        standalone_width="${SOPHIA_STANDALONE_WIDTH:-500}"
        standalone_height="${SOPHIA_STANDALONE_HEIGHT:-500}"
        standalone_present_mode="${SOPHIA_STANDALONE_PRESENT_MODE:-2}"
        [[ "$standalone_width" =~ ^[1-9][0-9]*$
            && "$standalone_height" =~ ^[1-9][0-9]*$ ]] || {
            echo "SOPHIA_STANDALONE_WIDTH and SOPHIA_STANDALONE_HEIGHT must be positive integers." >&2
            exit 1
        }
        [[ "$standalone_present_mode" =~ ^[0-3]$ ]] || {
            echo "SOPHIA_STANDALONE_PRESENT_MODE must be a Vulkan present mode from 0 through 3." >&2
            exit 1
        }
        session_args+=(
            --session-app-arg=standalone=--c
            "--session-app-arg=standalone=$SOPHIA_STANDALONE_FRAME_COUNT"
            --session-app-arg=standalone=--width
            "--session-app-arg=standalone=$standalone_width"
            --session-app-arg=standalone=--height
            "--session-app-arg=standalone=$standalone_height"
            --session-app-arg=standalone=--present_mode
            "--session-app-arg=standalone=$standalone_present_mode"
        )
    fi
else
    session_args+=(
        "--session-app=terminal=$terminal_bin"
        --session-start=terminal
        --session-app-arg=terminal=--config
        --session-app-arg=terminal=NONE
        --session-app-arg=terminal=--override
        --session-app-arg=terminal=linux_display_server=x11
        --session-app-arg=terminal=--override
        --session-app-arg=terminal=background_opacity=1
    )
    if [[ "$FIREFOX_M10_SELECTION_PROOF" == true ]]; then
        session_args+=(
            "--session-app-arg=terminal=$ROOT_DIR/tools/fixtures/firefox_m10_selection_kitty_probe.sh"
        )
    elif [[ "$FIREFOX_M10_PROOF" == true || "$FIREFOX_M10_LIFECYCLE_PROOF" == true ]]; then
        session_args+=(
            "--session-app-arg=terminal=$ROOT_DIR/tools/fixtures/firefox_m10_kitty_probe.sh"
        )
    else
        session_args+=(
            --session-app-arg=terminal=--title
            "--session-app-arg=terminal=Sophia ${SESSION_PROFILE^} TTY3"
        )
    fi
fi
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
        firefox_page="file://$ROOT_DIR/tools/fixtures/firefox_m8_local_page.html"
        if [[ "$FIREFOX_M10_RENDERING_PROOF" == true ]]; then
            firefox_page="${firefox_page}?rendering_only=1"
        elif [[ "$FIREFOX_M10_PROOF" == true || "$FIREFOX_M10_SELECTION_PROOF" == true ]]; then
            firefox_page="${firefox_page}?selection_peer=kitty"
        elif [[ "$FIREFOX_M10_LIFECYCLE_PROOF" == true ]]; then
            firefox_page="${firefox_page}?lifecycle_only=1"
        fi
        session_args+=(
            "--session-app=firefox=$firefox_bin"
            --session-action-app=firefox=firefox
            --session-app-arg=firefox=--no-remote
            --session-app-arg=firefox=--new-instance
        )
        if [[ "$FIREFOX_M10_ANY_PROOF" == true ]]; then
            session_args+=(
                --session-app-arg=firefox=--profile
                "--session-app-arg=firefox=$firefox_m10_profile_dir"
            )
        fi
        session_args+=(
            "--session-app-arg=firefox=$firefox_page"
        )
    fi
elif [[ "$SESSION_PROFILE" == native ]]; then
    session_args+=(
        --session-action-app=terminal=terminal
        --wm-process="$SOPHIA_NATIVE_WM_BIN"
    )
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
if [[ "$FIREFOX_M10_ANY_PROOF" == true ]]; then
    session_environment+=(
        "SOPHIA_FIREFOX_M10_KITTY_PROBE_DIR=$firefox_m10_probe_dir"
        "SOPHIA_FIREFOX_M10_PROOF_SLICE=$(
            if [[ "$FIREFOX_M10_SELECTION_PROOF" == true ]]; then
                echo selection
            elif [[ "$FIREFOX_M10_RENDERING_PROOF" == true ]]; then
                echo rendering
            elif [[ "$FIREFOX_M10_LIFECYCLE_PROOF" == true ]]; then
                echo lifecycle
            else
                echo promotion
            fi
        )"
        GDK_BACKEND=x11
        GTK_USE_PORTAL=0
        MOZ_ENABLE_WAYLAND=0
        MOZ_FORCE_DISABLE_E10S=1
        MOZ_USE_XINPUT2=1
    )
fi
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
if [[ "$SESSION_PROFILE" == standalone
    && "$standalone_workload" == vkcube
    && -n "${SOPHIA_STANDALONE_FRAME_COUNT:-}" ]]; then
    printf 'sophia_rendering_benchmark schema=1 workload=vkcube-xcb requested_frames=%s surface_width=%s surface_height=%s vulkan_present_mode=%s\n' \
        "$SOPHIA_STANDALONE_FRAME_COUNT" "$standalone_width" "$standalone_height" \
        "$standalone_present_mode" >>"$SESSION_LOG"
elif [[ "$SESSION_PROFILE" == standalone
    && "$standalone_workload" == glxgears ]]; then
    printf 'sophia_glxgears_benchmark schema=1 duration_seconds=%s surface_width=%s surface_height=%s swap_interval=1\n' \
        "$glxgears_duration" "$glxgears_width" "$glxgears_height" >>"$SESSION_LOG"
elif [[ "$SESSION_PROFILE" == standalone
    && "$standalone_workload" == xterm ]]; then
    printf 'sophia_terminal_benchmark schema=2 workload=xterm-cpu duration_seconds=%s surface_width=%s surface_height=%s lines_per_iteration=%s interval_msec=%s\n' \
        "$xterm_duration" "$xterm_width" "$xterm_height" \
        "$xterm_lines" "$xterm_interval_msec" >>"$SESSION_LOG"
fi
python3 "$TTY_MODE_HELPER" graphics
python3 "$TTY_MODE_HELPER" keyboard-off
stty raw -echo
lifecycle_phase entering graphics_takeover
setsid "${session_command[@]}" > >(tee -a "$SESSION_LOG") 2>&1 &
session_pid=$!
if [[ -n "$SESSION_WATCHDOG_SECONDS" ]]; then
    (
        sleep "$SESSION_WATCHDOG_SECONDS"
        if kill -0 "$session_pid" 2>/dev/null; then
            printf 'sophia_session_watchdog schema=1 result=deadline_exceeded deadline_seconds=%s session_pid=%s action=terminate_process_group\n' \
                "$SESSION_WATCHDOG_SECONDS" "$session_pid" >>"$SESSION_LOG"
            printf 'deadline_exceeded\n' >"$WATCHDOG_TRIGGERED_FILE"
            kill -TERM -- "-$session_pid" 2>/dev/null || true
            sleep 2
            kill -KILL -- "-$session_pid" 2>/dev/null || true
        fi
    ) &
    watchdog_pid=$!
    echo "Independent session watchdog armed for ${SESSION_WATCHDOG_SECONDS} seconds."
fi
lifecycle_phase complete graphics_takeover
lifecycle_phase entering session
set +e
wait_targets=("$session_pid" "$guard_pid")
[[ -z "$watchdog_pid" ]] || wait_targets+=("$watchdog_pid")
wait -n "${wait_targets[@]}"
status=$?
set -e
if [[ -s "$WATCHDOG_TRIGGERED_FILE" ]]; then
    echo "Session deadline exceeded; automatic recovery requested." >&2
    emergency_session_shutdown=watchdog_term
    exit 124
fi
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
