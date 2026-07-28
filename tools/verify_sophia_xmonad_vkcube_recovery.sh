#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
LOG_DIR="${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"
WAIT_SECONDS="${SOPHIA_VERIFY_WAIT_SECONDS:-5}"

fail() {
    echo "vkcube xmonad verification failed: $*" >&2
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

[[ -s "$SESSION_LOG" ]] || fail "missing session log: $SESSION_LOG"

deadline=$((SECONDS + WAIT_SECONDS))
while ! grep -Eq '^sophia_live_session_cleanup schema=1 status=clean ' "$SESSION_LOG" ||
    ! grep -Eq '^sophia_live_session schema=15 status=bounded_complete ' "$SESSION_LOG"; do
    ((SECONDS < deadline)) || fail "session log is incomplete"
    sleep 0.1
done

if grep -Eq \
    '(^Error:|panicked at|admission_group_(invalid|overflowed)|mismatched.transaction|status=(failed|degraded)([[:space:]]|$))' \
    "$SESSION_LOG"; then
    fail "session contains an error, invalid admission group, or degraded status"
fi

mapfile -t armed < <(
    grep -E \
        '^sophia_live_visual_admission schema=1 status=armed transaction=[0-9]+ surface=[0-9]+$' \
        "$SESSION_LOG"
)
((${#armed[@]} > 0)) || fail "no retirement-gated visual admission was armed"

for record in "${armed[@]}"; do
    transaction="$(field "$record" transaction)" ||
        fail "armed admission lacks a transaction"
    surface="$(field "$record" surface)" ||
        fail "armed admission lacks a surface"
    grep -Eq \
        "^sophia_live_visual_candidate schema=1 status=selected transaction=${transaction} surface=${surface} width=[0-9]+ height=[0-9]+ evidence=PresentedBuffer$" \
        "$SESSION_LOG" ||
        fail "surface $surface transaction $transaction was not selected from presented-buffer evidence"
    grep -Eq \
        "^sophia_live_visual_admission schema=1 status=presented transaction=${transaction} surface=${surface}$" \
        "$SESSION_LOG" ||
        fail "surface $surface transaction $transaction never completed visual admission"
    grep -Eq \
        "^sophia_live_session_present schema=2 status=retired transaction=${transaction} surface=${surface} " \
        "$SESSION_LOG" ||
        fail "surface $surface transaction $transaction has no matching page-flip retirement"
done

grep -Eq '^sophia_live_session_cleanup schema=1 status=clean ' "$SESSION_LOG" ||
    fail "session cleanup was not clean"

echo "vkcube xmonad verification passed: ${#armed[@]} visual admission(s) retired exactly"
