#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_FILE="${1:-/tmp/sophia-qemu-xmonad-producer-overload.log}"

fail() {
    echo "QEMU xmonad producer-overload verification failed: $*" >&2
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
    fail "an overload application exited before session teardown"
fi

grep -Fxq \
    'sophia_qemu_xmonad schema=2 status=starting isolation=headless control=qmp-unix profile=xmonad windows=2 gpu_mode=virgl host_render_node=explicit' \
    "$EVIDENCE_FILE" || fail "the explicit two-window virgl profile is missing"
grep -Fxq \
    'sophia_qemu_topology schema=1 status=observed requested_heads=2 connectors=2 connected=2' \
    "$EVIDENCE_FILE" || fail "the two-output guest topology is missing"
grep -Fxq \
    'sophia_qemu_xmonad schema=1 status=running windows=2 profile=xmonad mode=producer-overload producer=bounded-dri3-present interval_usec=5000 cpu_client=xterm' \
    "$EVIDENCE_FILE" || fail "the bounded DRI3 overload profile is missing"
grep -Fxq \
    'sophia_live_session_mode schema=1 mode=normal configured_apps=2 startup_apps=1' \
    "$EVIDENCE_FILE" || fail "the isolated overload application set is missing"
grep -Fxq \
    'sophia_live_x11_route_capacity schema=1 input=64 control=32 protocol=512 presentations=64' \
    "$EVIDENCE_FILE" || fail "the Present feedback route does not cover the authority burst"
grep -Fxq 'sophia_session_app schema=1 status=started id=cpu source=startup' \
    "$EVIDENCE_FILE" || fail "the static CPU client did not start"
grep -Eq '^sophia_session_app schema=2 status=started id=gpu source=action ' \
    "$EVIDENCE_FILE" || fail "the overload producer did not start through policy"
grep -Fxq \
    'sophia_qemu_overload_client schema=1 status=running buffers=3 interval_usec=5000 feedback=complete-idle' \
    "$EVIDENCE_FILE" || fail "the producer's bounded DMA-BUF pool is missing"
(( $(count '^sophia_session_app schema=2 status=admitted source=action ') == 1 )) ||
    fail "the unthrottled producer was not admitted exactly once"

launch_line="$(line_number '^sophia_qemu_producer_overload schema=1 status=launch_begin chord=meta_l[+]p app=gpu$')"
warmup_line="$(line_number '^sophia_qemu_producer_overload schema=1 status=warmup_complete copies=[0-9]+ skips=[0-9]+$')"
window_start_line="$(line_number '^sophia_qemu_producer_overload schema=1 status=window_started duration_msec=10000 phases=2$')"
window_complete_line="$(line_number '^sophia_qemu_producer_overload schema=1 status=window_complete duration_msec=10000 phases=2$')"
logout_line="$(line_number '^sophia_qemu_producer_overload schema=1 status=logout_begin chord=meta_l[+]shift[+]q$')"
for value in \
    "$launch_line" "$warmup_line" "$window_start_line" \
    "$window_complete_line" "$logout_line"; do
    [[ "$value" =~ ^[1-9][0-9]*$ ]] || fail "the overload sequence is incomplete"
done
(( launch_line < warmup_line \
    && warmup_line < window_start_line \
    && window_start_line < window_complete_line \
    && window_complete_line < logout_line )) ||
    fail "launch, overload, and logout markers are out of order"

warmup="$(sed -n "${warmup_line}p" "$EVIDENCE_FILE")"
warmup_copies="$(field "$warmup" copies)" || fail "warmup lacks copies"
warmup_skips="$(field "$warmup" skips)" || fail "warmup lacks skips"
(( warmup_copies >= 20 && warmup_skips >= 1 )) ||
    fail "warmup did not prove an above-refresh producer"

(( $(count '^sophia_qemu_producer_overload schema=1 status=phase_complete ') == 2 )) ||
    fail "both five-second overload phases are required"
awk '
    function value(key, i, pair) {
        for (i = 1; i <= NF; i++) {
            split($i, pair, "=")
            if (pair[1] == key) return pair[2] + 0
        }
        return -1
    }
    /^sophia_qemu_producer_overload schema=1 status=phase_complete / {
        phases++
        if (value("phase") != phases || value("duration_msec") != 5000 ||
            value("copies") < 20 || value("skips") < 1 ||
            value("submissions") < 20 || value("retirements") < 20) exit 1
        difference = value("submissions") - value("retirements")
        if (difference < -1 || difference > 1) exit 1
        copies += value("copies")
        skips += value("skips")
    }
    END { if (phases != 2 || copies < 40 || skips < 2) exit 1 }
' "$EVIDENCE_FILE" || fail "the two overload phases lack sustained production and dropping"

# Page-flip retirement is the only presentation boundary. A second submit may
# not overlap the immutable buffer already leased to KMS.
awk '
    /sophia_live_native_page_flip schema=1 status=submitted output=1 / {
        in_flight++
        submissions++
        if (in_flight > 1) exit 1
    }
    /sophia_live_native_page_flip schema=1 status=retired output=1 / {
        in_flight--
        retirements++
        if (in_flight < 0) exit 1
    }
    END {
        if (submissions < 40 || retirements < 40 || in_flight != 0) exit 1
    }
' "$EVIDENCE_FILE" || fail "output 1 had overlapping or unretired KMS submissions"

scheduler="$(grep -E '^sophia_live_present_scheduler schema=1 status=complete ' "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$scheduler" ]] || fail "Present-scheduler completion is missing"
for pair in \
    surface_content_capacity=256 \
    pending_limit=1 \
    in_flight_limit=1 \
    max_latest_deferred_per_surface=1 \
    max_pending_queued=1 \
    max_total_queued=1; do
    actual="$(field "$scheduler" "${pair%%=*}")" || fail "scheduler lacks ${pair%%=*}"
    [[ "$actual" == "${pair#*=}" ]] || fail "${pair%%=*} is $actual, expected ${pair#*=}"
done
supersessions="$(field "$scheduler" pending_supersessions)" ||
    fail "scheduler lacks pending_supersessions"
surface_supersessions="$(field "$scheduler" surface_content_supersessions)" ||
    fail "scheduler lacks surface_content_supersessions"
scheduler_supersessions="$(field "$scheduler" scheduler_supersessions)" ||
    fail "scheduler lacks scheduler_supersessions"
surface_content_capacity="$(field "$scheduler" surface_content_capacity)" ||
    fail "scheduler lacks surface_content_capacity"
max_surface_content_deferred="$(field "$scheduler" max_surface_content_deferred)" ||
    fail "scheduler lacks max_surface_content_deferred"
shutdown_rejections="$(field "$scheduler" shutdown_present_rejections)" ||
    fail "scheduler lacks shutdown_present_rejections"
present_rejections="$(field "$scheduler" present_rejections)" ||
    fail "scheduler lacks present_rejections"
native_suspend_rejections="$(field "$scheduler" native_suspend_present_rejections)" ||
    fail "scheduler lacks native_suspend_present_rejections"
other_rejections="$(field "$scheduler" other_present_rejections)" ||
    fail "scheduler lacks other_present_rejections"
max_sources="$(field "$scheduler" max_live_sources)" || fail "scheduler lacks max_live_sources"
max_fences="$(field "$scheduler" max_live_fences)" || fail "scheduler lacks max_live_fences"
max_presentations="$(field "$scheduler" max_live_presentations)" ||
    fail "scheduler lacks max_live_presentations"
[[ "$supersessions" =~ ^[1-9][0-9]*$ ]] || fail "no pending frame was superseded"
[[ "$surface_supersessions" =~ ^[1-9][0-9]*$ ]] ||
    fail "surface-content admission did not supersede an overloaded frame"
[[ "$scheduler_supersessions" =~ ^[0-9]+$ ]] ||
    fail "scheduler_supersessions is not numeric"
(( supersessions == surface_supersessions + scheduler_supersessions )) ||
    fail "supersession totals do not match their owning queues"
[[ "$max_surface_content_deferred" =~ ^[1-9][0-9]*$ ]] &&
    (( max_surface_content_deferred <= surface_content_capacity )) ||
    fail "general surface-content ordering exceeded its bounded capacity"
[[ "$shutdown_rejections" =~ ^[0-9]+$ ]] ||
    fail "shutdown_present_rejections is not numeric"
[[ "$present_rejections" =~ ^[1-9][0-9]*$ ]] ||
    fail "present_rejections is not positive"
[[ "$native_suspend_rejections" =~ ^[0-9]+$ ]] &&
    (( native_suspend_rejections <= 1 )) ||
    fail "native suspend rejected more than one owned Present"
[[ "$other_rejections" =~ ^[0-9]+$ ]] && (( other_rejections <= 1 )) ||
    fail "unexpected Present rejection population exceeded one lifecycle edge"
[[ "$max_sources" =~ ^[1-9][0-9]*$ ]] && (( max_sources <= 16 )) ||
    fail "DMA-BUF source ownership exceeded the bounded producer pool"
[[ "$max_fences" =~ ^[0-9]+$ ]] && (( max_fences <= 16 )) ||
    fail "fence ownership exceeded the bounded producer pool"
[[ "$max_presentations" =~ ^[0-9]+$ ]] && (( max_presentations <= 2 )) ||
    fail "more than one in-flight plus one pending Present was owned"

resources="$(grep -E '^sophia_live_native_resources schema=[0-9]+ status=complete ' "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$resources" ]] || fail "native resource completion is missing"
imports="$(field "$resources" import_cache_imports)" || fail "resources lack import_cache_imports"
evictions="$(field "$resources" import_cache_evictions)" || fail "resources lack import_cache_evictions"
[[ "$imports" =~ ^[1-9][0-9]*$ && "$evictions" == "$imports" ]] ||
    fail "renderer imports did not evict exactly once"
requests="$(field "$resources" worker_requests)" || fail "resources lack worker_requests"
completions="$(field "$resources" worker_completions)" || fail "resources lack worker_completions"
max_worker="$(field "$resources" max_worker_request_msec)" || fail "resources lack max_worker_request_msec"
deferrals=0
if (( $(field "$resources" schema) >= 7 )); then
    deferrals="$(field "$resources" frame_slot_deferrals)" || fail "schema-7 resources lack frame_slot_deferrals"
    [[ "$(field "$resources" frame_slot_stale_releases)" == 0 ]] || fail "native frame-slot release was stale"
fi
[[ "$requests" =~ ^[1-9][0-9]*$ ]] && (( requests == completions + deferrals )) ||
    fail "renderer-worker ownership did not balance"
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

completion="$(grep -E '^sophia_live_session schema=16 status=bounded_complete ' "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$completion" ]] || fail "schema-16 completion is missing"
complete_copy="$(field "$completion" present_complete_copy)" || fail "completion lacks present_complete_copy"
complete_flip="$(field "$completion" present_complete_flip)" || fail "completion lacks present_complete_flip"
complete_skip="$(field "$completion" present_complete_skip)" || fail "completion lacks present_complete_skip"
idle="$(field "$completion" present_idle)" || fail "completion lacks present_idle"
complete_routed="$(field "$completion" present_complete_routed)" ||
    fail "completion lacks present_complete_routed"
idle_routed="$(field "$completion" present_idle_routed)" ||
    fail "completion lacks present_idle_routed"
route_failures="$(field "$completion" present_route_failures)" ||
    fail "completion lacks present_route_failures"
controlled_rejections="$(field "$completion" present_controlled_rejections)" ||
    fail "completion lacks present_controlled_rejections"
[[ "$controlled_rejections" =~ ^[0-9]+$ ]] ||
    fail "present_controlled_rejections is not numeric"
(( $(count '^sophia_live_present_progress schema=1 ') >= 2 )) ||
    fail "bounded cumulative Present progress is missing"
awk '
    function value(key, i, pair) {
        for (i = 1; i <= NF; i++) {
            split($i, pair, "=")
            if (pair[1] == key && pair[2] ~ /^[0-9]+$/) return pair[2] + 0
        }
        return -1
    }
    /^sophia_live_present_progress schema=1 / {
        copy = value("complete_copy")
        flip = value("complete_flip")
        skip = value("complete_skip")
        idle = value("idle")
        if (copy < 0 || flip < 0 || skip < 0 || idle < 0) exit 1
        if (samples > 0 &&
            (copy < previous_copy || flip < previous_flip ||
             skip < previous_skip || idle < previous_idle)) exit 1
        samples++
        previous_copy = copy
        previous_flip = flip
        previous_skip = skip
        previous_idle = idle
    }
    END { if (samples < 2) exit 1 }
' "$EVIDENCE_FILE" || fail "cumulative Present progress regressed"
progress="$(grep -E '^sophia_live_present_progress schema=1 ' "$EVIDENCE_FILE" | tail -n 1)"
for pair in \
    complete_copy="$complete_copy" \
    complete_flip="$complete_flip" \
    complete_skip="$complete_skip" \
    idle="$idle"; do
    actual="$(field "$progress" "${pair%%=*}")" ||
        fail "final Present progress lacks ${pair%%=*}"
    [[ "$actual" == "${pair#*=}" ]] ||
        fail "final Present progress ${pair%%=*} is $actual, expected ${pair#*=}"
done
(( complete_copy >= warmup_copies + 40 && complete_skip >= warmup_skips + 2 )) ||
    fail "completion does not cover the warmup and measured overload"
(( complete_skip == present_rejections )) ||
    fail "Skip feedback does not match backend rejection ownership"
(( present_rejections == supersessions + controlled_rejections + native_suspend_rejections + shutdown_rejections + other_rejections )) ||
    fail "Present rejections do not match their bounded lifecycle populations"
(( idle == complete_copy + complete_flip + complete_skip )) ||
    fail "Present completion and Idle feedback are not balanced"
(( complete_routed == complete_copy + complete_flip + complete_skip \
    && idle_routed == idle && route_failures == 0 )) ||
    fail "client-visible Present feedback did not route exactly once"
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
    present_live_transactions=0; do
    actual="$(field "$completion" "${pair%%=*}")" || fail "completion lacks ${pair%%=*}"
    [[ "$actual" == "${pair#*=}" ]] || fail "${pair%%=*} is $actual, expected ${pair#*=}"
done

control="$(grep -E '^sophia_live_session_control schema=(1|2) status=complete ' "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$control" ]] || fail "session-control completion is missing"
enqueued="$(field "$control" enqueued)" || fail "control completion lacks enqueued"
dispatched="$(field "$control" dispatched)" || fail "control completion lacks dispatched"
delivered="$(field "$control" delivered)" || fail "control completion lacks delivered"
stale_retired=0
if [[ "$(field "$control" schema)" == 2 ]]; then
    stale_retired="$(field "$control" stale_retired)" ||
        fail "schema-2 control completion lacks stale_retired"
fi
(( dispatched == enqueued && delivered + stale_retired == dispatched )) ||
    fail "control completion is not balanced"
for pair in rejected=0 timed_out=0 unexpected=0 pending=0; do
    actual="$(field "$control" "${pair%%=*}")" || fail "control completion lacks ${pair%%=*}"
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
        if (value("retirements") + 0 >= 40) active++
        else if (value("submissions") == "1" && value("retirements") == "0" && value("callbacks") == "0") baseline_only++
        if (value("nonzero_exports") + 0 <= 0) exit 1
    }
    END { if (outputs != 2 || active != 1 || baseline_only != 1) exit 1 }
' "$EVIDENCE_FILE" || fail "the overload workload was not confined to one active output"

grep -Eq '^sophia_live_present_cadence schema=1 status=complete .* nonadvancing=0 ' \
    "$EVIDENCE_FILE" || fail "displayed Present cadence did not advance monotonically"
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
    'sophia_qemu_guest schema=1 status=complete scenario=xmonad-producer-overload' \
    "$EVIDENCE_FILE" || fail "the QEMU guest did not exit normally"

echo "QEMU xmonad producer-overload evidence passed: $EVIDENCE_FILE"
