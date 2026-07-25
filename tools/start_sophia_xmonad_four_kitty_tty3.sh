#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad

cat <<'INSTRUCTIONS'
Four-Kitty atomic resize proof:
  1. Wait for the startup Kitty prompt.
  2. Press Super+Enter three times, waiting for each new Kitty prompt.
  3. Confirm the four-window Tall layout has one full-height left pane and
     three equal-height right panes.
  4. Type a harmless command in every Kitty after focusing it with Super+J.
  5. Close the fourth Kitty with Super+Shift+C, then reopen it with Super+Enter.
  6. Confirm the layout remains sharp, stable, and interactive.
  7. Press Super+Shift+Q for normal logout.
  8. From a text TTY, run:
       tools/verify_sophia_xmonad_four_kitty.sh
Do not use Ctrl+Alt+Backspace during the normal proof.
INSTRUCTIONS

exec "$ROOT_DIR/tools/start_sophia_tty3.sh" "$@"
