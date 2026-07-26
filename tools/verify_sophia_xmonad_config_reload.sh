#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
RUNTIME_ROOT="${XDG_RUNTIME_DIR:-/tmp}"
SESSION_LOG="${1:-$STATE_HOME/sophia/xmonad-session/session.log}"
SEQUENCE_LOG="${2:-$RUNTIME_ROOT/sophia-xmonad-config-reload-${UID}/sequence.log}"

fail() {
    echo "external xmonad config verification failed: $*" >&2
    exit 1
}

[[ -s "$SESSION_LOG" ]] || fail "missing session evidence: $SESSION_LOG"
[[ -s "$SEQUENCE_LOG" ]] || fail "missing sequence evidence: $SEQUENCE_LOG"
grep -Eq '^commit=[0-9a-f]{40}$' "$SEQUENCE_LOG" || fail "sequence commit is missing"
for record in \
    'phase=external_baseline source=core_fallback focus_ring_width=2' \
    'phase=core_live_applied generation=2 focus_ring_width=4' \
    'phase=pending_restart_retained candidate_width=6 active_width=4' \
    'phase=invalid_rejected active_width=4' \
    'phase=core_restored generation=3 focus_ring_width=2'; do
    grep -Fxq "$record" "$SEQUENCE_LOG" || fail "missing sequence record: $record"
done
grep -Fq 'FAILED ' "$SEQUENCE_LOG" && fail "guarded sequence reported failure"
if grep -Eqi '(^Error:|panicked at|status=(failed|degraded)([[:space:]]|$))' "$SESSION_LOG"; then
    fail "session contains an error, panic, or degraded status"
fi

grep -q '^sophia_live_wm_chrome schema=1 status=negotiated source=core_fallback capability=false clearance=2$' \
    "$SESSION_LOG" || fail "external bridge claimed compositor chrome capability"
grep -q '^sophia_live_wm schema=1 status=ready adapter=external ' "$SESSION_LOG" ||
    fail "external WM transport was not active"
if grep -q '^sophia_wm_config_reload ' "$SESSION_LOG"; then
    fail "external xmonad unexpectedly consumed native wm.kdl policy"
fi

awk '
    /^sophia_config_reload schema=1 status=applied generation=2 .*chrome_changed=true / { phase = 1; next }
    phase == 1 && /^sophia_live_resize_epoch schema=1 status=committed .* matched_surfaces=2$/ { phase = 2; next }
    phase == 2 && /^sophia_live_compositor_chrome_set schema=1 status=composed .* eligible_surfaces=2 .* focus_rings=1 primitives=4 clearance=4$/ { phase = 3; next }
    phase == 3 && /^sophia_config_reload schema=1 status=pending_restart generation=3 / { phase = 4; next }
    phase == 4 && /^sophia_config_reload schema=1 status=rejected reason=parse / { phase = 5; next }
    phase == 5 && /^sophia_config_reload schema=1 status=applied generation=3 .*chrome_changed=true / { phase = 6; next }
    phase == 6 && /^sophia_live_resize_epoch schema=1 status=committed .* matched_surfaces=2$/ { phase = 7; next }
    phase == 7 && /^sophia_live_compositor_chrome_set schema=1 status=composed .* eligible_surfaces=2 .* focus_rings=1 primitives=4 clearance=2$/ { phase = 8 }
    phase >= 4 && phase < 6 && /chrome_set .* clearance=6$/ { bad = 1 }
    END { exit !(phase == 8 && !bad) }
' "$SESSION_LOG" || fail "core fallback phases are incomplete, partial, or out of order"

grep -Eq '^sophia_live_session_health schema=1 status=clean .*pending_input=0 .*wm_degraded=false$' \
    "$SESSION_LOG" || fail "session health did not drain"
grep -q '^sophia_live_session_cleanup schema=1 status=clean ' "$SESSION_LOG" ||
    fail "session cleanup is missing"

echo "External xmonad core-config isolation evidence passed: $SESSION_LOG"
