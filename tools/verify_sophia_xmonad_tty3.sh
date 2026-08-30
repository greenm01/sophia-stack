#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
LOG_DIR="${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"
GUARD_LOG="${2:-$LOG_DIR/input-guard.log}"
RECOVERY_LOG="${3:-$LOG_DIR/recovery.log}"

fail() {
    echo "xmonad TTY3 verification failed: $*" >&2
    exit 1
}
require_file() {
    [[ -s "$1" ]] || fail "missing or empty evidence file: $1"
}
require_line() {
    local pattern="$1" file="$2" description="$3"
    grep -Eq "$pattern" "$file" || fail "$description ($file)"
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
require_eq() {
    local line="$1" key="$2" expected="$3" actual
    actual="$(field "$line" "$key")" || fail "completion is missing $key"
    [[ "$actual" == "$expected" ]] || fail "$key is $actual, expected $expected"
}
require_positive() {
    local line="$1" key="$2" actual
    actual="$(field "$line" "$key")" || fail "completion is missing $key"
    [[ "$actual" =~ ^[0-9]+$ ]] || fail "$key is not an integer: $actual"
    (( actual > 0 )) || fail "$key did not record activity"
}
require_count_at_least() {
    local pattern="$1" file="$2" minimum="$3" description="$4" count
    count="$(grep -Ec "$pattern" "$file" || true)"
    (( count >= minimum )) ||
        fail "$description: observed $count, required $minimum ($file)"
}
require_value_at_least() {
    local line="$1" key="$2" minimum="$3" actual
    actual="$(field "$line" "$key")" || fail "record is missing $key"
    [[ "$actual" =~ ^[0-9]+$ ]] || fail "$key is not an integer: $actual"
    (( actual >= minimum )) || fail "$key is $actual, expected at least $minimum"
}
require_value_at_most() {
    local line="$1" key="$2" maximum="$3" actual
    actual="$(field "$line" "$key")" || fail "record is missing $key"
    [[ "$actual" =~ ^[0-9]+$ ]] || fail "$key is not an integer: $actual"
    (( actual <= maximum )) || fail "$key is $actual, expected at most $maximum"
}
line_number() {
    grep -nEm1 "$1" "$2" | cut -d: -f1
}
line_number_after() {
    local pattern="$1" file="$2" minimum="$3"
    grep -nE "$pattern" "$file" |
        cut -d: -f1 |
        awk -v minimum="$minimum" '$1 > minimum { print; exit }'
}

require_file "$SESSION_LOG"
require_file "$GUARD_LOG"
require_file "$RECOVERY_LOG"

if grep -Eqi '(^Error:|panicked at|^sophia_[^[:space:]]+ .*status=(failed|degraded)([[:space:]]|$))' \
    "$SESSION_LOG"; then
    fail "session log contains a Sophia error, panic, or degraded status"
fi
if grep -Fq 'Failed to become owner of clipboard selection' "$SESSION_LOG"; then
    fail "Kitty could not acquire the X11 clipboard selection"
fi
if grep -Fq 'Failed to convert selection to data from clipboard' "$SESSION_LOG"; then
    fail "Kitty could not convert the X11 clipboard selection"
fi

require_line '^sophia_live_wm schema=1 status=ready adapter=external socket=private restarts=0$' \
    "$SESSION_LOG" "external xmonad policy never became ready"
require_line '^sophia_session_app schema=1 status=started id=terminal source=startup$' \
    "$SESSION_LOG" "startup Kitty was not launched"
require_line \
    '^sophia_session_app schema=1 status=exited id=terminal source=startup exit_status=exit status: 0$' \
    "$SESSION_LOG" "startup Kitty did not exit cleanly"
require_line \
    '^sophia_live_session_input_pipeline schema=1 status=desktop_pointer_active source=post_startup_exit$' \
    "$SESSION_LOG" "pointer input did not remain active after the last window closed"
require_line '^sophia_session_app schema=1 status=started id=terminal source=action$' \
    "$SESSION_LOG" "Super-Enter did not launch a second Kitty"
require_count_at_least \
    '^sophia_session_app schema=1 status=started id=terminal source=(startup|action)$' \
    "$SESSION_LOG" 3 "the startup, clipboard-peer, and desktop-relaunch Kittys were not recorded"
require_line '^sophia_live_wm schema=1 status=layout_committed .* outcome=Committed$' \
    "$SESSION_LOG" "xmonad did not commit a layout"
require_line '^sophia_live_wm schema=1 status=focus_committed .* target=surface$' \
    "$SESSION_LOG" "xmonad did not commit focus"
startup_content="$(
    grep -E '^sophia_live_session_startup schema=2 status=content_ready source=stable_present_scanout nonzero_rgb_pixels=[1-9][0-9]*$' \
        "$SESSION_LOG" | head -n 1
)"
[[ -n "$startup_content" ]] ||
    fail "startup never proved nonzero mixed-composition pixels"
