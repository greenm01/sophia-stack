#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
SESSION_LOG="${1:-${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}/session.log}"

fail() { echo "Firefox PRIMARY verification failed: $*" >&2; exit 1; }
line() { grep -nE "$1" "$SESSION_LOG" | sed -n '1p' | cut -d: -f1 || true; }
after() { grep -nE "$1" "$SESSION_LOG" | cut -d: -f1 | awk -v n="$2" '$1 > n { print; exit }' || true; }

[[ -s "$SESSION_LOG" ]] || fail "missing session log: $SESSION_LOG"
! grep -Eqi '(^Error:|panicked at|status=(failed|degraded)([[:space:]]|$))' "$SESSION_LOG" ||
    fail 'session reported a fatal or degraded result'
[[ "$(grep -Ec '^sophia_session_app schema=1 status=started id=terminal source=startup$' "$SESSION_LOG")" == 1 ]] ||
    fail 'PRIMARY slice requires one startup Kitty'
[[ "$(grep -Ec '^sophia_session_app schema=1 status=started id=firefox source=action$' "$SESSION_LOG")" == 1 ]] ||
    fail 'PRIMARY slice requires one Firefox action launch'
! grep -Eq 'checkpoint=clipboard_peer|stage_complete stage=clipboard' "$SESSION_LOG" ||
    fail 'PRIMARY slice replayed completed CLIPBOARD work'

source_armed="$(line '^sophia_firefox_primary schema=1 status=checkpoint checkpoint=source_armed title_bytes=251 content=redacted$')"
kitty_received="$(line '^sophia_firefox_primary schema=1 status=checkpoint checkpoint=kitty_received title_bytes=253 content=redacted$')"
confirmed="$(line '^sophia_firefox_primary schema=1 status=checkpoint checkpoint=confirmed title_bytes=252 content=redacted$')"
completion="$(line '^sophia_firefox_primary schema=1 status=complete checkpoints=3 selection_owner_changes=([2-9]|[1-9][0-9]+) selection_conversions=([2-9]|[1-9][0-9]+) content=redacted$')"
[[ -n "$source_armed" && -n "$kitty_received" && -n "$confirmed" && -n "$completion" ]] ||
    fail 'ordered PRIMARY checkpoints or completion evidence is missing'
(( source_armed < kitty_received && kitty_received < confirmed
    && confirmed < completion )) ||
    fail 'PRIMARY checkpoints are out of order'

firefox_owner="$(after '^sophia_firefox_m8 schema=1 status=selection_observed kind=owner_change ' "$source_armed")"
firefox_conversion="$(after '^sophia_firefox_m8 schema=1 status=selection_observed kind=conversion ' "$firefox_owner")"
firefox_notify="$(after '^.*sophia_x11_selection_delivery schema=1 stage=socket_flushed kind=notify .* synthetic=true .* property_present=true content=redacted$' "$firefox_conversion")"
[[ -n "$firefox_owner" && -n "$firefox_conversion" && -n "$firefox_notify" ]] ||
    fail 'Firefox-to-Kitty PRIMARY transfer evidence is missing'
(( source_armed < firefox_owner && firefox_owner < firefox_conversion && firefox_conversion < firefox_notify
    && firefox_notify < kitty_received )) ||
    fail 'Firefox-to-Kitty PRIMARY transfer is outside its checkpoint interval'

kitty_owner="$(after '^sophia_firefox_m8 schema=1 status=selection_observed kind=owner_change ' "$kitty_received")"
kitty_conversion="$(after '^sophia_firefox_m8 schema=1 status=selection_observed kind=conversion ' "$kitty_owner")"
kitty_notify="$(after '^.*sophia_x11_selection_delivery schema=1 stage=socket_flushed kind=notify .* synthetic=true .* property_present=true content=redacted$' "$kitty_conversion")"
[[ -n "$kitty_owner" && -n "$kitty_conversion" && -n "$kitty_notify" ]] ||
    fail 'Kitty-to-Firefox PRIMARY transfer evidence is missing'
(( kitty_owner < kitty_conversion && kitty_conversion < kitty_notify
    && kitty_notify < confirmed )) ||
    fail 'Kitty-to-Firefox PRIMARY transfer is outside its checkpoint interval'

grep -Eq '^sophia_live_session_health schema=1 status=clean .* pending_wm=0 pending_actions=0 pending_input=0 .*wm_degraded=false' "$SESSION_LOG" ||
    fail 'session health did not drain cleanly'
grep -Eq '^sophia_live_layout_health schema=1 status=clean recovery_extents=0 constraint_relayout_pending=false$' "$SESSION_LOG" ||
    fail 'temporary layout constraints did not drain'
grep -Eq '^sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 ' "$SESSION_LOG" ||
    fail 'application/frontend cleanup did not complete'
echo "Firefox PRIMARY workflow verified: $SESSION_LOG"
