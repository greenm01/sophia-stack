#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SPECULA_ROOT="${SOPHIA_SPECULA_ROOT:-$HOME/src/Specula}"
EXPECTED_SPECULA_COMMIT="3946f892cc078d5cfea3629f46bd826c246bf2a9"
TLC_MEMORY_LIMIT="${SOPHIA_SPECULA_TLC_MEMORY_LIMIT:-24G}"
TLC_WORKER_LIMIT="${SOPHIA_SPECULA_TLC_WORKER_LIMIT:-8}"
RUN_ID="${1:-sophia-x11-wm-bridge-$(date -u +%Y%m%dT%H%M%SZ)}"

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    echo "usage: tools/run_specula_x11_wm_bridge.sh [RUN_ID]"
    exit 0
fi
if (( $# > 1 )) || [[ ! "$RUN_ID" =~ ^[0-9A-Za-z._-]+$ ]]; then
    echo "usage: tools/run_specula_x11_wm_bridge.sh [RUN_ID]" >&2
    exit 2
fi
[[ -d "$SPECULA_ROOT/.git" ]] || {
    echo "Specula checkout is missing: $SPECULA_ROOT" >&2
    exit 2
}
[[ "$(git -C "$SPECULA_ROOT" rev-parse HEAD)" == "$EXPECTED_SPECULA_COMMIT" ]] || {
    echo "Specula checkout is not at the audited commit." >&2
    exit 2
}
[[ -z "$(git -C "$ROOT_DIR" status --porcelain)" ]] || {
    echo "Commit or stash Sophia changes before starting an audit." >&2
    exit 2
}
[[ ! -e "$SPECULA_ROOT/runs/$RUN_ID" ]] || {
    echo "Specula run already exists: $SPECULA_ROOT/runs/$RUN_ID" >&2
    exit 2
}

# A clean local clone keeps build products and prior evidence out of Specula's
# private artifact copy while preserving the exact committed Sophia input.
staging_root="$(mktemp -d)"
trap 'rm -rf "$staging_root"' EXIT
artifact="$staging_root/sophia-stack"
git clone --quiet --local --no-hardlinks "$ROOT_DIR" "$artifact"
git -C "$artifact" checkout --quiet "$(git -C "$ROOT_DIR" rev-parse HEAD)"

scope="Complete workspace projection, delayed legacy ConfigureWindow/FocusWindow, restart/reseed, and proactive safe-pixel admission; exclude general X11 wire, input, clipboard, rendering/KMS, Hagia, and public policy"
(
    cd "$SPECULA_ROOT"
    "$ROOT_DIR/tools/specula_dev.sh" run \
        --run-id="$RUN_ID" \
        --fresh-context \
        --artifact="$artifact" \
        --keep-original \
        --agent=codex \
        --max-parallel=1 \
        --tlc-memory-limit="$TLC_MEMORY_LIMIT" \
        --tlc-worker-limit="$TLC_WORKER_LIMIT" \
        "sophia-x11-wm-bridge|greenm01/sophia-stack|Rust|$scope"
)
