#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SESSION="$ROOT_DIR/tools/fixtures/physical_xmonad_xmobar_session_pass.log"
GUARD="$ROOT_DIR/tools/fixtures/physical_xmonad_xmobar_guard_pass.log"
RECOVERY="$ROOT_DIR/tools/fixtures/physical_xmonad_xmobar_recovery_pass.log"
TMP="$(mktemp /tmp/sophia-xmobar-hardware-verifier.XXXXXX)"
trap 'rm -f "$TMP"' EXIT

"$ROOT_DIR/tools/verify_sophia_xmonad_xmobar_hardware_smoke.sh" \
    "$SESSION" "$GUARD" "$RECOVERY"

for mutation in \
    'status=started id=statusbar source=startup' \
    'status=reduced outputs=2 changed=2 rejected=0 active_reservations=1' \
    'eligible_surfaces=1 frames=1 focused_frames=1 unfocused_frames=0 focus_rings=1 primitives=8 clearance=4' \
    'role=client_positioned kind=button' \
    'role=client_positioned kind=axis' \
    'action=Logout'; do
    grep -Fv "$mutation" "$SESSION" >"$TMP"
    if "$ROOT_DIR/tools/verify_sophia_xmonad_xmobar_hardware_smoke.sh" \
        "$TMP" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
        echo "xmobar hardware verifier accepted evidence missing: $mutation" >&2
        exit 1
    fi
done

echo "xmobar hardware-smoke verifier regressions passed"
