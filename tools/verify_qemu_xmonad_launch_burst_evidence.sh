#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_FILE="${1:-/tmp/sophia-qemu-xmonad-launch-burst.log}"

fail() {
    echo "QEMU xmonad launch-burst verification failed: $*" >&2
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
if grep -Eq '(^Error:|panicked at|admission_group_(invalid|overflowed)|status=(failed|degraded)([[:space:]]|$))' \
    "$EVIDENCE_FILE"; then
    fail "evidence contains a Sophia, guest, or harness failure"
fi
if grep -Eq '^sophia_session_app schema=2 status=failed source=action ' "$EVIDENCE_FILE"; then
    fail "an accepted launch failed or timed out"
fi

grep -Fxq \
    'sophia_live_session_mode schema=1 mode=normal configured_apps=14 startup_apps=13' \
    "$EVIDENCE_FILE" || fail "the isolated launch-capacity session profile is missing"
# Guest stderr can prefix a serial record at the byte-stream boundary. Keep the
# complete controlled record suffix strict while tolerating that harmless noise.
(( $(count 'sophia_session_app schema=1 status=started id=holder([1-9]|1[0-2]) source=startup$') == 12 )) ||
    fail "the twelve managed capacity holders did not start"
grep -Eq '^sophia_live_wm schema=1 status=ready adapter=external ' \
    "$EVIDENCE_FILE" || fail "external xmonad policy did not become ready"
grep -Fxq \
    'sophia_live_session_startup schema=2 status=output_baseline_ready outputs=2/2' \
    "$EVIDENCE_FILE" || fail "both output baselines were not presented"
mapfile -t startup_outputs < <(
    grep -E '^sophia_live_native_startup_output schema=1 status=presented output=[0-9]+ proof=synchronous_modeset submission=1$' \
        "$EVIDENCE_FILE"
)
(( ${#startup_outputs[@]} == 2 )) ||
    fail "expected two synchronously presented startup outputs"
[[ "$(printf '%s\n' "${startup_outputs[@]}" | sed -n 's/.* output=\([0-9][0-9]*\) .*/\1/p' | sort -u | wc -l)" == 2 ]] ||
    fail "startup output evidence contains duplicate identities"
grep -Eq 'sophia_live_native_page_flip schema=1 status=retired output=[0-9]+ ' \
    "$EVIDENCE_FILE" || fail "no asynchronous page flip retired"

grep -Fxq \
    'sophia_qemu_launch_burst schema=1 status=sent chord=meta_l+ret requests=32' \
    "$EVIDENCE_FILE" || fail "the 32-request QMP burst is missing"
settled="$(grep -E '^sophia_qemu_launch_burst schema=1 status=settled ' \
    "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$settled" ]] || fail "the bounded burst settlement is missing"
for pair in active_preload=12 queued=4 admitted=4; do
    actual="$(field "$settled" "${pair%%=*}")" ||
        fail "burst settlement is missing ${pair%%=*}"
    [[ "$actual" == "${pair#*=}" ]] ||
        fail "${pair%%=*} is $actual, expected ${pair#*=}"
done
settled_rejected="$(field "$settled" rejected)" ||
    fail "burst settlement is missing rejected"
[[ "$settled_rejected" =~ ^[0-9]+$ ]] \
    && (( settled_rejected >= 20 && settled_rejected <= 28 )) ||
    fail "burst rejected $settled_rejected requests, expected 20-28 after valid coalescing"
grep -Fxq \
    'sophia_qemu_launch_burst schema=1 status=recovery_admitted queued=5 admitted=5' \
    "$EVIDENCE_FILE" || fail "a post-capacity launch was not admitted"

(( $(count '^sophia_session_app schema=2 status=queued source=action ') == 5 )) ||
    fail "accepted launch accounting is not exactly four burst plus one recovery"
(( $(count '^sophia_session_app schema=2 status=started .* source=action ') == 5 )) ||
    fail "not every accepted launch started"
(( $(count '^sophia_session_app schema=2 status=admitted source=action ') == 5 )) ||
    fail "not every started launch reached stable admission"
rejected_count="$(count '^sophia_session_app schema=2 status=rejected source=action .* reason=capacity$')"
(( rejected_count == settled_rejected )) ||
    fail "capacity rejection records do not match the settlement summary"
launch_action_count="$(count '^sophia_live_wm schema=1 status=session_action_committed .* action=LaunchTerminal$')"
(( launch_action_count == 5 + settled_rejected )) ||
    fail "committed launch actions do not balance burst outcomes and the recovery launch"

awk '
    /^sophia_session_app schema=2 status=started .* source=action / {
        in_flight++
        if (in_flight > 1) exit 1
    }
    /^sophia_session_app schema=2 status=(admitted|failed).* source=action / {
        in_flight--
        if (in_flight < 0) exit 1
    }
    END { if (in_flight != 0) exit 1 }
' "$EVIDENCE_FILE" || fail "application admission was not strictly sequential"

awk '
    /^sophia_qemu_launch_burst schema=1 status=settled / { phase = 1; next }
    phase == 1 && /^sophia_qemu_launch_burst schema=1 status=capacity_release_wait / {
        phase = 2
        next
    }
    phase == 2 && /^sophia_session_app schema=1 status=exited id=holder([1-9]|1[0-2]) source=managed / {
        phase = 3
        next
    }
    phase == 3 && /^sophia_qemu_launch_burst schema=1 status=capacity_released managed_exits=[1-9][0-9]*$/ {
        phase = 4
        next
    }
    phase == 4 && /^sophia_qemu_launch_burst schema=1 status=recovery_launch_begin / {
        phase = 5
        next
    }
    phase == 5 && /^sophia_session_app schema=2 status=queued source=action / {
        phase = 6
        next
    }
    phase == 6 && /^sophia_session_app schema=2 status=started .* source=action / {
        phase = 7
        next
    }
    phase == 7 && /^sophia_session_app schema=2 status=admitted source=action / {
        phase = 8
        next
    }
    phase == 8 && /^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$/ {
        recovery_focus_seen = 1
        next
    }
    phase == 8 && /^sophia_qemu_launch_burst schema=1 status=recovery_admitted / {
        phase = 9
        next
    }
    phase == 9 && /^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$/ {
        recovery_focus_seen = 1
        next
    }
    phase == 9 && /^sophia_qemu_launch_burst schema=1 status=recovery_focus_ready source=x11-control$/ {
        if (!recovery_focus_seen) exit 1
        phase = 10
        next
    }
    phase == 10 && /^sophia_qemu_launch_burst schema=1 status=action_probe_begin chord=meta_l\+j$/ {
        phase = 11
        next
    }
    phase == 11 && /^sophia_live_wm schema=1 status=physical_action_committed action=/ {
        phase = 12
        next
    }
    phase == 12 && /^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$/ {
        phase = 13
        next
    }
    phase == 13 && /^sophia_qemu_launch_burst schema=1 status=action_probe_committed chord=meta_l\+j focus=applied$/ {
        phase = 14
        next
    }
    phase == 14 && /^sophia_qemu_launch_burst schema=1 status=logout_begin / {
        phase = 15
        next
    }
    phase == 15 && /^sophia_live_wm schema=1 status=session_action_committed .* action=Logout$/ {
        phase = 16
        next
    }
    END { if (phase != 16) exit 1 }
' "$EVIDENCE_FILE" ||
    fail "capacity release, recovery launch, action probe, and logout are out of order"

launches="$(grep -E '^sophia_session_launches schema=2 status=complete ' \
    "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$launches" ]] || fail "launch completion summary is missing"
for pair in admission_timeouts=0; do
    actual="$(field "$launches" "${pair%%=*}")" ||
        fail "launch completion is missing ${pair%%=*}"
    [[ "$actual" == "${pair#*=}" ]] ||
        fail "${pair%%=*} is $actual, expected ${pair#*=}"
done
summary_rejected="$(field "$launches" rejected)" ||
    fail "launch completion is missing rejected"
[[ "$summary_rejected" == "$settled_rejected" ]] ||
    fail "launch completion rejected $summary_rejected, expected $settled_rejected"
peak_depth="$(field "$launches" peak_depth)" || fail "launch completion is missing peak_depth"
[[ "$peak_depth" =~ ^[1-4]$ ]] ||
    fail "launch queue peak depth is $peak_depth, expected 1-4 with twelve active holders"

grep -Eq '^sophia_live_session_health schema=1 status=clean .* pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false$' \
    "$EVIDENCE_FILE" || fail "final session health is not clean"
grep -Fxq \
    'sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=none' \
    "$EVIDENCE_FILE" || fail "native presentation did not drain"
grep -Eq '^sophia_live_session_cleanup schema=1 status=clean app_groups=0([[:space:]]|$)' \
    "$EVIDENCE_FILE" || fail "application cleanup did not drain"

output_count="$(count '^sophia_live_output schema=1 status=complete output=[0-9]+ .*nonzero_exports=[1-9][0-9]*$')"
(( output_count == 2 )) || fail "expected two nonzero output summaries; found $output_count"
completion="$(grep -E '^sophia_live_session schema=16 status=bounded_complete ' \
    "$EVIDENCE_FILE" | tail -n 1)"
[[ -n "$completion" ]] || fail "schema-16 completion is missing"
for pair in \
    authority_batches_dropped=0 \
    native_submit_failures=0 \
    native_retire_failures=0 \
    native_callback_rejected=0 \
    native_in_flight=false \
    native_cleanup_pending=false \
    wm_policy=external \
    wm_restarts=0 \
    wm_degraded=false; do
    actual="$(field "$completion" "${pair%%=*}")" ||
        fail "completion is missing ${pair%%=*}"
    [[ "$actual" == "${pair#*=}" ]] ||
        fail "${pair%%=*} is $actual, expected ${pair#*=}"
done
grep -Fxq \
    'sophia_qemu_guest schema=1 status=complete scenario=xmonad-launch-burst' \
    "$EVIDENCE_FILE" || fail "the QEMU guest did not exit normally"

echo "QEMU xmonad launch-burst evidence passed: $EVIDENCE_FILE"
