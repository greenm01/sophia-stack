#!/usr/bin/env bash
set -euo pipefail
umask 077

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SESSION_PROFILE="${SOPHIA_TTY_PROFILE:-}"
TARGET_VT="${SOPHIA_TTY_NUMBER:-3}"
[[ "$TARGET_VT" =~ ^[1-9][0-9]*$ && "$TARGET_VT" -le 63 ]] || {
    echo "SOPHIA_TTY_NUMBER must be an integer from 1 through 63." >&2
    exit 1
}
TARGET_TTY="/dev/tty$TARGET_VT"
case "$SESSION_PROFILE" in
    hagia|hagia-policy|kitty|native|standalone|xmonad) ;;
    *)
        echo "SOPHIA_TTY_PROFILE must be hagia, hagia-policy, kitty, native, standalone, or xmonad." >&2
        exit 1
        ;;
esac
LAUNCH_LOG="/tmp/sophia-${SESSION_PROFILE}-tty${TARGET_VT}-launch.log"
TTY_MODE_HELPER="$ROOT_DIR/tools/sophia_tty_mode.py"
HANDOFF_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/sophia"
HANDOFF_LOG="$HANDOFF_DIR/tty-handoff.log"
mkdir -p "$HANDOFF_DIR"
chmod 700 "$HANDOFF_DIR"
: >"$HANDOFF_LOG"
chmod 600 "$HANDOFF_LOG"
exec > >(tee "$LAUNCH_LOG") 2>&1
echo "Retaining complete launcher output in $LAUNCH_LOG"

if [[ ! -t 0 || "$(tty)" != "$TARGET_TTY" ]]; then
    echo "Switch to tty$TARGET_VT, log in, then run:" >&2
    if [[ "$SESSION_PROFILE" == hagia-policy ]]; then
        echo "  $ROOT_DIR/tools/start_sophia_hagia_policy_tty4.sh" >&2
    elif [[ "$SESSION_PROFILE" == native ]]; then
        echo "  $ROOT_DIR/tools/start_sophia_native_hot_reload_tty3.sh" >&2
    elif [[ "$SESSION_PROFILE" == standalone ]]; then
        if [[ "${SOPHIA_ENABLE_DIRECT_SCANOUT:-0}" == 1 ]]; then
            echo "  just direct-scanout-probe" >&2
        else
            echo "  $ROOT_DIR/tools/start_sophia_vkcube_standalone_tty3.sh" >&2
        fi
    else
        echo "  $ROOT_DIR/tools/start_sophia_${SESSION_PROFILE}_tty3.sh" >&2
    fi
    exit 1
fi
origin_tty="$(tty)"
origin_vt="${origin_tty#/dev/tty}"
origin_tty_state="$(stty -g)"
origin_kd_mode="$(python3 "$TTY_MODE_HELPER" get)"
origin_keyboard_mode="$(python3 "$TTY_MODE_HELPER" get-keyboard)"

display_manager=""
display_manager_vt=""
display_manager_tty=""
display_manager_tty_state=""
display_manager_kd_mode=""
display_manager_keyboard_mode=""
display_manager_stopped=false
sudo_keepalive_pid=""
sudo_lease_owner="$BASHPID"
graphical_processes=(river niri sway Hyprland kwin_wayland Xorg)
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
terminate_named_processes() {
    local record pid
    while read -r record; do
        [[ -n "$record" ]] || continue
        pid="${record##*:}"
        [[ "$pid" =~ ^[0-9]+$ ]] || continue
        echo "Stopping lingering graphical process $record..."
        if ! kill -TERM "$pid" 2>/dev/null; then
            sudo -n kill -TERM "$pid"
        fi
    done < <(live_named_processes "$@")
}
if [[ -e /var/service/lightdm || -e /var/service/greetd ]]; then
    sudo -v
fi
for candidate in lightdm greetd; do
    if [[ -e "/var/service/$candidate" ]] \
        && sudo -n sv status "$candidate" 2>/dev/null | grep -q '^run:'; then
        display_manager="$candidate"
        break
    fi
done

if [[ "$display_manager" == greetd ]]; then
    display_manager_vt="$(awk '
        /^\[[^]]+\][[:space:]]*$/ { terminal = ($0 ~ /^\[terminal\][[:space:]]*$/) }
        terminal && /^[[:space:]]*vt[[:space:]]*=/ {
            sub(/^[^=]*=[[:space:]]*/, "")
            gsub(/[[:space:]]/, "")
            print
            exit
        }
    ' /etc/greetd/config.toml 2>/dev/null || true)"
    [[ "$display_manager_vt" =~ ^[1-9][0-9]*$ && "$display_manager_vt" -le 63 ]] || {
        echo "Could not resolve greetd's configured VT; refusing graphical takeover." >&2
        exit 1
    }
    display_manager_tty="/dev/tty$display_manager_vt"
    display_manager_tty_state="$(sudo -n stty -g -F "$display_manager_tty")"
    display_manager_kd_mode="$(
        sudo -n env SOPHIA_SESSION_TTY="$display_manager_tty" \
            python3 "$TTY_MODE_HELPER" get
    )"
    display_manager_keyboard_mode="$(
        sudo -n env SOPHIA_SESSION_TTY="$display_manager_tty" \
            python3 "$TTY_MODE_HELPER" get-keyboard
    )"
