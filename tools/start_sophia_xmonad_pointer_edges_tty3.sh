#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad

cat <<'INSTRUCTIONS'
Two-output pointer-union proof:
  1. Wait for the startup Kitty and hardware cursor.
  2. Cross the internal output seam left-to-right and right-to-left. The cursor
     must cross immediately; the seam must not act like an outer boundary.
  3. Away from a corner, push hard against the far-left external edge, then
     reverse direction. The cursor must stay visible and move immediately.
  4. Repeat at the far-right external edge.
  5. On the left output, repeat at its top and bottom external edges.
  6. On the right output, repeat at its top and bottom external edges.
  7. Keep every test away from corners so each movement identifies one axis.
  8. Press Super+Shift+Q for normal logout. Do not use Ctrl+Alt+Backspace.

After logout this wrapper verifies one ordered clamp/reversal at each outer
horizontal edge, two at each vertical edge, clean cursor health, normal
session teardown, and exact TTY restoration.
INSTRUCTIONS

set +e
"$ROOT_DIR/tools/start_sophia_tty3.sh" "$@"
session_status=$?
set -e
if ((session_status != 0)); then
    echo "Sophia pointer-edge session exited with status $session_status." >&2
    exit "$session_status"
fi

"$ROOT_DIR/tools/verify_sophia_xmonad_pointer_edges.sh"
