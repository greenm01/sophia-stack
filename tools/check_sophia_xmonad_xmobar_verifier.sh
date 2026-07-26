#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SESSION="$ROOT_DIR/tools/fixtures/physical_xmonad_xmobar_session_pass.log"
GUARD="$ROOT_DIR/tools/fixtures/physical_xmonad_xmobar_guard_pass.log"
RECOVERY="$ROOT_DIR/tools/fixtures/physical_xmonad_xmobar_recovery_pass.log"
TEMP_FILE="$(mktemp)"
trap 'rm -f -- "$TEMP_FILE"' EXIT

"$ROOT_DIR/tools/verify_sophia_xmonad_xmobar.sh" \
    "$SESSION" "$GUARD" "$RECOVERY"

for mutation in \
    'status=started id=statusbar source=startup' \
    'status=reduced outputs=2 changed=2 rejected=0 active_reservations=1' \
    'status=applied output=2 ' \
    'eligible_surfaces=1 frames=1 focused_frames=1 unfocused_frames=0 focus_rings=1 primitives=8 clearance=4' \
    'status=target_routed role=client_positioned kind=button' \
    'status=target_routed role=client_positioned kind=axis' \
    'workspace=2 ' \
    'workspace=1 ' \
    'schema=6 status=quiesced target=' \
    'status=active source=resume' \
    'action=Logout'; do
    grep -Fv "$mutation" "$SESSION" >"$TEMP_FILE"
    if "$ROOT_DIR/tools/verify_sophia_xmonad_xmobar.sh" \
        "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
        echo "xmobar verifier accepted evidence missing: $mutation" >&2
        exit 1
    fi
done

sed 's/work=2560x1426_0_14/work=2560x1425_0_14/' \
    "$SESSION" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_xmonad_xmobar.sh" \
    "$TEMP_FILE" "$GUARD" "$RECOVERY"; then
    echo "xmobar verifier accepted an inexact top-edge reservation" >&2
    exit 1
fi

sed 's/emergency=false/emergency=true/' "$RECOVERY" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_xmonad_xmobar.sh" \
    "$SESSION" "$GUARD" "$TEMP_FILE"; then
    echo "xmobar verifier accepted emergency TTY recovery" >&2
    exit 1
fi

echo "xmobar xmonad verifier fixtures passed"
