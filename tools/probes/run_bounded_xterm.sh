#!/usr/bin/env bash
set -euo pipefail

# Bounded CPU-terminal scrollback probe. Drives sustained core-X drawing /
# SHM present traffic through an unmodified xterm for a fixed window so the
# Sophia software-Present (CPU) composition path is exercised, then reports one
# self-describing client-cadence line for report_sophia_terminal_performance.sh.
#
# Unlike the glxgears/vkcube probes (GPU DRI3 flip path), this exercises the
# XSoftwareBufferStore patch-batch path and the CPU composition metrics.

DURATION_SECONDS="${SOPHIA_XTERM_DURATION_SECONDS:-20}"
SURFACE_WIDTH="${SOPHIA_XTERM_WIDTH:-500}"
SURFACE_HEIGHT="${SOPHIA_XTERM_HEIGHT:-500}"
LINES_PER_ITERATION="${SOPHIA_XTERM_LINES:-1}"
INTERVAL_MSEC="${SOPHIA_XTERM_INTERVAL_MSEC:-16}"
XTERM_BIN="${SOPHIA_XTERM_BIN:-$(command -v xterm || true)}"

fail() {
    echo "Sophia bounded xterm client failed: $*" >&2
    exit 1
}

[[ "$DURATION_SECONDS" =~ ^[1-9][0-9]*$ ]] ||
    fail "duration must be a positive integer"
[[ "$SURFACE_WIDTH" =~ ^[1-9][0-9]*$ ]] ||
    fail "surface width must be a positive integer"
[[ "$SURFACE_HEIGHT" =~ ^[1-9][0-9]*$ ]] ||
    fail "surface height must be a positive integer"
[[ "$LINES_PER_ITERATION" =~ ^[1-9][0-9]*$ ]] ||
    fail "lines per iteration must be a positive integer"
[[ "$INTERVAL_MSEC" =~ ^[1-9][0-9]*$ ]] && ((INTERVAL_MSEC <= 1000)) ||
    fail "iteration interval must be an integer from 1 through 1000 milliseconds"
printf -v INTERVAL_SECONDS '%d.%03d' \
    "$((INTERVAL_MSEC / 1000))" "$((INTERVAL_MSEC % 1000))"

# SOPHIA_XTERM_WIDTH/HEIGHT are the intended *pixel* surface size (the session
# reports them verbatim on the sophia_terminal_benchmark line). xterm's
# -geometry, however, is expressed in character *cells*, not pixels: passing
# "500x500" directly requests a 500-column by 500-row terminal, which with any
# font is a multi-thousand-pixel window. On the CPU software-Present path the
# X authority backs each window with one immutable buffer bounded by
# X_AUTHORITY_SOFTWARE_BUFFER_MAX_BYTES (64 MiB); a ~4004x5004 window overruns
# that bound, so ImageText8 is rejected with BadWindow and xterm aborts. Pin a
# fixed-metric core font and a fixed internal border so the pixel->cell
# conversion is deterministic, and clamp the pixel intent well under the cap.
XTERM_CELL_WIDTH=6      # matches the pinned "6x13" core bitmap font
XTERM_CELL_HEIGHT=13
XTERM_INTERNAL_BORDER=2 # pinned via -b so the frame is (cells*cell + 2*border)
XTERM_MAX_PIXELS=2048   # 2048x2048x4 = 16 MiB, safely under the 64 MiB cap
clamp_pixels() {
    local value="$1"
    ((value < XTERM_CELL_HEIGHT)) && value=$XTERM_CELL_HEIGHT
    ((value > XTERM_MAX_PIXELS)) && value=$XTERM_MAX_PIXELS
    printf '%s' "$value"
}
SURFACE_WIDTH="$(clamp_pixels "$SURFACE_WIDTH")"
SURFACE_HEIGHT="$(clamp_pixels "$SURFACE_HEIGHT")"
XTERM_COLS=$(((SURFACE_WIDTH - 2 * XTERM_INTERNAL_BORDER) / XTERM_CELL_WIDTH))
XTERM_ROWS=$(((SURFACE_HEIGHT - 2 * XTERM_INTERNAL_BORDER) / XTERM_CELL_HEIGHT))
((XTERM_COLS < 1)) && XTERM_COLS=1
((XTERM_ROWS < 1)) && XTERM_ROWS=1

