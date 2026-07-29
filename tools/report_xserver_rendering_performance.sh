#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=tools/lib/rendering_performance.sh
source "$ROOT_DIR/tools/lib/rendering_performance.sh"

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
LOG_DIR="${SOPHIA_XSERVER_BENCHMARK_LOG_DIR:-$STATE_HOME/sophia/rendering-reference}"
BENCHMARK_LOG="${1:-$LOG_DIR/xserver-vkcube.log}"

fail() {
    echo "Xserver rendering performance report failed: $*" >&2
    exit 1
}

[[ -s "$BENCHMARK_LOG" ]] || fail "missing benchmark log: $BENCHMARK_LOG"

metadata="$(
    grep -E '^xserver_rendering_benchmark schema=1 ' "$BENCHMARK_LOG" |
        tail -n 1
)"
[[ -n "$metadata" ]] || fail "missing benchmark metadata"
environment="$(
    grep -E '^xserver_present_environment schema=1 ' "$BENCHMARK_LOG" |
        tail -n 1
)"
[[ -n "$environment" ]] || fail "missing Xserver environment"
completion="$(
    grep -E '^xserver_present_probe schema=1 status=complete ' "$BENCHMARK_LOG" |
        tail -n 1
)"
[[ -n "$completion" ]] || fail "missing successful Present probe completion"

vendor="$(rendering_performance_field "$environment" vendor)" ||
    fail "environment lacks server vendor"
[[ "$vendor" != Sophia ]] || fail "reference display is Sophia, not an Xserver"

gpu_line="$(grep -E '^Selected GPU [0-9]+: ' "$BENCHMARK_LOG" | head -n 1)"
[[ -n "$gpu_line" ]] || fail "missing selected Vulkan provider"
gpu_identity="${gpu_line#*: }"
gpu_sha256="$(printf '%s' "$gpu_identity" | sha256sum | awk '{print $1}')"

TIMESTAMPS_FILE="$(mktemp)"
INTERVALS_FILE="$(mktemp)"
trap 'rm -f "$TIMESTAMPS_FILE" "$INTERVALS_FILE" "$INTERVALS_FILE.fps"' EXIT

awk '
    /^xserver_present_feedback schema=1 kind=complete / &&
        (/ mode=Flip / || / mode=Copy / || / mode=SuboptimalCopy /) {
        for (field_index = 1; field_index <= NF; field_index++) {
            if ($field_index ~ /^ust=[0-9]+$/) {
                split($field_index, pair, "=")
                print pair[2]
            }
        }
    }
' "$BENCHMARK_LOG" >"$TIMESTAMPS_FILE"

if ! read -r timestamp_count fps p95_msec < <(
    rendering_performance_cadence "$TIMESTAMPS_FILE" "$INTERVALS_FILE"
); then
    fail "need at least three advancing Present completion timestamps"
fi

requested_frames="$(rendering_performance_field "$metadata" requested_frames)" ||
    fail "metadata lacks requested_frames"
surface_width="$(rendering_performance_field "$metadata" surface_width)" ||
    fail "metadata lacks surface_width"
surface_height="$(rendering_performance_field "$metadata" surface_height)" ||
    fail "metadata lacks surface_height"
vulkan_present_mode="$(rendering_performance_field "$metadata" vulkan_present_mode)" ||
    fail "metadata lacks vulkan_present_mode"
flips="$(rendering_performance_field "$completion" flips)" ||
    fail "completion lacks flips"
copies="$(rendering_performance_field "$completion" copies)" ||
    fail "completion lacks copies"
skips="$(rendering_performance_field "$completion" skips)" ||
    fail "completion lacks skips"
presentation_path=Mixed
if ((flips > 0 && copies == 0)); then
    presentation_path=Flip
elif ((copies > 0 && flips == 0)); then
    presentation_path=Copy
fi
output_width="$(rendering_performance_field "$completion" output_width)" ||
    fail "completion lacks output_width"
output_height="$(rendering_performance_field "$completion" output_height)" ||
    fail "completion lacks output_height"
output_source="$(rendering_performance_field "$completion" output_source)" ||
    fail "completion lacks output_source"
output_pixels="$((output_width * output_height))"

printf '%s\n' \
    "xserver_rendering_performance schema=2 status=pass workload=vkcube-xcb server_vendor=$vendor requested_frames=$requested_frames surface_width=$surface_width surface_height=$surface_height vulkan_present_mode=$vulkan_present_mode x_present_path=$presentation_path gpu_sha256=$gpu_sha256 output_width=$output_width output_height=$output_height output_pixels=$output_pixels output_source=$output_source present_samples=$timestamp_count fps=$fps p95_frame_msec=$p95_msec flips=$flips copies=$copies skips=$skips"
