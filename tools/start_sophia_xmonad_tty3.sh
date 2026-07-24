#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad
printf '%s\n' \
    'Normal xmonad promotion sequence after the first Kitty prompt appears:' \
    '  1. Type in Kitty; move across both outputs and click-drag a selection.' \
    '  2. Press Super+Enter and type independently in the second Kitty.' \
    '  3. Press Super+J to change focus and Super+Space to change layout.' \
    '  4. Press Super+2, then Super+1, verifying hidden windows receive no input.' \
    '  5. Press Super+Shift+C to close a focused Kitty.' \
    '  6. Press Super+Shift+Q for normal logout.' \
    'Do not press Ctrl+Alt+Backspace during this normal capture.'
exec "$ROOT_DIR/tools/start_sophia_tty3.sh" "$@"
