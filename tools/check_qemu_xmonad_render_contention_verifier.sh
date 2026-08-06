#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_qemu_xmonad_render_contention_evidence.sh"
FIXTURE="$(mktemp)"
MUTATION="$(mktemp)"
trap 'rm -f -- "$FIXTURE" "$MUTATION"' EXIT

{
    echo 'sophia_qemu_xmonad schema=2 status=starting isolation=headless control=qmp-unix profile=xmonad windows=3 gpu_mode=virgl host_render_node=explicit'
    echo 'sophia_qemu_topology schema=1 status=observed requested_heads=2 connectors=2 connected=2'
    echo 'sophia_live_session_mode schema=1 mode=normal configured_apps=4 startup_apps=2'
    echo 'sophia_session_app schema=1 status=started id=gpu1 source=startup'
    echo 'sophia_session_app schema=1 status=started id=statusbar source=startup'
    echo 'sophia_live_work_area schema=1 status=reduced outputs=2 changed=2 rejected=0 active_reservations=1'
    echo 'sophia_live_visual_candidate_identity schema=1 status=selected transaction=1 surface=11 source=dma_buf buffer=1'
    echo 'sophia_qemu_render_contention schema=1 status=started producers=1 cpu_bar=xmobar gpu=virgl'
    echo 'sophia_qemu_render_contention schema=1 status=launch_begin chord=meta_l+ret app=gpu2'
    echo 'sophia_session_app schema=2 status=started id=gpu2 source=action transaction=2'
    echo 'sophia_live_visual_candidate_identity schema=1 status=selected transaction=2 surface=22 source=dma_buf buffer=2'
    echo 'sophia_session_app schema=2 status=admitted source=action transaction=2 surface=22'
    echo 'sophia_qemu_render_contention schema=1 status=producer_ready app=gpu2 producers=2'
    echo 'sophia_qemu_render_contention schema=1 status=launch_begin chord=meta_l+p app=gpu3'
    echo 'sophia_session_app schema=2 status=started id=gpu3 source=action transaction=3'
    echo 'sophia_live_visual_candidate_identity schema=1 status=selected transaction=3 surface=33 source=dma_buf buffer=3'
    echo 'sophia_session_app schema=2 status=admitted source=action transaction=3 surface=33'
    echo 'sophia_qemu_render_contention schema=1 status=producer_ready app=gpu3 producers=3'
    echo 'sophia_qemu_render_contention schema=1 status=window_started producers=3 minimum_frames=30'
    for frame in $(seq 1 30); do
        for surface in 11 22 33; do
            transaction=$((frame * 10 + surface))
            echo "sophia_live_session_present schema=2 status=retired transaction=$transaction surface=$surface source=640x390 target=640x390_0_0 clip=640x390_0_0 unit_scale=true"
        done
    done
    echo 'sophia_qemu_render_contention schema=1 status=window_complete producers=3 dmabuf_surfaces=3 minimum_retirements=30 retirements=90'
    echo 'sophia_live_rendering_efficiency schema=1 status=complete cpu_updates=8 cpu_replacements=1 cpu_patch_updates=7 cpu_patch_rects=7 cpu_payload_bytes=4096 exact_pixel_metric_frames=3 damage_scoped_metric_frames=90 composition_target_reuses=89'
    echo 'sophia_live_native_resources schema=5 status=complete target_creations=90 pipeline_creations=90 frame_surface_creations=90 cpu_target_creations=0 dmabuf_target_creations=90 composition_target_creations=1 composition_target_reuses=89 generation_replacements=0 recovery_replacements=0 snapshot_captures=90 snapshot_promotions=89 snapshot_rollbacks=1 snapshot_evictions=90 snapshot_live_entries=0 snapshot_live_bytes=0 import_cache_imports=90 import_cache_hits=90 import_cache_evictions=90 import_cache_live_entries=0 import_cache_descriptor_mismatches=0 import_cache_capacity_rejections=0 worker_requests=92 worker_completions=92 worker_failures=0 worker_soft_stalls=0 worker_hard_stalls=0 worker_release_enqueue_failures=0 max_worker_request_msec=50'
    echo 'sophia_live_present_cadence schema=1 status=complete samples=90 advancing_intervals=89 nonadvancing=0 overflowed=false mean_fps=180.000 p95_frame_msec=6.000'
    echo 'sophia_live_session_control schema=1 status=complete enqueued=9 dispatched=9 delivered=9 rejected=0 timed_out=0 unexpected=0 pending=0 peak_depth=2 max_queue_dwell_msec=2 max_ack_msec=80'
    echo 'sophia_live_session schema=16 status=bounded_complete authority_batches_dropped=0 native_submit_failures=0 native_retire_failures=0 native_callback_rejected=0 native_callback_queue_saturated=0 native_in_flight=false native_cleanup_pending=false wm_policy=external wm_restarts=0 wm_degraded=false present_live_sources=0 present_live_fences=0 present_live_transactions=0 present_controlled_rejections=0'
    echo 'sophia_live_output schema=1 status=complete output=1 checksum=10 submissions=91 retirements=90 callbacks=90 nonzero_exports=90'
    echo 'sophia_live_output schema=1 status=complete output=2 checksum=20 submissions=1 retirements=0 callbacks=0 nonzero_exports=1'
    echo 'sophia_live_session_health schema=1 status=clean protocol_errors=0 pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false'
    echo 'sophia_live_layout_health schema=1 status=clean recovery_extents=0 constraint_relayout_pending=false'
    echo 'sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=999'
    echo 'sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 namespace=revoked xauthority=removed'
    echo 'sophia_qemu_guest schema=1 status=complete scenario=xmonad-render-contention'
} >"$FIXTURE"

