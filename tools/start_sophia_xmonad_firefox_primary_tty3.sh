#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad
export SOPHIA_X11_AUTHORITY_TRACE=1
export SOPHIA_SESSION_VERBOSE_TRACE=true
cat <<'INSTRUCTIONS'
Firefox PRIMARY-only gate:
  Super+F. Drag-select the entire Firefox token. Super+J to Kitty.
  Middle-click once in Kitty, then drag-select its entire return token.
  Super+J to Firefox, middle-click once, then press Super+Shift+Q.
INSTRUCTIONS
exec "$ROOT_DIR/tools/start_sophia_tty3.sh" --firefox-m10-primary-proof "$@"
