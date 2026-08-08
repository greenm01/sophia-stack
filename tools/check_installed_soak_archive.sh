#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_installed_soak_archive.sh"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TEMP_DIR"' EXIT
RUN_ROOT="$TEMP_DIR/runs"
RUN="$RUN_ROOT/0001"
COMMIT=1111111111111111111111111111111111111111
SOPHIA_DIGEST=db0abd9a1033a5227f41a0304dd26b0c6f3acfc21fe0b3858dfce5c338064146
STARTED=2026-08-06T02:00:00Z

install -d -m 700 "$RUN"
install -m 600 \
    "$ROOT_DIR/tools/fixtures/installed_soak_archive_session_pass.log" \
    "$RUN/session.log"
install -m 600 \
    "$ROOT_DIR/tools/fixtures/physical_xmonad_hardware_smoke_guard_pass.log" \
    "$RUN/input-guard.log"
install -m 600 \
    "$ROOT_DIR/tools/fixtures/physical_xmonad_hardware_smoke_recovery_pass.log" \
    "$RUN/recovery.log"
install -m 600 "$ROOT_DIR/tools/fixtures/installed_lifecycle_normal_pass.log" \
    "$RUN/lifecycle.log"
install -m 600 "$ROOT_DIR/tools/fixtures/installed_runtime_identity_pass.log" \
    "$RUN/runtime-identity.log"
source "$ROOT_DIR/tools/lib/installed_soak_evidence.sh"
sophia_soak_write_summary "$RUN/session.log" "$RUN/soak-summary.kdl"
printf 'sophia_installed_session schema=1 status=starting profile=xmonad version=0.1.0 commit=%s release=/opt/sophia/releases/test started_at_utc=%s launch_id=soak-test\n' \
    "$COMMIT" "$STARTED" >"$RUN/identity.log"
identity_sha256="$(sha256sum "$RUN/identity.log" | awk '{ print $1 }')"
printf 'schema=1\nversion=0.1.0\ncommit=%s\nrelease_id=test\nrecord_schema=4\nrecord_kind=normal\nsession_started_at_utc=%s\nlaunch_identity_sha256=%s\nsophia_binary_sha256=%s\n' \
    "$COMMIT" "$STARTED" "$identity_sha256" "$SOPHIA_DIGEST" >"$RUN/manifest"
printf 'sophia_installed_cycle schema=1 status=passed exit_status=0\n' \
    >"$RUN/result.kdl"
write_checksums() {
    local run="$1"
    (
        cd "$run"
        sha256sum manifest result.kdl identity.log runtime-identity.log \
            session.log input-guard.log recovery.log lifecycle.log \
            soak-summary.kdl >SHA256SUMS
    )
}
write_checksums "$RUN"

env SOPHIA_PROMOTION_RUN_ROOT="$RUN_ROOT" "$VERIFY"
env SOPHIA_PROMOTION_RUN_ROOT="$RUN_ROOT" "$VERIFY" 7200000 10 5

STAGED_RELEASE="$TEMP_DIR/release"
install -d -m 755 "$STAGED_RELEASE/bin" "$STAGED_RELEASE/tools/lib"
install -m 755 "$ROOT_DIR/tools/verify_installed_soak_archive.sh" \
    "$STAGED_RELEASE/bin/sophia-verify-soak"
install -m 755 "$ROOT_DIR/tools/verify_installed_session_soak.sh" \
    "$STAGED_RELEASE/bin/sophia-verify-soak-session"
install -m 755 "$ROOT_DIR/tools/verify_installed_login_cycle.sh" \
    "$STAGED_RELEASE/bin/sophia-verify-login-cycle"
install -m 755 "$ROOT_DIR/tools/verify_installed_runtime_identity.sh" \
    "$STAGED_RELEASE/bin/sophia-verify-runtime-identity"
install -m 755 "$ROOT_DIR/tools/verify_installed_session_lifecycle.sh" \
    "$STAGED_RELEASE/bin/sophia-verify-lifecycle"
install -m 644 "$ROOT_DIR/tools/lib/installed_soak_evidence.sh" \
    "$STAGED_RELEASE/tools/lib/installed_soak_evidence.sh"
env SOPHIA_PROMOTION_RUN_ROOT="$RUN_ROOT" \
    "$STAGED_RELEASE/bin/sophia-verify-soak"

FAILED_ROOT="$TEMP_DIR/failed-runs"
install -d -m 700 "$FAILED_ROOT"
cp -a "$RUN" "$FAILED_ROOT/0001"
printf 'sophia_installed_cycle schema=1 status=failed exit_status=1 reason=session_exit\n' \
    >"$FAILED_ROOT/0001/result.kdl"
write_checksums "$FAILED_ROOT/0001"
if env SOPHIA_PROMOTION_RUN_ROOT="$FAILED_ROOT" "$VERIFY" >/dev/null 2>&1; then
    echo "installed soak verifier accepted a failed latest attempt" >&2
    exit 1
fi

APPLICATION_ROOT="$TEMP_DIR/application-runs"
install -d -m 700 "$APPLICATION_ROOT"
cp -a "$RUN" "$APPLICATION_ROOT/0001"
sed -i 's/name=firefox version=[^ ]* digest=[0-9a-f]*/name=firefox version=unavailable digest=unavailable/' \
    "$APPLICATION_ROOT/0001/runtime-identity.log"
write_checksums "$APPLICATION_ROOT/0001"
if env SOPHIA_PROMOTION_RUN_ROOT="$APPLICATION_ROOT" "$VERIFY" >/dev/null 2>&1; then
    echo "installed soak verifier accepted no exact Firefox identity" >&2
    exit 1
fi

DIGEST_ROOT="$TEMP_DIR/digest-runs"
install -d -m 700 "$DIGEST_ROOT"
cp -a "$RUN" "$DIGEST_ROOT/0001"
sed -i \
    's/^sophia_binary_sha256=.*/sophia_binary_sha256=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/' \
    "$DIGEST_ROOT/0001/manifest"
write_checksums "$DIGEST_ROOT/0001"
if env SOPHIA_PROMOTION_RUN_ROOT="$DIGEST_ROOT" "$VERIFY" >/dev/null 2>&1; then
    echo "installed soak verifier accepted a false Sophia binary digest" >&2
    exit 1
fi

TAMPERED_ROOT="$TEMP_DIR/tampered-runs"
install -d -m 700 "$TAMPERED_ROOT"
cp -a "$RUN" "$TAMPERED_ROOT/0001"
printf '\n' >>"$TAMPERED_ROOT/0001/session.log"
if env SOPHIA_PROMOTION_RUN_ROOT="$TAMPERED_ROOT" "$VERIFY" >/dev/null 2>&1; then
    echo "installed soak verifier accepted a modified archive" >&2
    exit 1
fi

SUMMARY_ROOT="$TEMP_DIR/summary-runs"
install -d -m 700 "$SUMMARY_ROOT"
cp -a "$RUN" "$SUMMARY_ROOT/0001"
sed -i 's/practical_complete=14/practical_complete=13/' \
    "$SUMMARY_ROOT/0001/soak-summary.kdl"
write_checksums "$SUMMARY_ROOT/0001"
if env SOPHIA_PROMOTION_RUN_ROOT="$SUMMARY_ROOT" "$VERIFY" >/dev/null 2>&1; then
    echo "installed soak verifier accepted a false redacted summary" >&2
    exit 1
fi

echo "installed soak archive verifier checks passed"
