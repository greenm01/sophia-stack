#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_DIR="$(mktemp -d)"
trap 'rm -rf -- "$FIXTURE_DIR"' EXIT

# A session's own distribution is what a percentile is taken over; the
# single-correlation record and the clock record remain its preconditions.
DISTRIBUTION_SAMPLES="${DISTRIBUTION_SAMPLES:-400}"
DISTRIBUTION_P99_USEC="${DISTRIBUTION_P99_USEC:-9000}"
DISTRIBUTION_DWELL_USEC="${DISTRIBUTION_DWELL_USEC:-1000}"
DISTRIBUTION_FLIP_USEC="${DISTRIBUTION_FLIP_USEC:-5000}"
REFRESH_MILLIHZ="${REFRESH_MILLIHZ:-60000}"

write_sample() {
    local path="$1" latency="$2" fallbacks="${3:-0}"
    local queue_dwell="${4:-1}" dwell_to_submit="${5:-2}" submit_to_flip="${6:-5}"
    printf '%s\n' \
        "sophia_live_input_latency schema=1 status=complete source=libinput_to_kernel_page_flip ingress_msec=100 queue_dwell_msec=$queue_dwell dwell_to_submit_msec=$dwell_to_submit submit_to_page_flip_msec=$submit_to_flip full_chain_msec=$latency" \
        "sophia_live_page_flip_clock schema=1 status=complete source=kernel_monotonic timestamps=4 fallbacks=$fallbacks pending=0" \
        >"$path"
}

write_distribution_sample() {
    local path="$1" latency="$2"
    write_sample "$path" "$latency"
    printf '%s\n' \
        "sophia_live_native_head schema=2 status=ready output=1 head=1 connector=DP-1 connector_id=1 mode=2560x1440 refresh_millihz=$REFRESH_MILLIHZ mirrored=false" \
        "sophia_live_input_latency_distribution schema=1 status=complete source=libinput_to_kernel_page_flip samples=$DISTRIBUTION_SAMPLES evicted=0 abandoned=0 unsettled=0 min_usec=2000 p50_usec=4000 p95_usec=6000 p99_usec=$DISTRIBUTION_P99_USEC max_usec=12000 max_queue_dwell_usec=$DISTRIBUTION_DWELL_USEC max_submit_to_page_flip_usec=$DISTRIBUTION_FLIP_USEC" \
        >>"$path"
}

logs=()
for sample in $(seq 1 20); do
    log="$FIXTURE_DIR/sample-$sample.log"
    write_distribution_sample "$log" "$((8 + sample / 10))"
    logs+=("$log")
done

# 60000 millihertz is a 17 ms refresh, read from the session rather than the
# environment; the percentile comes from the sessions' own distributions.
report="$("$ROOT_DIR/tools/report_sophia_input_latency.sh" "${logs[@]}")"
grep -Fq \
    'sophia_input_latency_report schema=3 status=passed failed_gates=none samples=8000 percentile_source=distribution refresh_source=measured p99_msec=9 max_msec=12 refresh_msec=17 end_to_end_budget_refreshes=2 end_to_end_budget_msec=34 max_queue_dwell_msec=1 queue_dwell_budget_msec=1 max_dwell_to_submit_msec=2 dwell_to_submit_budget_msec=17 max_submit_to_page_flip_msec=5 submit_to_page_flip_budget_msec=17' \
    <<<"$report"

# Rebuild the fixture with one distribution parameter changed, so each control
# exercises the population the gate actually reads.
rebuild() {
    logs=()
    for sample in $(seq 1 20); do
        log="$FIXTURE_DIR/sample-$sample.log"
        write_distribution_sample "$log" "$((8 + sample / 10))"
        logs+=("$log")
    done
}

reject() {
    local description="$1" expected="$2"
    local report
    if report="$("$ROOT_DIR/tools/report_sophia_input_latency.sh" "${logs[@]}" 2>&1)"; then
        echo "input latency reporter accepted $description" >&2
        exit 1
    fi
    grep -Fq "$expected" <<<"$report" || {
        echo "input latency reporter rejected $description for the wrong reason:" >&2
        echo "  expected: $expected" >&2
        echo "  observed: $report" >&2
        exit 1
    }
}

# A p99 at the two-refresh budget is not below it.
DISTRIBUTION_P99_USEC=34000 rebuild
reject "a p99 at the two-refresh budget" "failed_gates=full_chain_p99"

# The percentile must rest on a population. Twenty samples make p99 the
# maximum, which is a bound and not a percentile.
DISTRIBUTION_SAMPLES=1 rebuild
reject "a p99 over too few samples" "insufficient_samples"

# Stage gates still bite, read from the distribution's own maxima.
DISTRIBUTION_DWELL_USEC=2000 rebuild
reject "excessive queue dwell" "failed_gates=queue_dwell"

DISTRIBUTION_FLIP_USEC=18000 rebuild
reject "excessive submit-to-flip latency" "submit_to_page_flip"

# A faster display tightens every derived budget: at 144 Hz two refreshes are
# 14 ms, so a 9 ms p99 passes but the 17 ms-derived budget no longer applies.
REFRESH_MILLIHZ=144000 DISTRIBUTION_P99_USEC=13000 rebuild
report="$("$ROOT_DIR/tools/report_sophia_input_latency.sh" "${logs[@]}")"
grep -Fq 'refresh_source=measured' <<<"$report" &&
    grep -Fq 'refresh_msec=7 end_to_end_budget_refreshes=2 end_to_end_budget_msec=14' \
        <<<"$report" || {
    echo "input latency reporter did not derive the budget from the measured refresh" >&2
    echo "  observed: $report" >&2
    exit 1
}

REFRESH_MILLIHZ=144000 DISTRIBUTION_P99_USEC=15000 rebuild
reject "a p99 above two refreshes on a faster display" "failed_gates=full_chain_p99"

# Preconditions still hold on the single-correlation and clock records.
rebuild
write_sample "${logs[19]}" 10 1
reject "a fallback page-flip timestamp" "page-flip clock evidence is incomplete"

# Evidence predating the distribution falls back to one value per session, and
# twenty of those cannot support a p99.
logs=()
for sample in $(seq 1 20); do
    log="$FIXTURE_DIR/legacy-$sample.log"
    write_sample "$log" "$((8 + sample / 10))"
    logs+=("$log")
done
reject "a legacy per-session population as a p99" "insufficient_samples"

echo "Sophia input latency reporter checks passed"
