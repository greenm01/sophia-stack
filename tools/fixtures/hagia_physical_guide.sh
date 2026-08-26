#!/bin/sh
set -eu

evidence="${SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE:-/tmp/sophia-hagia-policy-physical.log}"
proof_text="${SOPHIA_HAGIA_PHYSICAL_TEXT:-hagiapolicyproof}"
proof_result="${SOPHIA_INPUT_PROOF_RESULT:-}"

case "$proof_text" in
    *[!a-z]*|'')
        echo "invalid Hagia physical proof text" >&2
        exit 2
        ;;
esac
if [ -z "$proof_result" ]; then
    echo "Sophia did not provide the physical proof result path" >&2
    exit 2
fi

action_pattern() {
    printf '^sophia_live_wm schema=1 status=physical_action_committed action=%s$' "$1"
}

action_count() {
    grep -Ec "$(action_pattern "$1")" "$evidence" 2>/dev/null || true
}

# These counts are cumulative across the whole guide, and several actions are
# asked for more than once, so a count above the step's expectation is not
# evidence of an extra press: it is what a later legitimate press looks like from
# an earlier step. Catching an operator's extra keypress needs the run's final
# totals, not a threshold here, and `verify_hagia_policy_physical.sh` checks them
# at the end of the run. It reads the expectations out of the calls below, so a
# step added here is a step it starts checking.
wait_for_action_count() {
    action="$1"
    expected="$2"
    while [ "$(action_count "$action")" -lt "$expected" ]; do
        sleep 0.1
    done
}

# Waits that depend on the session rather than on the operator get a bound, so a
# session that will never satisfy them fails legibly instead of hanging.
wait_for_shell_line_bounded() {
    pattern="$1"
    attempts=1200
    while ! grep -Eq "$pattern" "$evidence" 2>/dev/null; do
        attempts=$((attempts - 1))
        if [ "$attempts" -le 0 ]; then
            printf '\033[2J\033[H'
            echo 'Physical proof aborted: the session never produced' >&2
            echo "  $pattern" >&2
            echo 'If the browser opened but never joined the layout, the active view is' >&2
            echo 'placing a single window. Press Super+N to change layout, or focus the' >&2
            echo 'browser, and restart the proof run.' >&2
            exit 2
        fi
        sleep 0.1
    done
}

wait_for_restart() {
    pattern='^sophia_live_wm schema=4 status=restarted adapter=sophia_wm_v1 epoch=2 restarts=1 preserved_layout=true$'
    while ! grep -Eq "$pattern" "$evidence" 2>/dev/null; do
        sleep 0.1
    done
}

wait_for_nonempty_restore() {
    while ! awk '
        /^sophia_live_wm schema=1 status=physical_action_committed action=40$/ {
            restore = 1
        }
        restore && (/^hagia_policy_checkpoint schema=1 status=saved candidate_nonempty=true$/ || / event=checkpoint status=saved detail="candidate_nonempty=true"$/) {
            restored = 1
        }
        END { exit restored ? 0 : 1 }
    ' "$evidence" 2>/dev/null; do
        sleep 0.1
    done
}

wait_for_secondary_nonzero_submission() {
    while ! awk '
        /^sophia_live_wm schema=1 status=physical_action_committed action=5$/ {
            moved = 1
        }
        moved && /^.*sophia_live_native_head_page_flip schema=2 status=submitted output=2 .*nonzero_rgb_pixels: [1-9][0-9]*.*$/ {
            presented = 1
        }
        END { exit presented ? 0 : 1 }
    ' "$evidence" 2>/dev/null; do
        sleep 0.1
    done
}

shell_count() {
    pattern="$1"
    grep -Ec "$pattern" "$evidence" 2>/dev/null || true
}

wait_for_shell_count() {
    pattern="$1"
    expected="$2"
    while [ "$(shell_count "$pattern")" -lt "$expected" ]; do
        sleep 0.1
    done
}

wait_for_shell_line() {
    pattern="$1"
    while ! grep -Eq "$pattern" "$evidence" 2>/dev/null; do
        sleep 0.1
    done
}

show_step() {
    printf '\033[2J\033[H'
    printf '%s\n\n%s\n\n%s\n' \
        'HAGIA RESTART PROOF' "$1" 'This screen advances only after Sophia commits the action.'
}

show_step 'Press Super+Shift+F once. Confirm the window remains fullscreen.'
wait_for_action_count 37 1

show_step 'Press Super+N once to select the next layout. Then wait for the scene to return.'
wait_for_action_count 66 1
wait_for_restart

show_step 'Restart committed. Confirm fullscreen survived, then press Super+Shift+F once.'
wait_for_action_count 37 2

show_step 'Press Super+N once to select the next layout.'
wait_for_action_count 66 2

show_step 'IMPORTANT — READ BOTH LINES BEFORE CLICKING
1. Confirm the numbered indicator strip remains visible above this fullscreen window.
2. Click 2 in the strip, then click 1 to return here.'
wait_for_action_count 12 1
wait_for_action_count 11 1

show_step 'Press Super+M once.'
wait_for_action_count 38 1

show_step 'Press Super+M once more.'
wait_for_action_count 38 2

