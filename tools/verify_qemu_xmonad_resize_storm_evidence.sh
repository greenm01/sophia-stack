#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_FILE="${1:-/tmp/sophia-qemu-xmonad-resize-storm.log}"

fail() {
    echo "QEMU xmonad resize-storm verification failed: $*" >&2
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

[[ -s "$EVIDENCE_FILE" ]] || fail "missing evidence: $EVIDENCE_FILE"
if grep -Eq \
    '(^Error:|panicked at|mismatched.transaction|status=(failed|degraded)([[:space:]]|$)|sophia_live_wm schema=1 status=layout_timeout |sophia_live_resize_epoch schema=1 status=aborted )' \
    "$EVIDENCE_FILE"; then
    fail "evidence contains an error, resize timeout, rollback, or degraded result"
fi

grep -Fxq \
    'sophia_live_session_mode schema=1 mode=normal configured_apps=1 startup_apps=1' \
    "$EVIDENCE_FILE" || fail "the isolated resize-storm profile is missing"
grep -Eq 'sophia_session_app schema=1 status=started id=renderer source=startup$' \
    "$EVIDENCE_FILE" || fail "the continuously redrawing Xterm did not start"
grep -Eq '^sophia_live_wm schema=1 status=ready adapter=external ' \
    "$EVIDENCE_FILE" || fail "external xmonad policy did not become ready"
grep -Fxq \
    'sophia_live_session_startup schema=2 status=output_baseline_ready outputs=2/2' \
    "$EVIDENCE_FILE" || fail "both output baselines were not presented"
(( $(count '^sophia_live_resize schema=2 status=requested ') == 12 )) ||
    fail "the storm did not request exactly twelve resizes"
(( $(count '^sophia_live_resize schema=2 status=committed ') == 12 )) ||
    fail "the storm did not commit exactly twelve exact-pixel resizes"

awk '
    function value(key, i, pair) {
        for (i = 1; i <= NF; i++) {
            split($i, pair, "=")
            if (pair[1] == key) return pair[2]
        }
        return ""
    }
    /^sophia_live_resize schema=2 status=requested / {
        step = value("step") + 0
        if (phase != 0 || step != expected + 1 || value("total") != 12) exit 1
        transaction = value("transaction")
        surface = value("surface")
        width = value("width")
        height = value("height")
        phase = 1
        next
    }
    phase >= 1 && /^sophia_live_wm schema=1 status=layout_committed / && value("transaction") == transaction {
        if (value("surfaces") != 1 || value("configure_deliveries") != 1 || value("outcome") != "Committed") exit 1
        phase = 2
        next
    }
    phase >= 2 && /^sophia_live_resize_epoch schema=1 status=committed / && value("transaction") == transaction {
        if (value("matched_surfaces") != 1) exit 1
        phase = 3
        next
    }
    /^sophia_live_resize schema=2 status=committed / {
        if (phase != 3 || value("transaction") != transaction || value("surface") != surface) exit 1
        if (value("width") != width || value("height") != height) exit 1
        if (value("step") != expected + 1 || value("total") != 12) exit 1
        if (value("configure_delivered") != "true" || value("pixels") != "true") exit 1
        expected++
        phase = 0
        next
    }
    END { if (expected != 12 || phase != 0) exit 1 }
' "$EVIDENCE_FILE" || fail "resize requests, layout commits, and exact pixels are not causally paired"

grep -Eq '^sophia_live_resize_storm schema=1 status=complete steps=12 surface=[0-9]+ exact_pixels=true$' \
    "$EVIDENCE_FILE" || fail "the exact-pixel storm completion is missing"
grep -Fxq \
    'sophia_qemu_resize_storm schema=1 status=post_storm_frame_retired steps=12' \
    "$EVIDENCE_FILE" || fail "rendering did not continue after the final resize"
grep -Eq 'sophia_live_output_repaint schema=1 status=presented output=[0-9]+ mode=partial ' \
    "$EVIDENCE_FILE" || fail "the CPU renderer never used partial-damage repaint"
(( $(count 'sophia_live_native_page_flip schema=1 status=retired output=') >= 12 )) ||
    fail "fewer than twelve native frames retired during the storm"

rendering="$(grep -E '^sophia_live_rendering_efficiency schema=1 status=complete ' "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$rendering" ]] || fail "rendering-efficiency completion is missing"
for key in cpu_patch_updates cpu_payload_bytes; do
    value="$(field "$rendering" "$key")" || fail "rendering completion is missing $key"
    [[ "$value" =~ ^[1-9][0-9]*$ ]] || fail "$key did not prove active CPU rendering"
done

resources="$(grep -E '^sophia_live_native_resources schema=5 status=complete ' "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$resources" ]] || fail "native resource completion is missing"
requests="$(field "$resources" worker_requests)" || fail "resource completion lacks worker_requests"
completions="$(field "$resources" worker_completions)" || fail "resource completion lacks worker_completions"
[[ "$requests" =~ ^[1-9][0-9]*$ && "$requests" == "$completions" ]] ||
    fail "renderer-worker ownership did not retire exactly once"
for pair in \
    worker_failures=0 \
    worker_hard_stalls=0 \
    worker_release_enqueue_failures=0 \
    snapshot_live_entries=0 \
    snapshot_live_bytes=0 \
    import_cache_live_entries=0; do
    actual="$(field "$resources" "${pair%%=*}")" || fail "resource completion lacks ${pair%%=*}"
    [[ "$actual" == "${pair#*=}" ]] || fail "${pair%%=*} is $actual, expected ${pair#*=}"
done

transport="$(grep -E '^sophia_live_wm_transport schema=2 status=complete ' "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$transport" ]] || fail "WM transport completion is missing"
for pair in pending=0 stale_responses=0; do
    actual="$(field "$transport" "${pair%%=*}")" || fail "WM transport lacks ${pair%%=*}"
    [[ "$actual" == "${pair#*=}" ]] || fail "${pair%%=*} is $actual, expected ${pair#*=}"
done
completion="$(grep -E '^sophia_live_session schema=16 status=bounded_complete ' "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$completion" ]] || fail "schema-16 completion is missing"
for pair in \
    authority_batches_dropped=0 \
    native_submit_failures=0 \
    native_retire_failures=0 \
    native_callback_rejected=0 \
    native_in_flight=false \
    native_cleanup_pending=false \
    wm_policy=external \
    wm_restarts=0 \
    wm_degraded=false \
    surface_resize=committed; do
    actual="$(field "$completion" "${pair%%=*}")" || fail "completion lacks ${pair%%=*}"
    [[ "$actual" == "${pair#*=}" ]] || fail "${pair%%=*} is $actual, expected ${pair#*=}"
done
grep -Eq '^sophia_live_session_health schema=1 status=clean .* pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false$' \
    "$EVIDENCE_FILE" || fail "final session health is not clean"
grep -Fxq \
    'sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=none' \
    "$EVIDENCE_FILE" || fail "native presentation did not drain"
grep -Eq '^sophia_live_session_cleanup schema=1 status=clean app_groups=0([[:space:]]|$)' \
    "$EVIDENCE_FILE" || fail "application cleanup did not drain"
grep -Fxq \
    'sophia_qemu_guest schema=1 status=complete scenario=xmonad-resize-storm' \
    "$EVIDENCE_FILE" || fail "the QEMU guest did not exit normally"

echo "QEMU xmonad resize-storm evidence passed: $EVIDENCE_FILE"
