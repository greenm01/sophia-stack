#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_qemu_xmonad_launch_burst_evidence.sh"
FIXTURE="$(mktemp)"
MUTATION="$(mktemp)"
trap 'rm -f -- "$FIXTURE" "$MUTATION"' EXIT

{
    echo 'sophia_live_session_mode schema=1 mode=normal configured_apps=14 startup_apps=13'
    for holder in $(seq 1 12); do
        echo "sophia_session_app schema=1 status=started id=holder$holder source=startup"
    done
    echo 'sophia_live_wm schema=1 status=ready adapter=external socket=private restarts=0'
    echo 'sophia_live_session_startup schema=2 status=output_baseline_ready outputs=2/2'
    echo 'sophia_live_native_startup_output schema=1 status=presented output=1 proof=synchronous_modeset submission=1'
    echo 'sophia_live_native_startup_output schema=1 status=presented output=2 proof=synchronous_modeset submission=1'
    echo 'sophia_live_native_page_flip schema=1 status=retired output=1 submission=2 frame=3'
    echo 'sophia_qemu_launch_burst schema=1 status=sent chord=meta_l+ret requests=32'
    for transaction in $(seq 1 4); do
        echo "sophia_session_app schema=2 status=queued source=action transaction=$transaction depth=$transaction"
        echo "sophia_session_app schema=2 status=started id=terminal source=action transaction=$transaction"
        echo "sophia_session_app schema=2 status=admitted source=action transaction=$transaction surface=$transaction"
    done
    for transaction in $(seq 5 32); do
        echo "sophia_session_app schema=2 status=rejected source=action transaction=$transaction reason=capacity"
    done
    for transaction in $(seq 1 32); do
        echo "sophia_live_wm schema=1 status=session_action_committed transaction=$transaction action=LaunchTerminal"
    done
    echo 'sophia_qemu_launch_burst schema=1 status=settled active_preload=12 queued=4 admitted=4 rejected=28'
    echo 'sophia_qemu_launch_burst schema=1 status=capacity_release_wait source=managed_exit'
    echo 'sophia_session_app schema=1 status=exited id=holder1 source=managed exit_status=exit status: 0'
    echo 'sophia_qemu_launch_burst schema=1 status=capacity_released managed_exits=1'
    echo 'sophia_qemu_launch_burst schema=1 status=recovery_launch_begin chord=meta_l+ret'
    echo 'sophia_live_wm schema=1 status=physical_action_committed action=768'
    echo 'sophia_session_app schema=2 status=queued source=action transaction=26 depth=1'
    echo 'sophia_live_wm schema=1 status=session_action_committed transaction=26 action=LaunchTerminal'
    echo 'sophia_session_app schema=2 status=started id=terminal source=action transaction=26'
    echo 'sophia_session_app schema=2 status=admitted source=action transaction=26 surface=26'
    echo 'sophia_qemu_launch_burst schema=1 status=recovery_admitted queued=5 admitted=5'
    echo 'sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control'
    echo 'sophia_qemu_launch_burst schema=1 status=recovery_focus_ready source=x11-control'
    echo 'sophia_qemu_launch_burst schema=1 status=action_probe_begin chord=meta_l+j'
    echo 'sophia_live_wm schema=1 status=physical_action_committed action=257'
    echo 'sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control'
    echo 'sophia_qemu_launch_burst schema=1 status=action_probe_committed chord=meta_l+j focus=applied'
    echo 'sophia_qemu_launch_burst schema=1 status=logout_begin chord=meta_l+shift+q'
    echo 'sophia_live_wm schema=1 status=session_action_committed transaction=27 action=Logout'
    echo 'sophia_session_launches schema=2 status=complete peak_depth=4 rejected=28 admission_timeouts=0 withdrawn=0'
    echo 'sophia_live_session_health schema=1 status=clean protocol_errors=0 pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false'
    echo 'sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=none'
    echo 'sophia_live_output schema=1 status=complete output=1 checksum=1 submissions=3 retirements=2 callbacks=2 nonzero_exports=2'
    echo 'sophia_live_output schema=1 status=complete output=2 checksum=2 submissions=1 retirements=0 callbacks=0 nonzero_exports=1'
    echo 'sophia_live_session schema=16 status=bounded_complete authority_batches_dropped=0 native_submit_failures=0 native_retire_failures=0 native_callback_rejected=0 native_in_flight=false native_cleanup_pending=false wm_policy=external wm_restarts=0 wm_degraded=false'
    echo 'sophia_live_session_cleanup schema=1 status=clean app_groups=0'
    echo 'sophia_qemu_guest schema=1 status=complete scenario=xmonad-launch-burst'
} >"$FIXTURE"

"$VERIFY" "$FIXTURE" >/dev/null

awk '!removed && /status=rejected source=action/ { removed=1; next } { print }' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "launch-burst verifier accepted incomplete rejection accounting" >&2
    exit 1
fi

sed '/status=recovery_admitted /d' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "launch-burst verifier accepted no post-capacity recovery" >&2
    exit 1
fi

awk '
    /status=recovery_launch_begin/ { recovery = 1 }
    recovery && !removed && /status=session_action_committed .* action=LaunchTerminal$/ {
        removed = 1
        next
    }
    { print }
' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "launch-burst verifier accepted unbalanced recovery action accounting" >&2
    exit 1
fi

sed '/status=capacity_released /d' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "launch-burst verifier accepted capacity reuse without a managed release" >&2
    exit 1
fi

awk '
    /status=recovery_launch_begin/ { recovery = 1 }
    recovery && !removed && /status=focus_applied source=x11-control/ {
        removed = 1
        next
    }
    { print }
' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "launch-burst verifier accepted recovery without focus evidence" >&2
    exit 1
fi

sed '/status=action_probe_committed/d' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "launch-burst verifier accepted no post-burst committed action probe" >&2
    exit 1
fi

sed 's/output=2 proof=synchronous_modeset/output=1 proof=synchronous_modeset/' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "launch-burst verifier accepted duplicate startup outputs" >&2
    exit 1
fi

sed 's/peak_depth=4/peak_depth=5/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "launch-burst verifier accepted an impossible pending depth" >&2
    exit 1
fi

awk '!removed && /status=started id=holder12 source=startup/ { removed=1; next } { print }' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "launch-burst verifier accepted an incomplete active-capacity preload" >&2
    exit 1
fi

sed 's/^sophia_session_app schema=1 status=started id=holder3 /xterm: sophia_session_app schema=1 status=started id=holder3 /' \
    "$FIXTURE" >"$MUTATION"
if ! "$VERIFY" "$MUTATION" >/dev/null; then
    echo "launch-burst verifier rejected a complete serial record with an stderr prefix" >&2
    exit 1
fi

grep -Fq 'xmonad-launch-burst' "$ROOT_DIR/tools/qemu_guest_init.sh"
grep -Fq 'meta_l+ret 32' "$ROOT_DIR/tools/qemu_session_harness.sh"
grep -Fq 'status=action_probe_begin chord=meta_l+j' "$ROOT_DIR/tools/qemu_session_harness.sh"
grep -Fq 'qemu_xmonad_launch_burst_acceptance.sh' "$ROOT_DIR/tools/check_atomic_scanout_local.sh"

echo "QEMU xmonad launch-burst verifier regressions passed."
