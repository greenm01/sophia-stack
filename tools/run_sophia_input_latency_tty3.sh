#!/usr/bin/env bash
set -euo pipefail

# Commit-pinned physical input-to-photon gate. Each sample creates a Linux
# uinput keyboard, routes "sophia" through the normal libinput worker, and
# requires the resulting pixels to retire on a real kernel page flip.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
# Sessions, not samples: each session now carries its own latency
# distribution, so the p99 population is the presses within these runs rather
# than one value per run.
SAMPLES="${SOPHIA_INPUT_LATENCY_SAMPLES:-20}"
# The reporter now reads refresh from each session's own head record; this
# remains only as the fallback for evidence that predates that record.
REFRESH_MSEC="${SOPHIA_INPUT_LATENCY_REFRESH_MSEC:-17}"
MAX_QUEUE_DWELL_MSEC="${SOPHIA_INPUT_LATENCY_MAX_QUEUE_DWELL_MSEC:-1}"
MAX_DWELL_TO_SUBMIT_MSEC="${SOPHIA_INPUT_LATENCY_MAX_DWELL_TO_SUBMIT_MSEC:-}"
MAX_SUBMIT_TO_FLIP_MSEC="${SOPHIA_INPUT_LATENCY_MAX_SUBMIT_TO_FLIP_MSEC:-}"
KEY_INTERVAL_MSEC=0
MAX_SESSION_START_ATTEMPTS=3
# How long one sample's session may run after injection is triggered.
#
# `SOPHIA_LIVE_SESSION_RUNTIME_MSEC` does not bound this: with an input proof
# requested, the global runtime deadline only bounds startup and never ends the
# session (`global_runtime_deadline_ends_session`). So a proof that never
# completes leaves the session running and this script waiting on it forever,
# which is what a hung run looks like from the outside.
PROOF_TIMEOUT_SECONDS="${SOPHIA_INPUT_LATENCY_PROOF_TIMEOUT_SECONDS:-90}"
# The seat the emergency guard listens on. Sophia is given only the virtual
# injector device, so it never reacts to the real keyboard; but it does take
# DRM master, so the console is behind its output and an operator has no way
# to interrupt a wedged run. The guard is that way.
GUARD_SEAT="${SOPHIA_INPUT_LATENCY_GUARD_SEAT:-seat0}"
COMMIT="$(git -C "$ROOT_DIR" rev-parse HEAD)"
RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)"
ARCHIVE_ROOT="$STATE_HOME/sophia/rendering-benchmarks/$COMMIT/input-latency"
PENDING="$ARCHIVE_ROOT/.${RUN_ID}.pending"
FINAL="$ARCHIVE_ROOT/$RUN_ID"
INJECTOR_PID=
PROOF_PID=

fail() {
    echo "Sophia input latency gate failed: $*" >&2
    exit 1
}

is_retryable_pre_input_cursor_failure() {
    local session_log="$1" trigger_file="$2" result_file="$3"

    [[ -s "$session_log" && ! -e "$trigger_file" && ! -e "$result_file" ]] &&
        ! grep -Fq \
            'sophia_live_session_input schema=1 status=ready source=physical' \
            "$session_log" &&
        ! grep -Fq \
            'sophia_live_input_latency schema=1 status=complete ' \
            "$session_log" &&
        grep -Fq \
            'sophia_live_session_pointer schema=2 status=unavailable source=hardware_cursor error=hardware cursor update failed: Permission denied (os error 13)' \
            "$session_log" &&
        grep -Fq \
            'native session cannot provide an owned atomic cursor: hardware cursor update failed: Permission denied (os error 13)' \
            "$session_log"
}

