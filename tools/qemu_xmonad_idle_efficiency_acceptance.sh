#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$ROOT_DIR"
xmonad_bin="$(tools/resolve_xmonad_bin.sh)"
xmobar_bin="$(tools/resolve_sophia_xmobar.sh)"
SOPHIA_QEMU_IMAGE_PROFILE=rendering \
    SOPHIA_XMONAD_BIN="$xmonad_bin" \
    SOPHIA_XMOBAR_BIN="$xmobar_bin" \
    tools/build_qemu_session_initramfs.sh
SOPHIA_QEMU_SCENARIO=xmonad-idle-efficiency \
    SOPHIA_QEMU_GPU_MODE=virgl \
    SOPHIA_QEMU_MEMORY_MIB="${SOPHIA_QEMU_MEMORY_MIB:-4096}" \
    SOPHIA_QEMU_CPUS="${SOPHIA_QEMU_CPUS:-4}" \
    tools/qemu_session_harness.sh
