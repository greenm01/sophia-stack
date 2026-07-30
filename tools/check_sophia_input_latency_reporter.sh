#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_DIR="$(mktemp -d)"
trap 'rm -rf -- "$FIXTURE_DIR"' EXIT

write_sample() {
    local path="$1" latency="$2" fallbacks="${3:-0}"
    printf '%s\n' \
        "sophia_live_input_latency schema=1 status=complete source=libinput_to_kernel_page_flip ingress_msec=100 queue_dwell_msec=1 dwell_to_submit_msec=2 submit_to_page_flip_msec=5 full_chain_msec=$latency" \
        "sophia_live_page_flip_clock schema=1 status=complete source=kernel_monotonic timestamps=4 fallbacks=$fallbacks pending=0" \
        >"$path"
}

logs=()
for sample in $(seq 1 20); do
    log="$FIXTURE_DIR/sample-$sample.log"
    write_sample "$log" "$((8 + sample / 10))"
    logs+=("$log")
done

report="$("$ROOT_DIR/tools/report_sophia_input_latency.sh" "${logs[@]}")"
grep -Fq \
    'sophia_input_latency_report schema=1 status=passed samples=20 p95_msec=9 max_msec=10 refresh_budget_msec=17' \
    <<<"$report"

write_sample "${logs[18]}" 17
write_sample "${logs[19]}" 17
if "$ROOT_DIR/tools/report_sophia_input_latency.sh" "${logs[@]}" >/dev/null 2>&1; then
    echo "input latency reporter accepted a p95 at the refresh budget" >&2
    exit 1
fi

write_sample "${logs[18]}" 10
write_sample "${logs[19]}" 10 1
if "$ROOT_DIR/tools/report_sophia_input_latency.sh" "${logs[@]}" >/dev/null 2>&1; then
    echo "input latency reporter accepted a fallback page-flip timestamp" >&2
    exit 1
fi

echo "Sophia input latency reporter checks passed"
