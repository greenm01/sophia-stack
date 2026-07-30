#!/usr/bin/env bash
set -euo pipefail

# Offline regression for tools/probes/run_bounded_xterm.sh geometry resolution.
# Requires no TTY, GPU, X server, or xterm binary: it drives the probe's
# dry-run geometry mode and asserts the pixel->cell conversion never produces a
# window whose CPU buffer approaches X_AUTHORITY_SOFTWARE_BUFFER_MAX_BYTES.
#
# This guards the terminal-benchmark hard-lock regression: passing the pixel
# intent straight into xterm's character-cell -geometry once produced a
# 4004x5004 px window (~80 MB) that overran the 64 MiB software-buffer cap,
# was rejected BadWindow, and aborted the session. See docs/research-log.md
# 2026-07-30.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROBE="$ROOT_DIR/tools/probes/run_bounded_xterm.sh"

# Must stay strictly under X_AUTHORITY_SOFTWARE_BUFFER_MAX_BYTES (64 MiB).
CAP_BYTES=$((64 * 1024 * 1024))
FAKE_XTERM="$(mktemp)"
trap 'rm -f "$FAKE_XTERM"' EXIT
printf '%s\n' \
    '#!/usr/bin/env sh' \
    'while [ "$#" -gt 0 ]; do' \
    '    if [ "$1" = -e ]; then' \
    '        shift' \
    '        exec "$@"' \
    '    fi' \
    '    shift' \
    'done' \
    'exit 2' >"$FAKE_XTERM"
chmod 700 "$FAKE_XTERM"

fail() {
    echo "bounded xterm geometry regression failed: $*" >&2
    exit 1
}

field() {
    # field <line> <key> -> value
    local line="$1" key="$2" token
    for token in $line; do
        if [[ "$token" == "$key="* ]]; then
            printf '%s' "${token#*=}"
            return 0
        fi
    done
    return 1
}

resolve() {
    # resolve <width_px> <height_px> -> geometry line
    SOPHIA_XTERM_PRINT_GEOMETRY=1 \
    SOPHIA_XTERM_WIDTH="$1" \
    SOPHIA_XTERM_HEIGHT="$2" \
        "$PROBE"
}

# Sweep the default, the exact pathological pre-fix intent, and absurd overrides.
for pair in 500x500 1x1 100x100 2000x2000 5000x5000 100000x100000; do
    width="${pair%x*}"
    height="${pair#*x}"
    line="$(resolve "$width" "$height")"

    [[ "$line" == sophia_xterm_geometry\ * ]] ||
        fail "no geometry line for intent ${pair}: '$line'"

    cols="$(field "$line" cols)" || fail "missing cols for ${pair}"
    rows="$(field "$line" rows)" || fail "missing rows for ${pair}"
    bytes="$(field "$line" buffer_bytes)" || fail "missing buffer_bytes for ${pair}"
    lines="$(field "$line" lines_per_iteration)" ||
        fail "missing lines_per_iteration for ${pair}"
    interval="$(field "$line" interval_msec)" ||
        fail "missing interval_msec for ${pair}"

    ((cols >= 1)) || fail "cols<1 for ${pair}: $cols"
    ((rows >= 1)) || fail "rows<1 for ${pair}: $rows"
    ((bytes > 0)) || fail "buffer_bytes<=0 for ${pair}: $bytes"
    ((lines == 8)) || fail "unexpected default line batch for ${pair}: $lines"
    ((interval == 16)) || fail "unexpected default interval for ${pair}: $interval"
    ((bytes < CAP_BYTES)) ||
        fail "intent ${pair} resolves to ${bytes} bytes, at/over the ${CAP_BYTES} cap"
done

# The exact pre-fix crash input must resolve well under the cap (not to the old
# 4004x5004 / ~80 MB window).
crash_line="$(resolve 500 500)"
crash_bytes="$(field "$crash_line" buffer_bytes)" || fail "missing crash buffer_bytes"
((crash_bytes < 2 * 1024 * 1024)) ||
    fail "the 500x500 default resolves to ${crash_bytes} bytes; expected a small window"

# Invalid pacing cannot silently restore the unbounded producer.
if SOPHIA_XTERM_PRINT_GEOMETRY=1 SOPHIA_XTERM_INTERVAL_MSEC=0 \
    "$PROBE" >/dev/null 2>&1; then
    fail "the probe accepted a zero iteration interval"
fi
if SOPHIA_XTERM_PRINT_GEOMETRY=1 SOPHIA_XTERM_INTERVAL_MSEC=1001 \
    "$PROBE" >/dev/null 2>&1; then
    fail "the probe accepted an interval above 1000ms"
fi

# Exercise the real timed inner loop through an xterm-shaped test double. This
# proves that pacing reaches the client process and that reported line totals
# remain derived from the declared batch size.
paced_output="$(
    SOPHIA_XTERM_BIN="$FAKE_XTERM" \
    SOPHIA_XTERM_DURATION_SECONDS=1 \
    SOPHIA_XTERM_LINES=2 \
    SOPHIA_XTERM_INTERVAL_MSEC=100 \
        "$PROBE"
)"
paced_line="$(grep -E '^sophia_xterm_client schema=2 status=complete ' <<<"$paced_output")"
[[ -n "$paced_line" ]] || fail "paced probe emitted no client completion"
paced_batch="$(field "$paced_line" lines_per_iteration)" ||
    fail "paced probe completion lacks lines_per_iteration"
paced_interval="$(field "$paced_line" interval_msec)" ||
    fail "paced probe completion lacks interval_msec"
paced_lines="$(field "$paced_line" lines)" ||
    fail "paced probe completion lacks lines"
paced_iterations="$(field "$paced_line" iterations)" ||
    fail "paced probe completion lacks iterations"
((paced_batch == 2 && paced_interval == 100 && paced_iterations > 0)) ||
    fail "paced probe reported an unexpected workload: '$paced_line'"
((paced_lines == paced_batch * paced_iterations)) ||
    fail "paced probe line total is inconsistent: '$paced_line'"

echo "bounded xterm geometry regressions passed"
