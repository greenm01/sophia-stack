#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TEMP_DIR"' EXIT
RUN="$TEMP_DIR/0001"
COMMIT=3103e31000000000000000000000000000000000
SOPHIA_SHA256=db0abd9a1033a5227f41a0304dd26b0c6f3acfc21fe0b3858dfce5c338064146
STARTED_AT=2026-08-06T12:34:56Z

install -d -m 700 "$RUN"
install -m 600 "$ROOT_DIR/tools/fixtures/physical_native_chrome_pass.log" \
    "$RUN/session.log"
install -m 600 "$ROOT_DIR/tools/fixtures/physical_native_chrome_sequence_pass.log" \
    "$RUN/sequence.log"
install -m 600 "$ROOT_DIR/tools/fixtures/physical_xmonad_hardware_smoke_guard_pass.log" \
    "$RUN/input-guard.log"
install -m 600 "$ROOT_DIR/tools/fixtures/installed_native_chrome_recovery_pass.log" \
    "$RUN/recovery.log"
install -m 600 "$ROOT_DIR/tools/fixtures/installed_lifecycle_normal_pass.log" \
    "$RUN/lifecycle.log"
install -m 600 "$ROOT_DIR/tools/fixtures/installed_runtime_identity_pass.log" \
    "$RUN/runtime-identity.log"
printf 'sophia_installed_session schema=1 status=starting profile=native version=0.1.0 commit=%s release=/opt/sophia/releases/test started_at_utc=%s launch_id=test\n' \
    "$COMMIT" "$STARTED_AT" >"$RUN/identity.log"
identity_sha256="$(sha256sum "$RUN/identity.log" | awk '{ print $1 }')"
printf 'schema=1\nversion=0.1.0\ncommit=%s\nrelease_id=test\nbuilt_at_utc=2026-08-06T12:00:00Z\nrecord_schema=4\nrecord_kind=native-chrome\nsession_started_at_utc=%s\nlaunch_identity_sha256=%s\nsophia_binary_sha256=%s\nnative_wm_binary_sha256=8c802845e2fc4dee4e4624215f05343715eb0974eaa717f1286de2fa6965cf7f\n' \
    "$COMMIT" "$STARTED_AT" "$identity_sha256" "$SOPHIA_SHA256" \
    >"$RUN/manifest"
printf 'sophia_installed_native_chrome schema=1 status=passed exit_status=0\n' \
    >"$RUN/result.kdl"
(
    cd "$RUN"
    sha256sum manifest result.kdl identity.log runtime-identity.log \
        session.log sequence.log input-guard.log recovery.log lifecycle.log \
        >SHA256SUMS
)

"$ROOT_DIR/tools/verify_installed_native_chrome_archive.sh" "$RUN"

cp "$RUN/sequence.log" "$TEMP_DIR/sequence.log"
printf '\n' >>"$RUN/sequence.log"
if "$ROOT_DIR/tools/verify_installed_native_chrome_archive.sh" "$RUN" \
    >/dev/null 2>&1; then
    echo "native-chrome archive verifier accepted modified sequence evidence" >&2
    exit 1
fi
mv "$TEMP_DIR/sequence.log" "$RUN/sequence.log"

cp "$RUN/manifest" "$TEMP_DIR/manifest"
sed -i \
    's/^native_wm_binary_sha256=.*/native_wm_binary_sha256=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/' \
    "$RUN/manifest"
(
    cd "$RUN"
    sha256sum manifest result.kdl identity.log runtime-identity.log \
        session.log sequence.log input-guard.log recovery.log lifecycle.log \
        >SHA256SUMS
)
if "$ROOT_DIR/tools/verify_installed_native_chrome_archive.sh" "$RUN" \
    >/dev/null 2>&1; then
    echo "native-chrome archive verifier accepted a false native-WM digest" >&2
    exit 1
fi
mv "$TEMP_DIR/manifest" "$RUN/manifest"

sed -i "1s/$COMMIT/4103e31000000000000000000000000000000000/" \
    "$RUN/sequence.log"
(
    cd "$RUN"
    sha256sum manifest result.kdl identity.log runtime-identity.log \
        session.log sequence.log input-guard.log recovery.log lifecycle.log \
        >SHA256SUMS
)
if "$ROOT_DIR/tools/verify_installed_native_chrome_archive.sh" "$RUN" \
    >/dev/null 2>&1; then
    echo "native-chrome archive verifier accepted a mismatched proof commit" >&2
    exit 1
fi

echo "installed native-chrome archive verifier checks passed"
