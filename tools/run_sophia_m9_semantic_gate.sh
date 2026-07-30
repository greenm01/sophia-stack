#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_DIR="${1:-}"

fail() {
    echo "Milestone 9 semantic gate failed: $*" >&2
    exit 1
}

[[ "$EVIDENCE_DIR" == /* ]] ||
    fail "pass one absolute evidence-directory path"
mkdir -p "$EVIDENCE_DIR"
chmod 700 "$EVIDENCE_DIR"

COMMIT="$(git -C "$ROOT_DIR" rev-parse HEAD)"
SEQUENCE="$EVIDENCE_DIR/sequence.log"
RUNNER_LOG="$EVIDENCE_DIR/runner.log"
M7_EVIDENCE="$EVIDENCE_DIR/qemu-m7.log"
M8_EVIDENCE="$EVIDENCE_DIR/qemu-m8-mix.log"
INPUT_LATENCY_EVIDENCE="$EVIDENCE_DIR/qemu-input-latency.log"

: >"$RUNNER_LOG"
chmod 600 "$RUNNER_LOG"
exec > >(tee "$RUNNER_LOG") 2>&1
printf 'commit=%s\nphase=local-regressions-started\n' "$COMMIT" >"$SEQUENCE"
chmod 600 "$SEQUENCE"

cd "$ROOT_DIR"
tools/check_atomic_scanout_local.sh
printf '%s\n' 'phase=local-regressions-complete' >>"$SEQUENCE"

SOPHIA_QEMU_EVIDENCE="$M7_EVIDENCE" tools/qemu_xmonad_m7_acceptance.sh
printf '%s\n' 'phase=qemu-m7-complete' >>"$SEQUENCE"

SOPHIA_QEMU_EVIDENCE="$M8_EVIDENCE" tools/qemu_xmonad_m8_mix_acceptance.sh
printf '%s\n' 'phase=qemu-m8-mix-complete' >>"$SEQUENCE"

SOPHIA_QEMU_EVIDENCE="$INPUT_LATENCY_EVIDENCE" \
    tools/run_sophia_input_latency_qemu.sh
printf '%s\n' 'phase=qemu-input-latency-complete' >>"$SEQUENCE"

tools/verify_sophia_m9_semantic_gate.sh "$EVIDENCE_DIR" "$COMMIT"
printf '%s\n' 'phase=semantic-gate-complete' >>"$SEQUENCE"
