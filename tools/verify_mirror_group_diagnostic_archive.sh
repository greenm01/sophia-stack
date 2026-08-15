#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
state_home="${XDG_STATE_HOME:-$HOME/.local/state}"
run_root="${SOPHIA_MIRROR_DIAGNOSTIC_ROOT:-$state_home/sophia/diagnostics/mirror-group-runs}"
run="${1:-}"
if [[ -z "$run" ]]; then
    run="$(find "$run_root" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort -V | tail -n 1 || true)"
fi
[[ -n "$run" && -s "$run/SHA256SUMS" ]] || {
    echo "mirror-group diagnostic archive is missing: ${run:-$run_root}" >&2
    exit 1
}
(
    cd "$run"
    sha256sum -c --status SHA256SUMS
) || {
    echo "mirror-group diagnostic archive checksum verification failed: $run" >&2
    exit 1
}
[[ "$(sed -n 's/^record_kind=//p' "$run/manifest")" == mirror_group_diagnostic ]] || {
    echo "mirror-group diagnostic archive has the wrong record kind: $run" >&2
    exit 1
}
for key in source_commit evidence_sha256 sophia_binary_sha256 profile_sha256 \
    kernel_delta_sha256 stage exit signal kernel_capture; do
    [[ "$(grep -c "^${key}=" "$run/manifest")" == 1 ]] || {
        echo "mirror-group diagnostic archive has invalid $key cardinality: $run" >&2
        exit 1
    }
done
source_commit="$(sed -n 's/^source_commit=//p' "$run/manifest")"
[[ "$source_commit" =~ ^[0-9a-f]{40}$ ]] &&
    git -C "$ROOT_DIR" cat-file -e "$source_commit^{commit}" || {
    echo "mirror-group diagnostic archive has an invalid source commit: $run" >&2
    exit 1
}
"$ROOT_DIR/tools/verify_mirror_group_diagnostic.sh" \
    "$run/session.log" "$run/kernel-delta.log" >/dev/null

for pair in \
    "evidence_sha256:session.log" \
    "profile_sha256:profile.kdl" \
    "kernel_delta_sha256:kernel-delta.log"; do
    key="${pair%%:*}"
    file="${pair#*:}"
    actual="$(sha256sum "$run/$file" | awk '{ print $1 }')"
    [[ "$(sed -n "s/^${key}=//p" "$run/manifest")" == "$actual" ]] || {
        echo "mirror-group diagnostic $file digest does not match its manifest: $run" >&2
        exit 1
    }
done
identity="$(grep -E '^sophia_mirror_group_gate schema=1 status=starting ' "$run/session.log")"
failure="$(grep -E '^sophia_mirror_group_gate schema=1 status=failed ' "$run/session.log")"
[[ "$(sed -n 's/.* source_commit=\([0-9a-f]\{40\}\) .*/\1/p' <<<"$identity")" == "$source_commit" ]] || {
    echo "mirror-group diagnostic evidence and manifest name different source commits: $run" >&2
    exit 1
}
[[ "$(sed -n 's/.* sophia_sha256=\([0-9a-f]\{64\}\) .*/\1/p' <<<"$identity")" == \
    "$(sed -n 's/^sophia_binary_sha256=//p' "$run/manifest")" ]] || {
    echo "mirror-group diagnostic evidence and manifest name different Sophia binaries: $run" >&2
    exit 1
}
[[ "$(sed -n 's/.* profile_sha256=\([0-9a-f]\{64\}\)$/\1/p' <<<"$identity")" == \
    "$(sed -n 's/^profile_sha256=//p' "$run/manifest")" ]] || {
    echo "mirror-group diagnostic evidence and manifest name different profiles: $run" >&2
    exit 1
}
stage="$(sed -n 's/.* stage=\([^ ]*\) .*/\1/p' <<<"$failure")"
exit_status="$(sed -n 's/.* exit=\([0-9]*\) .*/\1/p' <<<"$failure")"
signal="$(sed -n 's/.* signal=\([0-9]*\) .*/\1/p' <<<"$failure")"
kernel_capture="$(sed -n 's/.* kernel_capture=\([^ ]*\)$/\1/p' <<<"$failure")"
for key in stage exit signal kernel_capture; do
    value="$(sed -n "s/^${key}=//p" "$run/manifest")"
    case "$key" in
        stage) expected="$stage" ;;
        exit) expected="$exit_status" ;;
        signal) expected="$signal" ;;
        kernel_capture) expected="$kernel_capture" ;;
    esac
    [[ "$value" == "$expected" ]] || {
        echo "mirror-group diagnostic $key disagrees between evidence and manifest: $run" >&2
        exit 1
    }
done
expected_result="sophia_mirror_group_diagnostic schema=1 status=failed stage=$stage exit=$exit_status signal=$signal kernel_capture=$kernel_capture"
[[ "$(cat "$run/result.kdl")" == "$expected_result" ]] || {
    echo "mirror-group diagnostic archive result disagrees with its evidence: $run" >&2
    exit 1
}

echo "mirror-group diagnostic archive verified: run=$run commit=$source_commit stage=$stage exit=$exit_status"