# Dry-run: print the resolved geometry and CPU-buffer estimate, then exit before
# touching xterm. Lets tools/check_bounded_xterm_geometry.sh assert the
# pixel->cell conversion stays under the software-buffer cap with no X server,
# GPU, or xterm binary present.
if [[ -n "${SOPHIA_XTERM_PRINT_GEOMETRY:-}" ]]; then
    geometry_pixel_width=$((XTERM_COLS * XTERM_CELL_WIDTH + 2 * XTERM_INTERNAL_BORDER))
    geometry_pixel_height=$((XTERM_ROWS * XTERM_CELL_HEIGHT + 2 * XTERM_INTERNAL_BORDER))
    printf 'sophia_xterm_geometry cols=%s rows=%s pixel_width=%s pixel_height=%s buffer_bytes=%s lines_per_iteration=%s interval_msec=%s\n' \
        "$XTERM_COLS" "$XTERM_ROWS" \
        "$geometry_pixel_width" "$geometry_pixel_height" \
        "$((geometry_pixel_width * geometry_pixel_height * 4))" \
        "$LINES_PER_ITERATION" "$INTERVAL_MSEC"
    exit 0
fi

[[ -n "$XTERM_BIN" && -x "$XTERM_BIN" ]] ||
    fail "xterm is not installed"
command -v timeout >/dev/null || fail "timeout is unavailable"
command -v sleep >/dev/null || fail "sleep is unavailable"

COUNT_FILE="$(mktemp)"
INNER_SCRIPT="$(mktemp)"
trap 'rm -f "$COUNT_FILE" "$INNER_SCRIPT"' EXIT

# The inner workload uses its own process-external timer and records every
# completed scrollback burst to $3. A wall-clock test inside the producer is
# insufficient: when xterm applies backpressure, the shell can remain blocked
# while writing past that test and never finalize the count file. The nested
# timeout interrupts that producer; the xterm-owned shell then exits normally
# and leaves deterministic completed-burst evidence. Small, regularly
# paced batches avoid turning one shell write into more ordered visual facts
# than the bounded authority channel can preserve at once.
cat >"$INNER_SCRIPT" <<'INNER'
duration_seconds="$1"
lines_per_iteration="$2"
count_file="$3"
interval_seconds="$4"
: >"$count_file"
set +e
workload_pid=
parent_watchdog_pid=
cleanup_children() {
    if [ -n "$parent_watchdog_pid" ]; then
        kill "$parent_watchdog_pid" 2>/dev/null || true
        wait "$parent_watchdog_pid" 2>/dev/null || true
        parent_watchdog_pid=
    fi
    if [ -n "$workload_pid" ]; then
        # GNU timeout forwards TERM to the producer it owns. This matters when
        # the X server disappears: xterm can leave its command child outside
        # the session application's process group, and that orphan otherwise
        # retains the gate's log pipe until duration_seconds expires.
        kill -TERM "$workload_pid" 2>/dev/null || true
        wait "$workload_pid" 2>/dev/null || true
        workload_pid=
    fi
}
trap 'trap - HUP INT TERM; cleanup_children; exit 143' HUP INT TERM
trap cleanup_children EXIT

