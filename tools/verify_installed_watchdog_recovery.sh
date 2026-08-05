#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
SESSION_DIR="$STATE_HOME/sophia/xmonad-session"
SESSION_LOG="${1:-$SESSION_DIR/session.log}"
GUARD_LOG="${2:-$SESSION_DIR/input-guard.log}"
RECOVERY_LOG="${3:-$SESSION_DIR/recovery.log}"
LIFECYCLE_LOG="${4:-$SESSION_DIR/lifecycle.log}"

fail() {
    echo "installed watchdog recovery verification failed: $*" >&2
    exit 1
}
require_file() {
    [[ -s "$1" ]] || fail "missing or empty evidence file: $1"
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

for evidence in "$SESSION_LOG" "$GUARD_LOG" "$RECOVERY_LOG" "$LIFECYCLE_LOG"; do
    require_file "$evidence"
done

grep -Eq '^sophia_live_session_startup schema=2 status=ready .*outputs_ready=[1-9][0-9]*/[1-9][0-9]*' \
    "$SESSION_LOG" || fail "the installed session did not reach visible startup readiness"
mapfile -t watchdog_records < <(
    grep -E '^sophia_session_watchdog schema=1 result=deadline_exceeded ' "$SESSION_LOG"
)
(( ${#watchdog_records[@]} == 1 )) ||
    fail "expected one deadline record, found ${#watchdog_records[@]}"
watchdog="${watchdog_records[0]}"
require_eq "$watchdog" deadline_seconds 45
require_eq "$watchdog" action terminate_process_group
session_pid="$(field "$watchdog" session_pid)" || fail "deadline record is missing session_pid"
[[ "$session_pid" =~ ^[1-9][0-9]*$ ]] || fail "invalid session_pid: $session_pid"

grep -Fxq 'sophia_session_input_guard schema=1 status=armed' "$GUARD_LOG" ||
    fail "the independent input guard did not arm"
if grep -Fq 'status=triggered' "$GUARD_LOG"; then
    fail "the local emergency chord, not the watchdog, ended the session"
fi

recovery="$(
    grep -E '^sophia_tty_recovery schema=3 profile=xmonad ' "$RECOVERY_LOG" |
        tail -n 1
)"
[[ -n "$recovery" ]] || fail "schema-3 xmonad recovery record is missing"
require_eq "$recovery" termios_restored true
require_eq "$recovery" emergency true
require_eq "$recovery" session_shutdown watchdog_term
require_eq "$recovery" session_exit_status none
kd_before="$(field "$recovery" kd_mode_before)" || fail "recovery is missing kd_mode_before"
kd_after="$(field "$recovery" kd_mode_after)" || fail "recovery is missing kd_mode_after"
[[ "$kd_before" == "$kd_after" ]] ||
    fail "KD mode was not restored: before=$kd_before after=$kd_after"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VERIFY_LIFECYCLE="${SOPHIA_VERIFY_LIFECYCLE_BIN:-$SCRIPT_DIR/sophia-verify-lifecycle}"
if [[ ! -x "$VERIFY_LIFECYCLE" && -x "$SCRIPT_DIR/verify_installed_session_lifecycle.sh" ]]; then
    VERIFY_LIFECYCLE="$SCRIPT_DIR/verify_installed_session_lifecycle.sh"
fi
[[ -x "$VERIFY_LIFECYCLE" ]] || fail "installed lifecycle verifier is unavailable"
"$VERIFY_LIFECYCLE" "$LIFECYCLE_LOG" watchdog

echo "installed Sophia watchdog recovery verified: $SESSION_LOG"
