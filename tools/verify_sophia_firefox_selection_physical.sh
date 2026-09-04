#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
SESSION_LOG="${1:-${SOPHIA_HAGIA_LOG_DIR:-$STATE_HOME/sophia/hagia-session}/session.log}"

fail() { echo "focused Firefox selection verification failed: $*" >&2; exit 1; }
line() { grep -nE "$1" "$SESSION_LOG" | head -n 1 | cut -d: -f1 || true; }
after() { grep -nE "$1" "$SESSION_LOG" | cut -d: -f1 | awk -v n="$2" '$1 > n { print; exit }' || true; }
interval() {
    local first="$1" last="$2" name="$3" owner conversion
    owner="$(after '^sophia_firefox_m8 schema=1 status=selection_observed kind=owner_change ' "$first")"
    conversion="$(after '^sophia_firefox_m8 schema=1 status=selection_observed kind=conversion ' "$owner")"
    [[ -n "$owner" && -n "$conversion" ]] && (( owner < conversion && conversion < last )) ||
        fail "$name did not complete an ordered owner-to-requestor transfer"
}

[[ -s "$SESSION_LOG" ]] || fail "missing session log: $SESSION_LOG"
! grep -Eqi '(^Error:|panicked at|status=(failed|degraded)([[:space:]]|$))' "$SESSION_LOG" ||
    fail 'session reported a fatal or degraded result'
[[ "$(grep -Ec '^sophia_session_app schema=1 status=started id=terminal source=startup$' "$SESSION_LOG")" == 1 ]] ||
    fail 'selection slice requires one Kitty'
[[ "$(grep -Ec '^sophia_session_app schema=1 status=started id=firefox source=action$' "$SESSION_LOG")" == 1 ]] ||
    fail 'selection slice requires one Firefox launch'

keyboard="$(line '^sophia_firefox_m8 schema=1 status=stage_complete stage=keyboard ')"
clipboard_peer="$(line '^sophia_firefox_selection schema=1 status=kitty_checkpoint checkpoint=clipboard_peer ')"
clipboard="$(line '^sophia_firefox_m8 schema=1 status=stage_complete stage=clipboard ')"
primary_peer="$(line '^sophia_firefox_selection schema=1 status=kitty_checkpoint checkpoint=primary_peer ')"
primary="$(line '^sophia_firefox_m8 schema=1 status=stage_complete stage=primary ')"
[[ -n "$keyboard" && -n "$clipboard_peer" && -n "$clipboard" && -n "$primary_peer" && -n "$primary" ]] ||
    fail 'selection stage evidence is incomplete'
interval "$keyboard" "$clipboard_peer" 'Firefox-to-Kitty CLIPBOARD'
interval "$clipboard_peer" "$clipboard" 'Kitty-to-Firefox CLIPBOARD'
interval "$clipboard" "$primary_peer" 'Firefox-to-Kitty PRIMARY'
interval "$primary_peer" "$primary" 'Kitty-to-Firefox PRIMARY'

grep -Eq '^sophia_firefox_selection schema=1 status=complete stages=4 kitty_checkpoints=3 selection_owner_changes=([4-9]|[1-9][0-9]+) selection_conversions=([4-9]|[1-9][0-9]+) content=redacted$' "$SESSION_LOG" ||
    fail 'focused selection completion record is missing'
grep -Eq '^sophia_live_session_health schema=1 status=clean .* pending_wm=0 pending_actions=0 pending_input=0 .*wm_degraded=false' "$SESSION_LOG" ||
    fail 'session health did not drain cleanly'
grep -Eq '^sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 ' "$SESSION_LOG" ||
    fail 'application/frontend cleanup did not complete'
echo "focused Firefox selection workflow verified: $SESSION_LOG"
