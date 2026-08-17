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

for required in \
    '^sophia_output_v1_reference schema=1 status=settled kind=Committed topology_epoch=[0-9]+ heads=3 groups=2$' \
    'sophia_live_output_authority schema=2 status=committed .* outputs=2 ' \
    '^sophia_live_output_topology_health schema=1 status=clean quarantined=false$' \
    '^sophia_live_session_health schema=1 status=clean '; do
    grep -Eq "$required" "$EVIDENCE" || {
        printf 'sophia_mixed_output_gate schema=1 status=failed stage=telemetry exit=1\n' \
            | tee -a "$EVIDENCE"
        echo "Mixed-output telemetry requirement is missing: $required" >&2
        exit 1
    }
done
if grep -Eq '(^Error:|panicked at|status=(failed|degraded|rolled_back)([[:space:]]|$))' \
    "$EVIDENCE"; then
    echo "Mixed-output evidence contains a failure, degradation, or rollback." >&2
    exit 1
fi
if grep -Eq 'sophia_native_composition_sampling schema=2 status=(fallback|unavailable)([[:space:]]|$)' \
    "$EVIDENCE"; then
    echo "Mixed-output evidence contains a composition sampling fallback." >&2
    exit 1
fi

mapfile -t extended_ready < <(
    grep -E "sophia_live_native_head schema=2 status=ready .* connector=$EXTENDED " "$EVIDENCE"
)
if (( ${#extended_ready[@]} != 1 )); then
    echo "The extended connector did not map to exactly one opaque head." >&2
    exit 1
fi
extended_head="$(sed -n 's/.* head=\([0-9][0-9]*\) .*/\1/p' <<<"${extended_ready[0]}")"
committed_transaction="$(sed -n 's/.*sophia_live_output_authority schema=2 status=committed transaction=\([0-9][0-9]*\) .*/\1/p' "$EVIDENCE" | tail -n1)"
effect_line="$(grep -nE "sophia_live_output_authority schema=1 status=effect_pending transaction=$committed_transaction " "$EVIDENCE" | tail -n1 | cut -d: -f1)"
first_presented_line="$(grep -nEm1 "sophia_live_output_authority schema=2 status=first_presented transaction=$committed_transaction " "$EVIDENCE" | cut -d: -f1)"
if [[ -z "$extended_head" || -z "$committed_transaction" || -z "$effect_line" \
    || -z "$first_presented_line" || "$effect_line" -ge "$first_presented_line" ]]; then
    echo "Mixed-output first-presentation ordering is incomplete." >&2
    exit 1
fi
sed -n "${effect_line},${first_presented_line}p" "$EVIDENCE" \
    | grep -Eq "sophia_live_head_composition_plan schema=1 status=ready .* head=$extended_head .* mapping=exact exact=1 downsampled=0 upsampled=0 active=1 fallback=0 unavailable=0 " || {
        echo "The extended head did not prepare an exact, unsampled first frame." >&2
        exit 1
    }

mapfile -t completed_outputs < <(
    grep -E 'sophia_live_native_head schema=3 status=complete ' "$EVIDENCE" \
        | sed -n 's/.* output=\([0-9][0-9]*\) .*/\1/p' \
        | sort | uniq -c | awk '{ print $1 }' | sort -n
)
if [[ "${completed_outputs[*]}" != "1 2" ]]; then
    echo "Completed heads do not prove one singleton and one two-head logical output." >&2
    exit 1
fi

echo "Did the two mirror heads show matching content, and did the extended head show"
echo "the SOPHIA MIXED NATIVE SHARP marker with crisp outline text and no soft resampling?"
echo "Type yes to record visible-pixel acceptance."
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
