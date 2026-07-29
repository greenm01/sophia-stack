#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
XSERVER_REPORTER="$ROOT_DIR/tools/report_xserver_rendering_performance.sh"
COMPARATOR="$ROOT_DIR/tools/compare_sophia_xserver_rendering.sh"
SOPHIA_FIXTURE="$ROOT_DIR/tools/fixtures/rendering_performance_pass.log"
XSERVER_FIXTURE="$ROOT_DIR/tools/fixtures/xserver_rendering_performance_pass.log"

report="$("$XSERVER_REPORTER" "$XSERVER_FIXTURE")"
[[ "$report" == *" status=pass "* ]]
[[ "$report" == *" schema=2 "* ]]
[[ "$report" == *" server_vendor=The_X.Org_Foundation "* ]]
[[ "$report" == *" requested_frames=900 "* ]]
[[ "$report" == *" output_width=2560 output_height=1440 "* ]]
[[ "$report" == *" fps=59.999 "* ]]
[[ "$report" == *" p95_frame_msec=16.667 "* ]]
[[ "$report" == *" x_present_path=Copy "* ]]

comparison="$("$COMPARATOR" "$SOPHIA_FIXTURE" "$XSERVER_FIXTURE")"
[[ "$comparison" == *" status=pass "* ]]
[[ "$comparison" == *" comparability=cadence_only "* ]]
[[ "$comparison" == *" path_match=false "* ]]
[[ "$comparison" == *" fps_ratio=1.0000 "* ]]
[[ "$comparison" == *" p95_ratio=1.0000 "* ]]

MISMATCH_FIXTURE="$(mktemp)"
trap 'rm -f "$MISMATCH_FIXTURE"' EXIT
sed 's/LLVM fixture/other provider/' "$XSERVER_FIXTURE" >"$MISMATCH_FIXTURE"
if "$COMPARATOR" "$SOPHIA_FIXTURE" "$MISMATCH_FIXTURE" >/dev/null 2>&1; then
    echo "rendering comparison accepted a Vulkan-provider mismatch" >&2
    exit 1
fi
if SOPHIA_RENDER_MIN_BASELINE_RATIO=1.01 \
    "$COMPARATOR" "$SOPHIA_FIXTURE" "$XSERVER_FIXTURE" >/dev/null 2>&1; then
    echo "rendering comparison accepted a below-threshold cadence" >&2
    exit 1
fi

PROBE_BIN="$(mktemp)"
xcb_flags=()
read -r -a xcb_flags <<<"$(pkg-config --cflags --libs xcb xcb-present xcb-randr)"
cc -std=c11 -D_POSIX_C_SOURCE=200809L -Wall -Wextra -Werror \
    "$ROOT_DIR/tools/probes/xserver_present_probe.c" \
    -o "$PROBE_BIN" \
    "${xcb_flags[@]}"
rm -f "$PROBE_BIN"

echo "Xserver rendering performance reporter regressions passed"
