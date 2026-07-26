#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_DIR="${1:-}"
EXPECTED_COMMIT="${2:-$(git -C "$ROOT_DIR" rev-parse HEAD)}"
SEQUENCE="$EVIDENCE_DIR/sequence.log"
M7_EVIDENCE="$EVIDENCE_DIR/qemu-m7.log"
M8_EVIDENCE="$EVIDENCE_DIR/qemu-m8-mix.log"

fail() {
    echo "Milestone 9 semantic verification failed: $*" >&2
    exit 1
}

[[ "$EXPECTED_COMMIT" =~ ^[0-9a-f]{40}$ ]] ||
    fail "expected commit must be a full Git object ID"
for evidence in "$SEQUENCE" "$M7_EVIDENCE" "$M8_EVIDENCE"; do
    [[ -s "$evidence" ]] || fail "missing evidence: $evidence"
done
grep -Fxq "commit=$EXPECTED_COMMIT" "$SEQUENCE" ||
    fail "semantic evidence belongs to another commit"
for phase in local-regressions-complete qemu-m7-complete qemu-m8-mix-complete; do
    grep -Fxq "phase=$phase" "$SEQUENCE" ||
        fail "missing completed phase: $phase"
done

"$ROOT_DIR/tools/verify_qemu_xmonad_m7_evidence.sh" "$M7_EVIDENCE"
"$ROOT_DIR/tools/verify_qemu_xmonad_m8_mix_evidence.sh" "$M8_EVIDENCE"

echo "Milestone 9 unattended semantic evidence passed: $EVIDENCE_DIR"
