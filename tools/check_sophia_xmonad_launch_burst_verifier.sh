#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="$(mktemp)"
mutation="$(mktemp)"
trap 'rm -f "$fixture" "$mutation"' EXIT

{
    echo 'sophia_live_session_startup schema=2 status=output_baseline_ready outputs=2/2'
    echo 'sophia_live_session_startup schema=2 status=ready elapsed_msec=500 surface=true visual_detail=true presented=true outputs_ready=2/2 recovery_attempts=0'
    for transaction in $(seq 1 16); do
        echo "sophia_session_app schema=2 status=queued source=action transaction=$transaction depth=1"
        echo "sophia_session_app schema=2 status=started id=terminal source=action transaction=$transaction"
        echo "sophia_session_app schema=2 status=admitted source=action transaction=$transaction surface=$transaction"
    done
    echo 'sophia_session_app schema=2 status=rejected source=action transaction=17 reason=capacity'
    echo 'sophia_live_session_input_pipeline schema=1 status=key_routed'
    echo 'sophia_session_launches schema=1 status=complete peak_depth=1 rejected=1 admission_timeouts=0'
    echo 'sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=none'
    echo 'sophia_live_output schema=1 status=complete output=1 checksum=1 submissions=2 retirements=1 callbacks=1 nonzero_exports=2'
    echo 'sophia_live_output schema=1 status=complete output=2 checksum=2 submissions=2 retirements=1 callbacks=1 nonzero_exports=2'
    echo 'sophia_live_session schema=16 status=bounded_complete placeholder=true'
    echo 'sophia_live_session_cleanup schema=1 status=clean app_groups=0'
} >"$fixture"

"$ROOT_DIR/tools/verify_sophia_xmonad_launch_burst.sh" "$fixture" >/dev/null

grep -v 'status=output_baseline_ready' "$fixture" >"$mutation"
if "$ROOT_DIR/tools/verify_sophia_xmonad_launch_burst.sh" "$mutation" >/dev/null 2>&1; then
    echo "launch-burst verifier accepted missing output baseline" >&2
    exit 1
fi

sed '0,/status=admitted/{/status=admitted/d;}' "$fixture" >"$mutation"
if "$ROOT_DIR/tools/verify_sophia_xmonad_launch_burst.sh" "$mutation" >/dev/null 2>&1; then
    echo "launch-burst verifier accepted an incomplete admission" >&2
    exit 1
fi

echo "xmonad launch-burst verifier self-check passed."
