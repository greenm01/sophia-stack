#!/usr/bin/env bash
set -euo pipefail

# Commit-pinned physical input-to-photon gate. Each sample creates a Linux
# uinput keyboard, routes "sophia" through the normal libinput worker, and
# requires the resulting pixels to retire on a real kernel page flip.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
SAMPLES="${SOPHIA_INPUT_LATENCY_SAMPLES:-20}"
REFRESH_BUDGET_MSEC="${SOPHIA_INPUT_LATENCY_REFRESH_MSEC:-17}"
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
17 ms. Override with SOPHIA_INPUT_LATENCY_SAMPLES and
SOPHIA_INPUT_LATENCY_REFRESH_MSEC.
EOF
    exit 0
fi
[[ $# -eq 0 ]] || fail "unexpected arguments (use --help)"
[[ "$SAMPLES" =~ ^[1-9][0-9]*$ && "$SAMPLES" -le 100 ]] ||
    fail "SOPHIA_INPUT_LATENCY_SAMPLES must be an integer from 1 through 100"
[[ "$REFRESH_BUDGET_MSEC" =~ ^[1-9][0-9]*$ ]] ||
    fail "SOPHIA_INPUT_LATENCY_REFRESH_MSEC must be a positive integer"
[[ -t 0 && "$(tty)" == /dev/tty3 ]] ||
    fail "run this interactively from a logged-in local TTY3"
[[ -e /dev/uinput ]] ||
    fail "/dev/uinput is missing (load the uinput module before running)"
[[ -w /dev/uinput ]] ||
    fail "/dev/uinput is not writable by the current user"
[[ -z "$(git -C "$ROOT_DIR" status --porcelain)" ]] ||
    fail "commit or discard the dirty worktree before collecting evidence"
[[ ! -e "$FINAL" && ! -e "$PENDING" ]] ||
    fail "evidence destination already exists: $FINAL"

mkdir -p "$ARCHIVE_ROOT"
mkdir "$PENDING"
chmod 700 "$PENDING"
trap 'preserve_pending $?' EXIT
printf 'source_commit=%s\nrun_id=%s\nsamples=%s\nrefresh_budget_msec=%s\n' \
    "$COMMIT" "$RUN_ID" "$SAMPLES" "$REFRESH_BUDGET_MSEC" \
    >"$PENDING/source.env"
chmod 600 "$PENDING/source.env"

cd "$ROOT_DIR"
tools/probes/uinput_text_injector.py --self-test |
    tee "$PENDING/injector-self-test.log"
cargo build --quiet --release --offline -p sophia-cli \
    --features atomic-scanout-live
tools/atomic_scanout_preflight.sh | tee "$PENDING/preflight.log"

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

    SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$session_log" \
        SOPHIA_LIVE_SESSION_RUNTIME_MSEC=30000 \
        SOPHIA_LIVE_SESSION_SKIP_BUILD=1 \
        SOPHIA_ATOMIC_SCANOUT_SKIP_PREFLIGHT=1 \
        tools/live_session_persistent_hardware_proof.sh \
        "--input-devices=$input_device" \
        --expect-physical-text=sophia \
        --exit-after-input-proof &
    PROOF_PID=$!

    session_deadline=$((SECONDS + 20))
    while ! grep -Fq \
        'sophia_live_session_input schema=1 status=ready source=physical' \
        "$session_log" 2>/dev/null; do
        kill -0 "$PROOF_PID" 2>/dev/null ||
            fail "Sophia exited before sample $sample requested physical input"
        ((SECONDS < session_deadline)) ||
            fail "Sophia did not request physical input for sample $sample"
        sleep 0.01
    done
    : >"$trigger_file"
    chmod 600 "$trigger_file"

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
SOPHIA_INPUT_LATENCY_REFRESH_MSEC="$REFRESH_BUDGET_MSEC" \
    tools/report_sophia_input_latency.sh \
    "$PENDING"/sample-*/session.log | tee "$PENDING/report.log"
report_status="${PIPESTATUS[0]}"
set -e

mv "$PENDING" "$FINAL"
trap - EXIT
echo "Evidence retained in $FINAL"
exit "$report_status"
