#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad

cat <<'INSTRUCTIONS'
Focused cross-Kitty clipboard proof:
  1. Wait for the startup Kitty prompt, then press Super+3.
  2. Press Super+Enter and wait for the workspace-3 Kitty prompt.
  3. Type:
       printf 'sophia-clipboard-3\n'
     Select only the printed word with the pointer and press Ctrl+Shift+C.
  4. Press Super+Enter. In the new focused Kitty press Ctrl+Shift+V once.
     Wait three seconds. Record whether the exact word appears.
  5. Press Super+Shift+Q for normal logout. Do not retry or close either
     workspace-3 Kitty; the first transfer attempt is the diagnostic proof.
  6. Return here and type "done"; the session log contains content-redacted
     request, property, notify, and property-read stages.
Do not use Ctrl+Alt+Backspace during this normal proof.
INSTRUCTIONS

exec "$ROOT_DIR/tools/start_sophia_tty3.sh" "$@"
