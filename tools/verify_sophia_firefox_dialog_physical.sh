#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
SESSION_LOG="${1:-${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}/session.log}"

fail() { echo "Firefox dialog canary verification failed: $*" >&2; exit 1; }
line() { grep -nE "$1" "$SESSION_LOG" | sed -n "${2:-1}p" | cut -d: -f1 || true; }
after() { grep -nE "$1" "$SESSION_LOG" | cut -d: -f1 | awk -v n="$2" '$1 > n { print; exit }' || true; }

[[ -s "$SESSION_LOG" ]] || fail "missing session log: $SESSION_LOG"
! grep -Eqi '(^Error:|panicked at|status=(failed|degraded)([[:space:]]|$)|Gdk-CRITICAL|gdk_window_thaw_toplevel_updates)' "$SESSION_LOG" ||
    fail 'session reported a fatal, degraded, or GDK freeze result'
[[ "$(grep -Ec '^sophia_session_app schema=1 status=started id=terminal source=startup$' "$SESSION_LOG")" == 1 ]] ||
    fail 'dialog canary requires one startup Kitty'
[[ "$(grep -Ec '^sophia_session_app schema=1 status=started id=firefox source=action$' "$SESSION_LOG")" == 1 ]] ||
    fail 'dialog canary requires one Firefox action launch'
[[ "$(grep -Ec '^sophia_session_app schema=2 status=started id=firefox source=action transaction=[0-9]+$' "$SESSION_LOG")" == 1 ]] ||
    fail 'dialog canary requires one correlated Firefox launch transaction'

firefox_start="$(line '^sophia_session_app schema=1 status=started id=firefox source=action$')"
firefox_start_record="$(grep -E '^sophia_session_app schema=2 status=started id=firefox source=action transaction=[0-9]+$' "$SESSION_LOG")"
firefox_transaction="${firefox_start_record##*transaction=}"
[[ -n "$firefox_transaction" ]] || fail 'Firefox launch transaction is missing'
[[ "$(grep -Ec "^sophia_session_app schema=2 status=surface_observed source=action transaction=${firefox_transaction} surface=[0-9]+$" "$SESSION_LOG")" == 1 ]] ||
    fail 'dialog canary requires one surface from the Firefox launch transaction'
surface_record="$(grep -E "^sophia_session_app schema=2 status=surface_observed source=action transaction=${firefox_transaction} surface=[0-9]+$" "$SESSION_LOG")"
firefox_surface="${surface_record##*surface=}"
surface_observed="$(line "^sophia_session_app schema=2 status=surface_observed source=action transaction=${firefox_transaction} surface=${firefox_surface}$")"
page_ready="$(line '^sophia_firefox_dialog schema=1 status=checkpoint checkpoint=page_ready title_bytes=245 content=redacted$')"
modal_ready="$(line '^sophia_firefox_dialog schema=1 status=checkpoint checkpoint=modal_ready title_bytes=246 content=redacted$')"
confirmed="$(line '^sophia_firefox_dialog schema=1 status=checkpoint checkpoint=confirmed title_bytes=247 content=redacted$')"
completion="$(line '^sophia_firefox_dialog schema=1 status=complete checkpoints=3 pointer_buttons=([4-9]|[1-9][0-9]+) recovery_extents=0 content=redacted$')"
[[ -n "$page_ready" && -n "$modal_ready" && -n "$confirmed" && -n "$completion" ]] ||
    fail 'ordered page, modal, confirmation, or completion evidence is missing'

full_frame_pattern="^sophia_live_session_present schema=2 status=retired transaction=[0-9]+ surface=${firefox_surface} source=1276x1422 target=1276x1422_2_16 clip=1276x1422_2_16 unit_scale=true$"
page_frame="$(after "$full_frame_pattern" "$page_ready")"
open_click="$(after '^sophia_firefox_dialog schema=1 status=pointer_batch routed=[1-9][0-9]* total=[1-9][0-9]* content=redacted$' "$page_frame")"
modal_frame="$(after "$full_frame_pattern" "$modal_ready")"
confirm_click="$(after '^sophia_firefox_dialog schema=1 status=pointer_batch routed=[1-9][0-9]* total=[1-9][0-9]* content=redacted$' "$modal_frame")"
confirmed_frame="$(after "$full_frame_pattern" "$confirmed")"
[[ -n "$page_frame" && -n "$open_click" && -n "$modal_frame" && -n "$confirm_click" && -n "$confirmed_frame" ]] ||
    fail 'page, modal, confirmed frame, or routed click evidence is missing'
