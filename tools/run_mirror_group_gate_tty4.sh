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
for status in /sys/class/drm/card*-*/status; do
    [[ "$(cat "$status" 2>/dev/null)" == "connected" ]] || continue
    name="${status%/status}"
    printf '  %-20s first_mode=%s\n' "${name##*/}" \
        "$(head -1 "${name}/modes" 2>/dev/null || echo none)"
done
echo

if [[ "$(tty)" != "$TTY_REQUIRED" ]]; then
    echo "You are on $(tty); this needs $TTY_REQUIRED and DRM master." >&2
    echo "Switch with Ctrl+Alt+F4, log in, and run this again from there." >&2
    exit 2
fi

echo "Building..."
(cd "$ROOT_DIR" && cargo build --quiet --release --offline -p sophia-cli \
    --features "atomic-scanout-live" --bin sophia)
echo

if [[ -n "$(git -C "$ROOT_DIR" status --porcelain)" ]]; then
    echo "The Sophia worktree must be clean so the binary has one exact source identity." >&2
    exit 2
fi
source_commit="$(git -C "$ROOT_DIR" rev-parse HEAD)"
sophia_sha256="$(sha256sum "$ROOT_DIR/target/release/sophia" | awk '{ print $1 }')"
profile_sha256="$(sha256sum "$PROFILE" | awk '{ print $1 }')"
: >"$EVIDENCE"
printf 'sophia_mirror_group_gate schema=1 status=starting source_commit=%s sophia_sha256=%s profile_sha256=%s\n' \
    "$source_commit" "$sophia_sha256" "$profile_sha256" | tee -a "$EVIDENCE"

echo "What success looks like, and only you can judge it:"
echo "  - BOTH monitors showing the same thing for ~$((RUNTIME_MSEC / 1000))s"
echo "  - DP-1 still 2560x1440 and DP-2 still 1920x1080, neither downgraded"
echo "  - DP-2 may show black bars if the aspects differ; that is 'fit' working"
echo
echo "Evidence: $EVIDENCE"
echo "Running now."
echo

set +e
(
    cd "$ROOT_DIR"
    # Native scanout is armed separately from the session itself, because a
    # session that drives real KMS is a different act from one that does not.
    SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 \
        ./target/release/sophia sophia-live-session \
        --display="$DISPLAY_NAME" \
        --native-scanout \
        --desktop-profile="$PROFILE" \
        --max-runtime-ms="$RUNTIME_MSEC"
) 2>&1 | tee -a "$EVIDENCE"
status="${PIPESTATUS[0]}"
set -e

echo
echo "=== exit=$status ==="
if (( status != 0 )); then
    printf 'sophia_mirror_group_gate schema=1 status=failed exit=%s\n' "$status" | tee -a "$EVIDENCE"
    exit "$status"
fi
grep -E "sophia_live_native_page_flip|sophia_live_output|mirror|head" "$EVIDENCE" \
    | tail -20 || true
echo
echo "Did both monitors show the same scene, with DP-1 at 2560x1440 and DP-2 at 1920x1080?"
echo "Type yes to record visible-pixel acceptance."
read -r visual_confirmation </dev/tty
if [[ "$visual_confirmation" != "yes" ]]; then
    printf 'sophia_mirror_group_gate schema=1 status=failed reason=visual_confirmation\n' | tee -a "$EVIDENCE"
    echo "Visible mirroring was not confirmed; evidence remains at $EVIDENCE." >&2
    exit 1
fi
printf '%s\n' \
    'sophia_mirror_group_gate schema=1 status=visual_confirmed outputs=1 connectors=2 heads=2 dp1_mode=2560x1440 dp2_mode=1920x1080' \
    'sophia_mirror_group_gate schema=1 status=passed exit=0' | tee -a "$EVIDENCE"
"$ROOT_DIR/tools/verify_mirror_group_physical.sh" "$EVIDENCE"
"$ROOT_DIR/tools/archive_mirror_group_physical_run.sh" "$EVIDENCE"
echo "Full verified log at $EVIDENCE."
