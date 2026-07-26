#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE="$ROOT_DIR/tools/fixtures/physical_xmonad_config_reload_pass.log"
SEQUENCE="$ROOT_DIR/tools/fixtures/physical_xmonad_config_reload_sequence_pass.log"
TMP="$(mktemp /tmp/sophia-xmonad-config-verifier.XXXXXX)"
trap 'rm -f "$TMP"' EXIT

"$ROOT_DIR/tools/verify_sophia_xmonad_config_reload.sh" "$FIXTURE" "$SEQUENCE"

expect_failure() {
    local label=$1
    if "$ROOT_DIR/tools/verify_sophia_xmonad_config_reload.sh" "$TMP" "$SEQUENCE" >/dev/null 2>&1; then
        echo "external config verifier accepted invalid evidence: $label" >&2
        exit 1
    fi
}

sed 's/source=core_fallback capability=false/source=wm_policy capability=true/' "$FIXTURE" >"$TMP"
expect_failure bridge_claimed_chrome
sed '/status=pending_restart/d' "$FIXTURE" >"$TMP"
expect_failure missing_pending_restart
sed '/status=pending_restart/a sophia_live_compositor_chrome_set schema=1 status=composed generation=25 eligible_surfaces=2 frames=0 focused_frames=0 unfocused_frames=0 focus_rings=1 primitives=4 clearance=6' "$FIXTURE" >"$TMP"
expect_failure partial_candidate

echo "External xmonad config verifier regressions passed."
