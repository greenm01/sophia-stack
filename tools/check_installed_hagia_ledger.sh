#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="$(mktemp -d)"
trap 'rm -rf -- "$fixture"' EXIT
prefix="$fixture/prefix"
release="$prefix/releases/test"
state="$fixture/state"
session="$state/sophia/hagia-session"
identity_dir="$state/sophia/installed-session"
install -d -m 700 "$release/target/release" "$session" "$identity_dir"
printf '#!/usr/bin/env bash\nexit 0\n' >"$release/target/release/sophia"
printf '#!/usr/bin/env bash\nexit 0\n' >"$release/target/release/hagia"
chmod 755 "$release/target/release/sophia" "$release/target/release/hagia"
sophia_digest="$(sha256sum "$release/target/release/sophia" | awk '{print $1}')"
hagia_digest="$(sha256sum "$release/target/release/hagia" | awk '{print $1}')"
printf 'schema=3\nversion=0.1.0\ncommit=0123456789abcdef\nrelease_id=test\nhagia_included=true\nhagia_binary_sha256=%s\n' \
    "$hagia_digest" >"$release/manifest"
(
    cd "$release"
    sha256sum target/release/sophia target/release/hagia >SHA256SUMS
)
ln -s releases/test "$prefix/current"

write_identity() {
    local launch_id="$1"
    printf 'sophia_installed_session schema=1 status=starting profile=hagia version=0.1.0 commit=0123456789abcdef release=%s started_at_utc=2026-08-09T12:00:00Z launch_id=%s\n' \
        "$release" "$launch_id" >"$identity_dir/launch.log"
    {
        printf 'sophia_runtime_identity schema=2 kind=system kernel=test mesa=test\n'
        printf 'sophia_runtime_identity schema=2 kind=application name=sophia version=0.1.0 digest=%s\n' "$sophia_digest"
        for application in kitty firefox xmonad xmobar; do
            printf 'sophia_runtime_identity schema=2 kind=application name=%s version=test digest=unavailable\n' "$application"
        done
        printf 'sophia_runtime_identity schema=2 kind=application name=hagia version=packaged digest=%s\n' "$hagia_digest"
        printf 'sophia_runtime_identity schema=2 kind=input seat=seat0 names_sha256=%064d\n' 0
        printf 'sophia_runtime_identity schema=2 kind=output connector=card0-test status=connected edid_sha256=%064d\n' 0
    } >"$identity_dir/runtime-identity.log"
}

write_lifecycle() {
    local exit_status="$1" emergency="$2"
    {
        for marker in 'entering preflight' 'complete preflight' \
            'entering input_guard' 'complete input_guard' \
            'entering graphics_takeover' 'complete graphics_takeover' \
            'entering session'; do
            set -- $marker
            printf 'sophia_session_lifecycle schema=1 status=%s phase=%s installed=true build=false manual_service=false runtime=owner vt=local\n' "$1" "$2"
        done
        printf 'sophia_session_lifecycle schema=1 status=returned phase=handoff installed=true exit_status=%s emergency=%s handoff=display_manager\n' \
            "$exit_status" "$emergency"
    } >"$session/lifecycle.log"
}

write_normal_session() {
    {
        printf 'sophia_session_app schema=1 status=started id=terminal source=startup\n'
        printf 'sophia_live_session_startup schema=2 status=ready outputs_ready=1/1 elapsed_msec=10\n'
        printf 'sophia_live_wm schema=1 status=physical_action_committed action=33\n'
        printf 'sophia_live_wm schema=1 status=session_action_committed serial=1 action=Logout\n'
        printf 'hagia_policy_checkpoint schema=1 status=saved candidate_nonempty=true\n'
        printf 'sophia_live_session_health schema=1 status=clean pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false\n'
        printf 'sophia_live_layout_health schema=2 status=clean pending=0\n'
        printf 'sophia_live_session_protocol_errors schema=1 expected=0 unexpected=0\n'
        printf 'sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=none\n'
        printf 'sophia_live_session_cleanup schema=1 status=clean app_groups=0\n'
        printf 'sophia_live_session schema=16 status=bounded_complete physical_input=enabled wm_policy=external wm_degraded=false native_submit_failures=0 native_retire_failures=0 native_callback_rejected=0 native_in_flight=false native_cleanup_pending=false\n'
    } >"$session/session.log"
    printf 'sophia_session_input_guard schema=1 status=armed\n' >"$session/input-guard.log"
    printf 'sophia_tty_recovery schema=3 profile=hagia kd_mode_before=text kd_mode_after=text termios_restored=true emergency=false session_shutdown=not_requested session_exit_status=none\n' >"$session/recovery.log"
    write_lifecycle 0 false
}

