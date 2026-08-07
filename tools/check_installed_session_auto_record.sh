#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TEMP_DIR"' EXIT
STATE_HOME="$TEMP_DIR/state"
PREFIX="$TEMP_DIR/opt/sophia"
RELEASE="$PREFIX/releases/0.1.0-test"
COMMIT=2222222222222222222222222222222222222222

install -d -m 755 \
    "$RELEASE/bin" "$RELEASE/tools/lib" "$RELEASE/target/release"
printf 'sophia-test-binary\n' >"$RELEASE/target/release/sophia"
printf 'sophia-wm-demo-test-binary\n' \
    >"$RELEASE/target/release/sophia-wm-demo"
install -m 755 \
    "$ROOT_DIR/tools/installed/sophia-session" \
    "$RELEASE/bin/sophia-session"
install -m 755 \
    "$ROOT_DIR/tools/installed/sophia-kitty-session" \
    "$RELEASE/bin/sophia-kitty-session"
install -m 755 \
    "$ROOT_DIR/tools/installed/sophia-firefox-proof" \
    "$RELEASE/bin/sophia-firefox-proof"
install -m 755 \
    "$ROOT_DIR/tools/installed/sophia-xterm-proof" \
    "$RELEASE/bin/sophia-xterm-proof"
install -m 755 \
    "$ROOT_DIR/tools/installed/sophia-truecolor-proof" \
    "$RELEASE/bin/sophia-truecolor-proof"
install -m 755 \
    "$ROOT_DIR/tools/installed/sophia-recovery-proof" \
    "$RELEASE/bin/sophia-recovery-proof"
install -m 755 \
    "$ROOT_DIR/tools/record_installed_session_run.sh" \
    "$RELEASE/bin/sophia-record-run"
install -m 755 \
    "$ROOT_DIR/tools/record_installed_firefox_attempt.sh" \
    "$RELEASE/bin/sophia-record-firefox-attempt"
install -m 755 \
    "$ROOT_DIR/tools/record_installed_xterm_run.sh" \
    "$RELEASE/bin/sophia-record-xterm-run"
install -m 755 \
    "$ROOT_DIR/tools/record_installed_truecolor_run.sh" \
    "$RELEASE/bin/sophia-record-truecolor-run"
install -m 755 \
    "$ROOT_DIR/tools/record_installed_fallback_run.sh" \
    "$RELEASE/bin/sophia-record-fallback-run"
install -m 755 \
    "$ROOT_DIR/tools/record_installed_emergency_run.sh" \
    "$RELEASE/bin/sophia-record-emergency-run"
install -m 755 \
    "$ROOT_DIR/tools/record_installed_watchdog_run.sh" \
    "$RELEASE/bin/sophia-record-watchdog-run"
install -m 755 \
    "$ROOT_DIR/tools/record_installed_native_chrome_run.sh" \
    "$RELEASE/bin/sophia-record-native-chrome-run"
install -m 755 \
    "$ROOT_DIR/tools/verify_installed_login_cycle.sh" \
    "$RELEASE/bin/sophia-verify-login-cycle"
install -m 755 \
    "$ROOT_DIR/tools/verify_sophia_firefox_physical.sh" \
    "$RELEASE/bin/sophia-verify-firefox-run"
install -m 755 \
    "$ROOT_DIR/tools/verify_sophia_firefox_physical_runs.sh" \
    "$RELEASE/bin/sophia-verify-firefox-runs"
install -m 755 \
    "$ROOT_DIR/tools/verify_installed_xterm_session.sh" \
    "$RELEASE/bin/sophia-verify-xterm-run"
install -m 755 \
    "$ROOT_DIR/tools/verify_installed_xterm_runs.sh" \
    "$RELEASE/bin/sophia-verify-xterm-runs"
install -m 755 \
    "$ROOT_DIR/tools/verify_installed_truecolor_session.sh" \
    "$RELEASE/bin/sophia-verify-truecolor-run"
