#!/usr/bin/env bash
set -euo pipefail

# Commit-pinned physical terminal CPU-path gate. Run from local TTY3. The
# benchmark still owns display-manager takeover, input recovery, and its
# independent watchdog; this wrapper adds persistent-kernel-log preflight,
# immutable evidence archiving, and fail-closed post-run verification.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
KERNEL_LOG="${SOPHIA_KERNEL_LOG:-/var/log/socklog/kernel/current}"
MAX_PAGE_FLIP_STALL_RETRIES="${SOPHIA_TERMINAL_MAX_STALL_RETRIES:-8}"
COMMIT="$(git -C "$ROOT_DIR" rev-parse HEAD)"
ARCHIVE_ROOT="$STATE_HOME/sophia/rendering-benchmarks/$COMMIT/terminal-cpu"
RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)"
PENDING="$ARCHIVE_ROOT/.${RUN_ID}.pending"
FINAL="$ARCHIVE_ROOT/$RUN_ID"
SESSION_DIR="$STATE_HOME/sophia/standalone-session"

fail() {
    echo "Sophia terminal gate failed: $*" >&2
    exit 1
}

# Retry only the attributed host fault: a schema-2 hard stall whose event
# poller is empty, routed, and clean. A queued or rejected callback is a Sophia
# defect and must fail the gate on its first occurrence.
is_retryable_host_page_flip_stall() {
    local session_log="$1"

    grep -Eq 'sophia_live_native_page_flip_stall schema=2 status=hard_stall .*poller_pending=0 poller_routes=[1-9][0-9]* poller_last_read=WouldBlock poller_last_decoded=0 poller_last_rejected=0 .*action=terminate_session' "$session_log" 2>/dev/null || return 1
    grep -Eq '^Error: .*hard-stall boundary' "$session_log" || return 1
}

should_retry_host_page_flip_stall() {
    local benchmark_status="$1" retries="$2" retry_limit="$3" session_log="$4"

    ((benchmark_status != 0 && retries < retry_limit)) || return 1
    is_retryable_host_page_flip_stall "$session_log"
}

# A recovery guard belongs to one session process. Wait until the operator is
# back at the originating TTY before starting a retry whose guard must be armed
# again.
await_retry_operator() {
    local attempt="$1" retry="$2" retry_limit="$3" evidence="$4" retry_log="$5"

    printf 'terminal-gate-retry schema=1 status=awaiting_operator attempt=%s retry=%s/%s reason=below_process_page_flip_stall evidence=%s\n' \
        "$attempt" "$retry" "$retry_limit" "$evidence" |
        tee -a "$retry_log"
    echo
    echo "Attempt $attempt hit the attributed host page-flip stall and was retained."
    echo "The next attempt creates a fresh Ctrl-Alt-Backspace recovery guard."
    if ! read -r -p "Press Enter when you are back on TTY3 and ready to arm it: "; then
        echo "Operator input closed before the retry was authorized." >&2
        return 1
    fi
    printf 'terminal-gate-retry schema=1 status=operator_ready attempt=%s retry=%s/%s reason=below_process_page_flip_stall evidence=%s\n' \
        "$attempt" "$retry" "$retry_limit" "$evidence" |
        tee -a "$retry_log"
}