require_line \
    '^sophia_live_session_startup schema=2 status=output_baseline_ready outputs=2/2$' \
    "$SESSION_LOG" "both native outputs did not establish a retired startup baseline"
startup_ready="$(
    grep -E '^sophia_live_session_startup schema=2 status=ready ' "$SESSION_LOG" |
        head -n 1
)"
[[ -n "$startup_ready" ]] || fail "startup readiness evidence is missing"
require_eq "$startup_ready" outputs_ready 2/2
recovery_attempts="$(field "$startup_ready" recovery_attempts)" ||
    fail "startup readiness is missing recovery_attempts"
[[ "$recovery_attempts" == 0 || "$recovery_attempts" == 1 ]] ||
    fail "startup recovery attempts is $recovery_attempts, expected 0 or 1"
require_line '^sophia_live_wm schema=1 status=session_action_committed .* action=Logout$' \
    "$SESSION_LOG" "normal Super-Shift-Q logout was not committed"
for action in \
    1 \
    3 \
    257 \
    258 \
    259 \
    769; do
    require_line \
        "^sophia_live_wm schema=1 status=physical_action_committed action=${action}$" \
        "$SESSION_LOG" "required physical WM action $action was not committed"
done
startup_exit_line="$(
    line_number 'status=exited id=terminal source=startup exit_status=exit status: 0$' "$SESSION_LOG"
)"
clipboard_peer_line="$(
    line_number 'status=started id=terminal source=action$' "$SESSION_LOG"
)"
desktop_pointer_line="$(
    line_number 'status=desktop_pointer_active source=post_startup_exit$' "$SESSION_LOG"
)"
super_enter_line="$(
    line_number_after \
        'status=physical_action_committed action=768$' \
        "$SESSION_LOG" "$desktop_pointer_line"
)"
action_terminal_line="$(
    line_number_after \
        'status=started id=terminal source=action$' \
        "$SESSION_LOG" "$super_enter_line"
)"
(( clipboard_peer_line < startup_exit_line
    && startup_exit_line < desktop_pointer_line
    && desktop_pointer_line < super_enter_line
    && super_enter_line < action_terminal_line )) ||
    fail "clipboard peer, last-window exit, pointer recovery, and desktop relaunch are out of order"
require_line '^sophia_live_wm schema=1 status=hidden_focus_cleared transaction=[0-9]+$' \
    "$SESSION_LOG" "workspace-away did not clear Engine and X11 focus"
require_line '^sophia_live_session_input_pipeline schema=2 status=key_suppressed reason=no_focus$' \
    "$SESSION_LOG" "no key was suppressed while the workspace had no focus"
