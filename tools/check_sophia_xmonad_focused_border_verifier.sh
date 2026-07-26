#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
verifier="$ROOT_DIR/tools/verify_sophia_xmonad_focused_border.sh"
fixture="$ROOT_DIR/tools/fixtures/physical_xmonad_focused_border_pass.log"
tmp="$(mktemp /tmp/sophia-focused-border-verifier.XXXXXX)"
trap 'rm -f "$tmp"' EXIT

"$verifier" "$fixture"

expect_failure() {
    local label=$1
    if "$verifier" "$tmp" >/dev/null 2>&1; then
        echo "focused-border verifier accepted invalid evidence: $label" >&2
        exit 1
    fi
}

sed '/surface=20 generation=200 primitives=4/d' "$fixture" >"$tmp"
expect_failure missing_second_focus_border

sed 's/surface=10 generation=101 primitives=4/surface=10 generation=100 primitives=4/g' \
    "$fixture" >"$tmp"
expect_failure missing_resize_generation

sed '/visible_surfaces=0 focus=none/d' "$fixture" >"$tmp"
expect_failure missing_workspace_hide

sed '/status=active source=resume/d' "$fixture" >"$tmp"
expect_failure missing_vt_resume

sed 's/native_mixed_exports=8/native_mixed_exports=0/' "$fixture" >"$tmp"
expect_failure missing_mixed_composition

cp "$fixture" "$tmp"
printf '%s\n' 'sophia_live_session schema=15 status=failed reason=test' >>"$tmp"
expect_failure failure_marker

echo "Physical xmonad focused-border verifier regressions passed."
