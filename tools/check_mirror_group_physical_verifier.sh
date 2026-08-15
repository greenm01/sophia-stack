#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="$ROOT_DIR/tools/fixtures/mirror_group_physical_pass.log"
work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT

"$ROOT_DIR/tools/verify_mirror_group_physical.sh" "$fixture" >/dev/null

reject_mutation() {
    local expression="$1" description="$2"
    cp "$fixture" "$work/rejected.log"
    sed -i "$expression" "$work/rejected.log"
    if "$ROOT_DIR/tools/verify_mirror_group_physical.sh" "$work/rejected.log" >/dev/null 2>&1; then
        echo "mirror-group verifier accepted $description" >&2
        exit 1
    fi
}

reject_mutation '/connector=DP-2/d' 'a missing mirror head'
reject_mutation 's/mode=1920x1080/mode=2560x1440/' 'a downgraded secondary mode'
reject_mutation 's/native_cleanup_pending=false/native_cleanup_pending=true/' 'undrained native ownership'
reject_mutation 's/outcome=drained drained=true abandoned_scanouts=0/outcome=forced_detach_timeout drained=false abandoned_scanouts=1/' 'forced native detach'
reject_mutation '/status=visual_confirmed/d' 'missing visible-pixel confirmation'
reject_mutation '/sophia_live_native_startup_output/d' 'missing logical startup-output proof'
reject_mutation '/status=direct_cpu output=1 connector_id=102/d' 'missing direct-CPU mirror bootstrap'
reject_mutation 's/worker_failures=0/worker_failures=1/' 'a failed mirror renderer worker'

sed '/sophia_live_native_startup_output/p' "$fixture" >"$work/duplicate-startup-output.log"
if "$ROOT_DIR/tools/verify_mirror_group_physical.sh" "$work/duplicate-startup-output.log" >/dev/null 2>&1; then
    echo "mirror-group verifier accepted duplicate logical startup-output proof" >&2
    exit 1
fi

awk '
    /status=submitted output=1 connector_id=94 / { submitted = $0; next }
    /status=callback_accepted output=1 connector_id=94 / { print $0; print submitted; next }
    { print }
' "$fixture" >"$work/reordered.log"
if "$ROOT_DIR/tools/verify_mirror_group_physical.sh" "$work/reordered.log" >/dev/null 2>&1; then
    echo "mirror-group verifier accepted callback evidence before submission" >&2
    exit 1
fi

printf '#!/usr/bin/env bash\nexit 0\n' >"$work/sophia"
printf 'schema 1\n' >"$work/profile.kdl"
chmod 755 "$work/sophia"
commit="$(git -C "$ROOT_DIR" rev-parse HEAD)"
sophia_sha256="$(sha256sum "$work/sophia" | awk '{ print $1 }')"
profile_sha256="$(sha256sum "$work/profile.kdl" | awk '{ print $1 }')"
sed \
    -e "s/source_commit=[0-9a-f]\{40\}/source_commit=$commit/" \
    -e "s/sophia_sha256=[0-9a-f]\{64\}/sophia_sha256=$sophia_sha256/" \
    -e "s/profile_sha256=[0-9a-f]\{64\}/profile_sha256=$profile_sha256/" \
    "$fixture" >"$work/archive.log"
archive="$(env \
    XDG_STATE_HOME="$work/state" \
    SOPHIA_MIRROR_SOPHIA_BIN="$work/sophia" \
    SOPHIA_MIRROR_PROFILE="$work/profile.kdl" \
    "$ROOT_DIR/tools/archive_mirror_group_physical_run.sh" "$work/archive.log")"
run_dir="${archive##*: }"
[[ -s "$run_dir/SHA256SUMS" ]] || {
    echo "mirror-group archive was not created" >&2
    exit 1
}
[[ "$run_dir" == "$work/state/sophia/promotion/mirror-group-runs/"* ]] || {
    echo "passing mirror-group evidence escaped the promotion archive" >&2
    exit 1
}
"$ROOT_DIR/tools/verify_mirror_group_physical_archive.sh" "$run_dir" >/dev/null
printf '\n' >>"$run_dir/session.log"
if "$ROOT_DIR/tools/verify_mirror_group_physical_archive.sh" "$run_dir" >/dev/null 2>&1; then
    echo "mirror-group archive accepted tampered evidence" >&2
    exit 1
fi

