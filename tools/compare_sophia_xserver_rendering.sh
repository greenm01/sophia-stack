#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=tools/lib/rendering_performance.sh
source "$ROOT_DIR/tools/lib/rendering_performance.sh"

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
SOPHIA_LOG="${1:-${SOPHIA_STANDALONE_LOG_DIR:-$STATE_HOME/sophia/standalone-session}/session.log}"
XSERVER_LOG="${2:-${SOPHIA_XSERVER_BENCHMARK_LOG_DIR:-$STATE_HOME/sophia/rendering-reference}/xserver-vkcube.log}"
MIN_BASELINE_RATIO="${SOPHIA_RENDER_MIN_BASELINE_RATIO:-0.90}"

fail() {
    echo "Sophia/Xserver rendering comparison failed: $*" >&2
    exit 1
}

sophia_report="$("$ROOT_DIR/tools/report_sophia_rendering_performance.sh" "$SOPHIA_LOG")"
xserver_report="$("$ROOT_DIR/tools/report_xserver_rendering_performance.sh" "$XSERVER_LOG")"

for field_name in workload requested_frames surface_width surface_height \
    vulkan_present_mode gpu_sha256 output_pixels; do
    sophia_value="$(
        rendering_performance_field "$sophia_report" "$field_name"
    )" || fail "Sophia report lacks $field_name"
    xserver_value="$(
        rendering_performance_field "$xserver_report" "$field_name"
    )" || fail "Xserver report lacks $field_name"
    [[ "$sophia_value" != unknown ]] ||
        fail "Sophia report has no comparable $field_name; rerun the bounded benchmark"
    [[ "$sophia_value" == "$xserver_value" ]] ||
        fail "$field_name mismatch: Sophia=$sophia_value Xserver=$xserver_value"
done

xserver_present_path="$(
    rendering_performance_field "$xserver_report" x_present_path
)" || fail "Xserver report lacks x_present_path"
sophia_present_path="$(
    rendering_performance_field "$sophia_report" x_present_path
)" || fail "Sophia report lacks x_present_path"
path_match=false
[[ "$sophia_present_path" == "$xserver_present_path" ]] && path_match=true
requested_frames="$(
    rendering_performance_field "$sophia_report" requested_frames
)"
sophia_fps="$(rendering_performance_field "$sophia_report" fps)" ||
    fail "Sophia report lacks fps"
xserver_fps="$(rendering_performance_field "$xserver_report" fps)" ||
    fail "Xserver report lacks fps"
sophia_p95="$(rendering_performance_field "$sophia_report" p95_frame_msec)" ||
    fail "Sophia report lacks p95_frame_msec"
xserver_p95="$(rendering_performance_field "$xserver_report" p95_frame_msec)" ||
    fail "Xserver report lacks p95_frame_msec"

fps_ratio="$(
    awk -v observed="$sophia_fps" -v baseline="$xserver_fps" \
        'BEGIN { printf "%.4f", observed / baseline }'
)"
p95_ratio="$(
    awk -v observed="$sophia_p95" -v baseline="$xserver_p95" \
        'BEGIN { printf "%.4f", baseline / observed }'
)"
status=pass
awk -v ratio="$fps_ratio" -v minimum="$MIN_BASELINE_RATIO" \
    'BEGIN { exit !(ratio >= minimum) }' || status=below_baseline
awk -v ratio="$p95_ratio" -v minimum="$MIN_BASELINE_RATIO" \
    'BEGIN { exit !(ratio >= minimum) }' || status=below_baseline

printf '%s\n' \
    "sophia_xserver_rendering_comparison schema=2 status=$status comparability=cadence_only sophia_present_path=$sophia_present_path xserver_present_path=$xserver_present_path path_match=$path_match workload=vkcube-xcb requested_frames=$requested_frames sophia_fps=$sophia_fps xserver_fps=$xserver_fps fps_ratio=$fps_ratio sophia_p95_msec=$sophia_p95 xserver_p95_msec=$xserver_p95 p95_ratio=$p95_ratio min_baseline_ratio=$MIN_BASELINE_RATIO"

[[ "$status" == pass ]] ||
    fail "Sophia cadence is below the configured same-provider Xserver gate"
