#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PASS="$ROOT_DIR/tools/fixtures/installed_session_soak_pass.log"
IDENTITY_PASS="$ROOT_DIR/tools/fixtures/installed_runtime_identity_pass.log"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TEMP_DIR"' EXIT
TEMP_FILE="$TEMP_DIR/mutated.log"
CAPTURE_RELEASE="$TEMP_DIR/release"

install -d -m 755 "$CAPTURE_RELEASE/target/release"
printf 'schema=1\nversion=0.1.0-test\n' >"$CAPTURE_RELEASE/manifest"
printf 'sophia-test-binary\n' >"$CAPTURE_RELEASE/target/release/sophia"
printf 'xmonad-test-binary\n' >"$CAPTURE_RELEASE/target/release/xmonad"
"$ROOT_DIR/tools/installed/capture-runtime-identity.sh" \
    "$TEMP_DIR/captured.log" "$CAPTURE_RELEASE"
sophia_digest="$(sha256sum "$CAPTURE_RELEASE/target/release/sophia" | awk '{print $1}')"
grep -Fxq \
    "sophia_runtime_identity schema=2 kind=application name=sophia version=0.1.0-test digest=$sophia_digest" \
    "$TEMP_DIR/captured.log"

"$ROOT_DIR/tools/verify_installed_session_soak.sh" "$PASS" 7200000 2 2
sed 's/^sophia_live_session schema=14 status=bounded_complete /sophia_live_session schema=16 status=bounded_complete /' \
    "$PASS" >"$TEMP_FILE"
"$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2
"$ROOT_DIR/tools/verify_installed_runtime_identity.sh" "$IDENTITY_PASS"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$PASS" 7200001 2 2; then
    echo "installed soak verifier accepted an undersized duration" >&2
    exit 1
fi
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$PASS" 7200000 3 2; then
    echo "installed soak verifier accepted too few terminal actions" >&2
    exit 1
fi
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$PASS" 7200000 2 3; then
    echo "installed soak verifier accepted too few Firefox actions" >&2
    exit 1
fi
sed '/status=complete stages=8 /d' "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted no Firefox interaction proof" >&2
    exit 1
fi
sed '/status=complete output=2 /d' "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted only one output" >&2
    exit 1
fi
sed '/name=firefox /d' "$IDENTITY_PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_runtime_identity.sh" "$TEMP_FILE"; then
    echo "runtime identity verifier accepted a missing Firefox identity" >&2
    exit 1
fi
sed '/name=sophia /d' "$IDENTITY_PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_runtime_identity.sh" "$TEMP_FILE"; then
    echo "runtime identity verifier accepted a missing Sophia identity" >&2
    exit 1
fi
sed 's/name=sophia version=0.1.0 digest=[0-9a-f]*/name=sophia version=0.1.0 digest=unavailable/' \
    "$IDENTITY_PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_runtime_identity.sh" "$TEMP_FILE"; then
    echo "runtime identity verifier accepted an unavailable Sophia digest" >&2
    exit 1
fi
wrong_sophia_digest=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
if "$ROOT_DIR/tools/verify_installed_runtime_identity.sh" \
    "$IDENTITY_PASS" "$wrong_sophia_digest"; then
    echo "runtime identity verifier accepted the wrong expected Sophia digest" >&2
    exit 1
fi
sed 's/status=connected/status=disconnected/' "$IDENTITY_PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_runtime_identity.sh" "$TEMP_FILE"; then
    echo "runtime identity verifier accepted no connected output" >&2
    exit 1
fi
cp "$IDENTITY_PASS" "$TEMP_FILE"
printf 'clipboard=forbidden\n' >>"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_runtime_identity.sh" "$TEMP_FILE"; then
    echo "runtime identity verifier accepted application content" >&2
    exit 1
fi

echo "installed session verifier checks passed"
