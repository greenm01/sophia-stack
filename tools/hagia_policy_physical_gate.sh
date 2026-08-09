#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
hagia_bin="${SOPHIA_HAGIA_BIN:-$(command -v hagia || true)}"
kitty_bin="${SOPHIA_TERMINAL_BIN:-$(command -v kitty || true)}"
firefox_bin="${SOPHIA_FIREFOX_BIN:-$(command -v firefox || true)}"
seat="${SOPHIA_HAGIA_PHYSICAL_SEAT:-}"
display="${SOPHIA_HAGIA_PHYSICAL_DISPLAY:-:291}"
runtime_msec="${SOPHIA_HAGIA_PHYSICAL_RUNTIME_MSEC:-120000}"
evidence="${SOPHIA_HAGIA_PHYSICAL_EVIDENCE:-/tmp/sophia-hagia-policy-physical.log}"
proof_text="${SOPHIA_HAGIA_PHYSICAL_TEXT:-hagia-policy-proof}"
restart_marker="${evidence}.restart"

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
if [[ ! "$runtime_msec" =~ ^[0-9]+$ ]] || (( runtime_msec < 30000 )); then
    echo "SOPHIA_HAGIA_PHYSICAL_RUNTIME_MSEC must be at least 30000" >&2
    exit 2
fi

echo "Hagia installed physical policy gate"
echo "This takes exclusive DRM/KMS and seat input. Evidence: $evidence"
echo "Use two connected outputs. After Kitty appears:"
echo "  1. Press Super+Y once; confirm fullscreen."
echo "  2. Press Super+Right once; Hagia will checkpoint and restart."
echo "  3. After the scene returns, confirm fullscreen survived."
echo "  4. Press Super+Y, Super+M twice, Super+N, Super+R, Super+Left, Super+Right."
echo "  5. Type '$proof_text' into Kitty. The input proof ends the session."

rm -f "$restart_marker"
trap 'rm -f "$restart_marker"' EXIT

SOPHIA_HAGIA_BIN="$hagia_bin" \
SOPHIA_HAGIA_RESTART_MARKER="$restart_marker" \
SOPHIA_HAGIA_FAULT_AFTER=checkpoint_saved \
SOPHIA_HAGIA_FAULT_OCCURRENCE=4 \
SOPHIA_HAGIA_FAULT_DELAY_MSEC=200 \
SOPHIA_LIVE_SESSION_DISPLAY="$display" \
SOPHIA_LIVE_SESSION_RUNTIME_MSEC="$runtime_msec" \
SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$evidence" \
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
    "--wm-process=$ROOT_DIR/tools/fixtures/hagia_restart_once.sh" \
    --wm-interface=sophia_wm_v1 \
    "--input-seat=$seat" \
    "--expect-physical-text=$proof_text" \
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

before_restart '^sophia_live_wm schema=1 status=physical_action_committed action=37$'
before_restart '^sophia_live_wm schema=1 status=physical_action_committed action=34$'
before_restart '^hagia_policy_checkpoint schema=1 status=saved candidate_nonempty=true$'
before_restart '^hagia_policy_projection schema=1 status=active_output_changed$'

after_restart '^hagia_policy_checkpoint schema=1 status=loaded candidate_nonempty=true$'
after_restart '^hagia_policy_checkpoint schema=1 status=reconciled candidate_nonempty=true$'
after_restart '^hagia_policy_refresh schema=1 status=requested reason=checkpoint_reconciled policy_generation=2 outputs=2$'
for action in 37 39 40 33 34; do
    after_restart "^sophia_live_wm schema=1 status=physical_action_committed action=$action$"
done
maximize_count="$(awk -v limit="$restart_line" \
    'NR > limit && /^sophia_live_wm schema=1 status=physical_action_committed action=38$/ { count++ } END { print count + 0 }' \
    "$evidence")"
if (( maximize_count < 2 )); then
    echo "two post-restart maximize transitions were not committed" >&2
    exit 1
fi
after_restart '^hagia_policy_projection schema=1 status=active_output_changed$'

if grep -Eq '^hagia_policy_checkpoint schema=1 status=(discarded|disabled) ' "$evidence"; then
    echo "Hagia checkpoint was discarded or disabled" >&2
    exit 1
fi

echo "Hagia physical policy gate passed"
