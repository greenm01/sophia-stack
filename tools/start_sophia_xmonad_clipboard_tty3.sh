#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad

cat <<'INSTRUCTIONS'
Focused cross-Kitty clipboard proof:
  1. Wait for the startup workspace-1 Kitty prompt. Type:
       printf 'sophia-clipboard-roundtrip\n'
     Select only the printed word with the pointer and press Ctrl+Shift+C.
  2. Press Super+3, then Super+Enter. Wait for the workspace-3 Kitty prompt
     and press Ctrl+Shift+V once. Wait three seconds and verify the exact word.
  3. Select only that pasted word and press Ctrl+Shift+C.
  4. Press Super+1. In the original Kitty press Ctrl+Shift+V once. Wait three
     seconds and verify the same exact word appears a second time.
  5. Press Super+Shift+Q for normal logout. Do not retry either transfer or
     close either Kitty; the two first attempts are the diagnostic proof.
  6. Return here and type "done". The session summary must contain at least
     two owner changes and two conversions with content redacted.
Do not use Ctrl+Alt+Backspace during this normal proof.
INSTRUCTIONS

exec "$ROOT_DIR/tools/start_sophia_tty3.sh" "$@"
