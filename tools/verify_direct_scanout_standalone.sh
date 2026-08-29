#!/usr/bin/env bash
set -euo pipefail

# Verifies a direct-scanout standalone probe: the right policy ran, one surface
# was laid out, and the direct path engaged lawfully.
#
# Separate from `verify_sophia_standalone_vkcube.sh` because that one asserts
# the natural-size reference policy, which this run deliberately does not use.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
SESSION_LOG="${1:-$STATE_HOME/sophia/standalone-session/session.log}"

fail() {
    printf 'direct scanout standalone probe failed: %s\n' "$1" >&2
    exit 1
}

[[ -s "$SESSION_LOG" ]] || fail "no session evidence at $SESSION_LOG"

# The policy that can produce an eligible frame. A run under the natural-size
# policy would report zeros for a reason that has nothing to do with the row.
grep -Eq '^sophia_wm_demo schema=1 status=ready generation=[0-9]+ layout_policy=columns$' \
    "$SESSION_LOG" ||
    fail "the filling layout policy did not start; this run cannot produce an eligible frame"
grep -Eq '^sophia_live_wm schema=1 status=layout_committed .* surfaces=1 .* outcome=Committed$' \
    "$SESSION_LOG" ||
    fail "a single-surface layout did not commit"

"$ROOT_DIR/tools/verify_direct_scanout_sessions.sh" "$SESSION_LOG"
echo "direct scanout standalone probe passed: $SESSION_LOG"