check_retry_classifier() {
    local fixture status
    fixture="$(mktemp -d)"
    status=0

    printf '%s\n' \
        'sophia_live_session_pointer schema=2 status=unavailable source=hardware_cursor error=hardware cursor update failed: Permission denied (os error 13)' \
        'Error: "native session cannot provide an owned atomic cursor: hardware cursor update failed: Permission denied (os error 13)"' \
        >"$fixture/session.log"
    if ! is_retryable_pre_input_cursor_failure \
        "$fixture/session.log" "$fixture/inject" "$fixture/result"; then
        echo "pre-input cursor EACCES was not classified as retryable" >&2
        status=1
    fi

    : >"$fixture/inject"
    if is_retryable_pre_input_cursor_failure \
        "$fixture/session.log" "$fixture/inject" "$fixture/result"; then
        echo "cursor EACCES after an injection trigger was classified as retryable" >&2
        status=1
    fi
    rm -f -- "$fixture/inject"

    printf '%s\n' \
        'sophia_live_session_input schema=1 status=ready source=physical' \
        >>"$fixture/session.log"
    if is_retryable_pre_input_cursor_failure \
        "$fixture/session.log" "$fixture/inject" "$fixture/result"; then
        echo "cursor EACCES after physical-input readiness was classified as retryable" >&2
        status=1
    fi

    rm -rf -- "$fixture"
    ((status == 0)) || return "$status"
    echo "Sophia input latency retry classifier checks passed"
}

stop_children() {
    if [[ -n "$PROOF_PID" ]]; then
        kill "$PROOF_PID" 2>/dev/null || true
        wait "$PROOF_PID" 2>/dev/null || true
        PROOF_PID=
    fi
    if [[ -n "$INJECTOR_PID" ]]; then
        kill "$INJECTOR_PID" 2>/dev/null || true
        wait "$INJECTOR_PID" 2>/dev/null || true
        INJECTOR_PID=
    fi
}

preserve_pending() {
    local status="$1"
    stop_children
    if ((status != 0)) && [[ -d "$PENDING" ]]; then
        echo "Incomplete evidence retained in $PENDING" >&2
    fi
}

if [[ "${1:-}" == --help ]]; then
    cat <<EOF
Usage: tools/run_sophia_input_latency_tty3.sh

Run from a logged-in local TTY3 with DRM released and /dev/uinput writable.
The default gate collects 20 independent samples and requires p95 below
two 17 ms refresh periods. It separately caps queue dwell at 1 ms,
dwell-to-submit at 17 ms, and submit-to-flip at 17 ms. Override the sample
count, refresh period, or stage budgets with the SOPHIA_INPUT_LATENCY_*
environment variables.
EOF
    exit 0
fi
terminate_proof() {
    local pid="$1"
    [[ -n "$pid" ]] || return 0
    kill -0 "$pid" 2>/dev/null || return 0
    pkill -TERM -P "$pid" 2>/dev/null || true
    kill -TERM "$pid" 2>/dev/null || true
    local deadline=$((SECONDS + 5))
    while kill -0 "$pid" 2>/dev/null && ((SECONDS < deadline)); do
        sleep 0.05
    done
    if kill -0 "$pid" 2>/dev/null; then
        pkill -KILL -P "$pid" 2>/dev/null || true
        kill -KILL "$pid" 2>/dev/null || true
    fi
}

check_proof_termination() {
    # The failure this bounds is a session that will not exit on its own, so
    # the tree must ignore TERM at every level. An earlier version of this
    # check had only the parent ignore it: its child `sleep` died on TERM and
    # took the parent down with it, so the escalation never ran and the check
    # passed even with the KILL path removed.
    local parent child
    bash -c 'trap "" TERM
             bash -c '"'"'trap "" TERM; echo $$ >"$1"; while :; do sleep 1; done'"'"' _ "$1" &
             wait' _ "$PROOF_TEST_CHILD_FILE" &
    parent=$!
    local ready=$((SECONDS + 5))
    while [[ ! -s "$PROOF_TEST_CHILD_FILE" ]] && ((SECONDS < ready)); do
        sleep 0.05
    done
    child="$(<"$PROOF_TEST_CHILD_FILE")"
    [[ -n "$child" ]] || fail "self-test child never started"

    terminate_proof "$parent"
    # Bounded, because an unbounded wait here is the very defect under test: a
    # terminator that fails to kill leaves this check hanging instead of
    # reporting, which is how a broken escalation would look like a slow pass.
    local settled=$((SECONDS + 5))
    while kill -0 "$parent" 2>/dev/null && ((SECONDS < settled)); do
        sleep 0.05
    done

    if kill -0 "$parent" 2>/dev/null; then
        kill -KILL "$parent" 2>/dev/null || true
        kill -KILL "$child" 2>/dev/null || true
        fail "terminate_proof left a TERM-ignoring session running"
    fi
    if kill -0 "$child" 2>/dev/null; then
        kill -KILL "$child" 2>/dev/null || true
        fail "terminate_proof left the session's child running"
    fi
    printf 'sophia_input_latency_runner schema=1 status=self_test_passed check=proof_termination\n'
}

