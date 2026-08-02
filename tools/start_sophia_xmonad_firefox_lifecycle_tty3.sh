#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad
export SOPHIA_X11_AUTHORITY_TRACE=1
cat <<'INSTRUCTIONS'
Focused Firefox lifecycle proof (about 45 seconds):
  1. Complete A1/B1, then launch Firefox with Super+F.
  2. After its page appears, close it with Ctrl+Q and complete A2/B2.
  3. From Kitty B launch Firefox again, then close it with Super+Shift+C.
  4. Complete A3/B3 and use Super+Shift+Q.
INSTRUCTIONS
exec "$ROOT_DIR/tools/start_sophia_tty3.sh" --firefox-m10-lifecycle-proof "$@"
