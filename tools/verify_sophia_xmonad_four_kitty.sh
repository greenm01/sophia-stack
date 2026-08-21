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

inset_geometry() {
    local geometry=$1 clearance=$2
    if [[ "$geometry" =~ ^([0-9]+)x([0-9]+)_(-?[0-9]+)_(-?[0-9]+)$ ]]; then
        local width="${BASH_REMATCH[1]}"
        local height="${BASH_REMATCH[2]}"
        local x="${BASH_REMATCH[3]}"
        local y="${BASH_REMATCH[4]}"
    else
        fail "cannot inset malformed geometry: $geometry"
    fi
    local doubled=$((clearance * 2))
    ((width > doubled && height > doubled)) ||
        fail "chrome clearance $clearance consumes geometry: $geometry"
    printf '%sx%s_%s_%s\n' \
        "$((width - doubled))" "$((height - doubled))" \
        "$((x + clearance))" "$((y + clearance))"
}

[[ -s "$SESSION_LOG" ]] || fail "missing session log: $SESSION_LOG"

deadline=$((SECONDS + WAIT_SECONDS))
while ! grep -Eq '^sophia_live_session_cleanup schema=1 status=clean ' "$SESSION_LOG" ||
    ! grep -Eq '^sophia_live_session schema=(15|16) status=bounded_complete ' "$SESSION_LOG"; do
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

four_window_epoch="$(
    grep -Em1 \
        '^sophia_live_resize_epoch schema=1 status=held transaction=[0-9]+ surfaces=[34]$' \
        "$four_window_log" || true
)"
[[ -n "$four_window_epoch" ]] ||
    fail "four-window resize epoch did not hold three or four surfaces"
four_window_transaction="$(field "$four_window_epoch" transaction)"
four_window_surfaces="$(field "$four_window_epoch" surfaces)"
grep -Eq \
    "^sophia_live_resize_epoch schema=1 status=committed transaction=${four_window_transaction} matched_surfaces=${four_window_surfaces}$" \
    "$four_window_log" ||
    fail "four-window resize epoch did not commit all $four_window_surfaces held surfaces together"

four_window_projection="$(
    grep -Em1 \
        "^sophia_live_wm schema=2 status=workspace_projection_committed transaction=${four_window_transaction} output=[0-9]+ workspace=1 visible_surfaces=4 focus=surface$" \
        "$four_window_log" || true
)"
[[ -n "$four_window_projection" ]] ||
    fail "four-window layout did not project four focused surfaces"
four_window_output="$(field "$four_window_projection" output)"
work_area_record="$(
    grep -E \
        "^sophia_live_work_area schema=1 status=applied output=${four_window_output} " \
        "$SESSION_LOG" | tail -n 1
)"
[[ -n "$work_area_record" ]] ||
    fail "four-window output has no applied Engine work area"
work_area="$(field "$work_area_record" work)"
if [[ "$work_area" =~ ^([0-9]+)x([0-9]+)_(-?[0-9]+)_(-?[0-9]+)$ ]]; then
    work_width="${BASH_REMATCH[1]}"
    work_height="${BASH_REMATCH[2]}"
    work_x="${BASH_REMATCH[3]}"
    work_y="${BASH_REMATCH[4]}"
else
    fail "four-window work-area geometry is malformed: $work_area"
fi
(( work_width >= 2 && work_height >= 3 )) ||
    fail "four-window work area is too small for the Tall layout: $work_area"

chrome_record="$(
    grep -Em1 \
        '^sophia_live_wm_chrome schema=1 status=negotiated source=[^ ]+ capability=(true|false) clearance=[0-9]+$' \
        "$SESSION_LOG" || true
)"
[[ -n "$chrome_record" ]] ||
    fail "active Engine chrome clearance was not recorded"
chrome_clearance="$(field "$chrome_record" clearance)" ||
    fail "active Engine chrome record lacks clearance"

