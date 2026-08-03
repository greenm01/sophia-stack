#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad
export SOPHIA_X11_AUTHORITY_TRACE=1
if [[ ! -t 0 || "$(tty)" != /dev/tty3 ]]; then
    echo 'Switch to TTY3, log in, then run:' >&2
    echo "  $ROOT_DIR/tools/start_sophia_xmonad_firefox_proof_tty3.sh" >&2
    exit 1
fi
cat <<'INSTRUCTIONS'
Physical Firefox Milestone 10 proof:
  1. Type A1, open Kitty B with Super+Enter, then type B1.
  2. Launch Firefox with Super+F. Follow its six short steps: type, navigate
     and scroll, resize, focus away/back, confirm the dialog, then Ctrl+Q.
  3. Type A2/B2. From Kitty B launch Firefox again and close it with
     Super+Shift+C.
  4. Type A3/B3. From Kitty B log out with Super+Shift+Q.

The windows retain the instructions, so there is nothing to memorize on TTY3.
CLIPBOARD and PRIMARY are intentionally absent; their focused gates are final.
Do not use Ctrl+Alt+Backspace in the normal proof run.
INSTRUCTIONS
exec "$ROOT_DIR/tools/start_sophia_tty3.sh" --firefox-m10-proof "$@"
