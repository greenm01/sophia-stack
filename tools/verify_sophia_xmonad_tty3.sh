#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
LOG_DIR="${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"
GUARD_LOG="${2:-$LOG_DIR/input-guard.log}"
RECOVERY_LOG="${3:-$LOG_DIR/recovery.log}"

fail() {
    echo "xmonad TTY3 verification failed: $*" >&2
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
    actual="$(field "$line" "$key")" || fail "completion is missing $key"
    [[ "$actual" == "$expected" ]] || fail "$key is $actual, expected $expected"
}
require_positive() {
    local line="$1" key="$2" actual
    actual="$(field "$line" "$key")" || fail "completion is missing $key"
    [[ "$actual" =~ ^[0-9]+$ ]] || fail "$key is not an integer: $actual"
    (( actual > 0 )) || fail "$key did not record activity"
}
require_count_at_least() {
    local pattern="$1" file="$2" minimum="$3" description="$4" count
    count="$(grep -Ec "$pattern" "$file" || true)"
    (( count >= minimum )) ||
        fail "$description: observed $count, required $minimum ($file)"
}
require_value_at_least() {
    local line="$1" key="$2" minimum="$3" actual
    actual="$(field "$line" "$key")" || fail "record is missing $key"
    [[ "$actual" =~ ^[0-9]+$ ]] || fail "$key is not an integer: $actual"
    (( actual >= minimum )) || fail "$key is $actual, expected at least $minimum"
}

require_file "$SESSION_LOG"
require_file "$GUARD_LOG"
require_file "$RECOVERY_LOG"

if grep -Eqi '(^|[[:space:]])(panic|error:|status=(failed|degraded))' "$SESSION_LOG"; then
    fail "session log contains an error, panic, or degraded status"
fi

require_line '^sophia_live_wm schema=1 status=ready adapter=external socket=private restarts=0$' \
    "$SESSION_LOG" "external xmonad policy never became ready"
require_line '^sophia_session_app schema=1 status=started id=terminal source=startup$' \
    "$SESSION_LOG" "startup Kitty was not launched"
require_line '^sophia_session_app schema=1 status=started id=terminal source=action$' \
    "$SESSION_LOG" "Super-Enter did not launch a second Kitty"
require_count_at_least \
    '^sophia_session_app schema=1 status=started id=terminal source=(startup|action)$' \
    "$SESSION_LOG" 2 "two independent Kitty launches were not recorded"
require_line '^sophia_live_wm schema=1 status=layout_committed .* outcome=Committed$' \
    "$SESSION_LOG" "xmonad did not commit a layout"
require_line '^sophia_live_wm schema=1 status=focus_committed .* target=surface$' \
    "$SESSION_LOG" "xmonad did not commit focus"
require_line '^sophia_live_wm schema=1 status=session_action_committed .* action=Logout$' \
    "$SESSION_LOG" "normal Super-Shift-Q logout was not committed"
for action in \
    1 \
    3 \
    257 \
    258 \
    769; do
    require_line \
        "^sophia_live_wm schema=1 status=physical_action_committed action=${action}$" \
        "$SESSION_LOG" "required physical WM action $action was not committed"
done
require_line \
    '^sophia_live_outputs schema=2 status=ready discovered=2 presentation=2 native_owned=2 multi_output_scanout=enabled ' \
    "$SESSION_LOG" "two-output native ownership was not established"
for output in 1 2; do
    require_line \
        "sophia_live_native_page_flip schema=1 status=retired output=${output} " \
        "$SESSION_LOG" "physical output $output did not retire a page flip"
done
cursor="$(
    grep -E '^sophia_live_session_cursor schema=2 ' "$SESSION_LOG" | tail -n 1
)"
[[ -n "$cursor" ]] || fail "final cursor health record is missing"
require_value_at_least "$cursor" buttons_routed 2
require_line '^sophia_live_session_health schema=1 status=clean .* wm_degraded=false$' \
    "$SESSION_LOG" "final session health was not clean"

mapfile -t completions < <(
    grep -E '^sophia_live_session schema=14 status=bounded_complete ' "$SESSION_LOG"
)
(( ${#completions[@]} == 1 )) ||
    fail "expected one schema-14 completion, found ${#completions[@]}"
completion="${completions[0]}"

startup_ready_msec="$(field "$completion" startup_ready_msec)" ||
    fail "completion is missing startup_ready_msec"
[[ "$startup_ready_msec" =~ ^[0-9]+$ ]] ||
    fail "startup_ready_msec is not an integer: $startup_ready_msec"
(( startup_ready_msec <= 8000 )) ||
    fail "startup readiness took ${startup_ready_msec}ms (limit: 8000ms)"

for key in physical_events physical_keys_routed physical_pointer_events \
    physical_pointer_routed wm_requests wm_committed native_submissions \
    native_retirements native_frame_uploads; do
    require_positive "$completion" "$key"
done
for assignment in \
    native_presentation=enabled \
    physical_input=enabled \
    wm_policy=external \
    wm_restarts=0 \
    wm_degraded=false \
    native_submit_failures=0 \
    native_retire_failures=0 \
    native_callback_rejected=0 \
    native_callback_queue_saturated=0 \
    native_in_flight=false \
    native_cleanup_pending=false \
    present_disconnect_failures=0 \
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
    fail "input queue did not drain: expected=$expected flushed=$flushed"

require_line '^sophia_session_input_guard schema=2 status=ready .* keyboards=[1-9][0-9]*$' \
    "$GUARD_LOG" "input guard did not discover a keyboard"
require_line '^sophia_session_input_guard schema=1 status=armed$' \
    "$GUARD_LOG" "emergency input guard was not armed"
if grep -Eq '^sophia_session_input_guard schema=1 status=triggered$' "$GUARD_LOG"; then
    fail "run used emergency recovery instead of normal logout"
fi

recovery="$(
    grep -E '^sophia_tty_recovery schema=2 profile=xmonad ' "$RECOVERY_LOG" | tail -n 1
)"
[[ -n "$recovery" ]] || fail "no xmonad TTY recovery record"
require_eq "$recovery" termios_restored true
require_eq "$recovery" emergency false
kd_before="$(field "$recovery" kd_mode_before)" || fail "recovery is missing kd_mode_before"
kd_after="$(field "$recovery" kd_mode_after)" || fail "recovery is missing kd_mode_after"
[[ "$kd_before" == "$kd_after" ]] ||
    fail "KD mode was not restored: before=$kd_before after=$kd_after"

echo "xmonad TTY3 physical session verified: $SESSION_LOG"
