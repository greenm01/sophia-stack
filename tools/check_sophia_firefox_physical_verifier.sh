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

echo "physical Firefox verifier fixtures passed"
