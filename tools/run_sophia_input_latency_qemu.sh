#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_GUEST="${SOPHIA_QEMU_INPUT_LATENCY_BUILD:-1}"

if [[ "$BUILD_GUEST" != "0" && "$BUILD_GUEST" != "1" ]]; then
    echo "SOPHIA_QEMU_INPUT_LATENCY_BUILD must be 0 or 1" >&2
    exit 1
fi

commit="$(git -C "$ROOT_DIR" rev-parse HEAD)"
timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
state_root="${XDG_STATE_HOME:-$HOME/.local/state}"
evidence_dir="$state_root/sophia/qemu-input-latency/$commit"
evidence="${SOPHIA_QEMU_EVIDENCE:-$evidence_dir/$timestamp.log}"

mkdir -p "$(dirname "$evidence")"

if [[ "$BUILD_GUEST" == "1" ]]; then
    "$ROOT_DIR/tools/build_qemu_session_initramfs.sh"
fi

SOPHIA_QEMU_EVIDENCE="$evidence" \
    SOPHIA_QEMU_SCENARIO=session \
    SOPHIA_QEMU_TWO_XTERM=0 \
    "$ROOT_DIR/tools/qemu_session_harness.sh"

echo "Sophia QEMU input-latency regression passed"
echo "Commit: $commit"
echo "Evidence: $evidence"
echo "This validates virtio-evdev/libinput-to-kernel-page-flip correlation; physical p95 remains a separate gate."
