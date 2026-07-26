#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE="$ROOT_DIR/tools/fixtures/physical_xmonad_keyboard_vt_pass.log"
TMP="$(mktemp /tmp/sophia-keyboard-vt-verifier.XXXXXX)"
trap 'rm -f "$TMP"' EXIT

"$ROOT_DIR/tools/verify_sophia_xmonad_keyboard_vt.sh" "$FIXTURE"

expect_failure() {
    local label=$1
    if "$ROOT_DIR/tools/verify_sophia_xmonad_keyboard_vt.sh" "$TMP" >/dev/null 2>&1; then
        echo "keyboard/VT verifier accepted invalid evidence: $label" >&2
        exit 1
    fi
}

sed 's/shifted_positions=21/shifted_positions=20/' "$FIXTURE" >"$TMP"
expect_failure missing_shifted_position
sed '/status=queued target=12 /d' "$FIXTURE" >"$TMP"
expect_failure missing_vt_target
sed '0,/outcome=drained/{s/outcome=drained/outcome=forced_detach_timeout/}' "$FIXTURE" >"$TMP"
expect_failure forced_detach

echo "xmonad keyboard/VT verifier regressions passed."
