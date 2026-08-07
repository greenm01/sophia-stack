#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_installed_xterm_session.sh"
SESSION="$ROOT_DIR/tools/fixtures/installed_xterm_session_pass.log"
GUARD="$ROOT_DIR/tools/fixtures/physical_firefox_guard_pass.log"
RECOVERY="$ROOT_DIR/tools/fixtures/physical_firefox_recovery_pass.log"
TEMP_FILE="$(mktemp)"
trap 'rm -f -- "$TEMP_FILE"' EXIT

"$VERIFY" "$SESSION" "$GUARD" "$RECOVERY"

for mutation in \
    'terminal=xterm' \
    'status=applied output=2 ' \
    'status=reduced outputs=2 ' \
    'status=committed transaction=407 surface=2097166 source=cpu_backing_snapshot' \
    'status=committed transaction=407 surface=2097166 source=2556x1422' \
    'status=retired output=1 submission=6 frame=7' \
    'status=captured images=0' \
    'status=rehydrated outputs=2 ' \
    'status=active source=resume' \
    'status=retired output=1 submission=7 frame=8' \
    'status=session_action_committed transaction=2 action=Logout' \
    'status=clean app_groups=0 frontend_workers=0'; do
    grep -Fv "$mutation" "$SESSION" >"$TEMP_FILE"
    if "$VERIFY" "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
        echo "installed xterm verifier accepted evidence missing: $mutation" >&2
        exit 1
    fi
done

for rewrite in \
    's/work=2560x1426_0_14/work=2560x1440_0_0/' \
    's/width=2556 height=1422/width=1278 height=1422/' \
    's/target=2556x1422_2_16/target=1278x1422_2_16/' \
    's/status=restored images=0/status=restored images=1/' \
    's/nonzero_outputs=2/nonzero_outputs=0/' \
    's/outcome=drained drained=true/outcome=forced_detach_timeout drained=false/' \
    's/expected=0 unexpected=0/expected=0 unexpected=1/'; do
    sed "$rewrite" "$SESSION" >"$TEMP_FILE"
    if "$VERIFY" "$TEMP_FILE" "$GUARD" "$RECOVERY" >/dev/null 2>&1; then
        echo "installed xterm verifier accepted invalid evidence: $rewrite" >&2
        exit 1
    fi
done

echo "installed xterm verifier fixtures passed"
