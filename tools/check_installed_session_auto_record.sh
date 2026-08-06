#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TEMP_DIR"' EXIT
STATE_HOME="$TEMP_DIR/state"
PREFIX="$TEMP_DIR/opt/sophia"
RELEASE="$PREFIX/releases/0.1.0-test"
COMMIT=2222222222222222222222222222222222222222

install -d -m 755 "$RELEASE/bin" "$RELEASE/tools/lib"
install -m 755 \
    "$ROOT_DIR/tools/installed/sophia-session" \
    "$RELEASE/bin/sophia-session"
install -m 755 \
    "$ROOT_DIR/tools/installed/sophia-kitty-session" \
    "$RELEASE/bin/sophia-kitty-session"
install -m 755 \
    "$ROOT_DIR/tools/record_installed_session_run.sh" \
    "$RELEASE/bin/sophia-record-run"
install -m 755 \
    "$ROOT_DIR/tools/record_installed_fallback_run.sh" \
    "$RELEASE/bin/sophia-record-fallback-run"
install -m 755 \
    "$ROOT_DIR/tools/verify_installed_login_cycle.sh" \
    "$RELEASE/bin/sophia-verify-login-cycle"
install -m 755 \
    "$ROOT_DIR/tools/verify_installed_fallback_session.sh" \
    "$RELEASE/bin/sophia-verify-fallback-session"
install -m 755 \
    "$ROOT_DIR/tools/verify_installed_fallback_run.sh" \
    "$RELEASE/bin/sophia-verify-fallback"
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
    'if [[ "${SOPHIA_TTY_PROFILE:-xmonad}" == kitty ]]; then' \
    '    state="${XDG_STATE_HOME}/sophia/kitty-session"' \
    '    session_fixture=installed_fallback_session_pass.log' \
    '    guard_fixture=installed_fallback_guard_pass.log' \
    '    recovery_fixture=installed_fallback_recovery_pass.log' \
    'fi' \
    'install -d -m 700 "$state"' \
    'install -m 600 "$SOPHIA_TEST_FIXTURE_ROOT/tools/fixtures/$session_fixture" "$state/session.log"' \
    'install -m 600 "$SOPHIA_TEST_FIXTURE_ROOT/tools/fixtures/$guard_fixture" "$state/input-guard.log"' \
    'install -m 600 "$SOPHIA_TEST_FIXTURE_ROOT/tools/fixtures/$recovery_fixture" "$state/recovery.log"' \
    'install -m 600 "$SOPHIA_TEST_FIXTURE_ROOT/tools/fixtures/installed_lifecycle_normal_pass.log" "$state/lifecycle.log"' \
    '[[ -z "${SOPHIA_TEST_RUNNER_MARKER:-}" ]] || touch "$SOPHIA_TEST_RUNNER_MARKER"' \
    'exit "${SOPHIA_TEST_SESSION_STATUS:-0}"' \
    >"$RELEASE/tools/run_sophia_xmonad_session.sh"
chmod 755 \
    "$RELEASE/bin/capture-runtime-identity" \
    "$RELEASE/tools/run_sophia_xmonad_session.sh"
printf 'schema=1\nversion=0.1.0\ncommit=%s\nrelease_id=0.1.0-test\nbuilt_at_utc=2026-08-05T00:00:00Z\n' \
    "$COMMIT" >"$RELEASE/manifest"
(
    cd "$RELEASE"
    find bin tools -type f -print0 | sort -z | xargs -0 sha256sum >SHA256SUMS
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

echo "installed session automatic cycle recording checks passed"