timeout --signal=TERM --kill-after=1 "$duration_seconds" sh -c '
    lines_per_iteration="$1"
    count_file="$2"
    interval_seconds="$3"
    iterations=0
    # This full-period 16-bit LCG makes each line look unrelated to the last
    # while keeping physical evidence reproducible. Its bounded arithmetic is
    # safe in every POSIX shell, and ten values fill most of the default xterm
    # row without adding an external random-number generator.
    visual_state=19753
    while :; do
        line=0
        while [ "$line" -lt "$lines_per_iteration" ]; do
            set --
            visual_field=0
            while [ "$visual_field" -lt 10 ]; do
                visual_state=$(( (visual_state * 25173 + 13849) % 65536 ))
                set -- "$@" "$visual_state"
                visual_field=$(( visual_field + 1 ))
            done
            printf "%05d %05d %05d %05d %05d %05d %05d %05d %05d %05d\n" \
                "$@"
            line=$(( line + 1 ))
        done
        iterations=$(( iterations + 1 ))
        printf "%s\n" "$iterations" >"$count_file"
        sleep "$interval_seconds"
    done
' sh "$lines_per_iteration" "$count_file" "$interval_seconds" &
workload_pid="$!"

# xterm normally owns this shell until the bounded workload exits. If xterm or
# its X server dies first, detect the vanished parent and stop the independently
# timed producer immediately instead of holding an inherited log descriptor for
# the remainder of the workload window.
xterm_parent_pid="$PPID"
(
    while kill -0 "$xterm_parent_pid" 2>/dev/null; do
        sleep 0.05
    done
    kill -TERM "$workload_pid" 2>/dev/null || true
) </dev/null >/dev/null 2>&1 &
parent_watchdog_pid="$!"

wait "$workload_pid"
workload_status="$?"
workload_pid=
kill "$parent_watchdog_pid" 2>/dev/null || true
wait "$parent_watchdog_pid" 2>/dev/null || true
parent_watchdog_pid=
trap - EXIT HUP INT TERM
set -e
case "$workload_status" in
    124|137|143) ;;
    *)
        echo "bounded xterm producer exited unexpectedly: $workload_status" >&2
        exit 1
        ;;
esac
test -s "$count_file" || {
    echo "bounded xterm producer completed no scrollback bursts" >&2
    exit 1
}
INNER

# Self-bounded by the inner producer timer; this timeout is the process-level
# safety net for xterm startup/teardown. xterm can linger after its child has
# completed while the X server drains its final traffic, so a timeout exit is
# accepted only when the incremental count file proves that the independently
# timed workload completed positive scrollback traffic.
safety_deadline=$(( DURATION_SECONDS + 5 ))

set +e
timeout --signal=TERM "$safety_deadline" \
    "$XTERM_BIN" \
    -fn "${XTERM_CELL_WIDTH}x${XTERM_CELL_HEIGHT}" \
    -b "$XTERM_INTERNAL_BORDER" \
    -geometry "${XTERM_COLS}x${XTERM_ROWS}" \
    -e sh "$INNER_SCRIPT" \
    "$DURATION_SECONDS" "$LINES_PER_ITERATION" "$COUNT_FILE" "$INTERVAL_SECONDS"
client_status="${PIPESTATUS[0]}"
set -e

if ((client_status != 0 && client_status != 124 && client_status != 143)); then
    fail "xterm exited with status $client_status"
fi

timed_exit=false
iterations=0
if [[ -s "$COUNT_FILE" ]]; then
    iterations="$(tr -d '[:space:]' <"$COUNT_FILE")"
    [[ "$iterations" =~ ^[0-9]+$ ]] || fail "xterm workload wrote a malformed iteration count"
    ((iterations > 0)) || fail "xterm workload produced no scrollback iterations"
    timed_exit=true
else
    fail "xterm workload did not complete its bounded window"
fi

lines=$(( iterations * LINES_PER_ITERATION ))

printf 'sophia_xterm_client schema=2 status=complete duration_seconds=%s lines_per_iteration=%s interval_msec=%s lines=%s iterations=%s timed_exit=%s\n' \
    "$DURATION_SECONDS" "$LINES_PER_ITERATION" "$INTERVAL_MSEC" \
    "$lines" "$iterations" "$timed_exit"