install -m 755 \
    "$ROOT_DIR/tools/verify_installed_truecolor_runs.sh" \
    "$RELEASE/bin/sophia-verify-truecolor-runs"
install -m 755 \
    "$ROOT_DIR/tools/verify_installed_fallback_session.sh" \
    "$RELEASE/bin/sophia-verify-fallback-session"
install -m 755 \
    "$ROOT_DIR/tools/verify_installed_fallback_run.sh" \
    "$RELEASE/bin/sophia-verify-fallback"
install -m 755 \
    "$ROOT_DIR/tools/verify_sophia_xmonad_emergency_tty3.sh" \
    "$RELEASE/bin/sophia-verify-emergency-run"
install -m 755 \
    "$ROOT_DIR/tools/verify_installed_emergency_archive.sh" \
    "$RELEASE/bin/sophia-verify-emergency"
install -m 755 \
    "$ROOT_DIR/tools/verify_installed_watchdog_recovery.sh" \
    "$RELEASE/bin/sophia-verify-watchdog-run"
install -m 755 \
    "$ROOT_DIR/tools/verify_installed_watchdog_archive.sh" \
    "$RELEASE/bin/sophia-verify-watchdog"
install -m 755 \
    "$ROOT_DIR/tools/verify_sophia_native_chrome.sh" \
    "$RELEASE/bin/sophia-verify-native-chrome-core"
install -m 755 \
    "$ROOT_DIR/tools/verify_installed_native_chrome_session.sh" \
    "$RELEASE/bin/sophia-verify-native-chrome-session"
install -m 755 \
    "$ROOT_DIR/tools/verify_installed_native_chrome_archive.sh" \
    "$RELEASE/bin/sophia-verify-native-chrome"
install -m 755 \
    "$ROOT_DIR/tools/verify_installed_runtime_identity.sh" \
    "$RELEASE/bin/sophia-verify-runtime-identity"
install -m 755 \
    "$ROOT_DIR/tools/verify_installed_session_lifecycle.sh" \
    "$RELEASE/bin/sophia-verify-lifecycle"
install -m 755 \
    "$ROOT_DIR/tools/verify_installed_session_cycles.sh" \
    "$RELEASE/bin/sophia-verify-cycles"
install -m 644 "$ROOT_DIR/tools/lib/installed_attempt_ledger.sh" \
    "$RELEASE/tools/lib/installed_attempt_ledger.sh"
install -m 644 "$ROOT_DIR/tools/lib/session_lifecycle.sh" \
    "$RELEASE/tools/lib/session_lifecycle.sh"

printf '%s\n' \
    '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    'install -m 600 "$SOPHIA_TEST_FIXTURE_ROOT/tools/fixtures/installed_runtime_identity_pass.log" "$1"' \
    >"$RELEASE/bin/capture-runtime-identity"
