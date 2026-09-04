#!/usr/bin/env bash
# Narrow TTY/session adapter for one typed desktop-comparison schedule row.
set -Eeuo pipefail
umask 077

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
diagnostic_root="${XDG_STATE_HOME:-$HOME/.local/state}/sophia/desktop-comparison"
mkdir -p "$diagnostic_root"
chmod 700 "$diagnostic_root"
diagnostic_log="$diagnostic_root/gate-last.log"
internal_mode=false
case "${1:-}" in
    --internal-niri|--internal-xlibre) internal_mode=true ;;
esac
if [[ $internal_mode == false ]]; then
    : >"$diagnostic_log"
    rm -f "$diagnostic_root/xrandr-last.log"
else
    touch "$diagnostic_log"
fi
chmod 600 "$diagnostic_log"
gate_stage=initialization
record_gate_stage() {
    gate_stage=$1
    printf 'desktop_comparison_gate schema=1 status=entered stage=%s\n' "$gate_stage" \
        >>"$diagnostic_log"
}
record_unexpected_failure() {
    local status=$1 line=$2 command=$3
    printf 'desktop_comparison_gate schema=1 status=failed stage=%s line=%s exit=%s command=%q\n' \
        "$gate_stage" "$line" "$status" "$command" >>"$diagnostic_log"
}
trap 'record_unexpected_failure "$?" "$LINENO" "$BASH_COMMAND"' ERR

fail() {
    echo "desktop comparison: $*" >&2
    printf 'desktop_comparison_gate schema=1 status=failed stage=%s detail=%q\n' \
        "$gate_stage" "$*" >>"$diagnostic_log"
    exit 1
}

xtask="${SOPHIA_DESKTOP_COMPARISON_XTASK:-}"
trace_helper="$repo/tools/desktop_comparison_tracefs.sh"
niri_bin="${SOPHIA_DESKTOP_COMPARISON_NIRI_BIN:-/usr/bin/niri}"
xlibre_prefix="${SOPHIA_DESKTOP_COMPARISON_XLIBRE_PREFIX:-$HOME/.local/opt/xlibre-56be9f4320ef}"
xserver="$xlibre_prefix/bin/Xorg"
xmonad="$xlibre_prefix/bin/xmonad"
xlibre_modules="$xlibre_prefix/lib/xorg/modules/xlibre-25"
hagia_bin="${SOPHIA_DESKTOP_COMPARISON_HAGIA_BIN:-$repo/../hagia/hagia}"
narthex_bin="${SOPHIA_DESKTOP_COMPARISON_NARTHEX_BIN:-$repo/../narthex/narthex}"
sophia_bin="${SOPHIA_DESKTOP_COMPARISON_SOPHIA_BIN:-$repo/target/release/sophia}"
runtime_root="${XDG_RUNTIME_DIR:-}"
[[ -n $runtime_root ]] || fail "XDG_RUNTIME_DIR is unset"
adapter_root="$runtime_root/sophia-desktop-comparison-adapter"
mkdir -p "$adapter_root"
chmod 700 "$adapter_root"

sophia_launcher=
sophia_supervisor=
cleanup_sophia_session() {
    if [[ -n $sophia_supervisor ]] && kill -0 "$sophia_supervisor" 2>/dev/null; then
        kill -TERM -- "-$sophia_supervisor" 2>/dev/null \
            || kill -TERM "$sophia_supervisor" 2>/dev/null \
            || true
    fi
    if [[ -n $sophia_launcher ]] && kill -0 "$sophia_launcher" 2>/dev/null; then
        for _ in {1..300}; do
            kill -0 "$sophia_launcher" 2>/dev/null || break
            sleep 0.1
        done
        if kill -0 "$sophia_launcher" 2>/dev/null; then
            echo "desktop comparison: Sophia launcher cleanup exceeded 30 seconds; sending TERM" >&2
            kill -TERM "$sophia_launcher" 2>/dev/null || true
            for _ in {1..100}; do
                kill -0 "$sophia_launcher" 2>/dev/null || break
                sleep 0.1
            done
        fi
        if kill -0 "$sophia_launcher" 2>/dev/null; then
            echo "desktop comparison: Sophia launcher ignored TERM; sending KILL" >&2
            kill -KILL "$sophia_launcher" 2>/dev/null || true
        fi
    fi
    [[ -z $sophia_launcher ]] || wait "$sophia_launcher" 2>/dev/null || true
    sophia_launcher=
    sophia_supervisor=
}
trap cleanup_sophia_session EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

