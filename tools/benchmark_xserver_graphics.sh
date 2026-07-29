#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
LOG_DIR="${SOPHIA_XSERVER_BENCHMARK_LOG_DIR:-$STATE_HOME/sophia/rendering-reference}"
VKCUBE_LOG="$LOG_DIR/xserver-vkcube.log"
GLXGEARS_LOG="$LOG_DIR/xserver-glxgears.log"
FRAME_COUNT="${SOPHIA_RENDER_FRAME_COUNT:-900}"
SURFACE_WIDTH="${SOPHIA_RENDER_SURFACE_WIDTH:-500}"
SURFACE_HEIGHT="${SOPHIA_RENDER_SURFACE_HEIGHT:-500}"
VULKAN_PRESENT_MODE="${SOPHIA_RENDER_VULKAN_PRESENT_MODE:-2}"
TIMEOUT_SECONDS="${SOPHIA_XSERVER_BENCHMARK_TIMEOUT_SECONDS:-45}"
GLXGEARS_MODE="${SOPHIA_XSERVER_GLXGEARS:-auto}"
GLXGEARS_SECONDS="${SOPHIA_XSERVER_GLXGEARS_SECONDS:-12}"

fail() {
    echo "Xserver graphics benchmark failed: $*" >&2
    exit 1
}

[[ -n "${DISPLAY:-}" ]] || fail "DISPLAY is unset; run this inside the Xorg/XLibre session"
[[ "${XDG_SESSION_TYPE:-}" != wayland && -z "${WAYLAND_DISPLAY:-}" ]] ||
    fail "Xwayland is not a valid Xserver baseline; use a direct Xorg/XLibre session"
[[ "$FRAME_COUNT" =~ ^[1-9][0-9]*$ ]] || fail "frame count must be positive"
[[ "$SURFACE_WIDTH" =~ ^[1-9][0-9]*$ ]] || fail "surface width must be positive"
[[ "$SURFACE_HEIGHT" =~ ^[1-9][0-9]*$ ]] || fail "surface height must be positive"
[[ "$VULKAN_PRESENT_MODE" == 2 ]] ||
    fail "the retained parity workload requires Vulkan FIFO present mode 2"
[[ "$TIMEOUT_SECONDS" =~ ^[1-9][0-9]*$ ]] || fail "timeout must be positive"
[[ "$GLXGEARS_SECONDS" =~ ^[1-9][0-9]*$ ]] ||
    fail "glxgears duration must be positive"
case "$GLXGEARS_MODE" in
    auto|true|false) ;;
    *) fail "SOPHIA_XSERVER_GLXGEARS must be auto, true, or false" ;;
esac

for command_name in cc pkg-config sha256sum vkcube; do
    command -v "$command_name" >/dev/null ||
        fail "required command is missing: $command_name"
done
pkg-config --exists xcb xcb-present xcb-randr ||
    fail "XCB Present/RandR development files are required for the cadence observer"

mkdir -p "$LOG_DIR"
chmod 700 "$LOG_DIR"
[[ ! -f "$VKCUBE_LOG" ]] || mv -f "$VKCUBE_LOG" "$VKCUBE_LOG.previous"
[[ ! -f "$GLXGEARS_LOG" ]] || mv -f "$GLXGEARS_LOG" "$GLXGEARS_LOG.previous"
PROBE_BIN="$(mktemp)"
trap 'rm -f "$PROBE_BIN"' EXIT
xcb_flags=()
read -r -a xcb_flags <<<"$(pkg-config --cflags --libs xcb xcb-present xcb-randr)"

cc -std=c11 -D_POSIX_C_SOURCE=200809L -Wall -Wextra -Werror \
    "$ROOT_DIR/tools/probes/xserver_present_probe.c" \
    -o "$PROBE_BIN" \
    "${xcb_flags[@]}"

