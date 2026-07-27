#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad

printf '%s\n' \
    'Fixed-extent recovery proof after the first Kitty prompt appears:' \
    '  1. Run: vkcube --wsi xcb' \
    '  2. Do not pass --width or --height. Allow one resize timeout.' \
    '  3. Verify the existing Kitty remains responsive and the cube appears' \
    '     as an independently placed fixed-size window.' \
    '  4. Press Ctrl+C in the launching Kitty if vkcube still owns its prompt.' \
    '  5. Press Super+Shift+Q for normal logout.' \
    '  6. Inspect ~/.local/state/sophia/xmonad-session/session.log for' \
    '     layout_timeout, recovery_configure_acknowledged, and a later' \
    '     layout_committed record.'

exec "$ROOT_DIR/tools/start_sophia_tty3.sh" "$@"
