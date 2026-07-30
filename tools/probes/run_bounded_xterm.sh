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
LINES_PER_ITERATION="${SOPHIA_XTERM_LINES:-8}"
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
command -v date >/dev/null || fail "date is unavailable"
command -v seq >/dev/null || fail "seq is unavailable"
command -v sleep >/dev/null || fail "sleep is unavailable"

COUNT_FILE="$(mktemp)"
INNER_SCRIPT="$(mktemp)"
trap 'rm -f "$COUNT_FILE" "$INNER_SCRIPT"' EXIT

# The inner workload self-bounds to $1 seconds and records its scrollback
# iteration count to $3 so the outer probe can report deterministic client
# throughput even though xterm prints no cadence line of its own. Small,
# regularly paced batches avoid turning one shell write into more ordered
# visual facts than the bounded authority channel can preserve at once.
cat >"$INNER_SCRIPT" <<'INNER'
duration_seconds="$1"
lines_per_iteration="$2"
count_file="$3"
interval_seconds="$4"
iterations=0
end_epoch=$(( $(date +%s) + duration_seconds ))
while [ "$(date +%s)" -lt "$end_epoch" ]; do
    seq 1 "$lines_per_iteration"
    iterations=$(( iterations + 1 ))
    sleep "$interval_seconds"
done
printf '%s\n' "$iterations" >"$count_file"
INNER

# Self-bounded by the inner loop; timeout is only a safety net so a hung xterm
# still returns and restores the session.
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
