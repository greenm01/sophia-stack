#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(dirname "$SCRIPT_PATH")"
if [[ -f "$SCRIPT_DIR/lib/installed_soak_evidence.sh" ]]; then
    source "$SCRIPT_DIR/lib/installed_soak_evidence.sh"
else
    source "$SCRIPT_DIR/../tools/lib/installed_soak_evidence.sh"
fi

session_log="${1:-}"
minimum_msec="${2:-7200000}"
minimum_terminal_actions="${3:-10}"
minimum_firefox_actions="${4:-5}"
[[ -s "$session_log" ]] || {
    echo "usage: tools/verify_installed_session_soak.sh SESSION_LOG [MIN_MSEC [MIN_TERMINALS [MIN_FIREFOX]]]" >&2
    exit 1
}
for value in "$minimum_msec" "$minimum_terminal_actions" "$minimum_firefox_actions"; do
    [[ "$value" =~ ^[0-9]+$ ]] || {
        echo "soak thresholds must be nonnegative integers" >&2
        exit 1
    }
done
if grep -Eqi '(^|[[:space:]])(panic|error([:[:space:]])|status=(failed|degraded))' "$session_log"; then
    echo "soak log contains an error, panic, or degraded status" >&2
    exit 1
fi
if grep -Eqi 'corrupted size vs\. prev_size|free\(\): invalid pointer|double free|malloc\(\):|allocator (error|failure|diagnostic)|out of memory' \
    "$session_log"; then
    echo "soak log contains an allocator diagnostic" >&2
    exit 1
fi
mapfile -t completions < <(
    grep -E '^sophia_live_session schema=(14|15|16) status=bounded_complete ' "$session_log" || true
)
(( ${#completions[@]} == 1 )) || {
    echo "soak requires exactly one supported completion; found ${#completions[@]}" >&2
    exit 1
}
completion="${completions[0]}"
line_field() {
    local line="$1" name="$2" token
    for token in $line; do
        [[ "$token" == "$name="* ]] && {
            printf '%s\n' "${token#*=}"
            return
        }
    done
    return 1
}
field() {
    line_field "$completion" "$1"
}
elapsed="$(field elapsed_msec)"
[[ "$elapsed" =~ ^[0-9]+$ ]] && (( elapsed >= minimum_msec )) || {
    echo "soak duration ${elapsed:-missing}ms is below ${minimum_msec}ms" >&2
    exit 1
}
terminal_actions="$(
    sophia_soak_count "$session_log" \
        '^sophia_session_app schema=(1|2) status=started id=terminal source=action( |$)'
)"
firefox_actions="$(
    sophia_soak_count "$session_log" \
        '^sophia_session_app schema=(1|2) status=started id=firefox source=action( |$)'
)"
layout_commits="$(
    grep -Ec '^sophia_live_wm schema=1 status=layout_committed .* outcome=Committed$' \
        "$session_log" || true
)"
focus_commits="$(
    grep -Ec '^sophia_live_wm schema=1 status=focus_committed .* target=surface$' \
        "$session_log" || true
)"
close_actions="$(
    grep -Ec '^sophia_live_wm schema=1 status=session_action_committed .* action=CloseFocused$' \
        "$session_log" || true
)"
terminal_exits="$(
    grep -Ec '^sophia_session_app schema=1 status=exited id=terminal .* exit_status=exit status: 0$' \
        "$session_log" || true
)"
firefox_exits="$(
    grep -Ec '^sophia_session_app schema=1 status=exited id=firefox .* exit_status=exit status: 0$' \
        "$session_log" || true
)"
workspace_away="$(
    grep -Ec '^sophia_live_wm schema=2 status=workspace_projection_committed .* focus=none$' \
        "$session_log" || true
)"
workspace_return="$(
    grep -Ec '^sophia_live_wm schema=2 status=workspace_projection_committed .* focus=surface$' \
        "$session_log" || true
)"
visual_resizes="$(
    grep -Ec '^sophia_live_resize_epoch schema=3 status=visual_committed .* width=[1-9][0-9]* height=[1-9][0-9]*$' \
        "$session_log" || true
)"
(( terminal_actions >= minimum_terminal_actions )) || {
    echo "soak has $terminal_actions terminal actions; $minimum_terminal_actions required" >&2
    exit 1
}
(( firefox_actions >= minimum_firefox_actions )) || {
    echo "soak has $firefox_actions Firefox actions; $minimum_firefox_actions required" >&2
    exit 1
}
(( layout_commits >= minimum_terminal_actions + minimum_firefox_actions )) || {
    echo "soak has $layout_commits layout commits; $((minimum_terminal_actions + minimum_firefox_actions)) required" >&2
    exit 1
}
(( terminal_exits >= minimum_terminal_actions )) || {
    echo "soak has $terminal_exits clean terminal exits; $minimum_terminal_actions required" >&2
    exit 1
}
(( firefox_exits >= minimum_firefox_actions )) || {
    echo "soak has $firefox_exits clean Firefox exits; $minimum_firefox_actions required" >&2
    exit 1
}
(( focus_commits >= 2 )) || {
    echo "soak has $focus_commits committed focus changes; two required" >&2
    exit 1
}
(( workspace_away >= 2 && workspace_return >= 2 )) || {
    echo "soak workspace switching is incomplete: away=$workspace_away return=$workspace_return" >&2
    exit 1
}
(( visual_resizes >= 2 )) || {
    echo "soak has $visual_resizes visually committed resizes; two required" >&2
    exit 1
}
minimum_close_actions=$((minimum_terminal_actions + minimum_firefox_actions))
(( close_actions >= minimum_close_actions )) || {
    echo "soak has $close_actions close actions; $minimum_close_actions required" >&2
    exit 1
}
for action in "${SOPHIA_SOAK_PRACTICAL_ACTION_IDS[@]}"; do
    action_count="$(sophia_soak_action_count "$session_log" "$action")"
    (( action_count >= 1 )) || {
        echo "soak is missing practical action $(sophia_soak_action_name "$action") ($action)" >&2
        exit 1
    }
