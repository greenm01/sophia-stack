#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TEMP_DIR"' EXIT
STATE_HOME="$TEMP_DIR/state"
SESSION_DIR="$STATE_HOME/sophia/xmonad-session"
IDENTITY_DIR="$STATE_HOME/sophia/installed-session"
RUN_ROOT="$STATE_HOME/sophia/promotion/runs"
PREFIX="$TEMP_DIR/opt/sophia"
RELEASE="$PREFIX/releases/0.1.0-test"
COMMIT=1111111111111111111111111111111111111111

install -d -m 700 "$SESSION_DIR" "$IDENTITY_DIR" "$RUN_ROOT"
install -d -m 755 "$RELEASE/bin" "$RELEASE/target/release"
install -m 600 \
    "$ROOT_DIR/tools/fixtures/physical_xmonad_hardware_smoke_session_pass.log" \
    "$SESSION_DIR/session.log"
install -m 600 \
    "$ROOT_DIR/tools/fixtures/physical_xmonad_hardware_smoke_guard_pass.log" \
    "$SESSION_DIR/input-guard.log"
install -m 600 \
    "$ROOT_DIR/tools/fixtures/physical_xmonad_hardware_smoke_recovery_pass.log" \
    "$SESSION_DIR/recovery.log"
install -m 600 "$ROOT_DIR/tools/fixtures/installed_lifecycle_normal_pass.log" \
    "$SESSION_DIR/lifecycle.log"
install -m 600 "$ROOT_DIR/tools/fixtures/installed_runtime_identity_pass.log" \
    "$IDENTITY_DIR/runtime-identity.log"
printf 'test\n' >"$RELEASE/bin/payload"
printf 'sophia-test-binary\n' >"$RELEASE/target/release/sophia"
printf 'schema=1\nversion=0.1.0\ncommit=%s\nrelease_id=0.1.0-test\nbuilt_at_utc=2026-08-05T00:00:00Z\n' \
    "$COMMIT" >"$RELEASE/manifest"
(
    cd "$RELEASE"
    sha256sum bin/payload target/release/sophia >SHA256SUMS
)
ln -s releases/0.1.0-test "$PREFIX/current"

set_identity() {
    local started_at_utc="$1"
    printf 'sophia_installed_session schema=1 status=starting profile=xmonad version=0.1.0 commit=%s release=%s started_at_utc=%s\n' \
        "$COMMIT" "$RELEASE" "$started_at_utc" >"$IDENTITY_DIR/launch.log"
}
recorder() {
    env \
        XDG_STATE_HOME="$STATE_HOME" \
        SOPHIA_INSTALL_PREFIX="$PREFIX" \
        SOPHIA_PROMOTION_RUN_ROOT="$RUN_ROOT" \
        "$ROOT_DIR/tools/record_installed_session_run.sh" "$@"
}
record() {
    set_identity "$1"
    recorder
}

sed -i \
    's/name=sophia version=0.1.0 digest=[0-9a-f]*/name=sophia version=0.1.0 digest=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/' \
    "$IDENTITY_DIR/runtime-identity.log"
set_identity 2026-08-05T11:59:00Z
if recorder begin >/dev/null 2>&1; then
    echo "installed recorder reserved a run with the wrong Sophia binary digest" >&2
    exit 1
fi
install -m 600 "$ROOT_DIR/tools/fixtures/installed_runtime_identity_pass.log" \
    "$IDENTITY_DIR/runtime-identity.log"

record 2026-08-05T12:00:00Z
if record 2026-08-05T12:00:00Z >/dev/null 2>&1; then
    echo "installed recorder accepted one launch twice" >&2
    exit 1
fi
record 2026-08-05T12:01:00Z
record 2026-08-05T12:02:00Z
env \
    XDG_STATE_HOME="$STATE_HOME" \
    SOPHIA_PROMOTION_RUN_ROOT="$RUN_ROOT" \
    "$ROOT_DIR/tools/verify_installed_session_cycles.sh" 3

set_identity 2026-08-05T12:03:00Z
failed_run="$(recorder begin)"
if recorder finish "$failed_run" 1 >/dev/null 2>&1; then
    echo "installed recorder accepted a nonzero session" >&2
    exit 1
fi
if env \
    XDG_STATE_HOME="$STATE_HOME" \
    SOPHIA_PROMOTION_RUN_ROOT="$RUN_ROOT" \
    "$ROOT_DIR/tools/verify_installed_session_cycles.sh" 3 >/dev/null 2>&1; then
    echo "installed cycle verifier skipped a failed intervening attempt" >&2
    exit 1
fi
record 2026-08-05T12:04:00Z
record 2026-08-05T12:05:00Z
record 2026-08-05T12:06:00Z
env \
    XDG_STATE_HOME="$STATE_HOME" \
    SOPHIA_PROMOTION_RUN_ROOT="$RUN_ROOT" \
    "$ROOT_DIR/tools/verify_installed_session_cycles.sh" 3

MUTATED_ROOT="$TEMP_DIR/mutated-runs"
install -d -m 700 "$MUTATED_ROOT"
cp -a "$RUN_ROOT/0007" "$MUTATED_ROOT/0001"
sed -i \
    's/^sophia_binary_sha256=.*/sophia_binary_sha256=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/' \
    "$MUTATED_ROOT/0001/manifest"
(
    cd "$MUTATED_ROOT/0001"
    sha256sum manifest result.kdl identity.log runtime-identity.log \
        session.log input-guard.log recovery.log lifecycle.log >SHA256SUMS
)
if env \
    XDG_STATE_HOME="$STATE_HOME" \
    SOPHIA_PROMOTION_RUN_ROOT="$MUTATED_ROOT" \
    "$ROOT_DIR/tools/verify_installed_session_cycles.sh" 1 >/dev/null 2>&1; then
    echo "installed cycle verifier accepted a false Sophia binary digest" >&2
    exit 1
fi

cp -a "$RUN_ROOT/0007" "$RUN_ROOT/9999"
if env \
    XDG_STATE_HOME="$STATE_HOME" \
    SOPHIA_PROMOTION_RUN_ROOT="$RUN_ROOT" \
    "$ROOT_DIR/tools/verify_installed_session_cycles.sh" 3 >/dev/null 2>&1; then
    echo "installed cycle verifier accepted a duplicate launch identity" >&2
    exit 1
fi

echo "installed cycle recorder and ledger checks passed"
