#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_sophia_xmonad_four_kitty.sh"
FIXTURE_DIR="$(mktemp -d)"
trap 'rm -rf "$FIXTURE_DIR"' EXIT

valid="$FIXTURE_DIR/valid.log"
cat >"$valid" <<'EOF'
sophia_session_app schema=1 status=started id=terminal source=startup
sophia_session_app schema=1 status=started id=terminal source=action
sophia_session_app schema=1 status=started id=terminal source=action
sophia_session_app schema=1 status=started id=terminal source=action
sophia_live_native_startup_output schema=1 status=presented output=1 proof=synchronous_modeset submission=1
sophia_live_native_startup_output schema=1 status=presented output=2 proof=synchronous_modeset submission=1
sophia_live_session_startup schema=2 status=output_baseline_ready outputs=2/2
sophia_live_resize_epoch schema=1 status=held transaction=4 surfaces=4
sophia_live_resize_epoch schema=1 status=committed transaction=4 matched_surfaces=4
sophia_live_session_present schema=2 status=retired transaction=10 surface=1 source=1280x1440 target=1280x1440_0_0 clip=1280x1440_0_0 unit_scale=true
sophia_live_session_present schema=2 status=retired transaction=11 surface=2 source=1280x480 target=1280x480_1280_0 clip=1280x480_1280_0 unit_scale=true
sophia_live_session_present schema=2 status=retired transaction=12 surface=3 source=1280x480 target=1280x480_1280_480 clip=1280x480_1280_480 unit_scale=true
sophia_live_session_present schema=2 status=retired transaction=13 surface=4 source=1280x480 target=1280x480_1280_960 clip=1280x480_1280_960 unit_scale=true
sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=none
sophia_live_session_health schema=1 status=clean protocol_errors=0 pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false
sophia_live_session_protocol_errors schema=1 expected=0 unexpected=0
sophia_live_session_control schema=1 status=complete enqueued=10 dispatched=10 delivered=10 rejected=0 timed_out=0 unexpected=0 pending=0 peak_depth=3 max_queue_dwell_msec=4 max_ack_msec=8
sophia_live_session_keys schema=1 status=complete pending=0 release_barrier_pending=0 peak_pressed=3 synthetic_releases=2 state_only_releases=1 orphan_releases_suppressed=1 removed_surface_keys=0
sophia_session_launches schema=1 status=complete peak_depth=3 rejected=0 admission_timeouts=0
sophia_live_owner_timing schema=2 status=complete max_child_reap_msec=25 max_input_phase_msec=12
sophia_live_wm_transport schema=1 status=complete peak_depth=2 pending=0 rejected=0 stale_responses=0 max_queue_dwell_msec=12 max_round_trip_msec=180
sophia_live_native_resources schema=2 status=complete target_creations=64 pipeline_creations=64 frame_surface_creations=64 cpu_target_creations=0 dmabuf_target_creations=0 composition_target_creations=64 generation_replacements=0 recovery_replacements=0
sophia_live_session schema=15 status=bounded_complete native_submit_failures=0 native_retire_failures=0 native_callback_rejected=0 native_callback_queue_saturated=0 native_in_flight=false native_cleanup_pending=false present_disconnect_failures=0 present_live_sources=0 present_live_fences=0 present_live_transactions=0 native_mixed_exports=64 native_target_recreations=64 native_frame_surface_creations=64 native_max_target_create_msec=14 native_max_frame_surface_create_msec=4 native_max_render_msec=10 native_max_submit_to_page_flip_msec=20 native_max_upload_msec=8 input_queue_dwell_max_msec=12
sophia_live_output schema=1 status=complete output=1 checksum=1 submissions=10 retirements=9 callbacks=9 nonzero_exports=1
sophia_live_output schema=1 status=complete output=2 checksum=2 submissions=7 retirements=6 callbacks=6 nonzero_exports=1
sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 namespace=revoked xauthority=removed
EOF