workspace_away_line="$(line_number 'status=physical_action_committed action=258$' "$SESSION_LOG")"
workspace_two_projection_line="$(
    line_number \
        'schema=2 status=workspace_projection_committed transaction=[0-9]+ output=1 workspace=2 visible_surfaces=0 focus=none$' \
        "$SESSION_LOG"
)"
focus_clear_line="$(line_number 'status=hidden_focus_cleared ' "$SESSION_LOG")"
suppressed_key_line="$(line_number 'status=key_suppressed reason=no_focus$' "$SESSION_LOG")"
workspace_three_action_line="$(
    line_number_after \
        'status=physical_action_committed action=259$' \
        "$SESSION_LOG" "$suppressed_key_line"
)"
workspace_three_projection_line="$(
    line_number_after \
        'schema=2 status=workspace_projection_committed transaction=[0-9]+ output=1 workspace=3 visible_surfaces=0 focus=none$' \
        "$SESSION_LOG" "$workspace_three_action_line"
)"
workspace_return_line="$(
    line_number_after \
        'status=physical_action_committed action=257$' \
        "$SESSION_LOG" "$workspace_three_projection_line"
)"
workspace_one_projection_line="$(
    line_number_after \
        'schema=2 status=workspace_projection_committed transaction=[0-9]+ output=1 workspace=1 visible_surfaces=[1-9][0-9]* focus=surface$' \
        "$SESSION_LOG" "$workspace_return_line"
)"
workspace_focus_restore_line="$(
    line_number_after \
        'status=workspace_focus_restore_queued transaction=[0-9]+ surface=[0-9]+$' \
        "$SESSION_LOG" "$workspace_one_projection_line"
)"
(( workspace_away_line < workspace_two_projection_line
    && workspace_two_projection_line < focus_clear_line
    && focus_clear_line < suppressed_key_line
    && suppressed_key_line < workspace_three_action_line
    && workspace_three_action_line < workspace_three_projection_line
    && workspace_three_projection_line < workspace_return_line
    && workspace_return_line < workspace_one_projection_line
    && workspace_one_projection_line < workspace_focus_restore_line )) ||
    fail "workspace visibility, focus clearing, and focus restoration are out of order"
require_line \
    '^sophia_live_outputs schema=2 status=ready discovered=2 presentation=2 native_owned=2 multi_output_scanout=enabled ' \
    "$SESSION_LOG" "two-output native ownership was not established"
for output in 1 2; do
    require_line \
        "sophia_live_native_page_flip schema=1 status=retired output=${output} " \
        "$SESSION_LOG" "physical output $output did not retire a page flip"
done
for record in \
    'sophia_live_session_vt schema=4 status=queued target=[0-9]+ modifier_releases=[2-4]' \
    'sophia_live_session_vt schema=4 status=preparing target=[0-9]+' \
    'sophia_live_session_vt schema=6 status=quiesced target=[0-9]+ outcome=(drained|forced_detach_timeout|forced_detach_revoked) drained=(true|false) abandoned_scanouts=[0-9]+ skipped_present=(none|[0-9]+)' \
    'sophia_live_session_vt schema=4 status=requested target=[0-9]+' \
    'sophia_live_seat schema=1 status=release_pending' \
    'sophia_live_seat schema=1 status=suspended' \
    'sophia_live_seat schema=1 status=acquire_pending' \
    'sophia_live_seat schema=1 status=active source=resume'; do
    require_line "^${record}$" "$SESSION_LOG" "VT lifecycle record is missing: $record"
done
renderer_capture="$(
    grep -E '^sophia_live_renderer_handoff schema=1 status=captured images=[1-9][0-9]*$' \
        "$SESSION_LOG" | head -n 1
)"
[[ -n "$renderer_capture" ]] || fail "VT release did not retain renderer-owned snapshots"
renderer_restore="$(
    grep -E '^sophia_live_renderer_handoff schema=1 status=restored images=[1-9][0-9]* source=seat_resume$' \
        "$SESSION_LOG" | head -n 1
)"
[[ -n "$renderer_restore" ]] || fail "VT resume did not restore renderer-owned snapshots"
captured_images="$(field "$renderer_capture" images)" || fail "renderer capture omitted its count"
restored_images="$(field "$renderer_restore" images)" || fail "renderer restore omitted its count"
[[ "$captured_images" == "$restored_images" ]] ||
    fail "renderer-image handoff restored $restored_images of $captured_images images"