if grep -Eq 'status=(layout_timeout|aborted)|rejected Present whose pixels do not match' \
    "$four_window_log"; then
    fail "four-window resize timed out, aborted, or rejected matching pixels"
fi

master_width=$((work_width / 2))
stack_width=$((work_width - master_width))
stack_x=$((work_x + master_width))
stack_height=$((work_height / 3))
last_stack_height=$((work_height - stack_height * 2))
middle_stack_y=$((work_y + stack_height))
last_stack_y=$((middle_stack_y + stack_height))
outer_targets=(
    "${master_width}x${work_height}_${work_x}_${work_y}"
    "${stack_width}x${stack_height}_${stack_x}_${work_y}"
    "${stack_width}x${stack_height}_${stack_x}_${middle_stack_y}"
    "${stack_width}x${last_stack_height}_${stack_x}_${last_stack_y}"
)
targets=()
for outer_target in "${outer_targets[@]}"; do
    targets+=("$(inset_geometry "$outer_target" "$chrome_clearance")")
done

mapfile -t managed_observations < <(
    grep -nE \
        '^sophia_session_app schema=2 status=surface_observed source=action transaction=[0-9]+ surface=[0-9]+$' \
        "$SESSION_LOG"
)
(( ${#managed_observations[@]} >= 1 )) ||
    fail "no action-launched managed surface was observed"
for observation in "${managed_observations[@]}"; do
    observation_line="${observation%%:*}"
    observation_record="${observation#*:}"
    action_transaction="$(field "$observation_record" transaction)"
    managed_surface="$(field "$observation_record" surface)"

    layout_match="$(
        awk -v start="$observation_line" '
            NR > start &&
                /^sophia_live_wm schema=1 status=layout_committed transaction=[0-9]+ surfaces=[0-9]+ moved_surfaces=[0-9]+ configure_deliveries=[0-9]+ outcome=Committed$/ {
                print NR ":" $0
                exit
            }
        ' "$SESSION_LOG"
    )"
    [[ -n "$layout_match" ]] ||
        fail "surface $managed_surface has no following committed WM layout"
    layout_line="${layout_match%%:*}"
    layout_record="${layout_match#*:}"
    layout_transaction="$(field "$layout_record" transaction)"
    (( layout_transaction > action_transaction )) ||
        fail "surface $managed_surface layout did not follow its launch action"

    projection="$(
        grep -Em1 \
            "^sophia_live_wm schema=2 status=workspace_projection_committed transaction=${layout_transaction} output=[0-9]+ workspace=[0-9]+ visible_surfaces=[0-9]+ focus=surface$" \
            "$SESSION_LOG" || true
    )"
    [[ -n "$projection" ]] ||
        fail "surface $managed_surface layout has no focused workspace projection"
    projection_output="$(field "$projection" output)"
    visible_surfaces="$(field "$projection" visible_surfaces)"
    (( visible_surfaces >= 1 && visible_surfaces <= 4 )) ||
        fail "surface $managed_surface projection has unsupported Tall count $visible_surfaces"
    moved_surfaces="$(field "$layout_record" moved_surfaces)"
    (( moved_surfaces == visible_surfaces )) ||
        fail "surface $managed_surface ManageSurface commit moved $moved_surfaces of $visible_surfaces visible surfaces"

    projection_work_area_record="$(
        grep -E \
            "^sophia_live_work_area schema=1 status=applied output=${projection_output} " \
            "$SESSION_LOG" | tail -n 1
    )"
    [[ -n "$projection_work_area_record" ]] ||
        fail "surface $managed_surface output has no Engine work area"
    projection_work_area="$(field "$projection_work_area_record" work)"
    if [[ "$projection_work_area" =~ ^([0-9]+)x([0-9]+)_(-?[0-9]+)_(-?[0-9]+)$ ]]; then
        projection_width="${BASH_REMATCH[1]}"
        projection_height="${BASH_REMATCH[2]}"
        projection_x="${BASH_REMATCH[3]}"
        projection_y="${BASH_REMATCH[4]}"
    else
        fail "surface $managed_surface work-area geometry is malformed: $projection_work_area"
    fi
    if (( visible_surfaces == 1 )); then
        expected_managed_allocation="$projection_work_area"
    else
        expected_managed_allocation="$((projection_width / 2))x${projection_height}_${projection_x}_${projection_y}"
    fi
    expected_managed_target="$(
        inset_geometry "$expected_managed_allocation" "$chrome_clearance"
    )"

    present_match="$(
        awk -v start="$layout_line" -v surface="$managed_surface" '
            NR > start &&
                /^sophia_live_session_present schema=2 status=retired transaction=[0-9]+ surface=/ &&
                index($0, " surface=" surface " ") {
                print NR ":" $0
                exit
            }
        ' "$SESSION_LOG"
    )"
    [[ -n "$present_match" ]] ||
        fail "surface $managed_surface has no retired Present after ManageSurface"
    present_record="${present_match#*:}"
    present_source="$(field "$present_record" source)"
    present_target="$(field "$present_record" target)"
    present_clip="$(field "$present_record" clip)"
    present_scale="$(field "$present_record" unit_scale)"
    [[ "$present_target" == "$expected_managed_target" ]] ||
        fail "surface $managed_surface first Present used $present_target, expected $expected_managed_target"
    [[ "$present_source" == "${present_target%%_*}" &&
        "$present_clip" == "$present_target" &&
        "$present_scale" == true ]] ||
        fail "surface $managed_surface first Present did not consume one pixel-matched layout snapshot"
    [[ "$present_target" != *_80_60 ]] ||
        fail "surface $managed_surface presented at the admission staging offset"

    following_layout="$(
        awk -v start="$layout_line" '
            NR > start &&
                /^sophia_live_wm schema=1 status=layout_committed transaction=[0-9]+ surfaces=[0-9]+ moved_surfaces=[0-9]+ configure_deliveries=[0-9]+ outcome=Committed$/ {
                print
                exit
            }
        ' "$SESSION_LOG"
    )"
    [[ -n "$following_layout" ]] ||
        fail "surface $managed_surface ManageSurface commit has no following stability transaction"
    following_moved="$(field "$following_layout" moved_surfaces)"
    (( following_moved == 0 )) ||
        fail "surface $managed_surface moved again in the following transaction"