fi
printf 'sophia_tty_handoff schema=1 status=armed profile=%s origin_vt=%s display_manager=%s manager_vt=%s\n' \
    "$SESSION_PROFILE" "$origin_vt" "${display_manager:-none}" "${display_manager_vt:-none}" \
    >>"$HANDOFF_LOG"

restore_origin_tty() {
    local status=0 restored_kd restored_keyboard restored_termios
    python3 "$TTY_MODE_HELPER" "$origin_kd_mode" 2>/dev/null || status=1
    python3 "$TTY_MODE_HELPER" "keyboard-$origin_keyboard_mode" 2>/dev/null || status=1
    stty "$origin_tty_state" 2>/dev/null || status=1
    restored_kd="$(python3 "$TTY_MODE_HELPER" get 2>/dev/null || echo unavailable)"
    restored_keyboard="$(python3 "$TTY_MODE_HELPER" get-keyboard 2>/dev/null || echo unavailable)"
    restored_termios="$(stty -g 2>/dev/null || echo unavailable)"
    [[ "$restored_kd" == "$origin_kd_mode" \
        && "$restored_keyboard" == "$origin_keyboard_mode" \
        && "$restored_termios" == "$origin_tty_state" ]] || status=1
    return "$status"
}

restore_greetd_tty() {
    [[ "$display_manager" == greetd && -n "$display_manager_tty" ]] || return 0
    local status=0
    sudo -n env SOPHIA_SESSION_TTY="$display_manager_tty" \
        python3 "$TTY_MODE_HELPER" "$display_manager_kd_mode" || status=1
    sudo -n env SOPHIA_SESSION_TTY="$display_manager_tty" \
        python3 "$TTY_MODE_HELPER" "keyboard-$display_manager_keyboard_mode" || status=1
    sudo -n stty -F "$display_manager_tty" "$display_manager_tty_state" || status=1
    verify_greetd_tty || status=1
    return "$status"
}

verify_greetd_tty() {
    [[ "$display_manager" == greetd && -n "$display_manager_tty" ]] || return 0
    [[ "$(sudo -n env SOPHIA_SESSION_TTY="$display_manager_tty" \
            python3 "$TTY_MODE_HELPER" get 2>/dev/null || echo unavailable)" \
        == "$display_manager_kd_mode" ]] || return 1
    [[ "$(sudo -n env SOPHIA_SESSION_TTY="$display_manager_tty" \
            python3 "$TTY_MODE_HELPER" get-keyboard 2>/dev/null || echo unavailable)" \
        == "$display_manager_keyboard_mode" ]] || return 1
    [[ "$(sudo -n stty -g -F "$display_manager_tty" 2>/dev/null || echo unavailable)" \
        == "$display_manager_tty_state" ]]
}

stop_sudo_keepalive() {
    [[ -n "$sudo_keepalive_pid" ]] || return 0
    while read -r child; do
        [[ -z "$child" ]] || kill "$child" 2>/dev/null || true
    done < <(pgrep -P "$sudo_keepalive_pid" 2>/dev/null || true)
    kill "$sudo_keepalive_pid" 2>/dev/null || true
    wait "$sudo_keepalive_pid" 2>/dev/null || true
    sudo_keepalive_pid=""
}

