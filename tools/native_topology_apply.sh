#!/usr/bin/env bash
set -euo pipefail

# Applies the configured topology to real outputs. THIS CHANGES WHAT MONITORS SHOW.
#
# The bound that makes this survivable: apply heads reuse the framebuffer each CRTC
# already scans out, so the only topology reachable is one whose scanout size
# matches what is displayed. Anything else declines as NeedsFramebuffer before a
# single commit is submitted. The first useful run therefore re-applies the topology
# already on screen, where success looks like nothing happening.
#
# Rollback heads are resolved before apply runs, from the topology still on screen,
# and the reducer requires a terminal rollback settlement after an apply failure.
#
# Recovery if a screen goes dark anyway: the kernel restores console output on VT
# switch, so Ctrl-Alt-F<n> to another VT and back. This process holds DRM master
# only while it runs, and it always exits.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_FILE="${SOPHIA_NATIVE_TOPOLOGY_APPLY:-/tmp/sophia-native-topology-apply.log}"
CONFIRM="${SOPHIA_NATIVE_OUTPUT_APPLY:-0}"

# shellcheck source=tools/lib/drm_master_guard.sh
. "$ROOT_DIR/tools/lib/drm_master_guard.sh"

mkdir -p "$(dirname "$EVIDENCE_FILE")"
: > "$EVIDENCE_FILE"

echo "Sophia native topology apply"
echo "This MUTATES real output state. Read the header of this script before running."
echo "Evidence: $EVIDENCE_FILE"
echo

if [[ "$CONFIRM" != "1" ]]; then
    echo "Refusing: set SOPHIA_NATIVE_OUTPUT_APPLY=1 to apply a topology to real" >&2
    echo "outputs. Run tools/native_topology_validate.sh first; it answers whether the" >&2
    echo "kernel accepts this desktop without changing anything." >&2
    exit 1
fi

sophia_require_drm_master_available SOPHIA_NATIVE_TOPOLOGY_APPLY_FORCE || exit 1

# Validate before mutating. A rejected topology must never reach apply, and finding
# that out with TEST_ONLY costs one ioctl.
echo "Validating before applying..."
"$ROOT_DIR/tools/native_topology_validate.sh" >/dev/null || {
    echo "Validation failed; refusing to apply. Run tools/native_topology_validate.sh" >&2
    echo "directly to see why." >&2
    exit 1
}
echo "Validation passed."
echo

echo "Building sophia-cli..."
(
    cd "$ROOT_DIR"
    cargo build --quiet --offline -p sophia-cli --features "atomic-scanout-live" --bin sophia
)

set +e
(
    cd "$ROOT_DIR"
    SOPHIA_NATIVE_OUTPUT_APPLY=1 \
        cargo run --quiet --offline -p sophia-cli \
        --features "atomic-scanout-live" \
        -- native-topology-apply
) 2>&1 | tee "$EVIDENCE_FILE"
apply_status="${PIPESTATUS[0]}"
set -e

echo
line="$(grep -m1 '^sophia_native_topology_apply ' "$EVIDENCE_FILE" || true)"
if [[ -z "$line" ]]; then
    echo "Apply produced no report line; output left at $EVIDENCE_FILE" >&2
    exit "${apply_status:-1}"
fi

field() { sed -n "s/.* $1=\([^ ]*\).*/\1/p" <<<"$line"; }
settlement="$(field settlement)"
rollback="$(field rollback)"

echo "Conclusion"
case "$settlement" in
    activated)
        echo "  The topology was applied. Every output is driven by the candidate this"
        echo "  host is configured for, committed as one atomic request."
        ;;
    not_applied)
        case "$rollback" in
            not_required)
                echo "  Apply was never reached, so nothing changed and nothing needed undoing."
                ;;
            restored)
                echo "  Apply failed and the previous topology was restored. The desktop is back"
                echo "  where it started."
                ;;
            failed)
                echo "  APPLY FAILED AND ROLLBACK FAILED. Output state is not what this host"
                echo "  started with and Sophia could not put it back. Switch VT with"
                echo "  Ctrl-Alt-F<n> and back to make the kernel restore the console."
                ;;
            *)
                echo "  Apply did not complete (rollback=$rollback)."
                ;;
        esac
        ;;
    *)
        echo "  Unrecognized settlement=$settlement."
        ;;
esac

echo
echo "Report line: $line"
echo "Evidence:    $EVIDENCE_FILE"

exit "$apply_status"
