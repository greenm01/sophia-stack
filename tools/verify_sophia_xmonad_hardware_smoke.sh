#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
LOG_DIR="${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"
GUARD_LOG="${2:-$LOG_DIR/input-guard.log}"
RECOVERY_LOG="${3:-$LOG_DIR/recovery.log}"

fail() {
    echo "xmonad hardware-smoke verification failed: $*" >&2
    exit 1
}

field() {
    local line=$1 key=$2 token
    for token in $line; do
        if [[ "$token" == "$key="* ]]; then
            printf '%s\n' "${token#*=}"
            return 0
        fi
    done
    return 1
}

for evidence in "$SESSION_LOG" "$GUARD_LOG" "$RECOVERY_LOG"; do
    [[ -s "$evidence" ]] || fail "missing evidence: $evidence"
done
if grep -Eqi '(^Error:|panicked at|status=(failed|degraded)([[:space:]]|$)|outcome=forced_detach_)' \
    "$SESSION_LOG"; then
    fail "session contains an error, degradation, or forced detach"
fi

grep -q '^sophia_live_session_startup schema=2 status=output_baseline_ready outputs=2/2$' \
    "$SESSION_LOG" || fail "both physical output baselines were not ready"
startup="$(
    grep -E '^sophia_live_session_startup schema=2 status=ready ' "$SESSION_LOG" |
        head -n 1
)"
[[ -n "$startup" ]] || fail "startup readiness is missing"
startup_msec="$(field "$startup" elapsed_msec)" ||
    fail "startup readiness lacks elapsed_msec"
((startup_msec <= 8000)) || fail "startup exceeded eight seconds: ${startup_msec}ms"

launches="$(
    grep -Ec '^sophia_session_app schema=1 status=started id=terminal source=(startup|action)$' \
        "$SESSION_LOG" || true
)"
((launches >= 4)) || fail "observed $launches Kitty launches; four required"
grep -Eq '^sophia_live_wm schema=1 status=layout_committed .* surfaces=4 .* outcome=Committed$' \
    "$SESSION_LOG" || fail "four-surface layout did not commit"

"$(dirname "$0")/verify_sophia_xmonad_pointer_focus.sh" "$SESSION_LOG" >/dev/null

for record in \
    'sophia_live_session_vt schema=4 status=queued target=2 modifier_releases=[2-4]' \
    'sophia_live_session_vt schema=6 status=quiesced target=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=none' \
    'sophia_live_session_vt schema=4 status=requested target=2' \
    'sophia_live_seat schema=1 status=suspended' \
    'sophia_live_seat schema=1 status=active source=resume'; do
    grep -Eq "^${record}$" "$SESSION_LOG" ||
        fail "missing VT lifecycle record: $record"
done
for output in 1 2; do
    output_completion="$(
        grep -E "^sophia_live_output schema=1 status=complete output=${output} " \
            "$SESSION_LOG" | tail -n 1 || true
    )"
    [[ -n "$output_completion" ]] ||
        fail "physical output $output has no completion record"
    submissions="$(field "$output_completion" submissions)" ||
        fail "physical output $output lacks submission count"
    retirements="$(field "$output_completion" retirements)" ||
        fail "physical output $output lacks retirement count"
    callbacks="$(field "$output_completion" callbacks)" ||
        fail "physical output $output lacks callback count"
    nonzero_exports="$(field "$output_completion" nonzero_exports)" ||
        fail "physical output $output lacks nonzero-export count"
    for value in "$submissions" "$retirements" "$callbacks" "$nonzero_exports"; do
        [[ "$value" =~ ^[0-9]+$ ]] ||
            fail "physical output $output has a nonnumeric completion count"
    done
    ((submissions == retirements + 1 && callbacks == retirements)) ||
        fail "physical output $output did not retain exactly one displayed buffer"
    ((nonzero_exports > 0)) ||
        fail "physical output $output never displayed nonzero content"
done

grep -Eq '^sophia_live_wm schema=1 status=session_action_committed .* action=Logout$' \
    "$SESSION_LOG" || fail "normal logout was not committed"
grep -Eq '^sophia_live_session_health schema=1 status=clean .*pending_input=0 .*wm_degraded=false$' \
    "$SESSION_LOG" || fail "session health did not drain"
grep -Eq '^sophia_live_session_protocol_errors schema=1 expected=[0-9]+ unexpected=0$' \
    "$SESSION_LOG" || fail "unexpected protocol errors were recorded"
grep -Eq '^sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=none$' \
    "$SESSION_LOG" || fail "native teardown did not drain"
grep -Eq '^sophia_live_session_cleanup schema=1 status=clean ' "$SESSION_LOG" ||
    fail "session cleanup is missing"

completion="$(
    grep -E '^sophia_live_session schema=15 status=bounded_complete ' "$SESSION_LOG" |
        tail -n 1
)"
[[ -n "$completion" ]] || fail "bounded completion is missing"
for assignment in \
    physical_input=enabled \
    wm_policy=external \
    wm_restarts=0 \
    wm_degraded=false \
    native_submit_failures=0 \
    native_retire_failures=0 \
    native_callback_rejected=0 \
    native_in_flight=false \
    native_cleanup_pending=false; do
    [[ " $completion " == *" $assignment "* ]] ||
        fail "completion does not contain $assignment"
done
for key in physical_keys_routed physical_pointer_routed native_mixed_exports; do
    value="$(field "$completion" "$key")" ||
        fail "completion is missing $key"
    ((value > 0)) || fail "$key did not record activity"
done

grep -q '^sophia_session_input_guard schema=1 status=armed$' "$GUARD_LOG" ||
    fail "input guard was not armed"
if grep -q '^sophia_session_input_guard schema=1 status=triggered$' "$GUARD_LOG"; then
    fail "normal smoke used emergency recovery"
fi
recovery="$(
    grep -E '^sophia_tty_recovery schema=3 profile=xmonad ' "$RECOVERY_LOG" |
        tail -n 1
)"
[[ -n "$recovery" ]] || fail "TTY recovery is missing"
for expected in termios_restored=true emergency=false; do
    [[ " $recovery " == *" $expected "* ]] ||
        fail "TTY recovery lacks $expected"
done
kd_before="$(field "$recovery" kd_mode_before)" || fail "recovery lacks kd_mode_before"
kd_after="$(field "$recovery" kd_mode_after)" || fail "recovery lacks kd_mode_after"
[[ "$kd_before" == "$kd_after" ]] || fail "KD mode was not restored"

echo "Short xmonad physical hardware smoke passed: $SESSION_LOG"