printf '%s\n' \
    '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    'state="${XDG_STATE_HOME}/sophia/xmonad-session"' \
    'session_fixture=physical_xmonad_hardware_smoke_session_pass.log' \
    'guard_fixture=physical_xmonad_hardware_smoke_guard_pass.log' \
    'recovery_fixture=physical_xmonad_hardware_smoke_recovery_pass.log' \
    'lifecycle_fixture=installed_lifecycle_normal_pass.log' \
    'session_status="${SOPHIA_TEST_SESSION_STATUS:-0}"' \
    'if [[ "${SOPHIA_TTY_PROFILE:-xmonad}" == kitty ]]; then' \
    '    state="${XDG_STATE_HOME}/sophia/kitty-session"' \
    '    session_fixture=installed_fallback_session_pass.log' \
    '    guard_fixture=installed_fallback_guard_pass.log' \
    '    recovery_fixture=installed_fallback_recovery_pass.log' \
    'fi' \
    'if [[ "${SOPHIA_INSTALLED_ATTEMPT_MODE:-}" == firefox ]]; then' \
    '    session_fixture=physical_firefox_session_pass.log' \
    '    guard_fixture=physical_firefox_guard_pass.log' \
    '    recovery_fixture=physical_firefox_recovery_pass.log' \
    'fi' \
    'if [[ "${SOPHIA_INSTALLED_ATTEMPT_MODE:-}" == xterm ]]; then' \
    '    session_fixture=installed_xterm_session_pass.log' \
    '    guard_fixture=physical_firefox_guard_pass.log' \
    '    recovery_fixture=physical_firefox_recovery_pass.log' \
    'fi' \
    'if [[ "${SOPHIA_INSTALLED_ATTEMPT_MODE:-}" == truecolor ]]; then' \
    '    session_fixture=installed_truecolor_session_pass.log' \
    '    guard_fixture=installed_truecolor_input_guard_pass.log' \
    '    recovery_fixture=installed_truecolor_recovery_pass.log' \
    'fi' \
    'if [[ "${SOPHIA_INSTALLED_ATTEMPT_MODE:-}" == watchdog ]]; then' \
    '    session_fixture=installed_watchdog_session_pass.log' \
    '    guard_fixture=installed_watchdog_guard_pass.log' \
    '    recovery_fixture=installed_watchdog_recovery_pass.log' \
    '    lifecycle_fixture=installed_lifecycle_watchdog_pass.log' \
    '    session_status="${SOPHIA_TEST_SESSION_STATUS:-124}"' \
    'fi' \
    'if [[ "${SOPHIA_INSTALLED_ATTEMPT_MODE:-}" == native-chrome ]]; then' \
    '    state="${XDG_STATE_HOME}/sophia/native-session"' \
    '    session_fixture=physical_native_chrome_pass.log' \
    '    guard_fixture=physical_xmonad_hardware_smoke_guard_pass.log' \
    '    recovery_fixture=installed_native_chrome_recovery_pass.log' \
    'fi' \
    'if [[ "${SOPHIA_TEST_SESSION_STATUS:-}" == 130 && "${SOPHIA_INSTALLED_ATTEMPT_MODE:-}" != watchdog ]]; then' \
    '    session_fixture=installed_emergency_session_pass.log' \
    '    guard_fixture=physical_xmonad_guard_emergency_pass.log' \
    '    recovery_fixture=physical_xmonad_recovery_emergency_pass.log' \
    '    lifecycle_fixture=installed_lifecycle_emergency_pass.log' \
    'fi' \
    'install -d -m 700 "$state"' \
    'install -m 600 "$SOPHIA_TEST_FIXTURE_ROOT/tools/fixtures/$session_fixture" "$state/session.log"' \
    'install -m 600 "$SOPHIA_TEST_FIXTURE_ROOT/tools/fixtures/$guard_fixture" "$state/input-guard.log"' \
    'install -m 600 "$SOPHIA_TEST_FIXTURE_ROOT/tools/fixtures/$recovery_fixture" "$state/recovery.log"' \
    'install -m 600 "$SOPHIA_TEST_FIXTURE_ROOT/tools/fixtures/$lifecycle_fixture" "$state/lifecycle.log"' \
    '[[ -z "${SOPHIA_TEST_RUNNER_MARKER:-}" ]] || touch "$SOPHIA_TEST_RUNNER_MARKER"' \
    'exit "$session_status"' \
    >"$RELEASE/tools/run_sophia_xmonad_session.sh"
chmod 755 \
    "$RELEASE/bin/capture-runtime-identity" \
    "$RELEASE/tools/run_sophia_xmonad_session.sh"
printf 'schema=1\nversion=0.1.0\ncommit=%s\nrelease_id=0.1.0-test\nbuilt_at_utc=2026-08-05T00:00:00Z\n' \
    "$COMMIT" >"$RELEASE/manifest"
