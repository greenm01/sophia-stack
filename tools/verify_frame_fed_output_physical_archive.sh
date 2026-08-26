#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
state_home="${XDG_STATE_HOME:-$HOME/.local/state}"
run_root="${SOPHIA_FRAME_FED_OUTPUT_RUN_ROOT:-$state_home/sophia/promotion/frame-fed-output-runs}"
hagia_root="${SOPHIA_HAGIA_ROOT:-$ROOT_DIR/../hagia}"
run="${1:-}"
if [[ -z "$run" ]]; then
    run="$(find "$run_root" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort -V | tail -n 1 || true)"
fi
[[ -n "$run" && -s "$run/SHA256SUMS" ]] || {
    echo "frame-fed output archive is missing: ${run:-$run_root}" >&2
    exit 1
}
(
    cd "$run"
    sha256sum -c --status SHA256SUMS
) || {
    echo "frame-fed output archive checksum verification failed: $run" >&2
    exit 1
}
[[ "$(cat "$run/result.kdl")" == \
    'sophia_frame_fed_output_physical schema=1 status=passed boundary=after_apply phases=2' ]] || {
    echo "frame-fed output archive is not passing: $run" >&2
    exit 1
}

required_keys=(
    record_schema record_kind recorded_at_utc source_commit hagia_commit success_text rollback_text
    success_evidence_sha256 rollback_evidence_sha256 evidence_pair_sha256
    sophia_binary_sha256 hagia_binary_sha256 core_config_path core_config_sha256
    desktop_profile_path desktop_profile_sha256 connectors_sha256
)
for key in "${required_keys[@]}"; do
    [[ "$(grep -c "^${key}=" "$run/manifest")" == 1 ]] || {
        echo "frame-fed output archive has invalid $key cardinality: $run" >&2
        exit 1
    }
done
[[ "$(sed -n 's/^record_schema=//p' "$run/manifest")" == 1 \
    && "$(sed -n 's/^record_kind=//p' "$run/manifest")" == frame_fed_output_physical ]] || {
    echo "frame-fed output archive has the wrong record identity: $run" >&2
    exit 1
}

manifest_value() {
    sed -n "s/^$1=//p" "$run/manifest"
}
source_commit="$(manifest_value source_commit)"
hagia_commit="$(manifest_value hagia_commit)"
for repo_and_commit in "$ROOT_DIR:$source_commit" "$hagia_root:$hagia_commit"; do
    repo="${repo_and_commit%:*}"
    commit="${repo_and_commit##*:}"
    [[ "$commit" =~ ^[0-9a-f]{40}$ ]] && git -C "$repo" cat-file -e "$commit^{commit}" \
        || { echo "frame-fed output archive has an invalid source commit: $repo" >&2; exit 1; }
    git -C "$repo" verify-commit "$commit" >/dev/null 2>&1 \
        || { echo "frame-fed output archive source commit lacks a valid signature: $repo" >&2; exit 1; }
done

check_digest() {
    local key="$1" file="$2" actual
    actual="$(sha256sum "$run/$file" | awk '{ print $1 }')"
    [[ "$(manifest_value "$key")" == "$actual" ]] || {
        echo "frame-fed output $file digest does not match its manifest: $run" >&2
        exit 1
    }
}
check_digest success_evidence_sha256 success.log
check_digest rollback_evidence_sha256 rollback.log
check_digest core_config_sha256 core.kdl
check_digest desktop_profile_sha256 desktop-profile.kdl
check_digest connectors_sha256 connectors.txt

pair_sha256="$(printf '%s %s\n' \
    "$(manifest_value success_evidence_sha256)" \
    "$(manifest_value rollback_evidence_sha256)" | sha256sum | awk '{ print $1 }')"
[[ "$(manifest_value evidence_pair_sha256)" == "$pair_sha256" ]] || {
    echo "frame-fed output evidence-pair digest is invalid: $run" >&2
    exit 1
}

valid_repo_path() {
    local path="$1"
    [[ -n "$path" && "$path" != /* && "$path" != ../* && "$path" != *'/../'* ]]
}
core_config_path="$(manifest_value core_config_path)"
desktop_profile_path="$(manifest_value desktop_profile_path)"
valid_repo_path "$core_config_path" && valid_repo_path "$desktop_profile_path" || {
    echo "frame-fed output archive contains an unsafe configuration path: $run" >&2
    exit 1
}
committed_digest() {
    local path="$1"
    git -C "$ROOT_DIR" show "$source_commit:$path" | sha256sum | awk '{ print $1 }'
}
[[ "$(manifest_value core_config_sha256)" == "$(committed_digest "$core_config_path")" \
    && "$(manifest_value desktop_profile_sha256)" == "$(committed_digest "$desktop_profile_path")" ]] || {
    echo "frame-fed output archive configuration is not from its signed commit: $run" >&2
    exit 1
}

identity_pattern='^sophia_frame_fed_output_gate schema=1 status=phase_started phase=(success|rollback) source_commit=[0-9a-f]{40} hagia_commit=[0-9a-f]{40} sophia_sha256=[0-9a-f]{64} hagia_sha256=[0-9a-f]{64} core_sha256=[0-9a-f]{64} profile_sha256=[0-9a-f]{64} connectors_sha256=[0-9a-f]{64}$'
for phase in success rollback; do
    log="$run/$phase.log"
    identity="$(grep -E "$identity_pattern" "$log")"
    [[ "$(grep -Ec "$identity_pattern" "$log")" == 1 ]] || {
        echo "frame-fed output archive has invalid $phase identity cardinality: $run" >&2
        exit 1
    }
    for mapping in \
        "source_commit:source_commit" \
        "hagia_commit:hagia_commit" \
        "sophia_sha256:sophia_binary_sha256" \
        "hagia_sha256:hagia_binary_sha256" \
        "core_sha256:core_config_sha256" \
        "profile_sha256:desktop_profile_sha256" \
        "connectors_sha256:connectors_sha256"; do
        evidence_key="${mapping%%:*}"
        manifest_key="${mapping#*:}"
        evidence_value="$(sed -n "s/.* ${evidence_key}=\([^ ]*\).*/\1/p" <<<"$identity")"
        [[ "$evidence_value" == "$(manifest_value "$manifest_key")" ]] || {
            echo "frame-fed output $phase evidence and manifest disagree on $evidence_key: $run" >&2
            exit 1
        }
    done
done

"$ROOT_DIR/tools/verify_frame_fed_output_evidence.sh" \
    "$run/success.log" "$run/rollback.log" \
    "$(manifest_value success_text)" "$(manifest_value rollback_text)" >/dev/null

echo "frame-fed output physical archive verified: run=$run commit=$source_commit hagia=$hagia_commit"
