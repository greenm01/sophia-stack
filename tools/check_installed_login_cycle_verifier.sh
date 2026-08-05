#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_installed_login_cycle.sh"
SESSION="$ROOT_DIR/tools/fixtures/physical_xmonad_hardware_smoke_session_pass.log"
GUARD="$ROOT_DIR/tools/fixtures/physical_xmonad_hardware_smoke_guard_pass.log"
RECOVERY="$ROOT_DIR/tools/fixtures/physical_xmonad_hardware_smoke_recovery_pass.log"
TEMP_FILE="$(mktemp)"
trap 'rm -f -- "$TEMP_FILE"' EXIT

"$VERIFY" "$SESSION" "$GUARD" "$RECOVERY"

grep -Fv 'status=session_action_committed transaction=6 action=Logout' \
    "$SESSION" >"$TEMP_FILE"
if "$VERIFY" "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "login-cycle verifier accepted a session without normal logout" >&2
    exit 1
fi
grep -Fv 'status=complete output=2 ' "$SESSION" >"$TEMP_FILE"
if "$VERIFY" "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "login-cycle verifier accepted only one output summary" >&2
    exit 1
fi
sed 's/elapsed_msec=650/elapsed_msec=8001/' "$SESSION" >"$TEMP_FILE"
if "$VERIFY" "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "login-cycle verifier accepted slow startup" >&2
    exit 1
fi
sed 's/native_in_flight=false/native_in_flight=true/' "$SESSION" >"$TEMP_FILE"
if "$VERIFY" "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "login-cycle verifier accepted native cleanup debt" >&2
    exit 1
fi
sed 's/emergency=false/emergency=true/' "$RECOVERY" >"$TEMP_FILE"
if "$VERIFY" "$SESSION" "$GUARD" "$TEMP_FILE" >/dev/null 2>&1; then
    echo "login-cycle verifier accepted emergency recovery" >&2
    exit 1
fi

echo "installed login-cycle verifier fixtures passed"
