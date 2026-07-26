#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad

cat <<'INSTRUCTIONS'
Short physical hardware smoke:
  1. Wait for the startup Kitty prompt. Press Super+Enter three times, waiting
     for four sharp, stable Kitty tiles. Type briefly in two tiles, then leave
     all four visible for ten seconds so presentation lifetime can settle.
  2. Move into an unfocused tile, click once, and type "focus-ok". Confirm the
     border and text move there.
  3. Press Ctrl+Alt+F2, then return with Ctrl+Alt+F3. Confirm keyboard, pointer,
     all four tiles, and the status bar still work.
  4. Press Super+Shift+Q for normal logout.
That is the complete sequence. Semantic clipboard/workspace/repeat/close tests
run unattended in QEMU and are intentionally not repeated here.
INSTRUCTIONS

exec "$ROOT_DIR/tools/start_sophia_tty3.sh" "$@"
