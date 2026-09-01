#!/usr/bin/env bash
set -euo pipefail

# Offline regression for report_sophia_terminal_performance.sh. Requires no TTY,
# GPU, or X server: it drives the reporter against a static fixture and against
# mutated fixtures that must fail closed.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORTER="$ROOT_DIR/tools/report_sophia_terminal_performance.sh"
FIXTURE="$ROOT_DIR/tools/fixtures/sophia_terminal_performance_pass.log"
STARTUP_ONLY_FIXTURE="$ROOT_DIR/tools/fixtures/sophia_terminal_performance_startup_only.log"

fail() {
    echo "terminal performance reporter regression failed: $*" >&2
    exit 1
}

report="$("$REPORTER" "$FIXTURE")"
[[ "$report" == *" status=pass "* ]] || fail "pass fixture did not pass"
[[ "$report" == *" schema=5 "* ]] || fail "missing schema"
[[ "$report" == *" workload=xterm-cpu "* ]] || fail "missing workload"
[[ "$report" == *" duration_seconds=20 "* ]] || fail "missing duration"
[[ "$report" == *" surface_width=500 surface_height=500 "* ]] || fail "missing surface size"
[[ "$report" == *" lines_per_iteration=1 interval_msec=16 "* ]] ||
    fail "missing paced workload"
[[ "$report" == *" client_lines=1200 "* ]] || fail "missing client lines"
[[ "$report" == *" cpu_patch_updates=539 "* ]] || fail "missing patch updates"
[[ "$report" == *" cpu_payload_bytes=13271040 "* ]] || fail "missing payload bytes"
[[ "$report" == *" cpu_max_compose_msec=4 "* ]] || fail "missing compose max"
[[ "$report" == *" cpu_compose_budget_msec=25 "* ]] || fail "missing compose budget"
[[ "$report" == *" partial_repaints=2 full_repaints=1 "* ]] || fail "missing repaint counts"
[[ "$report" == *" present_fps=58.900 "* ]] || fail "missing present cadence"
[[ "$report" == *" post_startup_updates=540 post_startup_compositions=500 "* ]] ||
    fail "missing post-startup progress"
[[ "$report" == *" changed_primary_retirements=480 "* ]] ||
    fail "missing changed retirement count"
[[ "$report" == *" source_max_gap_usec=16403 source_gap_budget_usec=48000 "* ]] ||
    fail "missing source cadence budget"
[[ "$report" == *" retirement_deadline_usec=34334 "* ]] ||
    fail "missing two-refresh retirement deadline"
[[ "$report" == *" display_max_gap_usec=33097 display_gap_budget_usec=34334 "* ]] ||
    fail "missing display cadence budget"
[[ "$report" == *" last_retirement_to_completion_msec=84 "* ]] ||
    fail "missing final display-progress boundary"

MUTATED="$(mktemp)"
trap 'rm -f "$MUTATED"' EXIT

# Fails closed when the immutable patch-batch path was never exercised.
# The retained field shape from the failed physical run had healthy aggregate
# counts but no work after readiness. It must remain a permanent negative
# control for this gate.
if "$REPORTER" "$STARTUP_ONLY_FIXTURE" >/dev/null 2>&1; then
    fail "reporter accepted startup-only CPU activity"
fi

# The schema is mandatory and unique; accepting its absence or duplicates
# would allow unrelated aggregate counters to stand in for continuous progress.
grep -v '^sophia_live_cpu_visual_progress ' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    fail "reporter accepted missing CPU visual-progress evidence"
fi
sed '/^sophia_live_cpu_visual_progress /p' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    fail "reporter accepted duplicate CPU visual-progress evidence"
fi

sed 's/changed_primary_retirements=480/changed_primary_retirements=2/' \
    "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    fail "reporter accepted fewer than three changed retirements"
fi

sed 's/source_max_gap_usec=16403/source_max_gap_usec=48001/' \
    "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    fail "reporter accepted a source gap above its cadence budget"
