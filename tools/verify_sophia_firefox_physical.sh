#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
LOG_DIR="${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"
GUARD_LOG="${2:-$LOG_DIR/input-guard.log}"
RECOVERY_LOG="${3:-$LOG_DIR/recovery.log}"

fail() {
    echo "physical Firefox M10 verification failed: $*" >&2
    exit 1
}
require_file() {
    [[ -s "$1" ]] || fail "missing or empty evidence file: $1"
}
count() {
    grep -Ec "$1" "$SESSION_LOG" || true
}
line_number() {
    local pattern="$1" ordinal="${2:-1}"
    grep -nE "$pattern" "$SESSION_LOG" 2>/dev/null |
        sed -n "${ordinal}p" |
        cut -d: -f1 || true
}
line_number_after() {
    local pattern="$1" minimum="$2"
    grep -nE "$pattern" "$SESSION_LOG" 2>/dev/null |
        cut -d: -f1 |
        awk -v minimum="$minimum" '$1 > minimum { print; exit }' || true
}
require_line_number() {
    local pattern="$1" description="$2" observed
    observed="$(line_number "$pattern")"
    [[ -n "$observed" ]] || fail "$description"
    printf '%s\n' "$observed"
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
    actual="$(field "$line" "$key")" || fail "record is missing $key"
    [[ "$actual" == "$expected" ]] || fail "$key is $actual, expected $expected"
}
require_at_least() {
    local line="$1" key="$2" minimum="$3" actual
    actual="$(field "$line" "$key")" || fail "record is missing $key"
    [[ "$actual" =~ ^[0-9]+$ ]] || fail "$key is not an integer: $actual"
    (( actual >= minimum )) || fail "$key is $actual, expected at least $minimum"
}
require_selection_interval() {
    local first="$1" last="$2" description="$3" owner conversion
    owner="$(line_number_after '^sophia_firefox_m8 schema=1 status=selection_observed kind=owner_change ' "$first")"
    [[ -n "$owner" ]] || fail "$description lacks a selection owner change"
    conversion="$(line_number_after '^sophia_firefox_m8 schema=1 status=selection_observed kind=conversion ' "$owner")"
    [[ -n "$conversion" ]] || fail "$description lacks a selection conversion"
    (( owner < conversion && conversion < last )) ||
        fail "$description lacks an ordered owner-to-requestor transfer"
}

require_file "$SESSION_LOG"
require_file "$GUARD_LOG"
require_file "$RECOVERY_LOG"

if grep -Eqi '(^Error:|panicked at|^sophia_[^[:space:]]+ .*status=(failed|degraded)([[:space:]]|$))' \
    "$SESSION_LOG"; then
    # Surface the specific proof/stage failure before the generic verdict so a
    # run self-reports (e.g. "died on resize") without a manual log read.
    grep -E '(Firefox M[0-9]+( Kitty)? proof incomplete: [^"]*|status=failed reason=[a-z_]+( stage=[a-z]+)?)' \
        "$SESSION_LOG" | sed 's/^/  cause: /' >&2 || true
    fail "session log contains a Sophia error, panic, or degraded status"
fi
if grep -Eqi '(Gdk-CRITICAL|gdk_window_thaw_toplevel_updates)' "$SESSION_LOG"; then
    fail "Firefox reported an unbalanced GDK toplevel update freeze"
fi
if grep -Eq '^sophia_live_session_pointer schema=5 status=focus_handoff_dropped reason=' \
    "$SESSION_LOG"; then
    fail "session dropped a pointer focus handoff"
fi
grep -Eq '^sophia_live_wm schema=1 status=ready adapter=external socket=private restarts=0$' \
    "$SESSION_LOG" || fail "external xmonad policy never became ready"
grep -Eq '^sophia_live_outputs schema=2 status=ready discovered=2 presentation=2 native_owned=2 multi_output_scanout=enabled ' \
    "$SESSION_LOG" || fail "two-output native ownership was not established"
for output in 1 2; do
    grep -Eq "^sophia_live_native_page_flip schema=1 status=retired output=${output} " \
        "$SESSION_LOG" || fail "output $output never retired a page flip"
done

[[ "$(count '^sophia_session_app schema=1 status=started id=terminal source=(startup|action)$')" == 2 ]] ||
    fail "the run must contain exactly two Kitty processes"
[[ "$(count '^sophia_session_app schema=1 status=started id=firefox source=action$')" == 2 ]] ||
    fail "Firefox was not action-launched exactly twice"
