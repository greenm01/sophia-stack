#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
SESSION_LOG="${1:-$STATE_HOME/sophia/xmonad-session/session.log}"

fail() {
    echo "xmonad keyboard/VT verification failed: $*" >&2
    exit 1
}

[[ -s "$SESSION_LOG" ]] || fail "missing session evidence: $SESSION_LOG"
if grep -Eqi '(^Error:|panicked at|status=(failed|degraded)([[:space:]]|$)|outcome=forced_detach_)' \
    "$SESSION_LOG"; then
    fail "session contains an error, degraded state, or forced detach"
fi

coverage="$(
    grep -E '^sophia_live_keyboard_coverage schema=1 status=complete ' "$SESSION_LOG" |
        tail -n 1
)"
[[ "$coverage" == *'shifted_positions=21 shifted_positions_required=21 '* ]] ||
    fail "all 21 shifted pc105 positions were not observed"
[[ "$coverage" == *'virtual_terminals=12 virtual_terminals_required=12 content=redacted' ]] ||
    fail "all twelve Ctrl-Alt-Fn targets were not observed"

mapfile -t targets < <(
    sed -n 's/^sophia_live_session_vt schema=4 status=queued target=\([0-9][0-9]*\) .*/\1/p' \
        "$SESSION_LOG" | sort -n -u
)
[[ "${targets[*]}" == '1 2 3 4 5 6 7 8 9 10 11 12' ]] ||
    fail "queued VT targets are incomplete: ${targets[*]:-none}"
for target in {1..12}; do
    grep -Eq "^sophia_live_session_vt schema=4 status=requested target=${target}$" "$SESSION_LOG" ||
        fail "VT target $target was not requested through libseat"
done

quiesced="$(
    grep -Ec '^sophia_live_session_vt schema=6 status=quiesced .* outcome=drained drained=true abandoned_scanouts=0 skipped_present=none$' \
        "$SESSION_LOG" || true
)"
resumed="$(
    grep -Ec '^sophia_live_seat schema=1 status=active source=resume$' "$SESSION_LOG" || true
)"
((quiesced >= 11 && resumed >= 11)) ||
    fail "away/return lifecycle is incomplete: quiesced=$quiesced resumed=$resumed"

grep -Eq '^sophia_live_session_keys schema=2 status=complete pending=0 release_barrier_pending=0 .*repeat_active_seats=0 ' \
    "$SESSION_LOG" || fail "key state did not drain"
grep -Eq '^sophia_live_session_health schema=1 status=clean .*pending_input=0 .*wm_degraded=false$' \
    "$SESSION_LOG" || fail "session health did not drain"
grep -q '^sophia_live_session_cleanup schema=1 status=clean ' "$SESSION_LOG" ||
    fail "session cleanup is missing"

echo "xmonad pc105 US and F1-F12 VT evidence passed: $SESSION_LOG"
