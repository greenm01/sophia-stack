#!/usr/bin/env bash
set -euo pipefail

# Answers the question mirroring rests on and cannot settle offline: will this
# driver let two connectors scan out one framebuffer?
#
# Validation-only. Every commit carries TEST_ONLY, so the kernel checks the
# request and discards it and no output changes. Unlike the topology probe this
# does allocate -- a shared framebuffer cannot be tested without one -- but the
# buffers are dumb buffers created and destroyed inside the probe, never shown.
# It needs DRM master, so it must run from a TTY with no compositor holding the
# card.
#
# A rejection alone would be ambiguous, so the probe runs a control: the same two
# heads with a framebuffer each. That separates "this driver refuses shared
# framebuffers" from "this driver refuses two CRTCs in one commit", and those two
# answers send the design somewhere completely different.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROBE_FILE="${SOPHIA_NATIVE_MIRROR_PROBE:-/tmp/sophia-native-mirror-probe.log}"

# shellcheck source=tools/lib/drm_master_guard.sh
. "$ROOT_DIR/tools/lib/drm_master_guard.sh"

mkdir -p "$(dirname "$PROBE_FILE")"
: > "$PROBE_FILE"

echo "Sophia native mirror probe"
echo "Validation-only: every commit carries TEST_ONLY, so no output changes."
echo "Probe: $PROBE_FILE"
echo

sophia_require_drm_master_available SOPHIA_NATIVE_MIRROR_PROBE_FORCE || exit 1

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
        -- native-mirror-probe
) 2>&1 | tee "$PROBE_FILE"
probe_status="${PIPESTATUS[0]}"
set -e

echo
line="$(grep -m1 '^sophia_native_mirror_probe ' "$PROBE_FILE" || true)"
if [[ -z "$line" ]]; then
    echo "Probe produced no report line; output left at $PROBE_FILE" >&2
    exit "${probe_status:-1}"
fi

field() { sed -n "s/.* $1=\([^ ]*\).*/\1/p" <<<"$line"; }
status="$(field status)"
connectors="$(field connectors)"
mode="$(field common_mode)"
shared="$(field shared_framebuffer)"
shared_errno="$(field shared_framebuffer_errno)"
separate="$(field separate_framebuffers)"
separate_errno="$(field separate_framebuffers_errno)"

errno_name() {
    case "$1" in
        0) echo "none" ;;
        1) echo "EPERM" ;;
        13) echo "EACCES" ;;
        22) echo "EINVAL" ;;
        28) echo "ENOSPC" ;;
        *) echo "errno $1" ;;
    esac
}

echo "Status:     $status"
echo "Connectors: $connectors at $mode"
echo "Shared FB:  $shared ($(errno_name "${shared_errno:-0}"))"
echo "Separate:   $separate ($(errno_name "${separate_errno:-0}"))"
echo

case "$status" in
    Probed) ;;
    SingleHead)
        echo "This machine has fewer than two connected connectors on one card, so"
        echo "it cannot present the question. Attach a second display and re-run."
        exit 2
        ;;
    NoCommonMode)
        echo "The two connectors share no mode. Mirroring is same-mode only because"
        echo "no plane scaling exists on this path, so there is nothing to ask here."
        exit 2
        ;;
    MasterUnavailable)
        echo "Another compositor holds DRM master. This says nothing about mirroring."
        exit 2
        ;;
    *)
        echo "The probe stopped before reaching the question: $status"
        exit 2
        ;;
esac

