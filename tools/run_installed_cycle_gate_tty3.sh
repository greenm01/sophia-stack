#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
RELEASE_DIR="$(cd "$(dirname "$SCRIPT_PATH")/.." && pwd)"
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
RUN_ROOT="$STATE_HOME/sophia/promotion/runs"
SESSION_LOG="$STATE_HOME/sophia/xmonad-session/session.log"
RUNTIME_ROOT="${XDG_RUNTIME_DIR:-}"
COUNT="${1:-10}"
STARTUP_TIMEOUT_SECONDS="${SOPHIA_CYCLE_STARTUP_TIMEOUT_SECONDS:-20}"
SESSION_TIMEOUT_SECONDS="${SOPHIA_CYCLE_SESSION_TIMEOUT_SECONDS:-30}"
INJECTOR="$RELEASE_DIR/tools/probes/uinput_text_injector.py"
SESSION="$RELEASE_DIR/bin/sophia-session"
VERIFY="$RELEASE_DIR/bin/sophia-verify-cycles"
FAILURE_ROOT="$STATE_HOME/sophia/promotion/cycle-runner-failures"
RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)-$$"
WORK_DIR=
INJECTOR_PID=
SESSION_PID=
SESSION_INPUT_FD=0

fail() {
    echo "Sophia installed-cycle gate failed: $*" >&2
    exit 1
}

latest_run() {
    find "$RUN_ROOT" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' \
        2>/dev/null | grep -E '^[0-9]+$' | sort -V | tail -n 1
}

session_log_identity() {
    stat -c '%d:%i' "$SESSION_LOG" 2>/dev/null || true
}

new_session_is_ready() {
    local previous_identity="$1" current_identity
    current_identity="$(session_log_identity)"
    [[ -n "$current_identity" && "$current_identity" != "$previous_identity" ]] ||
        return 1
    grep -Eq \
        '^sophia_live_session_startup schema=2 status=ready .* outputs_ready=2/2 recovery_attempts=0$' \
        "$SESSION_LOG"
}

terminate_process() {
    local pid="$1" state
    [[ -n "$pid" ]] || return 0
    if kill -0 "$pid" 2>/dev/null; then
        kill -TERM "$pid" 2>/dev/null || true
    fi
    for _ in {1..50}; do
        state="$(ps -o stat= -p "$pid" 2>/dev/null || true)"
        [[ -z "$state" || "$state" == Z* ]] && break
        sleep 0.1
    done
    state="$(ps -o stat= -p "$pid" 2>/dev/null || true)"
    if [[ -n "$state" && "$state" != Z* ]]; then
        kill -KILL "$pid" 2>/dev/null || true
    fi
}

stop_process() {
    local pid="$1"
    [[ -n "$pid" ]] || return 0
    terminate_process "$pid"
    wait "$pid" 2>/dev/null || true
}

process_is_running() {
    local pid="$1" state
    state="$(ps -o stat= -p "$pid" 2>/dev/null || true)"
    [[ -n "$state" && "$state" != Z* ]]
}

launch_installed_session() {
    local input_device="$1" wrapper_log="$2"
    env \
        SOPHIA_OPERATOR_INPUT_DEVICES="$input_device" \
        SOPHIA_SESSION_HANDOFF=cycle_runner \
        SOPHIA_ATTEMPT_LIFECYCLE_MODE=cycle \
        "$SESSION" \
        <&"$SESSION_INPUT_FD" >"$wrapper_log" 2>&1 &
    SESSION_PID=$!
}

cleanup() {
    local status=$? failure_dir
    stop_process "$SESSION_PID"
    stop_process "$INJECTOR_PID"
    if [[ -n "$WORK_DIR" && -d "$WORK_DIR" ]]; then
        if ((status == 0)); then
            rm -rf -- "$WORK_DIR"
        else
            install -d -m 700 "$FAILURE_ROOT"
            failure_dir="$FAILURE_ROOT/$RUN_ID"
            mv "$WORK_DIR" "$failure_dir"
            WORK_DIR=
            echo "Cycle-runner diagnostics retained in $failure_dir" >&2
        fi
    fi
    return "$status"
}

