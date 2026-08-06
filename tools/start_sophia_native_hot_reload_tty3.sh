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
source "$ROOT_DIR/tools/config/proof_helpers.sh"
stage_dwell_seconds="${SOPHIA_NATIVE_CHROME_STAGE_DWELL_SECONDS:-3}"
[[ "$stage_dwell_seconds" =~ ^[1-9][0-9]*$ \
    && "$stage_dwell_seconds" -le 10 ]] || {
    echo "SOPHIA_NATIVE_CHROME_STAGE_DWELL_SECONDS must be from 1 through 10." >&2
    exit 1
}

terminal_bin="${SOPHIA_TERMINAL_BIN:-$(command -v kitty || true)}"
if [[ -z "$terminal_bin" || ! -x "$terminal_bin" ]]; then
    echo "The native chrome proof requires Kitty; set SOPHIA_TERMINAL_BIN if it is installed elsewhere." >&2
    exit 1
fi

mkdir -p "$PROOF_DIR"
chmod 700 "$PROOF_DIR"

write_wm_config() {
    local path=$1
    local ring_enabled=$2
    local ring_width=$3
    local frame_enabled=$4
    local frame_width=$5
    local extra=${6:-}
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
            "    focus-ring enabled=#$ring_enabled width=$ring_width color=\"#70b7ff\"" \
            "    frame enabled=#$frame_enabled width=$frame_width focused-color=\"#70b7ff\" unfocused-color=\"#303030\"" \
            '}'
        [[ -z "$extra" ]] || printf '%s\n' "$extra"
    } >"$path"
    chmod 600 "$path"
}

write_wm_config "$WM_CONFIG" true 2 false 0
: >"$SEQUENCE_LOG"
chmod 600 "$SEQUENCE_LOG"
: >"$START_MARKER"
if [[ -f "$ROOT_DIR/manifest" ]]; then
    proof_commit="$(sed -n 's/^commit=//p' "$ROOT_DIR/manifest" | head -n 1)"
else
    proof_commit="$(git -C "$ROOT_DIR" rev-parse HEAD)"
fi
[[ "$proof_commit" =~ ^[0-9a-f]{40}$ ]] || {
    echo "The native chrome proof could not resolve its release commit." >&2
    exit 1
}
printf 'commit=%s\n' "$proof_commit" >>"$SEQUENCE_LOG"

