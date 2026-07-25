#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DISPLAY_NAME="${SOPHIA_XMONAD_LIVE_DISPLAY:-:277}"
EVIDENCE_FILE="${SOPHIA_XMONAD_LIVE_EVIDENCE:-/tmp/sophia-xmonad-live-session.log}"
kitty_bin="${SOPHIA_TERMINAL_BIN:-$(command -v kitty || true)}"
if [[ -z "$kitty_bin" || ! -x "$kitty_bin" ]]; then
    echo "real Kitty is required; set SOPHIA_TERMINAL_BIN" >&2
    exit 1
fi

xmonad_bin="$("$ROOT_DIR/tools/resolve_sophia_xmonad.sh")"

cd "$ROOT_DIR"
cargo build --quiet --offline -p sophia-cli --features atomic-scanout-live
cargo build --quiet --offline -p sophia-x11-wm-bridge

set +e
target/debug/sophia sophia-live-session \
    --session-mode=normal \
    "--session-app=terminal=$kitty_bin" \
    --session-start=terminal \
    --session-action-app=terminal=terminal \
    --session-app-arg=terminal=--config \
    --session-app-arg=terminal=NONE \
    --session-app-arg=terminal=--override \
    --session-app-arg=terminal=linux_display_server=x11 \
    --session-app-arg=terminal=--override \
    --session-app-arg=terminal=background_opacity=1 \
    --session-app-arg=terminal=--title \
    --session-app-arg=terminal=Sophia-xmonad-resize-proof \
    --startup-ready-timeout-ms=8000 \
    --display="$DISPLAY_NAME" \
    --max-runtime-ms=12000 \
    --inject-surface-resize=960x640 \
    --inject-text=sophia \
    --exit-after-input-proof \
    --wm-process="$ROOT_DIR/target/debug/sophia-x11-wm-bridge" \
    --wm-process-arg="--wm=$xmonad_bin" \
    --wm-process-arg=--profile=xmonad \
    --wm-process-arg=--wm-private-alias=xmonad/xmonad-x86_64-linux \
    >"$EVIDENCE_FILE" 2>&1
status=$?
set -e
cat "$EVIDENCE_FILE"
if (( status != 0 )); then
    exit "$status"
fi

grep -Eq '^sophia_live_wm schema=1 status=layout_committed .* moved_surfaces=1 configure_deliveries=1 outcome=Committed$' "$EVIDENCE_FILE"
grep -Eq '^sophia_live_session_present schema=2 status=retired .* source=960x640 .* unit_scale=true$' "$EVIDENCE_FILE"
if grep -Eq '^sophia_live_session_present schema=2 .* unit_scale=false$' "$EVIDENCE_FILE"; then
    echo "real Kitty resize presented mismatched source pixels" >&2
    exit 1
fi
grep -Eq '^sophia_session_app schema=1 status=started id=terminal source=startup$' "$EVIDENCE_FILE"
grep -Eq '^sophia_live_session_protocol_errors schema=1 expected=[0-9]+ unexpected=0$' "$EVIDENCE_FILE"
completion="$(grep -E '^sophia_live_session schema=(14|15) status=bounded_complete ' "$EVIDENCE_FILE")"
[[ " $completion " == *" startup_ready_msec="* ]]
[[ " $completion " == *" cpu_nonzero_frames="* ]]
[[ " $completion " == *" surface_resize=committed "* ]]
[[ " $completion " == *" input_pixel_change=true "* ]]
[[ " $completion " == *" input_text_match=true "* ]]
[[ " $completion " == *" wm_policy=external "* ]]
[[ " $completion " == *" wm_degraded=false "* ]]
[[ " $completion " == *" present_live_sources=0 "* ]]
[[ " $completion " == *" present_live_fences=0 "* ]]
[[ " $completion " == *" present_live_transactions=0 "* ]]

echo "real Kitty xmonad resize smoke passed: $EVIDENCE_FILE"
