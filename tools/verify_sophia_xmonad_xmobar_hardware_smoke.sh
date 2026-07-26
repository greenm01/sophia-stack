#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
LOG_DIR="${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"
GUARD_LOG="${2:-$LOG_DIR/input-guard.log}"
RECOVERY_LOG="${3:-$LOG_DIR/recovery.log}"

fail() {
    echo "xmobar hardware-smoke verification failed: $*" >&2
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
if grep -Eqi '(^Error:|panicked at|status=(failed|degraded)([[:space:]]|$))' \
    "$SESSION_LOG"; then
    fail "session contains an error, panic, or degradation"
fi

grep -q '^sophia_session_app schema=1 status=started id=statusbar source=startup$' \
    "$SESSION_LOG" || fail "status bar did not start"
grep -q '^sophia_live_work_area schema=1 status=reduced outputs=2 changed=2 rejected=0 active_reservations=1$' \
    "$SESSION_LOG" || fail "bar reservation did not reduce both outputs"

mapfile -t work_areas < <(
    grep -E '^sophia_live_work_area schema=1 status=applied output=[0-9]+ ' "$SESSION_LOG"
)
(( ${#work_areas[@]} == 2 )) ||
    fail "expected two applied work areas, observed ${#work_areas[@]}"
for line in "${work_areas[@]}"; do
    full="$(field "$line" full)" || fail "work area lacks full geometry"
    work="$(field "$line" work)" || fail "work area lacks reduced geometry"
    full_height="${full#*x}"
    full_height="${full_height%%_*}"
    work_height="${work#*x}"
    work_height="${work_height%%_*}"
    work_y="${work##*_}"
    ((work_y > 0 && work_height + work_y == full_height)) ||
        fail "work area does not exactly reserve the top edge: $line"
done

grep -Eq '^sophia_live_session_present schema=2 status=retired .* target=[1-9][0-9]*x[1-9][0-9]*_-?[0-9]+_[1-9][0-9]* .* unit_scale=true$' \
    "$SESSION_LOG" || fail "managed presentation did not begin below the bar"
grep -Eq '^sophia_live_compositor_chrome_set schema=1 status=composed generation=[0-9]+ eligible_surfaces=1 frames=1 focused_frames=1 unfocused_frames=0 focus_rings=1 primitives=8 clearance=4$' \
    "$SESSION_LOG" || fail "compositor chrome did not exclude the bar"
for kind in button axis; do
    grep -q "^sophia_live_session_pointer schema=4 status=target_routed role=client_positioned kind=${kind}$" \
        "$SESSION_LOG" || fail "bar did not receive pointer $kind input"
done

grep -Eq '^sophia_live_wm schema=1 status=session_action_committed .* action=Logout$' \
    "$SESSION_LOG" || fail "normal logout did not commit"
grep -Eq '^sophia_live_session_health schema=1 status=clean .*wm_degraded=false$' \
    "$SESSION_LOG" || fail "session health was not clean"
grep -q '^sophia_live_session_cleanup schema=1 status=clean ' "$SESSION_LOG" ||
    fail "session cleanup is missing"
grep -q '^sophia_session_input_guard schema=1 status=armed$' "$GUARD_LOG" ||
    fail "input guard was not armed"
if grep -q '^sophia_session_input_guard schema=1 status=triggered$' "$GUARD_LOG"; then
    fail "normal bar smoke used emergency recovery"
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

echo "Short xmobar physical hardware smoke passed: $SESSION_LOG"
