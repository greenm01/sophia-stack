#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_installed_session_lifecycle.sh"
NORMAL="$ROOT_DIR/tools/fixtures/installed_lifecycle_normal_pass.log"
EMERGENCY="$ROOT_DIR/tools/fixtures/installed_lifecycle_emergency_pass.log"
TEMP_FILE="$(mktemp)"
trap 'rm -f -- "$TEMP_FILE"' EXIT

"$VERIFY" "$NORMAL" normal
"$VERIFY" "$EMERGENCY" emergency

sed 's/runtime=owner/runtime=temporary/' "$NORMAL" >"$TEMP_FILE"
if "$VERIFY" "$TEMP_FILE" normal; then
    echo "installed lifecycle verifier accepted temporary runtime state" >&2
    exit 1
fi
grep -Fv 'status=complete phase=input_guard' "$NORMAL" >"$TEMP_FILE"
if "$VERIFY" "$TEMP_FILE" normal; then
    echo "installed lifecycle verifier accepted a missing guard phase" >&2
    exit 1
fi
sed 's/exit_status=0 emergency=false/exit_status=130 emergency=true/' \
    "$NORMAL" >"$TEMP_FILE"
if "$VERIFY" "$TEMP_FILE" normal; then
    echo "installed lifecycle verifier accepted emergency as normal handoff" >&2
    exit 1
fi

echo "installed lifecycle verifier fixtures passed"
