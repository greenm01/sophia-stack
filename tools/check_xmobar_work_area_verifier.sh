#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_sophia_xmobar_work_area_session.sh"
FIXTURE="$ROOT_DIR/tools/fixtures/installed_xmobar_work_area_pass.log"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

"$VERIFY" "$FIXTURE" >/dev/null
sed '0,/pixels=35840/s//pixels=35839/' "$FIXTURE" >"$tmp/wrong-repaint.log"
if "$VERIFY" "$tmp/wrong-repaint.log" >/dev/null 2>&1; then
    echo "xmobar verifier accepted a bar repaint with the wrong extent" >&2
    exit 1
fi
sed '0,/work=2560x1426_0_14/s//work=2560x1440_0_0/' "$FIXTURE" >"$tmp/wrong-work-area.log"
if "$VERIFY" "$tmp/wrong-work-area.log" >/dev/null 2>&1; then
    echo "xmobar verifier accepted an unreduced primary work area" >&2
    exit 1
fi

echo "xmobar/work-area verifier regression passed"