if [[ "${1:-}" == --self-test ]]; then
    [[ $# -eq 1 ]] || fail "--self-test does not accept additional arguments"
    check_retry_classifier
    PROOF_TEST_CHILD_FILE="$(mktemp)"
    trap 'rm -f -- "$PROOF_TEST_CHILD_FILE"' EXIT
    check_proof_termination
    exit 0
fi
[[ $# -eq 0 ]] || fail "unexpected arguments (use --help)"
[[ "$SAMPLES" =~ ^[1-9][0-9]*$ && "$SAMPLES" -le 100 ]] ||
    fail "SOPHIA_INPUT_LATENCY_SAMPLES must be an integer from 1 through 100"
[[ "$REFRESH_MSEC" =~ ^[1-9][0-9]*$ ]] ||
    fail "SOPHIA_INPUT_LATENCY_REFRESH_MSEC must be a positive integer"
if [[ -z "$MAX_DWELL_TO_SUBMIT_MSEC" ]]; then
    MAX_DWELL_TO_SUBMIT_MSEC="$REFRESH_MSEC"
fi
if [[ -z "$MAX_SUBMIT_TO_FLIP_MSEC" ]]; then
    MAX_SUBMIT_TO_FLIP_MSEC="$REFRESH_MSEC"
fi
for budget in "$MAX_QUEUE_DWELL_MSEC" "$MAX_DWELL_TO_SUBMIT_MSEC" \
    "$MAX_SUBMIT_TO_FLIP_MSEC"; do
    [[ "$budget" =~ ^[0-9]+$ ]] ||
        fail "stage latency budgets must be nonnegative integers"
done
[[ -t 0 && "$(tty)" == /dev/tty3 ]] ||
    fail "run this interactively from a logged-in local TTY3"
[[ -e /dev/uinput ]] ||
    fail "/dev/uinput is missing (run tools/setup_sophia_uinput.sh first)"
[[ -w /dev/uinput ]] ||
    fail "/dev/uinput is not writable (run tools/setup_sophia_uinput.sh, then start a fresh login or run newgrp input)"
[[ -z "$(git -C "$ROOT_DIR" status --porcelain)" ]] ||
    fail "commit or discard the dirty worktree before collecting evidence"
[[ ! -e "$FINAL" && ! -e "$PENDING" ]] ||
    fail "evidence destination already exists: $FINAL"

mkdir -p "$ARCHIVE_ROOT"
mkdir "$PENDING"
chmod 700 "$PENDING"
# A run interrupted at the console must not leave a session holding the GPU.
trap 'terminate_proof "${PROOF_PID:-}"; kill "${INJECTOR_PID:-}" 2>/dev/null || true; kill "${GUARD_PID:-}" 2>/dev/null || true; preserve_pending $?' EXIT

printf 'source_commit=%s\nrun_id=%s\nsamples=%s\nrefresh_msec=%s\nend_to_end_budget_refreshes=2\nmax_queue_dwell_msec=%s\nmax_dwell_to_submit_msec=%s\nmax_submit_to_flip_msec=%s\nkey_interval_msec=%s\nmax_session_start_attempts=%s\n' \
    "$COMMIT" "$RUN_ID" "$SAMPLES" "$REFRESH_MSEC" \
    "$MAX_QUEUE_DWELL_MSEC" "$MAX_DWELL_TO_SUBMIT_MSEC" \
    "$MAX_SUBMIT_TO_FLIP_MSEC" "$KEY_INTERVAL_MSEC" \
    "$MAX_SESSION_START_ATTEMPTS" \
    >"$PENDING/source.env"
chmod 600 "$PENDING/source.env"

cd "$ROOT_DIR"
tools/probes/uinput_text_injector.py \
    "--key-interval-ms=$KEY_INTERVAL_MSEC" --self-test |
    tee "$PENDING/injector-self-test.log"
cargo build --quiet --release --offline -p sophia-cli \
    --features atomic-scanout-live
tools/atomic_scanout_preflight.sh | tee "$PENDING/preflight.log"

GUARD_ARMED_FILE="$PENDING/input-guard.armed"
GUARD_TRIGGERED_FILE="$PENDING/input-guard.triggered"
rm -f "$GUARD_ARMED_FILE" "$GUARD_TRIGGERED_FILE"
"$ROOT_DIR/target/release/sophia" sophia-session-input-guard \
    "--input-seat=$GUARD_SEAT" \
    --armed-file="$GUARD_ARMED_FILE" \
    --triggered-file="$GUARD_TRIGGERED_FILE" \
    --owner-pid="$$" >>"$PENDING/input-guard.log" 2>&1 &
GUARD_PID=$!
echo "Safety check: press and release Ctrl-Alt-Backspace once to arm recovery."
echo "During a sample, press Ctrl-Alt-Backspace again to abort the run."
for _ in {1..600}; do
    [[ ! -s "$GUARD_ARMED_FILE" ]] || break
    kill -0 "$GUARD_PID" 2>/dev/null || {
        fail "input guard exited before arming; see $PENDING/input-guard.log"
    }
    sleep 0.05
done
[[ -s "$GUARD_ARMED_FILE" ]] ||
    fail "input guard was not armed within 30 seconds; refusing graphics takeover"
echo "Emergency input guard armed."

for ((sample = 1; sample <= SAMPLES; sample++)); do
    sample_dir="$PENDING/sample-$(printf '%03d' "$sample")"
    mkdir "$sample_dir"
    chmod 700 "$sample_dir"
    ready_file="$sample_dir/device"
    trigger_file="$sample_dir/inject"
    result_file="$sample_dir/injected-at-usec"
    session_log="$sample_dir/session.log"

    tools/probes/uinput_text_injector.py \
        --ready-file="$ready_file" \
        --trigger-file="$trigger_file" \
        --result-file="$result_file" \
        --timeout-seconds=180 \
        "--key-interval-ms=$KEY_INTERVAL_MSEC" \
        >"$sample_dir/injector.log" 2>&1 &
    INJECTOR_PID=$!

    ready_deadline=$((SECONDS + 5))
    while [[ ! -s "$ready_file" && $SECONDS -lt $ready_deadline ]]; do
        kill -0 "$INJECTOR_PID" 2>/dev/null ||
            fail "uinput injector exited before sample $sample became ready"
        sleep 0.01
    done
    [[ -s "$ready_file" ]] ||
        fail "uinput event node did not become ready for sample $sample"
    input_device="$(<"$ready_file")"
    [[ "$input_device" == /dev/input/event* && -e "$input_device" ]] ||
        fail "injector published an invalid input device: $input_device"

    for ((attempt = 1; attempt <= MAX_SESSION_START_ATTEMPTS; attempt++)); do
        SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$session_log" \
            SOPHIA_LIVE_SESSION_RUNTIME_MSEC=30000 \
            SOPHIA_LIVE_SESSION_SKIP_BUILD=1 \
            SOPHIA_ATOMIC_SCANOUT_SKIP_PREFLIGHT=1 \
            tools/live_session_persistent_hardware_proof.sh \
            "--input-devices=$input_device" \
            --expect-physical-text=sophia \
            --exit-after-input-proof &
        PROOF_PID=$!

        proof_exited=0
        session_deadline=$((SECONDS + 20))
        while ! grep -Fq \
            'sophia_live_session_input schema=1 status=ready source=physical' \
            "$session_log" 2>/dev/null; do
            if ! kill -0 "$PROOF_PID" 2>/dev/null; then
                proof_exited=1
                break
            fi
            ((SECONDS < session_deadline)) ||
                fail "Sophia did not request physical input for sample $sample"
            sleep 0.01
        done
        ((proof_exited == 0)) && break

        set +e
        wait "$PROOF_PID"
        proof_status=$?
        PROOF_PID=
        set -e
        if ((proof_status != 0 && attempt < MAX_SESSION_START_ATTEMPTS)) &&
            is_retryable_pre_input_cursor_failure \
                "$session_log" "$trigger_file" "$result_file"; then
            failed_session_log="$sample_dir/session-start-attempt-$(printf '%03d' "$attempt").log"
            mv "$session_log" "$failed_session_log"
            printf 'sophia_input_latency_runner schema=1 status=retrying sample=%s attempt=%s reason=pre_input_cursor_eacces\n' \
                "$sample" "$attempt" | tee -a "$sample_dir/attempts.log"
            sleep 0.1
            continue
        fi
        fail "Sophia exited before sample $sample requested physical input (status $proof_status)"
    done

    : >"$trigger_file"
    chmod 600 "$trigger_file"

    proof_deadline=$((SECONDS + PROOF_TIMEOUT_SECONDS))
    while kill -0 "$PROOF_PID" 2>/dev/null; do
        if [[ -s "$GUARD_TRIGGERED_FILE" ]]; then
            terminate_proof "$PROOF_PID"
            set +e
            wait "$PROOF_PID"
            set -e
            PROOF_PID=
            kill "$INJECTOR_PID" 2>/dev/null || true
            wait "$INJECTOR_PID" 2>/dev/null || true
            INJECTOR_PID=
            fail "operator requested emergency recovery during sample $sample"
        fi
        if ((SECONDS >= proof_deadline)); then
            terminate_proof "$PROOF_PID"
            set +e
            wait "$PROOF_PID"
            set -e
            PROOF_PID=
            kill "$INJECTOR_PID" 2>/dev/null || true
            wait "$INJECTOR_PID" 2>/dev/null || true
            INJECTOR_PID=
            fail "Sophia did not complete the input proof for sample $sample within ${PROOF_TIMEOUT_SECONDS}s"
        fi
        sleep 0.05
    done

    set +e
    wait "$PROOF_PID"
    proof_status=$?
    PROOF_PID=
    set -e
    kill "$INJECTOR_PID" 2>/dev/null || true
    wait "$INJECTOR_PID" 2>/dev/null || true
    INJECTOR_PID=
    ((proof_status == 0)) ||
        fail "Sophia proof failed for sample $sample with status $proof_status"
    [[ -s "$result_file" ]] ||
        fail "injector did not record sample $sample"
done

set +e
SOPHIA_INPUT_LATENCY_REFRESH_MSEC="$REFRESH_MSEC" \
SOPHIA_INPUT_LATENCY_MAX_QUEUE_DWELL_MSEC="$MAX_QUEUE_DWELL_MSEC" \
SOPHIA_INPUT_LATENCY_MAX_DWELL_TO_SUBMIT_MSEC="$MAX_DWELL_TO_SUBMIT_MSEC" \
SOPHIA_INPUT_LATENCY_MAX_SUBMIT_TO_FLIP_MSEC="$MAX_SUBMIT_TO_FLIP_MSEC" \
    tools/report_sophia_input_latency.sh \
    "$PENDING"/sample-*/session.log | tee "$PENDING/report.log"
report_status="${PIPESTATUS[0]}"
set -e

mv "$PENDING" "$FINAL"
trap - EXIT
echo "Evidence retained in $FINAL"
exit "$report_status"