done

for target in "${targets[@]}"; do
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
    '^sophia_live_session_keys schema=2 status=complete pending=0 release_barrier_pending=0 peak_pressed=[0-9]+ synthetic_releases=[0-9]+ state_only_releases=[0-9]+ orphan_releases_suppressed=[0-9]+ removed_surface_keys=0 repeat_active_seats=0 repeat_armed=[0-9]+ repeat_routed=[0-9]+ repeat_pulses=[0-9]+ repeat_coalesced=[0-9]+ repeat_cancelled=[0-9]+ repeat_capacity_exhausted=0$' \
    "$SESSION_LOG" ||
    fail "client pressed-key state did not drain"
mapfile -t completions < <(
    grep -E '^sophia_live_session schema=(15|16) status=bounded_complete ' "$SESSION_LOG"
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
    native_frame_surface_creations \
    native_max_target_create_msec native_max_frame_surface_create_msec \
    native_max_render_msec \
    native_max_submit_to_page_flip_msec native_max_upload_msec \
    input_queue_dwell_max_msec; do
    value="$(field "$completion" "$key")" ||
        fail "completion is missing $key"
    [[ "$value" =~ ^[0-9]+$ ]] ||
        fail "completion has nonnumeric $key=$value"
    if [[ "$key" == native_mixed_exports ]]; then
        (( value > 0 )) ||
            fail "four-window session produced no mixed exports"
    elif [[ "$key" != native_target_recreations &&
        "$key" != native_frame_surface_creations ]]; then
        (( value <= 100 )) ||
            fail "$key exceeded the 100ms promotion budget: $value"
    fi
done

mapfile -t resource_lines < <(
    grep -E '^sophia_live_native_resources schema=(5|6) status=complete ' "$SESSION_LOG"
)
(( ${#resource_lines[@]} == 1 )) ||
    fail "expected one native resource-lifetime record"
resources="${resource_lines[0]}"
for key in target_creations pipeline_creations frame_surface_creations cpu_target_creations \
    dmabuf_target_creations composition_target_creations composition_target_reuses \
    generation_replacements recovery_replacements snapshot_captures snapshot_promotions \
    snapshot_rollbacks snapshot_evictions snapshot_live_entries snapshot_live_bytes \
    import_cache_imports import_cache_hits \
    import_cache_evictions import_cache_live_entries import_cache_descriptor_mismatches \
    import_cache_capacity_rejections worker_requests worker_completions worker_failures \
    worker_soft_stalls worker_hard_stalls worker_release_enqueue_failures \
    max_worker_request_msec; do
    value="$(field "$resources" "$key")" ||
        fail "resource-lifetime record is missing $key"
    [[ "$value" =~ ^[0-9]+$ ]] ||
        fail "resource-lifetime record has nonnumeric $key=$value"
done
target_creations="$(field "$resources" target_creations)"
pipeline_creations="$(field "$resources" pipeline_creations)"
frame_surface_creations="$(field "$resources" frame_surface_creations)"
cpu_targets="$(field "$resources" cpu_target_creations)"
dmabuf_targets="$(field "$resources" dmabuf_target_creations)"
composition_targets="$(field "$resources" composition_target_creations)"
composition_target_reuses="$(field "$resources" composition_target_reuses)"
generation_replacements="$(field "$resources" generation_replacements)"
recovery_replacements="$(field "$resources" recovery_replacements)"
snapshot_captures="$(field "$resources" snapshot_captures)"
snapshot_promotions="$(field "$resources" snapshot_promotions)"
snapshot_rollbacks="$(field "$resources" snapshot_rollbacks)"
snapshot_evictions="$(field "$resources" snapshot_evictions)"
snapshot_live_entries="$(field "$resources" snapshot_live_entries)"
snapshot_live_bytes="$(field "$resources" snapshot_live_bytes)"
import_cache_imports="$(field "$resources" import_cache_imports)"
import_cache_hits="$(field "$resources" import_cache_hits)"
import_cache_evictions="$(field "$resources" import_cache_evictions)"
import_cache_live_entries="$(field "$resources" import_cache_live_entries)"
import_cache_descriptor_mismatches="$(field "$resources" import_cache_descriptor_mismatches)"
import_cache_capacity_rejections="$(field "$resources" import_cache_capacity_rejections)"
worker_requests="$(field "$resources" worker_requests)"
worker_completions="$(field "$resources" worker_completions)"
worker_failures="$(field "$resources" worker_failures)"
worker_soft_stalls="$(field "$resources" worker_soft_stalls)"
worker_hard_stalls="$(field "$resources" worker_hard_stalls)"
worker_release_enqueue_failures="$(field "$resources" worker_release_enqueue_failures)"
max_worker_request_msec="$(field "$resources" max_worker_request_msec)"
mixed_exports="$(field "$completion" native_mixed_exports)"
target_recreations="$(field "$completion" native_target_recreations)"
completion_frame_surfaces="$(field "$completion" native_frame_surface_creations)"
(( target_creations == pipeline_creations )) ||
    fail "target and pipeline creation counts diverged"
(( target_creations == cpu_targets + dmabuf_targets + composition_targets )) ||
    fail "resource-class creation counts do not sum to the total"
(( composition_targets > 0 && composition_targets + composition_target_reuses == mixed_exports )) ||
    fail "persistent composition target creation and reuse did not cover every mixed export"
(( frame_surface_creations == target_creations )) ||
    fail "render-target and frame-surface creation counts diverged"
(( completion_frame_surfaces == frame_surface_creations )) ||
    fail "completion and resource frame-surface counts diverged"
(( target_recreations == 0 )) ||
    fail "stable composition resources were recreated"
(( generation_replacements == 0 && recovery_replacements == 0 )) ||
    fail "stable CPU or direct DMA-BUF resources were replaced"
(( snapshot_captures > 0 && snapshot_captures == snapshot_promotions &&
    snapshot_captures == snapshot_evictions && snapshot_rollbacks == 0 &&
    snapshot_live_entries == 0 && snapshot_live_bytes == 0 )) ||
    fail "renderer-owned Present snapshots did not promote and drain exactly"
(( import_cache_imports > 0 && import_cache_hits > 0 )) ||
    fail "four-window composition did not exercise persistent import-cache reuse"
(( import_cache_imports == import_cache_evictions && import_cache_live_entries == 0 )) ||
    fail "import-cache resources did not drain completely"
(( import_cache_descriptor_mismatches == 0 && import_cache_capacity_rejections == 0 )) ||
    fail "import-cache validation or capacity rejected an export"
(( worker_requests > 0 && worker_requests == worker_completions )) ||
    fail "renderer-worker requests did not complete"
(( worker_failures == 0 && worker_soft_stalls == 0 && worker_hard_stalls == 0 &&
    worker_release_enqueue_failures == 0 )) ||
    fail "renderer worker failed, stalled, or lost a release"
(( max_worker_request_msec <= 100 )) ||
    fail "renderer-worker request exceeded the 100ms promotion budget: $max_worker_request_msec"

mapfile -t owner_timing_lines < <(
    grep -E '^sophia_live_owner_timing schema=2 status=complete ' "$SESSION_LOG"
)
(( ${#owner_timing_lines[@]} == 1 )) ||
    fail "expected one owner phase-timing record"
owner_timing="${owner_timing_lines[0]}"
for key in max_child_reap_msec max_input_phase_msec; do
    value="$(field "$owner_timing" "$key")" ||
        fail "owner phase-timing record is missing $key"
    [[ "$value" =~ ^[0-9]+$ ]] ||
        fail "owner phase-timing record has nonnumeric $key=$value"
    (( value <= 100 )) ||
        fail "$key exceeded the 100ms promotion budget: $value"
done

mapfile -t wm_transport_lines < <(
    grep -E '^sophia_live_wm_transport schema=2 status=complete ' "$SESSION_LOG"
)
(( ${#wm_transport_lines[@]} == 1 )) ||
    fail "expected one WM transport completion record"
wm_transport="${wm_transport_lines[0]}"
for assignment in pending=0 rejected=0; do
    [[ " $wm_transport " == *" $assignment "* ]] ||
        fail "WM transport ledger was not clean: $assignment"
done
for key in peak_depth action_ordered action_coalesced stale_responses max_queue_dwell_msec max_round_trip_msec; do
    value="$(field "$wm_transport" "$key")" ||
        fail "WM transport record is missing $key"
    [[ "$value" =~ ^[0-9]+$ ]] ||
        fail "WM transport record has nonnumeric $key=$value"
    case "$key" in
        peak_depth)
            (( value >= 1 && value <= 16 )) ||
                fail "WM transport peak depth exceeded its bound: $value"
            ;;
        stale_responses)
            (( value <= 16 )) ||
                fail "WM transport rejected too many stale responses: $value"
            ;;
        action_ordered)
            (( value >= 1 )) ||
                fail "WM transport did not retain an ordered physical action"
            ;;
        action_coalesced)
            (( value == 0 )) ||
                fail "WM transport coalesced a non-idempotent physical action: $value"
            ;;
        *)
            (( value <= 500 )) ||
                fail "$key exceeded the 500ms WM transport budget: $value"
            ;;
    esac
done

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
