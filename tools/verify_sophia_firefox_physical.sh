#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
LOG_DIR="${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"
GUARD_LOG="${2:-$LOG_DIR/input-guard.log}"
RECOVERY_LOG="${3:-$LOG_DIR/recovery.log}"

fail() {
    echo "physical Firefox M10 verification failed: $*" >&2
    exit 1
}
require_file() {
    [[ -s "$1" ]] || fail "missing or empty evidence file: $1"
}
count() {
    grep -Ec "$1" "$SESSION_LOG" || true
}
line_number() {
    local pattern="$1" ordinal="${2:-1}"
    grep -nE "$pattern" "$SESSION_LOG" 2>/dev/null |
        sed -n "${ordinal}p" |
        cut -d: -f1 || true
}
line_number_after() {
    local pattern="$1" minimum="$2"
    grep -nE "$pattern" "$SESSION_LOG" 2>/dev/null |
        cut -d: -f1 |
        awk -v minimum="$minimum" '$1 > minimum { print; exit }' || true
}
require_line_number() {
    local pattern="$1" description="$2" observed
    observed="$(line_number "$pattern")"
    [[ -n "$observed" ]] || fail "$description"
    printf '%s\n' "$observed"
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
    [[ "$actual" == "$expected" ]] || fail "$key is $actual, expected $expected"
}
require_at_least() {
    local line="$1" key="$2" minimum="$3" actual
    actual="$(field "$line" "$key")" || fail "record is missing $key"
    [[ "$actual" =~ ^[0-9]+$ ]] || fail "$key is not an integer: $actual"
    (( actual >= minimum )) || fail "$key is $actual, expected at least $minimum"
}

require_file "$SESSION_LOG"
require_file "$GUARD_LOG"
require_file "$RECOVERY_LOG"

if grep -Eqi '(^Error:|panicked at|^sophia_[^[:space:]]+ .*status=(failed|degraded)([[:space:]]|$))' \
    "$SESSION_LOG"; then
    fail "session log contains a Sophia error, panic, or degraded status"
fi
grep -Eq '^sophia_live_wm schema=1 status=ready adapter=external socket=private restarts=0$' \
    "$SESSION_LOG" || fail "external xmonad policy never became ready"
grep -Eq '^sophia_live_outputs schema=2 status=ready discovered=2 presentation=2 native_owned=2 multi_output_scanout=enabled ' \
    "$SESSION_LOG" || fail "two-output native ownership was not established"
for output in 1 2; do
    grep -Eq "^sophia_live_native_page_flip schema=1 status=retired output=${output} " \
        "$SESSION_LOG" || fail "output $output never retired a page flip"
done

[[ "$(count '^sophia_session_app schema=1 status=started id=terminal source=(startup|action)$')" == 2 ]] ||
    fail "the run must contain exactly two Kitty processes"
[[ "$(count '^sophia_session_app schema=1 status=started id=firefox source=action$')" == 2 ]] ||
    fail "Firefox was not action-launched exactly twice"
[[ "$(count '^sophia_session_app schema=1 status=exited id=firefox source=managed exit_status=exit status: 0$')" == 2 ]] ||
    fail "Firefox did not exit successfully exactly twice"

