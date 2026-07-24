#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PASS="$ROOT_DIR/tools/fixtures/installed_session_soak_pass.log"

"$ROOT_DIR/tools/verify_installed_session_soak.sh" "$PASS" 7200000 2 2
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

echo "installed session verifier checks passed"