restore_display_manager() {
    local status=$? origin_input_ok=true manager_input_ok=not_applicable manager_ready=not_applicable
    local activation_vt="$origin_vt" active_vt=unknown
    if ! restore_origin_tty; then
        echo "WARNING: could not restore and verify $origin_tty input state." >&2
        status=1
        origin_input_ok=false
    fi
    if [[ -n "$display_manager" && "$display_manager_stopped" == true ]]; then
        echo "Restoring $display_manager..."
        manager_input_ok=true
        manager_ready=true
        if ! restore_greetd_tty; then
            echo "WARNING: could not restore and verify $display_manager_tty before starting greetd." >&2
            status=1
            manager_input_ok=false
        fi
        if [[ "$manager_input_ok" == true ]]; then
            if ! sudo -n sv up "$display_manager"; then
                status=1
                manager_ready=false
            fi
        else
            manager_ready=false
        fi
        if [[ "$display_manager" == greetd && "$manager_ready" == true ]]; then
            manager_ready=false
            for _ in {1..200}; do
                if ps -C tuigreet -o tty= 2>/dev/null \
                    | grep -Eq "^[[:space:]]*tty${display_manager_vt}[[:space:]]*$"; then
                    manager_ready=true
                    break
                fi
                sleep 0.05
            done
            if [[ "$manager_ready" != true ]]; then
                echo "WARNING: greetd did not publish a greeter on $display_manager_tty." >&2
                status=1
            elif ! verify_greetd_tty; then
                echo "WARNING: greetd changed $display_manager_tty to an unverified input state." >&2
                status=1
                manager_input_ok=false
                manager_ready=false
            fi
        fi
        if [[ "$manager_ready" == true ]]; then
            [[ -z "$display_manager_vt" ]] || activation_vt="$display_manager_vt"
        else
            sudo -n sv down "$display_manager" 2>/dev/null || true
            if ! restore_origin_tty; then
                origin_input_ok=false
            fi
        fi
        if ! sudo -n chvt "$activation_vt"; then
            echo "WARNING: could not activate tty$activation_vt after restoring $display_manager." >&2
            status=1
        else
            active_vt="$(fgconsole 2>/dev/null || true)"
            printf 'sophia_tty_activation schema=1 requested=%s active=%s display_manager=%s\n' \
                "$activation_vt" "${active_vt:-unknown}" "$display_manager"
            if [[ -n "$active_vt" && "$active_vt" != "$activation_vt" ]]; then
                echo "WARNING: active VT is $active_vt rather than recovery VT $activation_vt." >&2
                status=1
            fi
        fi
    fi
    if ! printf 'sophia_tty_handoff schema=1 status=returned profile=%s origin_vt=%s origin_input=%s display_manager=%s manager_vt=%s manager_input=%s manager_ready=%s requested_vt=%s active_vt=%s exit_status=%s\n' \
        "$SESSION_PROFILE" "$origin_vt" "$origin_input_ok" "${display_manager:-none}" \
        "${display_manager_vt:-none}" "$manager_input_ok" "$manager_ready" \
        "$activation_vt" "$active_vt" "$status" >>"$HANDOFF_LOG"; then
        echo "WARNING: could not persist the final TTY handoff result." >&2
        status=1
    fi
    stop_sudo_keepalive
    return "$status"
}
trap restore_display_manager EXIT

if [[ -n "$display_manager" ]]; then
    # An optional overnight soak must not reach cleanup with an expired sudo
    # timestamp and an invisible password prompt. This child refreshes only the
    # launcher's existing lease and dies with, or at most 30 seconds after, it.
    (
        while kill -0 "$sudo_lease_owner" 2>/dev/null; do
            sleep 30
            sudo -n -v || exit
        done
    ) &
    sudo_keepalive_pid=$!
fi

# Ask whether every profile's session shape is still acceptable, while the
# display manager is still up and a failure costs a second rather than a TTY.
#
# Three physical runs died in argument validation with greetd already down:
# a desktop default the session could not satisfy, a window manager that could
# not serve, and a client override that would not parse. This is the check none
# of them had.
if ! cargo xtask profile check >/dev/null 2>"${TMPDIR:-/tmp}/sophia-profile-check.log"; then
    echo "A session profile would be refused; not taking the display." >&2
    cat "${TMPDIR:-/tmp}/sophia-profile-check.log" >&2
    exit 1
fi

if [[ -n "$display_manager" ]]; then
    echo "Stopping $display_manager so Sophia can own DRM..."
    sudo -n sv down "$display_manager"
    display_manager_stopped=true
    for _ in {1..100}; do
        [[ -n "$(live_named_processes "${graphical_processes[@]}")" ]] || break
        sleep 0.1
    done
    remaining_graphics="$(live_named_processes "${graphical_processes[@]}")"
    if [[ -n "$remaining_graphics" ]]; then
        terminate_named_processes "${graphical_processes[@]}"
        for _ in {1..50}; do
            [[ -n "$(live_named_processes "${graphical_processes[@]}")" ]] || break
            sleep 0.1
        done
        remaining_graphics="$(live_named_processes "${graphical_processes[@]}")"
    fi
    if [[ -n "$remaining_graphics" ]]; then
        echo "A graphical session remained active after $display_manager stopped; refusing takeover." >&2
        printf 'Still active (process:pid):\n%s\n' "$remaining_graphics" >&2
        exit 1
    fi
fi

cd "$ROOT_DIR"
case "$SESSION_PROFILE" in
    kitty) tools/run_sophia_kitty_session.sh "$@" ;;
    hagia) tools/run_sophia_xmonad_session.sh "$@" ;;
    hagia-policy) tools/hagia_policy_physical_gate.sh "$@" ;;
    native) tools/run_sophia_xmonad_session.sh "$@" ;;
    standalone) tools/run_sophia_xmonad_session.sh "$@" ;;
    xmonad) tools/run_sophia_xmonad_session.sh "$@" ;;
esac
