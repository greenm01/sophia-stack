#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad
printf '%s\n' \
    'Emergency recovery capture:' \
    '  1. Arm the independent guard when prompted.' \
    '  2. Wait for the initial Kitty prompt and confirm keyboard/pointer input.' \
    '  3. Press and release Ctrl+Alt+Backspace once.' \
    '  4. Wait for automatic return to this TTY; do not switch TTYs manually.' \
    '  5. Run: tools/verify_sophia_xmonad_emergency_tty3.sh'
exec "$ROOT_DIR/tools/start_sophia_tty3.sh" "$@"
