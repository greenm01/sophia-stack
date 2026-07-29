#!/usr/bin/env bash
set -euo pipefail

DURATION_SECONDS="${SOPHIA_GLXGEARS_DURATION_SECONDS:-20}"
SURFACE_WIDTH="${SOPHIA_GLXGEARS_WIDTH:-500}"
SURFACE_HEIGHT="${SOPHIA_GLXGEARS_HEIGHT:-500}"
GLXGEARS_BIN="${SOPHIA_GLXGEARS_BIN:-$(command -v glxgears || true)}"

fail() {
    echo "Sophia bounded glxgears client failed: $*" >&2
    exit 1
}

[[ "$DURATION_SECONDS" =~ ^[1-9][0-9]*$ ]] ||
    fail "duration must be a positive integer"
[[ "$SURFACE_WIDTH" =~ ^[1-9][0-9]*$ ]] ||
    fail "surface width must be a positive integer"
[[ "$SURFACE_HEIGHT" =~ ^[1-9][0-9]*$ ]] ||
    fail "surface height must be a positive integer"
[[ -n "$GLXGEARS_BIN" && -x "$GLXGEARS_BIN" ]] ||
    fail "glxgears is not installed"
command -v timeout >/dev/null || fail "timeout is unavailable"
command -v stdbuf >/dev/null || fail "stdbuf is unavailable"

CLIENT_LOG="$(mktemp)"
trap 'rm -f "$CLIENT_LOG"' EXIT

set +e
timeout --signal=TERM "$DURATION_SECONDS" \
    stdbuf -oL -eL "$GLXGEARS_BIN" \
    -info \
    -swapinterval 1 \
    -geometry "${SURFACE_WIDTH}x${SURFACE_HEIGHT}" \
    2>&1 | tee "$CLIENT_LOG"
client_status="${PIPESTATUS[0]}"
set -e

if ((client_status != 0 && client_status != 124 && client_status != 143)); then
    fail "glxgears exited with status $client_status"
fi

renderer="$(
    grep -E '^GL_RENDERER[[:space:]]*=' "$CLIENT_LOG" |
        head -n 1 || true
)"
[[ -n "$renderer" ]] || fail "glxgears did not report its OpenGL renderer"
samples="$(
    awk '/ frames in [0-9.]+ seconds = [0-9.]+ FPS/ { count++ } END { print count + 0 }' \
        "$CLIENT_LOG"
)"
((samples > 0)) || fail "glxgears produced no client cadence samples"
mean_fps="$(
    awk '
        / frames in [0-9.]+ seconds = [0-9.]+ FPS/ {
            total += $(NF - 1)
            count++
        }
        END { if (count > 0) printf "%.3f", total / count }
    ' "$CLIENT_LOG"
)"

printf 'sophia_glxgears_client schema=1 status=complete duration_seconds=%s samples=%s mean_fps=%s timed_exit=true\n' \
    "$DURATION_SECONDS" "$samples" "$mean_fps"
