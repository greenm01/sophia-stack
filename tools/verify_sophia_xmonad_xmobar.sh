#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
LOG_DIR="${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"
GUARD_LOG="${2:-$LOG_DIR/input-guard.log}"
RECOVERY_LOG="${3:-$LOG_DIR/recovery.log}"

fail() {
    echo "xmobar xmonad verification failed: $*" >&2
    exit 1
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

for evidence in "$SESSION_LOG" "$GUARD_LOG" "$RECOVERY_LOG"; do
    [[ -s "$evidence" ]] || fail "missing or empty evidence: $evidence"
done

if grep -Eqi '(^Error:|panicked at|status=(failed|degraded)([[:space:]]|$))' \
    "$SESSION_LOG"; then
    fail "session contains an error, panic, or degraded status"
fi

require_line '^sophia_session_app schema=1 status=started id=statusbar source=startup$' \
    "$SESSION_LOG" "xmobar was not started by the session registry"
require_line \
    '^sophia_live_work_area schema=1 status=reduced outputs=2 changed=2 rejected=0 active_reservations=1$' \
    "$SESSION_LOG" "one valid reservation was not reduced across both outputs"

mapfile -t work_areas < <(
    grep -E '^sophia_live_work_area schema=1 status=applied output=[0-9]+ ' "$SESSION_LOG"
)
(( ${#work_areas[@]} == 2 )) ||
    fail "expected exactly two applied work areas, observed ${#work_areas[@]}"

for line in "${work_areas[@]}"; do
    full="$(field "$line" full)" || fail "work-area record is missing full geometry"
    work="$(field "$line" work)" || fail "work-area record is missing reduced geometry"
    full_height="${full#*x}"
    full_height="${full_height%%_*}"
    work_height="${work#*x}"
    work_height="${work_height%%_*}"
    work_y="${work##*_}"
    [[ "$full_height" =~ ^[0-9]+$ && "$work_height" =~ ^[0-9]+$ &&
        "$work_y" =~ ^[0-9]+$ ]] ||
        fail "work-area geometry is malformed: $line"
    (( work_y > 0 && work_height + work_y == full_height )) ||
        fail "work area does not exactly reserve the top edge: $line"
done

require_line \
    '^sophia_live_session_present schema=2 status=retired .* target=[1-9][0-9]*x[1-9][0-9]*_-?[0-9]+_[1-9][0-9]* .* unit_scale=true$' \
    "$SESSION_LOG" "no pixel-matched managed presentation began below the bar"
require_line \
    '^sophia_live_compositor_chrome_set schema=1 status=composed generation=[0-9]+ eligible_surfaces=1 frames=1 focused_frames=1 unfocused_frames=0 focus_rings=1 primitives=8 clearance=4$' \
    "$SESSION_LOG" "managed Kitty chrome did not exclude the client-positioned bar"
for pointer_kind in button axis; do
    require_line \
        "^sophia_live_session_pointer schema=4 status=target_routed role=client_positioned kind=${pointer_kind}$" \
        "$SESSION_LOG" "pointer $pointer_kind input was not routed to the client-positioned bar"
done
require_line '^sophia_live_wm schema=2 status=workspace_projection_committed .* workspace=2 ' \
    "$SESSION_LOG" "workspace-away projection was not committed"
require_line '^sophia_live_wm schema=2 status=workspace_projection_committed .* workspace=1 ' \
    "$SESSION_LOG" "workspace-return projection was not committed"

for record in \
    'sophia_live_session_vt schema=4 status=queued target=[0-9]+ modifier_releases=[2-4]' \
    'sophia_live_session_vt schema=6 status=quiesced target=[0-9]+ outcome=drained drained=true abandoned_scanouts=0 skipped_present=none' \
    'sophia_live_seat schema=1 status=suspended' \
    'sophia_live_seat schema=1 status=active source=resume'; do
    require_line "^${record}$" "$SESSION_LOG" "VT lifecycle record is missing: $record"
done

require_line '^sophia_live_wm schema=1 status=session_action_committed .* action=Logout$' \
    "$SESSION_LOG" "normal xmonad logout was not committed"
require_line '^sophia_live_session_health schema=1 status=clean .* wm_degraded=false$' \
    "$SESSION_LOG" "session health was not clean"
require_line '^sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=none$' \
    "$SESSION_LOG" "native presentation did not drain cleanly"
require_line '^sophia_live_session_protocol_errors schema=1 expected=[0-9]+ unexpected=0$' \
    "$SESSION_LOG" "unexpected X protocol errors were recorded"
require_line '^sophia_live_session_cleanup schema=1 status=clean ' \
    "$SESSION_LOG" "session cleanup did not complete"

require_line '^sophia_session_input_guard schema=1 status=armed$' \
    "$GUARD_LOG" "emergency input guard was not armed"
if grep -Eq '^sophia_session_input_guard schema=1 status=triggered$' "$GUARD_LOG"; then
    fail "normal proof used emergency recovery"
fi

recovery="$(
    grep -E '^sophia_tty_recovery schema=3 profile=xmonad ' "$RECOVERY_LOG" |
        tail -n 1
)"
[[ -n "$recovery" ]] || fail "TTY recovery record is missing"
for expected in termios_restored=true emergency=false; do
    [[ " $recovery " == *" $expected "* ]] ||
        fail "TTY recovery does not contain $expected"
done
kd_before="$(field "$recovery" kd_mode_before)" || fail "recovery lacks kd_mode_before"
kd_after="$(field "$recovery" kd_mode_after)" || fail "recovery lacks kd_mode_after"
[[ "$kd_before" == "$kd_after" ]] || fail "KD mode was not restored"

echo "xmobar work-area physical session verified: $SESSION_LOG"