run_page_flip_phase() {
    # Phase two commits for real, so it is opt-in. It is still as close to a
    # no-op as the question allows: each CRTC is flipped to the framebuffer it is
    # already scanning out, at its current mode, with no ALLOW_MODESET. Nothing
    # on screen changes. TEST_ONLY cannot answer it, because the kernel rejects
    # that together with PAGE_FLIP_EVENT before looking at anything else -- and
    # the event behaviour is exactly what is being asked.
    echo
    echo "Phase 2: how many events does a two-CRTC page flip deliver?"
    set +e
    (
        cd "$ROOT_DIR"
        SOPHIA_NATIVE_MIRROR_PAGE_FLIP=1 cargo run --quiet --offline -p sophia-cli \
            --features "atomic-scanout-live" \
            -- native-mirror-page-flip
    ) 2>&1 | tee -a "$PROBE_FILE"
    set -e

    flip_line="$(grep -m1 '^sophia_native_mirror_page_flip ' "$PROBE_FILE" || true)"
    if [[ -z "$flip_line" ]]; then
        echo "Phase 2 produced no report line; output left at $PROBE_FILE" >&2
        return 1
    fi
    flip_field() { sed -n "s/.* $1=\([^ ]*\).*/\1/p" <<<"$flip_line"; }
    local phase commit first second unattributed
    phase="$(flip_field phase)"
    commit="$(flip_field commit)"
    first="$(flip_field events_first_crtc)"
    second="$(flip_field events_second_crtc)"
    unattributed="$(flip_field unattributed_events)"

    echo
    echo "Phase:      $phase"
    echo "Commit:     $commit"
    echo "Events:     first CRTC $first, second CRTC $second, unattributed $unattributed"
    echo

    if [[ "$phase" == "shared_framebuffer_size_mismatch" ]]; then
        echo "The two CRTCs already share one framebuffer but run different modes."
        echo
        echo "That is evidence, not an obstacle: the kernel console is driving two"
        echo "connectors from a single buffer on this hardware right now, which is a"
        echo "stronger demonstration that sharing works than a TEST_ONLY commit gives."
        echo "What it is not is a mirror group -- a group is same-mode, and these are not --"
        echo "so re-presenting the current state cannot answer how a group completes."
        echo "Reshaping the planes to make it one would change what is on the display."
        echo
        echo "Answer the event question inside the tty4 output gate instead, where Sophia"
        echo "owns the modeset and both heads are same-mode by construction."
        return 2
    fi
    if [[ "$phase" == "crtc_inactive" ]]; then
        echo "One of the two CRTCs is not currently scanning out, so there was nothing to"
        echo "re-flip. The probe will not modeset to create the condition -- that would"
        echo "change what is on your display to answer a question about events."
        return 2
    fi
    if [[ "$commit" != "accepted" ]]; then
        echo "The page-flip commit was refused, so the event question is unanswered."
        return 3
    fi
    if [[ "$first" == "1" && "$second" == "1" && "$unattributed" == "0" ]]; then
        echo "CONCLUSION: one event per CRTC."
        echo "Joint retirement is sound: a group that waits for every head will be woken"
        echo "once by each of them."
        return 0
    fi
    echo "CONCLUSION: the event count is NOT one per CRTC."
    echo "Joint retirement as written would wait for a head that never reports, so the"
    echo "retirement rule has to be rebuilt around what the driver actually delivers."
    return 3
}

if [[ "$shared" == "accepted" ]]; then
    echo "CONCLUSION: one framebuffer can drive both connectors."
    echo "A mirror group can own a single buffer, which is what the exporter-per-group"
    echo "and single-ADD_FB2 design assumes."
    if [[ "${SOPHIA_NATIVE_MIRROR_PAGE_FLIP:-0}" == "1" ]]; then
        run_page_flip_phase
        exit $?
    fi
    echo
    echo "Phase 2 (a real page flip, to count events per CRTC) did not run."
    echo "Re-run with SOPHIA_NATIVE_MIRROR_PAGE_FLIP=1 to include it."
    exit 0
fi

if [[ "$separate" != "accepted" ]]; then
    echo "CONCLUSION: inconclusive about sharing."
    echo "The control was refused too, so this driver will not take two CRTCs in one"
    echo "commit at all. That is a bigger constraint than buffer sharing and has to be"
    echo "understood before the group submit is designed."
    exit 3
fi

echo "CONCLUSION: this driver refuses a shared framebuffer."
echo "The control passed, so two CRTCs in one commit are fine and it is the sharing"
echo "specifically that is rejected. A group would need a buffer per head plus a copy,"
echo "which changes the ownership design rather than the submit."
exit 3
