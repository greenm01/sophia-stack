#!/bin/sh
set -eu

# The native-session guide. It drives the bounded product workflow that
# `todo.md` puts on the critical path -- three terminal launches, a visible
# focus-next, one close, and a normal logout -- through Sophia's own WM and
# shell protocols.
#
# It runs inside the startup Kitty and advances only on committed telemetry, so
# every screen the operator sees corresponds to a fact Sophia recorded rather
# than to elapsed time.

evidence="${SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE:-}"
proof_text="${SOPHIA_HAGIA_NATIVE_TEXT:-hagianativeproof}"
proof_result="${SOPHIA_INPUT_PROOF_RESULT:-}"
guide_claim="${SOPHIA_HAGIA_NATIVE_GUIDE_CLAIM:-}"

# Only the startup terminal is the guide. One application id carries one
# argument list, and the terminal the launch action spawns must be that same
# application -- a normal session refuses a physical text proof whose terminal
# action names anything but its single startup application. So every terminal
# this workflow launches also runs this script, and every instance after the
# first stands down into an ordinary shell. Without this, each new window ran a
# second guide, found its waits already satisfied, exited, and took the window
# with it, because a terminal exits when its command does.
if [ -n "$guide_claim" ] && ! (set -C; : >"$guide_claim") 2>/dev/null; then
    exec "${SHELL:-/bin/sh}"
fi

case "$proof_text" in
    *[!a-z]*|'')
        echo "invalid Hagia native proof text" >&2
        exit 2
        ;;
esac
if [ -z "$proof_result" ]; then
    echo "Sophia did not provide the physical proof result path" >&2
    exit 2
fi
if [ -z "$evidence" ]; then
    echo "Sophia did not provide the session evidence path" >&2
    exit 2
fi

abort() {
    printf '\033[2J\033[H'
    echo 'Native proof aborted: the session never produced' >&2
    echo "  $1" >&2
    echo 'Log out with Ctrl+Alt+Delete and inspect the session log.' >&2
    exit 2
}

action_pattern() {
    printf '^sophia_live_wm schema=1 status=physical_action_committed action=%s$' "$1"
}

action_count() {
    grep -Ec "$(action_pattern "$1")" "$evidence" 2>/dev/null || true
}

# Counts are cumulative across the whole guide, so a count above a step's
# expectation is what a later legitimate press looks like from an earlier step,
# not evidence of an extra one. The run's final totals answer that question, and
# `verify_hagia_native_session.sh` reads its expectations out of the calls below:
# a step added here is a step it starts checking.
#
# Every wait is bounded. An operator step gets a long bound because a person is
# reading a screen; a session step gets a short one because nobody is. The old
# switcher guide looped forever on session facts, which is how a profile that
# could no longer produce a line turned into a hang instead of a failure.
wait_for_action_count() {
    action="$1"
    expected="$2"
    attempts="${3:-3000}"
    while [ "$(action_count "$action")" -lt "$expected" ]; do
        attempts=$((attempts - 1))
        if [ "$attempts" -le 0 ]; then
            abort "$(action_pattern "$action") (x$expected)"
        fi
        sleep 0.1
    done
}

count_lines() {
    grep -Ec "$1" "$evidence" 2>/dev/null || true
}

wait_for_count() {
    pattern="$1"
    expected="$2"
    attempts="${3:-600}"
    while [ "$(count_lines "$pattern")" -lt "$expected" ]; do
        attempts=$((attempts - 1))
        if [ "$attempts" -le 0 ]; then
            abort "$pattern (x$expected)"
        fi
        sleep 0.1
    done
}

wait_for_line() {
    wait_for_count "$1" 1 "${2:-600}"
}

show_step() {
    printf '\033[2J\033[H'
    printf '%s\n\n%s\n\n%s\n' \
        'HAGIA NATIVE SESSION PROOF' "$1" \
        'This screen advances only after Sophia commits the step.'
}

# The phrase is typed first, while this terminal is the session's only window and
# therefore certainly focused. Sophia matches the routed key events, and this
# guide writes the received phrase to the proof result; both halves need the
# keystrokes to land here, which is only guaranteed before any other window
# exists. Doing it last would make the proof depend on where focus happened to
# settle after a close.
printf '\033[2J\033[H'
printf '%s\n\n%s\n\n' \
    'HAGIA NATIVE SESSION PROOF' \
    "Type $proof_text and press Enter to prove the physical input path."

while IFS= read -r line; do
    if [ "$line" = "$proof_text" ]; then
        break
    fi
    printf '%s\n' "Type the exact phrase: $proof_text"
done

umask 077
printf '%s' "$proof_text" >"$proof_result"

wait_for_line "^sophia_live_session_input schema=2 status=complete source=physical text=$proof_text expected_events=[1-9][0-9]* matched_events=[1-9][0-9]* pixel_change=true$"

# Three launches. Each one waits for its own admitted surface as well as its
# committed action, because an action Sophia committed and a window the operator
# can see are different facts and this proof is about the second one.
show_step 'Press Super+Return once. Wait for the new terminal to appear.'
wait_for_action_count 29 1
wait_for_count '^sophia_session_app schema=2 status=admitted source=action transaction=[1-9][0-9]* surface=[1-9][0-9]*$' 1

show_step 'Press Super+Return once more. Wait for the third terminal.'
wait_for_action_count 29 2
wait_for_count '^sophia_session_app schema=2 status=admitted source=action transaction=[1-9][0-9]* surface=[1-9][0-9]*$' 2

show_step 'Press Super+Return once more. Wait for the fourth terminal, then
confirm all four are laid out without overlap or gaps.'
wait_for_action_count 29 3
wait_for_count '^sophia_session_app schema=2 status=admitted source=action transaction=[1-9][0-9]* surface=[1-9][0-9]*$' 3

show_step 'Press Super+J once. Confirm the focus indicator moves to a different
terminal before continuing.'
wait_for_action_count 1 1

# Both remaining steps are shown together because the close may land on this
# window. Losing the guide then costs the operator nothing: the instruction for
# the last step is already on screen, and logout is a session shortcut that does
# not need a terminal. The verifier reads its expectations from this file rather
# than from what the guide lived long enough to execute.
show_step 'TWO STEPS -- READ BOTH BEFORE ACTING
1. Press Super+q once to close the focused terminal. This window may be the
   one that closes, which is expected.
2. Press Ctrl+Alt+Delete once to log out normally.'
wait_for_action_count 31 1
wait_for_line '^sophia_live_wm schema=1 status=session_action_committed transaction=[1-9][0-9]* action=CloseFocused$'

show_step 'Press Ctrl+Alt+Delete once to log out normally.'
wait_for_action_count 32 1
