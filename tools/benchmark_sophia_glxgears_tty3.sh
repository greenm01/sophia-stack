#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

export SOPHIA_TTY_PROFILE=standalone
export SOPHIA_STANDALONE_WORKLOAD=glxgears
export SOPHIA_STANDALONE_APP_BIN="$ROOT_DIR/tools/probes/run_bounded_glxgears.sh"
export SOPHIA_GLXGEARS_DURATION_SECONDS="${SOPHIA_GLXGEARS_DURATION_SECONDS:-20}"
export SOPHIA_GLXGEARS_WIDTH="${SOPHIA_GLXGEARS_WIDTH:-500}"
export SOPHIA_GLXGEARS_HEIGHT="${SOPHIA_GLXGEARS_HEIGHT:-500}"
[[ "$SOPHIA_GLXGEARS_DURATION_SECONDS" =~ ^[1-9][0-9]*$ ]] || {
    echo "SOPHIA_GLXGEARS_DURATION_SECONDS must be a positive integer." >&2
    exit 1
}
export SOPHIA_SESSION_WATCHDOG_SECONDS="${SOPHIA_SESSION_WATCHDOG_SECONDS:-$((SOPHIA_GLXGEARS_DURATION_SECONDS + 5))}"
export SOPHIA_SESSION_VERBOSE_TRACE="${SOPHIA_SESSION_VERBOSE_TRACE:-false}"
unset SOPHIA_STANDALONE_FRAME_COUNT

printf '%s\n' \
    "Starting bounded Sophia glxgears (${SOPHIA_GLXGEARS_DURATION_SECONDS} seconds, ${SOPHIA_GLXGEARS_WIDTH}x${SOPHIA_GLXGEARS_HEIGHT}, swap interval 1)." \
    'First checking the direct GLX/DRI3/Present path without taking over the TTY.' \
    'Move the pointer continuously over the window and confirm that all three gears remain smooth.' \
    'The application and Sophia exit automatically when the timer completes.' \
    "An independent ${SOPHIA_SESSION_WATCHDOG_SECONDS}-second deadline restores the TTY if Sophia locks." \
    'Ctrl+Alt+Backspace remains available for emergency recovery.'

cargo run --quiet --offline -p sophia-cli \
    --features native-session \
    -- x-authority-glxgears-smoke
"$ROOT_DIR/tools/start_sophia_tty3.sh" "$@"
"$ROOT_DIR/tools/report_sophia_glxgears_performance.sh"
