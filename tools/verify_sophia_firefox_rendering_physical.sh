#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
SESSION_LOG="${1:-${SOPHIA_HAGIA_LOG_DIR:-$STATE_HOME/sophia/hagia-session}/session.log}"
[[ -s "$SESSION_LOG" ]] || { echo "Missing Firefox rendering log: $SESSION_LOG" >&2; exit 1; }
awk -f "$ROOT_DIR/tools/lib/verify_firefox_rendering.awk" "$SESSION_LOG"
echo "Firefox changing-content/native-retirement canary verified: $SESSION_LOG"
