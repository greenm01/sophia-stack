#!/usr/bin/env bash
set -euo pipefail

evidence="${1:?usage: verify_mirror_group_physical.sh EVIDENCE}"

fail() {
    echo "mirror-group physical verification failed: $*" >&2
    exit 1
}

[[ -s "$evidence" ]] || fail "missing or empty evidence: $evidence"

count() {
    grep -Ec "$1" "$evidence" || true
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

require_field() {
    local line="$1" key="$2" expected="$3" actual
    actual="$(field "$line" "$key")" || fail "record is missing $key"
    [[ "$actual" == "$expected" ]] || fail "$key is $actual, expected $expected"
}

require_positive_field() {
    local line="$1" key="$2" actual
    actual="$(field "$line" "$key")" || fail "record is missing $key"
    [[ "$actual" =~ ^[0-9]+$ ]] || fail "$key is not an integer: $actual"
    (( actual > 0 )) || fail "$key must be positive"
}

if grep -Eqi '(^Error:|panicked at|status=(failed|degraded)([[:space:]]|$)|status=(hard_stall|head_lost)([[:space:]]|$))' \
    "$evidence"; then
    fail "evidence contains an error, panic, failed/degraded status, stall, or head loss"
fi

[[ "$(count '^sophia_mirror_group_gate schema=1 status=starting source_commit=[0-9a-f]{40} sophia_sha256=[0-9a-f]{64} profile_sha256=[0-9a-f]{64}$')" == 1 ]] ||
    fail "expected one exact source/binary/profile identity record"
grep -Fxq \
    'sophia_live_outputs schema=2 status=ready discovered=2 presentation=1 native_owned=2 multi_output_scanout=enabled layout=extended_horizontal' \
    "$evidence" || fail "one logical output was not backed by two owned heads"
grep -Fxq 'sophia_live_session_startup schema=2 status=output_baseline_ready outputs=1/1' \
    "$evidence" || fail "the logical output baseline was not presented"
grep -Eq '^sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=(none|[0-9]+)$' \
    "$evidence" || fail "native ownership did not suspend through a clean drain"

mapfile -t ready_heads < <(
    grep -E '^sophia_live_native_head schema=1 status=ready output=[0-9]+ connector=DP-[12] connector_id=[0-9]+ mode=[0-9]+x[0-9]+ refresh_millihz=[0-9]+ mirrored=true$' \
        "$evidence"
)
(( ${#ready_heads[@]} == 2 )) || fail "expected exactly two mirrored ready heads"
dp1="$(printf '%s\n' "${ready_heads[@]}" | grep ' connector=DP-1 ')"
dp2="$(printf '%s\n' "${ready_heads[@]}" | grep ' connector=DP-2 ')"
[[ -n "$dp1" && -n "$dp2" ]] || fail "DP-1 and DP-2 were not both reported"
require_field "$dp1" mode 2560x1440
require_field "$dp2" mode 1920x1080
dp1_output="$(field "$dp1" output)" || fail "DP-1 omitted output identity"
dp2_output="$(field "$dp2" output)" || fail "DP-2 omitted output identity"
[[ "$dp1_output" == "$dp2_output" ]] || fail "the heads do not share one logical output"
dp1_connector="$(field "$dp1" connector_id)" || fail "DP-1 omitted connector identity"
dp2_connector="$(field "$dp2" connector_id)" || fail "DP-2 omitted connector identity"
[[ "$dp1_connector" != "$dp2_connector" ]] || fail "the heads share one connector identity"
for connector in "$dp1_connector" "$dp2_connector"; do
    grep -Fxq "sophia_live_mirror_bootstrap schema=1 status=direct_cpu output=$dp1_output connector_id=$connector exports=1" \
        "$evidence" || fail "connector $connector did not use direct-CPU bootstrap"
    grep -Fxq "sophia_live_mirror_bootstrap schema=1 status=worker_ready output=$dp1_output connector_id=$connector workers=1" \
        "$evidence" || fail "connector $connector did not establish its renderer worker"
done

mapfile -t complete_heads < <(
    grep -E '^sophia_live_native_head schema=1 status=complete output=[0-9]+ connector_id=[0-9]+ checksum=[0-9]+ submissions=[0-9]+ retirements=[0-9]+ callbacks=[0-9]+ nonzero_exports=[0-9]+$' \
        "$evidence"
)
(( ${#complete_heads[@]} == 2 )) || fail "expected exactly two completed heads"
for connector in "$dp1_connector" "$dp2_connector"; do
    head_line="$(printf '%s\n' "${complete_heads[@]}" | grep " connector_id=$connector ")"
    [[ -n "$head_line" ]] || fail "connector $connector has no completion record"
    require_field "$head_line" output "$dp1_output"
    require_positive_field "$head_line" checksum
    require_positive_field "$head_line" submissions
    require_positive_field "$head_line" retirements
    require_positive_field "$head_line" callbacks
    require_positive_field "$head_line" nonzero_exports
done

# Completion counters can be positive for unrelated generations. Require one
# exact logical frame to have crossed submit, callback, and retire on both heads.
common_frame=
while read -r frame; do
    [[ -n "$frame" ]] || continue
    causal=true
    for connector in "$dp1_connector" "$dp2_connector"; do
        submit_line="$(grep -nEm1 "^sophia_live_native_head_page_flip schema=1 status=submitted output=$dp1_output connector_id=$connector .* frame=$frame$" "$evidence" | cut -d: -f1 || true)"
        callback_line="$(grep -nEm1 "^sophia_live_native_head_page_flip schema=1 status=callback_accepted output=$dp1_output connector_id=$connector callbacks=1 kernel_sequence=[0-9]+ frame=$frame$" "$evidence" | cut -d: -f1 || true)"
        retire_line="$(grep -nEm1 "^sophia_live_native_head_page_flip schema=1 status=retired output=$dp1_output connector_id=$connector submission=[0-9]+ frame=$frame$" "$evidence" | cut -d: -f1 || true)"
        if [[ -z "$submit_line" || -z "$callback_line" || -z "$retire_line" ]] \
            || (( submit_line >= callback_line || callback_line >= retire_line )); then
            causal=false
        fi
    done
    if [[ "$causal" == true ]]; then
        common_frame="$frame"
        break
    fi
done < <(
    sed -n "s/^sophia_live_native_head_page_flip schema=1 status=retired output=$dp1_output connector_id=$dp1_connector submission=[0-9][0-9]* frame=\([0-9][0-9]*\)$/\1/p" "$evidence"
)
[[ -n "$common_frame" ]] || fail "no logical frame completed causally on both heads"

grep -Fxq 'sophia_live_vsync schema=1 status=complete outputs=2 overlap_rejections=0 phase_rejections=0 policy=page_flip_paced' \
    "$evidence" || fail "both heads did not complete without pacing rejection"
session="$(grep -E '^sophia_live_session schema=16 status=bounded_complete ' "$evidence")"
[[ "$(printf '%s\n' "$session" | wc -l)" == 1 ]] || fail "expected one bounded session completion"
for pair in \
    native_presentation=enabled \
    native_submit_failures=0 \
    native_retire_failures=0 \
    native_callback_rejected=0 \
    native_callback_queue_saturated=0 \
    native_in_flight=false \
    native_cleanup_pending=false; do
    require_field "$session" "${pair%%=*}" "${pair#*=}"
done
require_positive_field "$session" native_submissions
require_positive_field "$session" native_retirements
require_positive_field "$session" native_callback_accepted
require_positive_field "$session" native_nonzero_exports
resources="$(grep -E '^sophia_live_native_resources schema=5 status=complete ' "$evidence")"
[[ "$(printf '%s\n' "$resources" | wc -l)" == 1 ]] ||
    fail "expected one native renderer resource completion"
require_positive_field "$resources" worker_requests
require_positive_field "$resources" worker_completions
for key in worker_failures worker_hard_stalls worker_release_enqueue_failures; do
    require_field "$resources" "$key" 0
done
require_field "$resources" worker_completions "$(field "$resources" worker_requests)"
session_line="$(grep -nEm1 '^sophia_live_session schema=16 status=bounded_complete ' "$evidence" | cut -d: -f1)"
last_retire_line="$(grep -nE '^sophia_live_native_head_page_flip schema=1 status=retired ' "$evidence" | tail -n1 | cut -d: -f1)"
(( last_retire_line < session_line )) || fail "bounded completion preceded physical retirement"
grep -Eq '^sophia_live_session_health schema=1 status=clean protocol_errors=0 pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false$' \
    "$evidence" || fail "session health was not clean"
grep -Fxq 'sophia_live_output_topology_health schema=1 status=clean quarantined=false' \
    "$evidence" || fail "output topology remained quarantined"
grep -Fxq 'sophia_mirror_group_gate schema=1 status=visual_confirmed outputs=1 connectors=2 heads=2 dp1_mode=2560x1440 dp2_mode=1920x1080' \
    "$evidence" || fail "operator did not confirm the visible mirror"
grep -Fxq 'sophia_mirror_group_gate schema=1 status=passed exit=0' "$evidence" ||
    fail "gate did not record a passing exit"

echo "mirror-group physical evidence passed: $evidence"
