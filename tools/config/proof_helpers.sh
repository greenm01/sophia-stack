#!/usr/bin/env bash

sophia_proof_log_lines() {
    if [[ -f "$SESSION_LOG" ]]; then
        wc -l <"$SESSION_LOG"
    else
        printf '0\n'
    fi
}

sophia_proof_wait_for_log() {
    local pattern=$1
    local timeout_seconds=${2:-30}
    local deadline=$((SECONDS + timeout_seconds))
    while ((SECONDS < deadline)); do
        if [[ -f "$SESSION_LOG" && "$SESSION_LOG" -nt "$START_MARKER" ]] &&
            grep -Eq "$pattern" "$SESSION_LOG" 2>/dev/null; then
            return 0
        fi
        sleep 0.1
    done
    printf 'FAILED waiting for: %s\n' "$pattern" >>"$SEQUENCE_LOG"
    return 1
}

sophia_proof_wait_for_new_log() {
    local pattern=$1
    local after_line=$2
    local timeout_seconds=${3:-30}
    local deadline=$((SECONDS + timeout_seconds))
    while ((SECONDS < deadline)); do
        if [[ -f "$SESSION_LOG" && "$SESSION_LOG" -nt "$START_MARKER" ]] &&
            tail -n "+$((after_line + 1))" "$SESSION_LOG" | grep -Eq "$pattern"; then
            return 0
        fi
        sleep 0.1
    done
    printf 'FAILED waiting after line %s for: %s\n' "$after_line" "$pattern" >>"$SEQUENCE_LOG"
    return 1
}
