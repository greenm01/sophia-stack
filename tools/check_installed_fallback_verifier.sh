#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_installed_fallback_session.sh"
SESSION="$ROOT_DIR/tools/fixtures/installed_fallback_session_pass.log"
GUARD="$ROOT_DIR/tools/fixtures/installed_fallback_guard_pass.log"
RECOVERY="$ROOT_DIR/tools/fixtures/installed_fallback_recovery_pass.log"
TEMP_FILE="$(mktemp)"
trap 'rm -f -- "$TEMP_FILE"' EXIT

"$VERIFY" "$SESSION" "$GUARD" "$RECOVERY"

grep -Fv 'status=exited id=terminal source=startup' "$SESSION" >"$TEMP_FILE"
if "$VERIFY" "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "fallback verifier accepted a session without normal Kitty exit" >&2
    exit 1
fi
sed 's/outputs_ready=2\/2/outputs_ready=1\/2/' "$SESSION" >"$TEMP_FILE"
if "$VERIFY" "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "fallback verifier accepted one-output startup readiness" >&2
    exit 1
fi
sed 's/elapsed_msec=650/elapsed_msec=8001/' "$SESSION" >"$TEMP_FILE"
if "$VERIFY" "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "fallback verifier accepted slow startup" >&2
    exit 1
fi
grep -Fv 'status=retired output=1 ' "$SESSION" >"$TEMP_FILE"
if "$VERIFY" "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "fallback verifier accepted no asynchronous retirement" >&2
    exit 1
fi
grep -Fv 'status=presented output=2 proof=synchronous_modeset' "$SESSION" >"$TEMP_FILE"
if "$VERIFY" "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "fallback verifier accepted a missing startup output" >&2
    exit 1
fi
sed 's/output=2 proof=synchronous_modeset/output=1 proof=synchronous_modeset/' \
    "$SESSION" >"$TEMP_FILE"
if "$VERIFY" "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "fallback verifier accepted duplicate startup output identities" >&2
    exit 1
fi
sed 's/physical_keys_routed=8/physical_keys_routed=0/' "$SESSION" >"$TEMP_FILE"
if "$VERIFY" "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "fallback verifier accepted no routed physical input" >&2
    exit 1
fi
sed 's/wm_policy=disabled/wm_policy=external/g' "$SESSION" >"$TEMP_FILE"
if "$VERIFY" "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "fallback verifier accepted an external-WM session" >&2
    exit 1
fi
cp "$GUARD" "$TEMP_FILE"
printf '%s\n' 'sophia_session_input_guard schema=1 status=triggered' >>"$TEMP_FILE"
if "$VERIFY" "$SESSION" "$TEMP_FILE" "$RECOVERY" >/dev/null 2>&1; then
    echo "fallback verifier accepted emergency guard recovery" >&2
    exit 1
fi
sed 's/profile=kitty/profile=xmonad/' "$RECOVERY" >"$TEMP_FILE"
if "$VERIFY" "$SESSION" "$GUARD" "$TEMP_FILE" >/dev/null 2>&1; then
    echo "fallback verifier accepted the wrong recovery profile" >&2
    exit 1
fi

echo "installed fallback verifier fixtures passed"
