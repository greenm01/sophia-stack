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
    'status=exited id=terminal source=startup ' \
    'status=desktop_pointer_active source=post_startup_exit' \
    'status=physical_action_committed action=768' \
    'status=physical_action_committed action=258' \
    'status=hidden_focus_cleared ' \
    'status=key_suppressed reason=no_focus' \
    'status=retired output=2 ' \
    'schema=6 status=quiesced target=' \
    'schema=1 status=active source=resume' \
    'source=2560x1440 target=2560x1440_0_0 clip=none unit_scale=true' \
    'sophia_live_session_cursor schema=3 '; do
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

sed 's/max_update_msec=9/max_update_msec=101/' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "xmonad verifier accepted a blocking cursor update above the owner budget" >&2
    exit 1
fi

sed '1i[0.123] [glfw error 65544]: optional DBus service unavailable' \
    "$SESSION" >"$TEMP_FILE"
"$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"

sed '$aError: "structured Sophia failure"' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "xmonad verifier accepted a structured Sophia failure" >&2
    exit 1
fi

sed 's/expected=2 unexpected=0/expected=2 unexpected=1/' \
    "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "xmonad verifier accepted an unexpected X protocol error" >&2
    exit 1
fi

echo "xmonad TTY3 verifier fixtures passed"
