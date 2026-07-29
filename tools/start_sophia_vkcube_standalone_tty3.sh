#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=standalone
export SOPHIA_SESSION_VERBOSE_TRACE=true

printf '%s\n' \
    'Standalone vkcube isolation proof:' \
    '  1. Sophia launches vkcube directly; Kitty, xmonad, and xmobar are absent.' \
    '  2. Confirm a centered natural-size window displays the spinning cube.' \
    '  3. Press Super+Shift+Q for normal logout.' \
    '  4. Press Ctrl+Alt+Backspace only for emergency recovery.' \
    '  5. Back at tty3, run:' \
    '     tools/verify_sophia_standalone_vkcube.sh'

exec "$ROOT_DIR/tools/start_sophia_tty3.sh" "$@"
