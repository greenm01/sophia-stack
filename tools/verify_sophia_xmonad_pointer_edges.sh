#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
LOG_DIR="${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"
GUARD_LOG="${2:-$LOG_DIR/input-guard.log}"
RECOVERY_LOG="${3:-$LOG_DIR/recovery.log}"

fail() {
    echo "xmonad pointer-edge verification failed: $*" >&2
    exit 1
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
require_at_least() {
    local line="$1" key="$2" minimum="$3" actual
    actual="$(field "$line" "$key")" || fail "record is missing $key"
    [[ "$actual" =~ ^[0-9]+$ ]] || fail "$key is not an integer: $actual"
    ((actual >= minimum)) ||
        fail "$key is $actual, expected at least $minimum"
}
ordered_pair_count() {
    local axis="$1" side="$2" output_slot="$3"
    awk -v axis="$axis" -v side="$side" -v output_slot="$output_slot" '
        $0 == "sophia_live_session_pointer schema=8 status=output_edge_confined axis=" axis " side=" side " output_slot=" output_slot {
            contacts++
            next
        }
        $0 == "sophia_live_session_pointer schema=8 status=edge_reverse_immediate axis=" axis " side=" side " output_slot=" output_slot &&
        pairs < contacts {
            pairs++
        }
        END { print pairs + 0 }
    ' "$SESSION_LOG"
}

for evidence in "$SESSION_LOG" "$GUARD_LOG" "$RECOVERY_LOG"; do
    [[ -s "$evidence" ]] || fail "missing or empty evidence file: $evidence"
done
if grep -Eqi \
    '(^Error:|panicked at|^sophia_[^[:space:]]+ .*status=(failed|degraded)([[:space:]]|$))' \
    "$SESSION_LOG"; then
    fail "session log contains a Sophia error, panic, or degraded status"
fi

grep -Eq \
    '^sophia_live_outputs schema=2 status=ready discovered=2 presentation=2 native_owned=2 multi_output_scanout=enabled ' \
    "$SESSION_LOG" ||
    fail "the session did not own two presentation outputs"
for boundary in \
    'horizontal minimum 0' \
    'horizontal maximum 1' \
    'vertical minimum 0' \
    'vertical minimum 1' \
    'vertical maximum 0' \
    'vertical maximum 1'; do
    read -r axis side output_slot <<<"$boundary"
    pairs="$(ordered_pair_count "$axis" "$side" "$output_slot")"
    ((pairs >= 1)) ||
        fail "$axis $side on output slot $output_slot has no ordered clamp/reversal pair"
done
for transition in \
    'from_slot=0 to_slot=1' \
    'from_slot=1 to_slot=0'; do
    grep -Eq \
        "^sophia_live_session_pointer schema=8 status=output_transition ${transition} boundary=free$" \
        "$SESSION_LOG" ||
        fail "free internal-seam transition is missing: $transition"
done

cursor="$(
    grep -E '^sophia_live_session_cursor schema=5 path=legacy_ioctl ' "$SESSION_LOG" |
        tail -n 1
)"
[[ -n "$cursor" ]] || fail "final cursor health record is missing"
require_at_least "$cursor" hardware_updates 1
require_at_least "$cursor" updates_primary_in_flight 1
require_eq "$cursor" hidden_updates 0
require_eq "$cursor" hardware_failures 0

health="$(
    grep -E '^sophia_live_session_health schema=1 status=clean ' "$SESSION_LOG" |
        tail -n 1
)"
[[ -n "$health" ]] || fail "final session health was not clean"
require_eq "$health" protocol_errors 0
require_eq "$health" pending_wm 0
require_eq "$health" pending_actions 0
require_eq "$health" pending_input 0
require_eq "$health" wm_degraded false

completion="$(
    grep -E '^sophia_live_session schema=(14|15) status=bounded_complete ' \
        "$SESSION_LOG" |
        tail -n 1
)"
[[ -n "$completion" ]] || fail "bounded session completion is missing"
require_at_least "$completion" physical_pointer_events 1
require_at_least "$completion" physical_pointer_routed 1
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

grep -Eq \
    '^sophia_live_wm schema=1 status=session_action_committed transaction=[0-9]+ action=Logout$' \
    "$SESSION_LOG" ||
    fail "normal Super-Shift-Q logout was not committed"
grep -Eq '^sophia_session_input_guard schema=1 status=armed$' "$GUARD_LOG" ||
    fail "emergency input guard was not armed"
if grep -Eq '^sophia_session_input_guard schema=1 status=triggered$' "$GUARD_LOG"; then
    fail "run used emergency recovery instead of normal logout"
fi

recovery="$(
    grep -E '^sophia_tty_recovery schema=3 profile=xmonad ' "$RECOVERY_LOG" |
        tail -n 1
)"
[[ -n "$recovery" ]] || fail "xmonad TTY recovery record is missing"
require_eq "$recovery" termios_restored true
require_eq "$recovery" emergency false
require_eq "$recovery" session_shutdown not_requested
require_eq "$recovery" session_exit_status none
kd_before="$(field "$recovery" kd_mode_before)" ||
    fail "recovery record is missing kd_mode_before"
kd_after="$(field "$recovery" kd_mode_after)" ||
    fail "recovery record is missing kd_mode_after"
[[ "$kd_before" == "$kd_after" ]] ||
    fail "KD mode was not restored: before=$kd_before after=$kd_after"

echo "xmonad pointer-edge verification passed: $SESSION_LOG"
