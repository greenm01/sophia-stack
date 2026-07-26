#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUNTIME_ROOT="${XDG_RUNTIME_DIR:-/tmp}"
PROOF_DIR="$RUNTIME_ROOT/sophia-xmonad-config-reload-${UID}"
CORE_CONFIG="$PROOF_DIR/config.kdl"
NEXT_CONFIG="$PROOF_DIR/config.next"
SESSION_LOG="${XDG_STATE_HOME:-${HOME}/.local/state}/sophia/xmonad-session/session.log"
SEQUENCE_LOG="$PROOF_DIR/sequence.log"
START_MARKER="$PROOF_DIR/start.marker"
source "$ROOT_DIR/tools/config/proof_helpers.sh"

mkdir -p "$PROOF_DIR"
chmod 700 "$PROOF_DIR"

write_core_config() {
    local path=$1
    local ring_width=$2
    local namespace_profile=$3
    local extra=${4:-}
    "$ROOT_DIR/tools/config/write_core_chrome_config.sh" \
        "$path" true "$ring_width" false 0 "$namespace_profile" true "$extra"
}

write_core_config "$CORE_CONFIG" 2 classic-shared
: >"$SEQUENCE_LOG"
chmod 600 "$SEQUENCE_LOG"
: >"$START_MARKER"
printf 'commit=%s\n' "$(git -C "$ROOT_DIR" rev-parse HEAD)" >>"$SEQUENCE_LOG"

(
    sophia_proof_wait_for_log '^sophia_live_session_startup schema=2 status=ready ' 180 || exit 1
    sophia_proof_wait_for_log '^sophia_live_wm_chrome schema=1 status=negotiated source=core_fallback capability=false clearance=2$' ||
        exit 1
    sophia_proof_wait_for_log '^sophia_live_wm schema=1 status=layout_committed .* surfaces=2 .* outcome=Committed$' 180 ||
        exit 1
    sophia_proof_wait_for_log '^sophia_live_compositor_chrome_set schema=1 status=composed .* eligible_surfaces=2 .* focus_rings=1 primitives=4 clearance=2$' ||
        exit 1
    printf '%s\n' 'phase=external_baseline source=core_fallback focus_ring_width=2' >>"$SEQUENCE_LOG"

    baseline="$(sophia_proof_log_lines)"
    write_core_config "$NEXT_CONFIG" 4 classic-shared
    mv -f "$NEXT_CONFIG" "$CORE_CONFIG"
    sophia_proof_wait_for_new_log '^sophia_config_reload schema=1 status=applied generation=2 .*chrome_changed=true ' "$baseline" ||
        exit 1
    sophia_proof_wait_for_new_log '^sophia_live_resize_epoch schema=1 status=committed transaction=[0-9]+ matched_surfaces=2$' "$baseline" ||
        exit 1
    sophia_proof_wait_for_new_log '^sophia_live_compositor_chrome_set schema=1 status=composed .* eligible_surfaces=2 .* focus_rings=1 primitives=4 clearance=4$' "$baseline" ||
        exit 1
    printf '%s\n' 'phase=core_live_applied generation=2 focus_ring_width=4' >>"$SEQUENCE_LOG"

    baseline="$(sophia_proof_log_lines)"
    write_core_config "$NEXT_CONFIG" 6 confined
    mv -f "$NEXT_CONFIG" "$CORE_CONFIG"
    sophia_proof_wait_for_new_log '^sophia_config_reload schema=1 status=pending_restart generation=3 ' "$baseline" ||
        exit 1
    sleep 2
    if tail -n "+$((baseline + 1))" "$SESSION_LOG" |
        grep -Eq 'status=applied generation=3|chrome_set .* clearance=6$'; then
        printf '%s\n' 'FAILED pending-restart candidate partially applied' >>"$SEQUENCE_LOG"
        exit 1
    fi
    printf '%s\n' 'phase=pending_restart_retained candidate_width=6 active_width=4' >>"$SEQUENCE_LOG"

    baseline="$(sophia_proof_log_lines)"
    write_core_config "$NEXT_CONFIG" 7 classic-shared 'unknown-node #true'
    mv -f "$NEXT_CONFIG" "$CORE_CONFIG"
    sophia_proof_wait_for_new_log '^sophia_config_reload schema=1 status=rejected reason=parse ' "$baseline" ||
        exit 1
    printf '%s\n' 'phase=invalid_rejected active_width=4' >>"$SEQUENCE_LOG"

    baseline="$(sophia_proof_log_lines)"
    write_core_config "$NEXT_CONFIG" 2 classic-shared
    mv -f "$NEXT_CONFIG" "$CORE_CONFIG"
    sophia_proof_wait_for_new_log '^sophia_config_reload schema=1 status=applied generation=3 .*chrome_changed=true ' "$baseline" ||
        exit 1
    sophia_proof_wait_for_new_log '^sophia_live_resize_epoch schema=1 status=committed transaction=[0-9]+ matched_surfaces=2$' "$baseline" ||
        exit 1
    sophia_proof_wait_for_new_log '^sophia_live_compositor_chrome_set schema=1 status=composed .* eligible_surfaces=2 .* focus_rings=1 primitives=4 clearance=2$' "$baseline" ||
        exit 1
    printf '%s\n' 'phase=core_restored generation=3 focus_ring_width=2' >>"$SEQUENCE_LOG"
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
    printf 'External config sequence log: %s\n' "$SEQUENCE_LOG"
    return "$status"
}
trap cleanup_sequence EXIT

printf '%s\n' \
    'External xmonad core-config proof:' \
    '  1. After startup Kitty appears, press Super+Enter and wait for its prompt.' \
    '  2. Confirm the core fallback ring changes 2px -> 4px atomically.' \
    '  3. It must remain 4px during the pending-restart and invalid phases.' \
    '  4. Confirm it returns atomically to 2px, then type in both Kitty windows.' \
    '  5. Press Super+Shift+Q for normal logout.' \
    "Evidence: $SESSION_LOG" \
    "Sequence: $SEQUENCE_LOG"

export SOPHIA_TTY_PROFILE=xmonad
"$ROOT_DIR/tools/start_sophia_tty3.sh" "--config=$CORE_CONFIG" "$@"
