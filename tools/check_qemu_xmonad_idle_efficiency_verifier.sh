#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_qemu_xmonad_idle_efficiency_evidence.sh"
FIXTURE="$(mktemp)"
MUTATION="$(mktemp)"
trap 'rm -f -- "$FIXTURE" "$MUTATION"' EXIT

{
    echo 'sophia_qemu_xmonad schema=2 status=starting isolation=headless control=qmp-unix profile=xmonad windows=2 gpu_mode=virgl host_render_node=explicit'
    echo 'sophia_qemu_topology schema=1 status=observed requested_heads=2 connectors=2 connected=2'
    echo 'sophia_live_session_mode schema=1 mode=normal configured_apps=2 startup_apps=1'
    echo 'sophia_session_app schema=1 status=started id=cpu source=startup'
    echo 'sophia_qemu_idle_efficiency schema=1 status=launch_begin chord=meta_l+p app=gpu'
    echo 'sophia_session_app schema=2 status=started id=gpu source=action transaction=2'
    echo 'sophia_session_app schema=2 status=admitted source=action transaction=2 surface=22'
    for frame in $(seq 1 10); do
        echo "sophia_live_session_present schema=2 status=retired transaction=$frame surface=22 source=636x796 target=636x796_642_2 clip=636x796_642_2 unit_scale=true"
    done
    echo 'sophia_qemu_idle_client schema=1 status=frozen producer=glxgears'
    echo 'sophia_qemu_idle_efficiency schema=1 status=producer_quiescent surfaces=1 retirements=10 stable_msec=1000'
    echo 'sophia_qemu_idle_efficiency schema=1 status=reuse_window_started focus_transitions=256'
    for transition in $(seq 1 256); do
        echo "sophia_live_wm schema=1 status=physical_action_committed action=$transition"
        echo "sophia_live_native_page_flip schema=1 status=submitted output=1 submission=$transition content=Some(RetainedMixed { frame: LiveProductionNativeFrameId($transition), nonzero_rgb_pixels: 1024 }) frame=$transition"
        echo "sophia_live_output_repaint schema=1 status=presented output=1 mode=partial rects=2 pixels=4096"
        echo "sophia_live_native_page_flip schema=1 status=retired output=1 submission=$transition frame=$transition"
    done
    echo 'sophia_qemu_idle_efficiency schema=1 status=reuse_window_complete focus_transitions=256 actions=256 repaints=256 partial_repaints=256 flips=256 producer_retirements=10'
    echo 'sophia_qemu_idle_efficiency schema=1 status=idle_window_started duration_msec=2000'
    echo 'sophia_qemu_idle_efficiency schema=1 status=idle_window_complete duration_msec=2000 repaints=0 page_flips=0 client_presents=0'
    echo 'sophia_qemu_idle_efficiency schema=1 status=logout_begin chord=meta_l+shift+q'
    echo 'sophia_live_native_resources schema=5 status=complete target_creations=268 pipeline_creations=268 frame_surface_creations=268 cpu_target_creations=0 dmabuf_target_creations=10 composition_target_creations=1 composition_target_reuses=260 generation_replacements=0 recovery_replacements=0 snapshot_captures=10 snapshot_promotions=10 snapshot_rollbacks=0 snapshot_evictions=10 snapshot_live_entries=0 snapshot_live_bytes=0 import_cache_imports=10 import_cache_hits=300 import_cache_evictions=10 import_cache_live_entries=0 import_cache_descriptor_mismatches=0 import_cache_capacity_rejections=0 worker_requests=270 worker_completions=270 worker_failures=0 worker_soft_stalls=0 worker_hard_stalls=0 worker_release_enqueue_failures=0 max_worker_request_msec=20'
    echo 'sophia_live_rendering_efficiency schema=1 status=complete cpu_updates=2 cpu_replacements=1 cpu_patch_updates=1 cpu_patch_rects=1 cpu_payload_bytes=4096 exact_pixel_metric_frames=2 damage_scoped_metric_frames=266 composition_target_reuses=260'
    echo 'sophia_live_session schema=16 status=bounded_complete authority_batches_dropped=0 native_submissions=268 native_submit_failures=0 native_retirements=267 native_retire_failures=0 native_frame_uploads=3 native_callback_rejected=0 native_callback_queue_saturated=0 native_in_flight=false native_cleanup_pending=false wm_policy=external wm_restarts=0 wm_degraded=false present_live_sources=0 present_live_fences=0 present_live_transactions=0 present_controlled_rejections=0'
    echo 'sophia_live_session_control schema=1 status=complete enqueued=259 dispatched=259 delivered=259 rejected=0 timed_out=0 unexpected=0 pending=0 peak_depth=1 max_queue_dwell_msec=1 max_ack_msec=2'
    echo 'sophia_live_output schema=1 status=complete output=1 checksum=10 submissions=268 retirements=267 callbacks=267 nonzero_exports=268'
    echo 'sophia_live_output schema=1 status=complete output=2 checksum=20 submissions=1 retirements=0 callbacks=0 nonzero_exports=1'
    echo 'sophia_live_session_health schema=1 status=clean protocol_errors=0 pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false'
    echo 'sophia_live_layout_health schema=2 status=clean recovery_extents=0 standing_targets=0 constraint_relayout_pending=false'
    echo 'sophia_live_session_protocol_errors schema=1 expected=0 unexpected=0'
    echo 'sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=11'
    echo 'sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 namespace=revoked xauthority=removed'
    echo 'sophia_qemu_guest schema=1 status=complete scenario=xmonad-idle-efficiency'
} >"$FIXTURE"

