#!/usr/bin/env bash
set -euo pipefail

evidence="${1:?usage: verify_hagia_policy_physical.sh EVIDENCE [PROOF_TEXT]}"
proof_text="${2:-hagiapolicyproof}"
checkpoint_saved='(^hagia_policy_checkpoint schema=1 status=saved candidate_nonempty=true$| event=checkpoint status=saved detail="candidate_nonempty=true"$)'
checkpoint_loaded='(^hagia_policy_checkpoint schema=1 status=loaded candidate_nonempty=true$| event=checkpoint status=loaded detail="candidate_nonempty=true"$)'
checkpoint_reconciled='(^hagia_policy_checkpoint schema=1 status=reconciled candidate_nonempty=true$| event=checkpoint status=reconciled detail="candidate_nonempty=true"$)'
active_output_changed='(^hagia_policy_projection schema=1 status=active_output_changed$| event=projection status=active_output_changed detail=$)'
policy_refresh='(^hagia_policy_refresh schema=1 status=requested reason=checkpoint_reconciled policy_generation=2 outputs=2$| event=policy_refresh status=requested detail=checkpoint_reconciled$)'

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

if [[ "$(grep -Ec '^sophia_hagia_policy_identity schema=2 status=bound sophia_commit=[0-9a-f]{40} hagia_commit=[0-9a-f]{40} sophia_sha256=[0-9a-f]{64} hagia_sha256=[0-9a-f]{64} hagia_shell_sha256=[0-9a-f]{64}$' "$evidence" || true)" != 1 ]]; then
    echo "Hagia physical policy evidence lacks one exact Sophia/Hagia/Hagia Shell identity" >&2
    exit 1
fi
if [[ "$(grep -Ec '^sophia_live_metadata_broker schema=1 status=ready protected=true peer_pid=[1-9][0-9]* revision=1$' "$evidence" || true)" != 1 ]]; then
    echo "Hagia physical policy evidence lacks one protected metadata-broker admission" >&2
    exit 1
fi
if ! grep -Eq '^sophia_live_metadata_broker schema=1 status=descriptor_committed surface=[0-9]+ content=redacted$' "$evidence"; then
    echo "Hagia physical policy evidence lacks a redacted descriptor commit" >&2
    exit 1
fi
if [[ "$(grep -Ec '^sophia_live_metadata_broker schema=1 status=stopped transport=disconnected process=terminated$' "$evidence" || true)" != 1 ]]; then
    echo "Hagia physical policy evidence lacks one clean metadata-broker shutdown" >&2
    exit 1
fi
if grep -Eq '(protected metadata broker exited|^sophia_live_metadata_broker schema=1 status=failed )' "$evidence"; then
    echo "Hagia physical policy evidence contains a metadata-broker failure" >&2
    exit 1
fi
broker_ready_line="$(grep -nEm1 '^sophia_live_metadata_broker schema=1 status=ready ' "$evidence" | cut -d: -f1)"
broker_descriptor_line="$(grep -nEm1 '^sophia_live_metadata_broker schema=1 status=descriptor_committed ' "$evidence" | cut -d: -f1)"
broker_stopped_line="$(grep -nEm1 '^sophia_live_metadata_broker schema=1 status=stopped ' "$evidence" | cut -d: -f1)"
if (( broker_ready_line >= broker_descriptor_line || broker_descriptor_line >= broker_stopped_line )); then
    echo "Hagia metadata-broker lifecycle is not ready -> descriptor -> stopped" >&2
    exit 1
fi

if (( $(grep -Ec '^sophia_live_metadata_broker schema=1 status=descriptor_committed surface=[0-9]+ content=redacted$' "$evidence" || true) < 2 )); then
    echo "Hagia physical policy evidence lacks two switcher descriptors" >&2
    exit 1
