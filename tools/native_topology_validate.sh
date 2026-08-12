#!/usr/bin/env bash
set -euo pipefail

# Runs startup's output-activation path against real hardware and changes nothing.
#
# Capabilities, topology projection, candidate reconciliation, plan preparation,
# head resolution, and the activation phase machine are the same code a session
# runs. Only the test phase reaches the kernel, as one TEST_ONLY request covering
# every head, so the answer is about the whole desktop rather than one output.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_FILE="${SOPHIA_NATIVE_TOPOLOGY_VALIDATE:-/tmp/sophia-native-topology-validate.log}"

# shellcheck source=tools/lib/drm_master_guard.sh
. "$ROOT_DIR/tools/lib/drm_master_guard.sh"

mkdir -p "$(dirname "$EVIDENCE_FILE")"
: > "$EVIDENCE_FILE"

echo "Sophia native topology validation"
echo "Read-only: the executor has no apply, and bringing the scanout up performs no"
echo "modeset. Nothing on screen changes."
echo "Evidence: $EVIDENCE_FILE"
echo

sophia_require_drm_master_available SOPHIA_NATIVE_TOPOLOGY_VALIDATE_FORCE || exit 1

# Build separately so compiler output never lands in the evidence log.
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
        -- native-topology-validate
) 2>&1 | tee "$EVIDENCE_FILE"
validate_status="${PIPESTATUS[0]}"
set -e

echo
line="$(grep -m1 '^sophia_native_topology_validate ' "$EVIDENCE_FILE" || true)"
if [[ -z "$line" ]]; then
    echo "Validation produced no report line; output left at $EVIDENCE_FILE" >&2
    exit "${validate_status:-1}"
fi

field() { sed -n "s/.* $1=\([^ ]*\).*/\1/p" <<<"$line"; }
validation="$(field validation)"
settlement="$(field settlement)"
heads="$(field heads)"

echo "Conclusion"
case "$validation" in
    accepted)
        echo "  The kernel accepted the configured topology across $heads head(s) as one"
        echo "  atomic request. The candidate this host would start with is valid."
        if [[ "$settlement" != "not_applied" ]]; then
            echo "  WARNING: settlement=$settlement, but a validation executor has no apply."
        fi
        ;;
    rejected)
        echo "  The kernel refused the configured topology. The candidate this host would"
        echo "  start with cannot be driven as one desktop. Capture $EVIDENCE_FILE."
        ;;
    busy)
        echo "  The device could not take the request now. Nothing was concluded; retry."
        ;;
    unbuildable)
        echo "  The heads could not be composed into a request, so nothing was submitted."
        echo "  That is a mistake in what was asked for, not hardware declining it."
        ;;
    *)
        echo "  No validation was attempted (validation=$validation)."
        ;;
esac

echo
echo "Report line: $line"
echo "Evidence:    $EVIDENCE_FILE"

exit "$validate_status"
