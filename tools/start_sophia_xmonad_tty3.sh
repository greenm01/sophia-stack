#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad
printf '%s\n' \
    'Normal xmonad promotion sequence after the first Kitty prompt appears:' \
    '  1. Type in Kitty; move across both outputs and click-drag a selection.' \
    '  2. Switch to another TTY and back to Sophia.' \
    '  3. Type exit in Kitty. On the empty desktop, move the pointer.' \
    '  4. Press Super+Enter and type independently in the new Kitty.' \
    '  5. Press Super+J to change focus and Super+Space to change layout.' \
    '  6. Press Super+2, type a harmless key while no window is visible, then' \
    '     press Super+1 and verify the hidden window received no input.' \
    '  7. Press Super+Shift+C to close the focused Kitty.' \
    '  8. Press Super+Shift+Q for normal logout.' \
    'Do not press Ctrl+Alt+Backspace during this normal capture.'
exec "$ROOT_DIR/tools/start_sophia_tty3.sh" "$@"