(( firefox_start < surface_observed && surface_observed < page_ready
    && page_ready < page_frame && page_frame < open_click && open_click < modal_ready
    && modal_ready < modal_frame && modal_frame < confirm_click && confirm_click < confirmed
    && confirmed < confirmed_frame && confirmed_frame < completion )) ||
    fail 'Firefox dialog checkpoints and frame retirements are out of order'

restart_count="$(awk -v first="$firefox_start" -v last="$page_frame" 'NR > first && NR < last && /^sophia_live_wm schema=1 status=restarted / { count += 1 } END { print count + 0 }' "$SESSION_LOG")"
(( restart_count <= 1 )) || fail "Firefox admission required $restart_count WM restarts"
if awk -v first="$page_frame" -v last="$completion" 'NR > first && NR < last && /^sophia_live_wm schema=1 status=restarted / { found=1 } END { exit !found }' "$SESSION_LOG"; then
    fail 'opening or confirming the DOM modal restarted the WM'
fi
if awk -v first="$page_frame" -v last="$completion" 'NR > first && NR < last && (/status=layout_timeout/ || /sophia_live_surface_admission .* status=frontend_admitted/ || /status=recovery_extent_retained /) { found=1 } END { exit !found }' "$SESSION_LOG"; then
    fail 'the DOM modal created a new toplevel or disturbed stable admission'
fi

retained_pattern="^sophia_live_resize_epoch schema=2 status=recovery_extent_retained surface=${firefox_surface} reason=standing_target_unmet target=1276x1422 content=redacted$"
cleared_pattern="^sophia_live_resize_epoch schema=2 status=recovery_extent_cleared surface=${firefox_surface} reason=standing_target_presented$"
standing_pattern="^sophia_live_resize_epoch schema=3 status=visual_committed transaction=[0-9]+ surface=${firefox_surface} width=1276 height=1422 source=standing_target_recovery$"
retained_count="$(grep -Ec "$retained_pattern" "$SESSION_LOG" || true)"
surface_retained_count="$(grep -Ec "^sophia_live_resize_epoch schema=2 status=recovery_extent_retained surface=${firefox_surface} " "$SESSION_LOG" || true)"
cleared_count="$(grep -Ec "$cleared_pattern" "$SESSION_LOG" || true)"
standing_count="$(grep -Ec "$standing_pattern" "$SESSION_LOG" || true)"
(( surface_retained_count == retained_count && retained_count <= 1 )) ||
    fail 'Firefox retained an unexpected or repeated fallback extent'
if (( retained_count == 1 )); then
    (( cleared_count == 1 && standing_count == 1 )) ||
        fail 'fallback recovery did not clear through the standing target'
    retained="$(line "$retained_pattern")"
    cleared="$(line "$cleared_pattern")"
    standing="$(line "$standing_pattern")"
    (( retained < cleared && cleared <= standing && standing < page_frame )) ||
        fail 'fallback recovery and the ready page frame are out of order'
elif (( cleared_count != 0 || standing_count != 0 )); then
    fail 'standing-target recovery appeared without a retained fallback extent'
fi

grep -Eq '^sophia_live_session_health schema=1 status=clean .* pending_wm=0 pending_actions=0 pending_input=0 .*wm_degraded=false' "$SESSION_LOG" ||
    fail 'session health did not drain cleanly'
grep -Eq '^sophia_live_layout_health schema=1 status=clean recovery_extents=0 constraint_relayout_pending=false$' "$SESSION_LOG" ||
    fail 'temporary layout constraints did not drain'
grep -Eq '^sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 ' "$SESSION_LOG" ||
    fail 'application/frontend cleanup did not complete'
echo "Firefox dialog canary verified: $SESSION_LOG"
