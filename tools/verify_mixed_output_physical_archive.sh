#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
state_home="${XDG_STATE_HOME:-$HOME/.local/state}"
run_root="${SOPHIA_MIXED_RUN_ROOT:-$state_home/sophia/promotion/mixed-output-runs}"
run="${1:-}"
if [[ -z "$run" ]]; then
    run="$(find "$run_root" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort -V | tail -n 1 || true)"
fi
[[ -n "$run" && -s "$run/SHA256SUMS" ]] || {
    echo "mixed-output archive is missing: ${run:-$run_root}" >&2
    exit 1
}
(
    cd "$run"
    sha256sum -c --status SHA256SUMS
) || {
    echo "mixed-output archive checksum verification failed: $run" >&2
    exit 1
}
[[ "$(sed -n 's/^record_schema=//p' "$run/manifest")" == 1 \
    && "$(sed -n 's/^record_kind=//p' "$run/manifest")" == mixed_output_physical ]] || {
    echo "mixed-output archive has the wrong record identity: $run" >&2
    exit 1
}
[[ "$(cat "$run/result.kdl")" == 'sophia_mixed_output_physical schema=1 status=passed' ]] || {
    echo "mixed-output archive is not passing: $run" >&2
    exit 1
}
for key in source_commit extended_connector evidence_sha256 sophia_binary_sha256 \
    wm_binary_sha256 core_config_sha256 desktop_profile_sha256; do
    [[ "$(grep -c "^${key}=" "$run/manifest")" == 1 ]] || {
        echo "mixed-output archive has invalid $key cardinality: $run" >&2
        exit 1
    }
done

source_commit="$(sed -n 's/^source_commit=//p' "$run/manifest")"
[[ "$source_commit" =~ ^[0-9a-f]{40}$ ]] \
    && git -C "$ROOT_DIR" cat-file -e "$source_commit^{commit}" || {
    echo "mixed-output archive has an invalid source commit: $run" >&2
    exit 1
}
git -C "$ROOT_DIR" verify-commit "$source_commit" >/dev/null 2>&1 || {
    echo "mixed-output archive source commit does not have a valid signature: $run" >&2
    exit 1
}

check_digest() {
    local key="$1" file="$2" actual
    actual="$(sha256sum "$run/$file" | awk '{ print $1 }')"
    [[ "$(sed -n "s/^${key}=//p" "$run/manifest")" == "$actual" ]] || {
        echo "mixed-output $file digest does not match its manifest: $run" >&2
        exit 1
    }
}
check_digest evidence_sha256 session.log
check_digest core_config_sha256 core.kdl
check_digest desktop_profile_sha256 desktop-profile.kdl

committed_file_sha256() {
    local path="$1"
    git -C "$ROOT_DIR" show "$source_commit:$path" | sha256sum | awk '{ print $1 }'
}
[[ "$(sed -n 's/^core_config_sha256=//p' "$run/manifest")" == \
    "$(committed_file_sha256 tools/config/sophia/core.kdl)" ]] || {
    echo "mixed-output archive core configuration is not from its signed commit: $run" >&2
    exit 1
}
[[ "$(sed -n 's/^desktop_profile_sha256=//p' "$run/manifest")" == \
    "$(committed_file_sha256 tools/fixtures/mixed_output_probe.kdl)" ]] || {
    echo "mixed-output archive desktop profile is not from its signed commit: $run" >&2
    exit 1
}

mapfile -t identities < <(
    grep -E '^sophia_mixed_output_gate schema=1 status=starting source_commit=[0-9a-f]{40} sophia_sha256=[0-9a-f]{64} wm_sha256=[0-9a-f]{64} heads=3 groups=2$' \
        "$run/session.log" || true
)
(( ${#identities[@]} == 1 )) || {
    echo "mixed-output archive has invalid identity cardinality: $run" >&2
    exit 1
}
identity="${identities[0]}"
[[ "$(sed -n 's/.* source_commit=\([0-9a-f]\{40\}\) .*/\1/p' <<<"$identity")" == "$source_commit" ]] || {
    echo "mixed-output evidence and manifest name different source commits: $run" >&2
    exit 1
}
[[ "$(sed -n 's/.* sophia_sha256=\([0-9a-f]\{64\}\) .*/\1/p' <<<"$identity")" == \
    "$(sed -n 's/^sophia_binary_sha256=//p' "$run/manifest")" ]] || {
    echo "mixed-output evidence and manifest name different Sophia binaries: $run" >&2
    exit 1
}
[[ "$(sed -n 's/.* wm_sha256=\([0-9a-f]\{64\}\) .*/\1/p' <<<"$identity")" == \
    "$(sed -n 's/^wm_binary_sha256=//p' "$run/manifest")" ]] || {
    echo "mixed-output evidence and manifest name different WM binaries: $run" >&2
    exit 1
}
grep -Fxq \
    'sophia_mixed_output_visual schema=1 status=confirmed mirror_content=matched extended_text=sharp resampling=none heads=3 groups=2' \
    "$run/session.log" || {
    echo "mixed-output archive lacks exact visible-pixel acceptance: $run" >&2
    exit 1
}
grep -Fxq 'sophia_mixed_output_gate schema=1 status=passed exit=0' "$run/session.log" || {
    echo "mixed-output archive lacks its passing gate record: $run" >&2
    exit 1
}
extended_connector="$(sed -n 's/^extended_connector=//p' "$run/manifest")"
[[ "$extended_connector" =~ ^[A-Za-z0-9._-]+$ ]] || {
    echo "mixed-output archive has an invalid extended connector: $run" >&2
    exit 1
}
"$ROOT_DIR/tools/verify_mixed_output_evidence.sh" \
    "$run/session.log" "$extended_connector" >/dev/null

echo "mixed-output physical archive verified: run=$run commit=$source_commit"
