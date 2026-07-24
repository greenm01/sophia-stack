#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PASS="$ROOT_DIR/tools/fixtures/installed_session_soak_pass.log"
IDENTITY_PASS="$ROOT_DIR/tools/fixtures/installed_runtime_identity_pass.log"
TEMP_FILE="$(mktemp)"
trap 'rm -f -- "$TEMP_FILE"' EXIT

"$ROOT_DIR/tools/verify_installed_session_soak.sh" "$PASS" 7200000 2 2
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
sed '/status=complete stages=6 /d' "$PASS" >"$TEMP_FILE"
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