[[ "$(count '^sophia_session_app schema=1 status=exited id=firefox source=managed exit_status=exit status: 0$')" == 2 ]] ||
    fail "Firefox did not exit successfully exactly twice"

kitty_before_a="$(require_line_number '^sophia_firefox_m10 schema=1 status=kitty_checkpoint terminal=a checkpoint=before content=redacted$' 'Kitty A pre-Firefox checkpoint is missing')"
kitty_before_b="$(require_line_number '^sophia_firefox_m10 schema=1 status=kitty_checkpoint terminal=b checkpoint=before content=redacted$' 'Kitty B pre-Firefox checkpoint is missing')"
kitty_clipboard="$(require_line_number '^sophia_firefox_m10 schema=1 status=kitty_checkpoint terminal=b checkpoint=clipboard_peer content=redacted$' 'Kitty B did not receive Firefox CLIPBOARD content')"
kitty_primary="$(require_line_number '^sophia_firefox_m10 schema=1 status=kitty_checkpoint terminal=b checkpoint=primary_peer content=redacted$' 'Kitty B did not receive Firefox PRIMARY content')"
kitty_normal_a="$(require_line_number '^sophia_firefox_m10 schema=1 status=kitty_checkpoint terminal=a checkpoint=after_normal_close content=redacted$' 'Kitty A normal-close checkpoint is missing')"
kitty_normal_b="$(require_line_number '^sophia_firefox_m10 schema=1 status=kitty_checkpoint terminal=b checkpoint=after_normal_close content=redacted$' 'Kitty B normal-close checkpoint is missing')"
kitty_forced_a="$(require_line_number '^sophia_firefox_m10 schema=1 status=kitty_checkpoint terminal=a checkpoint=after_forced_close content=redacted$' 'Kitty A forced-close checkpoint is missing')"
kitty_forced_b="$(require_line_number '^sophia_firefox_m10 schema=1 status=kitty_checkpoint terminal=b checkpoint=after_forced_close content=redacted$' 'Kitty B forced-close checkpoint is missing')"
first_start="$(line_number '^sophia_session_app schema=1 status=started id=firefox source=action$' 1)"
second_start="$(line_number '^sophia_session_app schema=1 status=started id=firefox source=action$' 2)"
first_exit="$(line_number '^sophia_session_app schema=1 status=exited id=firefox source=managed exit_status=exit status: 0$' 1)"
second_exit="$(line_number '^sophia_session_app schema=1 status=exited id=firefox source=managed exit_status=exit status: 0$' 2)"
[[ -n "$first_start" && -n "$first_exit" && -n "$second_start" && -n "$second_exit" ]] ||
    fail "Firefox launch/exit ordering evidence is incomplete"
(( kitty_before_a < first_start && kitty_before_b < first_start )) ||
    fail "both Kitty windows were not interactive before Firefox"
(( first_start < first_exit
    && first_start < kitty_clipboard && kitty_clipboard < kitty_primary
    && kitty_primary < first_exit
    && first_exit < kitty_normal_a && first_exit < kitty_normal_b
    && kitty_normal_a < second_start && kitty_normal_b < second_start
    && second_start < second_exit
    && second_exit < kitty_forced_a && second_exit < kitty_forced_b )) ||
    fail "Firefox close/restart and Kitty retention checkpoints are out of order"

