#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad

cat <<'INSTRUCTIONS'
Super-Enter launch-burst proof:
  1. As soon as Sophia owns the displays, press and release Super+Enter at
     least twenty times rapidly. It is valid to begin before Kitty is ready.
  2. Wait for the bounded launch queue to settle. Sophia will admit at most
     sixteen action-launched applications and reject excess requests.
  3. Confirm rendering remains stable. Focus several Kitty windows with
     Super+J and type a harmless command in the focused terminal.
  4. Press Super+Shift+Q for normal logout.
  5. From a text TTY, run:
       tools/verify_sophia_xmonad_launch_burst.sh
Do not use Ctrl+Alt+Backspace during the normal proof.
INSTRUCTIONS

exec "$ROOT_DIR/tools/start_sophia_tty3.sh" "$@"
