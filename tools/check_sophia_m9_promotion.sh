#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

set -- status
# The promotion script exposes its pure gate-policy helpers after dispatching
# status, allowing this checker to exercise them without collecting evidence.
source "$ROOT_DIR/tools/sophia_m9_promotion.sh" >/dev/null

gate_launcher_status_accepted 00-qemu-semantic 0
gate_launcher_status_accepted 04-emergency 0
gate_launcher_status_accepted 04-emergency 130
if gate_launcher_status_accepted 02-hardware-smoke 130; then
    echo "promotion accepted emergency launcher status for hardware smoke" >&2
    exit 1
fi
if gate_launcher_status_accepted 04-emergency 1; then
    echo "promotion accepted a generic emergency launcher failure" >&2
    exit 1
fi

echo "Milestone 9 promotion gate-policy checks passed"
