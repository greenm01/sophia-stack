#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUNTIME_ROOT="${XDG_RUNTIME_DIR:-/tmp}"
PROOF_DIR="$RUNTIME_ROOT/sophia-native-hot-reload-${UID}"
WM_CONFIG="$PROOF_DIR/wm.kdl"
NEXT_CONFIG="$PROOF_DIR/wm.next"
SESSION_LOG="${XDG_STATE_HOME:-${HOME}/.local/state}/sophia/native-session/session.log"
SEQUENCE_LOG="$PROOF_DIR/sequence.log"
START_MARKER="$PROOF_DIR/start.marker"

mkdir -p "$PROOF_DIR"
chmod 700 "$PROOF_DIR"

write_wm_config() {
    local path=$1
    local width=$2
    local extra=${3:-}
    {
        printf '%s\n' \
            '/- kdl-version 2' \
            'schema 2' \
            '' \
            'policy timeout-ms=300' \
            'workspace 1' \
            'workspace 2' \
            'workspace 3' \
            'workspace 4' \
            'workspace 5' \
            'workspace 6' \
            'workspace 7' \
            'workspace 8' \
            'workspace 9' \
            'layout "columns"' \
            'action "focus-next" id=1 behavior="focus-next"' \
            'action "workspace-two" id=2 behavior="activate-workspace" workspace=2' \
            'action "terminal" id=3 behavior="launch-application" application=1' \
            'action "logout" id=4 behavior="logout"' \
            'binding action=1 keycode=36 modifiers="super"' \
            'binding action=2 keycode=3 modifiers="super"' \
            'binding action=3 keycode=28 modifiers="super"' \
            'binding action=4 keycode=16 modifiers="super+shift"' \
            'chrome {' \
            "    focus-ring enabled=#true width=$width color=\"#70b7ff\"" \
            '    frame enabled=#false width=0 focused-color="#70b7ff" unfocused-color="#303030"' \
            '}'
        [[ -z "$extra" ]] || printf '%s\n' "$extra"
    } >"$path"
    chmod 600 "$path"
}

wait_for_log() {
    local pattern=$1
    local timeout_seconds=${2:-30}
    local deadline=$((SECONDS + timeout_seconds))
    while (( SECONDS < deadline )); do
        if [[ -f "$SESSION_LOG" && "$SESSION_LOG" -nt "$START_MARKER" ]] &&
            grep -Eq "$pattern" "$SESSION_LOG" 2>/dev/null; then
            return 0
        fi
        sleep 0.1
    done
    printf 'FAILED waiting for: %s\n' "$pattern" >>"$SEQUENCE_LOG"
    return 1
}

write_wm_config "$WM_CONFIG" 2
: >"$SEQUENCE_LOG"
chmod 600 "$SEQUENCE_LOG"
: >"$START_MARKER"

(
    wait_for_log '^sophia_live_session_startup schema=2 status=ready ' 180 || exit 1
    printf '%s\n' 'phase=baseline focus_ring_width=2' >>"$SEQUENCE_LOG"
    sleep 5

    write_wm_config "$NEXT_CONFIG" 6
    mv -f "$NEXT_CONFIG" "$WM_CONFIG"
    wait_for_log '^sophia_live_wm_policy schema=2 status=applied generation=2 .*focus_ring_width=6 .*clearance=6$' ||
        exit 1
    printf '%s\n' 'phase=valid_applied generation=2 focus_ring_width=6' >>"$SEQUENCE_LOG"
    sleep 5

    write_wm_config "$NEXT_CONFIG" 9 'unknown-node #true'
    mv -f "$NEXT_CONFIG" "$WM_CONFIG"
    wait_for_log '^sophia_wm_config_reload schema=2 status=rejected reason=parse ' || exit 1
    printf '%s\n' 'phase=invalid_rejected retained_focus_ring_width=6' >>"$SEQUENCE_LOG"
    sleep 5

    rm -f "$WM_CONFIG"
    wait_for_log '^sophia_wm_config_reload schema=2 status=rejected reason=read ' || exit 1
    printf '%s\n' 'phase=deletion_rejected retained_focus_ring_width=6' >>"$SEQUENCE_LOG"
    sleep 3

    write_wm_config "$NEXT_CONFIG" 4
    mv -f "$NEXT_CONFIG" "$WM_CONFIG"
    wait_for_log '^sophia_live_wm_policy schema=2 status=applied generation=3 .*focus_ring_width=4 .*clearance=4$' ||
        exit 1
    printf '%s\n' 'phase=recreated_applied generation=3 focus_ring_width=4' >>"$SEQUENCE_LOG"
) &
sequence_pid=$!

cleanup_sequence() {
    local status=$?
    if kill -0 "$sequence_pid" 2>/dev/null; then
        kill -TERM "$sequence_pid" 2>/dev/null || true
        wait "$sequence_pid" 2>/dev/null || true
    else
        wait "$sequence_pid" || status=1
    fi
    printf 'Native hot-reload sequence log: %s\n' "$SEQUENCE_LOG"
    return "$status"
}
trap cleanup_sequence EXIT

printf '%s\n' \
    'Native WM hot-reload proof:' \
    '  1. Wait five seconds after Kitty appears; the focus ring changes from 2px to 6px.' \
    '  2. It must remain 6px through the invalid-edit and deletion phases.' \
    '  3. It then changes to 4px after the configuration is recreated.' \
    '  4. During every phase, use Super+Enter and verify the new Kitty is interactive.' \
    '  5. Press Super+Shift+Q for normal logout after the final 4px ring appears.' \
    "Evidence: $SESSION_LOG" \
    "Sequence: $SEQUENCE_LOG"

export SOPHIA_TTY_PROFILE=native
"$ROOT_DIR/tools/start_sophia_tty3.sh" \
    --no-config \
    "--wm-process-arg=--wm-config=$WM_CONFIG" \
    "$@"
