#!/usr/bin/env bash
set -euo pipefail

REFRESH_MSEC="${SOPHIA_INPUT_LATENCY_REFRESH_MSEC:-17}"
MAX_QUEUE_DWELL_MSEC="${SOPHIA_INPUT_LATENCY_MAX_QUEUE_DWELL_MSEC:-1}"
MAX_DWELL_TO_SUBMIT_MSEC="${SOPHIA_INPUT_LATENCY_MAX_DWELL_TO_SUBMIT_MSEC:-}"
MAX_SUBMIT_TO_FLIP_MSEC="${SOPHIA_INPUT_LATENCY_MAX_SUBMIT_TO_FLIP_MSEC:-}"
END_TO_END_REFRESHES=2

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
full-chain p95 below two refresh periods. The default 17 ms refresh also gates
queue dwell at 1 ms, dwell-to-submit at 8 ms, and submit-to-flip at 17 ms.
EOF
    exit 0
fi
(($# > 0)) || fail "provide at least one session log"
[[ "$REFRESH_MSEC" =~ ^[1-9][0-9]*$ ]] ||
    fail "SOPHIA_INPUT_LATENCY_REFRESH_MSEC must be a positive integer"
if [[ -z "$MAX_DWELL_TO_SUBMIT_MSEC" ]]; then
    MAX_DWELL_TO_SUBMIT_MSEC=$((REFRESH_MSEC / 2))
    ((MAX_DWELL_TO_SUBMIT_MSEC > 0)) || MAX_DWELL_TO_SUBMIT_MSEC=1
fi
if [[ -z "$MAX_SUBMIT_TO_FLIP_MSEC" ]]; then
    MAX_SUBMIT_TO_FLIP_MSEC="$REFRESH_MSEC"
fi
for budget in "$MAX_QUEUE_DWELL_MSEC" "$MAX_DWELL_TO_SUBMIT_MSEC" \
    "$MAX_SUBMIT_TO_FLIP_MSEC"; do
    [[ "$budget" =~ ^[0-9]+$ ]] ||
        fail "stage latency budgets must be nonnegative integers"
done
END_TO_END_BUDGET_MSEC=$((REFRESH_MSEC * END_TO_END_REFRESHES))

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
failed_gates=()
if ((p95 >= END_TO_END_BUDGET_MSEC)); then
    failed_gates+=(full_chain_p95)
fi
if ((max_queue_dwell > MAX_QUEUE_DWELL_MSEC)); then
    failed_gates+=(queue_dwell)
fi
if ((max_dwell_to_submit > MAX_DWELL_TO_SUBMIT_MSEC)); then
    failed_gates+=(dwell_to_submit)
fi
if ((max_submit_to_flip > MAX_SUBMIT_TO_FLIP_MSEC)); then
    failed_gates+=(submit_to_page_flip)
fi
if ((${#failed_gates[@]} > 0)); then
    status=failed
    exit_status=1
fi
failed_gate_summary=none
if ((${#failed_gates[@]} > 0)); then
    printf -v failed_gate_summary '%s,' "${failed_gates[@]}"
    failed_gate_summary="${failed_gate_summary%,}"
fi
printf 'sophia_input_latency_report schema=2 status=%s failed_gates=%s samples=%s p95_msec=%s max_msec=%s refresh_msec=%s end_to_end_budget_refreshes=%s end_to_end_budget_msec=%s max_queue_dwell_msec=%s queue_dwell_budget_msec=%s max_dwell_to_submit_msec=%s dwell_to_submit_budget_msec=%s max_submit_to_page_flip_msec=%s submit_to_page_flip_budget_msec=%s\n' \
    "$status" "$failed_gate_summary" "$samples" "$p95" "$maximum" \
    "$REFRESH_MSEC" "$END_TO_END_REFRESHES" "$END_TO_END_BUDGET_MSEC" \
    "$max_queue_dwell" "$MAX_QUEUE_DWELL_MSEC" \
    "$max_dwell_to_submit" "$MAX_DWELL_TO_SUBMIT_MSEC" \
    "$max_submit_to_flip" "$MAX_SUBMIT_TO_FLIP_MSEC"
exit "$exit_status"