queued_vt_line="$(line_number 'schema=4 status=queued target=' "$SESSION_LOG")"
preparing_vt_line="$(line_number 'schema=4 status=preparing target=' "$SESSION_LOG")"
renderer_capture_line="$(line_number 'status=captured images=' "$SESSION_LOG")"
quiesced_vt_line="$(line_number 'schema=6 status=quiesced target=' "$SESSION_LOG")"
requested_vt_line="$(line_number 'schema=4 status=requested target=' "$SESSION_LOG")"
release_vt_line="$(line_number 'sophia_live_seat schema=1 status=release_pending$' "$SESSION_LOG")"
suspended_vt_line="$(line_number 'sophia_live_seat schema=1 status=suspended$' "$SESSION_LOG")"
acquire_vt_line="$(line_number 'sophia_live_seat schema=1 status=acquire_pending$' "$SESSION_LOG")"
renderer_restore_line="$(line_number 'status=restored images=.* source=seat_resume$' "$SESSION_LOG")"
resumed_vt_line="$(line_number 'sophia_live_seat schema=1 status=active source=resume$' "$SESSION_LOG")"
post_resume_flip_line="$(
    line_number_after \
        'sophia_live_native_page_flip schema=1 status=retired output=1 ' \
        "$SESSION_LOG" "$resumed_vt_line"
)"
[[ -n "$post_resume_flip_line" ]] || fail "no primary page flip retired after VT resume"
(( queued_vt_line < preparing_vt_line
    && preparing_vt_line < renderer_capture_line
    && renderer_capture_line < quiesced_vt_line
    && quiesced_vt_line < requested_vt_line
    && requested_vt_line < release_vt_line
    && release_vt_line < suspended_vt_line
    && suspended_vt_line < acquire_vt_line
    && acquire_vt_line < renderer_restore_line
    && renderer_restore_line < resumed_vt_line
    && resumed_vt_line < post_resume_flip_line )) ||
    fail "VT prepare, renderer handoff, release, acquire, and retirement records are out of order"
if grep -Eq 'status=forced_detach|outcome=forced_detach_|remained in flight during teardown' \
    "$SESSION_LOG"; then
    fail "operator-requested VT switch used the revoked-seat fallback"
fi
for boundary in \
    'horizontal minimum' \
    'horizontal maximum' \
    'vertical minimum' \
    'vertical maximum'; do
    read -r axis side <<<"$boundary"
    edge_line="$(
        line_number \
            "schema=7 status=output_edge_confined axis=${axis} side=${side}$" \
            "$SESSION_LOG"
    )"
    reverse_line="$(
        line_number_after \
            "schema=7 status=edge_reverse_immediate axis=${axis} side=${side}$" \
            "$SESSION_LOG" "$edge_line"
    )"
    [[ -n "$reverse_line" ]] ||
        fail "pointer did not reverse immediately from the ${axis} ${side} edge"
done
cursor="$(
    grep -E '^sophia_live_session_cursor schema=5 path=legacy_ioctl ' "$SESSION_LOG" | tail -n 1
)"
[[ -n "$cursor" ]] || fail "final cursor health record is missing"
require_value_at_least "$cursor" buttons_routed 2
require_value_at_most "$cursor" initialization_max_msec 100
require_value_at_most "$cursor" max_update_msec 100
require_value_at_least "$cursor" updates_primary_in_flight 1
require_eq "$cursor" hidden_updates 0
keys="$(
    grep -E '^sophia_live_session_keys schema=2 status=complete ' "$SESSION_LOG" |
        tail -n 1
)"
[[ -n "$keys" ]] || fail "final held-key repeat evidence is missing"
require_eq "$keys" pending 0
require_eq "$keys" release_barrier_pending 0
require_eq "$keys" repeat_active_seats 0
require_value_at_least "$keys" repeat_routed 2
require_eq "$keys" repeat_capacity_exhausted 0
repeat_routed="$(field "$keys" repeat_routed)" ||
    fail "held-key repeat evidence is missing repeat_routed"
repeat_pulses="$(field "$keys" repeat_pulses)" ||
    fail "held-key repeat evidence is missing repeat_pulses"
[[ "$repeat_routed" == "$repeat_pulses" ]] ||
    fail "held-key repeat did not drain exactly: routed=$repeat_routed pulses=$repeat_pulses"
require_line '^sophia_live_session_health schema=1 status=clean .* wm_degraded=false$' \
    "$SESSION_LOG" "final session health was not clean"