(
    sophia_proof_wait_for_log '^sophia_live_session_startup schema=2 status=ready ' 180 || exit 1
    sophia_proof_wait_for_log '^sophia_live_wm schema=1 status=layout_committed .* surfaces=2 .* outcome=Committed$' 180 ||
        exit 1
    sophia_proof_wait_for_log '^sophia_live_compositor_chrome_set schema=1 status=composed .* eligible_surfaces=2 frames=0 focused_frames=0 unfocused_frames=0 focus_rings=1 primitives=4 clearance=2$' ||
        exit 1
    printf '%s\n' 'phase=ring_baseline focus_ring_width=2 frame_width=0' >>"$SEQUENCE_LOG"
    # Keep each retired phase visible long enough for physical inspection.
    sleep "$stage_dwell_seconds"

    baseline="$(sophia_proof_log_lines)"
    write_wm_config "$NEXT_CONFIG" true 6 false 0
    mv -f "$NEXT_CONFIG" "$WM_CONFIG"
    sophia_proof_wait_for_new_log '^sophia_live_wm_policy schema=2 status=applied generation=2 .*focus_ring_width=6 .*frame_width=0 clearance=6$' "$baseline" ||
        exit 1
    sophia_proof_wait_for_new_log '^sophia_live_resize_epoch schema=1 status=committed transaction=[0-9]+ matched_surfaces=2$' "$baseline" ||
        exit 1
    sophia_proof_wait_for_new_log '^sophia_live_session_present schema=2 status=retired .* target=[0-9]+x[0-9]+_[0-9]+_6 ' "$baseline" ||
        exit 1
    sophia_proof_wait_for_new_log '^sophia_live_compositor_chrome_set schema=1 status=composed .* eligible_surfaces=2 frames=0 focused_frames=0 unfocused_frames=0 focus_rings=1 primitives=4 clearance=6$' "$baseline" ||
        exit 1
    printf '%s\n' 'phase=ring_wide generation=2 focus_ring_width=6 frame_width=0' >>"$SEQUENCE_LOG"
    sleep "$stage_dwell_seconds"

    baseline="$(sophia_proof_log_lines)"
    write_wm_config "$NEXT_CONFIG" true 9 false 0 'unknown-node #true'
    mv -f "$NEXT_CONFIG" "$WM_CONFIG"
    sophia_proof_wait_for_new_log '^sophia_wm_config_reload schema=2 status=rejected reason=parse ' "$baseline" ||
        exit 1
    printf '%s\n' 'phase=invalid_rejected retained_focus_ring_width=6' >>"$SEQUENCE_LOG"
    sleep "$stage_dwell_seconds"

    baseline="$(sophia_proof_log_lines)"
    rm -f "$WM_CONFIG"
    sophia_proof_wait_for_new_log '^sophia_wm_config_reload schema=2 status=rejected reason=read ' "$baseline" ||
        exit 1
    printf '%s\n' 'phase=deletion_rejected retained_focus_ring_width=6' >>"$SEQUENCE_LOG"
    sleep "$stage_dwell_seconds"

    baseline="$(sophia_proof_log_lines)"
    write_wm_config "$NEXT_CONFIG" false 0 true 4
    mv -f "$NEXT_CONFIG" "$WM_CONFIG"
    sophia_proof_wait_for_new_log '^sophia_live_wm_policy schema=2 status=applied generation=3 .*focus_ring_width=0 .*frame_width=4 clearance=4$' "$baseline" ||
        exit 1
    sophia_proof_wait_for_new_log '^sophia_live_resize_epoch schema=1 status=committed transaction=[0-9]+ matched_surfaces=2$' "$baseline" ||
        exit 1
    sophia_proof_wait_for_new_log '^sophia_live_session_present schema=2 status=retired .* target=[0-9]+x[0-9]+_[0-9]+_4 ' "$baseline" ||
        exit 1
    sophia_proof_wait_for_new_log '^sophia_live_compositor_chrome_set schema=1 status=composed .* eligible_surfaces=2 frames=2 focused_frames=1 unfocused_frames=1 focus_rings=0 primitives=8 clearance=4$' "$baseline" ||
        exit 1
    printf '%s\n' 'phase=frame_only generation=3 focus_ring_width=0 frame_width=4' >>"$SEQUENCE_LOG"
    sleep "$stage_dwell_seconds"

    baseline="$(sophia_proof_log_lines)"
    write_wm_config "$NEXT_CONFIG" true 2 true 6
    mv -f "$NEXT_CONFIG" "$WM_CONFIG"
    sophia_proof_wait_for_new_log '^sophia_live_wm_policy schema=2 status=applied generation=4 .*focus_ring_width=2 .*frame_width=6 clearance=6$' "$baseline" ||
        exit 1
    sophia_proof_wait_for_new_log '^sophia_live_resize_epoch schema=1 status=committed transaction=[0-9]+ matched_surfaces=2$' "$baseline" ||
        exit 1
    sophia_proof_wait_for_new_log '^sophia_live_session_present schema=2 status=retired .* target=[0-9]+x[0-9]+_[0-9]+_6 ' "$baseline" ||
        exit 1
    sophia_proof_wait_for_new_log '^sophia_live_compositor_chrome_set schema=1 status=composed .* eligible_surfaces=2 frames=2 focused_frames=1 unfocused_frames=1 focus_rings=1 primitives=12 clearance=6$' "$baseline" ||
        exit 1
    printf '%s\n' 'phase=combined generation=4 focus_ring_width=2 frame_width=6' >>"$SEQUENCE_LOG"
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
    '  1. Wait while both Kitty windows start and the chrome modes advance automatically.' \
    '  2. After combined mode appears, focus both windows and type in each.' \
    '  3. Press Super+Shift+Q for normal logout.' \
    "Evidence: $SESSION_LOG" \
    "Sequence: $SEQUENCE_LOG"

export SOPHIA_TTY_PROFILE=native
session_args=(
    --no-config
    "--session-app=terminal-secondary=$terminal_bin"
    --session-start=terminal-secondary
    --session-app-arg=terminal-secondary=--config
    --session-app-arg=terminal-secondary=NONE
    --session-app-arg=terminal-secondary=--override
    --session-app-arg=terminal-secondary=linux_display_server=x11
    --session-app-arg=terminal-secondary=--override
    --session-app-arg=terminal-secondary=background_opacity=1
    --session-app-arg=terminal-secondary=--title
    "--session-app-arg=terminal-secondary=Sophia Native TTY3 Secondary"
    "--wm-process-arg=--wm-config=$WM_CONFIG"
    "$@"
)
if [[ "${SOPHIA_NATIVE_CHROME_INSTALLED:-false}" == true ]]; then
    export SOPHIA_INSTALLED_ATTEMPT_MODE=native-chrome
    export SOPHIA_NATIVE_CHROME_SEQUENCE_LOG="$SEQUENCE_LOG"
    "$ROOT_DIR/bin/sophia-session" "${session_args[@]}"
else
    "$ROOT_DIR/tools/start_sophia_tty3.sh" "${session_args[@]}"
fi
