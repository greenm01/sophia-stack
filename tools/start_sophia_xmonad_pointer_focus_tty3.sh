#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad

cat <<'INSTRUCTIONS'
Plain-click and click-drag focus proof:
  1. Wait for the startup Kitty prompt, then press Super+Enter and wait for the
     second Kitty prompt.
  2. Move the pointer into one tile without clicking. Press Super+J as needed
     until the focused border is visibly on the other tile.
  3. Move into the unfocused tile without pressing a button. Click and release
     the unmodified primary button without intentionally dragging.
  4. Type "click-focus" in that tile. The focused border and text must move
     there; the previously focused Kitty must not receive the text.
  5. Press Super+J as needed until the focused border moves away from that same
     tile.
  6. In the now-unfocused tile, press and hold the unmodified primary button,
     drag far enough to select visible text, and release.
  7. Type "drag-focus" in that tile. The border and text must return there,
     selection must have tracked the drag, and the other Kitty must not receive
     the text.
  8. Press Super+Shift+Q for normal logout. Do not use Ctrl+Alt+Backspace.

After normal logout this wrapper automatically verifies both ordered focus
handoffs and their following keys from the retained session log.
INSTRUCTIONS

set +e
"$ROOT_DIR/tools/start_sophia_tty3.sh" "$@"
session_status=$?
set -e
if ((session_status != 0)); then
    echo "Sophia pointer-focus session exited with status $session_status." >&2
    exit "$session_status"
fi

"$ROOT_DIR/tools/verify_sophia_xmonad_pointer_focus_pair.sh"
