#!/usr/bin/env bash
set -euo pipefail

# One-shot: prove native output mirroring on real hardware.
#
# Runs the output gate with a profile that groups DP-1 and DP-2 into one logical
# output. Each head keeps its own mode -- DP-1 at 2560x1440 and DP-2 at 1920x1080
# -- and the group's scene is placed onto each. Neither monitor is downgraded,
# which is the whole point of the projection architecture.
#
# This performs a REAL MODESET. Run it from /dev/tty4 with no compositor holding
# DRM master. Your screens will change.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${SOPHIA_MIRROR_PROFILE:-$ROOT_DIR/tools/fixtures/mirror_group_probe.kdl}"

echo "=== Sophia mirror-group gate ==="
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

echo "Connectors as sysfs sees them, before anything is committed:"
for connector in /sys/class/drm/card*-*/status; do
    name="${connector%/status}"
    name="${name##*/}"
    state="$(cat "$connector" 2>/dev/null || echo unknown)"
    [[ "$state" == "connected" ]] || continue
    first_mode="$(head -1 "${connector%/status}/modes" 2>/dev/null || true)"
    printf '  %-20s %s  first_mode=%s\n' "$name" "$state" "${first_mode:-none}"
done
echo

echo "What success looks like:"
echo "  - BOTH monitors showing the same thing"
echo "  - DP-1 still at 2560x1440 and DP-2 still at 1920x1080 (neither downgraded)"
echo "  - the evidence line reporting outputs=1 connectors=2 heads=2"
echo
echo "Known limit: the apply path reuses the framebuffer each CRTC already scans"
echo "out, so it declines a mode CHANGE. This group needs none. If apply reports a"
echo "size mismatch, that is that limit and not a mirroring fault."
echo

if [[ "$(tty)" != "${SOPHIA_NATIVE_OUTPUT_GATE_TTY:-/dev/tty4}" ]]; then
    echo "NOTE: you are on $(tty), and the gate requires ${SOPHIA_NATIVE_OUTPUT_GATE_TTY:-/dev/tty4}."
    echo "Switch with Ctrl+Alt+F4, log in, and run this again from there."
    echo
fi

echo "Running the gate. This commits to your displays."
echo
exec env \
    SOPHIA_NATIVE_OUTPUT_GATE_PROFILE="$PROFILE" \
    SOPHIA_NATIVE_OUTPUT_APPLY=1 \
    "$ROOT_DIR/tools/run_native_output_gate_tty4.sh"
