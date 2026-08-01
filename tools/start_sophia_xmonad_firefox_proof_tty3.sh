#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad
cat <<'INSTRUCTIONS'
Physical Firefox Milestone 10 proof:
  1. Follow the short checkpoint prompt shown inside each Kitty.
  2. Follow the current-step prompt shown inside the offline Firefox page.
  3. After both Kitty windows report A3/B3 complete, use Super+Shift+Q.

The windows retain the instructions, so there is nothing to memorize on TTY3.
Do not use Ctrl+Alt+Backspace in the normal proof run.
INSTRUCTIONS
exec "$ROOT_DIR/tools/start_sophia_tty3.sh" --firefox-m10-proof "$@"
