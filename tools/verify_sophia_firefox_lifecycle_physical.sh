#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
SESSION_LOG="${1:-${SOPHIA_HAGIA_LOG_DIR:-$STATE_HOME/sophia/hagia-session}/session.log}"

fail() { echo "focused Firefox lifecycle verification failed: $*" >&2; exit 1; }
line() { grep -nE "$1" "$SESSION_LOG" | sed -n "${2:-1}p" | cut -d: -f1 || true; }

[[ -s "$SESSION_LOG" ]] || fail "missing session log: $SESSION_LOG"
! grep -Eqi '(^Error:|panicked at|status=(failed|degraded)([[:space:]]|$))' "$SESSION_LOG" ||
    fail 'session reported a fatal or degraded result'
[[ "$(grep -Ec '^sophia_session_app schema=1 status=started id=terminal source=(startup|action)$' "$SESSION_LOG")" == 2 ]] ||
    fail 'lifecycle slice requires two Kitty processes'
[[ "$(grep -Ec '^sophia_session_app schema=1 status=started id=firefox source=action$' "$SESSION_LOG")" == 2 ]] ||
    fail 'lifecycle slice requires two Firefox launches'
[[ "$(grep -Ec '^sophia_session_app schema=1 status=exited id=firefox source=managed exit_status=exit status: 0$' "$SESSION_LOG")" == 2 ]] ||
    fail 'both Firefox processes must exit successfully'

first_start="$(line '^sophia_session_app schema=1 status=started id=firefox source=action$' 1)"
first_exit="$(line '^sophia_session_app schema=1 status=exited id=firefox source=managed exit_status=exit status: 0$' 1)"
normal_a="$(line '^sophia_firefox_m10 schema=1 status=kitty_checkpoint terminal=a checkpoint=after_normal_close ')"
normal_b="$(line '^sophia_firefox_m10 schema=1 status=kitty_checkpoint terminal=b checkpoint=after_normal_close ')"
second_start="$(line '^sophia_session_app schema=1 status=started id=firefox source=action$' 2)"
forced_close="$(line '^sophia_live_wm schema=1 status=session_action_committed .* action=CloseFocused$')"
second_exit="$(line '^sophia_session_app schema=1 status=exited id=firefox source=managed exit_status=exit status: 0$' 2)"
forced_a="$(line '^sophia_firefox_m10 schema=1 status=kitty_checkpoint terminal=a checkpoint=after_forced_close ')"
forced_b="$(line '^sophia_firefox_m10 schema=1 status=kitty_checkpoint terminal=b checkpoint=after_forced_close ')"
[[ -n "$first_start" && -n "$first_exit" && -n "$normal_a" && -n "$normal_b" && -n "$second_start" && -n "$forced_close" && -n "$second_exit" && -n "$forced_a" && -n "$forced_b" ]] ||
    fail 'lifecycle ordering evidence is incomplete'
(( first_start < first_exit && first_exit < normal_a && first_exit < normal_b
    && normal_a < second_start && normal_b < second_start
    && second_start < forced_close && forced_close < second_exit
    && second_exit < forced_a && second_exit < forced_b )) ||
    fail 'Firefox close/restart checkpoints are out of order'

grep -Eq '^sophia_firefox_lifecycle schema=1 status=complete page_ready=true kitty_checkpoints=6 content=redacted$' "$SESSION_LOG" ||
    fail 'focused lifecycle completion record is missing'
grep -Eq '^sophia_live_session_health schema=1 status=clean .* pending_wm=0 pending_actions=0 pending_input=0 .*wm_degraded=false' "$SESSION_LOG" ||
    fail 'session health did not drain cleanly'
grep -Eq '^sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 ' "$SESSION_LOG" ||
    fail 'application/frontend cleanup did not complete'
echo "focused Firefox lifecycle workflow verified: $SESSION_LOG"
