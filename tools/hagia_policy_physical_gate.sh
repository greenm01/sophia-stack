#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
hagia_bin="${SOPHIA_HAGIA_BIN:-$(command -v hagia || true)}"
kitty_bin="${SOPHIA_TERMINAL_BIN:-$(command -v kitty || true)}"
firefox_bin="${SOPHIA_FIREFOX_BIN:-$(command -v firefox || true)}"
seat="${SOPHIA_HAGIA_PHYSICAL_SEAT:-}"
display="${SOPHIA_HAGIA_PHYSICAL_DISPLAY:-:291}"
runtime_msec="${SOPHIA_HAGIA_PHYSICAL_RUNTIME_MSEC:-660000}"
sequence_timeout_msec="${SOPHIA_HAGIA_PHYSICAL_SEQUENCE_TIMEOUT_MSEC:-600000}"
evidence="${SOPHIA_HAGIA_PHYSICAL_EVIDENCE:-/tmp/sophia-hagia-policy-physical.log}"
proof_text="${SOPHIA_HAGIA_PHYSICAL_TEXT:-hagiapolicyproof}"
restart_marker="${evidence}.restart"
guide="${SOPHIA_HAGIA_PHYSICAL_GUIDE:-$ROOT_DIR/tools/fixtures/hagia_physical_guide.sh}"

if [[ ! "$proof_text" =~ ^[a-z]{1,24}$ ]]; then
    echo "SOPHIA_HAGIA_PHYSICAL_TEXT must contain 1-24 lowercase ASCII letters" >&2
    exit 2
fi
if [[ "${SOPHIA_HAGIA_PHYSICAL_ARM:-0}" != "1" ]]; then
    echo "set SOPHIA_HAGIA_PHYSICAL_ARM=1 to acknowledge exclusive DRM/input use" >&2
    exit 2
fi
if [[ -z "$seat" ]]; then
    echo "set SOPHIA_HAGIA_PHYSICAL_SEAT to the libinput seat (normally seat0)" >&2
    exit 2
fi
if [[ -z "$hagia_bin" || ! -x "$hagia_bin" ]]; then
    echo "set SOPHIA_HAGIA_BIN to a built Hagia executable" >&2
    exit 2
fi
if [[ -z "$kitty_bin" || ! -x "$kitty_bin" ]]; then
    echo "set SOPHIA_TERMINAL_BIN to real Kitty" >&2
    exit 2
fi
if [[ -z "$firefox_bin" || ! -x "$firefox_bin" ]]; then
    echo "set SOPHIA_FIREFOX_BIN to real Firefox" >&2
    exit 2
fi
if [[ ! -x "$guide" ]]; then
    echo "set SOPHIA_HAGIA_PHYSICAL_GUIDE to the executable proof guide" >&2
    exit 2
fi
if [[ ! "$runtime_msec" =~ ^[0-9]+$ ]] || (( runtime_msec < 30000 )); then
    echo "SOPHIA_HAGIA_PHYSICAL_RUNTIME_MSEC must be at least 30000" >&2
    exit 2
fi
if [[ ! "$sequence_timeout_msec" =~ ^[0-9]+$ ]] \
    || (( sequence_timeout_msec < 1000 || sequence_timeout_msec > 600000 )); then
    echo "SOPHIA_HAGIA_PHYSICAL_SEQUENCE_TIMEOUT_MSEC must be 1000-600000" >&2
    exit 2
fi

echo "Hagia installed physical policy gate"
echo "This takes exclusive DRM/KMS and seat input. Evidence: $evidence"
echo "Use two connected outputs. After Kitty appears:"
echo "  1. Press Super+Y once; confirm fullscreen."
echo "  2. Press Super+Right once; Hagia will checkpoint and restart."
echo "  3. After the scene returns, confirm fullscreen survived."
echo "  4. Press Super+Y, Super+M twice, Super+N, Super+R, Super+Left, Super+Right."
echo "  5. Only after step 4, type '$proof_text' and press Enter."
echo "     The phrase is the final signal and ends the session immediately."

rm -f "$restart_marker"
trap 'rm -f "$restart_marker"' EXIT

SOPHIA_HAGIA_BIN="$hagia_bin" \
SOPHIA_HAGIA_RESTART_MARKER="$restart_marker" \
SOPHIA_HAGIA_RESTART_REQUIRES_ACTION=37 \
SOPHIA_HAGIA_RESTART_AFTER_ACTION=34 \
SOPHIA_LIVE_SESSION_DISPLAY="$display" \
SOPHIA_LIVE_SESSION_RUNTIME_MSEC="$runtime_msec" \
SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$evidence" \
SOPHIA_LIVE_SESSION_VERIFY_MODE=caller \
SOPHIA_HAGIA_PHYSICAL_TEXT="$proof_text" \
    "$ROOT_DIR/tools/live_session_persistent_hardware_proof.sh" \
    --no-config \
    --session-mode=normal \
    "--session-app=terminal=$kitty_bin" \
    --session-start=terminal \
    --session-action-app=terminal=terminal \
    "--session-app=firefox=$firefox_bin" \
    --session-action-app=firefox=firefox \
    --session-app-arg=terminal=--config \
    --session-app-arg=terminal=NONE \
    --session-app-arg=terminal=--override \
    --session-app-arg=terminal=linux_display_server=x11 \
    --session-app-arg=terminal=--override \
    --session-app-arg=terminal=remember_window_size=no \
    "--session-app-arg=terminal=$guide" \
    "--wm-process=$ROOT_DIR/tools/fixtures/hagia_restart_once.sh" \
    --wm-interface=sophia_wm_v1 \
    "--input-seat=$seat" \
    "--expect-physical-text=$proof_text" \
    "--physical-sequence-timeout-ms=$sequence_timeout_msec" \
    --exit-after-input-proof

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
    local description="$1"
    local pattern="$2"
    if ! before_restart "$pattern"; then
        echo "Hagia physical policy gate is missing pre-restart evidence: $description" >&2
        exit 1
    fi
}

require_after() {
    local description="$1"
    local pattern="$2"
    if ! after_restart "$pattern"; then
        echo "Hagia physical policy gate is missing post-restart evidence: $description" >&2
        exit 1
    fi
}

require_line() {
    local description="$1"
    local pattern="$2"
    if ! grep -Eq "$pattern" "$evidence"; then
        echo "Hagia physical policy gate is missing session evidence: $description" >&2
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

echo "Hagia physical policy gate passed"
