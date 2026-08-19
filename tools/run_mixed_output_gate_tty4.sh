#!/usr/bin/env bash
set -euo pipefail

# Signed physical proof for one two-head mirror group beside one extended head.
# This performs a real modeset and therefore runs only from a recovery-safe TTY.

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
ROOT_DIR="$(cd "$(dirname "$SCRIPT_PATH")/.." && pwd)"
# This rig's one-command default keeps the previously fuzzy DP-2 head native.
# Environment overrides preserve the runner for other three-head arrangements.
MIRROR_PRIMARY="${SOPHIA_MIXED_MIRROR_PRIMARY:-DP-1}"
MIRROR_MEMBER="${SOPHIA_MIXED_MIRROR_MEMBER:-DP-3}"
EXTENDED="${SOPHIA_MIXED_EXTENDED:-DP-2}"
# The public output role receives connector-neutral labels. On this signed rig,
# startup discovery maps the three connector checks above to these opaque heads.
MIRROR_PRIMARY_LABEL="${SOPHIA_MIXED_MIRROR_PRIMARY_LABEL:-Display 1}"
MIRROR_MEMBER_LABEL="${SOPHIA_MIXED_MIRROR_MEMBER_LABEL:-Display 3}"
EXTENDED_LABEL="${SOPHIA_MIXED_EXTENDED_LABEL:-Display 2}"
KITTY_BIN="${SOPHIA_MIXED_KITTY:-$(command -v kitty || true)}"
RUNTIME_MSEC="${SOPHIA_MIXED_RUNTIME_MSEC:-30000}"
DISPLAY_NAME="${SOPHIA_MIXED_DISPLAY:-:294}"
EVIDENCE="${SOPHIA_MIXED_EVIDENCE:-/tmp/sophia-mixed-output.log}"
TTY_REQUIRED="${SOPHIA_MIXED_TTY:-/dev/tty4}"
CORE_CONFIG="$ROOT_DIR/tools/config/sophia-xmonad/core.kdl"
DESKTOP_PROFILE="$ROOT_DIR/tools/fixtures/mixed_output_probe.kdl"

# shellcheck source=tools/lib/drm_master_guard.sh
. "$ROOT_DIR/tools/lib/drm_master_guard.sh"

if [[ "$MIRROR_PRIMARY" == "$MIRROR_MEMBER" \
    || "$MIRROR_PRIMARY" == "$EXTENDED" \
    || "$MIRROR_MEMBER" == "$EXTENDED" ]]; then
    echo "The three mixed-topology connector labels must be distinct." >&2
    exit 2