done
workspace_views="$(sophia_soak_workspace_view_count "$session_log")"
workspace_moves="$(sophia_soak_workspace_move_count "$session_log")"
pointer_moves="$(sophia_soak_count "$session_log" '^sophia_live_wm schema=4 status=pointer_gesture_committed mode=move$')"
pointer_resizes="$(sophia_soak_count "$session_log" '^sophia_live_wm schema=4 status=pointer_gesture_committed mode=resize$')"
(( workspace_views >= 1 && workspace_moves >= 1 )) || {
    echo "soak requires one physical workspace view and move: view=$workspace_views move=$workspace_moves" >&2
    exit 1
}
(( pointer_moves >= 1 && pointer_resizes >= 1 )) || {
    echo "soak requires one committed pointer move and resize: move=$pointer_moves resize=$pointer_resizes" >&2
    exit 1
}
if grep -Eq '^sophia_live_wm schema=1 status=layout_timeout |^sophia_live_resize_epoch schema=[0-9]+ status=(aborted|queue_aborted) ' \
    "$session_log"; then
    echo "soak contains a layout timeout or aborted resize epoch" >&2
    exit 1
fi
mapfile -t layout_authority_summaries < <(
    grep -E '^sophia_live_layout_authority schema=1 status=' "$session_log" || true
)
[[ "${#layout_authority_summaries[@]}" == 1 \
    && "${layout_authority_summaries[0]}" == \
        'sophia_live_layout_authority schema=1 status=clean hidden_surface_commands=0' ]] || {
    echo "soak lacks the zero hidden-surface-command invariant" >&2
    exit 1
}
mapfile -t wm_transport_summaries < <(
    grep -E '^sophia_live_wm_transport schema=2 status=complete ' "$session_log" || true
)
[[ "${#wm_transport_summaries[@]}" == 1 \
    && " ${wm_transport_summaries[0]} " == *' pending=0 rejected=0 '* \
    && " ${wm_transport_summaries[0]} " == *' stale_responses=0 '* ]] || {
    echo "soak WM transport has pending, rejected, or stale work" >&2
    exit 1
}
mapfile -t selections < <(
    grep -E '^sophia_live_selection schema=1 status=complete .* content=redacted$' \
        "$session_log" || true
)
(( ${#selections[@]} == 1 )) || {
    echo "soak requires exactly one redacted selection summary" >&2
    exit 1
}
selection="${selections[0]}"
selection_owner_changes="$(line_field "$selection" owner_changes || true)"
selection_conversions="$(line_field "$selection" conversions || true)"
[[ "$selection_owner_changes" =~ ^[0-9]+$ \
    && "$selection_conversions" =~ ^[0-9]+$ \
    && "$selection_owner_changes" -ge 2 \
    && "$selection_conversions" -ge 2 ]] || {
    echo "soak clipboard activity is incomplete: owner_changes=${selection_owner_changes:-missing} conversions=${selection_conversions:-missing}" >&2
    exit 1
}
grep -Eq '^sophia_live_session_protocol_errors schema=1 expected=[0-9]+ unexpected=0$' \
    "$session_log" || {
    echo "soak protocol-error summary is missing or nonzero" >&2
    exit 1
}
mapfile -t output_summaries < <(
    grep -E '^sophia_live_output schema=1 status=complete .*callbacks=[1-9][0-9]* .*nonzero_exports=[1-9][0-9]*$' \
        "$session_log" || true
)
declare -A output_ids=()
for output_summary in "${output_summaries[@]}"; do
    output_id="$(line_field "$output_summary" output || true)"
    [[ "$output_id" =~ ^[0-9]+$ ]] || {
        echo "soak output summary has no numeric output identity" >&2
        exit 1
    }
    output_ids[$output_id]=1
done
outputs="${#output_ids[@]}"
(( outputs >= 2 )) || {
    echo "soak has $outputs distinct clean output summaries; two required" >&2
    exit 1
}
grep -Eq '^sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 namespace=revoked xauthority=removed$' \
    "$session_log" || {
    echo "soak final process/frontend cleanup is not clean" >&2
    exit 1
}
for positive in physical_events physical_keys_routed physical_pointer_events \
    physical_pointer_routed wm_requests wm_committed native_submissions \
    native_retirements native_frame_uploads; do
    actual="$(field "$positive")"
    [[ "$actual" =~ ^[0-9]+$ ]] && (( actual > 0 )) || {
        echo "soak completion has no $positive activity" >&2
        exit 1
    }
done
for assignment in \
    wm_degraded=false \
    native_submit_failures=0 \
    native_retire_failures=0 \
    native_callback_rejected=0 \
    native_callback_queue_saturated=0 \
    native_in_flight=false \
    native_cleanup_pending=false \
    authority_batches_dropped=0 \
    wm_restarts=0 \
    present_disconnect_failures=0 \
    present_route_failures=0 \
    present_live_sources=0 \
    present_live_fences=0 \
    present_live_transactions=0 \
    present_controlled_rejections=0; do
    actual="$(field "${assignment%%=*}")"
    [[ "$actual" == "${assignment#*=}" ]] || {
        echo "soak completion violates $assignment (actual=${actual:-missing})" >&2
        exit 1
    }
done
input_expected="$(field input_events_expected || true)"
input_flushed="$(field input_events_flushed || true)"
[[ "$input_expected" =~ ^[0-9]+$ \
    && "$input_flushed" == "$input_expected" ]] || {
    echo "soak input queue did not drain: expected=${input_expected:-missing} flushed=${input_flushed:-missing}" >&2
    exit 1
}
mapfile -t key_summaries < <(
    grep -E '^sophia_live_session_keys schema=2 status=complete ' "$session_log" || true
)
(( ${#key_summaries[@]} == 1 )) || {
    echo "soak requires exactly one held-key summary" >&2
    exit 1
}
keys="${key_summaries[0]}"
for assignment in pending=0 release_barrier_pending=0 repeat_active_seats=0 \
    repeat_capacity_exhausted=0; do
    actual="$(line_field "$keys" "${assignment%%=*}" || true)"
    [[ "$actual" == "${assignment#*=}" ]] || {
        echo "soak held-key summary violates $assignment (actual=${actual:-missing})" >&2
        exit 1
    }
done
mapfile -t cursor_summaries < <(
    grep -E '^sophia_live_session_cursor schema=4 path=legacy_ioctl ' \
        "$session_log" || true
)
(( ${#cursor_summaries[@]} == 1 )) || {
    echo "soak requires exactly one cursor summary" >&2
    exit 1
}
cursor="${cursor_summaries[0]}"
for assignment in hidden_updates=0 hardware_failures=0; do
    actual="$(line_field "$cursor" "${assignment%%=*}" || true)"
    [[ "$actual" == "${assignment#*=}" ]] || {
        echo "soak cursor summary violates $assignment (actual=${actual:-missing})" >&2
        exit 1
    }
done
mapfile -t page_flip_clocks < <(
    grep -E '^sophia_live_page_flip_clock schema=1 status=complete source=kernel_monotonic ' \
        "$session_log" || true
)
(( ${#page_flip_clocks[@]} == 1 )) || {
    echo "soak requires exactly one kernel page-flip clock summary" >&2
    exit 1
}
page_flip_clock="${page_flip_clocks[0]}"
timestamps="$(line_field "$page_flip_clock" timestamps || true)"
fallbacks="$(line_field "$page_flip_clock" fallbacks || true)"
pending_page_flips="$(line_field "$page_flip_clock" pending || true)"
[[ "$timestamps" =~ ^[1-9][0-9]*$ \
    && "$fallbacks" == 0 \
    && "$pending_page_flips" == 0 ]] || {
    echo "soak page-flip clock is incomplete: timestamps=${timestamps:-missing} fallbacks=${fallbacks:-missing} pending=${pending_page_flips:-missing}" >&2
    exit 1
}
grep -Eq '^sophia_live_session_health schema=1 status=clean .* pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false$' \
    "$session_log" || {
    echo "soak final health is not clean" >&2
    exit 1
}

echo "installed Sophia soak gate passed: elapsed_msec=$elapsed terminal_actions=$terminal_actions firefox_actions=$firefox_actions layout_commits=$layout_commits focus_commits=$focus_commits workspace_away=$workspace_away workspace_return=$workspace_return workspace_views=$workspace_views workspace_moves=$workspace_moves pointer_moves=$pointer_moves pointer_resizes=$pointer_resizes practical_actions=${#SOPHIA_SOAK_PRACTICAL_ACTION_IDS[@]} visual_resizes=$visual_resizes close_actions=$close_actions selection_owner_changes=$selection_owner_changes selection_conversions=$selection_conversions outputs=$outputs"
