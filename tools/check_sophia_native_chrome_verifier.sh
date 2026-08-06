#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE="$ROOT_DIR/tools/fixtures/physical_native_chrome_pass.log"
SEQUENCE="$ROOT_DIR/tools/fixtures/physical_native_chrome_sequence_pass.log"
TMP="$(mktemp /tmp/sophia-native-chrome-verifier.XXXXXX)"
trap 'rm -f "$TMP"' EXIT

"$ROOT_DIR/tools/verify_sophia_native_chrome.sh" "$FIXTURE" "$SEQUENCE"

expect_failure() {
    local label=$1
    if "$ROOT_DIR/tools/verify_sophia_native_chrome.sh" "$TMP" "$SEQUENCE" >/dev/null 2>&1; then
        echo "native chrome verifier accepted invalid evidence: $label" >&2
        exit 1
    fi
}

sed '/generation=3 .*focus_ring_width=0/d' "$FIXTURE" >"$TMP"
expect_failure missing_frame_policy
sed '/sophia_live_wm_chrome/d' "$FIXTURE" >"$TMP"
expect_failure missing_chrome_ownership
sed '/session_present .*target=.*_4 /d' "$FIXTURE" >"$TMP"
expect_failure missing_frame_presentation
sed '/frames=2 focused_frames=1 unfocused_frames=1 focus_rings=1/d' "$FIXTURE" >"$TMP"
expect_failure missing_combined_composition
sed '/visual_committed transaction=21 surface=2/d' "$FIXTURE" >"$TMP"
expect_failure missing_second_surface_retirement
sed 's/transaction=21 surface=2/transaction=21 surface=1/g' "$FIXTURE" >"$TMP"
expect_failure duplicate_surface_retirement
sed 's/native_cleanup_pending=false/native_cleanup_pending=true/' "$FIXTURE" >"$TMP"
expect_failure cleanup_debt

echo "Native schema-2 chrome verifier regressions passed."