owned_for_executable() {
    local expected process pid executable process_uid
    expected=$(readlink -f -- "$1") || return 1
    for process in /proc/[0-9]*; do
        pid=${process#/proc/}
        executable=$(readlink "$process/exe" 2>/dev/null) || continue
        [[ $executable == "$expected" ]] || continue
        process_uid=$(awk '/^Uid:/ { print $2; exit }' "$process/status" 2>/dev/null) || continue
        [[ $process_uid == "$(id -u)" ]] || continue
        printf '%s\n' "$pid"
    done
}

attest_capture() {
    local run=$1 supervisor=$2
    "$xtask" conformance desktop-comparison attest "$run" "$supervisor"
    "$xtask" conformance desktop-comparison capture "$run"
}

attest_qualify_capture() {
    local run=$1 supervisor=$2
    "$xtask" conformance desktop-comparison attest "$run" "$supervisor"
    "$xtask" conformance desktop-comparison qualify "$run"
    "$xtask" conformance desktop-comparison capture "$run"
}

require_x_topology() {
    local ready=false topology= query_status=0
    for _ in {1..50}; do
        if topology=$(xrandr --query 2>&1); then
            query_status=0
        else
            query_status=$?
            if grep -Eq 'X Error of failed request|Major opcode of failed request' \
                <<<"$topology"; then
                printf 'desktop_comparison_xrandr schema=1 status=protocol_error exit=%s\n%s\n' \
                    "$query_status" "$topology" >"$diagnostic_root/xrandr-last.log"
                chmod 600 "$diagnostic_root/xrandr-last.log"
                fail "xrandr encountered an X protocol error; inspect $diagnostic_root/xrandr-last.log"
            fi
        fi
        if { grep -Eq '^DP-1 connected primary 2560x1440\+0\+0' <<<"$topology" \
                && grep -Eq '^DP-2 connected 1920x1080\+2560\+0' <<<"$topology"; } \
            || { grep -Eq '^SOPHIA-1 connected primary 2560x1440\+0\+0' <<<"$topology" \
                && grep -Eq '^SOPHIA-2 connected 1920x1080\+2560\+0' <<<"$topology"; }; then
            ready=true
            break
        fi
        sleep 0.1
    done
    if [[ $ready != true ]]; then
        printf 'desktop_comparison_xrandr schema=1 status=topology_mismatch exit=%s\n%s\n' \
            "$query_status" "$topology" >"$diagnostic_root/xrandr-last.log"
        chmod 600 "$diagnostic_root/xrandr-last.log"
        fail "the exact two-output X topology did not become ready; inspect $diagnostic_root/xrandr-last.log"
    fi
}

run_niri_child() {
    local run=$1 niri_pid= niri_socket=
    cleanup_niri() {
        if [[ -n $niri_pid ]] && kill -0 "$niri_pid" 2>/dev/null; then
            if [[ -n $niri_socket ]]; then
                NIRI_SOCKET="$niri_socket" "$niri_bin" msg action quit --skip-confirmation \
                    >/dev/null 2>&1 || true
            fi
            for _ in {1..100}; do
                kill -0 "$niri_pid" 2>/dev/null || break
                sleep 0.05
            done
            kill -0 "$niri_pid" 2>/dev/null && kill "$niri_pid" 2>/dev/null || true
            wait "$niri_pid" 2>/dev/null || true
        fi
    }
    trap cleanup_niri EXIT
    trap 'exit 129' HUP
    trap 'exit 130' INT
    trap 'exit 143' TERM

    "$niri_bin" --session --config "$repo/validation/desktop-comparison/profiles/niri.kdl" \
        >"$adapter_root/niri.log" 2>&1 &
    niri_pid=$!
    for _ in {1..300}; do
        kill -0 "$niri_pid" 2>/dev/null \
            || fail "niri exited during startup; inspect $adapter_root/niri.log"
        responsive=()
        for candidate in "$runtime_root"/niri.*.sock; do
            [[ -S $candidate ]] || continue
            if NIRI_SOCKET="$candidate" "$niri_bin" msg --json version >/dev/null 2>&1; then
                responsive+=("$candidate")
            fi
        done
        if [[ ${#responsive[@]} -eq 1 ]]; then
            niri_socket=${responsive[0]}
            break
        fi
        sleep 0.1
    done
    [[ -n $niri_socket ]] || fail "one responsive niri IPC socket did not appear"
    socket_name=${niri_socket##*/}
    socket_suffix=".$niri_pid.sock"
    [[ $socket_name == niri.*"$socket_suffix" ]] \
        || fail "niri IPC socket does not bind the launched supervisor PID"
    wayland_display=${socket_name#niri.}
    wayland_display=${wayland_display%"$socket_suffix"}
    [[ -S $runtime_root/$wayland_display ]] || fail "niri Wayland socket is missing"
    export NIRI_SOCKET="$niri_socket"
    export WAYLAND_DISPLAY="$wayland_display"
    export XDG_SESSION_TYPE=wayland
    export XDG_CURRENT_DESKTOP=niri
    unset DISPLAY XAUTHORITY

    ready=false
    for _ in {1..300}; do
        outputs=$("$niri_bin" msg --json outputs 2>/dev/null || true)
        if jq -e '
            def output_map: if has("Outputs") then .Outputs else . end;
            output_map as $o
            | ($o | length) == 2
            and ($o["DP-1"] as $d
                | $d.current_mode != null
                and ($d.modes[$d.current_mode].width == 2560)
                and ($d.modes[$d.current_mode].height == 1440)
                and ($d.modes[$d.current_mode].refresh_rate >= 59900)
                and ($d.modes[$d.current_mode].refresh_rate <= 60100)
                and ($d.logical.x == 0) and ($d.logical.y == 0)
                and ($d.logical.scale == 1))
            and ($o["DP-2"] as $d
                | $d.current_mode != null
                and ($d.modes[$d.current_mode].width == 1920)
                and ($d.modes[$d.current_mode].height == 1080)
                and ($d.modes[$d.current_mode].refresh_rate >= 59900)
                and ($d.modes[$d.current_mode].refresh_rate <= 60100)
                and ($d.logical.x == 2560) and ($d.logical.y == 0)
                and ($d.logical.scale == 1))
        ' <<<"$outputs" >/dev/null 2>&1; then
            ready=true
            break
        fi
        sleep 0.1
    done
    [[ $ready == true ]] || fail "the exact two-output niri topology did not become ready"
    "$niri_bin" msg action focus-monitor DP-1 >/dev/null
    focused=false
    for _ in {1..100}; do
        focus=$("$niri_bin" msg --json focused-output 2>/dev/null || true)
        if jq -e '
            if type == "object" and has("FocusedOutput") then .FocusedOutput else . end
            | .name == "DP-1"
        ' <<<"$focus" >/dev/null 2>&1; then
            focused=true
            break
        fi
        sleep 0.05
    done
    [[ $focused == true ]] || fail "DP-1 did not become niri's focused output"
    mapfile -t supervisors < <(owned_for_executable "$niri_bin")
    [[ ${#supervisors[@]} -eq 1 && ${supervisors[0]} == "$niri_pid" ]] \
        || fail "launched niri is not the sole owned niri process"
    attest_capture "$run" "$niri_pid"
}

run_xlibre_child() {
    local run=$1 wm_pid= xmonad_alias=
    mkdir -p "$adapter_root/xmonad-config" "$adapter_root/xmonad-data" "$adapter_root/xmonad-cache"
    export XMONAD_CONFIG_DIR="$adapter_root/xmonad-config"
    export XMONAD_DATA_DIR="$adapter_root/xmonad-data"
    export XMONAD_CACHE_DIR="$adapter_root/xmonad-cache"
    export XDG_SESSION_TYPE=x11
    export XDG_CURRENT_DESKTOP=XMonad

    xmonad_alias="$XMONAD_CACHE_DIR/xmonad-$(uname -m)-linux"
    ln -sfn "$xmonad" "$xmonad_alias"
    "$xmonad_alias" >"$adapter_root/xmonad.log" 2>&1 &
    wm_pid=$!
    cleanup_xmonad() {
        kill -0 "$wm_pid" 2>/dev/null && kill "$wm_pid" 2>/dev/null || true
        wait "$wm_pid" 2>/dev/null || true
    }
    trap cleanup_xmonad EXIT
    trap 'exit 129' HUP
    trap 'exit 130' INT
    trap 'exit 143' TERM
    require_x_topology
    ewmh_ready=false
    for _ in {1..300}; do
        if xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null | grep -q 'window id'; then
            ewmh_ready=true
            break
        fi
        kill -0 "$wm_pid" 2>/dev/null || fail "xmonad exited during startup"
        sleep 0.1
    done
    [[ $ewmh_ready == true ]] || fail "xmonad did not publish EWMH readiness"
    mapfile -t supervisors < <(owned_for_executable "$xserver")
    [[ ${#supervisors[@]} -eq 1 ]] || fail "expected one owned XLibre supervisor"
    mapfile -t window_managers < <(owned_for_executable "$xmonad")
    [[ ${#window_managers[@]} -eq 1 && ${window_managers[0]} == "$wm_pid" ]] \
        || fail "launched xmonad is not the sole isolated xmonad"
    attest_capture "$run" "${supervisors[0]}"
}

run_sophia() {
    local run=$1 workload=$2 result=0 launcher_status=0 child_tty= max_runtime_ms=95000
    local session_log="${XDG_STATE_HOME:-$HOME/.local/state}/sophia/hagia-session/session.log"
    local watchdog=125
    if [[ $workload == soak-2h ]]; then
        watchdog=7265
        max_runtime_ms=7235000
    fi

    record_gate_stage sophia-launch
    (
        [[ -t 0 ]] || fail "Sophia launcher stdin is not a terminal"
        child_tty=$(tty) || fail "Sophia launcher could not resolve its terminal"
        [[ $child_tty == "$operator_tty" ]] \
            || fail "Sophia launcher terminal changed before startup"
        export SOPHIA_TTY_PROFILE=hagia
        export SOPHIA_TTY_NUMBER=3
        export SOPHIA_BIN="$sophia_bin"
        export SOPHIA_HAGIA_BIN="$hagia_bin"
        export SOPHIA_HAGIA_SHELL_BIN="$narthex_bin"
        export SOPHIA_DESKTOP_PROFILE="$repo/validation/desktop-comparison/profiles/hagia.kdl"
        export SOPHIA_SESSION_STARTUP=none
        export SOPHIA_SESSION_WATCHDOG_SECONDS="$watchdog"
        export SOPHIA_BUILD_SESSION=false
        exec "$repo/tools/start_sophia_tty3.sh" --shell-process="$narthex_bin" \
            --max-runtime-ms="$max_runtime_ms"
    ) <&"$operator_tty_fd" &
    sophia_launcher=$!

    for _ in {1..600}; do
        if ! kill -0 "$sophia_launcher" 2>/dev/null; then
            if wait "$sophia_launcher"; then
                launcher_status=0
            else
                launcher_status=$?
            fi
            sophia_launcher=
            fail "Sophia launcher exited during startup with status $launcher_status; inspect /tmp/sophia-hagia-tty3-launch.log"
        fi
        supervisors=()
        while read -r pid; do
            [[ -n $pid ]] || continue
            cmdline=$(tr '\0' ' ' <"/proc/$pid/cmdline" 2>/dev/null || true)
            [[ $cmdline == *" session run "* ]] && supervisors+=("$pid")
        done < <(owned_for_executable "$sophia_bin")
        if [[ ${#supervisors[@]} -eq 1 ]]; then
            sophia_supervisor=${supervisors[0]}
            if grep -q '^sophia_live_session schema=1 status=desktop_ready startup_apps=0$' \
                "$session_log" 2>/dev/null; then
                break
            fi
        else
            sophia_supervisor=
        fi
        sleep 0.1
    done
    if [[ -z $sophia_supervisor ]] \
        || ! grep -q '^sophia_live_session schema=1 status=desktop_ready startup_apps=0$' \
            "$session_log" 2>/dev/null; then
        result=1
        echo "desktop comparison: terminal-free Sophia did not become ready" >&2
    else
        shopt -s nullglob
        authorities=("$runtime_root"/.sophia-Xauthority-"$sophia_supervisor"-77-*)
        shopt -u nullglob
        if [[ ${#authorities[@]} -ne 1 ]]; then
            result=1
            echo "desktop comparison: could not bind Sophia's owner-only X authority" >&2
        else
            export DISPLAY=:77
            export XAUTHORITY=${authorities[0]}
            export XDG_SESSION_TYPE=x11
            export XDG_CURRENT_DESKTOP=Hagia
            unset WAYLAND_DISPLAY WAYLAND_SOCKET
            require_x_topology || result=$?
            if [[ $result -eq 0 ]]; then
                attest_qualify_capture "$run" "$sophia_supervisor" || result=$?
            fi
        fi
    fi

    if [[ $result -eq 0 ]]; then
        record_gate_stage sophia-orderly-exit
        for _ in {1..450}; do
            kill -0 "$sophia_launcher" 2>/dev/null || break
            sleep 0.1
        done
        if kill -0 "$sophia_launcher" 2>/dev/null; then
            result=1
            echo "desktop comparison: Sophia did not reach its bounded orderly exit" >&2
        else
            if wait "$sophia_launcher"; then
                launcher_status=0
            else
                launcher_status=$?
            fi
            sophia_launcher=
            sophia_supervisor=
            if [[ $launcher_status -ne 0 ]]; then
                result=$launcher_status
                echo "desktop comparison: Sophia teardown returned status $launcher_status" >&2
            fi
        fi
    fi
    [[ $result -ne 0 ]] && cleanup_sophia_session
    return "$result"
}

if [[ ${1:-} == --internal-niri ]]; then
    [[ $# -eq 2 ]] || fail "invalid internal niri invocation"
    run_niri_child "$2"
    exit
fi
if [[ ${1:-} == --internal-xlibre ]]; then
    [[ $# -eq 2 ]] || fail "invalid internal XLibre invocation"
    run_xlibre_child "$2"
    exit
fi

[[ $# -eq 1 ]] || fail "usage: desktop_comparison_tty3.sh RUN"
run=$1
[[ -x $xtask ]] || fail "the invoking xtask executable was not provided"
record_gate_stage tty-admission
[[ -t 0 ]] || fail "standard input is not a terminal; invoke the gate directly from tty3"
operator_tty=$(tty) || fail "could not resolve the operator terminal"
[[ $operator_tty == /dev/tty3 ]] || fail "run this one-row gate from tty3"
[[ $(< /sys/class/tty/tty0/active) == tty3 ]] || fail "tty3 must be the active VT"
# Login already admitted this inherited descriptor. Reopening the device path
# can fail after its ownership returns to root:tty, even while fd 0 remains valid.
operator_tty_fd=
exec {operator_tty_fd}<&0 || fail "could not duplicate the inherited operator terminal"
[[ -t $operator_tty_fd ]] || fail "the validated operator terminal descriptor is not a TTY"
operator_tty_fd_tty=$(tty <&"$operator_tty_fd") \
    || fail "could not resolve the duplicated operator terminal"
[[ $operator_tty_fd_tty == "$operator_tty" ]] \
    || fail "the duplicated operator terminal changed identity"
command -v jq >/dev/null || fail "jq is not installed"

record_gate_stage candidate-admission
status=$("$xtask" conformance desktop-comparison status "$run")
printf '%s\n' "$status"
next_stack=$(sed -n 's/.* next_stack=\([^ ]*\).*/\1/p' <<<"$status")
next_workload=$(sed -n 's/.* next_workload=\([^ ]*\).*/\1/p' <<<"$status")
[[ -n $next_stack && -n $next_workload ]] || fail "comparison matrix has no pending row"

record_gate_stage tracefs-admission
if ! mountpoint -q /sys/kernel/tracing; then
    sudo mount -t tracefs tracefs /sys/kernel/tracing
fi
if trace_probe=$(sudo -- "$trace_helper" --probe 2>&1); then
    printf '%s\n' "$trace_probe"
else
    trace_status=$?
    fail "tracefs probe failed with status $trace_status: $trace_probe"
fi

record_gate_stage stack-admission
case "$next_stack" in
    sophia)
        [[ -x $sophia_bin ]] || fail "release Sophia binary is missing; build it before the gate"
        [[ -x $hagia_bin && -x $narthex_bin ]] || fail "Hagia and Narthex comparison binaries are missing"
        run_sophia "$run" "$next_workload"
        ;;
    xlibre-xmonad)
        [[ -x $xserver && -x $xmonad ]] || fail "the pinned XLibre/xmonad prefix is incomplete"
        [[ -f $xlibre_modules/input/libinput_drv.so ]] \
            || fail "the pinned XLibre ABI-26 libinput driver is missing"
        [[ ! -e /tmp/.X1-lock && ! -S /tmp/.X11-unix/X1 ]] \
            || fail "X display :1 is already in use"
        export XAUTHORITY="$adapter_root/Xauthority"
        : >"$XAUTHORITY"
        chmod 600 "$XAUTHORITY"
        export XDG_SESSION_TYPE=x11
        export XDG_CURRENT_DESKTOP=XMonad
        startx "$repo/tools/desktop_comparison_tty3.sh" --internal-xlibre "$run" -- \
            "$xserver" :1 vt3 -keeptty -nolisten tcp -modulepath "$xlibre_modules" \
            -logfile "$adapter_root/Xorg.log" -logverbose 6
        ;;
    niri)
        [[ -x $niri_bin ]] || fail "niri is not installed at $niri_bin"
        "$niri_bin" validate --config "$repo/validation/desktop-comparison/profiles/niri.kdl"
        export NIRI_CONFIG="$repo/validation/desktop-comparison/profiles/niri.kdl"
        unset NIRI_SOCKET WAYLAND_DISPLAY WAYLAND_SOCKET DISPLAY XAUTHORITY
        dbus-run-session -- "$repo/tools/desktop_comparison_tty3.sh" --internal-niri "$run"
        ;;
    *)
        fail "typed schedule named unknown stack $next_stack"
        ;;
esac

record_gate_stage post-teardown-finalization
"$xtask" conformance desktop-comparison finalize "$run"
"$xtask" conformance desktop-comparison status "$run"
