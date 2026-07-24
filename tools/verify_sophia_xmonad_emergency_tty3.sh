#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
LOG_DIR="${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"
GUARD_LOG="${2:-$LOG_DIR/input-guard.log}"
RECOVERY_LOG="${3:-$LOG_DIR/recovery.log}"

fail() {
    echo "xmonad emergency verification failed: $*" >&2
    exit 1
}
require_file() {
    [[ -s "$1" ]] || fail "missing or empty evidence file: $1"
}
require_line() {
    local pattern="$1" file="$2" description="$3"
    grep -Eq "$pattern" "$file" || fail "$description ($file)"
}
field() {
    local line="$1" key="$2" token
    for token in $line; do
        if [[ "$token" == "$key="* ]]; then
            printf '%s\n' "${token#*=}"
            return 0
        fi
    done
    return 1
}
require_eq() {
    local line="$1" key="$2" expected="$3" actual
    actual="$(field "$line" "$key")" || fail "record is missing $key"
    [[ "$actual" == "$expected" ]] ||
        fail "$key is $actual, expected $expected"
}

require_file "$SESSION_LOG"
require_file "$GUARD_LOG"
require_file "$RECOVERY_LOG"

require_line '^sophia_live_session_input_pipeline schema=1 status=emergency_exit$' \
    "$SESSION_LOG" "the live owner did not observe the emergency chord"
require_line '^sophia_live_session_health schema=1 status=clean .*pending_input=0 .*wm_degraded=false$' \
    "$SESSION_LOG" "the live owner did not finish with clean session state"

mapfile -t completions < <(
    grep -E '^sophia_live_session schema=14 status=bounded_complete ' "$SESSION_LOG"
)
(( ${#completions[@]} == 1 )) ||
    fail "expected one bounded completion, found ${#completions[@]}"
completion="${completions[0]}"
for assignment in \
    native_submit_failures=0 \
    native_retire_failures=0 \
    native_in_flight=false \
    native_cleanup_pending=false \
    present_live_sources=0 \
    present_live_fences=0 \
    present_live_transactions=0; do
    require_eq "$completion" "${assignment%%=*}" "${assignment#*=}"
done
expected="$(field "$completion" input_events_expected)" ||
    fail "completion is missing input_events_expected"
flushed="$(field "$completion" input_events_flushed)" ||
    fail "completion is missing input_events_flushed"
[[ "$expected" == "$flushed" ]] ||
    fail "input did not drain before shutdown: expected=$expected flushed=$flushed"

require_line '^sophia_session_input_guard schema=1 status=armed$' \
    "$GUARD_LOG" "the independent input guard was not armed"
require_line '^sophia_session_input_guard schema=1 status=triggered$' \
    "$GUARD_LOG" "the independent input guard did not trigger"

recovery="$(
    grep -E '^sophia_tty_recovery schema=3 profile=xmonad ' "$RECOVERY_LOG" |
        tail -n 1
)"
[[ -n "$recovery" ]] || fail "schema-3 xmonad recovery record is missing"
require_eq "$recovery" termios_restored true
require_eq "$recovery" emergency true
require_eq "$recovery" session_shutdown graceful
require_eq "$recovery" session_exit_status 0
kd_before="$(field "$recovery" kd_mode_before)" || fail "recovery is missing kd_mode_before"
kd_after="$(field "$recovery" kd_mode_after)" || fail "recovery is missing kd_mode_after"
[[ "$kd_before" == "$kd_after" ]] ||
    fail "KD mode was not restored: before=$kd_before after=$kd_after"

echo "xmonad TTY3 graceful emergency recovery verified: $SESSION_LOG"