"$VERIFY" "$FIXTURE" >/dev/null

sed 's/native_frame_uploads=3/native_frame_uploads=2/' "$FIXTURE" >"$MUTATION"
"$VERIFY" "$MUTATION" >/dev/null

awk '
    !removed && /^sophia_live_wm schema=1 status=physical_action_committed / {
        removed = 1
        next
    }
    { print }
' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "idle-efficiency verifier accepted a missing focus transition" >&2
    exit 1
fi

sed '0,/mode=partial/s//mode=full/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "idle-efficiency verifier accepted a full retained repaint" >&2
    exit 1
fi

sed '/status=idle_window_started/a sophia_live_output_repaint schema=1 status=presented output=1 mode=partial rects=1 pixels=64' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "idle-efficiency verifier accepted hidden idle recomposition" >&2
    exit 1
fi

sed '/status=producer_quiescent/a sophia_live_session_present schema=2 status=retired transaction=99 surface=22 source=636x796 target=636x796_642_2 clip=636x796_642_2 unit_scale=true' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "idle-efficiency verifier accepted producer progress after quiescence" >&2
    exit 1
fi

sed 's/import_cache_hits=300/import_cache_hits=10/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "idle-efficiency verifier accepted a non-majority cache hit rate" >&2
    exit 1
fi

sed 's/native_frame_uploads=3/native_frame_uploads=4/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "idle-efficiency verifier accepted a redundant full-frame CPU upload" >&2
    exit 1
fi

sed '0,/content=Some(RetainedMixed/s//content=Some(Cpu/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "idle-efficiency verifier accepted a CPU submission in the retained window" >&2
    exit 1
fi

sed 's/worker_completions=270/worker_completions=269/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "idle-efficiency verifier accepted renderer-worker ownership debt" >&2
    exit 1
fi

sed 's/output=2 checksum=20 submissions=1 retirements=0 callbacks=0/output=2 checksum=20 submissions=268 retirements=267 callbacks=267/' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "idle-efficiency verifier accepted work on the baseline-only output" >&2
    exit 1
fi

sed 's/status=clean app_groups=0/status=clean app_groups=1/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "idle-efficiency verifier accepted dirty application cleanup" >&2
    exit 1
fi

grep -Fq 'xmonad-idle-efficiency' "$ROOT_DIR/tools/qemu_guest_init.sh"
grep -Fq 'status=idle_window_started duration_msec=2000' \
    "$ROOT_DIR/tools/qemu_session_harness.sh"
grep -Fq 'verify_qemu_xmonad_idle_efficiency_evidence.sh' \
    "$ROOT_DIR/tools/qemu_session_harness.sh"
grep -Fq 'qemu_xmonad_idle_efficiency_acceptance.sh' \
    "$ROOT_DIR/tools/check_atomic_scanout_local.sh"

echo "QEMU xmonad idle-efficiency verifier regressions passed."
