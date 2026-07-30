#!/usr/bin/env bash
set -euo pipefail

REFRESH_BUDGET_MSEC="${SOPHIA_INPUT_LATENCY_REFRESH_MSEC:-17}"

fail() {
    echo "Sophia input latency report failed: $*" >&2
    exit 1
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

if [[ "${1:-}" == --help ]]; then
    cat <<EOF
Usage: tools/report_sophia_input_latency.sh SESSION_LOG...

Require clean libinput-to-kernel-page-flip evidence in every log and report
full-chain p95 against SOPHIA_INPUT_LATENCY_REFRESH_MSEC (default: 17).
EOF
    exit 0
fi
(($# > 0)) || fail "provide at least one session log"
[[ "$REFRESH_BUDGET_MSEC" =~ ^[1-9][0-9]*$ ]] ||
    fail "SOPHIA_INPUT_LATENCY_REFRESH_MSEC must be a positive integer"

latencies=()
max_queue_dwell=0
max_dwell_to_submit=0
max_submit_to_flip=0

for session_log in "$@"; do
    [[ -s "$session_log" ]] || fail "missing or empty session log: $session_log"
    [[ "$(grep -Fc 'sophia_live_input_latency schema=1 status=complete ' "$session_log")" -eq 1 ]] ||
        fail "expected exactly one complete latency record: $session_log"
    [[ "$(grep -Fc 'sophia_live_page_flip_clock schema=1 status=complete ' "$session_log")" -eq 1 ]] ||
        fail "expected exactly one complete page-flip clock record: $session_log"
    latency_line="$(grep -F \
        'sophia_live_input_latency schema=1 status=complete ' "$session_log")"
    clock_line="$(grep -F \
        'sophia_live_page_flip_clock schema=1 status=complete ' "$session_log")"
    [[ "$(field "$latency_line" source)" == libinput_to_kernel_page_flip ]] ||
        fail "latency record used the wrong source: $session_log"
    [[ "$(field "$clock_line" source)" == kernel_monotonic ]] ||
        fail "page-flip record used the wrong clock: $session_log"

    full_chain="$(field "$latency_line" full_chain_msec)"
    queue_dwell="$(field "$latency_line" queue_dwell_msec)"
    dwell_to_submit="$(field "$latency_line" dwell_to_submit_msec)"
    submit_to_flip="$(field "$latency_line" submit_to_page_flip_msec)"
    timestamps="$(field "$clock_line" timestamps)"
    fallbacks="$(field "$clock_line" fallbacks)"
    pending="$(field "$clock_line" pending)"
    for value in "$full_chain" "$queue_dwell" "$dwell_to_submit" \
        "$submit_to_flip" "$timestamps" "$fallbacks" "$pending"; do
        [[ "$value" =~ ^[0-9]+$ ]] ||
            fail "latency record contains malformed numeric evidence: $session_log"
    done
    ((timestamps > 0 && fallbacks == 0 && pending == 0)) ||
        fail "page-flip clock evidence is incomplete: $session_log"
    ((queue_dwell + dwell_to_submit + submit_to_flip <= full_chain)) ||
        fail "stage timings exceed the full chain: $session_log"

    latencies+=("$full_chain")
    ((queue_dwell > max_queue_dwell)) && max_queue_dwell=$queue_dwell
    ((dwell_to_submit > max_dwell_to_submit)) &&
        max_dwell_to_submit=$dwell_to_submit
    ((submit_to_flip > max_submit_to_flip)) &&
        max_submit_to_flip=$submit_to_flip
done

samples="${#latencies[@]}"
mapfile -t sorted < <(printf '%s\n' "${latencies[@]}" | sort -n)
p95_rank=$(((95 * samples + 99) / 100))
p95="${sorted[p95_rank - 1]}"
maximum="${sorted[samples - 1]}"
status=passed
exit_status=0
if ((p95 >= REFRESH_BUDGET_MSEC)); then
    status=failed
    exit_status=1
fi
printf 'sophia_input_latency_report schema=1 status=%s samples=%s p95_msec=%s max_msec=%s refresh_budget_msec=%s max_queue_dwell_msec=%s max_dwell_to_submit_msec=%s max_submit_to_page_flip_msec=%s\n' \
    "$status" "$samples" "$p95" "$maximum" "$REFRESH_BUDGET_MSEC" \
    "$max_queue_dwell" "$max_dwell_to_submit" "$max_submit_to_flip"
exit "$exit_status"
