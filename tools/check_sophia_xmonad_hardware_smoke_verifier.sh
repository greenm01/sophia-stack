#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SESSION="$ROOT_DIR/tools/fixtures/physical_xmonad_hardware_smoke_session_pass.log"
GUARD="$ROOT_DIR/tools/fixtures/physical_xmonad_hardware_smoke_guard_pass.log"
RECOVERY="$ROOT_DIR/tools/fixtures/physical_xmonad_hardware_smoke_recovery_pass.log"
TMP="$(mktemp /tmp/sophia-hardware-smoke-verifier.XXXXXX)"
trap 'rm -f "$TMP"' EXIT

"$ROOT_DIR/tools/verify_sophia_xmonad_hardware_smoke.sh" \
    "$SESSION" "$GUARD" "$RECOVERY"

expect_session_failure() {
    local label=$1
    if "$ROOT_DIR/tools/verify_sophia_xmonad_hardware_smoke.sh" \
        "$TMP" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
        echo "hardware-smoke verifier accepted invalid evidence: $label" >&2
        exit 1
    fi
}

sed '/status=focus_requested source=pointer/d' "$SESSION" >"$TMP"
expect_session_failure missing_pointer_focus
sed '/status=active source=resume/d' "$SESSION" >"$TMP"
expect_session_failure missing_vt_resume
sed '/status=complete output=2 /d' "$SESSION" >"$TMP"
expect_session_failure missing_output_completion
sed 's/submissions=8 retirements=7 callbacks=7/submissions=8 retirements=6 callbacks=7/' \
    "$SESSION" >"$TMP"
expect_session_failure unbalanced_output_lifetime
sed 's/submissions=8 retirements=7/submissions=invalid retirements=7/' \
    "$SESSION" >"$TMP"
expect_session_failure nonnumeric_output_lifetime
sed 's/nonzero_exports=1/nonzero_exports=0/' "$SESSION" >"$TMP"
expect_session_failure blank_output
sed 's/native_submit_failures=0/native_submit_failures=1/' "$SESSION" >"$TMP"
expect_session_failure native_failure
sed 's/termios_restored=true/termios_restored=false/' "$RECOVERY" >"$TMP"
if "$ROOT_DIR/tools/verify_sophia_xmonad_hardware_smoke.sh" \
    "$SESSION" "$GUARD" "$TMP" >/dev/null 2>&1; then
    echo "hardware-smoke verifier accepted broken TTY recovery" >&2
    exit 1
fi

echo "xmonad hardware-smoke verifier regressions passed"