fi
if [[ "$(grep -Ec '^sophia_live_metadata_shell schema=1 status=ready protected=true peer_pid=[1-9][0-9]* revision=1 connection_epoch=1$' "$evidence" || true)" != 1 ]]; then
    echo "Hagia physical policy evidence lacks one protected Hagia Shell admission" >&2
    exit 1
fi
if [[ "$(grep -Ec '^sophia_live_metadata_shell schema=1 status=reconnected protected=true peer_pid=[1-9][0-9]* revision=1 connection_epoch=2 reason=proof_visible_restart$' "$evidence" || true)" != 1 ]]; then
    echo "Hagia physical policy evidence lacks the bounded Hagia Shell reconnect" >&2
    exit 1
fi
if [[ "$(grep -Ec '^sophia_live_metadata_shell schema=1 status=stopped transport=disconnected process=terminated$' "$evidence" || true)" != 1 ]]; then
    echo "Hagia physical policy evidence lacks one clean Hagia Shell shutdown" >&2
    exit 1
fi
for shell_evidence in \
    'proof restart:^sophia_live_metadata_shell schema=1 status=proof_restart_triggered visible_presentation=2 retained_pixels=true$' \
    'inert retained pixels:^sophia_live_metadata_shell schema=1 status=proof_inert_click observed=true activation=false$'; do
    description="${shell_evidence%%:*}"
    pattern="${shell_evidence#*:}"
    require_line "$description" "$pattern"
done
if (( $(grep -Ec '^sophia_live_metadata_shell schema=1 status=shortcut_admitted action=descriptor_switcher$' "$evidence" || true) < 3 \
    || $(grep -Ec '^sophia_live_metadata_shell schema=1 status=presented .* output=[1-9][0-9]* visible=true$' "$evidence" || true) < 3 \
    || $(grep -Ec '^sophia_live_metadata_shell schema=1 status=presented .* output=[1-9][0-9]* visible=false$' "$evidence" || true) < 2 \
    || $(grep -Ec '^sophia_live_metadata_broker schema=1 status=issuer_validated activation=[1-9][0-9]* target=redacted$' "$evidence" || true) < 2 )); then
    echo "Hagia physical policy evidence lacks the complete switcher presentation sequence" >&2
    exit 1
fi
if [[ "$(grep -Ec '^sophia_live_metadata_shell schema=1 status=activation_admitted activation=[1-9][0-9]* target=redacted$' "$evidence" || true)" -lt 1 \
    || "$(grep -Ec '^sophia_live_metadata_shell schema=1 status=activation_(admitted|duplicate) activation=[1-9][0-9]* target=redacted$' "$evidence" || true)" -lt 2 ]]; then
    echo "Hagia physical policy evidence lacks two issuer-validated switcher activations" >&2
    exit 1
fi
if grep -Eq '(^sophia_live_metadata_shell schema=1 status=(failed|transport_failed|candidate_rejected|activation_rejected|unavailable|disconnect_failed) |protected metadata shell exited)' "$evidence"; then
    echo "Hagia physical policy evidence contains a Hagia Shell failure" >&2
    exit 1
fi

