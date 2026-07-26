#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_sophia_xmonad_pointer_edges.sh"
FIXTURE="$ROOT_DIR/tools/fixtures/physical_xmonad_pointer_edges_pass.log"
GUARD="$ROOT_DIR/tools/fixtures/physical_firefox_guard_pass.log"
RECOVERY="$ROOT_DIR/tools/fixtures/physical_firefox_recovery_pass.log"
TEMP_FILE="$(mktemp)"
trap 'rm -f -- "$TEMP_FILE"' EXIT

"$VERIFY" "$FIXTURE" "$GUARD" "$RECOVERY"

for mutation in \
    'schema=8 status=output_edge_confined axis=horizontal side=minimum output_slot=0' \
    'schema=8 status=edge_reverse_immediate axis=horizontal side=maximum output_slot=1' \
    'schema=8 status=output_edge_confined axis=vertical side=minimum output_slot=1' \
    'schema=8 status=edge_reverse_immediate axis=vertical side=maximum output_slot=0' \
    'schema=8 status=output_transition from_slot=0 to_slot=1 boundary=free' \
    'schema=8 status=output_transition from_slot=1 to_slot=0 boundary=free'; do
    grep -Fv "$mutation" "$FIXTURE" >"$TEMP_FILE"
    if "$VERIFY" "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
        echo "pointer-edge verifier accepted evidence missing: $mutation" >&2
        exit 1
    fi
done

for mutation in \
    's/hidden_updates=0/hidden_updates=1/' \
    's/hardware_failures=0/hardware_failures=1/' \
    's/from_slot=0 to_slot=1 boundary=free/from_slot=0 to_slot=1 boundary=projected/' \
    's/wm_restarts=0/wm_restarts=1/'; do
    sed "$mutation" "$FIXTURE" >"$TEMP_FILE"
    if "$VERIFY" "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
        echo "pointer-edge verifier accepted invalid health evidence: $mutation" >&2
        exit 1
    fi
done

echo "xmonad pointer-edge verifier self-check passed"
