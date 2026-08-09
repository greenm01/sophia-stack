#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HAGIA_ROOT="${SOPHIA_HAGIA_ROOT:-$ROOT_DIR/../hagia}"

if [[ ! -t 0 || "$(tty)" != /dev/tty4 ]]; then
    echo "Switch to tty4 with Ctrl+Alt+F4, log in, and run:" >&2
    echo "  $ROOT_DIR/tools/run_current_hagia_policy_gate_tty4.sh" >&2
    exit 1
fi
if [[ ! -d "$HAGIA_ROOT/.git" ]]; then
    echo "Hagia checkout not found at $HAGIA_ROOT" >&2
    echo "Set SOPHIA_HAGIA_ROOT to its checkout path." >&2
    exit 1
fi
if [[ -n "$(git -C "$ROOT_DIR" status --short)" ]]; then
    echo "Sophia worktree must be clean before the physical proof." >&2
    exit 1
fi
if [[ -n "$(git -C "$HAGIA_ROOT" status --short)" ]]; then
    echo "Hagia worktree must be clean before the physical proof." >&2
    exit 1
fi

sophia_commit="$(git -C "$ROOT_DIR" rev-parse HEAD)"
hagia_commit="$(git -C "$HAGIA_ROOT" rev-parse HEAD)"
hagia_bin="${TMPDIR:-/tmp}/hagia-policy-${hagia_commit:0:12}"

echo "Building exact physical-proof binaries before DRM takeover..."
echo "Sophia: $sophia_commit"
echo "Hagia:  $hagia_commit"
(
    cd "$HAGIA_ROOT"
    nim c -d:release --path:src -o:"$hagia_bin" src/hagia.nim
)
(
    cd "$ROOT_DIR"
    cargo build --quiet --release --offline -p sophia-cli \
        --features atomic-scanout-live
)
"$hagia_bin" config check
sha256sum "$hagia_bin"

export SOPHIA_HAGIA_BIN="$hagia_bin"
export SOPHIA_LIVE_SESSION_SKIP_BUILD=1
exec "$ROOT_DIR/tools/start_sophia_hagia_policy_tty4.sh"
