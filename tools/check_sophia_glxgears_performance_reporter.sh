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
[[ "$report" == *" snapshot_captures=3 "* ]]
[[ "$report" == *" snapshot_promotions=3 "* ]]
[[ "$report" == *" import_cache_imports=3 "* ]]
[[ "$report" == *" import_cache_hits=2 "* ]]
[[ "$report" == *" cursor_updates_primary_in_flight=80 "* ]]
[[ "$report" == *" cursor_max_update_msec=1" ]]

grep -v '^GL_RENDERER' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter accepted missing renderer identity" >&2
    exit 1
fi

sed \
    's/samples=3 advancing_intervals=2/samples=2 advancing_intervals=1/' \
    "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter accepted insufficient advancing cadence" >&2
    exit 1
fi

sed 's/nonadvancing=0/nonadvancing=1/' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter accepted a nonadvancing cadence" >&2
    exit 1
fi

sed 's/overflowed=false/overflowed=true/' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter accepted an overflowed cadence" >&2
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
zero_hit_report="$("$REPORTER" "$MUTATED")"
if [[ "$zero_hit_report" != *" import_cache_hits=0 "* ]]; then
    echo "glxgears reporter rejected a valid zero-hit changing-buffer workload" >&2
    exit 1
fi

sed 's/import_cache_imports=3/import_cache_imports=0/' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter accepted missing DMA-BUF import evidence" >&2
    exit 1
fi

sed 's/snapshot_promotions=3/snapshot_promotions=0/' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter accepted missing snapshot promotion" >&2
    exit 1
fi

sed 's/snapshot_live_entries=0/snapshot_live_entries=1/' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter accepted live snapshot debt" >&2
    exit 1
fi

sed 's/updates_primary_in_flight=80/updates_primary_in_flight=0/' \
    "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter accepted no cursor updates overlapping primary flips" >&2
    exit 1
fi

sed 's/max_update_msec=1/max_update_msec=21/' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter accepted a blocking legacy cursor update" >&2
    exit 1
fi

# The atomic path passes this gate, and is held to its own shape rather than
# the legacy one. An atomic cursor cannot commit while a flip is in flight --
# the kernel serializes commits per CRTC -- so a positive overlap count there
# is a contradiction, where on the legacy ioctl it is the evidence that
# pointer motion happened at all.
sed -e 's/path=legacy_ioctl/path=atomic_plane/' \
    -e 's/updates_primary_in_flight=80/updates_primary_in_flight=0/' \
    "$FIXTURE" >"$MUTATED"
if ! "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter rejected a valid atomic cursor run" >&2
    exit 1
fi

sed 's/path=legacy_ioctl/path=atomic_plane/' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter accepted an atomic cursor committed during a flip" >&2
    exit 1
fi

# Shaped so only the path restriction can reject it: with a zero overlap
# count it would satisfy the atomic branch, so if it is refused, it is refused
# for being neither hardware path.
sed -e 's/path=legacy_ioctl/path=composited/' \
    -e 's/updates_primary_in_flight=80/updates_primary_in_flight=0/' \
    "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter accepted a cursor that was neither hardware path" >&2
    exit 1
fi

sed 's/mean_fps=59.999/mean_fps=54.999/' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter accepted pointer-motion cadence below 55 FPS" >&2
    exit 1
fi

sed 's/import_cache_descriptor_mismatches=0/import_cache_descriptor_mismatches=1/' \
    "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    echo "glxgears reporter accepted a DMA-BUF descriptor mismatch" >&2
    exit 1
fi

echo "glxgears performance reporter regressions passed"
