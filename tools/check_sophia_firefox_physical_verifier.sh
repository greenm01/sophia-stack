#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SESSION="$ROOT_DIR/tools/fixtures/physical_firefox_session_pass.log"
GUARD="$ROOT_DIR/tools/fixtures/physical_firefox_guard_pass.log"
RECOVERY="$ROOT_DIR/tools/fixtures/physical_firefox_recovery_pass.log"
TEMP_FILE="$(mktemp)"
trap 'rm -f -- "$TEMP_FILE"' EXIT

"$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$SESSION" "$GUARD" "$RECOVERY"
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
sed '/status=dialog_ready /d' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted missing popup readiness" >&2
    exit 1
fi
sed '/layout_committed transaction=19 surfaces=5 /d' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted missing popup-open layout" >&2
    exit 1
fi
sed '/layout_committed transaction=20 surfaces=4 /d' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted missing popup-close layout" >&2
    exit 1
fi
sed 's/matched_surfaces=3/matched_surfaces=2/' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "physical Firefox verifier accepted an incomplete resize epoch" >&2
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
