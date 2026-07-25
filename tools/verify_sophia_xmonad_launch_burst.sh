#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
LOG_DIR="${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"
WAIT_SECONDS="${SOPHIA_VERIFY_WAIT_SECONDS:-5}"

fail() {
    echo "xmonad launch-burst verification failed: $*" >&2
    exit 1
}

count() {
    grep -Ec "$1" "$SESSION_LOG" || true
}

line_number() {
    grep -nEm1 "$1" "$SESSION_LOG" | cut -d: -f1
}

[[ -s "$SESSION_LOG" ]] || fail "missing session log: $SESSION_LOG"

deadline=$((SECONDS + WAIT_SECONDS))
while ! grep -Eq '^sophia_live_session_cleanup schema=1 status=clean ' "$SESSION_LOG" ||
    ! grep -Eq '^sophia_live_session schema=14 status=bounded_complete ' "$SESSION_LOG"; do
    (( SECONDS < deadline )) || fail "session log is incomplete"
    sleep 0.1
done

if grep -Eq '(^Error:|panicked at|sophia_live_session_startup .*status=failed|status=degraded)' \
    "$SESSION_LOG"; then
    fail "session log contains a Sophia error, panic, startup failure, or degradation"
fi
if grep -Eq '^sophia_session_app schema=2 status=failed ' "$SESSION_LOG"; then
    fail "an admitted application failed or timed out"
fi

baseline_line="$(line_number '^sophia_live_session_startup schema=2 status=output_baseline_ready ')"
ready_line="$(line_number '^sophia_live_session_startup schema=2 status=ready ')"
first_start_line="$(line_number '^sophia_session_app schema=2 status=started .* source=action ')"
[[ -n "$baseline_line" && -n "$ready_line" && -n "$first_start_line" ]] ||
    fail "startup baseline, readiness, or action-start evidence is missing"
(( baseline_line < first_start_line && ready_line < first_start_line )) ||
    fail "an action application started before startup readiness"

queued="$(count '^sophia_session_app schema=2 status=queued source=action ')"
started="$(count '^sophia_session_app schema=2 status=started .* source=action ')"
admitted="$(count '^sophia_session_app schema=2 status=admitted source=action ')"
(( queued >= 2 && queued <= 16 )) ||
    fail "expected two to sixteen accepted burst requests, observed $queued"
(( started == queued )) ||
    fail "only $started of $queued accepted requests started"
(( admitted == started )) ||
    fail "only $admitted of $started action applications reached stable admission"

awk '
    /^sophia_session_app schema=2 status=started .* source=action / {
        in_flight++
        if (in_flight > 1) exit 1
    }
    /^sophia_session_app schema=2 status=(admitted|failed).* source=action / {
        in_flight--
        if (in_flight < 0) exit 1
    }
    END {
        if (in_flight != 0) exit 1
    }
' "$SESSION_LOG" || fail "more than one application admission was in flight"

grep -Eq '^sophia_live_session_input_pipeline schema=1 status=key_routed' "$SESSION_LOG" ||
    fail "post-burst terminal input was not routed"
grep -Eq '^sophia_session_launches schema=1 status=complete peak_depth=([0-9]|1[0-6]) rejected=[0-9]+ admission_timeouts=0$' \
    "$SESSION_LOG" ||
    fail "launch completion counters are missing or invalid"
grep -Eq '^sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=none$' \
    "$SESSION_LOG" ||
    fail "native presentation did not drain cleanly"
grep -Eq '^sophia_live_session_cleanup schema=1 status=clean ' "$SESSION_LOG" ||
    fail "session cleanup did not finish cleanly"

mapfile -t outputs < <(
    grep -E '^sophia_live_output schema=1 status=complete ' "$SESSION_LOG"
)
(( ${#outputs[@]} >= 1 )) || fail "session has no per-output completion evidence"
for output in "${outputs[@]}"; do
    callbacks="$(sed -n 's/.* callbacks=\([0-9][0-9]*\) .*/\1/p' <<<"$output")"
    retirements="$(sed -n 's/.* retirements=\([0-9][0-9]*\) .*/\1/p' <<<"$output")"
    [[ -n "$callbacks" && -n "$retirements" ]] ||
        fail "malformed output completion: $output"
    (( callbacks > 0 && callbacks == retirements )) ||
        fail "output callback lifecycle is incomplete: $output"
done

echo "xmonad launch-burst session verified: $SESSION_LOG"
