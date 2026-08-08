#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_qemu_xmonad_stale_response_evidence.sh"
FIXTURE="$(mktemp)"
MUTATION="$(mktemp)"
trap 'rm -f -- "$FIXTURE" "$MUTATION"' EXIT

{
    echo 'sophia_live_session_mode schema=1 mode=normal configured_apps=3 startup_apps=2'
    echo 'sophia_session_app schema=1 status=started id=primary source=startup'
    echo 'sophia_session_app schema=1 status=started id=secondary source=startup'
    echo 'sophia_live_wm schema=1 status=ready adapter=external socket=private restarts=0'
    echo 'sophia_live_session_startup schema=2 status=output_baseline_ready outputs=2/2'
    echo 'sophia_live_native_startup_output schema=1 status=presented output=1 proof=synchronous_modeset submission=1'
    echo 'sophia_live_native_startup_output schema=1 status=presented output=2 proof=synchronous_modeset submission=1'
    echo 'sophia_qemu_stale_response schema=1 status=launch_begin chord=meta_l+ret'
    echo 'sophia_session_app schema=2 status=queued source=action transaction=20 depth=1'
    echo 'sophia_session_app schema=2 status=started id=transient source=action transaction=20'
    echo 'sophia_session_app schema=2 status=completed id=transient source=action transaction=20 reason=normal_exit_after_surface exit_status=exit status: 0'
    echo 'sophia_live_wm schema=3 status=response_rejected reason=stale_layout transaction=20 source=manage removed_registered_surfaces=0'
    echo 'sophia_live_wm schema=2 status=restart_requested reason=stale_response error=none'
    echo 'sophia_live_wm schema=1 status=restarted restarts=1 preserved_layout=true'
    echo 'sophia_live_wm schema=4 status=reseed_queued phase=committed_layout request=relayout'
    echo 'sophia_live_wm schema=2 status=workspace_projection_committed transaction=21 output=1 workspace=1 visible_surfaces=2 focus=surface'
    echo 'sophia_qemu_stale_response schema=1 status=recovered restarts=1 visible_surfaces=2'
    echo 'sophia_qemu_stale_response schema=1 status=action_probe_begin chord=meta_l+j'
    echo 'sophia_live_wm schema=1 status=physical_action_committed action=1'
    echo 'sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control'
    echo 'sophia_qemu_stale_response schema=1 status=action_probe_committed chord=meta_l+j focus=applied'
    echo 'sophia_qemu_stale_response schema=1 status=logout_begin chord=meta_l+shift+q'
    echo 'sophia_live_wm schema=1 status=session_action_committed transaction=23 action=Logout'
    echo 'sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=none'
    echo 'sophia_live_wm_transport schema=2 status=complete peak_depth=2 pending=0 rejected=0 action_ordered=1 action_coalesced=0 stale_responses=1 max_queue_dwell_msec=4 max_round_trip_msec=90'
    echo 'sophia_live_session_health schema=1 status=clean protocol_errors=0 pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false'
    echo 'sophia_live_session schema=16 status=bounded_complete authority_batches_dropped=0 native_submit_failures=0 native_retire_failures=0 native_callback_rejected=0 native_in_flight=false native_cleanup_pending=false wm_policy=external wm_restarts=1 wm_degraded=false'
    echo 'sophia_live_output schema=1 status=complete output=1 checksum=1 submissions=3 retirements=2 callbacks=2 nonzero_exports=2'
    echo 'sophia_live_output schema=1 status=complete output=2 checksum=2 submissions=1 retirements=0 callbacks=0 nonzero_exports=1'
    echo 'sophia_live_session_cleanup schema=1 status=clean app_groups=0'
    echo 'sophia_qemu_guest schema=1 status=complete scenario=xmonad-stale-response'
} >"$FIXTURE"

"$VERIFY" "$FIXTURE" >/dev/null

for pattern in \
    'status=response_rejected reason=stale_layout' \
    'status=restart_requested reason=stale_response' \
    'status=restarted restarts=1' \
    'status=reseed_queued phase=committed_layout' \
    'status=action_probe_committed'; do
    grep -v "$pattern" "$FIXTURE" >"$MUTATION"
    if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
        echo "stale-response verifier accepted evidence missing: $pattern" >&2
        exit 1
    fi
done

sed '/status=restarted restarts=1/a sophia_live_wm schema=1 status=restarted restarts=2 preserved_layout=true' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "stale-response verifier accepted a second WM restart" >&2
    exit 1
fi

sed 's/stale_responses=1/stale_responses=0/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "stale-response verifier accepted incorrect stale-response accounting" >&2
    exit 1
fi

sed 's/wm_restarts=1/wm_restarts=0/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "stale-response verifier accepted incorrect completion restart accounting" >&2
    exit 1
fi

sed '/status=recovered /i Error: UnknownSurface' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "stale-response verifier accepted the historical UnknownSurface failure" >&2
    exit 1
fi

sed '/status=completed id=transient /d' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "stale-response verifier accepted an unobserved transient exit" >&2
    exit 1
fi

sed '/status=completed id=transient /a sophia_session_app schema=2 status=admitted source=action transaction=20 surface=3' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "stale-response verifier accepted admission after the transient exit" >&2
    exit 1
fi

grep -Fq 'xmonad-stale-response' "$ROOT_DIR/tools/qemu_guest_init.sh"
grep -Fq 'mode=stale-response' "$ROOT_DIR/tools/qemu_guest_init.sh"
grep -Fq 'status=recovered restarts=1 visible_surfaces=2' \
    "$ROOT_DIR/tools/qemu_session_harness.sh"
grep -Fq 'qemu_xmonad_stale_response_acceptance.sh' \
    "$ROOT_DIR/tools/check_atomic_scanout_local.sh"

echo "QEMU xmonad stale-response verifier regressions passed."