(
    cd "$RELEASE"
    find bin tools target -type f -print0 | sort -z | xargs -0 sha256sum \
        >SHA256SUMS
)
ln -s releases/0.1.0-test "$PREFIX/current"

session_env=(
    HOME="$TEMP_DIR/home"
    XDG_STATE_HOME="$STATE_HOME"
    SOPHIA_INSTALL_PREFIX="$PREFIX"
    SOPHIA_TEST_FIXTURE_ROOT="$ROOT_DIR"
)
env "${session_env[@]}" "$RELEASE/bin/sophia-session"
grep -Fxq 'sophia_installed_cycle schema=1 status=passed exit_status=0' \
    "$STATE_HOME/sophia/promotion/runs/0001/result.kdl"
[[ -f "$STATE_HOME/sophia/installed-session/launch.log" ]]
[[ -f "$STATE_HOME/sophia/installed-session/runtime-identity.log" ]]

# The Firefox fixture intentionally fails the generic desktop verifier. A
# passing archive therefore proves that the greetd entry selected its bounded
# Firefox recorder and verifier rather than the ordinary XMonad ledger.
if "$RELEASE/bin/sophia-verify-login-cycle" \
    "$ROOT_DIR/tools/fixtures/physical_firefox_session_pass.log" \
    "$ROOT_DIR/tools/fixtures/physical_firefox_guard_pass.log" \
    "$ROOT_DIR/tools/fixtures/physical_firefox_recovery_pass.log" \
    >/dev/null 2>&1; then
    echo "generic login verifier unexpectedly accepted the Firefox fixture" >&2
    exit 1
fi
env "${session_env[@]}" "$RELEASE/bin/sophia-firefox-proof"
firefox_run="$STATE_HOME/sophia/promotion/firefox-runs/0001"
grep -Fxq \
    'sophia_installed_firefox schema=1 status=passed exit_status=0' \
    "$firefox_run/result.kdl"
grep -Fxq 'record_schema=4' "$firefox_run/manifest"
grep -Fxq 'record_kind=firefox' "$firefox_run/manifest"
env "${session_env[@]}" "$RELEASE/bin/sophia-verify-firefox-runs" 1
[[ "$(find "$STATE_HOME/sophia/promotion/runs" -mindepth 1 -maxdepth 1 \
    -type d | wc -l)" == 1 ]]

env "${session_env[@]}" "$RELEASE/bin/sophia-xterm-proof"
xterm_run="$STATE_HOME/sophia/promotion/xterm-runs/0001"
grep -Fxq \
    'sophia_installed_xterm schema=1 status=passed exit_status=0' \
    "$xterm_run/result.kdl"
grep -Fxq 'record_schema=4' "$xterm_run/manifest"
grep -Fxq 'record_kind=xterm' "$xterm_run/manifest"
env "${session_env[@]}" "$RELEASE/bin/sophia-verify-xterm-runs" 1
[[ "$(find "$STATE_HOME/sophia/promotion/runs" -mindepth 1 -maxdepth 1 \
    -type d | wc -l)" == 1 ]]
sed -i '/kind=application name=xterm /d' "$xterm_run/runtime-identity.log"
(
    cd "$xterm_run"
    sha256sum manifest result.kdl identity.log runtime-identity.log \
        session.log input-guard.log recovery.log lifecycle.log >SHA256SUMS
)
if env "${session_env[@]}" "$RELEASE/bin/sophia-verify-xterm-runs" 1 \
    >/dev/null 2>&1; then
    echo "xterm verifier accepted a checksummed archive without xterm identity" >&2
    exit 1
fi

env "${session_env[@]}" "$RELEASE/bin/sophia-truecolor-proof"
truecolor_run="$STATE_HOME/sophia/promotion/truecolor-runs/0001"
grep -Fxq \
    'sophia_installed_truecolor schema=1 status=passed exit_status=0' \
    "$truecolor_run/result.kdl"
