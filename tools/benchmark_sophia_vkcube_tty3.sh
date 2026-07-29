#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_STANDALONE_FRAME_COUNT="${SOPHIA_STANDALONE_FRAME_COUNT:-900}"
export SOPHIA_SESSION_VERBOSE_TRACE=true

printf '%s\n' \
    "Starting the bounded Sophia vkcube benchmark (${SOPHIA_STANDALONE_FRAME_COUNT} frames)." \
    'The application exits automatically; do not press the logout chord.' \
    'Ctrl+Alt+Backspace remains available for emergency recovery.'

"$ROOT_DIR/tools/start_sophia_vkcube_standalone_tty3.sh" "$@"
"$ROOT_DIR/tools/report_sophia_rendering_performance.sh"
