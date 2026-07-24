#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
LOG_DIR="${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"
GUARD_LOG="${2:-$LOG_DIR/input-guard.log}"
RECOVERY_LOG="${3:-$LOG_DIR/recovery.log}"

fail() {
    echo "xmonad input proof failed: $*" >&2
    exit 1
}
field() {
    local line="$1" key="$2" token
    for token in $line; do
        if [[ "$token" == "$key="* ]]; then
            printf '%s\n' "${token#*=}"
            return
        fi
    done
    return 1
}
require_eq() {
    local line="$1" key="$2" expected="$3" actual
    actual="$(field "$line" "$key")" || fail "missing completion field: $key"
    [[ "$actual" == "$expected" ]] ||
        fail "$key is $actual, expected $expected"
}
require_positive() {
    local line="$1" key="$2" actual
    actual="$(field "$line" "$key")" || fail "missing completion field: $key"
    [[ "$actual" =~ ^[0-9]+$ ]] && (( actual > 0 )) ||
        fail "$key did not record activity"
}

for evidence in "$SESSION_LOG" "$GUARD_LOG" "$RECOVERY_LOG"; do
    [[ -s "$evidence" ]] || fail "missing evidence: $evidence"
done
if grep -Eqi '(^|[[:space:]])(panic|error:|status=(failed|degraded))' "$SESSION_LOG"; then
    fail "session log contains an error, panic, or degraded status"
fi
grep -Eq '^sophia_live_wm schema=1 status=ready adapter=external socket=private restarts=0$' \
    "$SESSION_LOG" || fail "xmonad never became ready"
grep -Eq '^sophia_session_app schema=1 status=started id=terminal source=startup$' \
    "$SESSION_LOG" || fail "startup terminal was not launched"
grep -Eq '^sophia_live_wm schema=1 status=layout_committed .* outcome=Committed$' \
    "$SESSION_LOG" || fail "xmonad layout was not committed"
grep -Eq '^sophia_live_wm schema=1 status=focus_committed .* target=surface$' \
    "$SESSION_LOG" || fail "xmonad focus was not committed"
grep -Eq '^sophia_live_session_input_pipeline schema=2 status=content_ready source=stable_present_scanout$' \
    "$SESSION_LOG" || fail "focused DMA-BUF content was not stably presented"
grep -Eq '^sophia_live_session_input_pipeline schema=1 status=key_observed$' \
    "$SESSION_LOG" || fail "physical key was not observed"
grep -Eq '^sophia_live_session_input_pipeline schema=1 status=key_routed$' \
    "$SESSION_LOG" || fail "physical key was not routed"
grep -Eq '^sophia_live_session_input schema=1 status=ready source=physical text=sophia$' \
    "$SESSION_LOG" || fail "exact physical text proof never became ready"
grep -Eq '^sophia_live_session_pointer schema=2 status=motion_routed$' \
    "$SESSION_LOG" || fail "physical pointer motion was not routed"
grep -Eq '^sophia_live_session_pointer schema=2 status=button_routed count=[1-9][0-9]*$' \
    "$SESSION_LOG" || fail "physical pointer button was not routed"
grep -Eq '^sophia_live_session_pointer schema=1 status=ready source=physical action=select$' \
    "$SESSION_LOG" || fail "physical pointer proof never became ready"

layout_line="$(grep -n -m1 '^sophia_live_wm schema=1 status=layout_committed .* outcome=Committed$' "$SESSION_LOG" | cut -d: -f1)"
focus_line="$(grep -n -m1 '^sophia_live_wm schema=1 status=focus_committed .* target=surface$' "$SESSION_LOG" | cut -d: -f1)"
content_line="$(grep -n -m1 '^sophia_live_session_input_pipeline schema=2 status=content_ready source=stable_present_scanout$' "$SESSION_LOG" | cut -d: -f1)"
input_line="$(grep -n -m1 '^sophia_live_session_input schema=1 status=ready source=physical text=sophia$' "$SESSION_LOG" | cut -d: -f1)"
(( layout_line < input_line && focus_line < input_line && content_line < input_line )) ||
    fail "physical input became ready before layout, focus, and content"
grep -Eq '^sophia_live_session_health schema=1 status=clean .* wm_degraded=false$' \
    "$SESSION_LOG" || fail "final health was not clean"

mapfile -t completions < <(
    grep -E '^sophia_live_session schema=14 status=bounded_complete ' "$SESSION_LOG"
)
(( ${#completions[@]} == 1 )) ||
    fail "expected one schema-14 completion; found ${#completions[@]}"
completion="${completions[0]}"
for assignment in \
    input_pixel_change=true \
    input_text_match=true \
    pointer_pixel_change=true \
    pointer_proof=enabled \
    native_presentation=enabled \
    physical_input=enabled \
    wm_policy=external \
    wm_restarts=0 \
    wm_degraded=false \
    native_submit_failures=0 \
    native_retire_failures=0 \
    native_callback_rejected=0 \
    native_in_flight=false \
    native_cleanup_pending=false \
    present_disconnect_failures=0 \
    present_live_sources=0 \
    present_live_fences=0 \
    present_live_transactions=0; do
    require_eq "$completion" "${assignment%%=*}" "${assignment#*=}"
done
for key in physical_events physical_keys_routed physical_pointer_events \
    physical_pointer_routed native_submissions native_retirements \
    native_frame_uploads wm_committed; do
    require_positive "$completion" "$key"
done
expected="$(field "$completion" input_events_expected)" ||
    fail "missing input_events_expected"
flushed="$(field "$completion" input_events_flushed)" ||
    fail "missing input_events_flushed"
[[ "$expected" == "$flushed" ]] ||
    fail "input queue did not drain: expected=$expected flushed=$flushed"

grep -Eq '^sophia_session_input_guard schema=1 status=armed$' "$GUARD_LOG" ||
    fail "independent input guard was not armed"
if grep -Eq '^sophia_session_input_guard schema=1 status=triggered$' "$GUARD_LOG"; then
    fail "proof used emergency recovery"
fi
recovery="$(
    grep -E '^sophia_tty_recovery schema=2 profile=xmonad ' "$RECOVERY_LOG" |
        tail -n 1
)"
[[ -n "$recovery" ]] || fail "xmonad TTY recovery record is missing"
require_eq "$recovery" termios_restored true
require_eq "$recovery" emergency false
kd_before="$(field "$recovery" kd_mode_before)" || fail "missing kd_mode_before"
kd_after="$(field "$recovery" kd_mode_after)" || fail "missing kd_mode_after"
[[ "$kd_before" == "$kd_after" ]] || fail "KD mode was not restored"

echo "xmonad exact physical input proof passed: $SESSION_LOG"
