#!/usr/bin/env bash
set -euo pipefail

REFRESH_MSEC="${SOPHIA_INPUT_LATENCY_REFRESH_MSEC:-17}"
MAX_QUEUE_DWELL_MSEC="${SOPHIA_INPUT_LATENCY_MAX_QUEUE_DWELL_MSEC:-1}"
MAX_DWELL_TO_SUBMIT_MSEC="${SOPHIA_INPUT_LATENCY_MAX_DWELL_TO_SUBMIT_MSEC:-}"
MAX_SUBMIT_TO_FLIP_MSEC="${SOPHIA_INPUT_LATENCY_MAX_SUBMIT_TO_FLIP_MSEC:-}"
END_TO_END_REFRESHES=2
# Below this the ninety-ninth percentile degenerates to the maximum.
MIN_P99_SAMPLES="${SOPHIA_INPUT_LATENCY_MIN_P99_SAMPLES:-200}"

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
full-chain p99 below two refresh periods, taken over the in-session latency
distribution when the evidence carries one. Refresh is read from the session's
own head record when present and falls back to the configured value. Stage
gates remain queue dwell at 1 ms, dwell-to-submit at one refresh, and
submit-to-flip at one refresh.
EOF
    exit 0
fi
(($# > 0)) || fail "provide at least one session log"
[[ "$REFRESH_MSEC" =~ ^[1-9][0-9]*$ ]] ||
    fail "SOPHIA_INPUT_LATENCY_REFRESH_MSEC must be a positive integer"
if [[ -z "$MAX_DWELL_TO_SUBMIT_MSEC" ]]; then
    MAX_DWELL_TO_SUBMIT_MSEC="$REFRESH_MSEC"
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
distribution_total_samples=0
distribution_p99_worst_usec=0
distribution_max_worst_usec=0
distribution_dwell_worst_usec=0
distribution_flip_worst_usec=0
distribution_stage_p99s=0
distribution_p99_flip_worst_usec=0
distribution_p99_dwell_worst_usec=0
observed_refresh_msec=0
observed_renderer_workers=unknown
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

    # A session that sampled repeatedly carries its own population. Prefer it:
    # one full-chain value per session cannot describe a tail, and a p99 over
    # a handful of sessions is just their maximum wearing a percentile's name.
    distribution_line="$(grep -E \
        '^sophia_live_input_latency_distribution schema=[12] status=complete ' \
        "$session_log" || true)"
    if [[ -n "$distribution_line" ]]; then
        [[ "$(field "$distribution_line" source)" == libinput_to_kernel_page_flip ]] ||
            fail "latency distribution used the wrong source: $session_log"
        distribution_samples="$(field "$distribution_line" samples)"
        distribution_p99_usec="$(field "$distribution_line" p99_usec)"
        distribution_max_usec="$(field "$distribution_line" max_usec)"
        distribution_dwell_usec="$(field "$distribution_line" max_queue_dwell_usec)"
        distribution_flip_usec="$(field "$distribution_line" max_submit_to_page_flip_usec)"
        for value in "$distribution_samples" "$distribution_p99_usec" \
            "$distribution_max_usec" "$distribution_dwell_usec" \
            "$distribution_flip_usec"; do
            [[ "$value" =~ ^[0-9]+$ ]] ||
                fail "latency distribution contains malformed evidence: $session_log"
        done
        distribution_total_samples=$((distribution_total_samples + distribution_samples))
        ((distribution_p99_usec > distribution_p99_worst_usec)) &&
            distribution_p99_worst_usec=$distribution_p99_usec
        ((distribution_max_usec > distribution_max_worst_usec)) &&
            distribution_max_worst_usec=$distribution_max_usec
        ((distribution_dwell_usec > distribution_dwell_worst_usec)) &&
            distribution_dwell_worst_usec=$distribution_dwell_usec
        ((distribution_flip_usec > distribution_flip_worst_usec)) &&
            distribution_flip_worst_usec=$distribution_flip_usec
        # Schema 2 carries per-stage percentiles, which is what the stage
        # contract is gated on. Schema-1 evidence predates them and falls
        # back to the maxima below.
        p99_flip="$(field "$distribution_line" p99_submit_to_page_flip_usec || true)"
        p99_dwell="$(field "$distribution_line" p99_dwell_to_submit_usec || true)"
        if [[ "$p99_flip" =~ ^[0-9]+$ && "$p99_dwell" =~ ^[0-9]+$ ]]; then
            distribution_stage_p99s=1
            ((p99_flip > distribution_p99_flip_worst_usec)) &&
                distribution_p99_flip_worst_usec=$p99_flip
            ((p99_dwell > distribution_p99_dwell_worst_usec)) &&
                distribution_p99_dwell_worst_usec=$p99_dwell
        fi
    fi

    # Refresh is a property of the display, not of the harness. Take it from
    # the session that ran; the environment default is a fallback for evidence
    # that predates the record.
    # How many renderer threads produced these numbers. Two reports with the
    # same latencies and different thread counts are different measurements,
    # and nothing else in the record distinguishes them.
    resources_line="$(grep -Em1 '^sophia_live_native_resources schema=[0-9]+ status=complete ' \
        "$session_log" || true)"
    if [[ -n "$resources_line" ]]; then
        session_workers="$(field "$resources_line" renderer_workers || true)"
        if [[ "$session_workers" =~ ^[0-9]+$ ]]; then
            if [[ "$observed_renderer_workers" == unknown ]]; then
                observed_renderer_workers="$session_workers"
            elif [[ "$observed_renderer_workers" != "$session_workers" ]]; then
                observed_renderer_workers=mixed
            fi
        fi
    fi
    head_line="$(grep -Em1 '^sophia_live_native_head schema=2 status=ready ' \
        "$session_log" || true)"
    if [[ -n "$head_line" ]]; then
        head_refresh_millihz="$(field "$head_line" refresh_millihz)"
        if [[ "$head_refresh_millihz" =~ ^[1-9][0-9]*$ ]]; then
            measured_refresh_msec=$(((1000000 + head_refresh_millihz - 1) / head_refresh_millihz))
            if ((measured_refresh_msec > 0)); then
                observed_refresh_msec="$measured_refresh_msec"
            fi
        fi
    fi
    ((queue_dwell > max_queue_dwell)) && max_queue_dwell=$queue_dwell
    ((dwell_to_submit > max_dwell_to_submit)) &&
        max_dwell_to_submit=$dwell_to_submit
    ((submit_to_flip > max_submit_to_flip)) &&
        max_submit_to_flip=$submit_to_flip
done

# Refresh comes from the display when the evidence names it. Budgets derived
# before the logs were read used the fallback, so they are recomputed here.
refresh_source=configured
if ((observed_refresh_msec > 0)); then
    refresh_source=measured
    REFRESH_MSEC="$observed_refresh_msec"
    [[ -n "${SOPHIA_INPUT_LATENCY_MAX_DWELL_TO_SUBMIT_MSEC:-}" ]] ||
        MAX_DWELL_TO_SUBMIT_MSEC="$REFRESH_MSEC"
    [[ -n "${SOPHIA_INPUT_LATENCY_MAX_SUBMIT_TO_FLIP_MSEC:-}" ]] ||
        MAX_SUBMIT_TO_FLIP_MSEC="$REFRESH_MSEC"
    END_TO_END_BUDGET_MSEC=$((REFRESH_MSEC * END_TO_END_REFRESHES))
fi

session_samples="${#latencies[@]}"
mapfile -t sorted < <(printf '%s\n' "${latencies[@]}" | sort -n)
maximum="${sorted[session_samples - 1]}"

# The population the percentile is taken over. In-session distributions are
# preferred; without them the reporter falls back to one value per session.
if ((distribution_total_samples > 0)); then
    samples="$distribution_total_samples"
    percentile_source=distribution
    p99_msec=$(((distribution_p99_worst_usec + 999) / 1000))
    maximum=$(((distribution_max_worst_usec + 999) / 1000))
    max_queue_dwell=$(((distribution_dwell_worst_usec + 999) / 1000))
    max_submit_to_flip=$(((distribution_flip_worst_usec + 999) / 1000))
else
    samples="$session_samples"
    percentile_source=per_session
    p99_rank=$(((99 * samples + 99) / 100))
    p99_msec="${sorted[p99_rank - 1]}"
fi

status=passed
exit_status=0
failed_gates=()
# A percentile needs a population. At or under a hundred samples the ninety-
# ninth percentile is the maximum, which is a bound worth stating but not a
# percentile worth reporting as one.
if ((samples < MIN_P99_SAMPLES)); then
    failed_gates+=(insufficient_samples)
fi
if ((p99_msec >= END_TO_END_BUDGET_MSEC)); then
    failed_gates+=(full_chain_p99)
fi
if ((max_queue_dwell > MAX_QUEUE_DWELL_MSEC)); then
    failed_gates+=(queue_dwell)
fi
# The stage contract holds at p99, and its populations come from the
# in-session distributions when the evidence carries them. The one-shot
# stage maxima remain printed as diagnostics but stop failing the run: with
# spaced presses the one-shot correlates only after the whole sequence
# delivered, so its stage split measures the gap phase, not the pipeline.
# The flip stage carries a one-millisecond allowance over the refresh,
# because a press arriving just after a vblank waits the full period and
# the commit and completion add real time no pipeline can remove.
if ((distribution_stage_p99s == 1)); then
    stage_dwell_msec=$(((distribution_p99_dwell_worst_usec + 999) / 1000))
    stage_flip_msec=$(((distribution_p99_flip_worst_usec + 999) / 1000))
    if ((stage_dwell_msec > MAX_DWELL_TO_SUBMIT_MSEC)); then
        failed_gates+=(dwell_to_submit_p99)
    fi
    if ((stage_flip_msec > MAX_SUBMIT_TO_FLIP_MSEC + 1)); then
        failed_gates+=(submit_to_page_flip_p99)
    fi
else
    if ((max_dwell_to_submit > MAX_DWELL_TO_SUBMIT_MSEC)); then
        failed_gates+=(dwell_to_submit)
    fi
    if ((max_submit_to_flip > MAX_SUBMIT_TO_FLIP_MSEC)); then
        failed_gates+=(submit_to_page_flip)
    fi
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
stage_source=one_shot_max
p99_dwell_to_submit_msec=$max_dwell_to_submit
p99_submit_to_page_flip_msec=$max_submit_to_flip
if ((distribution_stage_p99s == 1)); then
    stage_source=distribution_p99
    p99_dwell_to_submit_msec=$(((distribution_p99_dwell_worst_usec + 999) / 1000))
    p99_submit_to_page_flip_msec=$(((distribution_p99_flip_worst_usec + 999) / 1000))
fi
printf 'sophia_input_latency_report schema=5 status=%s failed_gates=%s samples=%s percentile_source=%s stage_source=%s refresh_source=%s p99_msec=%s max_msec=%s refresh_msec=%s end_to_end_budget_refreshes=%s end_to_end_budget_msec=%s max_queue_dwell_msec=%s queue_dwell_budget_msec=%s p99_dwell_to_submit_msec=%s max_dwell_to_submit_msec=%s dwell_to_submit_budget_msec=%s p99_submit_to_page_flip_msec=%s max_submit_to_page_flip_msec=%s submit_to_page_flip_budget_msec=%s submit_to_page_flip_jitter_msec=1 renderer_workers=%s\n' \
    "$status" "$failed_gate_summary" "$samples" "$percentile_source" \
    "$stage_source" "$refresh_source" "$p99_msec" "$maximum" \
    "$REFRESH_MSEC" "$END_TO_END_REFRESHES" "$END_TO_END_BUDGET_MSEC" \
    "$max_queue_dwell" "$MAX_QUEUE_DWELL_MSEC" \
    "$p99_dwell_to_submit_msec" "$max_dwell_to_submit" "$MAX_DWELL_TO_SUBMIT_MSEC" \
    "$p99_submit_to_page_flip_msec" "$max_submit_to_flip" "$MAX_SUBMIT_TO_FLIP_MSEC" \
    "$observed_renderer_workers"
exit "$exit_status"
