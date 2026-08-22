#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_qemu_xmonad_producer_overload_evidence.sh"
FIXTURE="$(mktemp)"
MUTATION="$(mktemp)"
trap 'rm -f -- "$FIXTURE" "$MUTATION"' EXIT

{
    echo 'sophia_qemu_xmonad schema=2 status=starting isolation=headless control=qmp-unix profile=xmonad windows=2 gpu_mode=virgl host_render_node=explicit'
    echo 'sophia_qemu_topology schema=1 status=observed requested_heads=2 connectors=2 connected=2'
    echo 'sophia_qemu_xmonad schema=1 status=running windows=2 profile=xmonad mode=producer-overload producer=bounded-dri3-present interval_usec=5000 cpu_client=xterm'
    echo 'sophia_live_session_mode schema=1 mode=normal configured_apps=2 startup_apps=1'
    echo 'sophia_live_x11_route_capacity schema=1 input=64 control=32 protocol=512 presentations=64'
    echo 'sophia_session_app schema=1 status=started id=cpu source=startup'
    echo 'sophia_qemu_producer_overload schema=1 status=launch_begin chord=meta_l+p app=gpu'
    echo 'sophia_session_app schema=2 status=started id=gpu source=action transaction=2'
    echo 'sophia_session_app schema=2 status=admitted source=action transaction=2 surface=22'
    echo 'sophia_qemu_overload_client schema=1 status=running buffers=3 interval_usec=5000 feedback=complete-idle'
    echo 'sophia_live_present_progress schema=1 complete_copy=20 complete_flip=0 complete_skip=2 idle=22'
    echo 'sophia_qemu_producer_overload schema=1 status=warmup_complete copies=20 skips=2'
    echo 'sophia_qemu_producer_overload schema=1 status=window_started duration_msec=10000 phases=2'
    echo 'sophia_qemu_producer_overload schema=1 status=phase_complete phase=1 duration_msec=5000 copies=25 skips=5 submissions=20 retirements=20'
    for frame in $(seq 1 40); do
        echo "sophia_live_native_page_flip schema=1 status=submitted output=1 submission=$frame content=Some(MixedPresent) frame=$frame"
        echo "sophia_live_native_page_flip schema=1 status=retired output=1 submission=$frame frame=$frame"
    done
    echo 'sophia_qemu_producer_overload schema=1 status=phase_complete phase=2 duration_msec=5000 copies=25 skips=5 submissions=20 retirements=20'
    echo 'sophia_qemu_producer_overload schema=1 status=window_complete duration_msec=10000 phases=2'
    echo 'sophia_qemu_producer_overload schema=1 status=logout_begin chord=meta_l+shift+q'
    echo 'sophia_live_present_scheduler schema=1 status=complete surface_content_capacity=256 pending_limit=1 in_flight_limit=1 pending_supersessions=12 surface_content_supersessions=12 scheduler_supersessions=0 max_surface_content_deferred=21 max_latest_deferred_per_surface=1 max_pending_queued=1 max_total_queued=1 max_live_sources=3 max_live_fences=3 max_live_presentations=2 present_rejections=18 native_suspend_present_rejections=1 shutdown_present_rejections=2 other_present_rejections=1'
    echo 'sophia_live_native_resources schema=5 status=complete target_creations=40 pipeline_creations=40 frame_surface_creations=40 cpu_target_creations=0 dmabuf_target_creations=40 composition_target_creations=1 composition_target_reuses=39 generation_replacements=0 recovery_replacements=0 snapshot_captures=40 snapshot_promotions=40 snapshot_rollbacks=0 snapshot_evictions=40 snapshot_live_entries=0 snapshot_live_bytes=0 import_cache_imports=40 import_cache_hits=10 import_cache_evictions=40 import_cache_live_entries=0 import_cache_descriptor_mismatches=0 import_cache_capacity_rejections=0 worker_requests=42 worker_completions=42 worker_failures=0 worker_soft_stalls=0 worker_hard_stalls=0 worker_release_enqueue_failures=0 max_worker_request_msec=20'
    echo 'sophia_live_present_cadence schema=1 status=complete samples=70 advancing_intervals=69 nonadvancing=0 overflowed=false mean_fps=60.000 p95_frame_msec=16.667'
    echo 'sophia_live_present_progress schema=1 complete_copy=70 complete_flip=0 complete_skip=18 idle=88'
    echo 'sophia_live_session schema=16 status=bounded_complete authority_batches_dropped=0 native_submissions=41 native_submit_failures=0 native_retirements=40 native_retire_failures=0 native_callback_rejected=0 native_callback_queue_saturated=0 native_in_flight=false native_cleanup_pending=false wm_policy=external wm_restarts=0 wm_degraded=false present_complete_copy=70 present_complete_flip=0 present_complete_skip=18 present_idle=88 present_complete_routed=88 present_idle_routed=88 present_route_failures=0 present_live_sources=0 present_live_fences=0 present_live_transactions=0 present_controlled_rejections=2'
    echo 'sophia_live_session_control schema=2 status=complete enqueued=3 dispatched=3 delivered=2 stale_retired=1 rejected=0 timed_out=0 unexpected=0 pending=0 peak_depth=1 max_queue_dwell_msec=1 max_ack_msec=2'
    echo 'sophia_live_output schema=1 status=complete output=1 checksum=10 submissions=41 retirements=40 callbacks=40 nonzero_exports=41'
    echo 'sophia_live_output schema=1 status=complete output=2 checksum=20 submissions=1 retirements=0 callbacks=0 nonzero_exports=1'
    echo 'sophia_live_session_health schema=1 status=clean protocol_errors=0 pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false'
    echo 'sophia_live_layout_health schema=2 status=clean recovery_extents=0 standing_targets=0 constraint_relayout_pending=false'
    echo 'sophia_live_session_protocol_errors schema=1 expected=0 unexpected=0'
    echo 'sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=none'
    echo 'sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 namespace=revoked xauthority=removed'
    echo 'sophia_qemu_guest schema=1 status=complete scenario=xmonad-producer-overload'
} >"$FIXTURE"

