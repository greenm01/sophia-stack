#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SESSION="$ROOT_DIR/tools/fixtures/physical_firefox_session_pass.log"
GUARD="$ROOT_DIR/tools/fixtures/physical_firefox_guard_pass.log"
RECOVERY="$ROOT_DIR/tools/fixtures/physical_firefox_recovery_pass.log"
TEMP_FILE="$(mktemp)"
trap 'rm -f -- "$TEMP_FILE"' EXIT

"$ROOT_DIR/tools/verify_sophia_xmonad_input_proof_tty3.sh" \
    "$SESSION" "$GUARD" "$RECOVERY"
sed '/status=ready source=physical text=sophia/d' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_xmonad_input_proof_tty3.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "input-proof verifier accepted no exact physical text result" >&2
    exit 1
fi
sed 's/input_text_match=true/input_text_match=false/' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_xmonad_input_proof_tty3.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "input-proof verifier accepted a semantic mismatch" >&2
    exit 1
fi

echo "xmonad exact-input verifier fixtures passed"
