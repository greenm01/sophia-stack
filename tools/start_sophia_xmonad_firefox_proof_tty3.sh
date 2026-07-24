#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad
cat <<'INSTRUCTIONS'
Physical Firefox proof:
  1. Super+Enter: launch a second Kitty; type a harmless command.
  2. Super+F: launch the offline Firefox proof page.
  3. Type sophia.
  4. Ctrl+A, Ctrl+C, Tab, Ctrl+V.
  5. Middle-click the full-page PRIMARY target.
  6. Super+Space to resize.
  7. Tab to the dialog button, Enter, then Enter to dismiss.
  8. Ctrl+Q; relaunch with Super+F; confirm Kitty remains interactive; Ctrl+Q.
  9. Super+Shift+Q for normal logout.
Do not use Ctrl+Alt+Backspace in the normal proof run.
INSTRUCTIONS
exec "$ROOT_DIR/tools/start_sophia_tty3.sh" --firefox-m8-proof "$@"
