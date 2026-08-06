#!/usr/bin/env bash
set -euo pipefail

evidence="${1:-/tmp/sophia-qemu-xmonad-interactive.log}"

fail() {
    echo "QEMU xmonad interactive verification failed: $*" >&2
    exit 1
}

count() {
    grep -Ec "$1" "$evidence" || true
}

[[ -s "$evidence" ]] || fail "missing evidence: $evidence"
if grep -Eq '(^Error:|panicked at|status=(failed|degraded)([[:space:]]|$))' "$evidence"; then
    fail "evidence contains a Sophia, guest, or harness failure"
fi
if grep -Eq '(vnc_msg_client_|input_event_(key|btn|rel|abs))' "$evidence"; then
    fail "evidence retained a raw host input trace"
fi

grep -Fxq \
    'sophia_qemu_interactive schema=2 status=starting isolation=manual display_backend=vnc-unix control=qmp-unix pointer=virtio-relative vmport=off host_drm=none host_vt=none guest_network=none storage=none proof_watchdog=off fault_injection=off' \
    "$evidence" || fail "the dedicated manual backend did not start"
grep -Fxq \
    'sophia_qemu_xmonad schema=1 status=running windows=1 profile=xmonad mode=interactive proof_watchdog=off fault_injection=off' \
    "$evidence" || fail "the guest did not select the interactive profile"
grep -Fxq \
    'sophia_qemu_interactive schema=1 status=display_attached backend=vnc-unix' \
    "$evidence" || fail "the VNC display was not attached"

for marker in \
    'host_input_delivered kind=pointer' \
    'qemu_input_delivered kind=motion' \
    'qemu_input_delivered kind=button'; do
    grep -Fxq "sophia_qemu_interactive schema=1 status=$marker" "$evidence" ||
        fail "missing reduced host stage: $marker"
done
grep -Eq '^sophia_qemu_interactive schema=2 status=host_input_delivered kind=keyboard count=([89]|[1-9][0-9]+)$' \
    "$evidence" || fail "host keyboard delivery is missing"
grep -Eq '^sophia_qemu_interactive schema=2 status=qemu_input_delivered kind=keyboard count=([89]|[1-9][0-9]+)$' \
    "$evidence" || fail "QEMU keyboard delivery is missing"

grep -Eq '^sophia_live_session_(input_devices schema=1|input_pipeline schema=3 status=poller_ready) .*active=[1-9][0-9]* keyboards=[1-9][0-9]* pointers=[1-9][0-9]* touch=[0-9]+' \
    "$evidence" || fail "virtio keyboard and pointer discovery is missing"
grep -Fxq \
    'sophia_live_session_startup schema=2 status=output_baseline_ready outputs=2/2' \
    "$evidence" || fail "both guest output baselines were not presented"
grep -Eq '^sophia_live_wm schema=2 status=workspace_projection_committed .* output=[0-9]+ .*visible_surfaces=1 focus=surface$' \
    "$evidence" || fail "the focused startup display target is missing"

grep -Eq '^sophia_live_session_pointer schema=(1|2) status=visible source=(physical|hardware_cursor)( position=center)?$' \
    "$evidence" || fail "the guest pointer was not visible"
for marker in \
    'sophia_live_session_pointer schema=2 status=motion_observed' \
    'sophia_live_session_pointer schema=2 status=motion_routed'; do
    grep -Fxq "$marker" "$evidence" || fail "missing pointer stage: $marker"
done
grep -Eq '^sophia_live_session_pointer schema=2 status=button_observed count=[1-9][0-9]*$' \
    "$evidence" || fail "a physical button was not observed"
grep -Eq '^sophia_live_session_pointer schema=2 status=button_routed count=[1-9][0-9]*$' \
    "$evidence" || fail "a physical button was not routed"
grep -Eq '^sophia_live_wm schema=3 status=focus_requested source=pointer surface=[0-9]+$' \
    "$evidence" || fail "the pointer did not select a managed client"
