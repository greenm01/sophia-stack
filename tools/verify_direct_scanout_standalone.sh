#!/usr/bin/env bash
set -euo pipefail

# Verifies a direct-scanout standalone probe: the session had the shape that
# can produce an eligible frame, and the direct path engaged lawfully.
#
# The shape is one client and no window manager. No WM is not a shortcut here:
# `sophia-wm-demo` lost its serving mode in 83596bfc, and a session without one
# honours the client's own geometry and draws no focus ring or border -- which
# is what a frame of exactly one client layer requires anyway.
#
# Separate from `verify_sophia_standalone_vkcube.sh`, which asserts the
# natural-size reference policy and a WM this run deliberately does not have.

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

# One client. A second surface is a second layer, and the frame is then
# something the compositor has to combine rather than hand over.
grep -qE '(^| )runtime_surfaces=1( |$)' <<<"$session" ||
    fail "the session did not run exactly one client surface"

"$ROOT_DIR/tools/verify_direct_scanout_sessions.sh" "$SESSION_LOG"
echo "direct scanout standalone probe passed: $SESSION_LOG"
