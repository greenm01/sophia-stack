#!/usr/bin/env bash
set -euo pipefail

# Answers one question that cannot be settled offline: does a TEST_ONLY modeset
# require plane state and a valid FB_ID? The probe is read-only, but it needs DRM
# master, so it must run from a TTY with no compositor holding the card.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROBE_FILE="${SOPHIA_NATIVE_TOPOLOGY_PROBE:-/tmp/sophia-native-topology-probe.log}"
FORCE="${SOPHIA_NATIVE_TOPOLOGY_PROBE_FORCE:-0}"

mkdir -p "$(dirname "$PROBE_FILE")"
: > "$PROBE_FILE"

echo "Sophia native topology probe"
echo "Read-only: every commit carries TEST_ONLY and the only framebuffer named is"
echo "one the CRTC already scans out, so nothing is allocated and no output changes."
echo "Probe: $PROBE_FILE"
echo

# A compositor holding the card means the probe reports MasterUnavailable and
# concludes nothing. Refusing up front is cheaper than a run that proves nothing.
blockers=()
[[ -n "${DISPLAY:-}" ]] && blockers+=("DISPLAY=${DISPLAY} is set")
[[ -n "${WAYLAND_DISPLAY:-}" ]] && blockers+=("WAYLAND_DISPLAY=${WAYLAND_DISPLAY} is set")
for name in Xorg X Xwayland xlibre-server sophia hyprland sway niri weston kwin_wayland gnome-shell; do
    if pgrep -x "$name" >/dev/null 2>&1; then
        blockers+=("$name is running")
    fi
done

if [[ "${#blockers[@]}" -gt 0 ]]; then
    echo "A display server appears to hold the card:" >&2
    printf '  - %s\n' "${blockers[@]}" >&2
    echo >&2
    if [[ "$FORCE" != "1" ]]; then
        echo "Atomic commits need DRM master even to validate, so this run would report" >&2
        echo "MasterUnavailable and conclude nothing. Switch to a bare TTY with no" >&2
        echo "compositor, or set SOPHIA_NATIVE_TOPOLOGY_PROBE_FORCE=1 to run anyway." >&2
        exit 1
    fi
    echo "SOPHIA_NATIVE_TOPOLOGY_PROBE_FORCE=1; running anyway." >&2
    echo >&2
fi

if [[ ! -d /dev/dri ]]; then
    echo "/dev/dri is missing; no primary card node to probe." >&2
    exit 1
fi

# Build separately so compiler output never lands in the probe log.
echo "Building sophia-cli..."
(
    cd "$ROOT_DIR"
    cargo build --quiet --offline -p sophia-cli --features "atomic-scanout-live" --bin sophia
)

set +e
(
    cd "$ROOT_DIR"
    cargo run --quiet --offline -p sophia-cli \
        --features "atomic-scanout-live" \
        -- native-topology-probe
) 2>&1 | tee "$PROBE_FILE"
probe_status="${PIPESTATUS[0]}"
set -e

echo
line="$(grep -m1 '^sophia_native_topology_probe ' "$PROBE_FILE" || true)"
if [[ -z "$line" ]]; then
    echo "Probe produced no report line; output left at $PROBE_FILE" >&2
    exit "${probe_status:-1}"
fi

field() { sed -n "s/.* $1=\([^ ]*\).*/\1/p" <<<"$line"; }
status="$(field status)"
without="$(field without_plane_state)"
with_fb="$(field with_current_framebuffer)"
reused="$(field reused_framebuffer)"

echo "Conclusion"
case "$status" in
    Probed)
        case "$without" in
            accepted)
                echo "  A TEST_ONLY modeset validates with connector and CRTC state alone."
                echo "  Resolving a candidate into heads does NOT need a framebuffer allocated"
                echo "  at the new mode's size before anything can be checked."
                ;;
            rejected)
                echo "  The kernel refused a modeset carrying no plane state."
                if [[ "$with_fb" == "accepted" ]]; then
                    echo "  The same modeset with plane state was accepted, so validation requires"
                    echo "  a valid FB_ID: a framebuffer must exist at the new mode's size before"
                    echo "  a topology can be validated. This changes the shape of the whole path."
                else
                    echo "  Adding plane state did not help (with_current_framebuffer=$with_fb,"
                    echo "  reused_framebuffer=$reused), so the refusal is not about FB_ID alone."
                    echo "  Capture this log; the topology itself may be invalid on this hardware."
                fi
                ;;
            *)
                echo "  No validation was attempted (without_plane_state=$without)."
                ;;
        esac
        ;;
    MasterUnavailable)
        echo "  Nothing was validated: another process holds DRM master. This is NOT a"
        echo "  rejected topology and must not be recorded as one. Re-run from a bare TTY."
        ;;
    *)
        echo "  The probe reached no conclusion: status=$status"
        ;;
esac

echo
echo "Report line: $line"
echo "Probe log:   $PROBE_FILE"

if [[ "$probe_status" -ne 0 ]]; then
    echo "Probe exited nonzero; nothing was concluded." >&2
fi

exit "$probe_status"
