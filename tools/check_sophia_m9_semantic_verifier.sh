#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMMIT=0123456789abcdef0123456789abcdef01234567
TMP_DIR="$(mktemp -d /tmp/sophia-m9-semantic-verifier.XXXXXX)"
trap 'rm -rf "$TMP_DIR"' EXIT

cp "$ROOT_DIR/tools/fixtures/qemu_xmonad_m7_pass.log" \
    "$TMP_DIR/qemu-m7.log"
cp "$ROOT_DIR/tools/fixtures/qemu_xmonad_m8_mix_pass.log" \
    "$TMP_DIR/qemu-m8-mix.log"
printf '%s\n' \
    "commit=$COMMIT" \
    phase=local-regressions-started \
    phase=local-regressions-complete \
    phase=qemu-m7-complete \
    phase=qemu-m8-mix-complete \
    >"$TMP_DIR/sequence.log"

"$ROOT_DIR/tools/verify_sophia_m9_semantic_gate.sh" "$TMP_DIR" "$COMMIT"

expect_failure() {
    local label=$1 expected_commit=${2:-$COMMIT}
    if "$ROOT_DIR/tools/verify_sophia_m9_semantic_gate.sh" \
        "$TMP_DIR" "$expected_commit" >/dev/null 2>&1; then
        echo "semantic verifier accepted invalid evidence: $label" >&2
        exit 1
    fi
}

sed -i '/phase=qemu-m8-mix-complete/d' "$TMP_DIR/sequence.log"
expect_failure missing_m8_phase
printf '%s\n' phase=qemu-m8-mix-complete >>"$TMP_DIR/sequence.log"
expect_failure wrong_commit fedcba9876543210fedcba9876543210fedcba98
printf '%s\n' 'sophia_qemu_guest schema=1 status=failed scenario=xmonad-m8-mix' \
    >>"$TMP_DIR/qemu-m8-mix.log"
expect_failure failed_m8_evidence

echo "Milestone 9 semantic verifier regressions passed"