SOPHIA_VERIFY_WAIT_SECONDS=0 "$VERIFY" "$valid" >/dev/null

expect_rejected() {
    local name="$1"
    local expression="$2"
    local replacement="$3"
    local mutated="$FIXTURE_DIR/$name.log"
    sed "s|$expression|$replacement|" "$valid" >"$mutated"
    if SOPHIA_VERIFY_WAIT_SECONDS=0 "$VERIFY" "$mutated" >/dev/null 2>&1; then
        echo "four-Kitty verifier mutation was accepted: $name" >&2
        exit 1
    fi
}

expect_rejected undrained 'outcome=drained drained=true' \
    'outcome=forced_detach_timeout drained=false'
expect_rejected abandoned 'abandoned_scanouts=0' 'abandoned_scanouts=1'
expect_rejected empty_submission \
    'sophia_live_session_native_suspend' \
    'sophia_live_native_page_flip schema=1 status=submitted output=2 submission=8 content=None\nsophia_live_session_native_suspend'
expect_rejected retirement_imbalance \
    'submissions=7 retirements=6 callbacks=6' \
    'submissions=7 retirements=5 callbacks=5'
expect_rejected callback_imbalance \
    'submissions=7 retirements=6 callbacks=6' \
    'submissions=7 retirements=6 callbacks=5'
expect_rejected incomplete \
    'sophia_live_session_cleanup schema=1 status=clean' \
    'sophia_live_session_cleanup schema=1 status=stalled'
expect_rejected missing_startup_output \
    'sophia_live_native_startup_output schema=1 status=presented output=2 proof=synchronous_modeset submission=1' \
    'sophia_live_native_startup_output schema=1 status=missing output=2 proof=none submission=0'
expect_rejected unsafe_epoch_reuse \
    'native_target_recreations=64' \
    'native_target_recreations=63'
expect_rejected partial_resize_epoch \
    'matched_surfaces=4' \
    'matched_surfaces=3'
expect_rejected oversized_resize_epoch \
    'status=held transaction=4 surfaces=4' \
    'status=held transaction=4 surfaces=5'
expect_rejected surface_count_mismatch \
    'native_frame_surface_creations=64' \
    'native_frame_surface_creations=63'
expect_rejected recovery_replacement \
    'recovery_replacements=0' \
    'recovery_replacements=1'
expect_rejected excessive_input_dwell \
    'input_queue_dwell_max_msec=12' \
    'input_queue_dwell_max_msec=101'
expect_rejected excessive_surface_create \
    'native_max_frame_surface_create_msec=4' \
    'native_max_frame_surface_create_msec=101'
expect_rejected blocking_wm_request \
    'max_round_trip_msec=180' \
    'max_round_trip_msec=501'
expect_rejected stalled_wm_queue \
    'max_queue_dwell_msec=12' \
    'max_queue_dwell_msec=501'
expect_rejected pending_wm_request \
    'peak_depth=2 pending=0 rejected=0' \
    'peak_depth=2 pending=1 rejected=0'
expect_rejected rejected_wm_request \
    'peak_depth=2 pending=0 rejected=0' \
    'peak_depth=2 pending=0 rejected=1'
expect_rejected blocking_input_phase \
    'max_input_phase_msec=12' \
    'max_input_phase_msec=101'
expect_rejected blocking_child_reap \
    'max_child_reap_msec=25' \
    'max_child_reap_msec=101'
expect_rejected admission_timeout \
    'admission_timeouts=0' \
    'admission_timeouts=1'
expect_rejected control_rejection \
    'delivered=10 rejected=0' \
    'delivered=9 rejected=1'
expect_rejected control_latency \
    'max_ack_msec=8' \
    'max_ack_msec=101'
expect_rejected pressed_key_debt \
    'sophia_live_session_keys schema=1 status=complete pending=0' \
    'sophia_live_session_keys schema=1 status=complete pending=1'

echo "four-Kitty verifier mutation checks passed"
