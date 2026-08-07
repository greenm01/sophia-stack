#!/usr/bin/env bash
set -euo pipefail

if [[ -n "${SOPHIA_XMONAD_BIN:-}" ]]; then
    [[ -x "$SOPHIA_XMONAD_BIN" ]] || {
        echo "SOPHIA_XMONAD_BIN is not executable: $SOPHIA_XMONAD_BIN" >&2
        exit 1
    }
    printf '%s\n' "$SOPHIA_XMONAD_BIN"
    exit 0
fi

if [[ "${SOPHIA_INSTALLED_SESSION:-false}" == true ]]; then
    echo "An installed Sophia session requires its packaged xmonad binary." >&2
    exit 1
fi

exec "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/build_sophia_xmonad.sh"
