#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_qemu_xmonad_resize_storm_evidence.sh"
FIXTURE="$(mktemp)"
MUTATION="$(mktemp)"
trap 'rm -f -- "$FIXTURE" "$MUTATION"' EXIT

{
    echo 'sophia_live_session_mode schema=1 mode=normal configured_apps=1 startup_apps=1'
    echo 'sophia_session_app schema=1 status=started id=renderer source=startup'
    echo 'sophia_live_wm schema=1 status=ready adapter=external socket=private restarts=0'
    echo 'sophia_live_session_startup schema=2 status=output_baseline_ready outputs=2/2'
    for step in $(seq 1 12); do
        transaction=$((1999999 + step))
        if (( step % 2 == 0 )); then width=800 height=600; else width=960 height=640; fi
        echo "sophia_live_resize schema=2 status=requested transaction=$transaction surface=3 width=$width height=$height step=$step total=12"
        echo "sophia_live_wm schema=1 status=layout_committed transaction=$transaction surfaces=1 moved_surfaces=1 configure_deliveries=1 outcome=Committed"
        echo "sophia_live_resize_epoch schema=1 status=committed transaction=$transaction matched_surfaces=1"
        echo "sophia_live_resize schema=2 status=committed transaction=$transaction surface=3 width=$width height=$height configure_delivered=true pixels=true step=$step total=12"
        echo "sophia_live_native_page_flip schema=1 status=retired output=1 submission=$step frame=$step"
    done
    echo 'sophia_live_resize_storm schema=1 status=complete steps=12 surface=3 exact_pixels=true'
    echo 'sophia_live_output_repaint schema=1 status=presented output=1 mode=partial rects=2 pixels=2000'
    echo 'sophia_qemu_resize_storm schema=1 status=post_storm_frame_retired steps=12'
    echo 'sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=none'
    echo 'sophia_live_rendering_efficiency schema=1 status=complete cpu_updates=30 cpu_replacements=1 cpu_patch_updates=29 cpu_patch_rects=29 cpu_payload_bytes=20000 exact_pixel_metric_frames=3 damage_scoped_metric_frames=20 composition_target_reuses=5'
    echo 'sophia_live_native_resources schema=5 status=complete target_creations=1 pipeline_creations=1 frame_surface_creations=1 cpu_target_creations=0 dmabuf_target_creations=0 composition_target_creations=1 composition_target_reuses=5 generation_replacements=0 recovery_replacements=0 snapshot_captures=0 snapshot_promotions=0 snapshot_rollbacks=0 snapshot_evictions=0 snapshot_live_entries=0 snapshot_live_bytes=0 import_cache_imports=0 import_cache_hits=0 import_cache_evictions=0 import_cache_live_entries=0 import_cache_descriptor_mismatches=0 import_cache_capacity_rejections=0 worker_requests=12 worker_completions=12 worker_failures=0 worker_soft_stalls=0 worker_hard_stalls=0 worker_release_enqueue_failures=0 max_worker_request_msec=10'
    echo 'sophia_live_wm_transport schema=2 status=complete peak_depth=1 pending=0 rejected=0 action_ordered=1 action_coalesced=0 stale_responses=0 max_queue_dwell_msec=1 max_round_trip_msec=10'
    echo 'sophia_live_session_health schema=1 status=clean protocol_errors=0 pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false'
    echo 'sophia_live_session schema=16 status=bounded_complete authority_batches_dropped=0 native_submit_failures=0 native_retire_failures=0 native_callback_rejected=0 native_in_flight=false native_cleanup_pending=false wm_policy=external wm_restarts=0 wm_degraded=false surface_resize=committed'
    echo 'sophia_live_session_cleanup schema=1 status=clean app_groups=0'
    echo 'sophia_qemu_guest schema=1 status=complete scenario=xmonad-resize-storm'
} >"$FIXTURE"

"$VERIFY" "$FIXTURE" >/dev/null

awk '!removed && /status=committed transaction=2000005 surface=3/ { removed=1; next } { print }' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "resize-storm verifier accepted a missing exact-pixel commit" >&2
    exit 1
fi

sed 's/transaction=2000006 surface=3 width=960 height=640 configure/transaction=2000006 surface=3 width=959 height=640 configure/' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "resize-storm verifier accepted nonmatching committed pixels" >&2
    exit 1
fi

sed '/status=complete steps=12 surface=3/i sophia_live_wm schema=1 status=layout_timeout transaction=2000004 preserved_layout=true rollback_transaction=8 rollback_configures=1 resize_state=pending' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "resize-storm verifier accepted resize timeout recovery" >&2
    exit 1
fi

sed '/status=post_storm_frame_retired/d' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "resize-storm verifier accepted no rendering after the storm" >&2
    exit 1
fi

sed 's/worker_completions=12/worker_completions=11/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "resize-storm verifier accepted renderer ownership debt" >&2
    exit 1
fi

grep -Fq 'xmonad-resize-storm' "$ROOT_DIR/tools/qemu_guest_init.sh"
grep -Fq -- '--inject-surface-resize-sequence=' "$ROOT_DIR/tools/qemu_guest_init.sh"
grep -Fq 'status=post_storm_frame_retired steps=12' "$ROOT_DIR/tools/qemu_session_harness.sh"
grep -Fq 'qemu_xmonad_resize_storm_acceptance.sh' "$ROOT_DIR/tools/check_atomic_scanout_local.sh"

echo "QEMU xmonad resize-storm verifier regressions passed."
