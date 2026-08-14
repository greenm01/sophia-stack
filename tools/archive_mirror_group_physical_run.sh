#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
evidence="${1:?usage: archive_mirror_group_physical_run.sh EVIDENCE}"
state_home="${XDG_STATE_HOME:-$HOME/.local/state}"
run_root="${SOPHIA_MIRROR_RUN_ROOT:-$state_home/sophia/promotion/mirror-group-runs}"
sophia_bin="${SOPHIA_MIRROR_SOPHIA_BIN:-$ROOT_DIR/target/release/sophia}"
profile="${SOPHIA_MIRROR_PROFILE:-$ROOT_DIR/tools/fixtures/mirror_group_probe.kdl}"

"$ROOT_DIR/tools/verify_mirror_group_physical.sh" "$evidence" >/dev/null
identity="$(grep -E '^sophia_mirror_group_gate schema=1 status=starting ' "$evidence")"
source_commit="$(sed -n 's/.* source_commit=\([0-9a-f]\{40\}\) .*/\1/p' <<<"$identity")"
recorded_sophia_sha256="$(sed -n 's/.* sophia_sha256=\([0-9a-f]\{64\}\) .*/\1/p' <<<"$identity")"
recorded_profile_sha256="$(sed -n 's/.* profile_sha256=\([0-9a-f]\{64\}\)$/\1/p' <<<"$identity")"
[[ -n "$source_commit" ]] && git -C "$ROOT_DIR" cat-file -e "$source_commit^{commit}" || {
    echo "mirror-group evidence has an invalid source commit" >&2
    exit 1
}
for file in "$sophia_bin" "$profile"; do
    [[ -r "$file" ]] || { echo "mirror-group archive input is unavailable: $file" >&2; exit 1; }
done
[[ -x "$sophia_bin" ]] || { echo "Sophia mirror-group binary is not executable" >&2; exit 1; }
sophia_sha256="$(sha256sum "$sophia_bin" | awk '{ print $1 }')"
profile_sha256="$(sha256sum "$profile" | awk '{ print $1 }')"
[[ "$sophia_sha256" == "$recorded_sophia_sha256" ]] || {
    echo "mirror-group binary no longer matches the verified run" >&2
    exit 1
}
[[ "$profile_sha256" == "$recorded_profile_sha256" ]] || {
    echo "mirror-group profile no longer matches the verified run" >&2
    exit 1
}

evidence_sha256="$(sha256sum "$evidence" | awk '{ print $1 }')"
install -d -m 700 "$run_root"
if grep -rlFx --include=manifest "evidence_sha256=$evidence_sha256" "$run_root" 2>/dev/null | grep -q .; then
    echo "mirror-group physical evidence is already archived" >&2
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
install -m 600 "$profile" "$run_dir/profile.kdl"
printf '%s\n' 'sophia_mirror_group_physical schema=1 status=passed' >"$run_dir/result.kdl"
printf 'record_schema=1\nrecord_kind=mirror_group_physical\nrecorded_at_utc=%s\nsource_commit=%s\nevidence_sha256=%s\nsophia_binary_sha256=%s\nprofile_sha256=%s\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$source_commit" "$evidence_sha256" \
    "$sophia_sha256" "$profile_sha256" >"$run_dir/manifest"
chmod 600 "$run_dir/manifest" "$run_dir/result.kdl"
(
    cd "$run_dir"
    sha256sum manifest profile.kdl result.kdl session.log >SHA256SUMS
)
chmod 600 "$run_dir/SHA256SUMS"
(
    cd "$run_dir"
    sha256sum -c --status SHA256SUMS
)
"$ROOT_DIR/tools/verify_mirror_group_physical_archive.sh" "$run_dir" >/dev/null
trap - ERR HUP INT TERM
echo "Recorded verified mirror-group physical run: $run_dir"
