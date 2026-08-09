#!/bin/sh
set -eu

root_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
temp_dir=$(mktemp -d)
trap 'rm -rf -- "$temp_dir"' EXIT HUP INT TERM

evidence="$temp_dir/evidence.log"
marker="$temp_dir/restart.marker"
proof_result="$temp_dir/proof.result"

printf '%s\n' \
    'sophia_live_wm schema=1 status=physical_action_committed action=37' \
    'sophia_live_wm schema=1 status=physical_action_committed action=34' \
    '2026-08-09T00:00:00Z INF hagia event=checkpoint status=saved detail="candidate_nonempty=true"' \
    'sophia_live_wm schema=4 status=restarted adapter=sophia_wm_v1 epoch=2 restarts=1 preserved_layout=true' \
    'sophia_live_wm schema=1 status=physical_action_committed action=37' \
    'sophia_live_wm schema=1 status=physical_action_committed action=38' \
    'sophia_live_wm schema=1 status=physical_action_committed action=38' \
    'sophia_live_wm schema=1 status=physical_action_committed action=39' \
    'sophia_live_wm schema=1 status=physical_action_committed action=40' \
    '2026-08-09T00:00:01Z INF hagia event=checkpoint status=saved detail="candidate_nonempty=true"' \
    'sophia_live_wm schema=1 status=physical_action_committed action=33' \
    'sophia_live_wm schema=1 status=physical_action_committed action=34' \
    >"$evidence"

set +e
env \
    SOPHIA_HAGIA_BIN=/usr/bin/sleep \
    SOPHIA_HAGIA_RESTART_MARKER="$marker" \
    SOPHIA_HAGIA_RESTART_AFTER_ACTION=34 \
    SOPHIA_HAGIA_RESTART_REQUIRES_ACTION=37 \
    SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$evidence" \
    "$root_dir/tools/fixtures/hagia_restart_once.sh" 30
wrapper_status=$?
set -e

if [ "$wrapper_status" -ne 137 ] || [ ! -e "$marker" ]; then
    echo "Hagia restart matcher did not terminate its fixture process" >&2
    exit 1
fi

set +e
printf '%s\n' hagiapolicyproof | timeout 2s env \
    SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$evidence" \
    SOPHIA_INPUT_PROOF_RESULT="$proof_result" \
    "$root_dir/tools/fixtures/hagia_physical_guide.sh" >/dev/null
guide_status=$?
set -e

if [ "$guide_status" -ne 124 ]; then
    echo "Hagia physical guide exited unexpectedly: $guide_status" >&2
    exit 1
fi
if [ "$(cat "$proof_result" 2>/dev/null || true)" != hagiapolicyproof ]; then
    echo "Hagia physical guide did not cross its structured checkpoint matcher" >&2
    exit 1
fi

printf '%s\n' 'Hagia physical matchers accepted structured checkpoint evidence.'