require_line '^sophia_live_session_protocol_errors schema=1 expected=[0-9]+ unexpected=0$' \
    "$SESSION_LOG" "normal session emitted an unexpected X protocol error"
selection="$(
    grep -E '^sophia_live_selection schema=1 status=complete ' "$SESSION_LOG" |
        tail -n 1
)"
[[ -n "$selection" ]] || fail "final selection evidence is missing"
require_value_at_least "$selection" owner_changes 2
require_value_at_least "$selection" conversions 2
require_line \
    '^sophia_live_session_present schema=2 status=retired transaction=[0-9]+ surface=[0-9]+ source=[1-9][0-9]*x[1-9][0-9]* target=[1-9][0-9]*x[1-9][0-9]*_-?[0-9]+_-?[0-9]+ clip=(none|[1-9][0-9]*x[1-9][0-9]*_-?[0-9]+_-?[0-9]+) unit_scale=true$' \
    "$SESSION_LOG" "no pixel-matched DMA-BUF presentation was retired"
if grep -Eq '^sophia_live_session_present schema=2 .* unit_scale=false$' "$SESSION_LOG"; then
    fail "session presented a DMA-BUF whose pixels did not match its logical surface"
fi

mapfile -t completions < <(
    grep -E '^sophia_live_session schema=(14|15|16) status=bounded_complete ' "$SESSION_LOG"
)
(( ${#completions[@]} == 1 )) ||
    fail "expected one supported completion, found ${#completions[@]}"
completion="${completions[0]}"

startup_ready_msec="$(field "$completion" startup_ready_msec)" ||
    fail "completion is missing startup_ready_msec"
[[ "$startup_ready_msec" =~ ^[0-9]+$ ]] ||
    fail "startup_ready_msec is not an integer: $startup_ready_msec"
(( startup_ready_msec <= 8000 )) ||
    fail "startup readiness took ${startup_ready_msec}ms (limit: 8000ms)"

for key in physical_events physical_keys_routed physical_pointer_events \
    physical_pointer_routed wm_requests wm_committed native_submissions \
    native_retirements native_frame_uploads; do
    require_positive "$completion" "$key"
done
for assignment in \
    native_presentation=enabled \
    physical_input=enabled \
    wm_policy=external \
    wm_restarts=0 \
    wm_degraded=false \
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
    require_eq "$completion" "${assignment%%=*}" "${assignment#*=}"
done

expected="$(field "$completion" input_events_expected)" ||
    fail "completion is missing input_events_expected"
flushed="$(field "$completion" input_events_flushed)" ||
    fail "completion is missing input_events_flushed"
[[ "$expected" == "$flushed" ]] ||
    fail "input queue did not drain: expected=$expected flushed=$flushed"

require_line '^sophia_session_input_guard schema=2 status=ready .* keyboards=[1-9][0-9]*$' \
    "$GUARD_LOG" "input guard did not discover a keyboard"
require_line '^sophia_session_input_guard schema=1 status=armed$' \
    "$GUARD_LOG" "emergency input guard was not armed"
if grep -Eq '^sophia_session_input_guard schema=1 status=triggered$' "$GUARD_LOG"; then
    fail "run used emergency recovery instead of normal logout"
fi

recovery="$(
    grep -E '^sophia_tty_recovery schema=3 profile=xmonad ' "$RECOVERY_LOG" | tail -n 1
)"
[[ -n "$recovery" ]] || fail "no xmonad TTY recovery record"
require_eq "$recovery" termios_restored true
require_eq "$recovery" emergency false
require_eq "$recovery" session_shutdown not_requested
require_eq "$recovery" session_exit_status none
kd_before="$(field "$recovery" kd_mode_before)" || fail "recovery is missing kd_mode_before"
kd_after="$(field "$recovery" kd_mode_after)" || fail "recovery is missing kd_mode_after"
[[ "$kd_before" == "$kd_after" ]] ||
    fail "KD mode was not restored: before=$kd_before after=$kd_after"

echo "xmonad TTY3 physical session verified: $SESSION_LOG"
