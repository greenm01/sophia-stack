#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
LOG_DIR="${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"

fail() {
    echo "xmonad click/drag focus verification failed: $*" >&2
    exit 1
}

[[ -s "$SESSION_LOG" ]] || fail "missing or empty session log: $SESSION_LOG"
if grep -Eq \
    '^sophia_live_session_pointer schema=5 status=focus_handoff_dropped reason=' \
    "$SESSION_LOG"; then
    fail "a pointer focus handoff was dropped"
fi

sequence="$(
    awk '
        function surface_field(record, value) {
            value = record
            sub(/^.*surface=/, "", value)
            sub(/ .*/, "", value)
            return value
        }
        function invalidate() {
            invalid = 1
            exit
        }
        phase == 0 &&
            /^sophia_live_wm schema=3 status=focus_requested source=pointer surface=[0-9]+$/ {
            target = surface_field($0)
            request_line = NR
            phase = 1
            next
        }
        phase == 1 &&
            /^sophia_live_wm schema=1 status=focus_committed transaction=[0-9]+ target=surface$/ {
            commit_line = NR
            phase = 2
            next
        }
        phase == 2 &&
            /^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$/ {
            applied_line = NR
            phase = 3
            next
        }
        phase == 3 &&
            $0 ~ "^sophia_live_session_pointer schema=5 status=focus_handoff_released surface=" target " count=[0-9]+$" {
            count = $0
            sub(/^.*count=/, "", count)
            minimum = completed == 0 ? 2 : 3
            if (count < minimum) {
                invalidate()
            }
            release_line = NR
            phase = 4
            next
        }
        phase == 4 &&
            $0 == "sophia_live_session_pointer schema=6 status=focused_key_routed surface=" target {
            completed++
            print completed, target, request_line, commit_line, applied_line, release_line, NR
            phase = 0
            if (completed == 2) {
                exit
            }
            next
        }
        END {
            if (invalid || completed != 2 || phase != 0) {
                exit 1
            }
        }
    ' "$SESSION_LOG"
)" || fail "plain-click and click-drag handoffs did not both complete in order"

first="$(printf '%s\n' "$sequence" | sed -n '1p')"
second="$(printf '%s\n' "$sequence" | sed -n '2p')"
[[ -n "$first" && -n "$second" ]] ||
    fail "two complete pointer focus sequences were not retained"

echo "xmonad click/drag focus verification passed: click=($first) drag=($second)"
