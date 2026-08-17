#!/usr/bin/env bash
set -euo pipefail

# One-shot: prove native output mirroring on real hardware.
#
# Runs a bounded live session with a profile that groups DP-1 and DP-2 into one
# logical output. Each head keeps its own mode -- DP-1 at 2560x1440 and DP-2 at
# 1920x1080 -- and the group's composed scene is placed onto each. Neither monitor
# is downgraded, which is the point of the projection architecture.
#
# It drives the live session rather than `native-topology-apply`, and that matters:
# the standalone apply command composes nothing and can only reuse the framebuffer
# each CRTC already scans out. This machine's console puts BOTH CRTCs on one
# 2560x1440 buffer, so DP-2's frame is 1440p while its mode is 1080p, and the apply
# admission refuses that forever. Establishing a group needs a buffer per head at
# that head's mode, which only something that composes can produce.
#
# This performs a REAL MODESET. Run it from /dev/tty4 with no compositor holding
# DRM master. Your screens will change for the duration.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${SOPHIA_MIRROR_PROFILE:-$ROOT_DIR/tools/fixtures/mirror_group_probe.kdl}"
RUNTIME_MSEC="${SOPHIA_MIRROR_RUNTIME_MSEC:-15000}"
DISPLAY_NAME="${SOPHIA_MIRROR_DISPLAY:-:191}"
EVIDENCE="${SOPHIA_MIRROR_EVIDENCE:-/tmp/sophia-mirror-group.log}"
TTY_REQUIRED="${SOPHIA_MIRROR_TTY:-/dev/tty4}"
KERNEL_MAX_LINES="${SOPHIA_MIRROR_KERNEL_MAX_LINES:-256}"
XTERM_BIN="${SOPHIA_MIRROR_XTERM:-}"

if [[ -z "$XTERM_BIN" ]]; then
    XTERM_BIN="$(command -v xterm || true)"
