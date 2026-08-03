#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad
export SOPHIA_X11_AUTHORITY_TRACE=1
cat <<'INSTRUCTIONS'
Firefox dialog canary:
  Super+F. Click Open proof dialog. Click Confirm Sophia dialog.
  When the page reports complete, press Super+Shift+Q.
INSTRUCTIONS
exec "$ROOT_DIR/tools/start_sophia_tty3.sh" --firefox-m10-dialog-proof "$@"