check_stall_retry_classifier() {
    local fixture status
    fixture="$(mktemp -d)"
    status=0

    printf '%s\n' 'sophia_live_native_page_flip_stall schema=2 status=hard_stall output=1 head=1 poller_pending=0 poller_routes=2 poller_last_read=WouldBlock poller_last_decoded=0 poller_last_rejected=0 action=terminate_session' 'Error: "native page flip exceeded the 500 ms hard-stall boundary"' >"$fixture/session.log"
    if ! should_retry_host_page_flip_stall 1 0 8 "$fixture/session.log"; then
        echo "attributed below-process page-flip stall was not retryable" >&2
        status=1
    fi
    if should_retry_host_page_flip_stall 0 0 8 "$fixture/session.log"; then
        echo "successful benchmark was retryable" >&2
        status=1
    fi
    if should_retry_host_page_flip_stall 1 8 8 "$fixture/session.log"; then
        echo "exhausted page-flip retry budget was retryable" >&2
        status=1
    fi

    printf '%s\n' 'sophia_live_native_page_flip_stall schema=2 status=hard_stall output=1 head=1 poller_pending=1 poller_routes=2 poller_last_read=WouldBlock poller_last_decoded=0 poller_last_rejected=0 action=terminate_session' 'Error: "native page flip exceeded the 500 ms hard-stall boundary"' >"$fixture/session.log"
    if is_retryable_host_page_flip_stall "$fixture/session.log"; then
        echo "Sophia-side pending callback was retryable" >&2
        status=1
    fi

    printf '%s\n' 'sophia_live_native_page_flip_stall schema=2 status=hard_stall output=1 head=1 poller_pending=0 poller_routes=2 poller_last_read=CallbacksDecoded poller_last_decoded=0 poller_last_rejected=1 action=terminate_session' 'Error: "native page flip exceeded the 500 ms hard-stall boundary"' >"$fixture/session.log"
    if is_retryable_host_page_flip_stall "$fixture/session.log"; then
        echo "Sophia-side rejected callback was retryable" >&2
        status=1
    fi

    printf '%s\n' 'sophia_live_native_page_flip_stall schema=2 status=hard_stall output=1 head=1 poller_pending=0 poller_routes=2 poller_last_read=WouldBlock poller_last_decoded=0 poller_last_rejected=0 action=terminate_session' >"$fixture/session.log"
    if is_retryable_host_page_flip_stall "$fixture/session.log"; then
        echo "stall without the matching terminal error was retryable" >&2
        status=1
    fi

    : >"$fixture/retries.log"
    if ! printf '\n' |
        await_retry_operator 1 1 8 attempt-001 "$fixture/retries.log" \
            >"$fixture/retry-prompt.log"; then
        echo "operator-ready retry handoff rejected acknowledgement" >&2
        status=1
    fi
    if ! grep -Fxq \
        'terminal-gate-retry schema=1 status=awaiting_operator attempt=1 retry=1/8 reason=below_process_page_flip_stall evidence=attempt-001' \
        "$fixture/retries.log" ||
        ! grep -Fxq \
            'terminal-gate-retry schema=1 status=operator_ready attempt=1 retry=1/8 reason=below_process_page_flip_stall evidence=attempt-001' \
            "$fixture/retries.log"; then
        echo "operator-ready retry handoff did not retain both states" >&2
        status=1
    fi
    if await_retry_operator 1 1 8 attempt-001 "$fixture/eof.log" \
        </dev/null >/dev/null 2>&1; then
        echo "operator-ready retry handoff accepted closed input" >&2
        status=1
    fi

    rm -rf -- "$fixture"
    ((status == 0)) || return "$status"
    echo "terminal gate page-flip stall retry classifier passed"
}

copy_if_present() {
    local source="$1" destination="$2"
    if [[ -s "$source" ]]; then
        install -m 600 "$source" "$destination"
    fi
}

capture_session_artifacts() {
    local destination="${1:-$PENDING}"
    copy_if_present "$SESSION_DIR/session.log" "$destination/session.log"
    copy_if_present "$SESSION_DIR/input-guard.log" "$destination/input-guard.log"
    copy_if_present "$SESSION_DIR/recovery.log" "$destination/recovery.log"
    copy_if_present "$SESSION_DIR/lifecycle.log" "$destination/lifecycle.log"
    copy_if_present /tmp/sophia-standalone-tty3-launch.log "$destination/launch.log"
}

run_finalized=false
preserve_pending_on_exit() {
    local original_status="$1"
    if [[ "$run_finalized" != true && -d "$PENDING" ]]; then
        capture_session_artifacts
        if [[ ! -s "$PENDING/result.kdl" ]]; then
            printf 'terminal-gate-result schema=1 status="interrupted" commit="%s" runner-status=%s failures="runner_interrupted"\n' \
                "$COMMIT" "$original_status" >"$PENDING/result.kdl"
            chmod 600 "$PENDING/result.kdl"
        fi
        echo "Incomplete evidence retained in $PENDING" >&2
    fi
}

if [[ "${1:-}" == --help ]]; then
    cat <<EOF
Usage: tools/run_sophia_terminal_gate_tty3.sh

Run from a logged-in local TTY3. Requires a clean worktree, sudo access,
running socklog-unix and nanoklogd services, and a nonempty kernel log.
Evidence is retained under:
  $STATE_HOME/sophia/rendering-benchmarks/<commit>/terminal-cpu/<UTC timestamp>/

Up to $MAX_PAGE_FLIP_STALL_RETRIES attributed below-process page-flip stalls
are retained and retried. Override with SOPHIA_TERMINAL_MAX_STALL_RETRIES.
Each retry pauses for Enter before its fresh Ctrl-Alt-Backspace recovery guard
must be armed.
EOF
    exit 0
fi
if [[ "${1:-}" == --self-test ]]; then
    check_stall_retry_classifier
    exit
