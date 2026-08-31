#!/usr/bin/env bash
set -euo pipefail

# Commit-pinned physical terminal CPU-path gate. Run from local TTY3. The
# benchmark owns display-manager takeover, input recovery, and its independent
# watchdog; this wrapper adds logging preflight, immutable evidence archiving,
# one machine verdict, and one independent operator visual verdict.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
KERNEL_LOG="${SOPHIA_KERNEL_LOG:-/var/log/socklog/kernel/current}"
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

is_affirmative_visual_answer() {
    case "${1:-}" in
        y|Y|yes|YES|Yes) return 0 ;;
        *) return 1 ;;
    esac
}

check_single_attempt_contract() {
    local answer
    for answer in y Y yes YES Yes; do
        is_affirmative_visual_answer "$answer" || {
            echo "affirmative visual answer was rejected: $answer" >&2
            return 1
        }
    done
    for answer in n N no NO No ""; do
        if is_affirmative_visual_answer "$answer"; then
            echo "non-affirmative visual answer was accepted: $answer" >&2
            return 1
        fi
    done
    echo "terminal gate single-attempt contract passed"
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
            printf 'terminal-gate-result schema=2 status="interrupted" machine-status="interrupted" visual-status="unknown" commit="%s" runner-status=%s failures="runner_interrupted"\n' \
                "$COMMIT" "$original_status" >"$PENDING/result.kdl"
            chmod 600 "$PENDING/result.kdl"
        fi
        echo "Incomplete evidence retained in $PENDING" >&2
    fi
}

if [[ "${1:-}" == --help ]]; then
    cat <<EOF
Usage: tools/run_sophia_terminal_gate_tty3.sh

Run one attempt from a logged-in local TTY3. Requires a clean worktree, sudo
access, running socklog-unix and nanoklogd services, and a nonempty kernel log.
The operator visual question is asked after the attempt even when the machine
benchmark fails, so the two verdicts remain independent. Evidence is retained
under:
  $STATE_HOME/sophia/rendering-benchmarks/<commit>/terminal-cpu/<UTC timestamp>/
EOF
    exit 0
fi
if [[ "${1:-}" == --self-test ]]; then
    check_single_attempt_contract
    exit
fi
[[ $# -eq 0 ]] || fail "unexpected arguments (use --help)"
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
printf 'source_commit=%s\nrun_id=%s\nkernel_log=%s\nattempts=1\nretry_policy=disabled\n' \
    "$COMMIT" "$RUN_ID" "$KERNEL_LOG" >"$PENDING/source.env"
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
attempt_dir="$PENDING/attempt-001"
mkdir "$attempt_dir"
chmod 700 "$attempt_dir"
echo
echo "Starting the terminal benchmark..."
set +e
"$ROOT_DIR/tools/benchmark_sophia_terminal_tty3.sh" 2>&1 |
    tee "$attempt_dir/operator.log"
benchmark_status="${PIPESTATUS[0]}"
set -e
capture_session_artifacts "$attempt_dir"

for artifact in session.log input-guard.log recovery.log lifecycle.log launch.log operator.log; do
    copy_if_present "$attempt_dir/$artifact" "$PENDING/$artifact"
done

visual_confirmed=false
visual_prompt_status=answered
answer=
if ! read -r -p "Did the centered xterm scroll continuously? [yes/no] " answer; then
    visual_prompt_status=input_closed
elif is_affirmative_visual_answer "$answer"; then
    visual_confirmed=true
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

machine_failures=()
((benchmark_status == 0)) || machine_failures+=(benchmark)
((report_status == 0)) || machine_failures+=(performance_report)
[[ "$kernel_delta_complete" == true ]] || machine_failures+=(kernel_log_rotated)

for artifact in session.log input-guard.log recovery.log lifecycle.log launch.log; do
    [[ -s "$PENDING/$artifact" ]] ||
        machine_failures+=("missing_${artifact//[.-]/_}")
done
grep -Eq '^sophia_session_input_guard schema=1 status=armed$' \
    "$PENDING/input-guard.log" 2>/dev/null || machine_failures+=(input_guard)
tail -n 1 "$PENDING/recovery.log" 2>/dev/null |
    grep -Eq '^sophia_tty_recovery schema=3 profile=standalone .* termios_restored=true emergency=false ' ||
    machine_failures+=(tty_recovery)
grep -Eq '^sophia_session_lifecycle schema=1 status=returned phase=handoff .* exit_status=0 emergency=false handoff=display_manager$' \
    "$PENDING/lifecycle.log" 2>/dev/null || machine_failures+=(session_handoff)
grep -Eq '^sophia_tty_activation schema=1 requested=3 active=3 display_manager=(greetd|lightdm)$' \
    "$PENDING/launch.log" 2>/dev/null || machine_failures+=(display_manager_restore)

if grep -Eqi \
    'amdgpu.*(ring.*(timeout|hung)|GPU reset|context is guilty|CS has been rejected|failed.*reset)' \
    "$PENDING/kernel-delta.log"; then
    machine_failures+=(amdgpu_kernel_error)
fi

visual_failures=()
[[ "$visual_prompt_status" == answered ]] || visual_failures+=(visual_input_closed)
[[ "$visual_confirmed" == true ]] || visual_failures+=(visual_confirmation)

machine_status=passed
((${#machine_failures[@]} == 0)) || machine_status=failed
visual_status=passed
((${#visual_failures[@]} == 0)) || visual_status=failed
status=passed
exit_status=0
if [[ "$machine_status" != passed || "$visual_status" != passed ]]; then
    status=failed
    exit_status=1
fi

machine_failure_list=none
if ((${#machine_failures[@]} > 0)); then
    machine_failure_list="$(IFS=,; printf '%s' "${machine_failures[*]}")"
fi
visual_failure_list=none
if ((${#visual_failures[@]} > 0)); then
    visual_failure_list="$(IFS=,; printf '%s' "${visual_failures[*]}")"
fi
failures=none
if [[ "$machine_failure_list" != none && "$visual_failure_list" != none ]]; then
    failures="$machine_failure_list,$visual_failure_list"
elif [[ "$machine_failure_list" != none ]]; then
    failures="$machine_failure_list"
elif [[ "$visual_failure_list" != none ]]; then
    failures="$visual_failure_list"
fi

printf 'terminal-gate-result schema=2 status="%s" machine-status="%s" visual-status="%s" commit="%s" benchmark-status=%s report-status=%s visual-confirmed=%s visual-prompt="%s" kernel-delta-complete=%s attempts=1 retry-policy="disabled" machine-failures="%s" visual-failures="%s" failures="%s"\n' \
    "$status" "$machine_status" "$visual_status" "$COMMIT" "$benchmark_status" \
    "$report_status" "$visual_confirmed" "$visual_prompt_status" \
    "$kernel_delta_complete" "$machine_failure_list" "$visual_failure_list" \
    "$failures" >"$PENDING/result.kdl"
chmod 600 "$PENDING/result.kdl"

mv "$PENDING" "$FINAL"
run_finalized=true
echo
cat "$FINAL/result.kdl"
[[ ! -s "$FINAL/performance.log" ]] || cat "$FINAL/performance.log"
echo "Evidence retained in $FINAL"
exit "$exit_status"
