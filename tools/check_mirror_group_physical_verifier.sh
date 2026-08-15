#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="$ROOT_DIR/tools/fixtures/mirror_group_physical_pass.log"
work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT

"$ROOT_DIR/tools/verify_mirror_group_physical.sh" "$fixture" >/dev/null

runner="$ROOT_DIR/tools/run_mirror_group_gate_tty4.sh"
mapfile -t clean_lines < <(
    grep -nF 'status --porcelain --untracked-files=all' "$runner" | cut -d: -f1
)
mapfile -t signature_lines < <(
    grep -nF 'verify-commit "$source_commit"' "$runner" | cut -d: -f1
)
build_line="$(grep -nFm1 'echo "Building..."' "$runner" | cut -d: -f1)"
(( ${#clean_lines[@]} >= 2 && ${#signature_lines[@]} >= 2 )) \
    && [[ -n "$build_line" ]] \
    && (( clean_lines[0] < build_line \
        && signature_lines[0] < build_line \
        && clean_lines[${#clean_lines[@]} - 1] > build_line \
        && signature_lines[${#signature_lines[@]} - 1] > build_line )) \
    && grep -Fq 'rev-parse HEAD)" != "$source_commit"' "$runner" || {
    echo "mirror-group runner does not preserve clean, signed source identity across the build" >&2
    exit 1
}
grep -Fq 'while [ "$i" -le 40 ]' "$runner" \
    && grep -Fq 'printf "Sophia Mirror AaZz 0123456789\n"; exec sh -i' "$runner" || {
    echo "mirror-group runner omitted the deterministic scrolling text workload" >&2
    exit 1
}
grep -Fq -- '--session-mode=normal' "$runner" \
    && grep -Fq -- '--session-app=terminal="$XTERM_BIN"' "$runner" \
    && grep -Fq -- '--session-start=terminal' "$runner" || {
    echo "mirror-group runner passes terminal arguments without declaring its startup application" >&2
    exit 1
}
if grep -Fq -- '--no-config' "$runner"; then
    echo "mirror-group runner combines mutually exclusive no-config and desktop-profile sources" >&2
    exit 1
fi

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
reject_mutation '/sophia_mirror_group_visual/d' 'missing readable-text confirmation'
reject_mutation '/sophia_live_session_cleanup/d' 'missing clean frontend/session teardown'
reject_mutation '/sophia_live_native_startup_output/d' 'missing logical startup-output proof'
reject_mutation '/status=direct_cpu output=1 connector_id=102/d' 'missing direct-CPU mirror bootstrap'
reject_mutation 's/worker_failures=0/worker_failures=1/' 'a failed mirror renderer worker'
reject_mutation 's/connector_id=102 checksum=111/connector_id=102 checksum=222/' 'divergent mirror checksums'
reject_mutation 's/cpu_checksum=111/cpu_checksum=222/' 'native heads stale behind the final CPU scene'
reject_mutation 's/source=cpu checksum=111/source=retained_mixed checksum=111/' 'focus-only retained content as terminal evidence'
reject_mutation '/sophia_live_mirror_generation schema=1 status=presented/d' 'missing logical CPU-generation presentation'
reject_mutation '/sophia_live_mirror_head_damage.*connector_id=102/d' 'missing secondary projected damage'
reject_mutation 's/connector_id=102 frame=7 width=1920/connector_id=102 frame=8 width=1920/' 'damage from a different logical generation'
reject_mutation 's/connector_id=102 frame=7 width=1920 height=1080/connector_id=102 frame=7 width=2560 height=1440/' 'damage in the wrong physical coordinate space'
reject_mutation 's/connector_id=102 frame=7 width=1920 height=1080 mode=full rects=1/connector_id=102 frame=7 width=1920 height=1080 mode=full rects=0/' 'empty projected damage'

sed $'s/^sophia_live_native_head_page_flip /\033[2m2026-08-15T12:57:29Z\033[0m INFO native_scanout: \033[0m sophia_live_native_head_page_flip /; s/^sophia_live_mirror_head_damage /\033[2m2026-08-15T12:57:29Z\033[0m INFO native_scanout: \033[0m sophia_live_mirror_head_damage /; s/^sophia_live_mirror_generation /\033[2m2026-08-15T12:57:29Z\033[0m INFO native_scanout: \033[0m sophia_live_mirror_generation /' \
    "$fixture" >"$work/tracing-prefixed.log"
"$ROOT_DIR/tools/verify_mirror_group_physical.sh" "$work/tracing-prefixed.log" >/dev/null

cp "$fixture" "$work/rejected.log"
printf '%s\n' \
    'sophia_live_output_damage schema=1 status=queue_rejected output=1 reason=OutputMismatch' \
    >>"$work/rejected.log"
if "$ROOT_DIR/tools/verify_mirror_group_physical.sh" "$work/rejected.log" >/dev/null 2>&1; then
    echo "mirror-group verifier accepted OutputMismatch damage" >&2
    exit 1
fi

sed '/sophia_mirror_group_gate schema=1 status=passed exit=0/d' \
    "$fixture" >"$work/candidate.log"
"$ROOT_DIR/tools/verify_mirror_group_physical.sh" --candidate "$work/candidate.log" >/dev/null
if "$ROOT_DIR/tools/verify_mirror_group_physical.sh" "$work/candidate.log" >/dev/null 2>&1; then
    echo "mirror-group promotion verifier accepted an unpassed candidate" >&2
    exit 1
fi
if "$ROOT_DIR/tools/verify_mirror_group_physical.sh" --candidate "$fixture" >/dev/null 2>&1; then
    echo "mirror-group candidate verifier accepted pre-recorded promotion evidence" >&2
    exit 1
fi
cp "$work/candidate.log" "$work/rejected.log"
printf '%s\n' 'sophia_mirror_group_gate schema=1 status=passed exit=1' >>"$work/rejected.log"
if "$ROOT_DIR/tools/verify_mirror_group_physical.sh" --candidate "$work/rejected.log" >/dev/null 2>&1; then
    echo "mirror-group candidate verifier accepted a malformed early pass marker" >&2
    exit 1
fi

for failure in \
    'sophia_live_session_client_fatal schema=1 status=detected source=primary' \
    'sophia_x11_authority_backpressure schema=1 status=shutdown client=1 client_known=true transaction=8 waited_msec=2 failure=cancelled' \
    'sophia_x11_authority_backpressure schema=1 status=transport_failure client=1 client_known=true transaction=9 waited_msec=1 failure=disconnected' \
    'X authority observed transaction channel is full for transaction 9' \
    'xterm: fatal IO error 11 (Resource temporarily unavailable) or KillClient on X server ":191"'; do
    cp "$fixture" "$work/rejected.log"
    printf '%s\n' "$failure" >>"$work/rejected.log"
    if "$ROOT_DIR/tools/verify_mirror_group_physical.sh" "$work/rejected.log" >/dev/null 2>&1; then
        echo "mirror-group verifier accepted an X11 authority/client failure" >&2
        exit 1
    fi
done

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

awk '
    !inserted && /status=submitted output=1 connector_id=94 .* frame=7$/ {
        print "sophia_live_native_head_page_flip schema=1 status=submitted output=1 connector_id=94 submission=2 content=Cpu frame=6"
        print "sophia_live_native_head_page_flip schema=1 status=submitted output=1 connector_id=102 submission=2 content=Cpu frame=6"
        print "sophia_live_native_head_page_flip schema=1 status=callback_accepted output=1 connector_id=94 callbacks=1 kernel_sequence=70 frame=6"
        print "sophia_live_native_head_page_flip schema=1 status=retired output=1 connector_id=94 submission=2 frame=6"
        print "sophia_live_mirror_head_damage schema=1 status=presented output=1 connector_id=94 frame=6 width=2560 height=1440 mode=skip rects=0 pixels=0"
        print "sophia_live_native_head_page_flip schema=1 status=callback_accepted output=1 connector_id=102 callbacks=1 kernel_sequence=71 frame=6"
        print "sophia_live_native_head_page_flip schema=1 status=retired output=1 connector_id=102 submission=2 frame=6"
        print "sophia_live_mirror_head_damage schema=1 status=presented output=1 connector_id=102 frame=6 width=1920 height=1080 mode=skip rects=0 pixels=0"
        inserted = 1
    }
    { print }
' "$fixture" >"$work/earlier-skip.log"
"$ROOT_DIR/tools/verify_mirror_group_physical.sh" "$work/earlier-skip.log" >/dev/null || {
    echo "mirror-group verifier stopped at an earlier causal skip frame" >&2
    exit 1
}

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
real_git="$(command -v git)"
mkdir "$work/fake-bin"
cat >"$work/fake-bin/git" <<EOF
#!/usr/bin/env bash
if [[ " \$* " == *" verify-commit "* ]]; then
    exit 1
fi
exec "$real_git" "\$@"
EOF
chmod 755 "$work/fake-bin/git"
if env PATH="$work/fake-bin:$PATH" \
    "$ROOT_DIR/tools/verify_mirror_group_physical_archive.sh" "$run_dir" >/dev/null 2>&1; then
    echo "mirror-group archive verifier accepted an unverifiable commit signature" >&2
    exit 1
fi
if env \
    PATH="$work/fake-bin:$PATH" \
    XDG_STATE_HOME="$work/unsigned-state" \
    SOPHIA_MIRROR_SOPHIA_BIN="$work/sophia" \
    SOPHIA_MIRROR_PROFILE="$work/profile.kdl" \
    "$ROOT_DIR/tools/archive_mirror_group_physical_run.sh" "$work/archive.log" >/dev/null 2>&1; then
    echo "mirror-group promotion archive accepted an unverifiable commit signature" >&2
    exit 1
fi
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

sed '/sophia_mirror_group_gate schema=1 status=passed exit=0/d' \
    "$work/archive.log" >"$work/verification-diagnostic.log"
cat >>"$work/verification-diagnostic.log" <<EOF
sophia_mirror_group_kernel schema=1 status=captured availability=unavailable continuity=unknown lines=0 total_lines=0 truncated=false
sophia_mirror_group_gate schema=1 status=failed stage=verification exit=1 signal=0 kernel_capture=unavailable
EOF
"$ROOT_DIR/tools/verify_mirror_group_diagnostic.sh" \
    "$work/verification-diagnostic.log" "$work/kernel-unavailable.log" >/dev/null
verification_archive="$(env \
    XDG_STATE_HOME="$work/verification-state" \
    SOPHIA_MIRROR_SOPHIA_BIN="$work/sophia" \
    SOPHIA_MIRROR_PROFILE="$work/profile.kdl" \
    "$ROOT_DIR/tools/archive_mirror_group_diagnostic_run.sh" \
    "$work/verification-diagnostic.log" "$work/kernel-unavailable.log")"
verification_dir="${verification_archive##*: }"
[[ "$verification_dir" == "$work/verification-state/sophia/diagnostics/mirror-group-runs/"* ]] || {
    echo "verification failure did not enter the diagnostic archive" >&2
    exit 1
}
"$ROOT_DIR/tools/verify_mirror_group_diagnostic_archive.sh" "$verification_dir" >/dev/null

echo "mirror-group physical verifier checks passed"
