#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SESSION="$ROOT_DIR/tools/fixtures/physical_xmonad_tty3_pass.log"
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
    'status=physical_action_committed action=259' \
    'workspace=2 visible_surfaces=0 focus=none' \
    'workspace=3 visible_surfaces=0 focus=none' \
    'workspace=1 visible_surfaces=2 focus=surface' \
    'status=hidden_focus_cleared ' \
    'status=key_suppressed reason=no_focus' \
    'status=workspace_focus_restore_queued ' \
    'status=retired output=2 ' \
    'status=captured images=' \
    'status=restored images=' \
    'schema=6 status=quiesced target=' \
    'schema=1 status=active source=resume' \
    'source=2560x1440 target=2560x1440_0_0 clip=none unit_scale=true' \
    'sophia_live_selection schema=1 status=complete ' \
    'sophia_live_session_cursor schema=5 path=legacy_ioctl ' \
    'status=output_edge_confined axis=horizontal side=minimum' \
    'status=edge_reverse_immediate axis=vertical side=maximum' \
    'sophia_live_session_keys schema=2 '; do
    grep -Fv "$mutation" "$SESSION" >"$TEMP_FILE"
    if "$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
        "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
        echo "xmonad verifier accepted evidence missing: $mutation" >&2
        exit 1
    fi
done

sed 's/status=restored images=2 source=seat_resume/status=restored images=1 source=seat_resume/' \
    "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "xmonad verifier accepted an incomplete renderer-image handoff" >&2
    exit 1
fi

sed 's/repeat_routed=6/repeat_routed=0/' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "xmonad verifier accepted a session without held-key repeat" >&2
    exit 1
fi

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

sed 's/updates_primary_in_flight=2/updates_primary_in_flight=0/' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "xmonad verifier accepted no legacy cursor updates during primary flips" >&2
    exit 1
fi

sed 's/hidden_updates=0/hidden_updates=1/' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "xmonad verifier accepted a cursor hidden outside all outputs" >&2
    exit 1
fi

for mutation in \
    's/status=complete owner_changes=2 conversions=2 content=redacted/status=complete owner_changes=1 conversions=2 content=redacted/' \
    's/status=complete owner_changes=2 conversions=2 content=redacted/status=complete owner_changes=2 conversions=1 content=redacted/'; do
    sed "$mutation" "$SESSION" >"$TEMP_FILE"
    if "$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
        "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
        echo "xmonad verifier accepted incomplete clipboard evidence: $mutation" >&2
        exit 1
    fi
done

sed '1i[0.123] [glfw error 65544]: optional DBus service unavailable' \
    "$SESSION" >"$TEMP_FILE"
"$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"

sed '1i[0.123] [glfw error 65544]: X11: Failed to become owner of clipboard selection' \
    "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "xmonad verifier accepted a Kitty clipboard ownership failure" >&2
    exit 1
fi

sed '1i[0.123] [glfw error 65545]: X11: Failed to convert selection to data from clipboard' \
    "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "xmonad verifier accepted a Kitty clipboard conversion failure" >&2
    exit 1
fi

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

# The atomic cursor path, held to its own shape: zero overlap is correct
# there and would mean a motionless pointer on the legacy ioctl.
sed -e 's/path=legacy_ioctl/path=atomic_plane/' \
    -e 's/updates_primary_in_flight=[0-9]*/updates_primary_in_flight=0/' \
    "$SESSION" >"$TEMP_FILE"
if ! "$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "xmonad verifier rejected a valid atomic cursor session" >&2
    exit 1
fi

sed 's/path=legacy_ioctl/path=atomic_plane/' "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "xmonad verifier accepted an atomic cursor overlapping a flip" >&2
    exit 1
fi

sed -e 's/path=legacy_ioctl/path=composited/' \
    -e 's/updates_primary_in_flight=[0-9]*/updates_primary_in_flight=0/' \
    "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
    echo "xmonad verifier accepted a cursor on neither hardware path" >&2
    exit 1
fi

echo "xmonad TTY3 verifier fixtures passed"
