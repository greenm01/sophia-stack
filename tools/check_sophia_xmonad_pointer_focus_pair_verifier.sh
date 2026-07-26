#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
verify="$ROOT_DIR/tools/verify_sophia_xmonad_pointer_focus_pair.sh"
fixture="$ROOT_DIR/tools/fixtures/physical_xmonad_pointer_focus_pair_pass.log"
tmp="$(mktemp /tmp/sophia-pointer-focus-pair.XXXXXX)"
trap 'rm -f "$tmp"' EXIT

"$verify" "$fixture"

expect_failure() {
    local label=$1
    if "$verify" "$tmp" >/dev/null 2>&1; then
        echo "pointer-focus pair verifier accepted invalid evidence: $label" >&2
        exit 1
    fi
}

sed '/surface=1/d' "$fixture" >"$tmp"
expect_failure missing_drag_sequence

sed 's/surface=1 count=3/surface=1 count=2/' "$fixture" >"$tmp"
expect_failure drag_without_motion

sed 's/focused_key_routed surface=1/focused_key_routed surface=2/' "$fixture" >"$tmp"
expect_failure drag_key_wrong_surface

cp "$fixture" "$tmp"
printf '%s\n' \
    'sophia_live_session_pointer schema=5 status=focus_handoff_dropped reason=timeout' \
    >>"$tmp"
expect_failure dropped_handoff

echo "xmonad click/drag focus verifier self-check passed"
