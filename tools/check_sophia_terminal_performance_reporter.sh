#!/usr/bin/env bash
set -euo pipefail

# Offline regression for report_sophia_terminal_performance.sh. Requires no TTY,
# GPU, or X server: it drives the reporter against a static fixture and against
# mutated fixtures that must fail closed.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORTER="$ROOT_DIR/tools/report_sophia_terminal_performance.sh"
FIXTURE="$ROOT_DIR/tools/fixtures/sophia_terminal_performance_pass.log"

fail() {
    echo "terminal performance reporter regression failed: $*" >&2
    exit 1
}

report="$("$REPORTER" "$FIXTURE")"
[[ "$report" == *" status=pass "* ]] || fail "pass fixture did not pass"
[[ "$report" == *" schema=1 "* ]] || fail "missing schema"
[[ "$report" == *" workload=xterm-cpu "* ]] || fail "missing workload"
[[ "$report" == *" duration_seconds=20 "* ]] || fail "missing duration"
[[ "$report" == *" surface_width=500 surface_height=500 "* ]] || fail "missing surface size"
[[ "$report" == *" client_lines=120000 "* ]] || fail "missing client lines"
[[ "$report" == *" cpu_patch_updates=539 "* ]] || fail "missing patch updates"
[[ "$report" == *" cpu_payload_bytes=13271040 "* ]] || fail "missing payload bytes"
[[ "$report" == *" cpu_max_compose_msec=4 "* ]] || fail "missing compose max"
[[ "$report" == *" partial_repaints=2 full_repaints=1 "* ]] || fail "missing repaint counts"
[[ "$report" == *" present_fps=58.900 "* ]] || fail "missing present cadence"

MUTATED="$(mktemp)"
trap 'rm -f "$MUTATED"' EXIT

# Fails closed when the immutable patch-batch path was never exercised.
sed 's/cpu_patch_updates=539/cpu_patch_updates=0/' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    fail "reporter accepted zero CPU patch traffic"
fi

# Fails closed when the CPU path repainted full frames only (no damage-driven
# partial repaint).
grep -v 'mode=partial' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    fail "reporter accepted a run with no partial repaint"
fi

# Fails closed on unexpected protocol errors.
sed 's/unexpected=0/unexpected=1/' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    fail "reporter accepted unexpected protocol errors"
fi

# Fails closed when the bounded client did not complete its window.
sed 's/timed_exit=true/timed_exit=false/' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    fail "reporter accepted an unbounded client run"
fi

echo "terminal performance reporter regressions passed"
