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
without_errno="$(field without_plane_state_errno)"
with_fb="$(field with_current_framebuffer)"
with_fb_errno="$(field with_current_framebuffer_errno)"
reused="$(field reused_framebuffer)"
mode_size="$(field mode_size)"
fb_size="$(field framebuffer_size)"

errno_name() {
    case "$1" in
        0) echo "none reported" ;;
        13) echo "EACCES" ;;
        16) echo "EBUSY" ;;
        22) echo "EINVAL" ;;
        28) echo "ENOSPC" ;;
        34) echo "ERANGE" ;;
        95) echo "EOPNOTSUPP" ;;
        *) echo "errno $1" ;;
    esac
}

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
                echo "  Probe one refused with $(errno_name "$without_errno"): the kernel will not"
                echo "  validate a modeset carrying no plane state."
                if [[ "$with_fb" == "accepted" ]]; then
                    echo "  Probe two, the same modeset plus plane state, was accepted. Validation"
                    echo "  therefore requires a valid FB_ID: a framebuffer must exist at the new"
                    echo "  mode's size before a topology can be validated at all. This decides the"
                    echo "  shape of head resolution."
                elif [[ "$reused" != "true" ]]; then
                    echo "  Probe two never ran: the CRTC had no current framebuffer to reuse, so"
                    echo "  the FB_ID question is still open. Re-run where an output is active."
                elif [[ "$mode_size" != "$fb_size" ]]; then
                    echo "  Probe two also refused with $(errno_name "$with_fb_errno"), but it asked"
                    echo "  for mode $mode_size while reusing a $fb_size framebuffer. That mismatch"
                    echo "  alone explains the refusal, so this run says nothing about FB_ID policy."
                    echo "  Ask again with a scratch framebuffer allocated at the mode's own size."
                else
                    echo "  Probe two also refused with $(errno_name "$with_fb_errno") while mode and"
                    echo "  framebuffer agree at $mode_size. Plane state is present and correctly"
                    echo "  sized, so the refusal is about the topology itself rather than FB_ID."
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
