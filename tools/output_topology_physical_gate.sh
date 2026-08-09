#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
hagia_bin="${SOPHIA_HAGIA_BIN:-$(command -v hagia || true)}"
kitty_bin="${SOPHIA_TERMINAL_BIN:-$(command -v kitty || true)}"
seat="${SOPHIA_OUTPUT_TOPOLOGY_SEAT:-}"
display="${SOPHIA_OUTPUT_TOPOLOGY_DISPLAY:-:293}"
runtime_msec="${SOPHIA_OUTPUT_TOPOLOGY_RUNTIME_MSEC:-180000}"
evidence="${SOPHIA_OUTPUT_TOPOLOGY_EVIDENCE:-/tmp/sophia-output-topology-physical.log}"
proof_text="${SOPHIA_OUTPUT_TOPOLOGY_TEXT:-topology-proof}"

if [[ "${SOPHIA_OUTPUT_TOPOLOGY_ARM:-0}" != "1" ]]; then
    echo "set SOPHIA_OUTPUT_TOPOLOGY_ARM=1 to acknowledge exclusive DRM/input use and physical connector changes" >&2
    exit 2
fi
if [[ -z "$seat" ]]; then
    echo "set SOPHIA_OUTPUT_TOPOLOGY_SEAT to the libinput seat (normally seat0)" >&2
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
if [[ ! "$runtime_msec" =~ ^[0-9]+$ ]] || (( runtime_msec < 60000 )); then
    echo "SOPHIA_OUTPUT_TOPOLOGY_RUNTIME_MSEC must be at least 60000" >&2
    exit 2
fi

connected_outputs=0
for status in /sys/class/drm/card*-*/status; do
    [[ -r "$status" ]] || continue
    if [[ "$(<"$status")" == "connected" ]]; then
        connected_outputs=$((connected_outputs + 1))
    fi
done
if (( connected_outputs != 2 )); then
    echo "connect exactly two physical outputs before running this gate (observed $connected_outputs)" >&2
    exit 2
fi

echo "Sophia physical output-topology gate"
echo "This takes exclusive DRM/KMS and seat input. Evidence: $evidence"
echo "After Kitty is visibly settled on two outputs:"
echo "  1. Disconnect one non-primary output and wait for the desktop to settle."
echo "  2. Reconnect that output and wait for the desktop to settle again."
echo "  3. Confirm Kitty survived, then type '$proof_text' into Kitty."

RUST_LOG="${RUST_LOG:-sophia=info,sophia_backend_live=info}" \
SOPHIA_HAGIA_BIN="$hagia_bin" \
SOPHIA_LIVE_SESSION_DISPLAY="$display" \
SOPHIA_LIVE_SESSION_RUNTIME_MSEC="$runtime_msec" \
SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$evidence" \
    "$ROOT_DIR/tools/live_session_persistent_hardware_proof.sh" \
    --no-config \
    --session-mode=normal \
    "--session-app=terminal=$kitty_bin" \
    --session-start=terminal \
    --session-app-arg=terminal=--config \
    --session-app-arg=terminal=NONE \
    --session-app-arg=terminal=--override \
    --session-app-arg=terminal=linux_display_server=x11 \
    "--wm-process=$hagia_bin" \
    --wm-interface=sophia_wm_v1 \
    "--input-seat=$seat" \
    "--expect-physical-text=$proof_text" \
    --exit-after-input-proof

require_count() {
    local pattern="$1"
    local minimum="$2"
    local description="$3"
    local count
    count="$(grep -Ec "$pattern" "$evidence" || true)"
    if (( count < minimum )); then
        echo "$description: expected at least $minimum, observed $count" >&2
        exit 1
    fi
}

require_count 'sophia_live_input_epoch schema=1 reason=output_topology' 2 \
    'topology security-epoch advances'
require_count 'sophia_live_output_topology schema=1 status=published .*changed=true' 2 \
    'changed topology publications'
require_count 'sophia_live_output_topology schema=1 status=policy_committed' 2 \
    'topology policy settlements'
require_count 'sophia_live_output_topology schema=1 status=settled .*input=enabled' 2 \
    'post-policy topology presentations'

if ! grep -Eq 'sophia_live_output_topology schema=1 status=published .*generation=2 .*outputs=1 ' "$evidence"; then
    echo "one-output loss publication is missing" >&2
    exit 1
fi
if ! grep -Eq 'sophia_live_output_topology schema=1 status=published .*generation=3 .*outputs=2 ' "$evidence"; then
    echo "two-output return publication is missing or did not advance generation" >&2
    exit 1
fi
if ! grep -Eq '^sophia_live_output_topology_health schema=1 status=clean quarantined=false$' "$evidence"; then
    echo "clean settled topology health evidence is missing" >&2
    exit 1
fi

echo "Sophia physical output-topology gate passed"
