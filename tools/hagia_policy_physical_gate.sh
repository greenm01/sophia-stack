#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
hagia_bin="${SOPHIA_HAGIA_BIN:-$(command -v hagia || true)}"
kitty_bin="${SOPHIA_TERMINAL_BIN:-$(command -v kitty || true)}"
browser_bin="${SOPHIA_BROWSER_BIN:-${SOPHIA_FIREFOX_BIN:-}}"
if [[ -z "$browser_bin" ]]; then
    browser_bin="$(command -v helium || command -v firefox || true)"
fi
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
if [[ -z "$browser_bin" || ! -x "$browser_bin" ]]; then
    echo "set SOPHIA_BROWSER_BIN to an executable browser" >&2
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
echo "  2. Press Super+N once; Hagia will checkpoint the new layout and restart."
echo "  3. After the scene returns, confirm fullscreen and the layout survived."
echo "  4. Press Super+Y, Super+N, Super+M twice, Super+I, Super+R, Super+Left, Super+Right."
echo "  5. Only after step 4, type '$proof_text' and press Enter."
echo "     The phrase is the final signal and ends the session immediately."

rm -f "$restart_marker"
trap 'rm -f "$restart_marker"' EXIT

SOPHIA_HAGIA_BIN="$hagia_bin" \
SOPHIA_HAGIA_RESTART_MARKER="$restart_marker" \
SOPHIA_HAGIA_RESTART_REQUIRES_ACTION=37 \
SOPHIA_HAGIA_RESTART_AFTER_ACTION=66 \
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
    "--session-app=browser=$browser_bin" \
    --session-action-app=firefox=browser \
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

"$ROOT_DIR/tools/verify_hagia_policy_physical.sh" "$evidence" "$proof_text"
SOPHIA_HAGIA_BIN="$hagia_bin" \
    "$ROOT_DIR/tools/archive_hagia_policy_physical_run.sh" \
    "$evidence" "$proof_text"
echo "Hagia physical policy gate passed"
