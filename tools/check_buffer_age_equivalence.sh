#!/usr/bin/env bash
set -euo pipefail

# Proves damage-limited repaint pixel-identical to full repaint on this host's
# real GPU, through a render node only: no DRM master, no display takeover, and
# safe to run inside a desktop session. This is the executable form of the
# model's `RepaintMatchesFullRepaint`, plus a negative control proving a lying
# damage table is caught by the same comparison.
#
# The gated tests skip silently without a device, and a skipped proof looks
# exactly like a passing one from the outside, so this wrapper refuses instead
# when no render node is writable.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

node=""
for candidate in /dev/dri/renderD*; do
    if [[ -r "$candidate" && -w "$candidate" ]]; then
        node="$candidate"
        break
    fi
done
if [[ -z "$node" ]]; then
    echo "No writable render node; the pixel-equivalence proof cannot run here." >&2
    exit 2
fi

echo "Buffer-age pixel equivalence on $node"
(
    cd "$ROOT_DIR"
    SOPHIA_RUN_REAL_GBM_SMOKE=1 cargo test --quiet --offline \
        -p sophia-backend-live \
        --features libdrm-events,gbm-probe \
        --test buffer_age_equivalence
)
echo "Buffer-age pixel equivalence passed"
