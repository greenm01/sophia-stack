#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_FILE="${1:-/tmp/sophia-qemu-xmonad-stale-response.log}"

fail() {
    echo "QEMU xmonad stale-response verification failed: $*" >&2
    exit 1
}

count() {
    grep -Ec "$1" "$EVIDENCE_FILE" || true
}

field() {
    local line="$1" key="$2" token
    for token in $line; do
        if [[ "$token" == "$key="* ]]; then
            printf '%s\n' "${token#*=}"
            return 0
        fi
    done
    return 1
}

[[ -s "$EVIDENCE_FILE" ]] || fail "missing evidence: $EVIDENCE_FILE"
if grep -Eq '(^Error:|panicked at|UnknownSurface|status=(failed|degraded)([[:space:]]|$))' \
    "$EVIDENCE_FILE"; then
    fail "evidence contains a Sophia, guest, or harness failure"
fi

grep -Fxq \
    'sophia_live_session_mode schema=1 mode=normal configured_apps=3 startup_apps=2' \
    "$EVIDENCE_FILE" || fail "the isolated stale-response profile is missing"
(( $(count 'sophia_session_app schema=1 status=started id=(primary|secondary) source=startup$') == 2 )) ||
    fail "the two persistent startup clients did not start"
grep -Eq '^sophia_live_wm schema=1 status=ready adapter=external ' \
    "$EVIDENCE_FILE" || fail "external xmonad policy did not become ready"
grep -Fxq \
    'sophia_live_session_startup schema=2 status=output_baseline_ready outputs=2/2' \
    "$EVIDENCE_FILE" || fail "both output baselines were not presented"
(( $(count '^sophia_live_native_startup_output schema=1 status=presented output=[0-9]+ proof=synchronous_modeset submission=1$') == 2 )) ||
    fail "expected two synchronously presented startup outputs"

(( $(count '^sophia_session_app schema=2 status=queued source=action ') == 1 )) ||
    fail "the short-lived launch was not queued exactly once"
(( $(count '^sophia_session_app schema=2 status=started id=transient source=action ') == 1 )) ||
    fail "the short-lived launch did not start exactly once"
(( $(count '^sophia_session_app schema=2 status=completed id=transient source=action .* reason=normal_exit_after_surface ') == 1 )) ||
    fail "the short-lived client did not exit normally after surface observation"
(( $(count '^sophia_session_app schema=2 status=(admitted|failed) .*source=action ') == 0 )) ||
    fail "the short-lived client reached an invalid terminal admission state"
(( $(count '^sophia_live_wm schema=3 status=response_rejected reason=stale_layout .* source=manage removed_registered_surfaces=0$') == 1 )) ||
    fail "the stale ManageSurface response was not rejected exactly once"
(( $(count '^sophia_live_wm schema=2 status=restart_requested reason=stale_response error=none$') == 1 )) ||
    fail "the stale response did not request exactly one transport restart"
(( $(count '^sophia_live_wm schema=1 status=restarted restarts=1 preserved_layout=true$') == 1 )) ||
    fail "the WM transport did not restart exactly once"
(( $(count '^sophia_live_wm schema=1 status=restarted ') == 1 )) ||
    fail "the WM transport restarted more than once"
(( $(count '^sophia_live_wm schema=4 status=reseed_queued phase=committed_layout request=relayout$') == 1 )) ||
    fail "committed layout was not reseeded exactly once"
grep -Fxq \
    'sophia_qemu_stale_response schema=1 status=recovered restarts=1 visible_surfaces=2' \
    "$EVIDENCE_FILE" || fail "the two persistent surfaces were not recovered"
grep -Fxq \
    'sophia_qemu_stale_response schema=1 status=action_probe_committed chord=meta_l+j focus=applied' \
    "$EVIDENCE_FILE" || fail "post-restart focus cycling did not commit"

