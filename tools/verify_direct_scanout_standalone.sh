#!/usr/bin/env bash
set -euo pipefail

# Verifies a direct-scanout standalone probe: the session had the shape that
# can produce an eligible frame, and the direct path engaged lawfully.
#
# The shape is one client and no window manager -- not a shortcut, but what
# direct scanout requires: a session without a WM honours the client's own
# geometry and draws no focus ring or border over the frame. `sophia-wm-demo`
# could not serve one anyway since 83596bfc.
#
# Separate from `verify_sophia_standalone_vkcube.sh`, which proves the
# single-application session itself rather than what reached the plane.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
SESSION_LOG="${1:-$STATE_HOME/sophia/standalone-session/session.log}"

fail() {
    printf 'direct scanout standalone probe failed: %s\n' "$1" >&2
    exit 1
}

[[ -s "$SESSION_LOG" ]] || fail "no session evidence at $SESSION_LOG"

session="$(grep -E '^sophia_live_session schema=16 ' "$SESSION_LOG" | tail -n1 || true)"
[[ -n "$session" ]] || fail "the session did not reach a bounded completion"

# A window manager would draw a focus ring or a border over the client, and
# every command that paints means the composed image is not the client's
# buffer. Checked rather than assumed, because a session that quietly acquired
# one would report zeros for a reason unrelated to this row.
grep -qE '(^| )wm_policy=disabled( |$)' <<<"$session" ||
    fail "a window manager ran; its chrome makes every frame ineligible"

# The client presented something. `runtime_surfaces` is the count still live at
# the end, which is zero for a bounded client that exits on purpose -- checking
# it for one was this verifier failing a run in which everything worked.
#
# One client is what the direct-scanout verdicts prove instead: a second
# surface is a second layer, and every frame would report `layer_count`.
grep -qE '^sophia_live_session_present schema=2 status=retired ' "$SESSION_LOG" ||
    fail "the client never presented a frame"

"$ROOT_DIR/tools/verify_direct_scanout_sessions.sh" "$SESSION_LOG"
echo "direct scanout standalone probe passed: $SESSION_LOG"