fi

sed 's/display_max_gap_usec=33097/display_max_gap_usec=34335/' \
    "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    fail "reporter accepted a display gap above its two-refresh budget"
fi

sed 's/first_retirement_after_ready_msec=32/first_retirement_after_ready_msec=1001/' \
    "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    fail "reporter accepted a late first changed retirement"
fi

sed 's/last_retirement_after_ready_msec=19916/last_retirement_after_ready_msec=18999/' \
    "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    fail "reporter accepted display progress ending more than one second early"
fi

sed 's/pending_updates=0/pending_updates=1/' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    fail "reporter accepted an update pending after native drain"
fi

sed 's/max_update_to_retirement_usec=16000/max_update_to_retirement_usec=34335/' \
    "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    fail "reporter accepted retirement later than two refresh intervals"
fi

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

# Fails closed when the client did not run the benchmark's declared cadence.
sed 's/interval_msec=16 lines=1200/interval_msec=17 lines=1200/' \
    "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    fail "reporter accepted mismatched client pacing"
fi
sed 's/lines=1200/lines=1201/' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    fail "reporter accepted inconsistent client line totals"
fi

# Fails closed when CPU composition exceeds the established hardware budget.
sed 's/cpu_max_compose_msec=4/cpu_max_compose_msec=26/' "$FIXTURE" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    fail "reporter accepted CPU composition above 25ms"
fi

# The budget is configurable only as a positive integer for named follow-up
# gates; malformed or zero overrides must not silently disable it.
if SOPHIA_TERMINAL_COMPOSE_BUDGET_MSEC=0 \
    "$REPORTER" "$FIXTURE" >/dev/null 2>&1; then
    fail "reporter accepted a zero CPU composition budget"
fi
if SOPHIA_TERMINAL_COMPOSE_BUDGET_MSEC=invalid \
    "$REPORTER" "$FIXTURE" >/dev/null 2>&1; then
    fail "reporter accepted a malformed CPU composition budget"
fi

# Schema-2 rendering evidence, which a current session emits and which carries
# the copy-on-write figures. The schema-1 fixture above stays: archived runs
# carry it, and a reader that accepted only one of the two would either orphan
# them or stop reading live sessions.
SCHEMA2="$(mktemp)"
trap 'rm -f -- "$MUTATED" "$SCHEMA2"' EXIT
sed 's/^sophia_live_rendering_efficiency schema=1 status=complete \(.*\)$/sophia_live_rendering_efficiency schema=2 status=complete \1 cpu_cow_splits=12 cpu_resident_buffers_peak=2 cpu_resident_bytes_peak=1998848/' \
    "$FIXTURE" >"$SCHEMA2"
grep -q 'sophia_live_rendering_efficiency schema=2 ' "$SCHEMA2" ||
    fail "the schema-2 fixture rewrite did not apply"
schema2_report="$("$REPORTER" "$SCHEMA2")" ||
    fail "reporter rejected schema-2 rendering evidence"
grep -Fq 'cpu_cow_splits=12 cpu_resident_buffers_peak=2 cpu_resident_bytes_peak=1998848' \
    <<<"$schema2_report" ||
    fail "reporter did not carry the copy-on-write figures into its record"

# A registry that never held a buffer means the software path was not
# exercised, which is the one thing this workload exists to prove.
sed 's/cpu_resident_buffers_peak=2/cpu_resident_buffers_peak=0/' "$SCHEMA2" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    fail "reporter accepted a run whose CPU registry never held a buffer"
fi

# More copies than patches that could have caused them is an accounting
# contradiction, not a slow session.
sed 's/cpu_cow_splits=12/cpu_cow_splits=99999/' "$SCHEMA2" >"$MUTATED"
if "$REPORTER" "$MUTATED" >/dev/null 2>&1; then
    fail "reporter accepted more copies than patches"
fi

echo "terminal performance reporter regressions passed"