printf '%s\n' \
    "Starting the Xserver vkcube reference (${FRAME_COUNT} frames, ${SURFACE_WIDTH}x${SURFACE_HEIGHT}, FIFO)." \
    'Keep this terminal and workspace otherwise idle until the cube exits.'

set +e
{
    printf 'xserver_rendering_benchmark schema=1 workload=vkcube-xcb requested_frames=%s surface_width=%s surface_height=%s vulkan_present_mode=%s\n' \
        "$FRAME_COUNT" "$SURFACE_WIDTH" "$SURFACE_HEIGHT" "$VULKAN_PRESENT_MODE"
    "$PROBE_BIN" "$SURFACE_WIDTH" "$SURFACE_HEIGHT" "$TIMEOUT_SECONDS" -- \
        vkcube \
        --wsi xcb \
        --c "$FRAME_COUNT" \
        --width "$SURFACE_WIDTH" \
        --height "$SURFACE_HEIGHT" \
        --present_mode "$VULKAN_PRESENT_MODE"
} 2>&1 | tee "$VKCUBE_LOG"
probe_status="${PIPESTATUS[0]}"
set -e
((probe_status == 0)) || fail "Present probe or vkcube failed; see $VKCUBE_LOG"

"$ROOT_DIR/tools/report_xserver_rendering_performance.sh" "$VKCUBE_LOG"

glxgears_bin="$(command -v glxgears || true)"
if [[ "$GLXGEARS_MODE" == false ||
    ( "$GLXGEARS_MODE" == auto && -z "$glxgears_bin" ) ]]; then
    reason=disabled
    [[ "$GLXGEARS_MODE" == auto ]] && reason=missing_binary
    printf 'xserver_glxgears_performance schema=1 status=skipped reason=%s\n' "$reason" |
        tee "$GLXGEARS_LOG"
elif [[ -z "$glxgears_bin" ]]; then
    fail "glxgears was required but is not installed"
else
    command -v timeout >/dev/null || fail "timeout is required for the GLX probe"
    command -v stdbuf >/dev/null || fail "stdbuf is required for the GLX probe"
    set +e
    timeout --signal=TERM "$GLXGEARS_SECONDS" \
        stdbuf -oL -eL "$glxgears_bin" 2>&1 | tee "$GLXGEARS_LOG"
    glxgears_status="${PIPESTATUS[0]}"
    set -e
    if ((glxgears_status != 0 && glxgears_status != 124 && glxgears_status != 143)); then
        fail "glxgears failed with status $glxgears_status; see $GLXGEARS_LOG"
    fi
    glxgears_samples="$(
        awk '/ frames in [0-9.]+ seconds = [0-9.]+ FPS/ { count++ } END { print count + 0 }' \
            "$GLXGEARS_LOG"
    )"
    ((glxgears_samples > 0)) ||
        fail "glxgears produced no cadence samples; see $GLXGEARS_LOG"
    glxgears_fps="$(
        awk '
            / frames in [0-9.]+ seconds = [0-9.]+ FPS/ {
                total += $(NF - 1)
                count++
            }
            END { if (count > 0) printf "%.3f", total / count }
        ' "$GLXGEARS_LOG"
    )"
    printf 'xserver_glxgears_performance schema=1 status=pass samples=%s mean_fps=%s duration_seconds=%s role=compatibility_probe\n' \
        "$glxgears_samples" "$glxgears_fps" "$GLXGEARS_SECONDS" |
        tee -a "$GLXGEARS_LOG"
fi

sophia_log="${SOPHIA_STANDALONE_LOG_DIR:-$STATE_HOME/sophia/standalone-session}/session.log"
if [[ -s "$sophia_log" ]]; then
    "$ROOT_DIR/tools/compare_sophia_xserver_rendering.sh" \
        "$sophia_log" "$VKCUBE_LOG"
else
    printf '%s\n' \
        "No Sophia benchmark was found at $sophia_log." \
        'Run tools/benchmark_sophia_vkcube_tty3.sh, then rerun the comparison.'
fi
