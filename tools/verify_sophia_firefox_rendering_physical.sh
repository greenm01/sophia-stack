#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
SESSION_LOG="${1:-${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}/session.log}"

fail() { echo "Firefox rendering canary verification failed: $*" >&2; exit 1; }
line() { grep -nE "$1" "$SESSION_LOG" | sed -n "${2:-1}p" | cut -d: -f1 || true; }

[[ -s "$SESSION_LOG" ]] || fail "missing session log: $SESSION_LOG"
! grep -Eqi '(^Error:|panicked at|status=(failed|degraded)([[:space:]]|$))' "$SESSION_LOG" ||
    fail 'session reported a fatal or degraded result'
[[ "$(grep -Ec '^sophia_session_app schema=1 status=started id=terminal source=startup$' "$SESSION_LOG")" == 1 ]] ||
    fail 'rendering canary requires one startup Kitty'
[[ "$(grep -Ec '^sophia_session_app schema=1 status=started id=firefox source=action$' "$SESSION_LOG")" == 1 ]] ||
    fail 'rendering canary requires one Firefox action launch'

firefox_start="$(line '^sophia_session_app schema=1 status=started id=firefox source=action$')"
surface_record="$(grep -E '^sophia_session_app schema=2 status=surface_observed source=action transaction=[0-9]+ surface=[0-9]+$' "$SESSION_LOG" | head -n 1 || true)"
[[ -n "$surface_record" ]] || fail 'Firefox action surface was not observed'
firefox_surface="${surface_record##*surface=}"
surface_observed="$(line "^sophia_session_app schema=2 status=surface_observed source=action transaction=[0-9]+ surface=${firefox_surface}$")"
page_ready="$(line '^sophia_firefox_rendering schema=1 status=page_ready title_bytes=249 content=redacted$')"
full_retirement="$(line "^sophia_live_session_present schema=2 status=retired transaction=[0-9]+ surface=${firefox_surface} source=1276x1422 target=1276x1422_2_16 clip=1276x1422_2_16 unit_scale=true$")"
completion="$(line '^sophia_firefox_rendering schema=1 status=complete page_ready=true recovery_extents=0 content=redacted$')"
[[ -n "$page_ready" ]] || fail 'the isolated Firefox document never became ready'
[[ -n "$full_retirement" ]] || fail 'Firefox never retired a complete full-height left-column frame'
[[ -n "$completion" ]] || fail 'rendering canary completion record is missing'
(( firefox_start < surface_observed && surface_observed < page_ready
    && surface_observed < full_retirement && page_ready < completion
    && full_retirement < completion )) ||
    fail 'Firefox rendering evidence is out of order'

restart_count="$(awk -v start="$firefox_start" 'NR > start && /^sophia_live_wm schema=1 status=restarted / { count += 1 } END { print count + 0 }' "$SESSION_LOG")"
(( restart_count <= 1 )) || fail "Firefox admission required $restart_count WM restarts"

retained_pattern="^sophia_live_resize_epoch schema=2 status=recovery_extent_retained surface=${firefox_surface} reason=standing_target_unmet target=1276x1422 content=redacted$"
retained_count="$(grep -Ec "$retained_pattern" "$SESSION_LOG" || true)"
surface_retained_count="$(grep -Ec "^sophia_live_resize_epoch schema=2 status=recovery_extent_retained surface=${firefox_surface} " "$SESSION_LOG" || true)"
cleared_pattern="^sophia_live_resize_epoch schema=2 status=recovery_extent_cleared surface=${firefox_surface} reason=standing_target_presented$"
standing_target_pattern="^sophia_live_resize_epoch schema=3 status=visual_committed transaction=[0-9]+ surface=${firefox_surface} width=1276 height=1422 source=standing_target_recovery$"
cleared_count="$(grep -Ec "$cleared_pattern" "$SESSION_LOG" || true)"
standing_target_count="$(grep -Ec "$standing_target_pattern" "$SESSION_LOG" || true)"
(( surface_retained_count == retained_count )) ||
    fail 'Firefox retained an unexpected fallback recovery extent'
(( retained_count <= 1 )) || fail 'Firefox retained its fallback extent more than once'
if (( retained_count == 1 )); then
    retained="$(line "$retained_pattern")"
    (( cleared_count == 1 && standing_target_count == 1 )) ||
        fail 'fallback recovery did not clear through the standing target'
    cleared="$(line "$cleared_pattern")"
    standing_target="$(line "$standing_target_pattern")"
    (( retained < cleared && cleared <= standing_target && standing_target < full_retirement )) ||
        fail 'fallback recovery and exact retirement are out of order'
elif (( cleared_count != 0 || standing_target_count != 0 )); then
    fail 'standing-target recovery appeared without a retained fallback extent'
fi

grep -Eq '^sophia_live_session_health schema=1 status=clean .* pending_wm=0 pending_actions=0 pending_input=0 .*wm_degraded=false' "$SESSION_LOG" ||
    fail 'session health did not drain cleanly'
grep -Eq '^sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 ' "$SESSION_LOG" ||
    fail 'application/frontend cleanup did not complete'
echo "Firefox full-height rendering canary verified: $SESSION_LOG"
