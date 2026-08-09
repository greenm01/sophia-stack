#!/usr/bin/env bash
set -euo pipefail

evidence="${1:?usage: verify_hagia_policy_physical.sh EVIDENCE [PROOF_TEXT]}"
proof_text="${2:-hagiapolicyproof}"

[[ -s "$evidence" ]] || {
    echo "Hagia physical policy evidence is missing: $evidence" >&2
    exit 1
}
[[ "$proof_text" =~ ^[a-z]{1,24}$ ]] || {
    echo "Hagia physical proof text must contain 1-24 lowercase ASCII letters" >&2
    exit 2
}

restart_line="$(awk \
    '/^sophia_live_wm schema=4 status=restarted adapter=sophia_wm_v1 epoch=2 restarts=1 preserved_layout=true$/ { line = NR } END { if (line) print line }' \
    "$evidence")"
if [[ -z "$restart_line" ]]; then
    echo "Hagia restart evidence is missing" >&2
    exit 1
fi

before_restart() {
    local pattern="$1"
    awk -v limit="$restart_line" -v pattern="$pattern" \
        'NR < limit && $0 ~ pattern { found = 1 } END { exit found ? 0 : 1 }' "$evidence"
}

after_restart() {
    local pattern="$1"
    awk -v limit="$restart_line" -v pattern="$pattern" \
        'NR > limit && $0 ~ pattern { found = 1 } END { exit found ? 0 : 1 }' "$evidence"
}

require_before() {
    local description="$1" pattern="$2"
    if ! before_restart "$pattern"; then
        echo "Hagia physical policy evidence is missing pre-restart evidence: $description" >&2
        exit 1
    fi
}

require_after() {
    local description="$1" pattern="$2"
    if ! after_restart "$pattern"; then
        echo "Hagia physical policy evidence is missing post-restart evidence: $description" >&2
        exit 1
    fi
}

require_line() {
    local description="$1" pattern="$2"
    if ! grep -Eq "$pattern" "$evidence"; then
        echo "Hagia physical policy evidence is missing session evidence: $description" >&2
        exit 1
    fi
}

require_before "fullscreen action" \
    '^sophia_live_wm schema=1 status=physical_action_committed action=37$'
require_before "active-output action" \
    '^sophia_live_wm schema=1 status=physical_action_committed action=34$'
require_before "nonempty checkpoint" \
    '^hagia_policy_checkpoint schema=1 status=saved candidate_nonempty=true$'
require_before "active-output projection" \
    '^hagia_policy_projection schema=1 status=active_output_changed$'

require_after "checkpoint load" \
    '^hagia_policy_checkpoint schema=1 status=loaded candidate_nonempty=true$'
require_after "checkpoint reconciliation" \
    '^hagia_policy_checkpoint schema=1 status=reconciled candidate_nonempty=true$'
require_after "generation-2 policy refresh" \
    '^hagia_policy_refresh schema=1 status=requested reason=checkpoint_reconciled policy_generation=2 outputs=2$'
for action in 37 39 40 33 34; do
    require_after "physical action $action" \
        "^sophia_live_wm schema=1 status=physical_action_committed action=$action$"
done

restore_line="$(awk -v limit="$restart_line" \
    'NR > limit && /^sophia_live_wm schema=1 status=physical_action_committed action=40$/ { print NR; exit }' \
    "$evidence")"
if [[ -z "$restore_line" ]] || ! awk -v limit="$restore_line" \
    'NR > limit && /^hagia_policy_checkpoint schema=1 status=saved candidate_nonempty=true$/ { found = 1; exit } END { exit found ? 0 : 1 }' \
    "$evidence"; then
    echo "Hagia restore did not retain a nonempty policy checkpoint" >&2
    exit 1
fi

maximize_count="$(awk -v limit="$restart_line" \
    'NR > limit && /^sophia_live_wm schema=1 status=physical_action_committed action=38$/ { count++ } END { print count + 0 }' \
    "$evidence")"
if (( maximize_count < 2 )); then
    echo "two post-restart maximize transitions were not committed" >&2
    exit 1
fi
require_after "active-output projection" \
    '^hagia_policy_projection schema=1 status=active_output_changed$'

require_line "exact physical text completion" \
    "^sophia_live_session_input schema=2 status=complete source=physical text=$proof_text expected_events=[1-9][0-9]* matched_events=[1-9][0-9]* pixel_change=true$"
require_line "bounded session completion" \
    '^sophia_live_session schema=16 status=bounded_complete .* native_in_flight=false native_cleanup_pending=false .* wm_restarts=1 wm_degraded=false '
require_line "clean session health" \
    '^sophia_live_session_health schema=1 status=clean protocol_errors=0 pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false$'
require_line "clean output topology" \
    '^sophia_live_output_topology_health schema=1 status=clean quarantined=false$'
require_line "clean process cleanup" \
    '^sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 namespace=revoked xauthority=removed$'

if grep -Eq '^hagia_policy_checkpoint schema=1 status=(discarded|disabled) ' "$evidence"; then
    echo "Hagia checkpoint was discarded or disabled" >&2
    exit 1
fi

echo "Hagia physical policy evidence passed"
