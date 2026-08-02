#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad
export SOPHIA_X11_AUTHORITY_TRACE=1
cat <<'INSTRUCTIONS'
Focused Firefox selection proof (about 30 seconds):
  1. Press Super+F from the startup Kitty.
  2. Follow Firefox and Kitty's current CLIPBOARD and PRIMARY prompts.
  3. When Firefox completes PRIMARY, press Super+Shift+Q.

Direction-specific tokens make stale selections fail immediately.
INSTRUCTIONS
exec "$ROOT_DIR/tools/start_sophia_tty3.sh" --firefox-m10-selection-proof "$@"