mapfile -t firefox_admission_starts < <(
    grep -E '^sophia_session_app schema=2 status=started id=firefox source=action transaction=[0-9]+$' \
        "$SESSION_LOG"
)
(( ${#firefox_admission_starts[@]} == 2 )) ||
    fail "expected exactly two correlated Firefox admission starts"
firefox_exit_lines=("$first_exit" "$second_exit")
admission_restart_count=0
for index in 0 1; do
    start_record="${firefox_admission_starts[$index]}"
    action_transaction="$(field "$start_record" transaction)" ||
        fail "Firefox admission start lacks its action transaction"
    admission_start_line="$(line_number "^sophia_session_app schema=2 status=started id=firefox source=action transaction=${action_transaction}$")"
    surface_observed_line="$(line_number_after "^sophia_session_app schema=2 status=surface_observed source=action transaction=${action_transaction} surface=[0-9]+$" "$admission_start_line")"
    [[ -n "$surface_observed_line" ]] ||
        fail "Firefox action $action_transaction never observed a surface"
    surface_record="$(sed -n "${surface_observed_line}p" "$SESSION_LOG")"
    firefox_admission_surface="$(field "$surface_record" surface)" ||
        fail "Firefox surface observation lacks its opaque surface"
    admitted_line="$(line_number_after "^sophia_session_app schema=2 status=admitted source=action transaction=${action_transaction} surface=${firefox_admission_surface}$" "$surface_observed_line")"
    [[ -n "$admitted_line" ]] ||
        fail "Firefox action $action_transaction never completed visual admission"
    (( admitted_line < firefox_exit_lines[index] )) ||
        fail "Firefox action $action_transaction exited before admission completed"
    visual_presented_line="$(line_number_after "^sophia_live_visual_admission schema=1 status=presented transaction=[0-9]+ surface=${firefox_admission_surface}$" "$surface_observed_line")"
    [[ -n "$visual_presented_line" ]] && (( visual_presented_line < admitted_line )) ||
        fail "Firefox action $action_transaction lacks retired admission pixels"

    mapfile -t admission_restarts < <(awk -v first="$surface_observed_line" -v last="$admitted_line" '
        NR > first && NR < last && /^sophia_live_wm schema=1 status=restarted / { print NR }
    ' "$SESSION_LOG")
    (( ${#admission_restarts[@]} <= 1 )) ||
        fail "Firefox action $action_transaction restarted the WM more than once"
    admission_restart_count=$((admission_restart_count + ${#admission_restarts[@]}))
    if (( ${#admission_restarts[@]} == 1 )); then
        restart_line="${admission_restarts[0]}"
        committed_phase_line="$(line_number_after '^sophia_live_wm schema=4 status=reseed_queued phase=committed_layout request=relayout$' "$restart_line")"
        manage_phase_line="$(line_number_after "^sophia_live_wm schema=4 status=reseed_queued phase=pending_admission request=manage surface=${firefox_admission_surface}$" "$committed_phase_line")"
        [[ -n "$committed_phase_line" && -n "$manage_phase_line" ]] \
            && (( committed_phase_line < manage_phase_line && manage_phase_line < admitted_line )) ||
            fail "Firefox recovery did not queue committed layout before manage replay"
        seed_commit_line="$(line_number_after '^sophia_live_wm schema=1 status=layout_committed .* outcome=Committed$' "$manage_phase_line")"
        [[ -n "$seed_commit_line" ]] && (( seed_commit_line < admitted_line )) ||
            fail "Firefox recovery lacks a committed-layout reseed"
        if awk -v first="$restart_line" -v last="$seed_commit_line" -v surface="$firefox_admission_surface" '
            NR > first && NR < last &&
                $0 ~ /^sophia_live_visual_admission schema=1 status=armed / &&
                $0 ~ ("surface=" surface "$") { found=1 }
            END { exit !found }
        ' "$SESSION_LOG"; then
            fail "committed-layout reseed consumed Firefox admission pixels"
        fi
        replay_arm_line="$(line_number_after "^sophia_live_visual_admission schema=1 status=armed transaction=[0-9]+ surface=${firefox_admission_surface}$" "$seed_commit_line")"
        [[ -n "$replay_arm_line" ]] && (( replay_arm_line < visual_presented_line )) ||
            fail "Firefox manage replay did not arm its retained visual candidate"
    fi
done

forced_close="$(line_number_after '^sophia_live_wm schema=1 status=session_action_committed .* action=CloseFocused$' "$second_start")"
[[ -n "$forced_close" ]] && (( forced_close < second_exit )) ||
    fail "second Firefox was not closed through the WM close path"
if awk -v first="$first_start" -v last="$first_exit" \
    'NR > first && NR < last && /status=session_action_committed .* action=CloseFocused$/ { found=1 } END { exit !found }' \
    "$SESSION_LOG"; then
    fail "first Firefox used the WM close path instead of Ctrl+Q"
fi

grep -Eq '^sophia_firefox_m8 schema=1 status=page_ready .* content=redacted$' \
    "$SESSION_LOG" || fail "offline Firefox page never became ready"
for stage in loaded keyboard clipboard primary scroll resize refocus dialog; do
    [[ "$(count "^sophia_firefox_m8 schema=1 status=stage_complete stage=$stage ")" == 1 ]] ||
        fail "Firefox stage did not complete exactly once: $stage"
done
keyboard_line="$(line_number '^sophia_firefox_m8 schema=1 status=stage_complete stage=keyboard ')"
clipboard_line="$(line_number '^sophia_firefox_m8 schema=1 status=stage_complete stage=clipboard ')"
primary_line="$(line_number '^sophia_firefox_m8 schema=1 status=stage_complete stage=primary ')"
require_selection_interval "$keyboard_line" "$kitty_clipboard" 'Firefox-to-Kitty CLIPBOARD handoff'
require_selection_interval "$kitty_clipboard" "$clipboard_line" 'Kitty-to-Firefox CLIPBOARD handoff'
require_selection_interval "$clipboard_line" "$kitty_primary" 'Firefox-to-Kitty PRIMARY handoff'
require_selection_interval "$kitty_primary" "$primary_line" 'Kitty-to-Firefox PRIMARY handoff'
navigation_ready_line="$(require_line_number '^sophia_firefox_m8 schema=1 status=navigation_ready content=redacted$' 'replacement Firefox document never became ready')"
scroll_line="$(line_number '^sophia_firefox_m8 schema=1 status=stage_complete stage=scroll ')"
(( primary_line < navigation_ready_line && navigation_ready_line < scroll_line )) ||
    fail "Firefox navigation readiness is not ordered between PRIMARY and scroll completion"
axis_routes="$(awk -v first="$navigation_ready_line" -v last="$scroll_line" '
    NR > first && NR < last && /^sophia_live_session_pointer schema=9 status=axis_batch / {
        for (i = 1; i <= NF; i++) {
            if ($i ~ /^routed=[0-9]+$/) {
                split($i, field, "=")
                count += field[2]
            }
        }
    }
    END { print count + 0 }
' "$SESSION_LOG")"
(( axis_routes >= 2 )) ||
    fail "Firefox's DOM scroll stage followed $axis_routes routed wheel packets; expected at least two for the XI2 baseline and delta"
resize_line="$(line_number '^sophia_firefox_m8 schema=1 status=stage_complete stage=resize ')"
resize_epoch="$(line_number_after '^sophia_live_resize_epoch schema=1 status=committed .* matched_surfaces=3$' "$scroll_line")"
resize_layout="$(line_number_after '^sophia_live_wm schema=1 status=layout_committed .* surfaces=4 .* outcome=Committed$' "$scroll_line")"
resize_action="$(line_number_after '^sophia_live_wm schema=1 status=physical_action_committed action=3$' "$scroll_line")"
resize_projection="$(line_number_after '^sophia_live_wm schema=2 status=workspace_projection_committed .* visible_surfaces=3 focus=surface$' "$scroll_line")"
visual_armed_line="$(line_number_after '^sophia_live_resize_epoch schema=3 status=visual_armed ' "$scroll_line")"
visual_committed_line="$(line_number_after '^sophia_live_resize_epoch schema=3 status=visual_committed ' "$scroll_line")"
[[ -n "$resize_epoch" && -n "$resize_layout" && -n "$resize_action" ]] \
    && (( resize_epoch < resize_action && resize_layout < resize_action )) ||
    fail "the Firefox resize stage lacks a committed three-surface layout epoch"
[[ -n "$visual_armed_line" && -n "$visual_committed_line" ]] \
    && (( visual_armed_line < visual_committed_line && visual_committed_line < resize_line )) ||
    fail "the Firefox resize target was not visually committed by exact Present retirement"
visual_armed="$(sed -n "${visual_armed_line}p" "$SESSION_LOG")"
visual_committed="$(sed -n "${visual_committed_line}p" "$SESSION_LOG")"
for assignment in \
    "transaction=$(field "$visual_armed" transaction)" \
    "surface=$(field "$visual_armed" surface)" \
    "width=$(field "$visual_armed" width)" \
    "height=$(field "$visual_armed" height)"; do
    require_eq "$visual_committed" "${assignment%%=*}" "${assignment#*=}"
done
require_at_least "$visual_committed" width 1
require_at_least "$visual_committed" height 1
if awk -v first="$visual_armed_line" -v last="$visual_committed_line" \
    'NR >= first && NR <= last && /outcome=RejectedStaleSurface/ { found=1 } END { exit !found }' \
    "$SESSION_LOG"; then
    fail "the Firefox resize Present became stale before visual retirement"
fi
[[ -n "$resize_action" ]] && (( resize_action < resize_line )) ||
    fail "the Firefox resize stage lacks an ordered Super+Space layout action"
[[ -n "$resize_projection" ]] && (( resize_action < resize_projection && resize_projection < resize_line )) ||
    fail "the Firefox resize stage did not retain all three managed surfaces"
m8_completion="$(grep -E '^sophia_firefox_m8 schema=1 status=complete stages=8 selection_owner_changes=[0-9]+ selection_conversions=[0-9]+ content=redacted$' "$SESSION_LOG" | tail -n 1)"
[[ -n "$m8_completion" ]] || fail "Firefox eight-stage proof did not complete"
require_at_least "$m8_completion" selection_owner_changes 4
require_at_least "$m8_completion" selection_conversions 4
refocus_line="$(line_number '^sophia_firefox_m8 schema=1 status=stage_complete stage=refocus ')"
firefox_surface="$(field "$visual_armed" surface)" || fail "resize evidence has no Firefox surface"
focus_away_action="$(line_number_after '^sophia_live_wm schema=1 status=physical_action_committed action=1$' "$resize_line")"
focus_away_line="$(awk -v first="$focus_away_action" -v last="$refocus_line" -v firefox="$firefox_surface" '
    NR > first && NR < last && /^sophia_live_wm schema=1 status=focus_reconciled / && $0 !~ ("index: " firefox ",") { print NR; exit }
' "$SESSION_LOG")"
[[ -n "$focus_away_line" ]] || fail "Firefox refocus stage lacks a committed focus transition away"
focus_away_applied="$(line_number_after '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$' "$focus_away_line")"
focus_return_action="$(line_number_after '^sophia_live_wm schema=1 status=physical_action_committed action=1$' "$focus_away_applied")"
focus_return_line="$(line_number_after "^sophia_live_wm schema=1 status=focus_reconciled .* surface=SurfaceId \\{ index: ${firefox_surface}," "$focus_return_action")"
focus_return_applied="$(line_number_after '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$' "$focus_return_line")"
xi_focus_out="$(line_number_after "^sophia_x11_focus_delivery schema=1 .* window=${firefox_surface} focused=false .* xi2_selected=true content=redacted$" "$resize_line")"
xi_focus_in="$(line_number_after "^sophia_x11_focus_delivery schema=1 .* window=${firefox_surface} focused=true .* xi2_selected=true content=redacted$" "$xi_focus_out")"
[[ -n "$focus_away_action" && -n "$focus_away_applied" && -n "$focus_return_action"
    && -n "$focus_return_line" && -n "$focus_return_applied" ]] ||
    fail "Firefox refocus action/control ordering evidence is incomplete"
[[ -n "$xi_focus_out" && -n "$xi_focus_in" ]] ||
    fail "Firefox did not receive selected XI2 FocusOut/FocusIn"
(( resize_line < focus_away_action
    && focus_away_action < focus_away_line
    && focus_away_line < focus_away_applied
    && focus_away_applied < focus_return_action
    && focus_return_action < focus_return_line
    && focus_return_line < focus_return_applied
    && focus_return_applied < refocus_line
    && resize_line < xi_focus_out
    && xi_focus_out < xi_focus_in
    && xi_focus_in < refocus_line )) ||
    fail "Firefox DOM refocus is not ordered after focus-away, XI2 out/in, and focus return"
dialog_ready_line="$(require_line_number '^sophia_firefox_m8 schema=1 status=dialog_ready content=redacted$' 'Firefox modal never became ready')"
dialog_line="$(line_number '^sophia_firefox_m8 schema=1 status=stage_complete stage=dialog ')"
if awk -v first="$refocus_line" -v last="$dialog_line" '
    NR > first && NR < last &&
        ($0 ~ /^sophia_live_wm .*status=layout_timeout / || $0 ~ /^sophia_live_wm .*status=restarted /) { found=1 }
    END { exit !found }
' "$SESSION_LOG"; then
    fail "Firefox modal interaction timed out or restarted the WM bridge"
fi
(( refocus_line < dialog_ready_line
    && dialog_ready_line < dialog_line
    && dialog_line < first_exit )) ||
    fail "Firefox modal ready/confirm lifecycle is out of order"
m10_completion="$(grep -E '^sophia_firefox_m10 schema=2 status=complete kitty_checkpoints=[0-9]+ selection_owner_changes=[0-9]+ selection_conversions=[0-9]+ content=redacted$' "$SESSION_LOG" | tail -n 1)"
[[ -n "$m10_completion" ]] || fail "Firefox M10 Kitty proof did not complete"
require_eq "$m10_completion" kitty_checkpoints 8
require_at_least "$m10_completion" selection_owner_changes 4
require_at_least "$m10_completion" selection_conversions 4

grep -Eq '^sophia_live_wm schema=1 status=session_action_committed .* action=Logout$' \
    "$SESSION_LOG" || fail "normal logout was not committed"
grep -Eq '^sophia_live_session_protocol_errors schema=1 expected=[0-9]+ unexpected=0$' \
    "$SESSION_LOG" || fail "unexpected X protocol errors were observed"
selection="$(grep -E '^sophia_live_selection schema=1 status=complete ' "$SESSION_LOG" | tail -n 1)"
[[ -n "$selection" ]] || fail "selection summary is missing"
require_at_least "$selection" owner_changes 4
require_at_least "$selection" conversions 4
health="$(grep -E '^sophia_live_session_health schema=1 status=clean ' "$SESSION_LOG" | tail -n 1)"
[[ -n "$health" ]] || fail "clean session health summary is missing"
for assignment in protocol_errors=0 pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false; do
    require_eq "$health" "${assignment%%=*}" "${assignment#*=}"
done
layout_health="$(grep -E '^sophia_live_layout_health schema=1 status=clean ' "$SESSION_LOG" | tail -n 1 || true)"
[[ -n "$layout_health" ]] || fail "clean layout health summary is missing"
for assignment in recovery_extents=0 constraint_relayout_pending=false; do
    require_eq "$layout_health" "${assignment%%=*}" "${assignment#*=}"
done
keys="$(grep -E '^sophia_live_session_keys schema=2 status=complete ' "$SESSION_LOG" | tail -n 1)"
[[ -n "$keys" ]] || fail "final key-state summary is missing"
for assignment in pending=0 release_barrier_pending=0 repeat_active_seats=0 repeat_capacity_exhausted=0; do
    require_eq "$keys" "${assignment%%=*}" "${assignment#*=}"
done

mapfile -t completions < <(grep -E '^sophia_live_session schema=(14|15|16) status=bounded_complete ' "$SESSION_LOG")
(( ${#completions[@]} == 1 )) || fail "expected exactly one bounded session completion"
completion="${completions[0]}"
for assignment in \
    native_presentation=enabled physical_input=enabled wm_policy=external \
    wm_degraded=false native_submit_failures=0 \
    native_retire_failures=0 native_callback_rejected=0 \
    native_callback_queue_saturated=0 native_in_flight=false \
    native_cleanup_pending=false present_disconnect_failures=0 \
    present_live_sources=0 present_live_fences=0 present_live_transactions=0; do
    require_eq "$completion" "${assignment%%=*}" "${assignment#*=}"
done
completion_restarts="$(field "$completion" wm_restarts)" || fail "completion lacks wm_restarts"
[[ "$completion_restarts" =~ ^[0-9]+$ ]] ||
    fail "wm_restarts is not an integer: $completion_restarts"
(( completion_restarts == admission_restart_count )) ||
    fail "completion reports $completion_restarts WM restarts, observed $admission_restart_count during Firefox admission"
expected="$(field "$completion" input_events_expected)" || fail "completion lacks input_events_expected"
flushed="$(field "$completion" input_events_flushed)" || fail "completion lacks input_events_flushed"
[[ "$expected" == "$flushed" ]] || fail "input queue did not drain: expected=$expected flushed=$flushed"

grep -Eq '^sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 namespace=revoked xauthority=removed$' \
    "$SESSION_LOG" || fail "application/frontend/authority cleanup was not clean"
grep -Eq '^sophia_session_input_guard schema=2 status=ready .* keyboards=[1-9][0-9]*$' \
    "$GUARD_LOG" || fail "input guard did not discover a keyboard"
grep -Eq '^sophia_session_input_guard schema=1 status=armed$' \
    "$GUARD_LOG" || fail "input guard was not armed"
if grep -Eq '^sophia_session_input_guard schema=1 status=triggered$' "$GUARD_LOG"; then
    fail "run used emergency recovery instead of normal logout"
fi
recovery="$(grep -E '^sophia_tty_recovery schema=3 profile=xmonad ' "$RECOVERY_LOG" | tail -n 1)"
[[ -n "$recovery" ]] || fail "xmonad TTY recovery record is missing"
for assignment in termios_restored=true emergency=false session_shutdown=not_requested session_exit_status=none; do
    require_eq "$recovery" "${assignment%%=*}" "${assignment#*=}"
done
kd_before="$(field "$recovery" kd_mode_before)" || fail "recovery lacks kd_mode_before"
kd_after="$(field "$recovery" kd_mode_after)" || fail "recovery lacks kd_mode_after"
[[ "$kd_before" == "$kd_after" ]] || fail "KD mode was not restored"

echo "physical Firefox M10 workflow verified: $SESSION_LOG"
