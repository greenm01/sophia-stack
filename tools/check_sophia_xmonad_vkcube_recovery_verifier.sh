#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_sophia_xmonad_vkcube_recovery.sh"
PASS="$ROOT_DIR/tools/fixtures/xmonad_vkcube_recovery_pass.log"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TEMP_DIR"' EXIT

SOPHIA_VERIFY_WAIT_SECONDS=0 "$VERIFY" "$PASS" >/dev/null

sed '/transaction=634/d' "$PASS" >"$TEMP_DIR/static.log"
if SOPHIA_VERIFY_WAIT_SECONDS=0 "$VERIFY" "$TEMP_DIR/static.log" >/dev/null 2>&1; then
    echo "vkcube verifier accepted fewer than three retired Presents" >&2
    exit 1
fi

sed 's/source=cpu_buffer/source=x_pixmap/' "$PASS" >"$TEMP_DIR/wrong-source.log"
if SOPHIA_VERIFY_WAIT_SECONDS=0 "$VERIFY" "$TEMP_DIR/wrong-source.log" >/dev/null 2>&1; then
    echo "vkcube verifier accepted a source that cannot carry exact Present identity" >&2
    exit 1
fi

sed '/status=armed transaction=626/a sophia_live_visual_admission schema=1 status=committed transaction=626 surface=6291456 source=cpu_backing_snapshot' \
    "$PASS" >"$TEMP_DIR/substituted.log"
if SOPHIA_VERIFY_WAIT_SECONDS=0 "$VERIFY" "$TEMP_DIR/substituted.log" >/dev/null 2>&1; then
    echo "vkcube verifier accepted CPU backing substituted for the selected Present" >&2
    exit 1
fi

sed 's/mode=Copy ust=1000 msc=10/mode=Skip ust=0 msc=0/' "$PASS" >"$TEMP_DIR/skip.log"
if SOPHIA_VERIFY_WAIT_SECONDS=0 "$VERIFY" "$TEMP_DIR/skip.log" >/dev/null 2>&1; then
    echo "vkcube verifier accepted zero-clock Skip feedback" >&2
    exit 1
fi

echo "vkcube xmonad verifier self-test passed"