fi
if [[ "$XTERM_BIN" != /* || ! -x "$XTERM_BIN" ]]; then
    echo "Set SOPHIA_MIRROR_XTERM to an absolute executable xterm path." >&2
    exit 2
fi

echo "=== Sophia mirror-group proof ==="
echo

if [[ ! -r "$PROFILE" ]]; then
    echo "Profile not readable: $PROFILE" >&2
    exit 2
fi

# The profile carries display configuration, so the loader refuses one that is
# group- or world-readable. Fix it here rather than failing three steps later.
mode="$(stat -c '%a' "$PROFILE")"
if [[ "$mode" != "600" ]]; then
    echo "Tightening profile permissions ($mode -> 600)"
    chmod 600 "$PROFILE"
fi

echo "Profile: $PROFILE"
sed 's/^/  | /' "$PROFILE"
echo

echo "Connected connectors, straight from sysfs:"
connected_connectors=()
for status in /sys/class/drm/card*-*/status; do
    [[ "$(cat "$status" 2>/dev/null)" == "connected" ]] || continue
    name="${status%/status}"
    connected_connectors+=("${name##*/}")
    printf '  %-20s first_mode=%s\n' "${name##*/}" \
        "$(head -1 "${name}/modes" 2>/dev/null || echo none)"
done
echo

# This gate is two-head by construction: the profile mirrors one pair and its
# own pass line asserts connectors=2 heads=2. A third connected head becomes a
# logical output with nothing placed on it, so it exports no pixels and fails
# the independent-present check with an error that describes the symptom rather
# than the cause. Say the cause here instead.
if (( ${#connected_connectors[@]} != 2 )); then
    echo "The mirror-group proof needs exactly two connected heads; observed ${#connected_connectors[@]}." >&2
    printf '  %s\n' "${connected_connectors[@]}" >&2
    echo "Disconnect the extras, or run tools/run_mixed_output_gate_tty4.sh for a three-head rig." >&2
    exit 2
fi

if [[ "$(tty)" != "$TTY_REQUIRED" ]]; then
    echo "You are on $(tty); this needs $TTY_REQUIRED and DRM master." >&2
    echo "Switch with Ctrl+Alt+F4, log in, and run this again from there." >&2
    exit 2
fi

if [[ -n "$(git -C "$ROOT_DIR" status --porcelain --untracked-files=all)" ]]; then
    echo "The Sophia worktree must be clean so the binary has one exact source identity." >&2
    exit 2
fi
source_commit="$(git -C "$ROOT_DIR" rev-parse HEAD)"
if ! git -C "$ROOT_DIR" verify-commit "$source_commit" >/dev/null 2>&1; then
    echo "Sophia HEAD must have a valid cryptographic signature before a physical gate." >&2
    exit 2
fi

echo "Building..."
(cd "$ROOT_DIR" && cargo build --quiet --release --offline -p sophia-cli \
    --features "atomic-scanout-live" --bin sophia)
echo

if [[ -n "$(git -C "$ROOT_DIR" status --porcelain --untracked-files=all)" ]] \
    || [[ "$(git -C "$ROOT_DIR" rev-parse HEAD)" != "$source_commit" ]]; then
    echo "Sophia source identity changed during the physical-gate build." >&2
    exit 2
fi
if ! git -C "$ROOT_DIR" verify-commit "$source_commit" >/dev/null 2>&1; then
    echo "Sophia HEAD signature no longer verifies after the physical-gate build." >&2
    exit 2
fi

sophia_sha256="$(sha256sum "$ROOT_DIR/target/release/sophia" | awk '{ print $1 }')"
profile_sha256="$(sha256sum "$PROFILE" | awk '{ print $1 }')"
: >"$EVIDENCE"
printf 'sophia_mirror_group_gate schema=1 status=starting source_commit=%s sophia_sha256=%s profile_sha256=%s\n' \
    "$source_commit" "$sophia_sha256" "$profile_sha256" | tee -a "$EVIDENCE"

diagnostic_tmp="$(mktemp -d)"
kernel_before="$diagnostic_tmp/kernel-before.log"
kernel_after="$diagnostic_tmp/kernel-after.log"
kernel_delta="$diagnostic_tmp/kernel-delta.log"
trap 'rm -rf -- "$diagnostic_tmp"' EXIT

capture_kernel_snapshot() {
    local destination="$1"
    if dmesg --ctime >"$destination" 2>/dev/null; then
        return 0
    fi
    if command -v sudo >/dev/null 2>&1 && sudo -n dmesg --ctime >"$destination" 2>/dev/null; then
        return 0
    fi
    : >"$destination"
    return 1
}

kernel_before_available=false
if capture_kernel_snapshot "$kernel_before"; then
    kernel_before_available=true
fi

finish_failed_run() {
    local failure_stage="$1" failure_exit="$2"
    local failure_signal=0 kernel_capture=unavailable kernel_summary
    if (( failure_exit >= 128 && failure_exit <= 255 )); then
        failure_signal=$((failure_exit - 128))
    fi
    : >"$kernel_delta"
    if [[ "$kernel_before_available" == true ]] && capture_kernel_snapshot "$kernel_after"; then
        if kernel_summary="$("$ROOT_DIR/tools/collect_mirror_group_kernel_delta.sh" \
            "$kernel_before" "$kernel_after" "$kernel_delta" "$KERNEL_MAX_LINES")"; then
            kernel_capture=available
        fi
    fi
    if [[ "$kernel_capture" == unavailable ]]; then
        kernel_summary='availability=unavailable continuity=unknown lines=0 total_lines=0 truncated=false'
        : >"$kernel_delta"
    fi
    printf 'sophia_mirror_group_kernel schema=1 status=captured %s\n' "$kernel_summary" | tee -a "$EVIDENCE"
    printf 'sophia_mirror_group_gate schema=1 status=failed stage=%s exit=%s signal=%s kernel_capture=%s\n' \
        "$failure_stage" "$failure_exit" "$failure_signal" "$kernel_capture" | tee -a "$EVIDENCE"
    if ! "$ROOT_DIR/tools/archive_mirror_group_diagnostic_run.sh" "$EVIDENCE" "$kernel_delta"; then
        echo "Failed to archive mirror-group diagnostic; raw evidence remains at $EVIDENCE." >&2
    fi
}

echo "What success looks like, and only you can judge it:"
echo "  - BOTH monitors showing the same thing for ~$((RUNTIME_MSEC / 1000))s"
echo "  - DP-1 still 2560x1440 and DP-2 still 1920x1080, neither downgraded"
echo "  - DP-2 may show black bars if the aspects differ; that is 'fit' working"
echo "  - xterm will scroll 40 numbered mixed-case rows, then print the exact marker"
echo "    'Sophia Mirror AaZz 0123456789' and leave an interactive shell"
echo "  - Wait for the scene to settle, then verify identical logical content and"
echo "    letter case, stable legibility, and white-on-black presentation on both"
echo "  - DP-2's scaled glyph edges should look sharp and smooth, not blocky or stepped"
echo
echo "Evidence: $EVIDENCE"
echo "Running now."
echo

set +e
(
    cd "$ROOT_DIR"
    # Native scanout is armed separately from the session itself, because a
    # session that drives real KMS is a different act from one that does not.
    # No session bus exists on this rig, and Kitty's portal lookups behave
    # differently every run without one: one client fast-failed with
    # ServiceUnknown while another blocked ~30s on a Notify call that never got
    # a reply, which is libdbus's default timeout. A client stalled that long
    # never answers a configure, so every layout transaction times out and the
    # gate fails somewhere new each run. Point the bus at an address that
    # refuses immediately so client startup is deterministic; the gate proves
    # output topology, not portals.
    DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent/sophia-gate-no-session-bus \
    SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 \
        SOPHIA_NATIVE_COMPOSITION_PIXEL_TRACE=final-regions \
        ./target/release/sophia sophia-live-session \
        --display="$DISPLAY_NAME" \
        --native-scanout \
        --desktop-profile="$PROFILE" \
        --session-mode=normal \
        --session-app=terminal="$XTERM_BIN" \
        --session-start=terminal \
        --session-app-arg=terminal=-cm \
        --session-app-arg=terminal=-dc \
        --session-app-arg=terminal=-fn \
        --session-app-arg=terminal=6x13 \
        --session-app-arg=terminal=-fg \
        --session-app-arg=terminal=#ffffff \
        --session-app-arg=terminal=-bg \
        --session-app-arg=terminal=#000000 \
        --session-app-arg=terminal=-cr \
        --session-app-arg=terminal=#ffffff \
        --session-app-arg=terminal=-xrm \
        --session-app-arg=terminal='*cursorBlink:false' \
        --session-app-arg=terminal=-e \
        --session-app-arg=terminal=sh \
        --session-app-arg=terminal=-c \
        --session-app-arg=terminal='i=1; while [ "$i" -le 40 ]; do printf "SophiaScroll%02d AaZz\n" "$i"; i=$((i+1)); done; printf "Sophia Mirror AaZz 0123456789\n"; exec sh -i' \
        --max-runtime-ms="$RUNTIME_MSEC"
) 2>&1 | tee -a "$EVIDENCE"
status="${PIPESTATUS[0]}"
set -e

echo
echo "=== exit=$status ==="
if (( status != 0 )); then
    finish_failed_run runtime "$status"
    exit "$status"
fi
grep -E "sophia_live_native_page_flip|sophia_live_output|mirror|head" "$EVIDENCE" \
    | tail -20 || true
echo
echo "Did both monitors show the same scene at their native modes, including stable,"
echo "legible white-on-black text with identical logical content and letter case,"
echo "and did DP-2's scaled text look sharp rather than blocky or stair-stepped?"
echo "Type yes to record visible-pixel acceptance."
visual_confirmation=
if ! read -r visual_confirmation </dev/tty || [[ "$visual_confirmation" != "yes" ]]; then
    finish_failed_run visual_confirmation 1
    echo "Visible mirroring was not confirmed; evidence remains at $EVIDENCE." >&2
    exit 1
fi
printf '%s\n' \
    'sophia_mirror_group_gate schema=1 status=visual_confirmed outputs=1 connectors=2 heads=2 dp1_mode=2560x1440 dp2_mode=1920x1080' | tee -a "$EVIDENCE"
printf '%s\n' \
    'sophia_mirror_group_visual schema=1 status=confirmed content=logically_identical case=mixed legibility=stable_both scaled_text=sharp_not_blocky foreground=white background=black' | tee -a "$EVIDENCE"
if ! "$ROOT_DIR/tools/verify_mirror_group_physical.sh" --candidate "$EVIDENCE"; then
    finish_failed_run verification 1
    echo "Mirror verification failed; diagnostic evidence remains at $EVIDENCE." >&2
    exit 1
fi
printf '%s\n' 'sophia_mirror_group_gate schema=1 status=passed exit=0' | tee -a "$EVIDENCE"
"$ROOT_DIR/tools/verify_mirror_group_physical.sh" "$EVIDENCE"
"$ROOT_DIR/tools/archive_mirror_group_physical_run.sh" "$EVIDENCE"
echo "Full verified log at $EVIDENCE."
