#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SESSION="$ROOT_DIR/tools/fixtures/physical_firefox_session_pass.log"
GUARD="$ROOT_DIR/tools/fixtures/physical_firefox_guard_pass.log"
RECOVERY="$ROOT_DIR/tools/fixtures/physical_firefox_recovery_pass.log"
TEMP_FILE="$(mktemp)"
trap 'rm -f -- "$TEMP_FILE"' EXIT

"$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
    "$SESSION" "$GUARD" "$RECOVERY"

for mutation in \
    'status=physical_action_committed action=258' \
    'status=hidden_focus_cleared ' \
    'status=key_suppressed reason=no_focus' \
    'status=retired output=2 ' \
    'sophia_live_session_cursor schema=2 '; do
    grep -Fv "$mutation" "$SESSION" >"$TEMP_FILE"
    if "$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
        "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
        echo "xmonad verifier accepted evidence missing: $mutation" >&2
        exit 1
    fi
done

sed 's/buttons_routed=4/buttons_routed=1/' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "xmonad verifier accepted a pointer proof without click-drag transitions" >&2
    exit 1
fi

echo "xmonad TTY3 verifier fixtures passed"
