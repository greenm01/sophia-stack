#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_FILE="${1:-/tmp/sophia-qemu-xmonad-render-contention.log}"

fail() {
    echo "QEMU xmonad render-contention verification failed: $*" >&2
    exit 1
}

count() {
    grep -Ec "$1" "$EVIDENCE_FILE" || true
}

field() {
    local line="$1" key="$2" token
    for token in $line; do
        if [[ "$token" == "$key="* ]]; then
            printf '%s\n' "${token#*=}"
            return 0
        fi
    done
    return 1
}

line_number() {
    local pattern=$1
    awk -v pattern="$pattern" '$0 ~ pattern { print NR; exit }' "$EVIDENCE_FILE"
}

[[ -s "$EVIDENCE_FILE" ]] || fail "missing evidence: $EVIDENCE_FILE"
if grep -Eq \
    '(^Error:|panicked at|mismatched.transaction|status=(failed|degraded)([[:space:]]|$)|sophia_live_wm schema=1 status=layout_timeout |sophia_live_resize_epoch schema=1 status=aborted )' \
    "$EVIDENCE_FILE"; then
    fail "evidence contains an error, timeout, rollback, or degraded result"
fi
if grep -Eq '^sophia_session_app schema=1 status=exited id=(gpu1|gpu2|gpu3|statusbar) ' \
    "$EVIDENCE_FILE"; then
    fail "a contention application exited before session teardown"
fi

grep -Fxq \
    'sophia_qemu_xmonad schema=2 status=starting isolation=headless control=qmp-unix profile=xmonad windows=3 gpu_mode=virgl host_render_node=explicit' \
    "$EVIDENCE_FILE" || fail "the explicit virgl profile is missing"
grep -Fxq \
    'sophia_qemu_topology schema=1 status=observed requested_heads=2 connectors=2 connected=2' \
    "$EVIDENCE_FILE" || fail "the two-output guest topology is missing"
grep -Fxq \
    'sophia_live_session_mode schema=1 mode=normal configured_apps=4 startup_apps=2' \
    "$EVIDENCE_FILE" || fail "the isolated contention application set is missing"
grep -Eq '^sophia_live_work_area schema=1 status=reduced outputs=2 .*active_reservations=1$' \
    "$EVIDENCE_FILE" || fail "the CPU-composited bar did not reserve both work areas"
grep -Fxq 'sophia_session_app schema=1 status=started id=gpu1 source=startup' \
    "$EVIDENCE_FILE" || fail "the first DMA-BUF producer did not start"
grep -Fxq 'sophia_session_app schema=1 status=started id=statusbar source=startup' \
    "$EVIDENCE_FILE" || fail "xmobar did not start"
(( $(count '^sophia_session_app schema=2 status=admitted source=action ') == 2 )) ||
    fail "the two serialized producers were not admitted exactly once"

gpu1_line="$(line_number '^sophia_session_app schema=1 status=started id=gpu1 source=startup$')"
started_line="$(line_number '^sophia_qemu_render_contention schema=1 status=started producers=1 ')"
gpu2_begin_line="$(line_number '^sophia_qemu_render_contention schema=1 status=launch_begin chord=meta_l[+]ret app=gpu2$')"
gpu2_start_line="$(line_number '^sophia_session_app schema=2 status=started id=gpu2 source=action ')"
gpu2_ready_line="$(line_number '^sophia_qemu_render_contention schema=1 status=producer_ready app=gpu2 producers=2$')"
gpu3_begin_line="$(line_number '^sophia_qemu_render_contention schema=1 status=launch_begin chord=meta_l[+]p app=gpu3$')"
gpu3_start_line="$(line_number '^sophia_session_app schema=2 status=started id=gpu3 source=action ')"
gpu3_ready_line="$(line_number '^sophia_qemu_render_contention schema=1 status=producer_ready app=gpu3 producers=3$')"
window_start_line="$(line_number '^sophia_qemu_render_contention schema=1 status=window_started producers=3 minimum_frames=30$')"
window_complete_line="$(line_number '^sophia_qemu_render_contention schema=1 status=window_complete producers=3 ' )"
for value in \
    "$gpu1_line" "$started_line" "$gpu2_begin_line" "$gpu2_start_line" \
    "$gpu2_ready_line" "$gpu3_begin_line" "$gpu3_start_line" "$gpu3_ready_line" \
    "$window_start_line" "$window_complete_line"; do
    [[ "$value" =~ ^[1-9][0-9]*$ ]] || fail "the producer launch sequence is incomplete"
