#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORTER="$ROOT_DIR/tools/report_sophia_glxgears_performance.sh"
FIXTURE="$ROOT_DIR/tools/fixtures/sophia_glxgears_performance_pass.log"
MUTATED="$(mktemp)"
trap 'rm -f "$MUTATED"' EXIT

report="$("$REPORTER" "$FIXTURE")"
[[ "$report" == *" status=pass "* ]]
[[ "$report" == *" role=compatibility_probe "* ]]
[[ "$report" == *" client_mean_fps=59.981 "* ]]
[[ "$report" == *" present_fps=59.999 "* ]]
[[ "$report" == *" p95_frame_msec=16.667 "* ]]
[[ "$report" == *" native_mixed_exports=3 "* ]]
[[ "$report" == *" present_complete_copy=3 "* ]]
[[ "$report" == *" import_cache_hits=2 "* ]]

grep -v '^GL_RENDERER' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter accepted missing renderer identity" >&2
    exit 1
fi

sed 's/ust=1033334/ust=1016667/' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter accepted insufficient advancing cadence" >&2
    exit 1
fi

sed 's/native_mixed_exports=3/native_mixed_exports=0/' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter accepted missing mixed composition evidence" >&2
    exit 1
fi

sed 's/native_submit_failures=0/native_submit_failures=1/' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter accepted native submission failure" >&2
    exit 1
fi

sed 's/import_cache_hits=2/import_cache_hits=0/' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter accepted missing retained-image cache hit" >&2
    exit 1
fi

sed 's/import_cache_descriptor_mismatches=0/import_cache_descriptor_mismatches=1/' \
    "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter accepted a DMA-BUF descriptor mismatch" >&2
    exit 1
fi

echo "glxgears performance reporter regressions passed"
