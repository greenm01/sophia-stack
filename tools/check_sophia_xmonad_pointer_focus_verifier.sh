#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_sophia_xmonad_pointer_focus.sh"
FIXTURES="$ROOT_DIR/tools/fixtures"

"$VERIFY" "$FIXTURES/physical_xmonad_pointer_focus_pass.log"

for rejected in \
    physical_xmonad_pointer_focus_early_release.log \
    physical_xmonad_pointer_focus_dropped.log; do
    if "$VERIFY" "$FIXTURES/$rejected" >/dev/null 2>&1; then
        echo "pointer-focus verifier accepted invalid evidence: $rejected" >&2
        exit 1
    fi
done

echo "xmonad pointer-focus verifier self-check passed"