mapfile -t shell_shortcut_lines < <(grep -nE '^sophia_live_metadata_shell schema=1 status=shortcut_admitted action=descriptor_switcher$' "$evidence" | cut -d: -f1)
mapfile -t shell_visible_lines < <(grep -nE '^sophia_live_metadata_shell schema=1 status=presented .* output=[1-9][0-9]* visible=true$' "$evidence" | cut -d: -f1)
mapfile -t shell_hidden_lines < <(grep -nE '^sophia_live_metadata_shell schema=1 status=presented .* output=[1-9][0-9]* visible=false$' "$evidence" | cut -d: -f1)
mapfile -t shell_issuer_lines < <(grep -nE '^sophia_live_metadata_broker schema=1 status=issuer_validated activation=[1-9][0-9]* target=redacted$' "$evidence" | cut -d: -f1)
mapfile -t shell_activation_lines < <(grep -nE '^sophia_live_metadata_shell schema=1 status=activation_(admitted|duplicate) activation=[1-9][0-9]* target=redacted$' "$evidence" | cut -d: -f1)
shell_ready_line="$(grep -nEm1 '^sophia_live_metadata_shell schema=1 status=ready ' "$evidence" | cut -d: -f1)"
shell_restart_line="$(grep -nEm1 '^sophia_live_metadata_shell schema=1 status=proof_restart_triggered ' "$evidence" | cut -d: -f1)"
shell_reconnected_line="$(grep -nEm1 '^sophia_live_metadata_shell schema=1 status=reconnected ' "$evidence" | cut -d: -f1)"
shell_inert_line="$(grep -nEm1 '^sophia_live_metadata_shell schema=1 status=proof_inert_click ' "$evidence" | cut -d: -f1)"
shell_stopped_line="$(grep -nEm1 '^sophia_live_metadata_shell schema=1 status=stopped ' "$evidence" | cut -d: -f1)"
first_shell_admitted_line="$(grep -nEm1 '^sophia_live_metadata_shell schema=1 status=activation_admitted ' "$evidence" | cut -d: -f1)"
if ! (( first_shell_admitted_line == shell_activation_lines[0] \
    && shell_ready_line < shell_shortcut_lines[0] \
    && shell_shortcut_lines[0] < shell_visible_lines[0] \
    && shell_visible_lines[0] < shell_issuer_lines[0] \
    && shell_issuer_lines[0] < shell_activation_lines[0] \
    && shell_activation_lines[0] < shell_hidden_lines[0] \
    && shell_hidden_lines[0] < shell_shortcut_lines[1] \
    && shell_shortcut_lines[1] < shell_visible_lines[1] \
    && shell_visible_lines[1] < shell_restart_line \
    && shell_restart_line < shell_reconnected_line \
    && shell_reconnected_line < shell_inert_line \
    && shell_inert_line < shell_shortcut_lines[2] \
    && shell_shortcut_lines[2] < shell_visible_lines[2] \
    && shell_visible_lines[2] < shell_issuer_lines[1] \
    && shell_issuer_lines[1] < shell_activation_lines[1] \
    && shell_activation_lines[1] < shell_hidden_lines[1] \
    && shell_hidden_lines[1] < shell_stopped_line )); then
    echo "Hagia Shell lifecycle is not present -> activate -> withdraw -> restart -> inert -> activate -> withdraw -> stopped" >&2
    exit 1
fi

if ! awk '
    /^sophia_live_metadata_shell schema=1 status=shortcut_admitted action=descriptor_switcher$/ {
        delete nonzero
        candidate = 1
    }
    candidate && /sophia_live_native_head_page_flip schema=2 status=submitted output=[1-9][0-9]* .*nonzero_rgb_pixels: [1-9][0-9]*/ {
        for (field = 1; field <= NF; field++) {
            if ($field ~ /^output=[1-9][0-9]*$/) {
                output = $field
                sub(/^output=/, "", output)
                nonzero[output] = 1
            }
        }
    }
    candidate && /^sophia_live_metadata_shell schema=1 status=presented .* output=[1-9][0-9]* visible=true$/ {
        output = ""
        for (field = 1; field <= NF; field++) {
            if ($field ~ /^output=[1-9][0-9]*$/) {
                output = $field
                sub(/^output=/, "", output)
            }
        }
        if (output != "" && nonzero[output]) verified++
        candidate = 0
    }
    END { exit verified >= 3 ? 0 : 1 }
' "$evidence"; then
    echo "Hagia Shell switcher did not produce three output-local nonzero presentations" >&2
    exit 1
fi

require_before "fullscreen action" \
    '^sophia_live_wm schema=1 status=physical_action_committed action=37$'
require_before "layout-cycle action" \
    '^sophia_live_wm schema=1 status=physical_action_committed action=66$'
