#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_FILE="${1:-/tmp/sophia-qemu-xmonad-idle-efficiency.log}"

fail() {
    echo "QEMU xmonad idle-efficiency verification failed: $*" >&2
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
if grep -Eq '^sophia_session_app schema=1 status=exited id=(cpu|gpu) ' \
    "$EVIDENCE_FILE"; then
    fail "an efficiency application exited before session teardown"
fi

grep -Fxq \
    'sophia_qemu_xmonad schema=2 status=starting isolation=headless control=qmp-unix profile=xmonad windows=2 gpu_mode=virgl host_render_node=explicit' \
    "$EVIDENCE_FILE" || fail "the explicit two-window virgl profile is missing"
grep -Fxq \
    'sophia_qemu_topology schema=1 status=observed requested_heads=2 connectors=2 connected=2' \
    "$EVIDENCE_FILE" || fail "the two-output guest topology is missing"
grep -Fxq \
    'sophia_live_session_mode schema=1 mode=normal configured_apps=2 startup_apps=1' \
    "$EVIDENCE_FILE" || fail "the isolated efficiency application set is missing"
grep -Fxq 'sophia_session_app schema=1 status=started id=cpu source=startup' \
    "$EVIDENCE_FILE" || fail "the static CPU client did not start"
grep -Eq '^sophia_session_app schema=2 status=started id=gpu source=action ' \
    "$EVIDENCE_FILE" || fail "the DMA-BUF producer did not start through policy"
(( $(count '^sophia_session_app schema=2 status=admitted source=action ') == 1 )) ||
    fail "the DMA-BUF producer was not admitted exactly once"

cpu_line="$(line_number '^sophia_session_app schema=1 status=started id=cpu source=startup$')"
launch_line="$(line_number '^sophia_qemu_idle_efficiency schema=1 status=launch_begin chord=meta_l[+]p app=gpu$')"
gpu_line="$(line_number '^sophia_session_app schema=2 status=started id=gpu source=action ')"
frozen_line="$(line_number '^sophia_qemu_idle_client schema=1 status=frozen producer=glxgears$')"
quiescent_line="$(line_number '^sophia_qemu_idle_efficiency schema=1 status=producer_quiescent surfaces=1 ')"
reuse_start_line="$(line_number '^sophia_qemu_idle_efficiency schema=1 status=reuse_window_started focus_transitions=256$')"
reuse_complete_line="$(line_number '^sophia_qemu_idle_efficiency schema=1 status=reuse_window_complete focus_transitions=256 ' )"
idle_start_line="$(line_number '^sophia_qemu_idle_efficiency schema=1 status=idle_window_started duration_msec=2000$')"
idle_complete_line="$(line_number '^sophia_qemu_idle_efficiency schema=1 status=idle_window_complete duration_msec=2000 ' )"
logout_line="$(line_number '^sophia_qemu_idle_efficiency schema=1 status=logout_begin chord=meta_l[+]shift[+]q$')"
for value in \
    "$cpu_line" "$launch_line" "$gpu_line" "$frozen_line" "$quiescent_line" \
    "$reuse_start_line" "$reuse_complete_line" "$idle_start_line" \
    "$idle_complete_line" "$logout_line"; do
    [[ "$value" =~ ^[1-9][0-9]*$ ]] || fail "the efficiency sequence is incomplete"
done
(( cpu_line < launch_line \
    && launch_line < gpu_line \
    && gpu_line < frozen_line \
    && frozen_line < quiescent_line \
    && quiescent_line < reuse_start_line \
    && reuse_start_line < reuse_complete_line \
    && reuse_complete_line < idle_start_line \
    && idle_start_line < idle_complete_line \
    && idle_complete_line < logout_line )) ||
    fail "producer, reuse, idle, and logout markers are out of order"

quiescent="$(sed -n "${quiescent_line}p" "$EVIDENCE_FILE")"
marker_retirements="$(field "$quiescent" retirements)" ||
    fail "the quiescent marker lacks retirements"
[[ "$marker_retirements" =~ ^[1-9][0-9]*$ ]] ||
    fail "the producer retirement total is not positive"
raw_retirements="$(count '^sophia_live_session_present schema=2 status=retired ')"
(( marker_retirements == raw_retirements && raw_retirements >= 10 )) ||
    fail "the frozen producer lacks ten causal DMA-BUF retirements"
(( $(awk '
    /^sophia_live_session_present schema=2 status=retired / {
        for (i = 1; i <= NF; i++) if ($i ~ /^surface=[0-9]+$/) surfaces[$i] = 1
    }
    END { for (surface in surfaces) count++; print count + 0 }
' "$EVIDENCE_FILE") == 1 )) || fail "the workload did not retain exactly one DMA-BUF surface"

awk -v start="$reuse_start_line" -v complete="$reuse_complete_line" '
    NR <= start || NR >= complete { next }
    /^sophia_live_wm schema=1 status=physical_action_committed action=/ { actions++ }
    /sophia_live_output_repaint schema=1 status=presented output=1 / {
        repaints++
        if ($0 ~ / mode=partial /) partial++
    }
    /sophia_live_native_page_flip schema=1 status=retired output=1 / { flips++ }
    /sophia_live_native_page_flip schema=1 status=submitted output=1 / {
        if ($0 ~ / content=Some[(]RetainedMixed /) retained_submits++
        else nonretained_submits++
    }
    /^sophia_live_session_present schema=2 status=retired / { presents++ }
    END {
        if (actions != 256 || repaints < 256 || partial != repaints || flips < 256 ||
            retained_submits < 256 || nonretained_submits != 0 || presents != 0) exit 1
    }
' "$EVIDENCE_FILE" || fail "the retained-image phase lacks 256 partial, page-flip-retired retained submissions"

reuse="$(sed -n "${reuse_complete_line}p" "$EVIDENCE_FILE")"
for pair in \
    focus_transitions=256 \
    actions=256 \
    producer_retirements="$marker_retirements"; do
    actual="$(field "$reuse" "${pair%%=*}")" || fail "reuse completion lacks ${pair%%=*}"
    [[ "$actual" == "${pair#*=}" ]] || fail "${pair%%=*} is $actual, expected ${pair#*=}"
done
for key in repaints partial_repaints flips; do
    value="$(field "$reuse" "$key")" || fail "reuse completion lacks $key"
    [[ "$value" =~ ^[0-9]+$ ]] && (( value >= 256 )) ||
        fail "$key does not cover every focus transition"
done
[[ "$(field "$reuse" repaints)" == "$(field "$reuse" partial_repaints)" ]] ||
    fail "the reuse window contains a non-partial repaint"

idle="$(sed -n "${idle_complete_line}p" "$EVIDENCE_FILE")"
for pair in repaints=0 page_flips=0 client_presents=0; do
    actual="$(field "$idle" "${pair%%=*}")" || fail "idle completion lacks ${pair%%=*}"
    [[ "$actual" == "${pair#*=}" ]] || fail "idle ${pair%%=*} is $actual, expected ${pair#*=}"
done
awk -v start="$idle_start_line" -v complete="$idle_complete_line" '
    NR <= start || NR >= complete { next }
    /sophia_live_output_repaint schema=1 status=presented / { work++ }
    /sophia_live_native_page_flip schema=1 status=retired / { work++ }
    /^sophia_live_session_present schema=2 status=retired / { work++ }
    END { if (work != 0) exit 1 }
' "$EVIDENCE_FILE" || fail "the idle window performed rendering or client-present work"

resources="$(grep -E '^sophia_live_native_resources schema=5 status=complete ' "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$resources" ]] || fail "native resource completion is missing"
imports="$(field "$resources" import_cache_imports)" || fail "resources lack import_cache_imports"
hits="$(field "$resources" import_cache_hits)" || fail "resources lack import_cache_hits"
evictions="$(field "$resources" import_cache_evictions)" || fail "resources lack import_cache_evictions"
[[ "$imports" =~ ^[1-9][0-9]*$ && "$hits" =~ ^[1-9][0-9]*$ && "$evictions" =~ ^[0-9]+$ ]] ||
    fail "import-cache metrics are not positive integers"
(( imports >= 10 && hits > imports && evictions == imports )) ||
    fail "retained repaints did not produce a majority import-cache hit rate"
composition_reuses="$(field "$resources" composition_target_reuses)" ||
    fail "resources lack composition_target_reuses"
(( composition_reuses >= 256 )) || fail "composition target reuse did not cover the focus phase"
requests="$(field "$resources" worker_requests)" || fail "resources lack worker_requests"
completions="$(field "$resources" worker_completions)" || fail "resources lack worker_completions"
max_worker="$(field "$resources" max_worker_request_msec)" || fail "resources lack max_worker_request_msec"
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
    actual="$(field "$resources" "${pair%%=*}")" || fail "resources lack ${pair%%=*}"
    [[ "$actual" == "${pair#*=}" ]] || fail "${pair%%=*} is $actual, expected ${pair#*=}"
done

rendering="$(grep -E '^sophia_live_rendering_efficiency schema=1 status=complete ' "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$rendering" ]] || fail "rendering-efficiency completion is missing"
for key in cpu_updates cpu_replacements damage_scoped_metric_frames composition_target_reuses; do
    value="$(field "$rendering" "$key")" || fail "rendering completion lacks $key"
    [[ "$value" =~ ^[1-9][0-9]*$ ]] || fail "$key did not prove the mixed static desktop"
done

completion="$(grep -E '^sophia_live_session schema=16 status=bounded_complete ' "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$completion" ]] || fail "schema-16 completion is missing"
native_frame_uploads="$(field "$completion" native_frame_uploads)" ||
    fail "completion lacks native_frame_uploads"
[[ "$native_frame_uploads" =~ ^[0-9]+$ ]] &&
    (( native_frame_uploads >= 2 && native_frame_uploads <= 3 )) ||
    fail "native_frame_uploads is $native_frame_uploads, expected the two- or three-frame startup baseline"
for pair in \
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
        if (value("retirements") + 0 >= 256) active++
        else if (value("submissions") == "1" && value("retirements") == "0" && value("callbacks") == "0") baseline_only++
        if (value("nonzero_exports") + 0 <= 0) exit 1
    }
    END { if (outputs != 2 || active != 1 || baseline_only != 1) exit 1 }
' "$EVIDENCE_FILE" || fail "the efficiency workload was not confined to one active output"

grep -Eq '^sophia_live_session_health schema=1 status=clean .* pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false$' \
    "$EVIDENCE_FILE" || fail "final session health is not clean"
grep -Fxq \
    'sophia_live_layout_health schema=2 status=clean recovery_extents=0 standing_targets=0 constraint_relayout_pending=false' \
    "$EVIDENCE_FILE" || fail "layout recovery state did not drain"
grep -Eq '^sophia_live_session_protocol_errors schema=1 expected=[0-9]+ unexpected=0$' \
    "$EVIDENCE_FILE" || fail "unexpected X11 protocol errors were observed"
grep -Eq '^sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 ' \
    "$EVIDENCE_FILE" || fail "native presentation did not drain"
grep -Eq '^sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 ' \
    "$EVIDENCE_FILE" || fail "application and frontend cleanup did not drain"
grep -Fxq \
    'sophia_qemu_guest schema=1 status=complete scenario=xmonad-idle-efficiency' \
    "$EVIDENCE_FILE" || fail "the QEMU guest did not exit normally"

echo "QEMU xmonad idle-efficiency evidence passed: $EVIDENCE_FILE"