fi
if [[ "$KITTY_BIN" != /* || ! -x "$KITTY_BIN" ]]; then
    echo "Set SOPHIA_MIXED_KITTY to an absolute executable Kitty path." >&2
    exit 2
fi
if [[ ! -r "$CORE_CONFIG" || ! -r "$DESKTOP_PROFILE" ]]; then
    echo "The mixed-output proof configuration is missing from the signed tree." >&2
    exit 2
fi
if [[ ! "$RUNTIME_MSEC" =~ ^[0-9]+$ ]] || (( RUNTIME_MSEC < 15000 )); then
    echo "SOPHIA_MIXED_RUNTIME_MSEC must be an integer of at least 15000." >&2
    exit 2
fi
if [[ "$(tty)" != "$TTY_REQUIRED" ]]; then
    echo "Run this real-modeset gate from $TTY_REQUIRED; current TTY is $(tty)." >&2
    exit 2
fi

mapfile -t connected_labels < <(
    for status in /sys/class/drm/card*-*/status; do
        [[ -r "$status" && "$(<"$status")" == connected ]] || continue
        connector="${status%/status}"
        basename "$connector" | sed -E 's/^card[0-9]+-//'
    done | sort
)
if (( ${#connected_labels[@]} != 3 )); then
    echo "The mixed proof requires exactly three connected physical heads; observed ${#connected_labels[@]}." >&2
    printf '  %s\n' "${connected_labels[@]}" >&2
    exit 2
fi
for required in "$MIRROR_PRIMARY" "$MIRROR_MEMBER" "$EXTENDED"; do
    printf '%s\n' "${connected_labels[@]}" | grep -Fxq "$required" || {
        echo "Configured proof connector is not connected: $required" >&2
        exit 2
    }
done

if [[ -n "$(git -C "$ROOT_DIR" status --porcelain --untracked-files=all)" ]]; then
    echo "Sophia worktree must be clean before a signed physical gate." >&2
    exit 2
fi
source_commit="$(git -C "$ROOT_DIR" rev-parse HEAD)"
git -C "$ROOT_DIR" verify-commit "$source_commit" >/dev/null 2>&1 || {
    echo "Sophia HEAD must have a valid cryptographic signature." >&2
    exit 2
}
sophia_require_drm_master_available SOPHIA_MIXED_FORCE || exit 1

echo "Building the signed mixed-topology candidate..."
(
    cd "$ROOT_DIR"
    cargo build --quiet --release --offline -p sophia-cli -p sophia-wm-demo \
        --features sophia-cli/atomic-scanout-live
)
if [[ -n "$(git -C "$ROOT_DIR" status --porcelain --untracked-files=all)" \
    || "$(git -C "$ROOT_DIR" rev-parse HEAD)" != "$source_commit" ]]; then
    echo "Sophia source identity changed during the physical-gate build." >&2
    exit 2
fi
git -C "$ROOT_DIR" verify-commit "$source_commit" >/dev/null 2>&1 || {
    echo "Sophia HEAD signature no longer verifies after the build." >&2
    exit 2
}

sophia_bin="$ROOT_DIR/target/release/sophia"
wm_bin="$ROOT_DIR/target/release/sophia-wm-demo"
sophia_sha256="$(sha256sum "$sophia_bin" | awk '{ print $1 }')"
wm_sha256="$(sha256sum "$wm_bin" | awk '{ print $1 }')"
: >"$EVIDENCE"
printf 'sophia_mixed_output_gate schema=1 status=starting source_commit=%s sophia_sha256=%s wm_sha256=%s heads=3 groups=2\n' \
    "$source_commit" "$sophia_sha256" "$wm_sha256" | tee -a "$EVIDENCE"

echo "The session will first start extended, then sophia_output_v1 will form:"
echo "  mirror:   $MIRROR_PRIMARY + $MIRROR_MEMBER"
echo "  extended: $EXTENDED"
echo "  authority labels: $MIRROR_PRIMARY_LABEL + $MIRROR_MEMBER_LABEL; $EXTENDED_LABEL"
echo "One Kitty should remain on the mirrored output; the other should move to the"
echo "right-hand extended output. The extended Kitty is the native-sharp visual gate."

set +e
(
    cd "$ROOT_DIR"
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
    RUST_LOG="${RUST_LOG:-sophia=info,sophia_backend_live=info}" \
        "$sophia_bin" sophia-live-session \
        --display="$DISPLAY_NAME" \
        --native-scanout \
        --config="$CORE_CONFIG" \
        --desktop-profile="$DESKTOP_PROFILE" \
        --session-mode=normal \
        --session-app=mirror="$KITTY_BIN" \
        --session-start=mirror \
        --session-app-arg=mirror=--config \
        --session-app-arg=mirror=NONE \
        --session-app-arg=mirror=--override \
        --session-app-arg=mirror=linux_display_server=x11 \
        --session-app-arg=mirror=--override \
        --session-app-arg=mirror=cursor_blink_interval=0 \
        --session-app-arg=mirror=sh \
        --session-app-arg=mirror=-lc \
        --session-app-arg=mirror='printf "SOPHIA MIXED NATIVE SHARP AaZz 0123456789\n"; exec sh -i' \
        --session-action-app=terminal=mirror \
        --session-app=proof="$KITTY_BIN" \
        --session-start=proof \
        --session-app-arg=proof=--config \
        --session-app-arg=proof=NONE \
        --session-app-arg=proof=--override \
        --session-app-arg=proof=linux_display_server=x11 \
        --session-app-arg=proof=--override \
        --session-app-arg=proof=cursor_blink_interval=0 \
        --session-app-arg=proof=sh \
        --session-app-arg=proof=-lc \
        --session-app-arg=proof='printf "SOPHIA MIXED NATIVE SHARP AaZz 0123456789\n"; exec sh -i' \
        --session-action-app=firefox=proof \
        --wm-process="$wm_bin" \
        --wm-interface=sophia_wm_v1 \
        --wm-process-arg=live-mixed-output-proof \
        --wm-process-arg="$MIRROR_PRIMARY_LABEL" \
        --wm-process-arg="$MIRROR_MEMBER_LABEL" \
        --wm-process-arg="$EXTENDED_LABEL" \
        --max-runtime-ms="$RUNTIME_MSEC"
) 2>&1 | tee -a "$EVIDENCE"
status="${PIPESTATUS[0]}"
set -e
if (( status != 0 )); then
    printf 'sophia_mixed_output_gate schema=1 status=failed stage=runtime exit=%s\n' "$status" \
        | tee -a "$EVIDENCE"
    exit "$status"
fi

if ! bash "$ROOT_DIR/tools/verify_mixed_output_evidence.sh" "$EVIDENCE" "$EXTENDED" \
    | tee -a "$EVIDENCE"; then
    printf 'sophia_mixed_output_gate schema=1 status=failed stage=telemetry exit=1\n' \
        | tee -a "$EVIDENCE"
    exit 1
fi

# The mirror member is a resampled copy by construction: one logical output at
# the primary's mode, one client buffer, and a smaller panel to put it on. Only
# the extended head carries the sharpness claim, and an operator asked about
# "matching content" cannot be expected to know that the softer copy is correct.
echo "Three screens, three questions:"
echo "  $MIRROR_PRIMARY (mirror primary, native): shows the desktop crisply."
echo "  $MIRROR_MEMBER (mirror member): shows the SAME content as $MIRROR_PRIMARY."
echo "    It is a downscaled copy and is expected to look softer -- that is not a fault."
echo "  $EXTENDED (extended, native): shows its own terminal with the"
echo "    SOPHIA MIXED NATIVE SHARP marker, crisp, with no soft resampling."
echo "Type yes if all three held."
confirmation=
if ! read -r confirmation </dev/tty || [[ "$confirmation" != yes ]]; then
    printf 'sophia_mixed_output_gate schema=1 status=failed stage=visual_confirmation exit=1\n' \
        | tee -a "$EVIDENCE"
    exit 1
fi
printf 'sophia_mixed_output_visual schema=1 status=confirmed mirror_content=matched extended_text=sharp resampling=none heads=3 groups=2\n' \
    | tee -a "$EVIDENCE"
printf 'sophia_mixed_output_gate schema=1 status=passed exit=0\n' | tee -a "$EVIDENCE"
echo "Verified candidate evidence: $EVIDENCE"