show_step 'IMPORTANT — READ ALL THREE LINES BEFORE ACTING
1. Press and release Super+Shift+B.
2. This window will disappear.
3. Press and release Super+Alt+B anyway. Do not wait for another prompt.'
wait_for_action_count 39 1
wait_for_action_count 40 1
wait_for_nonempty_restore

show_step 'Press Super+Shift+Right once. Confirm the window moves to the other output.'
wait_for_action_count 5 1
wait_for_secondary_nonzero_submission

show_step 'The other output presented the window. Press Super+Shift+Left once to move it back.'
wait_for_action_count 6 1

show_step 'Press Super+Left once.'
wait_for_action_count 33 1

show_step 'Press Super+Right once.'
wait_for_action_count 34 1

show_step 'Press Super+B once to launch the browser. Wait while Sophia publishes its second switcher row.'
# The launch commits a physical action as well as a session one, and the totals
# check reads its expectations from these lines alone. Waiting on the session
# record only would leave action 30 committed by every run and asked for by
# none, which fails a correct run.
wait_for_action_count 30 1
wait_for_shell_line '^sophia_live_wm schema=1 status=session_action_committed transaction=[1-9][0-9]* action=LaunchBrowser$'
wait_for_shell_line '^sophia_session_app schema=2 status=admitted source=action transaction=[1-9][0-9]* surface=[1-9][0-9]*$'
wait_for_shell_line_bounded '^sophia_live_wm schema=1 status=layout_committed transaction=[1-9][0-9]* surfaces=2 moved_surfaces=[0-9]+ configure_deliveries=[0-9]+ outcome=Committed$'
wait_for_shell_count '^sophia_live_metadata_broker schema=1 status=descriptor_committed surface=[0-9]+ content=redacted$' 2

show_step 'Press Super+P once. The switcher claims a strip of work area while it
is visible: confirm the windows behind it move clear of that strip rather than
staying underneath. Then click its first row to focus this terminal.'
wait_for_shell_count '^sophia_live_metadata_shell schema=1 status=presented .* visible=true$' 1
# The claim reaches the reduction only after its bundle presents; a band that
# reduced at admission would look identical on screen and be wrong in exactly
# the window the coordination model forbids.
wait_for_shell_count '^sophia_live_metadata_shell schema=1 status=reservation_presented .*$' 1
wait_for_shell_count '^sophia_live_metadata_shell schema=1 status=reservation_reduced bands=1$' 1
wait_for_shell_count '^sophia_live_metadata_shell schema=1 status=activation_admitted .* target=redacted$' 1
wait_for_shell_count '^sophia_live_metadata_shell schema=1 status=presented .* visible=false$' 1
# Withdrawal is a candidate that reserves nothing, committed through the same
# path, so the work area returns without a release message of its own.
wait_for_shell_count '^sophia_live_metadata_shell schema=1 status=reservation_reduced bands=0$' 1

show_step 'Press Super+P again. Sophia will restart Hagia Shell after the switcher is visibly presented.'
wait_for_shell_count '^sophia_live_metadata_shell schema=1 status=reservation_presented .*$' 2
wait_for_shell_count '^sophia_live_metadata_shell schema=1 status=reservation_reduced bands=1$' 2
wait_for_shell_line '^sophia_live_metadata_shell schema=1 status=proof_restart_triggered visible_presentation=2 retained_pixels=true$'
wait_for_shell_line '^sophia_live_metadata_shell schema=1 status=reconnected protected=true peer_pid=[1-9][0-9]* revision=1 connection_epoch=2 reason=proof_visible_restart$'

show_step 'The old switcher pixels are retained but inert, and so is its claim:
the work area stays reduced while no shell is alive to reproject it. Confirm the
windows have NOT expanded into the strip, then click the first row once.'
# No reservation line is expected here, and that is the assertion: losing the
# connection must not move the work area. Growing it while nothing can present
# into the strip is the half-new desktop the model rules out.
wait_for_shell_line '^sophia_live_metadata_shell schema=1 status=proof_inert_click observed=true activation=false$'

show_step 'Press Super+P once more. Click the first row when the fresh switcher appears.'
# The fresh epoch re-claims through a new candidate rather than resurrecting the
# old one. Its band is identical, so nothing reprojects and no reduced line
# follows -- an unchanged claim must not relayout the desktop.
wait_for_shell_count '^sophia_live_metadata_shell schema=1 status=reservation_presented .*$' 3
wait_for_shell_count '^sophia_live_metadata_shell schema=1 status=presented .* visible=true$' 3
wait_for_shell_count '^sophia_live_metadata_shell schema=1 status=activation_(admitted|duplicate) .* target=redacted$' 2
wait_for_shell_count '^sophia_live_metadata_shell schema=1 status=presented .* visible=false$' 2

printf '\033[2J\033[H'
printf '%s\n\n%s\n\n' \
    'ALL HAGIA ACTIONS COMMITTED' \
    "Type $proof_text and press Enter. This immediately ends the session."

while IFS= read -r line; do
    if [ "$line" = "$proof_text" ]; then
        break
    fi
    printf '%s\n' "Type the exact final phrase: $proof_text"
done

umask 077
printf '%s' "$proof_text" >"$proof_result"
printf '%s\n' 'Proof phrase accepted. Waiting for Sophia to complete the session.'
while :; do
    sleep 1
done
