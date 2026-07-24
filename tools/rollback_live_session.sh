#!/usr/bin/env bash
set -euo pipefail

PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"
current="$(readlink "$PREFIX/current" 2>/dev/null || true)"
previous="$(readlink "$PREFIX/previous" 2>/dev/null || true)"
[[ -n "$current" && -n "$previous" ]] || {
    echo "Rollback requires both $PREFIX/current and $PREFIX/previous." >&2
    exit 1
}
[[ -d "$PREFIX/$previous" ]] || {
    echo "Previous release is missing: $PREFIX/$previous" >&2
    exit 1
}

ln -sfn "$previous" "$PREFIX/current"
ln -sfn "$current" "$PREFIX/previous"
echo "Rolled back Sophia: current=$previous previous=$current"