grep -Fxq 'record_schema=4' "$truecolor_run/manifest"
grep -Fxq 'record_kind=truecolor' "$truecolor_run/manifest"
env "${session_env[@]}" "$RELEASE/bin/sophia-verify-truecolor-runs" 1
printf '%s\n' \
    'sophia_installed_truecolor schema=1 status=failed exit_status=0 reason=session_verification' \
    >"$truecolor_run/result.kdl"
(
    cd "$truecolor_run"
    sha256sum manifest result.kdl identity.log runtime-identity.log \
        session.log input-guard.log recovery.log lifecycle.log >SHA256SUMS
)
readjudication="$(
    env "${session_env[@]}" "$RELEASE/bin/sophia-verify-truecolor-runs" 1
)"
[[ "$readjudication" == *" reverified=1 "* ]] || {
    echo "TrueColor verifier did not re-adjudicate intact evidence" >&2
    exit 1
}
for rejected_result in \
    'sophia_installed_truecolor schema=1 status=failed exit_status=1 reason=session_verification' \
    'sophia_installed_truecolor schema=1 status=failed exit_status=0 reason=session_exit'; do
    printf '%s\n' "$rejected_result" >"$truecolor_run/result.kdl"
    (
        cd "$truecolor_run"
        sha256sum manifest result.kdl identity.log runtime-identity.log \
            session.log input-guard.log recovery.log lifecycle.log >SHA256SUMS
    )
    if env "${session_env[@]}" "$RELEASE/bin/sophia-verify-truecolor-runs" 1 \
        >/dev/null 2>&1; then
        echo "TrueColor verifier re-adjudicated an ineligible failure" >&2
        exit 1
    fi
done
printf '%s\n' \
    'sophia_installed_truecolor schema=1 status=failed exit_status=0 reason=session_verification' \
    >"$truecolor_run/result.kdl"
sed -i 's/region_red_pixels=9600/region_red_pixels=19200/' \
    "$truecolor_run/session.log"
(
    cd "$truecolor_run"
    sha256sum manifest result.kdl identity.log runtime-identity.log \
        session.log input-guard.log recovery.log lifecycle.log >SHA256SUMS
)
if env "${session_env[@]}" "$RELEASE/bin/sophia-verify-truecolor-runs" 1 \
    >/dev/null 2>&1; then
    echo "TrueColor verifier accepted a checksummed channel swap" >&2
    exit 1
fi

if env "${session_env[@]}" SOPHIA_TEST_SESSION_STATUS=1 \
    "$RELEASE/bin/sophia-session" >/dev/null 2>&1; then
    echo "installed wrapper hid a nonzero session exit" >&2
    exit 1
fi
grep -Fxq \
    'sophia_installed_cycle schema=1 status=failed exit_status=1 reason=session_exit' \
    "$STATE_HOME/sophia/promotion/runs/0002/result.kdl"
if env "${session_env[@]}" "$RELEASE/bin/sophia-verify-cycles" 1 >/dev/null 2>&1; then
    echo "cycle gate skipped the latest failed automatic attempt" >&2
    exit 1
fi

blocked_root="$TEMP_DIR/blocked-run-root"
blocked_marker="$TEMP_DIR/blocked-runner-started"
touch "$blocked_root"
if env "${session_env[@]}" \
    SOPHIA_PROMOTION_RUN_ROOT="$blocked_root" \
    SOPHIA_TEST_RUNNER_MARKER="$blocked_marker" \
    "$RELEASE/bin/sophia-session" >/dev/null 2>&1; then
    echo "installed wrapper launched without reserving an attempt" >&2
    exit 1
fi
[[ ! -e "$blocked_marker" ]] || {
    echo "installed runner started after ledger reservation failed" >&2
    exit 1
}

for _ in 1 2 3; do
    env "${session_env[@]}" "$RELEASE/bin/sophia-session"
