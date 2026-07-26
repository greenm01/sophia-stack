#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad

xmobar_bin="$("$ROOT_DIR/tools/resolve_sophia_xmobar.sh")"
[[ -x "$xmobar_bin" ]] || {
    echo "The focused status-bar proof requires an executable unmodified xmobar." >&2
    exit 1
}
export SOPHIA_XMOBAR_BIN="$xmobar_bin"

cat <<'INSTRUCTIONS'
Xmobar work-area and lifecycle proof:
  1. Wait for the bar to update and for the startup Kitty prompt.
  2. Confirm Kitty begins immediately below the bar with no seam, overlap, or
     occluded pixels.
  3. Type "focus-before-bar" in Kitty without pressing Enter.
  4. Move onto xmobar, click once, and scroll once. Move back into Kitty and
     type "-focus-after-bar"; the original prompt must still receive it.
  5. Press Super+2, confirm the bar remains visible and updating, then press
     Super+1 and confirm Kitty and its input focus return.
  6. Switch to another TTY with Ctrl+Alt+Fn, return to Sophia, and confirm the
     bar, Kitty pixels, pointer, and keyboard are still live.
  7. Press Super+Shift+Q for normal logout.
  8. From a text TTY run:
       tools/verify_sophia_xmonad_xmobar.sh
Do not use Ctrl+Alt+Backspace during this normal proof.
INSTRUCTIONS

exec "$ROOT_DIR/tools/start_sophia_tty3.sh" "$@"
