#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
LOG_DIR="${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"
WAIT_SECONDS="${SOPHIA_VERIFY_WAIT_SECONDS:-5}"

fail() {
    echo "four-Kitty xmonad verification failed: $*" >&2
    exit 1
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

[[ -s "$SESSION_LOG" ]] || fail "missing session log: $SESSION_LOG"

deadline=$((SECONDS + WAIT_SECONDS))
while ! grep -Eq '^sophia_live_session_cleanup schema=1 status=clean ' "$SESSION_LOG" ||
    ! grep -Eq '^sophia_live_session schema=14 status=bounded_complete ' "$SESSION_LOG"; do
    (( SECONDS < deadline )) || fail "session log is incomplete"
    sleep 0.1
done

if grep -Eq '(^Error:|panicked at|status=(failed|degraded)([[:space:]]|$))' "$SESSION_LOG"; then
    fail "session log contains a Sophia error, panic, or degraded status"
fi
if grep -Eq 'status=submitted .* content=None|outcome=forced_detach_|abandoned_scanouts=[1-9]' \
    "$SESSION_LOG"; then
    fail "session submitted empty output content or used forced native detach"
fi

grep -Eq \
    '^sophia_live_session_startup schema=2 status=output_baseline_ready outputs=2/2$' \
    "$SESSION_LOG" ||
    fail "both output baselines were not presented"
mapfile -t startup_outputs < <(
    grep -E '^sophia_live_native_startup_output schema=1 status=presented output=[0-9]+ proof=synchronous_modeset submission=1$' \
        "$SESSION_LOG"
)
(( ${#startup_outputs[@]} == 2 )) ||
    fail "expected two synchronously presented startup outputs"
[[ "$(printf '%s\n' "${startup_outputs[@]}" | sed -n 's/.* output=\([0-9][0-9]*\) .*/\1/p' | sort -u | wc -l)" == 2 ]] ||
    fail "startup output evidence contains duplicate output identities"

mapfile -t launches < <(
    grep -nE '^sophia_session_app schema=1 status=started id=terminal source=(startup|action)$' \
        "$SESSION_LOG"
)
(( ${#launches[@]} >= 4 )) ||
    fail "observed ${#launches[@]} Kitty launches, expected at least four"

fourth_line="${launches[3]%%:*}"
four_window_log="$(mktemp)"
trap 'rm -f "$four_window_log"' EXIT
tail -n "+$fourth_line" "$SESSION_LOG" >"$four_window_log"

grep -Eq '^sophia_live_resize_epoch schema=1 status=held transaction=[0-9]+ surfaces=3$' \
    "$four_window_log" ||
    fail "four-window resize epoch was not held"
grep -Eq '^sophia_live_resize_epoch schema=1 status=committed transaction=[0-9]+ matched_surfaces=3$' \
    "$four_window_log" ||
    fail "three resized surfaces did not commit together"

if grep -Eq 'status=(layout_timeout|aborted)|rejected Present whose pixels do not match' \
    "$four_window_log"; then
    fail "four-window resize timed out, aborted, or rejected matching pixels"
fi

for target in \
    '1280x1440_0_0' \
    '1280x480_1280_0' \
    '1280x480_1280_480' \
    '1280x480_1280_960'; do
    grep -Eq \
        "^sophia_live_session_present schema=2 status=retired .* source=${target%%_*} target=${target} .* unit_scale=true$" \
        "$four_window_log" ||
        fail "missing pixel-matched retired tile: $target"
done

grep -Eq '^sophia_live_session_health schema=1 status=clean ' "$SESSION_LOG" ||
    fail "session did not finish cleanly"
grep -Eq \
    '^sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=none$' \
    "$SESSION_LOG" ||
    fail "native presentation did not drain cleanly"
grep -Eq '^sophia_live_session_cleanup schema=1 status=clean ' "$SESSION_LOG" ||
    fail "session cleanup did not complete cleanly"
grep -Eq '^sophia_live_session_protocol_errors schema=1 expected=[0-9]+ unexpected=0$' \
    "$SESSION_LOG" ||
    fail "session recorded an unexpected X protocol error"
mapfile -t session_control_records < <(
    grep -E '^sophia_live_session_control schema=1 status=complete ' "$SESSION_LOG"
)
(( ${#session_control_records[@]} == 1 )) ||
    fail "expected one session-control completion record"
session_control="${session_control_records[0]}"
for assignment in rejected=0 timed_out=0 unexpected=0 pending=0; do
    [[ " $session_control " == *" $assignment "* ]] ||
        fail "session-control ledger was not clean: $assignment"
done
control_enqueued="$(field "$session_control" enqueued)"
control_dispatched="$(field "$session_control" dispatched)"
control_delivered="$(field "$session_control" delivered)"
control_queue_dwell="$(field "$session_control" max_queue_dwell_msec)"
control_ack_latency="$(field "$session_control" max_ack_msec)"
(( control_enqueued == control_dispatched && control_dispatched == control_delivered )) ||
    fail "session-control enqueue, dispatch, and delivery counts diverged"
(( control_queue_dwell <= 100 && control_ack_latency <= 100 )) ||
    fail "session-control latency exceeded 100ms"
grep -Eq \
    '^sophia_live_session_keys schema=1 status=complete pending=0 release_barrier_pending=0 peak_pressed=[0-9]+ synthetic_releases=[0-9]+ state_only_releases=[0-9]+ orphan_releases_suppressed=[0-9]+ removed_surface_keys=0$' \
    "$SESSION_LOG" ||
    fail "client pressed-key state did not drain"
mapfile -t completions < <(
    grep -E '^sophia_live_session schema=14 status=bounded_complete ' "$SESSION_LOG"
)
(( ${#completions[@]} == 1 )) ||
    fail "expected one completed session, found ${#completions[@]}"
completion="${completions[0]}"
for assignment in \
    native_submit_failures=0 \
    native_retire_failures=0 \
    native_callback_rejected=0 \
    native_callback_queue_saturated=0 \
    native_in_flight=false \
    native_cleanup_pending=false \
    present_disconnect_failures=0 \
    present_live_sources=0 \
    present_live_fences=0 \
    present_live_transactions=0; do
    [[ " $completion " == *" $assignment "* ]] ||
        fail "completion does not contain $assignment"
done
for key in native_mixed_exports native_target_recreations \
    native_max_submit_to_page_flip_msec native_max_upload_msec \
    input_queue_dwell_max_msec; do
    value="$(field "$completion" "$key")" ||
        fail "completion is missing $key"
    [[ "$value" =~ ^[0-9]+$ ]] ||
        fail "completion has nonnumeric $key=$value"
    if [[ "$key" == native_mixed_exports ]]; then
        (( value >= 32 )) ||
            fail "sustained mixed presentation produced only $value exports"
    elif [[ "$key" == native_target_recreations ]]; then
        (( value == 0 )) ||
            fail "stable workload recreated native targets: $value"
    else
        (( value <= 100 )) ||
            fail "$key exceeded the 100ms promotion budget: $value"
    fi
done

mapfile -t resource_lines < <(
    grep -E '^sophia_live_native_resources schema=1 status=complete ' "$SESSION_LOG"
)
(( ${#resource_lines[@]} == 1 )) ||
    fail "expected one native resource-lifetime record"
resources="${resource_lines[0]}"
for key in target_creations pipeline_creations cpu_target_creations \
    dmabuf_target_creations composition_target_creations epoch_replacements \
    recovery_replacements; do
    value="$(field "$resources" "$key")" ||
        fail "resource-lifetime record is missing $key"
    [[ "$value" =~ ^[0-9]+$ ]] ||
        fail "resource-lifetime record has nonnumeric $key=$value"
done
target_creations="$(field "$resources" target_creations)"
pipeline_creations="$(field "$resources" pipeline_creations)"
cpu_targets="$(field "$resources" cpu_target_creations)"
dmabuf_targets="$(field "$resources" dmabuf_target_creations)"
composition_targets="$(field "$resources" composition_target_creations)"
epoch_replacements="$(field "$resources" epoch_replacements)"
recovery_replacements="$(field "$resources" recovery_replacements)"
mixed_exports="$(field "$completion" native_mixed_exports)"
target_recreations="$(field "$completion" native_target_recreations)"
(( target_creations == pipeline_creations )) ||
    fail "target and pipeline creation counts diverged"
(( target_creations == cpu_targets + dmabuf_targets + composition_targets )) ||
    fail "resource-class creation counts do not sum to the total"
(( composition_targets > 0 && composition_targets <= 2 )) ||
    fail "stable two-output workload did not retain composition targets"
(( target_recreations == 0 )) ||
    fail "stable workload recreated a native target"
(( epoch_replacements == 0 && recovery_replacements == 0 )) ||
    fail "stable CPU or direct DMA-BUF resources were replaced"

grep -Eq \
    '^sophia_session_launches schema=1 status=complete peak_depth=([0-9]|1[0-6]) rejected=[0-9]+ admission_timeouts=0$' \
    "$SESSION_LOG" ||
    fail "application admission did not complete without timeout"

mapfile -t output_completions < <(
    grep -E '^sophia_live_output schema=1 status=complete ' "$SESSION_LOG"
)
(( ${#output_completions[@]} >= 1 )) ||
    fail "session has no per-output completion records"
for output_completion in "${output_completions[@]}"; do
    submissions="$(sed -n 's/.* submissions=\([0-9][0-9]*\) .*/\1/p' <<<"$output_completion")"
    retirements="$(sed -n 's/.* retirements=\([0-9][0-9]*\) .*/\1/p' <<<"$output_completion")"
    callbacks="$(sed -n 's/.* callbacks=\([0-9][0-9]*\) .*/\1/p' <<<"$output_completion")"
    [[ -n "$submissions" && -n "$retirements" && -n "$callbacks" ]] ||
        fail "malformed output completion: $output_completion"
    (( submissions == retirements + 1 )) ||
        fail "output did not retain exactly one displayed buffer: $output_completion"
    (( callbacks == retirements )) ||
        fail "output callback/retirement counts diverged: $output_completion"
done

echo "four-Kitty xmonad session verified: $SESSION_LOG"