done
[[ -s "$STATE_HOME/sophia/installed-session/launch.log.previous" ]]
[[ -s "$STATE_HOME/sophia/installed-session/runtime-identity.log.previous" ]]
[[ "$(stat -c %a "$STATE_HOME/sophia/installed-session/launch.log")" == 600 ]]
[[ "$(stat -c %a "$STATE_HOME/sophia/installed-session/runtime-identity.log")" == 600 ]]
env "${session_env[@]}" "$RELEASE/bin/sophia-verify-cycles" 3

env "${session_env[@]}" "$RELEASE/bin/sophia-kitty-session"
grep -Fxq 'sophia_installed_fallback schema=1 status=passed exit_status=0' \
    "$STATE_HOME/sophia/promotion/fallback-runs/0001/result.kdl"
env "${session_env[@]}" "$RELEASE/bin/sophia-verify-fallback"

if env "${session_env[@]}" SOPHIA_TEST_SESSION_STATUS=1 \
    "$RELEASE/bin/sophia-kitty-session" >/dev/null 2>&1; then
    echo "installed fallback wrapper hid a nonzero session exit" >&2
    exit 1
fi
grep -Fxq \
    'sophia_installed_fallback schema=1 status=failed exit_status=1 reason=session_exit' \
    "$STATE_HOME/sophia/promotion/fallback-runs/0002/result.kdl"
if env "${session_env[@]}" "$RELEASE/bin/sophia-verify-fallback" >/dev/null 2>&1; then
    echo "fallback verifier skipped the latest failed automatic attempt" >&2
    exit 1
fi
env "${session_env[@]}" "$RELEASE/bin/sophia-kitty-session"
env "${session_env[@]}" "$RELEASE/bin/sophia-verify-fallback"

printf '\n' >>"$STATE_HOME/sophia/promotion/fallback-runs/0003/result.kdl"
if env "${session_env[@]}" "$RELEASE/bin/sophia-verify-fallback" >/dev/null 2>&1; then
    echo "fallback verifier accepted a modified archive" >&2
    exit 1
fi

set +e
env "${session_env[@]}" "$RELEASE/bin/sophia-recovery-proof"
watchdog_status=$?
set -e
[[ "$watchdog_status" == 124 ]] || {
    echo "installed recovery wrapper changed the watchdog exit status" >&2
    exit 1
}
grep -Fxq 'sophia_installed_watchdog schema=1 status=passed exit_status=124' \
    "$STATE_HOME/sophia/promotion/watchdog-runs/0001/result.kdl"
env "${session_env[@]}" "$RELEASE/bin/sophia-verify-watchdog"

if env "${session_env[@]}" SOPHIA_TEST_SESSION_STATUS=1 \
    "$RELEASE/bin/sophia-recovery-proof" >/dev/null 2>&1; then
    echo "installed recovery wrapper hid an unexpected session exit" >&2
    exit 1
fi
grep -Fxq \
    'sophia_installed_watchdog schema=1 status=failed exit_status=1 reason=session_exit' \
    "$STATE_HOME/sophia/promotion/watchdog-runs/0002/result.kdl"
if env "${session_env[@]}" "$RELEASE/bin/sophia-verify-watchdog" >/dev/null 2>&1; then
    echo "watchdog verifier skipped the latest failed automatic attempt" >&2
    exit 1
fi
set +e
env "${session_env[@]}" "$RELEASE/bin/sophia-recovery-proof"
watchdog_status=$?
set -e
[[ "$watchdog_status" == 124 ]]
env "${session_env[@]}" "$RELEASE/bin/sophia-verify-watchdog"

printf 'sophia_installed_session schema=1 status=starting profile=xmonad version=0.1.0 commit=%s release=%s started_at_utc=2026-08-05T12:34:56Z launch_id=compatibility-import\n' \
    "$COMMIT" "$RELEASE" \
    >"$STATE_HOME/sophia/installed-session/launch.log"
