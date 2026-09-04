#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_installed_login_cycle.sh"
SESSION="$ROOT_DIR/tools/fixtures/installed_xterm_session_pass.log"
GUARD="$ROOT_DIR/tools/fixtures/installed_truecolor_input_guard_pass.log"
RECOVERY="$ROOT_DIR/tools/fixtures/installed_truecolor_recovery_pass.log"
TEMP_FILE="$(mktemp)"
trap 'rm -f -- "$TEMP_FILE"' EXIT

"$VERIFY" "$SESSION" "$GUARD" "$RECOVERY"

# Native page-flip records use tracing, whose production formatter prepends
# timestamp, level, target, and ANSI state before the stable schema payload.
sed $'s/^sophia_live_native_page_flip /\033[2m2026-08-06T10:25:02Z\033[0m INFO native_scanout: \033[0m sophia_live_native_page_flip /' \
    "$SESSION" >"$TEMP_FILE"
"$VERIFY" "$TEMP_FILE" "$GUARD" "$RECOVERY"

grep -Fv 'sophia_live_native_page_flip schema=1 status=retired ' \
    "$SESSION" >"$TEMP_FILE"
if "$VERIFY" "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "login-cycle verifier accepted a session without a page-flip retirement" >&2
    exit 1
fi

grep -Fv 'status=session_action_committed transaction=2 action=Logout' \
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
sed 's/elapsed_msec=314/elapsed_msec=8001/' "$SESSION" >"$TEMP_FILE"
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