self_test() {
    local fixture previous_identity test_fd test_pid
    fixture="$(mktemp -d)"
    SESSION_LOG="$fixture/session.log"
    RUN_ROOT="$fixture/runs"
    mkdir -p "$RUN_ROOT/0009" "$RUN_ROOT/0010" "$RUN_ROOT/not-a-run"
    [[ "$(latest_run)" == 0010 ]] || {
        echo "cycle runner did not select the latest numeric attempt" >&2
        rm -rf -- "$fixture"
        return 1
    }

    printf '%s\n' \
        'sophia_live_session_startup schema=2 status=ready elapsed_msec=10 surface=true visual_detail=true presented=true outputs_ready=2/2 recovery_attempts=0' \
        >"$SESSION_LOG"
    previous_identity="$(session_log_identity)"
    if new_session_is_ready "$previous_identity"; then
        echo "cycle runner accepted readiness from the preceding session" >&2
        rm -rf -- "$fixture"
        return 1
    fi

    mv "$SESSION_LOG" "$fixture/session.previous.log"
    printf '%s\n' \
        'sophia_live_session_startup schema=2 status=content_ready source=cpu_visual_detail nonzero_rgb_pixels=1' \
        >"$SESSION_LOG"
    if new_session_is_ready "$previous_identity"; then
        echo "cycle runner accepted an incidental startup record" >&2
        rm -rf -- "$fixture"
        return 1
    fi
    printf '%s\n' \
        'sophia_live_session_startup schema=2 status=ready elapsed_msec=10 surface=true visual_detail=true presented=true outputs_ready=2/2 recovery_attempts=0' \
        >>"$SESSION_LOG"
    new_session_is_ready "$previous_identity" || {
        echo "cycle runner rejected exact readiness from a new session" >&2
        rm -rf -- "$fixture"
        return 1
    }

    sleep 30 &
    test_pid=$!
    process_is_running "$test_pid" || {
        echo "cycle runner did not observe a live child" >&2
        stop_process "$test_pid"
        rm -rf -- "$fixture"
        return 1
    }
    stop_process "$test_pid"
    if process_is_running "$test_pid"; then
        echo "cycle runner left a stopped child running" >&2
        rm -rf -- "$fixture"
        return 1
    fi

    printf 'preserved-stdin\n' >"$fixture/input"
    printf '%s\n' \
        '#!/usr/bin/env bash' \
        'set -euo pipefail' \
        'read -r value' \
        'printf "%s\n" "$value"' \
        >"$fixture/fake-session"
    chmod 700 "$fixture/fake-session"
    exec {test_fd}<"$fixture/input"
    SESSION_INPUT_FD="$test_fd"
    SESSION="$fixture/fake-session"
    launch_installed_session /dev/input/event-test "$fixture/wrapper.log"
    wait "$SESSION_PID"
    SESSION_PID=
    exec {test_fd}<&-
    grep -Fxq preserved-stdin "$fixture/wrapper.log" || {
        echo "cycle runner did not preserve stdin for an asynchronous session" >&2
        rm -rf -- "$fixture"
        return 1
    }

    rm -rf -- "$fixture"
    "$INJECTOR" --chord=logout --self-test
    echo "installed cycle runner checks passed"
}

if [[ "${1:-}" == --help ]]; then
    cat <<'EOF'
Usage: sophia-run-cycles [COUNT]

Run from a logged-in local text VT with DRM released. The default performs ten
installed Sophia startup/logout cycles. Each cycle waits for exact two-output
readiness, sends Super+Shift+Q through a bounded uinput keyboard, verifies its
immutable archive, and stops at the first failure.
EOF
    exit 0
