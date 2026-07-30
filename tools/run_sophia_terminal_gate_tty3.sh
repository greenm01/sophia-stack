#!/usr/bin/env bash
set -euo pipefail

# Commit-pinned physical terminal CPU-path gate. Run from local TTY3. The
# benchmark still owns display-manager takeover, input recovery, and its
# independent watchdog; this wrapper adds persistent-kernel-log preflight,
# immutable evidence archiving, and fail-closed post-run verification.

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

copy_if_present() {
    local source="$1" destination="$2"
    if [[ -s "$source" ]]; then
        install -m 600 "$source" "$destination"
    fi
}

capture_session_artifacts() {
    copy_if_present "$SESSION_DIR/session.log" "$PENDING/session.log"
    copy_if_present "$SESSION_DIR/input-guard.log" "$PENDING/input-guard.log"
    copy_if_present "$SESSION_DIR/recovery.log" "$PENDING/recovery.log"
    copy_if_present "$SESSION_DIR/lifecycle.log" "$PENDING/lifecycle.log"
    copy_if_present /tmp/sophia-standalone-tty3-launch.log "$PENDING/launch.log"
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
EOF
    exit 0
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
printf 'source_commit=%s\nrun_id=%s\nkernel_log=%s\n' \
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
set +e
"$ROOT_DIR/tools/benchmark_sophia_terminal_tty3.sh" 2>&1 |
    tee "$PENDING/operator.log"
benchmark_status="${PIPESTATUS[0]}"
set -e

visual_confirmed=false
if ((benchmark_status == 0)); then
    read -r -p "Did the centered xterm scroll continuously? [yes/no] " answer
    case "$answer" in
        y|Y|yes|YES|Yes) visual_confirmed=true ;;
    esac
fi

capture_session_artifacts

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
printf 'terminal-gate-result schema=1 status="%s" commit="%s" benchmark-status=%s report-status=%s visual-confirmed=%s kernel-delta-complete=%s failures="%s"\n' \
    "$status" "$COMMIT" "$benchmark_status" "$report_status" \
    "$visual_confirmed" "$kernel_delta_complete" "$failure_list" \
    >"$PENDING/result.kdl"
chmod 600 "$PENDING/result.kdl"

mv "$PENDING" "$FINAL"
run_finalized=true
echo
cat "$FINAL/result.kdl"
[[ ! -s "$FINAL/performance.log" ]] || cat "$FINAL/performance.log"
echo "Evidence retained in $FINAL"
exit "$exit_status"
