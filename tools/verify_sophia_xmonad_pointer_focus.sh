#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
LOG_DIR="${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"

fail() {
    echo "xmonad pointer-focus verification failed: $*" >&2
    exit 1
}

[[ -s "$SESSION_LOG" ]] || fail "missing or empty session log: $SESSION_LOG"

if grep -Eq \
    '^sophia_live_session_pointer schema=5 status=focus_handoff_dropped reason=' \
    "$SESSION_LOG"; then
    fail "a pointer focus handoff was dropped"
fi

request_record="$(
    grep -nEm1 \
        '^sophia_live_wm schema=3 status=focus_requested source=pointer surface=[0-9]+$' \
        "$SESSION_LOG" || true
)"
[[ -n "$request_record" ]] || fail "no primary click requested focus"
request_line="${request_record%%:*}"
request="${request_record#*:}"
surface="${request##*surface=}"

commit_line="$(
    awk -v minimum="$request_line" '
        NR > minimum &&
        /^sophia_live_wm schema=1 status=focus_committed transaction=[0-9]+ target=surface$/ {
            print NR
            exit
        }
    ' "$SESSION_LOG"
)"
[[ -n "$commit_line" ]] || fail "Engine did not commit focus after the pointer request"

applied_line="$(
    awk -v minimum="$commit_line" '
        NR > minimum &&
        /^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$/ {
            print NR
            exit
        }
    ' "$SESSION_LOG"
)"
[[ -n "$applied_line" ]] || fail "the X frontend did not acknowledge pointer-selected focus"

release_record="$(
    awk -v minimum="$applied_line" -v surface="$surface" '
        NR > minimum &&
        $0 ~ "^sophia_live_session_pointer schema=5 status=focus_handoff_released surface=" surface " count=[2-9][0-9]*$" {
            print NR ":" $0
            exit
        }
    ' "$SESSION_LOG"
)"
[[ -n "$release_record" ]] ||
    fail "ordered press/release input was not delivered after focus applied"
release_line="${release_record%%:*}"

(( request_line < commit_line
    && commit_line < applied_line
    && applied_line < release_line )) ||
    fail "focus request, commit, frontend acknowledgment, and input release are out of order"

echo \
    "xmonad pointer-focus verification passed: surface=$surface request_line=$request_line commit_line=$commit_line applied_line=$applied_line release_line=$release_line"
