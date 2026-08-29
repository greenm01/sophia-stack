#!/usr/bin/env bash
set -euo pipefail

# Re-verifies an archived direct-scanout run independently of the run that
# produced it: checksums, record identity, the signed commit, and the evidence
# itself through the same verifier the gate used.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
state_home="${XDG_STATE_HOME:-$HOME/.local/state}"
run_root="${SOPHIA_DIRECT_SCANOUT_RUN_ROOT:-$state_home/sophia/promotion/direct-scanout-runs}"
run="${1:-}"
if [[ -z "$run" ]]; then
    run="$(find "$run_root" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort -V | tail -n 1 || true)"
fi
[[ -n "$run" && -s "$run/SHA256SUMS" ]] || {
    echo "Direct-scanout archive is missing: ${run:-$run_root}" >&2
    exit 1
}
(
    cd "$run"
    sha256sum -c --status SHA256SUMS
) || {
    echo "Direct-scanout archive checksum verification failed: $run" >&2
    exit 1
}

manifest="$run/manifest"
field() {
    local key="$1" value
    value="$(sed -n "s/^$key=\(.*\)$/\1/p" "$manifest")"
    [[ -n "$value" ]] || {
        echo "Direct-scanout manifest is missing $key: $run" >&2
        exit 1
    }
    printf '%s\n' "$value"
}
[[ "$(field record_kind)" == direct_scanout ]] || {
    echo "Direct-scanout manifest records another kind of run: $run" >&2
    exit 1
}
source_commit="$(field source_commit)"
git -C "$ROOT_DIR" cat-file -e "$source_commit^{commit}" 2>/dev/null || {
    echo "Direct-scanout archive names a commit this checkout does not have: $source_commit" >&2
    exit 1
}
git -C "$ROOT_DIR" verify-commit "$source_commit" >/dev/null 2>&1 || {
    echo "Direct-scanout archive names a commit without a valid signature: $source_commit" >&2
    exit 1
}
# The evidence's own identity line and the manifest must agree; a manifest is a
# summary, and a summary that drifts from what it summarises proves nothing.
identity="$(grep -E '^sophia_direct_scanout_identity schema=1 status=bound ' "$run/session.log")"
grep -q "source_commit=$source_commit " <<<"$identity" || {
    echo "Direct-scanout manifest and evidence disagree on the source commit: $run" >&2
    exit 1
}
[[ "$(field evidence_sha256)" == "$(sha256sum "$run/session.log" | awk '{ print $1 }')" ]] || {
    echo "Direct-scanout manifest does not describe its own evidence: $run" >&2
    exit 1
}

"$ROOT_DIR/tools/verify_direct_scanout_standalone.sh" "$run/session.log" >/dev/null
echo "Direct-scanout archive verified: $run"
