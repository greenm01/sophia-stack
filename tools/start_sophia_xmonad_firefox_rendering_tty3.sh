#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad
export SOPHIA_X11_AUTHORITY_TRACE=1
cat <<'INSTRUCTIONS'
Firefox rendering canary:
  Press Super+F. Firefox must fill the left column with no black region.
  Press Super+Shift+Q to finish. Do not interact with the page.
INSTRUCTIONS
exec "$ROOT_DIR/tools/start_sophia_tty3.sh" --firefox-m10-rendering-proof "$@"
