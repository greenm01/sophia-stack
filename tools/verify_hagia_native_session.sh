#!/usr/bin/env bash
set -euo pipefail

# Verifies one native Hagia session against the bounded product workflow on the
# roadmap's critical path: three terminal launches, a visible focus-next, one
# close, and a normal logout, with Sophia's WM and shell protocols carrying all
# of it and no xmonad compatibility bridge in the session.
#
# The promotion this evidence supports is the three native frame slots, so the
# schema-7 block is checked as a balance rather than as a set of present fields:
# every renderer-worker request must have settled as a completion or a bounded
# deferral, no slot may have been leased at completion, and no stale release may
# have been refused.

evidence="${1:?usage: verify_hagia_native_session.sh EVIDENCE [PROOF_TEXT]}"
proof_text="${2:-hagianativeproof}"

fail() {
    echo "Hagia native session verification failed: $*" >&2
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

count() {
    grep -Ec "$1" "$evidence" || true
}

first_line() {
    grep -nEm1 "$1" "$evidence" | cut -d: -f1
}

require_exactly() {
    local description="$1" pattern="$2" expected="$3" observed
    observed="$(count "$pattern")"
    (( observed == expected )) ||
        fail "expected $expected $description, found $observed"
}

require_line() {
    local description="$1" pattern="$2"
    grep -Eq "$pattern" "$evidence" || fail "missing $description"
}

[[ -s "$evidence" ]] || fail "evidence is missing or empty: $evidence"
[[ "$proof_text" =~ ^[a-z]{1,24}$ ]] ||
    fail "proof text must contain 1-24 lowercase ASCII letters"

# Identity. One line, both signed commits, all three binary digests, and the
# desktop profile the session actually loaded.
identity_pattern='^sophia_hagia_native_identity schema=1 status=bound sophia_commit=[0-9a-f]{40} hagia_commit=[0-9a-f]{40} sophia_sha256=[0-9a-f]{64} hagia_sha256=[0-9a-f]{64} hagia_shell_sha256=[0-9a-f]{64} desktop_profile_sha256=[0-9a-f]{64}$'
require_exactly "bound Sophia/Hagia identity" "$identity_pattern" 1
identity="$(grep -E "$identity_pattern" "$evidence")"
profile_sha256="$(field "$identity" desktop_profile_sha256)"

# The profile identity must describe the profile that ran. A session started
# with --no-config loads the compiled profile while an exported digest still
# names a file on disk, which is how the switcher gate came to print an identity
# for a profile it was not using.
profile_line="$(grep -E '^sophia_live_desktop_profile schema=1 status=loaded ' "$evidence" || true)"
[[ -n "$profile_line" ]] || fail "session recorded no desktop-profile identity"
[[ "$(field "$profile_line" root_sha256)" == "$profile_sha256" ]] ||
    fail "the loaded desktop profile is not the one bound to this run"

# This is the native path. The compatibility bridge has its own gates; a bridge
# record here means the session under test was not the one being promoted.
if grep -Eq 'sophia-x11-wm-bridge|legacy WM did not configure' "$evidence"; then
    fail "evidence contains xmonad compatibility bridge activity"
fi
if grep -Eq '(^Error:|panicked at|status=(failed|degraded)([[:space:]]|$))' "$evidence"; then
    fail "evidence contains a Sophia error, panic, or degraded status"
fi

require_line "native WM readiness" \
    '^sophia_live_wm schema=4 status=ready adapter=sophia_wm_v1 socket=session_owned epoch=1 restarts=0$'
require_line "a presented startup output" \
    '^sophia_live_native_startup_output schema=1 status=presented output=[0-9]+ proof=synchronous_modeset submission=1$'

# The physical input path, proven before the workflow while the startup terminal
# was the session's only window.
require_line "exact physical text completion" \
    "^sophia_live_session_input schema=2 status=complete source=physical text=$proof_text expected_events=[1-9][0-9]* matched_events=[1-9][0-9]* pixel_change=true$"

# The workflow itself. Session actions and their physical commits are separate
# facts: the first says policy decided, the second says Sophia committed the
# operator's keypress, and a run missing either did not prove the shortcut path.
require_exactly "committed terminal launches" \
    '^sophia_live_wm schema=1 status=session_action_committed transaction=[1-9][0-9]* action=LaunchTerminal$' 3
require_exactly "committed close action" \
    '^sophia_live_wm schema=1 status=session_action_committed transaction=[1-9][0-9]* action=CloseFocused$' 1
require_exactly "committed logout action" \
    '^sophia_live_wm schema=1 status=session_action_committed transaction=[1-9][0-9]* action=Logout$' 1
launch_admissions="$(count '^sophia_session_app schema=2 status=admitted source=action transaction=[1-9][0-9]* surface=[1-9][0-9]*$')"
(( launch_admissions >= 3 )) ||
    fail "expected three admitted launch surfaces, found $launch_admissions"

# Ordering. Each launch must project before the next is requested, so the
# projections are read as a sequence rather than as a set: a run that committed
# three launches and projected them in one late batch is not the ordered commit
# path this gate promotes.
mapfile -t launch_lines < <(
    grep -nE '^sophia_live_wm schema=1 status=session_action_committed transaction=[1-9][0-9]* action=LaunchTerminal$' \
        "$evidence" | cut -d: -f1
)
mapfile -t projection_lines < <(
    grep -nE '^sophia_live_wm schema=2 status=workspace_projection_committed transaction=[1-9][0-9]* output=[0-9]+ workspace=[0-9]+ visible_surfaces=[0-9]+ focus=surface$' \
        "$evidence" | cut -d: -f1
)
(( ${#projection_lines[@]} >= 4 )) ||
    fail "expected at least four focused workspace projections, found ${#projection_lines[@]}"
for index in 0 1 2; do
    launch="${launch_lines[$index]}"
    settled=false
    for projection in "${projection_lines[@]}"; do
        if (( projection > launch )); then
            settled=true
            break
        fi
    done
    [[ "$settled" == true ]] ||
        fail "terminal launch $((index + 1)) never reached a focused projection"
done

focus_next_line="$(first_line '^sophia_live_wm schema=1 status=physical_action_committed action=1$')"
close_line="$(first_line '^sophia_live_wm schema=1 status=session_action_committed transaction=[1-9][0-9]* action=CloseFocused$')"
logout_line="$(first_line '^sophia_live_wm schema=1 status=session_action_committed transaction=[1-9][0-9]* action=Logout$')"
[[ -n "$focus_next_line" ]] || fail "focus-next was never committed"
(( focus_next_line > launch_lines[2] )) ||
    fail "focus-next was committed before the third terminal launch"
(( close_line > focus_next_line )) ||
    fail "the close was committed before the focus change it follows"
(( logout_line > close_line )) ||
    fail "logout was committed before the close"

# A focus change nobody could see is not the proof this step asks for, so the
# focus-next must be followed by its own committed projection.
focus_projection=false
for projection in "${projection_lines[@]}"; do
    if (( projection > focus_next_line && projection < close_line )); then
        focus_projection=true
        break
    fi
done
[[ "$focus_projection" == true ]] ||
    fail "focus-next committed no visible workspace projection"

# Shell-role separation. No switcher runs in this workflow, so what is checked
# is the boundary rather than the feature: the broker and the shell are separate
# protected admissions, descriptors reach Engine redacted, and both stop cleanly.
require_exactly "protected metadata-broker admission" \
    '^sophia_live_metadata_broker schema=1 status=ready protected=true peer_pid=[1-9][0-9]* revision=[1-9][0-9]*$' 1
require_exactly "protected Hagia Shell admission" \
    '^sophia_live_metadata_shell schema=1 status=ready protected=true peer_pid=[1-9][0-9]* revision=1 connection_epoch=1$' 1
require_exactly "clean metadata-broker shutdown" \
    '^sophia_live_metadata_broker schema=1 status=stopped transport=disconnected process=terminated$' 1
require_exactly "clean Hagia Shell shutdown" \
    '^sophia_live_metadata_shell schema=1 status=stopped transport=disconnected process=terminated$' 1
require_line "a redacted descriptor commit" \
    '^sophia_live_metadata_broker schema=1 status=descriptor_committed surface=[0-9]+ content=redacted$'
if grep -Eq '(protected metadata (broker|shell) exited|^sophia_live_metadata_(broker|shell) schema=1 status=(failed|transport_failed|candidate_rejected|activation_rejected|unavailable|disconnect_failed) )' "$evidence"; then
    fail "evidence contains a metadata broker or shell failure"
fi
broker_ready_line="$(first_line '^sophia_live_metadata_broker schema=1 status=ready ')"
broker_descriptor_line="$(first_line '^sophia_live_metadata_broker schema=1 status=descriptor_committed ')"
broker_stopped_line="$(first_line '^sophia_live_metadata_broker schema=1 status=stopped ')"
shell_ready_line="$(first_line '^sophia_live_metadata_shell schema=1 status=ready ')"
shell_stopped_line="$(first_line '^sophia_live_metadata_shell schema=1 status=stopped ')"
(( broker_ready_line < broker_descriptor_line && broker_descriptor_line < broker_stopped_line )) ||
    fail "metadata-broker lifecycle is not ready -> descriptor -> stopped"
(( shell_ready_line < shell_stopped_line )) ||
    fail "Hagia Shell lifecycle is not ready -> stopped"

# Session-control transport: a balanced ledger and bounded latency. Input and WM
# work share the owner thread, so an unbounded dwell here is what a session that
# felt unusable looks like in evidence.
mapfile -t session_control_records < <(
    grep -E '^sophia_live_session_control schema=(1|2) status=complete ' "$evidence"
)
(( ${#session_control_records[@]} == 1 )) ||
    fail "expected one session-control completion record"
session_control="${session_control_records[0]}"
for assignment in rejected=0 timed_out=0 unexpected=0 pending=0; do
    [[ " $session_control " == *" $assignment "* ]] ||
        fail "session-control ledger was not clean: $assignment"
done
control_stale_retired=0
if [[ "$(field "$session_control" schema)" == 2 ]]; then
    control_stale_retired="$(field "$session_control" stale_retired)"
fi
(( $(field "$session_control" enqueued) == $(field "$session_control" dispatched) &&
    $(field "$session_control" dispatched) ==
    $(field "$session_control" delivered) + control_stale_retired )) ||
    fail "session-control enqueue, dispatch, and delivery counts diverged"
(( $(field "$session_control" max_queue_dwell_msec) <= 100 &&
    $(field "$session_control" max_ack_msec) <= 100 )) ||
    fail "session-control latency exceeded 100ms"

# Native drain and clean teardown.
require_line "clean native presentation drain" \
    '^sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=none$'
require_line "clean session health" \
    '^sophia_live_session_health schema=1 status=clean protocol_errors=0 pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false$'
require_line "clean output topology" \
    '^sophia_live_output_topology_health schema=1 status=clean quarantined=false$'
require_line "clean process cleanup" \
    '^sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 namespace=revoked xauthority=removed$'
require_line "zero unexpected protocol errors" \
    '^sophia_live_session_protocol_errors schema=1 expected=[0-9]+ unexpected=0$'
require_line "drained client key state" \
    '^sophia_live_session_keys schema=2 status=complete pending=0 release_barrier_pending=0 .* removed_surface_keys=0 repeat_active_seats=0 .* repeat_capacity_exhausted=0$'

mapfile -t completions < <(
    grep -E '^sophia_live_session schema=16 status=bounded_complete ' "$evidence"
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
    present_live_transactions=0 \
    input_text_match=true \
    input_pixel_change=true \
    wm_restarts=0 \
    wm_degraded=false; do
    [[ " $completion " == *" $assignment "* ]] ||
        fail "completion does not contain $assignment"
done
for key in input_queue_dwell_max_msec native_max_submit_to_page_flip_msec \
    native_max_render_msec native_max_upload_msec; do
    value="$(field "$completion" "$key")" || fail "completion is missing $key"
    [[ "$value" =~ ^[0-9]+$ ]] || fail "completion has nonnumeric $key=$value"
    (( value <= 100 )) || fail "$key exceeded the 100ms promotion budget: $value"
done
(( $(field "$completion" native_nonzero_exports) > 0 )) ||
    fail "the session presented no nonzero content"

# The three-slot promotion evidence.
mapfile -t resource_lines < <(
    grep -E '^sophia_live_native_resources schema=7 status=complete ' "$evidence"
)
(( ${#resource_lines[@]} == 1 )) ||
    fail "expected one schema-7 native resource-lifetime record"
resources="${resource_lines[0]}"
for key in frame_slot_acquisitions frame_slot_reuses frame_slot_deferrals \
    frame_slot_stale_releases frame_slots_leased frame_slots_high_watermark \
    worker_requests worker_completions worker_failures worker_hard_stalls \
    worker_release_enqueue_failures; do
    value="$(field "$resources" "$key")" ||
        fail "schema-7 resource record is missing $key"
    [[ "$value" =~ ^[0-9]+$ ]] ||
        fail "schema-7 resource record has nonnumeric $key=$value"
done
for key in worker_failures worker_hard_stalls worker_release_enqueue_failures \
    frame_slot_stale_releases; do
    (( $(field "$resources" "$key") == 0 )) || fail "$key must be zero"
done
(( $(field "$resources" frame_slot_acquisitions) > 0 )) ||
    fail "the native frame-slot pool was never acquired"
(( $(field "$resources" frame_slots_high_watermark) > 0 )) ||
    fail "the native frame-slot pool reported no live ownership"
(( $(field "$resources" frame_slots_high_watermark) <= 3 )) ||
    fail "the native frame-slot pool exceeded its three-slot capacity"
# No leaked lease. A slot still leased after the session drained means a page
# flip retired without releasing its buffer, which is exactly the failure the
# three-slot ledger exists to make impossible. Nothing else checks this today.
(( $(field "$resources" frame_slots_leased) == 0 )) ||
    fail "a native frame slot was still leased at completion"
(( $(field "$resources" worker_requests) ==
    $(field "$resources" worker_completions) + $(field "$resources" frame_slot_deferrals) )) ||
    fail "renderer-worker requests did not settle as completion or bounded deferral"

# Exact TTY restoration, which the runner records after the session returns.
require_line "exact TTY recovery" \
    '^sophia_tty_recovery schema=3 profile=hagia kd_mode_before=[^ ]+ kd_mode_after=[^ ]+ termios_restored=true emergency=false session_shutdown=not_requested session_exit_status=none$'

# The run the guide asked for, against the run that happened. The guide's waits
# are the only statement of expected totals; restating them here would make this
# file a second owner of the same fact.
guide="${SOPHIA_HAGIA_NATIVE_GUIDE:-$(dirname "$0")/fixtures/hagia_native_session_guide.sh}"
[[ -r "$guide" ]] ||
    fail "the native guide is unreadable, so its action totals cannot be checked: $guide"

declare -A expected_actions=()
while read -r action expectation; do
    # Each step waits for a cumulative count, so the largest is the run's total.
    if [[ -z "${expected_actions[$action]:-}" ]] || (( expectation > expected_actions[$action] )); then
        expected_actions[$action]="$expectation"
    fi
done < <(grep -oE '^[[:space:]]*wait_for_action_count [0-9]+ [0-9]+' "$guide" | awk '{ print $2, $3 }')
(( ${#expected_actions[@]} != 0 )) ||
    fail "the native guide requested no actions, which cannot be right"

declare -A committed_actions=()
while read -r observed action; do
    committed_actions[$action]="$observed"
done < <(grep -oE '^sophia_live_wm schema=1 status=physical_action_committed action=[0-9]+$' "$evidence" \
    | sed 's/.*action=//' | sort -n | uniq -c | awk '{ print $1, $2 }')

action_total_failures=0
for action in "${!expected_actions[@]}"; do
    observed="${committed_actions[$action]:-0}"
    if (( observed != expected_actions[$action] )); then
        echo "action $action was committed $observed times; the guide asked for ${expected_actions[$action]}" >&2
        action_total_failures=$((action_total_failures + 1))
    fi
done
for action in "${!committed_actions[@]}"; do
    if [[ -z "${expected_actions[$action]:-}" ]]; then
        echo "action $action was committed ${committed_actions[$action]} times but the guide never asked for it" >&2
        action_total_failures=$((action_total_failures + 1))
    fi
done
(( action_total_failures == 0 )) ||
    fail "the session that ran is not the session the guide specified; re-run the proof"

echo "Hagia native session evidence passed"
