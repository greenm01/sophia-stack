#!/usr/bin/env bash
set -euo pipefail

configured="${SOPHIA_XMONAD_BIN:-}"
if [[ -n "$configured" ]]; then
    [[ -x "$configured" ]] || { echo "configured xmonad binary is not executable: $configured" >&2; exit 1; }
    printf '%s\n' "$configured"
    exit 0
fi

exec "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/build_sophia_xmonad.sh"
