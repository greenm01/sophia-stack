#!/usr/bin/env bash
set -euo pipefail

# Records one verified direct-scanout run as immutable evidence.
#
# The archive binds the run to a signed source commit, the Sophia binary that
# produced it, the client whose buffer reached the plane, and both
# configurations the session loaded -- so a later reader can tell which code,
# which client, and which policy produced the promotion.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
evidence="${1:?usage: archive_direct_scanout_run.sh EVIDENCE}"
state_home="${XDG_STATE_HOME:-$HOME/.local/state}"
run_root="${SOPHIA_DIRECT_SCANOUT_RUN_ROOT:-$state_home/sophia/promotion/direct-scanout-runs}"
sophia_bin="${SOPHIA_DIRECT_SCANOUT_SOPHIA_BIN:-$ROOT_DIR/target/release/sophia}"
client_bin="${SOPHIA_DIRECT_SCANOUT_CLIENT_BIN:-$(command -v kitty || true)}"

"$ROOT_DIR/tools/verify_direct_scanout_standalone.sh" "$evidence" >/dev/null
identity="$(grep -E '^sophia_direct_scanout_identity schema=1 status=bound ' "$evidence")"
source_commit="$(sed -n 's/.* source_commit=\([0-9a-f]\{40\}\) .*/\1/p' <<<"$identity")"
recorded_sophia_sha256="$(sed -n 's/.* sophia_sha256=\([0-9a-f]\{64\}\) .*/\1/p' <<<"$identity")"
recorded_client_sha256="$(sed -n 's/.* client_sha256=\([0-9a-f]\{64\}\) .*/\1/p' <<<"$identity")"
recorded_core_sha256="$(sed -n 's/.* core_sha256=\([0-9a-f]\{64\}\) .*/\1/p' <<<"$identity")"
recorded_desktop_sha256="$(sed -n 's/.* desktop_sha256=\([0-9a-f]\{64\}\)$/\1/p' <<<"$identity")"

[[ "$source_commit" =~ ^[0-9a-f]{40}$ ]] &&
    git -C "$ROOT_DIR" cat-file -e "$source_commit^{commit}" || {
    echo "Direct-scanout evidence has an invalid source commit" >&2
    exit 1
}
git -C "$ROOT_DIR" verify-commit "$source_commit" >/dev/null 2>&1 || {
    echo "Direct-scanout evidence source commit lacks a valid signature" >&2
    exit 1
}
for binary in "$sophia_bin" "$client_bin"; do
    [[ -n "$binary" && -x "$binary" ]] || {
        echo "Direct-scanout evidence binary is unavailable: ${binary:-unset}" >&2
        exit 1
    }
done

evidence_sha256="$(sha256sum "$evidence" | awk '{ print $1 }')"
sophia_sha256="$(sha256sum "$sophia_bin" | awk '{ print $1 }')"
client_sha256="$(sha256sum "$client_bin" | awk '{ print $1 }')"
[[ "$sophia_sha256" == "$recorded_sophia_sha256" ]] || {
    echo "The Sophia binary no longer matches the verified run" >&2
    exit 1
}
[[ "$client_sha256" == "$recorded_client_sha256" ]] || {
    echo "The client binary no longer matches the verified run" >&2
    exit 1
}

install -d -m 700 "$run_root"
# Archiving the same evidence twice would turn one session into two proofs.
if grep -rlFx --include=manifest "evidence_sha256=$evidence_sha256" \
    "$run_root" 2>/dev/null | grep -q .; then
    echo "Direct-scanout evidence is already archived" >&2
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
trap 'rm -rf -- "$run_dir"' ERR HUP INT TERM

install -m 600 "$evidence" "$run_dir/session.log"
printf '%s\n' 'sophia_direct_scanout schema=1 status=passed' >"$run_dir/result.kdl"
printf 'record_schema=1\nrecord_kind=direct_scanout\nrecorded_at_utc=%s\nsource_commit=%s\nevidence_sha256=%s\nsophia_binary_sha256=%s\nclient_binary_sha256=%s\ncore_config_sha256=%s\ndesktop_profile_sha256=%s\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$source_commit" "$evidence_sha256" \
    "$sophia_sha256" "$client_sha256" "$recorded_core_sha256" "$recorded_desktop_sha256" \
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
"$ROOT_DIR/tools/verify_direct_scanout_archive.sh" "$run_dir" >/dev/null
trap - ERR HUP INT TERM
echo "Recorded verified direct-scanout run: $run_dir"
