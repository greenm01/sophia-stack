#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORTER="$ROOT_DIR/tools/report_sophia_rendering_performance.sh"
FIXTURE="$ROOT_DIR/tools/fixtures/rendering_performance_pass.log"

report="$("$REPORTER" "$FIXTURE")"
[[ "$report" == *" status=pass "* ]]
[[ "$report" == *" schema=2 "* ]]
[[ "$report" == *" requested_frames=900 "* ]]
[[ "$report" == *" surface_width=500 surface_height=500 "* ]]
[[ "$report" == *" fps=59.999 "* ]]
[[ "$report" == *" p95_frame_msec=16.667 "* ]]
[[ "$report" == *" cpu_replacements=1 "* ]]
[[ "$report" == *" cpu_patch_updates=2 "* ]]

SOPHIA_RENDER_BASELINE_FPS=60 "$REPORTER" "$FIXTURE" >/dev/null
if SOPHIA_RENDER_BASELINE_FPS=70 "$REPORTER" "$FIXTURE" >/dev/null 2>&1; then
    echo "rendering performance reporter accepted a below-baseline run" >&2
    exit 1
fi

echo "rendering performance reporter regressions passed"