done
(( gpu1_line < started_line \
    && started_line < gpu2_begin_line \
    && gpu2_begin_line < gpu2_start_line \
    && gpu2_start_line < gpu2_ready_line \
    && gpu2_ready_line < gpu3_begin_line \
    && gpu3_begin_line < gpu3_start_line \
    && gpu3_start_line < gpu3_ready_line \
    && gpu3_ready_line < window_start_line \
    && window_start_line < window_complete_line )) ||
    fail "producer launch, admission, and contention markers are out of order"

awk '
    function value(key, i, pair) {
        for (i = 1; i <= NF; i++) {
            split($i, pair, "=")
            if (pair[1] == key) return pair[2]
        }
        return ""
    }
    /^sophia_qemu_render_contention schema=1 status=window_started / {
        if (active || starts != 0) exit 1
        active = 1
        starts++
        next
    }
    active && /^sophia_live_session_present schema=2 status=retired / {
        surface = value("surface")
        if (surface !~ /^[0-9]+$/ || value("source") !~ /^[0-9]+x[0-9]+$/) exit 1
        if (value("target") !~ /^[0-9]+x[0-9]+_-?[0-9]+_-?[0-9]+$/) exit 1
        if (value("clip") == "none" || value("unit_scale") != "true") exit 1
        counts[surface]++
        next
    }
    /^sophia_qemu_render_contention schema=1 status=window_complete / {
        if (!active || completes != 0) exit 1
        active = 0
        completes++
        marker_surfaces = value("dmabuf_surfaces") + 0
        marker_minimum = value("minimum_retirements") + 0
        marker_total = value("retirements") + 0
    }
    END {
        if (active || starts != 1 || completes != 1) exit 1
        minimum = -1
        maximum = 0
        for (surface in counts) {
            surfaces++
            total += counts[surface]
            if (minimum < 0 || counts[surface] < minimum) minimum = counts[surface]
            if (counts[surface] > maximum) maximum = counts[surface]
        }
        if (surfaces != 3 || minimum < 30 || maximum - minimum > 2) exit 1
        if (marker_surfaces != surfaces || marker_minimum != minimum || marker_total != total) exit 1
    }
' "$EVIDENCE_FILE" || fail "the bounded window lacks fair, causal progress from three DMA-BUF surfaces"

