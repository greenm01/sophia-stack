#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_qemu_xmonad_interactive_evidence.sh"
FIXTURE="$ROOT_DIR/tools/fixtures/qemu_xmonad_interactive_pass.log"
MUTATION="$(mktemp)"
RAW_TRACE="$(mktemp)"
REDUCED="$(mktemp)"
trap 'rm -f -- "$MUTATION" "$RAW_TRACE" "$REDUCED"' EXIT

"$VERIFY" "$FIXTURE" >/dev/null
python3 "$ROOT_DIR/tools/check_qemu_rfb_input.py" >/dev/null

for pattern in \
    'status=display_attached' \
    'status=host_input_delivered kind=pointer' \
    'status=qemu_input_delivered kind=button' \
    'status=motion_routed' \
    'status=focus_requested source=pointer' \
    'status=focused_key_routed' \
    'status=key_routed' \
    'status=physical_action_committed' \
    'action=CloseFocused' \
    'action=Logout' \
    'status=clean app_groups=0'; do
    grep -v "$pattern" "$FIXTURE" >"$MUTATION"
    if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
        echo "interactive verifier accepted evidence missing: $pattern" >&2
        exit 1
    fi
done

sed '/status=ready actions=freeform/a input_event_key_qcode secret_typed_value' \
    "$FIXTURE" >"$MUTATION"
if "$VERIFY" "$MUTATION" >/dev/null 2>&1; then
    echo "interactive verifier accepted a retained raw input trace" >&2
    exit 1
fi

sed '/status=viewer_started/a vnc_client_connect dummy\nvnc_msg_client_key_event secret\ninput_event_key_qcode secret\nvnc_msg_client_pointer_event secret\ninput_event_rel secret\ninput_event_btn secret' \
    "$FIXTURE" >"$RAW_TRACE"
"$ROOT_DIR/tools/reduce_qemu_interactive_trace.sh" "$RAW_TRACE" "$REDUCED" >/dev/null
for marker in display_attached host_input_delivered qemu_input_delivered; do
    grep -q "status=$marker" "$REDUCED"
done
if grep -q 'secret' "$REDUCED"; then
    echo "interactive trace reducer retained input content" >&2
    exit 1
fi

grep -Fq 'sophia.scenario=xmonad-interactive' "$ROOT_DIR/tools/qemu_guest_init.sh"
grep -Fq 'mode=interactive proof_watchdog=off fault_injection=off' \
    "$ROOT_DIR/tools/qemu_guest_init.sh"
grep -Fq 'SOPHIA_QEMU_SCENARIO=xmonad-interactive' \
    "$ROOT_DIR/tools/qemu_xmonad_interactive.sh"
grep -Fq 'verify_qemu_xmonad_interactive_evidence.sh' \
    "$ROOT_DIR/tools/qemu_session_harness.sh"
grep -Fq 'machine+=",vmport=off"' "$ROOT_DIR/tools/qemu_session_harness.sh"

echo "QEMU xmonad interactive verifier regressions passed."