"$VERIFY" "$FIXTURE" >/dev/null

awk '!($0 ~ /status=retired/ && $0 ~ /surface=33/)' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "render-contention verifier accepted a starved producer" >&2
    exit 1
fi

awk '
    /status=window_complete/ {
        for (frame = 1; frame <= 3; frame++) {
            transaction = 9000 + frame
            print "sophia_live_session_present schema=2 status=retired transaction=" transaction " surface=11 source=640x390 target=640x390_0_0 clip=640x390_0_0 unit_scale=true"
        }
        sub(/retirements=90/, "retirements=93")
    }
    { print }
' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "render-contention verifier accepted excessive producer-service skew" >&2
    exit 1
fi

sed 's/minimum_retirements=30 retirements=90/minimum_retirements=29 retirements=90/' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "render-contention verifier accepted false window accounting" >&2
    exit 1
fi

sed 's/import_cache_hits=90/import_cache_hits=0/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "render-contention verifier accepted no import-cache reuse" >&2
    exit 1
fi

sed 's/worker_completions=92/worker_completions=91/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "render-contention verifier accepted renderer-worker ownership debt" >&2
    exit 1
fi

sed 's/max_worker_request_msec=50/max_worker_request_msec=101/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "render-contention verifier accepted an over-budget renderer request" >&2
    exit 1
fi

sed '/status=window_started/a sophia_live_wm schema=1 status=layout_timeout transaction=8 preserved_layout=true' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "render-contention verifier accepted layout recovery" >&2
    exit 1
fi

sed 's/cpu_patch_updates=7/cpu_patch_updates=0/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "render-contention verifier accepted an inactive CPU bar" >&2
    exit 1
fi

sed 's/output=2 checksum=20 submissions=1 retirements=0 callbacks=0/output=2 checksum=20 submissions=91 retirements=90 callbacks=90/' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "render-contention verifier accepted two active outputs" >&2
    exit 1
fi

grep -Fq 'xmonad-render-contention' "$ROOT_DIR/tools/qemu_guest_init.sh"
grep -Fq 'status=window_started producers=3 minimum_frames=30' \
    "$ROOT_DIR/tools/qemu_session_harness.sh"
grep -Fq 'verify_qemu_xmonad_render_contention_evidence.sh' \
    "$ROOT_DIR/tools/qemu_session_harness.sh"
grep -Fq 'qemu_xmonad_render_contention_acceptance.sh' \
    "$ROOT_DIR/tools/check_atomic_scanout_local.sh"

echo "QEMU xmonad render-contention verifier regressions passed."
