#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
evidence="${1:?usage: archive_mirror_group_diagnostic_run.sh EVIDENCE KERNEL_DELTA}"
kernel_delta="${2:?usage: archive_mirror_group_diagnostic_run.sh EVIDENCE KERNEL_DELTA}"
state_home="${XDG_STATE_HOME:-$HOME/.local/state}"
run_root="${SOPHIA_MIRROR_DIAGNOSTIC_ROOT:-$state_home/sophia/diagnostics/mirror-group-runs}"
sophia_bin="${SOPHIA_MIRROR_SOPHIA_BIN:-$ROOT_DIR/target/release/sophia}"
profile="${SOPHIA_MIRROR_PROFILE:-$ROOT_DIR/tools/fixtures/mirror_group_probe.kdl}"

"$ROOT_DIR/tools/verify_mirror_group_diagnostic.sh" "$evidence" "$kernel_delta" >/dev/null
identity="$(grep -E '^sophia_mirror_group_gate schema=1 status=starting ' "$evidence")"
failure="$(grep -E '^sophia_mirror_group_gate schema=1 status=failed ' "$evidence")"
source_commit="$(sed -n 's/.* source_commit=\([0-9a-f]\{40\}\) .*/\1/p' <<<"$identity")"
recorded_sophia_sha256="$(sed -n 's/.* sophia_sha256=\([0-9a-f]\{64\}\) .*/\1/p' <<<"$identity")"
recorded_profile_sha256="$(sed -n 's/.* profile_sha256=\([0-9a-f]\{64\}\)$/\1/p' <<<"$identity")"
stage="$(sed -n 's/.* stage=\([^ ]*\) .*/\1/p' <<<"$failure")"
exit_status="$(sed -n 's/.* exit=\([0-9]*\) .*/\1/p' <<<"$failure")"
signal="$(sed -n 's/.* signal=\([0-9]*\) .*/\1/p' <<<"$failure")"
kernel_capture="$(sed -n 's/.* kernel_capture=\([^ ]*\)$/\1/p' <<<"$failure")"
git -C "$ROOT_DIR" cat-file -e "$source_commit^{commit}" || {
    echo "mirror-group diagnostic has an invalid source commit" >&2
    exit 1
}
for file in "$sophia_bin" "$profile"; do
    [[ -r "$file" ]] || { echo "mirror-group diagnostic archive input is unavailable: $file" >&2; exit 1; }
done
[[ -x "$sophia_bin" ]] || { echo "Sophia mirror-group binary is not executable" >&2; exit 1; }
sophia_sha256="$(sha256sum "$sophia_bin" | awk '{ print $1 }')"
profile_sha256="$(sha256sum "$profile" | awk '{ print $1 }')"
[[ "$sophia_sha256" == "$recorded_sophia_sha256" ]] || {
    echo "mirror-group binary no longer matches the failed run" >&2
    exit 1
}
[[ "$profile_sha256" == "$recorded_profile_sha256" ]] || {
    echo "mirror-group profile no longer matches the failed run" >&2
    exit 1
}

evidence_sha256="$(sha256sum "$evidence" | awk '{ print $1 }')"
kernel_delta_sha256="$(sha256sum "$kernel_delta" | awk '{ print $1 }')"
install -d -m 700 "$run_root"
if grep -rlFx --include=manifest "evidence_sha256=$evidence_sha256" "$run_root" 2>/dev/null | grep -q .; then
    echo "mirror-group failure diagnostic is already archived" >&2
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
install -m 600 "$kernel_delta" "$run_dir/kernel-delta.log"
printf 'sophia_mirror_group_diagnostic schema=1 status=failed stage=%s exit=%s signal=%s kernel_capture=%s\n' \
    "$stage" "$exit_status" "$signal" "$kernel_capture" >"$run_dir/result.kdl"
printf 'record_schema=1\nrecord_kind=mirror_group_diagnostic\nrecorded_at_utc=%s\nsource_commit=%s\nevidence_sha256=%s\nsophia_binary_sha256=%s\nprofile_sha256=%s\nkernel_delta_sha256=%s\nstage=%s\nexit=%s\nsignal=%s\nkernel_capture=%s\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$source_commit" "$evidence_sha256" \
    "$sophia_sha256" "$profile_sha256" "$kernel_delta_sha256" "$stage" \
    "$exit_status" "$signal" "$kernel_capture" >"$run_dir/manifest"
chmod 600 "$run_dir/manifest" "$run_dir/result.kdl"
(
    cd "$run_dir"
    sha256sum kernel-delta.log manifest profile.kdl result.kdl session.log >SHA256SUMS
)
chmod 600 "$run_dir/SHA256SUMS"
"$ROOT_DIR/tools/verify_mirror_group_diagnostic_archive.sh" "$run_dir" >/dev/null
trap - ERR HUP INT TERM
echo "Recorded mirror-group failure diagnostic: $run_dir"