"$VERIFY" "$FIXTURE" >/dev/null

sed '0,/skips=5/s//skips=0/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "producer-overload verifier accepted a phase without frame dropping" >&2
    exit 1
fi

sed 's/protocol=512/protocol=64/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "producer-overload verifier accepted an input-sized Present feedback route" >&2
    exit 1
fi

sed 's/complete_copy=70 complete_flip=0 complete_skip=18 idle=88/complete_copy=19 complete_flip=0 complete_skip=18 idle=88/' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "producer-overload verifier accepted regressing cumulative Present progress" >&2
    exit 1
fi

sed '/status=submitted output=1 submission=1/a sophia_live_native_page_flip schema=1 status=submitted output=1 submission=99 content=Some(MixedPresent) frame=99' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "producer-overload verifier accepted overlapping KMS submissions" >&2
    exit 1
fi

sed 's/max_pending_queued=1/max_pending_queued=2/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "producer-overload verifier accepted two pending frames" >&2
    exit 1
fi

sed 's/max_surface_content_deferred=21/max_surface_content_deferred=257/' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "producer-overload verifier accepted an over-capacity authority backlog" >&2
    exit 1
fi

sed 's/max_latest_deferred_per_surface=1/max_latest_deferred_per_surface=2/' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "producer-overload verifier accepted two replaceable deferred frames" >&2
    exit 1
fi

sed 's/max_live_presentations=2/max_live_presentations=3/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "producer-overload verifier accepted excess Present ownership" >&2
    exit 1
fi

sed 's/pending_supersessions=12/pending_supersessions=11/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "producer-overload verifier accepted mismatched Skip feedback" >&2
    exit 1
fi

sed 's/surface_content_supersessions=12/surface_content_supersessions=11/' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "producer-overload verifier accepted inconsistent queue supersessions" >&2
    exit 1
fi

sed 's/shutdown_present_rejections=2/shutdown_present_rejections=1/' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "producer-overload verifier accepted unaccounted shutdown feedback" >&2
    exit 1
fi

sed 's/present_rejections=18/present_rejections=17/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "producer-overload verifier accepted unbalanced total rejection ownership" >&2
    exit 1
fi

sed 's/other_present_rejections=1/other_present_rejections=2/' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "producer-overload verifier accepted excess uncategorized rejections" >&2
    exit 1
fi

sed 's/worker_completions=42/worker_completions=41/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "producer-overload verifier accepted renderer-worker debt" >&2
    exit 1
fi

sed 's/present_live_sources=0/present_live_sources=1/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "producer-overload verifier accepted live DMA-BUF ownership" >&2
    exit 1
fi

sed 's/present_complete_routed=88/present_complete_routed=87/' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "producer-overload verifier accepted incomplete client-visible feedback" >&2
    exit 1
fi

sed 's/output=2 checksum=20 submissions=1 retirements=0 callbacks=0/output=2 checksum=20 submissions=41 retirements=40 callbacks=40/' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "producer-overload verifier accepted work on the baseline-only output" >&2
    exit 1
fi

sed 's/status=clean app_groups=0/status=clean app_groups=1/' "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "producer-overload verifier accepted dirty application cleanup" >&2
    exit 1
fi

grep -Fq 'xmonad-producer-overload' "$ROOT_DIR/tools/qemu_guest_init.sh"
grep -Fq 'status=window_started duration_msec=10000 phases=2' \
    "$ROOT_DIR/tools/qemu_session_harness.sh"
grep -Fq 'verify_qemu_xmonad_producer_overload_evidence.sh' \
    "$ROOT_DIR/tools/qemu_session_harness.sh"

echo "QEMU xmonad producer-overload verifier regressions passed."
