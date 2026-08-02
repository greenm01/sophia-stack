#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SESSION="$ROOT_DIR/tools/fixtures/physical_firefox_session_pass.log"
GUARD="$ROOT_DIR/tools/fixtures/physical_firefox_guard_pass.log"
RECOVERY="$ROOT_DIR/tools/fixtures/physical_firefox_recovery_pass.log"
TEMP_FILE="$(mktemp)"
RECOVERY_SESSION="$(mktemp)"
trap 'rm -f -- "$TEMP_FILE" "$RECOVERY_SESSION"' EXIT

"$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$SESSION" "$GUARD" "$RECOVERY"
awk '
    { print }
    /status=surface_observed source=action transaction=15 surface=4$/ {
        print "sophia_live_wm schema=1 status=layout_timeout transaction=15 preserved_layout=true"
        print "sophia_live_wm schema=1 status=restarted restarts=1 preserved_layout=true"
        print "sophia_live_wm schema=4 status=reseed_queued phase=committed_layout request=relayout"
        print "sophia_live_wm schema=4 status=reseed_queued phase=pending_admission request=manage surface=4"
        print "sophia_live_wm schema=1 status=layout_committed transaction=16 surfaces=3 moved_surfaces=0 configure_deliveries=2 outcome=Committed"
    }
' "$SESSION" | sed 's/wm_restarts=0/wm_restarts=1/' >"$RECOVERY_SESSION"
"$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$RECOVERY_SESSION" "$GUARD" "$RECOVERY"
sed '/status=restarted restarts=1 /a sophia_live_wm schema=1 status=restarted restarts=2 preserved_layout=true' \
    "$RECOVERY_SESSION" | sed 's/wm_restarts=1/wm_restarts=2/' >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted a repeated admission restart" >&2
    exit 1
fi
sed '/phase=pending_admission request=manage surface=4/a sophia_live_visual_admission schema=1 status=armed transaction=150 surface=4' \
    "$RECOVERY_SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted phase-one candidate consumption" >&2
    exit 1
fi
sed '/stage=primary /d' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted a missing PRIMARY stage" >&2
    exit 1
fi
grep -Fv 'status=axis_batch' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted missing scroll routing" >&2
    exit 1
fi
awk '
    /stage=primary / { inside=1 }
    inside && /status=axis_batch/ && !removed { removed=1; next }
    { print }
' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted only one post-PRIMARY scroll packet" >&2
    exit 1
fi
sed '/status=navigation_ready /d' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted missing navigation readiness" >&2
    exit 1
fi
sed 's/physical_action_committed action=3/physical_action_committed action=1/' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted the wrong resize action" >&2
    exit 1
fi
sed '/window=4 focused=false core_selected=true xi2_selected=true/d' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted missing XI2 FocusOut" >&2
    exit 1
fi
sed '/window=4 focused=true core_selected=true xi2_selected=true/d' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted missing XI2 FocusIn" >&2
    exit 1
fi
sed 's/index: 2, generation: 1/index: 4, generation: 1/' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted no focus transition away" >&2
    exit 1
fi
sed 's/index: 4, generation: 1/index: 2, generation: 1/' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted no focus return" >&2
    exit 1
fi
sed '/status=dialog_ready /d' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted missing modal readiness" >&2
    exit 1
fi
sed '/status=dialog_ready /a sophia_live_wm schema=1 status=layout_timeout transaction=19 preserved_layout=true' \
    "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted a modal interaction layout timeout" >&2
    exit 1
fi
sed '/status=dialog_ready /a sophia_live_wm schema=1 status=restarted attempt=1' \
    "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted a modal interaction WM restart" >&2
    exit 1
fi
sed '/status=dialog_ready /a Gdk-CRITICAL **: gdk_window_thaw_toplevel_updates: assertion failed' \
    "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted a GDK thaw underflow" >&2
    exit 1
fi
sed 's/matched_surfaces=3/matched_surfaces=2/' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted an incomplete resize epoch" >&2
    exit 1
fi
sed '/status=visual_committed /d' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted resize pixels without retirement" >&2
    exit 1
fi
sed '/status=visual_committed /i sophia_live_native_retirement schema=1 status=settled outcome=RejectedStaleSurface' \
    "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted a stale resize Present" >&2
    exit 1
fi
grep -Fv 'sophia_live_layout_health' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted missing layout health" >&2
    exit 1
fi
awk '
    /status=started id=firefox source=action/ {
        seen++
        if (seen == 2) next
    }
    { print }
' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted one Firefox launch" >&2
    exit 1
fi
sed '/terminal=a checkpoint=after_normal_close /d' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted missing Kitty retention evidence" >&2
    exit 1
fi
grep -Fv 'action=CloseFocused' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted missing forced close" >&2
    exit 1
fi
grep -Fv 'status=clean app_groups=0 frontend_workers=0' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted missing cleanup" >&2
    exit 1
fi
awk '
    { print }
    END { print "sophia_live_session_pointer schema=5 status=focus_handoff_dropped reason=capacity count=1" }
' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted a dropped pointer focus handoff" >&2
    exit 1
fi

echo "physical Firefox verifier fixtures passed"