kitty_before_a="$(require_line_number '^sophia_firefox_m10 schema=1 status=kitty_checkpoint terminal=a checkpoint=before content=redacted$' 'Kitty A pre-Firefox checkpoint is missing')"
kitty_before_b="$(require_line_number '^sophia_firefox_m10 schema=1 status=kitty_checkpoint terminal=b checkpoint=before content=redacted$' 'Kitty B pre-Firefox checkpoint is missing')"
kitty_normal_a="$(require_line_number '^sophia_firefox_m10 schema=1 status=kitty_checkpoint terminal=a checkpoint=after_normal_close content=redacted$' 'Kitty A normal-close checkpoint is missing')"
kitty_normal_b="$(require_line_number '^sophia_firefox_m10 schema=1 status=kitty_checkpoint terminal=b checkpoint=after_normal_close content=redacted$' 'Kitty B normal-close checkpoint is missing')"
kitty_forced_a="$(require_line_number '^sophia_firefox_m10 schema=1 status=kitty_checkpoint terminal=a checkpoint=after_forced_close content=redacted$' 'Kitty A forced-close checkpoint is missing')"
kitty_forced_b="$(require_line_number '^sophia_firefox_m10 schema=1 status=kitty_checkpoint terminal=b checkpoint=after_forced_close content=redacted$' 'Kitty B forced-close checkpoint is missing')"
first_start="$(line_number '^sophia_session_app schema=1 status=started id=firefox source=action$' 1)"
second_start="$(line_number '^sophia_session_app schema=1 status=started id=firefox source=action$' 2)"
first_exit="$(line_number '^sophia_session_app schema=1 status=exited id=firefox source=managed exit_status=exit status: 0$' 1)"
second_exit="$(line_number '^sophia_session_app schema=1 status=exited id=firefox source=managed exit_status=exit status: 0$' 2)"
[[ -n "$first_start" && -n "$first_exit" && -n "$second_start" && -n "$second_exit" ]] ||
    fail "Firefox launch/exit ordering evidence is incomplete"
(( kitty_before_a < first_start && kitty_before_b < first_start )) ||
    fail "both Kitty windows were not interactive before Firefox"
(( first_start < first_exit
    && first_exit < kitty_normal_a && first_exit < kitty_normal_b
    && kitty_normal_a < second_start && kitty_normal_b < second_start
    && second_start < second_exit
    && second_exit < kitty_forced_a && second_exit < kitty_forced_b )) ||
    fail "Firefox close/restart and Kitty retention checkpoints are out of order"

forced_close="$(line_number_after '^sophia_live_wm schema=1 status=session_action_committed .* action=CloseFocused$' "$second_start")"
[[ -n "$forced_close" ]] && (( forced_close < second_exit )) ||
    fail "second Firefox was not closed through the WM close path"
if awk -v first="$first_start" -v last="$first_exit" \
    'NR > first && NR < last && /status=session_action_committed .* action=CloseFocused$/ { found=1 } END { exit !found }' \
    "$SESSION_LOG"; then
    fail "first Firefox used the WM close path instead of Ctrl+Q"
fi

grep -Eq '^sophia_firefox_m8 schema=1 status=page_ready .* content=redacted$' \
    "$SESSION_LOG" || fail "offline Firefox page never became ready"
for stage in loaded keyboard clipboard primary scroll resize refocus dialog; do
    [[ "$(count "^sophia_firefox_m8 schema=1 status=stage_complete stage=$stage ")" == 1 ]] ||
        fail "Firefox stage did not complete exactly once: $stage"
done
primary_line="$(line_number '^sophia_firefox_m8 schema=1 status=stage_complete stage=primary ')"
scroll_line="$(line_number '^sophia_firefox_m8 schema=1 status=stage_complete stage=scroll ')"
axis_line="$(line_number_after '^sophia_live_session_pointer schema=3 status=axis_routed$' "$primary_line")"
[[ -n "$axis_line" ]] && (( axis_line < scroll_line )) ||
    fail "a routed physical wheel axis did not advance Firefox's DOM scroll stage"
resize_line="$(line_number '^sophia_firefox_m8 schema=1 status=stage_complete stage=resize ')"
resize_action="$(line_number_after '^sophia_live_wm schema=1 status=physical_action_committed action=1$' "$scroll_line")"
[[ -n "$resize_action" ]] && (( resize_action < resize_line )) ||
    fail "the Firefox resize stage lacks an ordered physical layout action"
grep -Eq '^sophia_firefox_m8 schema=1 status=complete stages=8 selection_owner_changes=[2-9][0-9]* selection_conversions=[2-9][0-9]* content=redacted$' \
    "$SESSION_LOG" || fail "Firefox eight-stage proof did not complete"
grep -Eq '^sophia_firefox_m10 schema=1 status=complete kitty_checkpoints=6 content=redacted$' \
    "$SESSION_LOG" || fail "Firefox M10 Kitty proof did not complete"