fi
if [[ "${1:-}" == --self-test ]]; then
    [[ $# -eq 1 ]] || fail "--self-test does not accept additional arguments"
    self_test
    exit 0
fi
[[ $# -le 1 ]] || fail "unexpected arguments (use --help)"
[[ "$COUNT" =~ ^[1-9][0-9]*$ && "$COUNT" -le 100 ]] ||
    fail "COUNT must be an integer from 1 through 100"
for timeout in "$STARTUP_TIMEOUT_SECONDS" "$SESSION_TIMEOUT_SECONDS"; do
    [[ "$timeout" =~ ^[1-9][0-9]*$ ]] ||
        fail "cycle timeouts must be positive integers"
done
[[ -t 0 && "$(tty)" =~ ^/dev/tty[0-9]+$ ]] ||
    fail "run this interactively from a logged-in local text VT"
# Duplicate greetd's descriptor; reopening /dev/tty erases the concrete VT name.
exec {SESSION_INPUT_FD}<&0 || fail "the local VT could not be retained"
[[ -n "$RUNTIME_ROOT" && "$RUNTIME_ROOT" == /* && -d "$RUNTIME_ROOT" ]] ||
    fail "XDG_RUNTIME_DIR must be an existing absolute directory"
[[ "$(stat -c %u "$RUNTIME_ROOT")" == "$UID" ]] ||
    fail "XDG_RUNTIME_DIR must be owned by the current user"
[[ -x "$INJECTOR" && -x "$SESSION" && -x "$VERIFY" ]] ||
    fail "the installed release is missing cycle-runner components"
[[ -e /dev/uinput && -w /dev/uinput ]] ||
    fail "/dev/uinput is not writable; run sophia-setup-uinput once"

(
    cd "$RELEASE_DIR"
    sha256sum -c SHA256SUMS >/dev/null
) || fail "the installed release failed checksum verification"

WORK_DIR="$(mktemp -d "$RUNTIME_ROOT/sophia-cycle-gate.XXXXXX")"
chmod 700 "$WORK_DIR"
trap cleanup EXIT

for ((cycle = 1; cycle <= COUNT; cycle++)); do
    cycle_dir="$WORK_DIR/cycle-$(printf '%03d' "$cycle")"
    mkdir "$cycle_dir"
    chmod 700 "$cycle_dir"
    ready_file="$cycle_dir/device"
    trigger_file="$cycle_dir/inject"
    result_file="$cycle_dir/injected-at-usec"
    timeout_file="$cycle_dir/session-timeout"
    previous_log_identity="$(session_log_identity)"
    previous_run="$(latest_run || true)"

    "$INJECTOR" \
        --chord=logout \
        --ready-file="$ready_file" \
        --trigger-file="$trigger_file" \
        --result-file="$result_file" \
        --timeout-seconds="$((STARTUP_TIMEOUT_SECONDS + SESSION_TIMEOUT_SECONDS))" \
        --key-interval-ms=8 \
        >"$cycle_dir/injector.log" 2>&1 &
    INJECTOR_PID=$!

    device_deadline=$((SECONDS + 5))
    while [[ ! -s "$ready_file" && $SECONDS -lt $device_deadline ]]; do
        kill -0 "$INJECTOR_PID" 2>/dev/null ||
            fail "uinput keyboard exited before cycle $cycle became ready"
        sleep 0.01
    done
    [[ -s "$ready_file" ]] || fail "uinput keyboard did not appear for cycle $cycle"
    input_device="$(<"$ready_file")"
    [[ "$input_device" == /dev/input/event* && -e "$input_device" ]] ||
        fail "uinput keyboard published an invalid event device"

    launch_installed_session "$input_device" "$cycle_dir/wrapper.log"
    session_deadline=$((SECONDS + SESSION_TIMEOUT_SECONDS))

    startup_deadline=$((SECONDS + STARTUP_TIMEOUT_SECONDS))
    while ! new_session_is_ready "$previous_log_identity"; do
        kill -0 "$SESSION_PID" 2>/dev/null || break
        ((SECONDS < startup_deadline)) || break
        sleep 0.02
    done
    new_session_is_ready "$previous_log_identity" ||
        fail "cycle $cycle did not reach exact two-output readiness"

    # Keep the virtual keyboard alive through release delivery and shutdown.
    sleep 0.25
    : >"$trigger_file"
    chmod 600 "$trigger_file"

    while process_is_running "$SESSION_PID"; do
        if ((SECONDS >= session_deadline)); then
            : >"$timeout_file"
            terminate_process "$SESSION_PID"
            break
        fi
        sleep 0.02
    done

    set +e
    wait "$SESSION_PID"
    session_status=$?
    set -e
    SESSION_PID=
    stop_process "$INJECTOR_PID"
    INJECTOR_PID=

    [[ ! -e "$timeout_file" ]] || fail "cycle $cycle exceeded its session deadline"
    ((session_status == 0)) || fail "cycle $cycle exited with status $session_status"
    [[ -s "$result_file" ]] || fail "cycle $cycle did not inject the logout chord"
    current_run="$(latest_run || true)"
    [[ -n "$current_run" && "$current_run" != "$previous_run" ]] ||
        fail "cycle $cycle did not create one new immutable attempt"
    "$VERIFY" 1 "$current_run" >/dev/null ||
        fail "cycle $cycle archive $current_run did not verify"
    printf 'sophia_installed_cycle_runner schema=1 status=cycle_complete cycle=%s/%s run=%s\n' \
        "$cycle" "$COUNT" "$current_run"
done

through_run="$(latest_run)"
"$VERIFY" "$COUNT" "$through_run"
printf 'sophia_installed_cycle_runner schema=1 status=complete cycles=%s through=%s\n' \
    "$COUNT" "$through_run"
