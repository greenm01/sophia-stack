#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
evidence="${1:?usage: archive_hagia_policy_physical_run.sh EVIDENCE [PROOF_TEXT]}"
proof_text="${2:-hagiapolicyproof}"
state_home="${XDG_STATE_HOME:-$HOME/.local/state}"
run_root="${SOPHIA_HAGIA_POLICY_RUN_ROOT:-$state_home/sophia/promotion/hagia-policy-runs}"
sophia_bin="${SOPHIA_HAGIA_POLICY_SOPHIA_BIN:-$ROOT_DIR/target/release/sophia}"
hagia_bin="${SOPHIA_HAGIA_BIN:-/opt/sophia/current/target/release/hagia}"
source_commit="${SOPHIA_HAGIA_PHYSICAL_SOURCE_COMMIT:-$(git -C "$ROOT_DIR" rev-parse HEAD)}"

"$ROOT_DIR/tools/verify_hagia_policy_physical.sh" "$evidence" "$proof_text" >/dev/null
[[ "$source_commit" =~ ^[0-9a-f]{40}$ ]] \
    && git -C "$ROOT_DIR" cat-file -e "$source_commit^{commit}" \
    || { echo "Hagia physical evidence has an invalid Sophia source commit" >&2; exit 1; }
for binary in "$sophia_bin" "$hagia_bin"; do
    [[ -x "$binary" ]] || {
        echo "Hagia physical evidence binary is unavailable: $binary" >&2
        exit 1
    }
done

evidence_sha256="$(sha256sum "$evidence" | awk '{ print $1 }')"
sophia_sha256="$(sha256sum "$sophia_bin" | awk '{ print $1 }')"
hagia_sha256="$(sha256sum "$hagia_bin" | awk '{ print $1 }')"
install -d -m 700 "$run_root"
if grep -rlFx --include=manifest "evidence_sha256=$evidence_sha256" \
    "$run_root" 2>/dev/null | grep -q .; then
    echo "Hagia physical policy evidence is already archived" >&2
    exit 1
fi

sequence=1
while true; do
    run_dir="$run_root/$(printf '%04d' "$sequence")"
    if mkdir -m 700 "$run_dir" 2>/dev/null; then
        break
    fi
    sequence=$((sequence + 1))
done
trap 'rm -rf "$run_dir"' ERR HUP INT TERM

install -m 600 "$evidence" "$run_dir/session.log"
printf '%s\n' \
    'sophia_hagia_policy_physical schema=1 status=passed' \
    >"$run_dir/result.kdl"
printf 'record_schema=1\nrecord_kind=hagia_policy_physical\nrecorded_at_utc=%s\nsource_commit=%s\nproof_text=%s\nevidence_sha256=%s\nsophia_binary_sha256=%s\nhagia_binary_sha256=%s\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$source_commit" "$proof_text" \
    "$evidence_sha256" "$sophia_sha256" "$hagia_sha256" \
    >"$run_dir/manifest"
chmod 600 "$run_dir/manifest" "$run_dir/result.kdl"
(
    cd "$run_dir"
    sha256sum manifest result.kdl session.log >SHA256SUMS
)
chmod 600 "$run_dir/SHA256SUMS"

(
    cd "$run_dir"
    sha256sum -c --status SHA256SUMS
)
"$ROOT_DIR/tools/verify_hagia_policy_physical.sh" \
    "$run_dir/session.log" "$proof_text" >/dev/null
trap - ERR HUP INT TERM
echo "Recorded verified Hagia physical policy run: $run_dir"