(( $(awk '
    /^sophia_live_visual_candidate_identity schema=1 status=selected / && / source=dma_buf / {
        for (i = 1; i <= NF; i++) if ($i ~ /^surface=[0-9]+$/) seen[$i] = 1
    }
    END { for (surface in seen) count++; print count + 0 }
' "$EVIDENCE_FILE") == 3 )) || fail "three distinct DMA-BUF identities were not selected"

rendering="$(grep -E '^sophia_live_rendering_efficiency schema=1 status=complete ' "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$rendering" ]] || fail "rendering-efficiency completion is missing"
for key in cpu_updates cpu_replacements cpu_patch_updates cpu_payload_bytes composition_target_reuses; do
    value="$(field "$rendering" "$key")" || fail "rendering completion lacks $key"
    [[ "$value" =~ ^[1-9][0-9]*$ ]] || fail "$key did not prove active CPU-bar composition"
done

resources="$(grep -E '^sophia_live_native_resources schema=5 status=complete ' "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$resources" ]] || fail "native resource completion is missing"
imports="$(field "$resources" import_cache_imports)" || fail "resource completion lacks import_cache_imports"
hits="$(field "$resources" import_cache_hits)" || fail "resource completion lacks import_cache_hits"
evictions="$(field "$resources" import_cache_evictions)" || fail "resource completion lacks import_cache_evictions"
[[ "$imports" =~ ^[0-9]+$ && "$hits" =~ ^[0-9]+$ && "$evictions" =~ ^[0-9]+$ ]] ||
    fail "import-cache metrics are not numeric"
(( imports >= 90 && hits > 0 && evictions == imports )) ||
    fail "import-cache pressure, reuse, and final eviction are incomplete"
requests="$(field "$resources" worker_requests)" || fail "resource completion lacks worker_requests"
completions="$(field "$resources" worker_completions)" || fail "resource completion lacks worker_completions"
max_worker="$(field "$resources" max_worker_request_msec)" || fail "resource completion lacks max_worker_request_msec"
[[ "$requests" =~ ^[1-9][0-9]*$ && "$requests" == "$completions" ]] ||
    fail "renderer-worker requests did not retire exactly once"
[[ "$max_worker" =~ ^[0-9]+$ ]] && (( max_worker <= 100 )) ||
    fail "renderer-worker request latency exceeded 100 ms"
for pair in \
    worker_failures=0 \
    worker_soft_stalls=0 \
    worker_hard_stalls=0 \
    worker_release_enqueue_failures=0 \
    snapshot_live_entries=0 \
    snapshot_live_bytes=0 \
    import_cache_live_entries=0 \
    import_cache_descriptor_mismatches=0 \
    import_cache_capacity_rejections=0; do
    actual="$(field "$resources" "${pair%%=*}")" || fail "resource completion lacks ${pair%%=*}"
    [[ "$actual" == "${pair#*=}" ]] || fail "${pair%%=*} is $actual, expected ${pair#*=}"
done

control="$(grep -E '^sophia_live_session_control schema=1 status=complete ' "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$control" ]] || fail "session-control completion is missing"
enqueued="$(field "$control" enqueued)" || fail "control completion lacks enqueued"
for key in dispatched delivered; do
    actual="$(field "$control" "$key")" || fail "control completion lacks $key"
    [[ "$actual" == "$enqueued" ]] || fail "$key is not balanced with enqueued controls"
done
for pair in rejected=0 timed_out=0 unexpected=0 pending=0; do
    actual="$(field "$control" "${pair%%=*}")" || fail "control completion lacks ${pair%%=*}"
    [[ "$actual" == "${pair#*=}" ]] || fail "${pair%%=*} is $actual, expected ${pair#*=}"
done
max_ack="$(field "$control" max_ack_msec)" || fail "control completion lacks max_ack_msec"
[[ "$max_ack" =~ ^[0-9]+$ ]] && (( max_ack <= 100 )) ||
    fail "frontend control acknowledgement exceeded 100 ms"

cadence="$(grep -E '^sophia_live_present_cadence schema=1 status=complete ' "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$cadence" ]] || fail "present-cadence completion is missing"
samples="$(field "$cadence" samples)" || fail "present cadence lacks samples"
advancing="$(field "$cadence" advancing_intervals)" || fail "present cadence lacks advancing_intervals"
nonadvancing="$(field "$cadence" nonadvancing)" || fail "present cadence lacks nonadvancing"
[[ "$samples" =~ ^[0-9]+$ && "$advancing" =~ ^[0-9]+$ ]] || fail "present cadence is not numeric"
(( samples >= 90 && advancing + 1 == samples && nonadvancing == 0 )) ||
    fail "present cadence did not advance monotonically"

completion="$(grep -E '^sophia_live_session schema=16 status=bounded_complete ' "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$completion" ]] || fail "schema-16 completion is missing"
for pair in \
    authority_batches_dropped=0 \
    native_submit_failures=0 \
    native_retire_failures=0 \
    native_callback_rejected=0 \
    native_callback_queue_saturated=0 \
    native_in_flight=false \
    native_cleanup_pending=false \
    wm_policy=external \
    wm_restarts=0 \
    wm_degraded=false \
    present_live_sources=0 \
    present_live_fences=0 \
    present_live_transactions=0 \
    present_controlled_rejections=0; do
    actual="$(field "$completion" "${pair%%=*}")" || fail "completion lacks ${pair%%=*}"
    [[ "$actual" == "${pair#*=}" ]] || fail "${pair%%=*} is $actual, expected ${pair#*=}"
done

(( $(count '^sophia_live_output schema=1 status=complete output=') == 2 )) ||
    fail "both output completion records are missing"
awk '
    function value(key, i, pair) {
        for (i = 1; i <= NF; i++) {
            split($i, pair, "=")
            if (pair[1] == key) return pair[2]
        }
        return ""
    }
    /^sophia_live_output schema=1 status=complete output=/ {
        outputs++
        if (value("submissions") + 0 >= 90 && value("retirements") + 0 > 0) {
            active++
        } else if (value("submissions") == "1" && value("retirements") == "0" && value("callbacks") == "0") {
            baseline_only++
        }
        if (value("nonzero_exports") + 0 <= 0) exit 1
    }
    END { if (outputs != 2 || active != 1 || baseline_only != 1) exit 1 }
' "$EVIDENCE_FILE" || fail "the workload was not confined to exactly one active output"

grep -Eq '^sophia_live_session_health schema=1 status=clean .* pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false$' \
    "$EVIDENCE_FILE" || fail "final session health is not clean"
grep -Fxq \
    'sophia_live_layout_health schema=2 status=clean recovery_extents=0 standing_targets=0 constraint_relayout_pending=false' \
    "$EVIDENCE_FILE" || fail "layout recovery state did not drain"
grep -Eq '^sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 ' \
    "$EVIDENCE_FILE" || fail "native presentation did not drain"
grep -Eq '^sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 ' \
    "$EVIDENCE_FILE" || fail "application and frontend cleanup did not drain"
grep -Fxq \
    'sophia_qemu_guest schema=1 status=complete scenario=xmonad-render-contention' \
    "$EVIDENCE_FILE" || fail "the QEMU guest did not exit normally"

echo "QEMU xmonad render-contention evidence passed: $EVIDENCE_FILE"
