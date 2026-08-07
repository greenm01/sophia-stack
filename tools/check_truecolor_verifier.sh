#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_installed_truecolor_session.sh"
SESSION="$ROOT_DIR/tools/fixtures/installed_truecolor_session_pass.log"
GUARD="$ROOT_DIR/tools/fixtures/installed_truecolor_input_guard_pass.log"
RECOVERY="$ROOT_DIR/tools/fixtures/installed_truecolor_recovery_pass.log"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

"$VERIFY" "$SESSION" "$GUARD" "$RECOVERY" >/dev/null
sed 's/region_red_pixels=9600/region_red_pixels=19200/' "$SESSION" >"$tmp/swapped.log"
if "$VERIFY" "$tmp/swapped.log" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "TrueColor verifier accepted a swapped palette channel" >&2
    exit 1
fi
sed 's/source=dma_buf/source=cpu_buffer/' "$SESSION" >"$tmp/no-kitty-dmabuf.log"
if "$VERIFY" "$tmp/no-kitty-dmabuf.log" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "TrueColor verifier accepted a non-DMA-BUF Kitty path" >&2
    exit 1
fi
sed '0,/composition=final source_stage=cpu/s//composition=intermediate source_stage=cpu/' \
    "$SESSION" >"$tmp/not-final.log"
if "$VERIFY" "$tmp/not-final.log" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "TrueColor verifier accepted pre-overdraw palette evidence" >&2
    exit 1
fi
sed '0,/submitted output=1 submission=7/s//submitted output=2 submission=7/' \
    "$SESSION" >"$tmp/wrong-output.log"
if "$VERIFY" "$tmp/wrong-output.log" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "TrueColor verifier accepted a palette outside the primary composition target" >&2
    exit 1
fi

echo "TrueColor verifier regression passed"
