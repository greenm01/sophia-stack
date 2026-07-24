#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=xmonad
export SOPHIA_SESSION_VERBOSE_TRACE=false
exec "$ROOT_DIR/tools/start_sophia_tty3.sh" \
    --max-runtime-ms=120000 \
    --expect-physical-text=sophia \
    --expect-physical-pointer \
    --exit-after-input-proof \
    "$@"
