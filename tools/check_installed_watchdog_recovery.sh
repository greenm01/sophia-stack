#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_installed_watchdog_recovery.sh"
SESSION="$ROOT_DIR/tools/fixtures/installed_watchdog_session_pass.log"
GUARD="$ROOT_DIR/tools/fixtures/installed_watchdog_guard_pass.log"
RECOVERY="$ROOT_DIR/tools/fixtures/installed_watchdog_recovery_pass.log"
LIFECYCLE="$ROOT_DIR/tools/fixtures/installed_lifecycle_watchdog_pass.log"
TEMP_FILE="$(mktemp)"
trap 'rm -f -- "$TEMP_FILE"' EXIT

"$VERIFY" "$SESSION" "$GUARD" "$RECOVERY" "$LIFECYCLE"

sed 's/deadline_seconds=45/deadline_seconds=44/' "$SESSION" >"$TEMP_FILE"
if "$VERIFY" "$TEMP_FILE" "$GUARD" "$RECOVERY" "$LIFECYCLE"; then
    echo "watchdog verifier accepted the wrong installed deadline" >&2
    exit 1
fi
cp "$GUARD" "$TEMP_FILE"
printf '%s\n' 'sophia_session_input_guard schema=1 status=triggered' >>"$TEMP_FILE"
if "$VERIFY" "$SESSION" "$TEMP_FILE" "$RECOVERY" "$LIFECYCLE"; then
    echo "watchdog verifier accepted a local emergency trigger" >&2
    exit 1
fi
sed 's/session_shutdown=watchdog_term/session_shutdown=graceful/' \
    "$RECOVERY" >"$TEMP_FILE"
if "$VERIFY" "$SESSION" "$GUARD" "$TEMP_FILE" "$LIFECYCLE"; then
    echo "watchdog verifier accepted a graceful-shutdown record" >&2
    exit 1
fi
sed 's/exit_status=124 emergency=true/exit_status=0 emergency=false/' \
    "$LIFECYCLE" >"$TEMP_FILE"
if "$VERIFY" "$SESSION" "$GUARD" "$RECOVERY" "$TEMP_FILE"; then
    echo "watchdog verifier accepted a normal lifecycle handoff" >&2
    exit 1
fi

echo "installed watchdog recovery verifier fixtures passed"
