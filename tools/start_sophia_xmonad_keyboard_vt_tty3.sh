#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad

cat <<'INSTRUCTIONS'
pc105 US and K_OFF virtual-terminal proof:
  1. At the Kitty prompt, type this exact shifted sequence without Enter:
       ~!@#$%^&*()_+{}|:"<>?
     Confirm every character is exact, then clear the line.
  2. Sophia is running on TTY3. For each target F1 through F12:
       a. From Sophia press Ctrl+Alt+Fn once.
       b. If it switches away, return with Ctrl+Alt+F3.
       c. Confirm Kitty keyboard and pointer input work after the return.
     Ctrl+Alt+F3 itself is the active-TTY observation and may not switch.
  3. Type a harmless word and move the pointer after the final return.
  4. Press Super+Shift+Q for normal logout.
  5. From a text TTY run:
       tools/verify_sophia_xmonad_keyboard_vt.sh
Do not use Ctrl+Alt+Backspace during this normal proof.
INSTRUCTIONS

"$ROOT_DIR/tools/start_sophia_tty3.sh" --max-runtime-ms=900000 "$@"