fi
[[ $# -eq 0 ]] || fail "unexpected arguments (use --help)"
[[ "$MAX_PAGE_FLIP_STALL_RETRIES" =~ ^[0-9]+$ ]] ||
    fail "SOPHIA_TERMINAL_MAX_STALL_RETRIES must be a nonnegative integer"
((MAX_PAGE_FLIP_STALL_RETRIES <= 32)) ||
    fail "SOPHIA_TERMINAL_MAX_STALL_RETRIES must not exceed 32"
[[ -t 0 && "$(tty)" == /dev/tty3 ]] ||
    fail "run this interactively from a logged-in local TTY3"
[[ -z "$(git -C "$ROOT_DIR" status --porcelain)" ]] ||
    fail "commit or discard the dirty worktree before collecting evidence"
[[ ! -e "$FINAL" && ! -e "$PENDING" ]] ||
    fail "evidence destination already exists: $FINAL"

mkdir -p "$ARCHIVE_ROOT"
chmod 700 "$STATE_HOME/sophia/rendering-benchmarks" \
    "$STATE_HOME/sophia/rendering-benchmarks/$COMMIT" \
    "$ARCHIVE_ROOT"
mkdir "$PENDING"
chmod 700 "$PENDING"
trap 'preserve_pending_on_exit $?' EXIT
printf 'source_commit=%s\nrun_id=%s\nkernel_log=%s\nmax_page_flip_stall_retries=%s\n' "$COMMIT" "$RUN_ID" "$KERNEL_LOG" "$MAX_PAGE_FLIP_STALL_RETRIES" >"$PENDING/source.env"
chmod 600 "$PENDING/source.env"

echo "Evidence will be retained in $FINAL"
echo "Checking persistent logging before graphics takeover..."
sudo -v
sudo sv status socklog-unix nanoklogd | tee "$PENDING/logging-services.log"
[[ "$(grep -c '^run:' "$PENDING/logging-services.log" || true)" -eq 2 ]] ||
    fail "socklog-unix and nanoklogd must both report run status"
sudo test -s "$KERNEL_LOG" || fail "kernel log is missing or empty: $KERNEL_LOG"

kernel_inode_before="$(sudo stat -c %i "$KERNEL_LOG")"
kernel_bytes_before="$(sudo stat -c %s "$KERNEL_LOG")"
printf 'inode=%s bytes=%s\n' "$kernel_inode_before" "$kernel_bytes_before" \
    >"$PENDING/kernel-before.env"
sudo tail -n 300 "$KERNEL_LOG" >"$PENDING/kernel-before.log"
chmod 600 "$PENDING"/kernel-before.*

{
    "$ROOT_DIR/tools/check_bounded_xterm_geometry.sh"
    "$ROOT_DIR/tools/check_sophia_terminal_performance_reporter.sh"
} | tee "$PENDING/preflight.log"

echo
echo "The benchmark will ask you to press and release Ctrl-Alt-Backspace."
echo "Confirm that the centered xterm scrolls continuously, then let it exit."
attempt=0
stall_retries=0
benchmark_status=1
attempt_dir=
while true; do
    attempt=$((attempt + 1))
    attempt_dir="$PENDING/attempt-$(printf '%03d' "$attempt")"
    mkdir "$attempt_dir"
    chmod 700 "$attempt_dir"
    echo
    echo "Starting terminal benchmark attempt $attempt..."
    set +e
    "$ROOT_DIR/tools/benchmark_sophia_terminal_tty3.sh" 2>&1 |
        tee "$attempt_dir/operator.log"
    benchmark_status="${PIPESTATUS[0]}"
    set -e
    capture_session_artifacts "$attempt_dir"

    if should_retry_host_page_flip_stall \
        "$benchmark_status" \
        "$stall_retries" \
        "$MAX_PAGE_FLIP_STALL_RETRIES" \
        "$attempt_dir/session.log"; then
        next_retry=$((stall_retries + 1))
        await_retry_operator \
            "$attempt" \
            "$next_retry" \
            "$MAX_PAGE_FLIP_STALL_RETRIES" \
            "$(basename "$attempt_dir")" \
            "$PENDING/stall-retries.log" ||
            fail "operator did not authorize terminal benchmark retry $next_retry"
        stall_retries="$next_retry"
        printf 'terminal-gate-retry schema=1 status=retrying attempt=%s retry=%s/%s reason=below_process_page_flip_stall evidence=%s\n' "$attempt" "$stall_retries" "$MAX_PAGE_FLIP_STALL_RETRIES" "$(basename "$attempt_dir")" | tee -a "$PENDING/stall-retries.log"
        continue
    fi
    break
done

for artifact in session.log input-guard.log recovery.log lifecycle.log launch.log operator.log; do
    copy_if_present "$attempt_dir/$artifact" "$PENDING/$artifact"
done

visual_confirmed=false
if ((benchmark_status == 0)); then
    read -r -p "Did the centered xterm scroll continuously? [yes/no] " answer
    case "$answer" in
        y|Y|yes|YES|Yes) visual_confirmed=true ;;
    esac
fi

report_status=1
if [[ -s "$PENDING/session.log" ]]; then
    set +e
    "$ROOT_DIR/tools/report_sophia_terminal_performance.sh" \
        "$PENDING/session.log" >"$PENDING/performance.log" 2>&1
    report_status=$?
    set -e
fi

kernel_inode_after="$(sudo stat -c %i "$KERNEL_LOG")"
kernel_bytes_after="$(sudo stat -c %s "$KERNEL_LOG")"
printf 'inode=%s bytes=%s\n' "$kernel_inode_after" "$kernel_bytes_after" \
    >"$PENDING/kernel-after.env"
sudo tail -n 1000 "$KERNEL_LOG" >"$PENDING/kernel-after.log"
kernel_delta_complete=false
if [[ "$kernel_inode_after" == "$kernel_inode_before" ]] \
    && ((kernel_bytes_after >= kernel_bytes_before)); then
    sudo tail -c "+$((kernel_bytes_before + 1))" "$KERNEL_LOG" \
        >"$PENDING/kernel-delta.log"
    kernel_delta_complete=true
else
    : >"$PENDING/kernel-delta.log"
fi
chmod 600 "$PENDING"/kernel-after.* "$PENDING/kernel-delta.log"

failures=()
((benchmark_status == 0)) || failures+=(benchmark)
if ((benchmark_status != 0)) &&
    is_retryable_host_page_flip_stall "$PENDING/session.log"; then
    failures+=(page_flip_stall_retry_budget)
fi
((report_status == 0)) || failures+=(performance_report)
[[ "$visual_confirmed" == true ]] || failures+=(visual_confirmation)
[[ "$kernel_delta_complete" == true ]] || failures+=(kernel_log_rotated)

for artifact in session.log input-guard.log recovery.log lifecycle.log launch.log; do
    [[ -s "$PENDING/$artifact" ]] || failures+=("missing_${artifact//[.-]/_}")
done
grep -Eq '^sophia_session_input_guard schema=1 status=armed$' \
    "$PENDING/input-guard.log" 2>/dev/null || failures+=(input_guard)
tail -n 1 "$PENDING/recovery.log" 2>/dev/null |
    grep -Eq '^sophia_tty_recovery schema=3 profile=standalone .* termios_restored=true emergency=false ' ||
    failures+=(tty_recovery)
grep -Eq '^sophia_session_lifecycle schema=1 status=returned phase=handoff .* exit_status=0 emergency=false handoff=display_manager$' \
    "$PENDING/lifecycle.log" 2>/dev/null || failures+=(session_handoff)
grep -Eq '^sophia_tty_activation schema=1 requested=3 active=3 display_manager=(greetd|lightdm)$' \
    "$PENDING/launch.log" 2>/dev/null || failures+=(display_manager_restore)

if grep -Eqi \
    'amdgpu.*(ring.*(timeout|hung)|GPU reset|context is guilty|CS has been rejected|failed.*reset)' \
    "$PENDING/kernel-delta.log"; then
    failures+=(amdgpu_kernel_error)
fi

status=passed
exit_status=0
if ((${#failures[@]} > 0)); then
    status=failed
    exit_status=1
fi
failure_list=none
if ((${#failures[@]} > 0)); then
    failure_list="$(IFS=,; printf '%s' "${failures[*]}")"
fi
printf 'terminal-gate-result schema=1 status="%s" commit="%s" benchmark-status=%s report-status=%s visual-confirmed=%s kernel-delta-complete=%s attempts=%s stall-retries=%s stall-retry-limit=%s failures="%s"\n' "$status" "$COMMIT" "$benchmark_status" "$report_status" "$visual_confirmed" "$kernel_delta_complete" "$attempt" "$stall_retries" "$MAX_PAGE_FLIP_STALL_RETRIES" "$failure_list" >"$PENDING/result.kdl"
chmod 600 "$PENDING/result.kdl"

mv "$PENDING" "$FINAL"
run_finalized=true
echo
cat "$FINAL/result.kdl"
[[ ! -s "$FINAL/performance.log" ]] || cat "$FINAL/performance.log"
echo "Evidence retained in $FINAL"
exit "$exit_status"