record=(env XDG_STATE_HOME="$state" SOPHIA_INSTALL_PREFIX="$prefix" \
    SOPHIA_VERIFY_HAGIA_SESSION_BIN="$ROOT_DIR/tools/verify_installed_hagia_session.sh" \
    SOPHIA_VERIFY_HAGIA_RECOVERY_BIN="$ROOT_DIR/tools/verify_installed_hagia_recovery.sh" \
    SOPHIA_VERIFY_IDENTITY_BIN="$ROOT_DIR/tools/verify_installed_runtime_identity.sh" \
    SOPHIA_VERIFY_LIFECYCLE_BIN="$ROOT_DIR/tools/verify_installed_session_lifecycle.sh" \
    "$ROOT_DIR/tools/record_installed_hagia_run.sh")

write_identity clean
write_normal_session
clean_run="$("${record[@]}" begin)"
[[ "$(grep -c '^hagia_binary_sha256=' "$clean_run/manifest")" == 1 ]]
grep -Fxq "hagia_binary_sha256=$hagia_digest" "$clean_run/manifest"
"${record[@]}" finish "$clean_run" 0
grep -Fxq 'sophia_installed_hagia schema=1 status=passed exit_status=0' "$clean_run/result.kdl"
grep -Eq '^sophia_hagia_coverage schema=1 .*physical_actions=1 .*checkpoints=1 ' "$clean_run/coverage.kdl"
env XDG_STATE_HOME="$state" SOPHIA_HAGIA_RUN_ROOT="$state/sophia/promotion/hagia-runs" \
    SOPHIA_VERIFY_IDENTITY_BIN="$ROOT_DIR/tools/verify_installed_runtime_identity.sh" \
    SOPHIA_VERIFY_LIFECYCLE_BIN="$ROOT_DIR/tools/verify_installed_session_lifecycle.sh" \
    "$ROOT_DIR/tools/verify_installed_hagia_archive.sh" "$clean_run"

write_identity recovery
write_normal_session
printf 'sophia_session_input_guard schema=1 status=armed\nsophia_session_input_guard schema=1 status=triggered\n' >"$session/input-guard.log"
printf 'sophia_tty_recovery schema=3 profile=hagia kd_mode_before=text kd_mode_after=text termios_restored=true emergency=true session_shutdown=graceful session_exit_status=0\n' >"$session/recovery.log"
write_lifecycle 130 true
recovery_run="$("${record[@]}" begin)"
"${record[@]}" finish "$recovery_run" 130
grep -Fxq 'sophia_installed_hagia schema=1 status=recovered exit_status=130' "$recovery_run/result.kdl"

write_identity failure
write_normal_session
failed_run="$("${record[@]}" begin)"
if "${record[@]}" finish "$failed_run" 1; then
    echo "Hagia ledger accepted an unexpected session exit" >&2
    exit 1
fi
grep -Fxq 'sophia_installed_hagia schema=1 status=failed exit_status=1 reason=session_exit' "$failed_run/result.kdl"

cp "$release/manifest" "$fixture/release-manifest"
sed -i 's/^hagia_binary_sha256=.*/hagia_binary_sha256=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/' \
    "$release/manifest"
write_identity mismatched-auxiliary
write_normal_session
run_count="$(find "$state/sophia/promotion/hagia-runs" -mindepth 1 -maxdepth 1 -type d | wc -l)"
if "${record[@]}" begin >/dev/null 2>&1; then
    echo "Hagia ledger accepted a mismatched release digest" >&2
    exit 1
fi
[[ "$(find "$state/sophia/promotion/hagia-runs" -mindepth 1 -maxdepth 1 -type d | wc -l)" == "$run_count" ]]

cp "$fixture/release-manifest" "$release/manifest"
printf 'hagia_binary_sha256=%s\n' "$hagia_digest" >>"$release/manifest"
write_identity duplicate-auxiliary
write_normal_session
if "${record[@]}" begin >/dev/null 2>&1; then
    echo "Hagia ledger accepted duplicate release digests" >&2
    exit 1
fi
[[ "$(find "$state/sophia/promotion/hagia-runs" -mindepth 1 -maxdepth 1 -type d | wc -l)" == "$run_count" ]]

grep -v '^hagia_binary_sha256=' "$fixture/release-manifest" >"$release/manifest"
write_identity appended-auxiliary
write_normal_session
appended_run="$("${record[@]}" begin)"
[[ "$(grep -c '^hagia_binary_sha256=' "$appended_run/manifest")" == 1 ]]
grep -Fxq "hagia_binary_sha256=$hagia_digest" "$appended_run/manifest"
"${record[@]}" finish "$appended_run" 0

printf '\n' >>"$clean_run/session.log"
if env XDG_STATE_HOME="$state" "$ROOT_DIR/tools/verify_installed_hagia_archive.sh" "$clean_run" >/dev/null 2>&1; then
    echo "Hagia archive verifier accepted tampered evidence" >&2
    exit 1
fi

echo "installed Hagia ledger checks passed"
