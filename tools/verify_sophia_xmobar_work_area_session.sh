#!/usr/bin/env bash
set -euo pipefail

SESSION_LOG="${1:?usage: verify_sophia_xmobar_work_area_session.sh SESSION_LOG}"

fail() {
    echo "xmobar/work-area verification failed: $*" >&2
    exit 1
}
require_line() {
    grep -Eq "$1" "$SESSION_LOG" || fail "$2"
}

[[ -s "$SESSION_LOG" ]] || fail "missing or empty session log: $SESSION_LOG"
require_line \
    '^sophia_session_app schema=1 status=started id=statusbar source=startup$' \
    "xmobar was not started as the status bar"
[[ "$(grep -Ec '^sophia_live_work_area schema=1 status=reduced outputs=2 changed=2 rejected=0 active_reservations=1$' "$SESSION_LOG")" == 1 ]] ||
    fail "the two-output work area was not reduced exactly once"
[[ "$(grep -Ec '^sophia_live_work_area schema=1 status=applied output=1 full=2560x1440_0_0 work=2560x1426_0_14$' "$SESSION_LOG")" == 1 ]] ||
    fail "the primary work area does not reserve exactly the 14-pixel bar"
[[ "$(grep -Ec '^sophia_live_work_area schema=1 status=applied output=2 full=1920x1080_2560_0 work=1920x1066_2560_14$' "$SESSION_LOG")" == 1 ]] ||
    fail "the secondary work area does not reserve exactly the 14-pixel bar"

reduced_line="$(grep -nEm1 '^sophia_live_work_area schema=1 status=reduced ' "$SESSION_LOG" | cut -d: -f1)"
complete_line="$(grep -nEm1 '^sophia_live_session schema=16 status=bounded_complete ' "$SESSION_LOG" | cut -d: -f1)"
[[ "$reduced_line" =~ ^[0-9]+$ && "$complete_line" =~ ^[0-9]+$ && "$complete_line" -gt "$reduced_line" ]] ||
    fail "normal session completion did not follow work-area negotiation"
bar_repaints="$({
    sed -n "$((reduced_line + 1)),$((complete_line - 1))p" "$SESSION_LOG" |
        grep -Ec 'sophia_live_output_repaint schema=1 status=presented output=1 mode=partial rects=1 pixels=35840$'
} || true)"
[[ "$bar_repaints" =~ ^[0-9]+$ && "$bar_repaints" -ge 3 ]] ||
    fail "fewer than three exact 2560x14 bar repaints reached output 1"
primary_retirements="$({
    sed -n "$((reduced_line + 1)),$((complete_line - 1))p" "$SESSION_LOG" |
        grep -Ec 'sophia_live_native_page_flip schema=1 status=retired output=1 '
} || true)"
[[ "$primary_retirements" =~ ^[0-9]+$ && "$primary_retirements" -ge 3 ]] ||
    fail "bar updates were not accompanied by primary-output retirement"
require_line \
    '^sophia_live_output schema=1 status=complete output=1 checksum=[1-9][0-9]* submissions=[1-9][0-9]* retirements=[1-9][0-9]* callbacks=[1-9][0-9]* nonzero_exports=[1-9][0-9]*$' \
    "the primary output did not complete with nonzero retired content"
require_line '^xmobar: Caught signal 15; exiting\.\.\.$' \
    "xmobar did not exit with the session"
require_line \
    '^sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 namespace=revoked xauthority=removed$' \
    "session cleanup left owned resources behind"

echo "xmobar/work-area session gate passed: bar_repaints=$bar_repaints primary_retirements=$primary_retirements"
