#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_STANDALONE_FRAME_COUNT="${SOPHIA_STANDALONE_FRAME_COUNT:-900}"
export SOPHIA_STANDALONE_WIDTH="${SOPHIA_STANDALONE_WIDTH:-500}"
export SOPHIA_STANDALONE_HEIGHT="${SOPHIA_STANDALONE_HEIGHT:-500}"
export SOPHIA_STANDALONE_PRESENT_MODE="${SOPHIA_STANDALONE_PRESENT_MODE:-2}"
export SOPHIA_SESSION_VERBOSE_TRACE=true

[[ "$SOPHIA_STANDALONE_PRESENT_MODE" == 2 ]] || {
    echo "The retained parity workload requires Vulkan FIFO present mode 2." >&2
    exit 1
}

printf '%s\n' \
    "Starting the bounded Sophia vkcube benchmark (${SOPHIA_STANDALONE_FRAME_COUNT} frames, ${SOPHIA_STANDALONE_WIDTH}x${SOPHIA_STANDALONE_HEIGHT}, FIFO)." \
    'The application exits automatically; do not press the logout chord.' \
    'Ctrl+Alt+Backspace remains available for emergency recovery.'

"$ROOT_DIR/tools/start_sophia_vkcube_standalone_tty3.sh" "$@"
"$ROOT_DIR/tools/report_sophia_rendering_performance.sh"
