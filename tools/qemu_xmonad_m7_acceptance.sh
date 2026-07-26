#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$ROOT_DIR"
xmonad_bin="$(tools/resolve_xmonad_bin.sh)"
SOPHIA_XMONAD_BIN="$xmonad_bin" tools/build_qemu_session_initramfs.sh
SOPHIA_QEMU_SCENARIO=xmonad-m7 tools/qemu_session_harness.sh
tools/verify_qemu_xmonad_m7_evidence.sh "${SOPHIA_QEMU_EVIDENCE:-/tmp/sophia-qemu-xmonad-m7.log}"

echo "Retained Milestone 7 xmonad acceptance passed."