grep -Eq '^sophia_live_session_pointer schema=6 status=focused_key_routed surface=[0-9]+$' \
    "$evidence" || fail "typed input did not reach the pointer-focused client"

(( $(count '^sophia_session_app schema=(1|2) status=started id=terminal source=action') >= 1 )) ||
    fail "Super+Return did not launch a terminal"
grep -Fxq 'sophia_live_session_input_pipeline schema=1 status=key_observed' "$evidence" ||
    fail "typed input was not observed"
grep -Fxq 'sophia_live_session_input_pipeline schema=1 status=key_routed' "$evidence" ||
    fail "typed input was not routed"
grep -Eq '^sophia_live_wm schema=1 status=physical_action_committed action=' "$evidence" ||
    fail "a physical focus action did not commit"
grep -Fxq 'sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control' \
    "$evidence" || fail "the focus change did not reach Engine"
grep -Eq '^sophia_live_wm schema=1 status=session_action_committed .* action=CloseFocused$' \
    "$evidence" || fail "the application close did not commit"
grep -Eq '^sophia_session_app schema=1 status=exited id=terminal source=managed exit_status=exit status: 0$' \
    "$evidence" || fail "the launched terminal did not close normally"

awk '
    /status=ready actions=freeform / { phase = 1; next }
    phase == 1 && /^sophia_session_app schema=(1|2) status=started id=terminal source=action/ {
        phase = 2
        next
    }
    phase == 2 && /^sophia_live_wm schema=3 status=focus_requested source=pointer surface=/ {
        phase = 3
        next
    }
    phase == 3 && /^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$/ {
        phase = 4
        next
    }
    phase == 4 && /^sophia_live_session_pointer schema=6 status=focused_key_routed surface=/ {
        phase = 5
        next
    }
    phase == 5 && /^sophia_live_wm schema=1 status=physical_action_committed action=/ {
        phase = 6
        next
    }
    phase == 6 && /^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$/ {
        phase = 7
        next
    }
    phase == 7 && /^sophia_live_wm schema=1 status=session_action_committed .* action=CloseFocused$/ {
        phase = 8
        next
    }
    phase == 8 && /^sophia_session_app schema=1 status=exited id=terminal source=managed exit_status=exit status: 0$/ {
        phase = 9
        next
    }
    phase == 9 && /^sophia_live_wm schema=1 status=session_action_committed .* action=Logout$/ {
        phase = 10
        next
    }
    END { if (phase != 10) exit 1 }
' "$evidence" || fail "launch, pointer focus, typing, keyboard focus, close, and manual logout are out of order"

grep -Fxq \
    'sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=none' \
    "$evidence" || fail "native presentation did not drain"
grep -Fxq \
    'sophia_live_session_health schema=1 status=clean protocol_errors=0 pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false' \
    "$evidence" || fail "final session health is not clean"
grep -Eq '^sophia_live_session_protocol_errors schema=1 expected=[0-9]+ unexpected=0$' \
    "$evidence" || fail "unexpected protocol errors were observed"
grep -Eq '^sophia_live_session schema=16 status=bounded_complete .*physical_keys_routed=[1-9][0-9]* .*native_submit_failures=0 .*native_retire_failures=0 .*native_callback_rejected=0 .*native_in_flight=false native_cleanup_pending=false ' \
    "$evidence" || fail "input routing or native completion did not drain"
(( $(count '^sophia_live_output schema=1 status=complete output=[0-9]+ .*nonzero_exports=[1-9][0-9]*$') == 2 )) ||
    fail "both outputs did not complete with visible content"
grep -Eq '^sophia_live_session_cleanup schema=1 status=clean app_groups=0([[:space:]]|$)' \
    "$evidence" || fail "session cleanup did not drain"
grep -Fxq 'sophia_qemu_guest schema=1 status=complete scenario=xmonad-interactive' \
    "$evidence" || fail "the guest did not power off after normal logout"
(( $(count 'status=restart_injected') == 0 )) ||
    fail "the interactive guest injected a compatibility-bridge restart"

echo "QEMU xmonad interactive evidence passed: $evidence"
