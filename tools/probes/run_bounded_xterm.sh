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
LINES_PER_ITERATION="${SOPHIA_XTERM_LINES:-200}"
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
[[ -n "$XTERM_BIN" && -x "$XTERM_BIN" ]] ||
    fail "xterm is not installed"
command -v timeout >/dev/null || fail "timeout is unavailable"
command -v date >/dev/null || fail "date is unavailable"
command -v seq >/dev/null || fail "seq is unavailable"

COUNT_FILE="$(mktemp)"
INNER_SCRIPT="$(mktemp)"
trap 'rm -f "$COUNT_FILE" "$INNER_SCRIPT"' EXIT

# The inner workload self-bounds to $1 seconds and records its scrollback
# iteration count to $3 so the outer probe can report deterministic client
# throughput even though xterm prints no cadence line of its own.
cat >"$INNER_SCRIPT" <<'INNER'
duration_seconds="$1"
lines_per_iteration="$2"
count_file="$3"
iterations=0
end_epoch=$(( $(date +%s) + duration_seconds ))
while [ "$(date +%s)" -lt "$end_epoch" ]; do
    seq 1 "$lines_per_iteration"
    iterations=$(( iterations + 1 ))
done
printf '%s\n' "$iterations" >"$count_file"
INNER

# Self-bounded by the inner loop; timeout is only a safety net so a hung xterm
# still returns and restores the session.
safety_deadline=$(( DURATION_SECONDS + 5 ))

set +e
timeout --signal=TERM "$safety_deadline" \
    "$XTERM_BIN" \
    -geometry "${SURFACE_WIDTH}x${SURFACE_HEIGHT}" \
    -e sh "$INNER_SCRIPT" \
    "$DURATION_SECONDS" "$LINES_PER_ITERATION" "$COUNT_FILE"
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

printf 'sophia_xterm_client schema=1 status=complete duration_seconds=%s lines=%s iterations=%s timed_exit=%s\n' \
    "$DURATION_SECONDS" "$lines" "$iterations" "$timed_exit"