grep -Eq '^sophia_live_wm schema=1 status=session_action_committed .* action=Logout$' \
    "$SESSION_LOG" || fail "normal logout was not committed"
grep -Eq '^sophia_live_session_protocol_errors schema=1 expected=[0-9]+ unexpected=0$' \
    "$SESSION_LOG" || fail "unexpected X protocol errors were observed"
selection="$(grep -E '^sophia_live_selection schema=1 status=complete ' "$SESSION_LOG" | tail -n 1)"
[[ -n "$selection" ]] || fail "selection summary is missing"
require_at_least "$selection" owner_changes 2
require_at_least "$selection" conversions 2
health="$(grep -E '^sophia_live_session_health schema=1 status=clean ' "$SESSION_LOG" | tail -n 1)"
[[ -n "$health" ]] || fail "clean session health summary is missing"
for assignment in protocol_errors=0 pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false; do
    require_eq "$health" "${assignment%%=*}" "${assignment#*=}"
done
keys="$(grep -E '^sophia_live_session_keys schema=2 status=complete ' "$SESSION_LOG" | tail -n 1)"
[[ -n "$keys" ]] || fail "final key-state summary is missing"
for assignment in pending=0 release_barrier_pending=0 repeat_active_seats=0 repeat_capacity_exhausted=0; do
    require_eq "$keys" "${assignment%%=*}" "${assignment#*=}"
done

mapfile -t completions < <(grep -E '^sophia_live_session schema=(14|15|16) status=bounded_complete ' "$SESSION_LOG")
(( ${#completions[@]} == 1 )) || fail "expected exactly one bounded session completion"
completion="${completions[0]}"
for assignment in \
    native_presentation=enabled physical_input=enabled wm_policy=external \
    wm_restarts=0 wm_degraded=false native_submit_failures=0 \
    native_retire_failures=0 native_callback_rejected=0 \
    native_callback_queue_saturated=0 native_in_flight=false \
    native_cleanup_pending=false present_disconnect_failures=0 \
    present_live_sources=0 present_live_fences=0 present_live_transactions=0; do
    require_eq "$completion" "${assignment%%=*}" "${assignment#*=}"
done
expected="$(field "$completion" input_events_expected)" || fail "completion lacks input_events_expected"
flushed="$(field "$completion" input_events_flushed)" || fail "completion lacks input_events_flushed"
[[ "$expected" == "$flushed" ]] || fail "input queue did not drain: expected=$expected flushed=$flushed"

grep -Eq '^sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 namespace=revoked xauthority=removed$' \
    "$SESSION_LOG" || fail "application/frontend/authority cleanup was not clean"
grep -Eq '^sophia_session_input_guard schema=2 status=ready .* keyboards=[1-9][0-9]*$' \
    "$GUARD_LOG" || fail "input guard did not discover a keyboard"
grep -Eq '^sophia_session_input_guard schema=1 status=armed$' \
    "$GUARD_LOG" || fail "input guard was not armed"
if grep -Eq '^sophia_session_input_guard schema=1 status=triggered$' "$GUARD_LOG"; then
    fail "run used emergency recovery instead of normal logout"
fi
recovery="$(grep -E '^sophia_tty_recovery schema=3 profile=xmonad ' "$RECOVERY_LOG" | tail -n 1)"
[[ -n "$recovery" ]] || fail "xmonad TTY recovery record is missing"
for assignment in termios_restored=true emergency=false session_shutdown=not_requested session_exit_status=none; do
    require_eq "$recovery" "${assignment%%=*}" "${assignment#*=}"
done
kd_before="$(field "$recovery" kd_mode_before)" || fail "recovery lacks kd_mode_before"
kd_after="$(field "$recovery" kd_mode_after)" || fail "recovery lacks kd_mode_after"
[[ "$kd_before" == "$kd_after" ]] || fail "KD mode was not restored"

echo "physical Firefox M10 workflow verified: $SESSION_LOG"
