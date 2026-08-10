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
    'sophia_live_wm schema=1 status=physical_action_committed action=66' \
    '2026-08-09T00:00:00Z INF hagia event=checkpoint status=saved detail="candidate_nonempty=true"' \
    'sophia_live_wm schema=4 status=restarted adapter=sophia_wm_v1 epoch=2 restarts=1 preserved_layout=true' \
    '2026-08-09T00:00:01Z INF hagia event=checkpoint status=loaded detail="candidate_nonempty=true"' \
    '2026-08-09T00:00:01Z INF hagia event=checkpoint status=reconciled detail="candidate_nonempty=true"' \
    '2026-08-09T00:00:01Z INF hagia event=policy_refresh status=requested detail=checkpoint_reconciled' \
    'sophia_live_wm schema=1 status=physical_action_committed action=37' \
    'sophia_live_wm schema=1 status=physical_action_committed action=66' \
    'sophia_live_wm schema=1 status=physical_action_committed action=38' \
    'sophia_live_wm schema=1 status=physical_action_committed action=38' \
    'sophia_live_wm schema=1 status=physical_action_committed action=39' \
    'sophia_live_wm schema=1 status=physical_action_committed action=40' \
    '2026-08-09T00:00:01Z INF hagia event=checkpoint status=saved detail="candidate_nonempty=true"' \
    'sophia_live_wm schema=1 status=physical_action_committed action=33' \
    'sophia_live_wm schema=1 status=physical_action_committed action=34' \
    'hagia_policy_projection schema=1 status=active_output_changed' \
    'sophia_live_session_input schema=2 status=complete source=physical text=hagiapolicyproof expected_events=34 matched_events=34 pixel_change=true' \
    'sophia_live_session schema=16 status=bounded_complete physical_input=enabled native_in_flight=false native_cleanup_pending=false native_submit_failures=0 wm_restarts=1 wm_degraded=false complete=true' \
    'sophia_live_session_health schema=1 status=clean protocol_errors=0 pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false' \
    'sophia_live_output_topology_health schema=1 status=clean quarantined=false' \
    'sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 namespace=revoked xauthority=removed' \
    >"$evidence"

set +e
env \
    SOPHIA_HAGIA_BIN=/usr/bin/sleep \
    SOPHIA_HAGIA_RESTART_MARKER="$marker" \
    SOPHIA_HAGIA_RESTART_AFTER_ACTION=66 \
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

"$root_dir/tools/verify_hagia_policy_physical.sh" "$evidence" hagiapolicyproof >/dev/null

printf '%s\n' 'Hagia physical matchers accepted structured checkpoint evidence.'