printf '%s\n' before-one before-two >"$work/kernel-before.log"
printf '%s\n' before-one before-two after-one after-two after-three >"$work/kernel-after.log"
kernel_summary="$("$ROOT_DIR/tools/collect_mirror_group_kernel_delta.sh" \
    "$work/kernel-before.log" "$work/kernel-after.log" "$work/kernel-delta.log" 2)"
[[ "$kernel_summary" == \
    'availability=available continuity=append lines=2 total_lines=3 truncated=true' ]] || {
    echo "mirror-group kernel delta reported the wrong append summary" >&2
    exit 1
}
[[ "$(cat "$work/kernel-delta.log")" == $'after-two\nafter-three' ]] || {
    echo "mirror-group kernel delta did not retain the bounded newest lines" >&2
    exit 1
}

printf '%s\n' reset-one reset-two reset-three >"$work/kernel-reset.log"
kernel_summary="$("$ROOT_DIR/tools/collect_mirror_group_kernel_delta.sh" \
    "$work/kernel-before.log" "$work/kernel-reset.log" "$work/kernel-delta.log" 2)"
[[ "$kernel_summary" == \
    'availability=available continuity=reset lines=2 total_lines=3 truncated=true' ]] || {
    echo "mirror-group kernel delta reported the wrong reset summary" >&2
    exit 1
}

cat >"$work/diagnostic.log" <<EOF
sophia_mirror_group_gate schema=1 status=starting source_commit=$commit sophia_sha256=$sophia_sha256 profile_sha256=$profile_sha256
sophia_live_native_page_flip connector=DP-1 status=submitted
amdgpu: The CS has been rejected, see dmesg for more information (-22).
sophia_mirror_group_kernel schema=1 status=captured availability=available continuity=append lines=2 total_lines=3 truncated=true
sophia_mirror_group_gate schema=1 status=failed stage=runtime exit=134 signal=6 kernel_capture=available
EOF
"$ROOT_DIR/tools/verify_mirror_group_diagnostic.sh" \
    "$work/diagnostic.log" "$work/kernel-delta.log" >/dev/null
diagnostic_archive="$(env \
    XDG_STATE_HOME="$work/state" \
    SOPHIA_MIRROR_SOPHIA_BIN="$work/sophia" \
    SOPHIA_MIRROR_PROFILE="$work/profile.kdl" \
    "$ROOT_DIR/tools/archive_mirror_group_diagnostic_run.sh" \
    "$work/diagnostic.log" "$work/kernel-delta.log")"
diagnostic_dir="${diagnostic_archive##*: }"
[[ "$diagnostic_dir" == "$work/state/sophia/diagnostics/mirror-group-runs/"* ]] || {
    echo "failed mirror-group evidence entered the promotion archive" >&2
    exit 1
}
"$ROOT_DIR/tools/verify_mirror_group_diagnostic_archive.sh" "$diagnostic_dir" >/dev/null
printf '\n' >>"$diagnostic_dir/kernel-delta.log"
if "$ROOT_DIR/tools/verify_mirror_group_diagnostic_archive.sh" "$diagnostic_dir" >/dev/null 2>&1; then
    echo "mirror-group diagnostic archive accepted a tampered kernel delta" >&2
    exit 1
fi

sed 's/exit=134 signal=6/exit=134 signal=0/' \
    "$work/diagnostic.log" >"$work/rejected-diagnostic.log"
if "$ROOT_DIR/tools/verify_mirror_group_diagnostic.sh" \
    "$work/rejected-diagnostic.log" "$work/kernel-delta.log" >/dev/null 2>&1; then
    echo "mirror-group diagnostic verifier accepted a mismatched signal" >&2
    exit 1
fi

: >"$work/kernel-unavailable.log"
sed \
    -e 's/availability=available continuity=append lines=2 total_lines=3 truncated=true/availability=unavailable continuity=unknown lines=0 total_lines=0 truncated=false/' \
    -e 's/stage=runtime exit=134 signal=6 kernel_capture=available/stage=visual_confirmation exit=1 signal=0 kernel_capture=unavailable/' \
    "$work/diagnostic.log" >"$work/unavailable-diagnostic.log"
"$ROOT_DIR/tools/verify_mirror_group_diagnostic.sh" \
    "$work/unavailable-diagnostic.log" "$work/kernel-unavailable.log" >/dev/null

echo "mirror-group physical verifier checks passed"
