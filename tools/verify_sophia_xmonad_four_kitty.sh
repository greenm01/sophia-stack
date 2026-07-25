#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
LOG_DIR="${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"
WAIT_SECONDS="${SOPHIA_VERIFY_WAIT_SECONDS:-5}"

fail() {
    echo "four-Kitty xmonad verification failed: $*" >&2
    exit 1
}

[[ -s "$SESSION_LOG" ]] || fail "missing session log: $SESSION_LOG"

deadline=$((SECONDS + WAIT_SECONDS))
while ! grep -Eq '^sophia_live_session_cleanup schema=1 status=clean ' "$SESSION_LOG" ||
    ! grep -Eq '^sophia_live_session schema=14 status=bounded_complete ' "$SESSION_LOG"; do
    (( SECONDS < deadline )) || fail "session log is incomplete"
    sleep 0.1
done

if grep -Eq '(^Error:|panicked at|status=(failed|degraded)([[:space:]]|$))' "$SESSION_LOG"; then
    fail "session log contains a Sophia error, panic, or degraded status"
fi
if grep -Eq 'status=submitted .* content=None|outcome=forced_detach_|abandoned_scanouts=[1-9]' \
    "$SESSION_LOG"; then
    fail "session submitted empty output content or used forced native detach"
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
grep -Eq \
    '^sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=none$' \
    "$SESSION_LOG" ||
    fail "native presentation did not drain cleanly"
grep -Eq '^sophia_live_session_cleanup schema=1 status=clean ' "$SESSION_LOG" ||
    fail "session cleanup did not complete cleanly"
grep -Eq '^sophia_live_session_protocol_errors schema=1 expected=[0-9]+ unexpected=0$' \
    "$SESSION_LOG" ||
    fail "session recorded an unexpected X protocol error"
mapfile -t completions < <(
    grep -E '^sophia_live_session schema=14 status=bounded_complete ' "$SESSION_LOG"
)
(( ${#completions[@]} == 1 )) ||
    fail "expected one completed session, found ${#completions[@]}"
completion="${completions[0]}"
for assignment in \
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
    [[ " $completion " == *" $assignment "* ]] ||
        fail "completion does not contain $assignment"
done

mapfile -t output_completions < <(
    grep -E '^sophia_live_output schema=1 status=complete ' "$SESSION_LOG"
)
(( ${#output_completions[@]} >= 1 )) ||
    fail "session has no per-output completion records"
for output_completion in "${output_completions[@]}"; do
    submissions="$(sed -n 's/.* submissions=\([0-9][0-9]*\) .*/\1/p' <<<"$output_completion")"
    retirements="$(sed -n 's/.* retirements=\([0-9][0-9]*\) .*/\1/p' <<<"$output_completion")"
    callbacks="$(sed -n 's/.* callbacks=\([0-9][0-9]*\) .*/\1/p' <<<"$output_completion")"
    [[ -n "$submissions" && -n "$retirements" && -n "$callbacks" ]] ||
        fail "malformed output completion: $output_completion"
    (( submissions == retirements + 1 )) ||
        fail "output did not retain exactly one displayed buffer: $output_completion"
    (( callbacks == retirements )) ||
        fail "output callback/retirement counts diverged: $output_completion"
done

echo "four-Kitty xmonad session verified: $SESSION_LOG"