awk '
    /^sophia_qemu_stale_response schema=1 status=launch_begin / { phase = 1; next }
    phase == 1 && /^sophia_session_app schema=2 status=started id=transient source=action / {
        phase = 2
        next
    }
    phase == 2 && /^sophia_session_app schema=2 status=completed id=transient source=action .* reason=normal_exit_after_surface / {
        phase = 3
        next
    }
    phase == 3 && /^sophia_live_wm schema=3 status=response_rejected reason=stale_layout .* source=manage removed_registered_surfaces=0$/ {
        phase = 4
        next
    }
    phase == 4 && /^sophia_live_wm schema=2 status=restart_requested reason=stale_response error=none$/ {
        phase = 5
        next
    }
    phase == 5 && /^sophia_live_wm schema=1 status=restarted restarts=1 preserved_layout=true$/ {
        phase = 6
        next
    }
    phase == 6 && /^sophia_live_wm schema=4 status=reseed_queued phase=committed_layout request=relayout$/ {
        phase = 7
        next
    }
    phase == 7 && /^sophia_qemu_stale_response schema=1 status=recovered restarts=1 visible_surfaces=2$/ {
        phase = 8
        next
    }
    phase == 8 && /^sophia_qemu_stale_response schema=1 status=action_probe_begin chord=meta_l\+j$/ {
        phase = 9
        next
    }
    phase == 9 && /^sophia_live_wm schema=1 status=physical_action_committed action=/ {
        phase = 10
        next
    }
    phase == 10 && /^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$/ {
        phase = 11
        next
    }
    phase == 11 && /^sophia_qemu_stale_response schema=1 status=action_probe_committed chord=meta_l\+j focus=applied$/ {
        phase = 12
        next
    }
    phase == 12 && /^sophia_qemu_stale_response schema=1 status=logout_begin / {
        phase = 13
        next
    }
    phase == 13 && /^sophia_live_wm schema=1 status=session_action_committed .* action=Logout$/ {
        phase = 14
        next
    }
    END { if (phase != 14) exit 1 }
' "$EVIDENCE_FILE" || fail "surface exit, restart, reseed, action probe, and logout are out of order"

transport="$(grep -E '^sophia_live_wm_transport schema=2 status=complete ' "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$transport" ]] || fail "WM transport completion is missing"
for pair in pending=0 stale_responses=1; do
    actual="$(field "$transport" "${pair%%=*}")" ||
        fail "WM transport completion is missing ${pair%%=*}"
    [[ "$actual" == "${pair#*=}" ]] ||
        fail "${pair%%=*} is $actual, expected ${pair#*=}"
done
grep -Eq '^sophia_live_session_health schema=1 status=clean .* pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false$' \
    "$EVIDENCE_FILE" || fail "final session health is not clean"
grep -Fxq \
    'sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=none' \
    "$EVIDENCE_FILE" || fail "native presentation did not drain"
grep -Eq '^sophia_live_session_cleanup schema=1 status=clean app_groups=0([[:space:]]|$)' \
    "$EVIDENCE_FILE" || fail "application cleanup did not drain"

completion="$(grep -E '^sophia_live_session schema=16 status=bounded_complete ' "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$completion" ]] || fail "schema-16 completion is missing"
for pair in \
    authority_batches_dropped=0 \
    native_submit_failures=0 \
    native_retire_failures=0 \
    native_callback_rejected=0 \
    native_in_flight=false \
    native_cleanup_pending=false \
    wm_policy=external \
    wm_restarts=1 \
    wm_degraded=false; do
    actual="$(field "$completion" "${pair%%=*}")" ||
        fail "completion is missing ${pair%%=*}"
    [[ "$actual" == "${pair#*=}" ]] ||
        fail "${pair%%=*} is $actual, expected ${pair#*=}"
done
(( $(count '^sophia_live_output schema=1 status=complete output=[0-9]+ .*nonzero_exports=[1-9][0-9]*$') == 2 )) ||
    fail "both outputs did not retain nonzero presentation"
grep -Fxq \
    'sophia_qemu_guest schema=1 status=complete scenario=xmonad-stale-response' \
    "$EVIDENCE_FILE" || fail "the QEMU guest did not exit normally"

echo "QEMU xmonad stale-response evidence passed: $EVIDENCE_FILE"
