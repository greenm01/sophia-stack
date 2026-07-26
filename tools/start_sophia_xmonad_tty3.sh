#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad
printf '%s\n' \
    'Normal xmonad promotion sequence after the first Kitty prompt appears:' \
    '  1. Type repeat-test-abcdef. Hold Left until the cursor moves several' \
    '     places, then hold Backspace until several characters disappear.' \
    '     Clear the line, type a unique word, click-drag it, and press Ctrl+Shift+C.' \
    '  2. Press Super+3, then Super+Enter. In the new Kitty press Ctrl+Shift+V,' \
    '     verify the exact word, select it, and press Ctrl+Shift+C.' \
    '  3. Press Super+1, then Super+Enter. Press Ctrl+Shift+V and verify the' \
    '     workspace-3 copy appears unchanged. Exit this peer, return to' \
    '     workspace 3, exit its peer, and return to workspace 1.' \
    '  4. Move the pointer hard against every edge of both outputs; it must' \
    '     remain visible and reverse direction immediately.' \
    '  5. Switch to another TTY and back to Sophia.' \
    '  6. Type exit in the startup Kitty. On the empty desktop, move the pointer.' \
    '  7. Press Super+Enter twice and type independently in both new Kittys.' \
    '  8. Press Super+J to change focus and Super+Space to change layout.' \
    '  9. Press Super+Shift+C to close the focused Kitty.' \
    ' 10. Press Super+2, type a harmless key while no window is visible, then' \
    '     press Super+3 and confirm it is also empty.' \
    ' 11. Press Super+1; verify the prior focus returns and the hidden window' \
    '     received no input.' \
    ' 12. Press Super+Shift+C to close the focused Kitty.' \
    ' 13. Press Super+Shift+Q for normal logout.' \
    'Do not press Ctrl+Alt+Backspace during this normal capture.'
exec "$ROOT_DIR/tools/start_sophia_tty3.sh" "$@"