require_before "checkpoint restart arm" \
    '^sophia_live_wm schema=4 status=proof_restart_armed adapter=sophia_wm_v1 boundary=checkpoint_replace action=66$'
require_before "nonempty checkpoint" \
    "$checkpoint_saved"
require_before "checkpoint restart trigger" \
    '^sophia_live_wm schema=4 status=proof_restart_triggered adapter=sophia_wm_v1 phase=checkpoint_saved action=66 preserved_layout=true$'

restart_arm_line="$(awk -v limit="$restart_line" \
    'NR < limit && /^sophia_live_wm schema=4 status=proof_restart_armed adapter=sophia_wm_v1 boundary=checkpoint_replace action=66$/ { print NR; exit }' \
    "$evidence")"
if [[ -z "$restart_arm_line" ]] || ! awk \
    -v lower="$restart_arm_line" -v upper="$restart_line" -v pattern="$checkpoint_saved" \
    'NR > lower && NR < upper && $0 ~ pattern { found = 1; exit } END { exit found ? 0 : 1 }' \
    "$evidence"; then
    echo "Hagia layout cycle was not checkpointed before restart" >&2
    exit 1
fi

require_after "checkpoint load" \
    "$checkpoint_loaded"
require_after "checkpoint reconciliation" \
    "$checkpoint_reconciled"
require_after "generation-2 policy refresh" \
    "$policy_refresh"
for action in 37 66 39 40 5 6 33 34; do
    require_after "physical action $action" \
        "^sophia_live_wm schema=1 status=physical_action_committed action=$action$"
done
require_after "indicator activation for view 2" \
    '^sophia_live_indicator_input schema=1 status=activated output=[1-9][0-9]* action=12$'
require_after "indicator return activation for view 1" \
    '^sophia_live_indicator_input schema=1 status=activated output=[1-9][0-9]* action=11$'
require_after "indicator view-2 policy commit" \
    '^sophia_live_wm schema=1 status=physical_action_committed action=12$'
require_after "indicator view-1 policy commit" \
    '^sophia_live_wm schema=1 status=physical_action_committed action=11$'

move_to_output_line="$(awk -v limit="$restart_line" \
    'NR > limit && /^sophia_live_wm schema=1 status=physical_action_committed action=5$/ { print NR; exit }' \
    "$evidence")"
move_back_line="$(awk -v limit="$move_to_output_line" \
    'NR > limit && /^sophia_live_wm schema=1 status=physical_action_committed action=6$/ { print NR; exit }' \
    "$evidence")"
if [[ -z "$move_to_output_line" || -z "$move_back_line" ]] || ! awk \
    -v lower="$move_to_output_line" -v upper="$move_back_line" \
    'NR > lower && NR < upper && /^.*sophia_live_native_head_page_flip schema=2 status=submitted output=2 .*nonzero_rgb_pixels: [1-9][0-9]*.*$/ { found = 1; exit } END { exit found ? 0 : 1 }' \
    "$evidence"; then
    echo "Hagia output move did not produce a nonzero secondary-head submission" >&2
    exit 1
fi

restore_line="$(awk -v limit="$restart_line" \
    'NR > limit && /^sophia_live_wm schema=1 status=physical_action_committed action=40$/ { print NR; exit }' \
    "$evidence")"
if [[ -z "$restore_line" ]] || ! awk -v limit="$restore_line" \
    'NR > limit && (/^hagia_policy_checkpoint schema=1 status=saved candidate_nonempty=true$/ || / event=checkpoint status=saved detail="candidate_nonempty=true"$/) { found = 1; exit } END { exit found ? 0 : 1 }' \
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
    "$active_output_changed"

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

if grep -Eq '(^hagia_policy_checkpoint schema=1 status=(discarded|disabled) | event=checkpoint status=(discarded|disabled) )' "$evidence"; then
    echo "Hagia checkpoint was discarded or disabled" >&2
    exit 1
fi

echo "Hagia physical policy evidence passed"
