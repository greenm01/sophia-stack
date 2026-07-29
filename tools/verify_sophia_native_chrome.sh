#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
RUNTIME_ROOT="${XDG_RUNTIME_DIR:-/tmp}"
SESSION_LOG="${1:-$STATE_HOME/sophia/native-session/session.log}"
SEQUENCE_LOG="${2:-$RUNTIME_ROOT/sophia-native-hot-reload-${UID}/sequence.log}"

fail() {
    echo "native schema-2 chrome verification failed: $*" >&2
    exit 1
}

[[ -s "$SESSION_LOG" ]] || fail "missing session evidence: $SESSION_LOG"
[[ -s "$SEQUENCE_LOG" ]] || fail "missing sequence evidence: $SEQUENCE_LOG"
if grep -Eqi '(^Error:|panicked at|status=(failed|degraded)([[:space:]]|$))' "$SESSION_LOG"; then
    fail "session contains an error, panic, or degraded status"
fi

required_sequence=(
    '^commit=[0-9a-f]{40}$'
    '^phase=ring_baseline focus_ring_width=2 frame_width=0$'
    '^phase=ring_wide generation=2 focus_ring_width=6 frame_width=0$'
    '^phase=invalid_rejected retained_focus_ring_width=6$'
    '^phase=deletion_rejected retained_focus_ring_width=6$'
    '^phase=frame_only generation=3 focus_ring_width=0 frame_width=4$'
    '^phase=combined generation=4 focus_ring_width=2 frame_width=6$'
)
for pattern in "${required_sequence[@]}"; do
    grep -Eq "$pattern" "$SEQUENCE_LOG" || fail "missing sequence record: $pattern"
done
grep -Fq 'FAILED waiting' "$SEQUENCE_LOG" &&
    fail "the guarded sequence timed out"

awk '
    /^sophia_live_wm_policy schema=2 status=applied generation=2 .*focus_ring_width=6 .*frame_width=0 clearance=6$/ {
        phase = 1
        next
    }
    phase == 1 && /^sophia_live_resize_epoch schema=1 status=committed .* matched_surfaces=2$/ {
        phase = 2
        next
    }
    phase == 2 && /^sophia_live_compositor_chrome_set schema=1 status=composed .* eligible_surfaces=2 frames=0 focused_frames=0 unfocused_frames=0 focus_rings=1 primitives=4 clearance=6$/ {
        phase = 3
        next
    }
    phase == 3 && /^sophia_wm_config_reload schema=2 status=rejected reason=parse / {
        phase = 4
        next
    }
    phase == 4 && /^sophia_wm_config_reload schema=2 status=rejected reason=read / {
        phase = 5
        next
    }
    phase == 5 && /^sophia_live_wm_policy schema=2 status=applied generation=3 .*focus_ring_width=0 .*frame_width=4 clearance=4$/ {
        phase = 6
        next
    }
    phase == 6 && /^sophia_live_resize_epoch schema=1 status=committed .* matched_surfaces=2$/ {
        phase = 7
        next
    }
    phase == 7 && /^sophia_live_compositor_chrome_set schema=1 status=composed .* eligible_surfaces=2 frames=2 focused_frames=1 unfocused_frames=1 focus_rings=0 primitives=8 clearance=4$/ {
        phase = 8
        next
    }
    phase == 8 && /^sophia_live_wm_policy schema=2 status=applied generation=4 .*focus_ring_width=2 .*frame_width=6 clearance=6$/ {
        phase = 9
        next
    }
    phase == 9 && /^sophia_live_resize_epoch schema=1 status=committed .* matched_surfaces=2$/ {
        phase = 10
        next
    }
    phase == 10 && /^sophia_live_compositor_chrome_set schema=1 status=composed .* eligible_surfaces=2 frames=2 focused_frames=1 unfocused_frames=1 focus_rings=1 primitives=12 clearance=6$/ {
        phase = 11
    }
    END { exit !(phase == 11) }
' "$SESSION_LOG" || fail "chrome phases or atomic resize boundaries are incomplete or out of order"

grep -Eq '^sophia_live_session_health schema=1 status=clean .*pending_input=0 .*wm_degraded=false$' \
    "$SESSION_LOG" || fail "session health did not drain cleanly"
grep -Eq '^sophia_live_session schema=(15|16) status=bounded_complete .*native_submit_failures=0 .*native_retire_failures=0 .*native_cleanup_pending=false ' \
    "$SESSION_LOG" || fail "bounded native completion is missing or unhealthy"
grep -q '^sophia_live_session_cleanup schema=1 status=clean ' "$SESSION_LOG" ||
    fail "session cleanup is missing"

echo "Native schema-2 ring/frame chrome evidence passed: $SESSION_LOG"
