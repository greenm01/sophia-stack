#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
LOG_DIR="${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"

fail() {
    echo "four-Kitty xmonad verification failed: $*" >&2
    exit 1
}

[[ -s "$SESSION_LOG" ]] || fail "missing session log: $SESSION_LOG"
if grep -Eq '(^Error:|panicked at|status=(failed|degraded)([[:space:]]|$))' "$SESSION_LOG"; then
    fail "session log contains a Sophia error, panic, or degraded status"
fi

mapfile -t launches < <(
    grep -nE '^sophia_session_app schema=1 status=started id=terminal source=(startup|action)$' \
        "$SESSION_LOG"
)
(( ${#launches[@]} >= 4 )) ||
    fail "observed ${#launches[@]} Kitty launches, expected at least four"

fourth_line="${launches[3]%%:*}"
four_window_log="$(mktemp)"
trap 'rm -f "$four_window_log"' EXIT
tail -n "+$fourth_line" "$SESSION_LOG" >"$four_window_log"

grep -Eq '^sophia_live_resize_epoch schema=1 status=held transaction=[0-9]+ surfaces=3$' \
    "$four_window_log" ||
    fail "four-window resize epoch was not held"
grep -Eq '^sophia_live_resize_epoch schema=1 status=committed transaction=[0-9]+ matched_surfaces=3$' \
    "$four_window_log" ||
    fail "three resized surfaces did not commit together"

if grep -Eq 'status=(layout_timeout|aborted)|rejected Present whose pixels do not match' \
    "$four_window_log"; then
    fail "four-window resize timed out, aborted, or rejected matching pixels"
fi

for target in \
    '1280x1440_0_0' \
    '1280x480_1280_0' \
    '1280x480_1280_480' \
    '1280x480_1280_960'; do
    grep -Eq \
        "^sophia_live_session_present schema=2 status=retired .* source=${target%%_*} target=${target} .* unit_scale=true$" \
        "$four_window_log" ||
        fail "missing pixel-matched retired tile: $target"
done

grep -Eq '^sophia_live_session_health schema=1 status=clean ' "$SESSION_LOG" ||
    fail "session did not finish cleanly"
grep -Eq '^sophia_live_session_protocol_errors schema=1 expected=[0-9]+ unexpected=0$' \
    "$SESSION_LOG" ||
    fail "session recorded an unexpected X protocol error"
completion="$(
    grep -E '^sophia_live_session schema=14 status=bounded_complete ' "$SESSION_LOG" |
        tail -n 1
)"
[[ "$completion" == *"native_submit_failures=0"* ]] ||
    fail "native presentation recorded a submit failure"
[[ "$completion" == *"native_retire_failures=0"* ]] ||
    fail "native presentation recorded a retirement failure"

echo "four-Kitty xmonad session verified: $SESSION_LOG"