env "${session_env[@]}" "$RELEASE/bin/sophia-record-watchdog-run"
grep -Fxq 'sophia_installed_watchdog schema=1 status=passed exit_status=124' \
    "$STATE_HOME/sophia/promotion/watchdog-runs/0004/result.kdl"
env "${session_env[@]}" "$RELEASE/bin/sophia-verify-watchdog"

native_sequence="$TEMP_DIR/native-sequence.log"
sed "1s/.*/commit=$COMMIT/" \
    "$ROOT_DIR/tools/fixtures/physical_native_chrome_sequence_pass.log" \
    >"$native_sequence"
env "${session_env[@]}" \
    SOPHIA_TTY_PROFILE=native \
    SOPHIA_INSTALLED_ATTEMPT_MODE=native-chrome \
    SOPHIA_NATIVE_CHROME_SEQUENCE_LOG="$native_sequence" \
    "$RELEASE/bin/sophia-session"
grep -Fxq \
    'sophia_installed_native_chrome schema=1 status=passed exit_status=0' \
    "$STATE_HOME/sophia/promotion/native-chrome-runs/0001/result.kdl"
env "${session_env[@]}" "$RELEASE/bin/sophia-verify-native-chrome"
printf '\n' >>"$STATE_HOME/sophia/promotion/native-chrome-runs/0001/sequence.log"
if env "${session_env[@]}" "$RELEASE/bin/sophia-verify-native-chrome" \
    >/dev/null 2>&1; then
    echo "native-chrome verifier accepted a modified automatic archive" >&2
    exit 1
fi

printf '\n' >>"$STATE_HOME/sophia/promotion/watchdog-runs/0004/result.kdl"
if env "${session_env[@]}" "$RELEASE/bin/sophia-verify-watchdog" >/dev/null 2>&1; then
    echo "watchdog verifier accepted a modified archive" >&2
    exit 1
fi

[[ ! -e "$STATE_HOME/sophia/promotion/emergency-runs" ]]
set +e
env "${session_env[@]}" SOPHIA_TEST_SESSION_STATUS=130 \
    "$RELEASE/bin/sophia-session"
emergency_status=$?
set -e
[[ "$emergency_status" == 130 ]] || {
    echo "installed wrapper changed the emergency exit status" >&2
    exit 1
}
grep -Fxq 'sophia_installed_emergency schema=1 status=passed exit_status=130' \
    "$STATE_HOME/sophia/promotion/emergency-runs/0001/result.kdl"
grep -Fxq \
    'sophia_installed_cycle schema=1 status=failed exit_status=130 reason=session_exit' \
    "$STATE_HOME/sophia/promotion/runs/0006/result.kdl"
env "${session_env[@]}" "$RELEASE/bin/sophia-verify-emergency"
if env "${session_env[@]}" "$RELEASE/bin/sophia-record-emergency-run" \
    >/dev/null 2>&1; then
    echo "emergency recorder accepted a duplicate automatic archive" >&2
    exit 1
fi
printf '\n' >>"$STATE_HOME/sophia/promotion/emergency-runs/0001/result.kdl"
if env "${session_env[@]}" "$RELEASE/bin/sophia-verify-emergency" >/dev/null 2>&1; then
    echo "emergency verifier accepted a modified archive" >&2
    exit 1
fi
printf 'sophia_installed_emergency schema=1 status=failed exit_status=130 reason=session_verification\n' \
    >"$STATE_HOME/sophia/promotion/emergency-runs/0001/result.kdl"
(
    cd "$STATE_HOME/sophia/promotion/emergency-runs/0001"
    sha256sum manifest result.kdl identity.log runtime-identity.log \
        session.log input-guard.log recovery.log lifecycle.log >SHA256SUMS
)
if env "${session_env[@]}" "$RELEASE/bin/sophia-verify-emergency" >/dev/null 2>&1; then
    echo "emergency verifier accepted a checksummed failed archive" >&2
    exit 1
fi

echo "installed session automatic cycle recording checks passed"
