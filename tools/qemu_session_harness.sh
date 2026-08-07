#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${SOPHIA_QEMU_OUT_DIR:-$ROOT_DIR/.qemu}"
KERNEL_VERSION="${SOPHIA_QEMU_KERNEL_VERSION:-$(uname -r)}"
KERNEL_IMAGE="${SOPHIA_QEMU_KERNEL:-/boot/vmlinuz-$KERNEL_VERSION}"
INITRAMFS="${SOPHIA_QEMU_INITRAMFS:-$OUT_DIR/sophia-$KERNEL_VERSION.img}"
SCENARIO="${SOPHIA_QEMU_SCENARIO:-session}"
TWO_XTERM="${SOPHIA_QEMU_TWO_XTERM:-0}"
GPU_MODE="${SOPHIA_QEMU_GPU_MODE:-software}"
RENDER_NODE="${SOPHIA_QEMU_RENDER_NODE:-/dev/dri/renderD128}"
if [[ "$SCENARIO" != "session" && "$SCENARIO" != "emergency-recovery" && "$SCENARIO" != "gtk-classic" && "$SCENARIO" != "gtk-confined" && "$SCENARIO" != "xmonad-m7" && "$SCENARIO" != "xmonad-idle-efficiency" && "$SCENARIO" != "xmonad-launch-burst" && "$SCENARIO" != "xmonad-producer-overload" && "$SCENARIO" != "xmonad-render-contention" && "$SCENARIO" != "xmonad-resize-storm" && "$SCENARIO" != "xmonad-stale-response" && "$SCENARIO" != "xmonad-m8-launcher" && "$SCENARIO" != "xmonad-m8-mix" && "$SCENARIO" != "xmonad-m8-soak" && "$SCENARIO" != "xmonad-interactive" ]]; then
    echo "SOPHIA_QEMU_SCENARIO must include a supported session or xmonad scenario" >&2
    exit 1
fi
if [[ "$GPU_MODE" != software && "$GPU_MODE" != virgl ]]; then
    echo "SOPHIA_QEMU_GPU_MODE must be software or virgl" >&2
    exit 1
fi
if [[ ("$SCENARIO" == "xmonad-idle-efficiency" || "$SCENARIO" == "xmonad-producer-overload" || "$SCENARIO" == "xmonad-render-contention") && "$GPU_MODE" != virgl ]]; then
    echo "$SCENARIO requires SOPHIA_QEMU_GPU_MODE=virgl" >&2
    exit 1
fi
if [[ "$SCENARIO" == "xmonad-interactive" && "$GPU_MODE" != software ]]; then
    echo "xmonad-interactive requires the visible software VNC backend" >&2
    exit 1
fi
if [[ "$TWO_XTERM" != "0" && "$TWO_XTERM" != "1" ]]; then
    echo "SOPHIA_QEMU_TWO_XTERM must be 0 or 1" >&2
    exit 1
fi
if [[ "$SCENARIO" == "emergency-recovery" && "$TWO_XTERM" != "0" ]]; then
    echo "SOPHIA_QEMU_TWO_XTERM is only supported by the session scenario" >&2
    exit 1
fi
if [[ "$SCENARIO" == "emergency-recovery" ]]; then
    DEFAULT_EVIDENCE_FILE="/tmp/sophia-qemu-emergency-recovery.log"
elif [[ "$SCENARIO" == gtk-* ]]; then
    DEFAULT_EVIDENCE_FILE="/tmp/sophia-qemu-$SCENARIO.log"
elif [[ "$SCENARIO" == xmonad-* ]]; then
    DEFAULT_EVIDENCE_FILE="/tmp/sophia-qemu-$SCENARIO.log"
else
    DEFAULT_EVIDENCE_FILE="/tmp/sophia-qemu-session.log"
fi
EVIDENCE_FILE="${SOPHIA_QEMU_EVIDENCE:-$DEFAULT_EVIDENCE_FILE}"
QEMU_BIN="${SOPHIA_QEMU_BIN:-qemu-system-x86_64}"
MEMORY_MIB="${SOPHIA_QEMU_MEMORY_MIB:-2048}"
VIRTUAL_CPUS="${SOPHIA_QEMU_CPUS:-2}"
VNC_SOCKET="${SOPHIA_QEMU_VNC_SOCKET:-$OUT_DIR/display.sock}"
QMP_SOCKET="${SOPHIA_QEMU_QMP_SOCKET:-$OUT_DIR/qmp.sock}"
SERIAL_FIFO="${SOPHIA_QEMU_SERIAL_FIFO:-$OUT_DIR/serial.fifo}"
INTERACTIVE_TRACE_FIFO="${SOPHIA_QEMU_INTERACTIVE_TRACE_FIFO:-$OUT_DIR/interactive-trace.fifo}"
INTERACTIVE_VIEWER="${SOPHIA_QEMU_INTERACTIVE_VIEWER:-auto}"
QEMU_PID=""
LOGGER_PID=""
TRACE_LOGGER_PID=""
VIEWER_PID=""

evidence_count() {
    grep -c "$1" "$EVIDENCE_FILE" 2>/dev/null || true
}

present_progress_field() {
    local key=$1
    awk -v key="$key" '
        /^sophia_live_present_progress schema=1 / {
            for (i = 1; i <= NF; i++) {
                split($i, pair, "=")
                if (pair[1] == key) value = pair[2]
            }
        }
        END { print value + 0 }
    ' "$EVIDENCE_FILE" 2>/dev/null
}

axis_route_count() {
    awk '
        /^sophia_live_session_pointer schema=9 status=axis_batch / {
            for (i = 1; i <= NF; i++) {
                if ($i ~ /^routed=[0-9]+$/) {
                    split($i, field, "=")
                    total += field[2]
                }
            }
        }
        END { print total + 0 }
    ' "$EVIDENCE_FILE" 2>/dev/null
}

dmabuf_retirement_surface_count() {
    awk '
        /^sophia_live_session_present schema=2 status=retired / {
            for (i = 1; i <= NF; i++) {
                if ($i ~ /^surface=[0-9]+$/) surfaces[$i] = 1
            }
        }
        END {
            for (surface in surfaces) count++
            print count + 0
        }
    ' "$EVIDENCE_FILE" 2>/dev/null
}

dmabuf_retirement_window_stats() {
    local start_line=$1
    awk -v start_line="$start_line" '
        NR > start_line && /^sophia_live_session_present schema=2 status=retired / {
            for (i = 1; i <= NF; i++) {
                if ($i ~ /^surface=[0-9]+$/) {
                    surface = $i
                    sub(/^surface=/, "", surface)
                    counts[surface]++
                }
            }
        }
        END {
            minimum = -1
            for (surface in counts) {
                surfaces++
                total += counts[surface]
                if (minimum < 0 || counts[surface] < minimum) minimum = counts[surface]
            }
            if (minimum < 0) minimum = 0
            print surfaces + 0, minimum + 0, total + 0
        }
    ' "$EVIDENCE_FILE" 2>/dev/null
}

wait_for_axis_route_count() {
    local baseline=$1
    local attempts=${2:-400}
    local current
    for _ in $(seq 1 "$attempts"); do
        current="$(axis_route_count)"
        if (( current > baseline )); then
            return 0
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then
            return 1
        fi
        sleep 0.05
    done
    return 1
}

evidence_has_after_line() {
    local pattern=$1
    local line=$2
    awk -v pattern="$pattern" -v line="$line" '
        NR > line && $0 ~ pattern { found = 1 }
        END { exit(found ? 0 : 1) }
    ' "$EVIDENCE_FILE"
}

wait_for_new_evidence() {
    local pattern=$1
    local baseline=$2
    local attempts=${3:-400}
    local current
    for _ in $(seq 1 "$attempts"); do
        current="$(evidence_count "$pattern")"
        if (( current > baseline )); then
            return 0
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then
            return 1
        fi
        sleep 0.05
    done
    return 1
}

wait_for_evidence_count_at_least() {
    local pattern=$1
    local expected=$2
    local attempts=${3:-400}
    local current
    for _ in $(seq 1 "$attempts"); do
        current="$(evidence_count "$pattern")"
        if (( current >= expected )); then
            return 0
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then
            return 1
        fi
        sleep 0.05
    done
    return 1
}

send_chord_and_wait() {
    local chord=$1
    local pattern=$2
    local label=$3
    local baseline
    baseline="$(evidence_count "$pattern")"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" "$chord"
    echo "sophia_qemu_xmonad_input schema=1 status=sent chord=$chord" | tee -a "$EVIDENCE_FILE"
    if ! wait_for_new_evidence "$pattern" "$baseline"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=action_evidence_timeout action=$label chord=$chord" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
}

position_left_content_pointer() {
    if ! "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" -4096 -4096 0 ||
        ! "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" 32 0 0; then
        return 1
    fi
    for _ in $(seq 1 8); do
        "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" 0 16 0 || return 1
    done
}

position_left_dialog_confirmation_pointer() {
    # The fixture places its modal confirmation over the isolated-page anchor.
    # Preserve that proven target instead of guessing browser chrome geometry.
    echo "sophia_qemu_xmonad_pointer schema=3 status=positioned anchor=dialog_confirmation source=isolated_page movement=none" |
        tee -a "$EVIDENCE_FILE"
}

run_pointer_focus_gesture() {
    local gesture=$1
    local key=$2
    local pointer_request_baseline
    local pointer_release_baseline
    local pointer_key_baseline

    pointer_request_baseline="$(evidence_count '^sophia_live_wm schema=3 status=focus_requested source=pointer surface=')"
    pointer_release_baseline="$(evidence_count '^sophia_live_session_pointer schema=5 status=focus_handoff_released surface=')"
    echo "sophia_qemu_xmonad_pointer_focus schema=1 status=begin gesture=$gesture" |
        tee -a "$EVIDENCE_FILE"
    case "$gesture" in
        click)
            if ! position_left_content_pointer ||
                ! "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" 0 0 1 left; then
                echo "sophia_qemu_xmonad schema=1 status=failed reason=qmp_focus_click_send" |
                    tee -a "$EVIDENCE_FILE"
                return 1
            fi
            echo "sophia_qemu_xmonad_pointer schema=6 status=sent source=qmp device=virtio-mouse action=focus_click anchor=left_content x_inset=32 y_steps=8x16 clicks=1 commands=13" |
                tee -a "$EVIDENCE_FILE"
            ;;
        drag)
            if ! position_left_content_pointer ||
                ! "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" drag 0 0 96 24 left; then
                echo "sophia_qemu_xmonad schema=1 status=failed reason=qmp_focus_drag_send" |
                    tee -a "$EVIDENCE_FILE"
                return 1
            fi
            echo "sophia_qemu_xmonad_pointer schema=3 status=sent source=qmp device=virtio-mouse action=focus_drag anchor=left_content x_inset=32 y_steps=8x16 drag=96x24 commands=14" |
                tee -a "$EVIDENCE_FILE"
            ;;
        *)
            echo "sophia_qemu_xmonad schema=1 status=failed reason=invalid_pointer_focus_gesture gesture=$gesture" |
                tee -a "$EVIDENCE_FILE"
            return 1
            ;;
    esac
    echo "sophia_qemu_xmonad_pointer_focus schema=1 status=gesture_sent gesture=$gesture" |
        tee -a "$EVIDENCE_FILE"
    if ! wait_for_new_evidence '^sophia_live_wm schema=3 status=focus_requested source=pointer surface=' "$pointer_request_baseline"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=pointer_focus_request_timeout gesture=$gesture" |
            tee -a "$EVIDENCE_FILE"
        return 1
    fi
    if ! wait_for_new_evidence '^sophia_live_session_pointer schema=5 status=focus_handoff_released surface=' "$pointer_release_baseline"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=pointer_focus_release_timeout gesture=$gesture" |
            tee -a "$EVIDENCE_FILE"
        return 1
    fi

    pointer_key_baseline="$(evidence_count '^sophia_live_session_pointer schema=6 status=focused_key_routed surface=')"
    echo "sophia_qemu_xmonad_pointer_focus schema=1 status=key_probe_begin gesture=$gesture events=2" |
        tee -a "$EVIDENCE_FILE"
    if ! "$ROOT_DIR/tools/qemu_qmp_type.py" "$QMP_SOCKET" --no-return "$key"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=qmp_pointer_focus_key_send gesture=$gesture" |
            tee -a "$EVIDENCE_FILE"
        return 1
    fi
    echo "sophia_qemu_xmonad_pointer_focus schema=1 status=key_probe_sent gesture=$gesture events=2" |
        tee -a "$EVIDENCE_FILE"
    if ! wait_for_new_evidence '^sophia_live_session_pointer schema=6 status=focused_key_routed surface=' "$pointer_key_baseline"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=pointer_focus_key_timeout gesture=$gesture" |
            tee -a "$EVIDENCE_FILE"
        return 1
    fi
    echo "sophia_qemu_xmonad_pointer_focus schema=1 status=complete gesture=$gesture" |
        tee -a "$EVIDENCE_FILE"
}

probe_empty_workspace_pointer() {
    local focus_request_baseline
    local button_suppressed_baseline
    local focus_request_after

    focus_request_baseline="$(evidence_count '^sophia_live_wm schema=3 status=focus_requested source=pointer surface=')"
    button_suppressed_baseline="$(evidence_count '^sophia_live_session_pointer schema=8 status=button_suppressed reason=no_target count=[0-9][0-9]* total=[2-9][0-9]*$')"
    echo "sophia_qemu_xmonad_pointer schema=5 status=begin action=empty_workspace_click" |
        tee -a "$EVIDENCE_FILE"
    if ! "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" -32 0 1 left; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=qmp_empty_workspace_click_send" |
            tee -a "$EVIDENCE_FILE"
        return 1
    fi
    if ! wait_for_new_evidence '^sophia_live_session_pointer schema=8 status=button_suppressed reason=no_target count=[0-9][0-9]* total=[2-9][0-9]*$' "$button_suppressed_baseline"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=empty_workspace_click_suppression_timeout" |
            tee -a "$EVIDENCE_FILE"
        return 1
    fi
    sleep 0.25
    focus_request_after="$(evidence_count '^sophia_live_wm schema=3 status=focus_requested source=pointer surface=')"
    if ((focus_request_after != focus_request_baseline)); then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=hidden_surface_focus_request" |
            tee -a "$EVIDENCE_FILE"
        return 1
    fi
    echo "sophia_qemu_xmonad_pointer schema=5 status=passed action=empty_workspace_click focus_requests=0 routed_buttons=0" |
        tee -a "$EVIDENCE_FILE"
}

send_launch_and_wait() {
    local chord=$1
    local pattern=$2
    local label=$3
    local admission_baseline
    admission_baseline="$(evidence_count '^sophia_session_app schema=2 status=admitted source=action ')"
    send_chord_and_wait "$chord" "$pattern" "$label"
    # The launch action itself publishes layout and focus records before the
    # new surface exists. Only the session's stable admission record proves
    # that a following close or input action can target the launched client.
    if ! wait_for_new_evidence '^sophia_session_app schema=2 status=admitted source=action transaction=[0-9][0-9]* surface=[0-9][0-9]*$' "$admission_baseline" 800; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=action_admission_timeout action=$label chord=$chord" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
}

cycle_x11_focus_and_wait() {
    local phase=$1
    local focus_baseline
    focus_baseline="$(evidence_count '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$')"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+j
    echo "sophia_qemu_xmonad_input schema=1 status=sent chord=meta_l+j phase=$phase" |
        tee -a "$EVIDENCE_FILE"
    wait_for_new_evidence '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$' "$focus_baseline" 400
}

send_close_and_wait() {
    local app=$1
    local action_baseline
    local exit_baseline
    exit_baseline="$(evidence_count "^sophia_session_app schema=1 status=exited id=$app ")"
    local close_committed=false
    for _ in $(seq 1 4); do
        action_baseline="$(evidence_count '^sophia_live_wm schema=1 status=session_action_committed .* action=CloseFocused$')"
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+shift+c
        echo "sophia_qemu_xmonad_input schema=1 status=sent chord=meta_l+shift+c app=$app" | tee -a "$EVIDENCE_FILE"
        if wait_for_new_evidence '^sophia_live_wm schema=1 status=session_action_committed .* action=CloseFocused$' "$action_baseline" 80; then
            close_committed=true
            break
        fi
        sleep 1
    done
    if [[ "$close_committed" != true ]]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=application_close_timeout app=$app" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
    if ! wait_for_new_evidence "^sophia_session_app schema=1 status=exited id=$app " "$exit_baseline" 800; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=application_close_timeout app=$app" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
}

send_firefox_close_and_wait() {
    local exit_baseline
    local action_baseline
    exit_baseline="$(evidence_count '^sophia_session_app schema=1 status=exited id=firefox ')"
    for _ in $(seq 1 4); do
        action_baseline="$(evidence_count '^sophia_live_wm schema=1 status=session_action_committed .* action=CloseFocused$')"
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+shift+c
        echo "sophia_qemu_xmonad_input schema=1 status=sent chord=meta_l+shift+c app=firefox" | tee -a "$EVIDENCE_FILE"
        if wait_for_new_evidence '^sophia_live_wm schema=1 status=session_action_committed .* action=CloseFocused$' "$action_baseline" 80; then
            break
        fi
        sleep 1
    done

    # Firefox can expose more than one managed top-level window. Closing the
    # focused one may leave the browser process alive, so use its native quit
    # chord while cycling the remaining managed surfaces. Ctrl+Q exits
    # the browser process instead of closing only one of its top-levels.
    for _ in $(seq 1 8); do
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" ctrl+q
        echo "sophia_qemu_xmonad_input schema=1 status=sent chord=ctrl+q app=firefox" | tee -a "$EVIDENCE_FILE"
        if wait_for_new_evidence '^sophia_session_app schema=1 status=exited id=firefox ' "$exit_baseline" 80; then
            return 0
        fi
        cycle_x11_focus_and_wait firefox-close-refocus || true
    done
    echo "sophia_qemu_xmonad schema=1 status=failed reason=application_close_timeout app=firefox" | tee -a "$EVIDENCE_FILE"
    return 1
}

wait_for_firefox_stage() {
    local stage=$1
    if ! wait_for_new_evidence "^sophia_firefox_m8 schema=1 status=stage_complete stage=$stage " 0 800; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=firefox_stage_timeout stage=$stage" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
}

isolate_focused_interaction_surface() {
    local moved_projection_baseline
    local projection_baseline
    local focus_baseline
    moved_projection_baseline="$(evidence_count '^sophia_live_wm schema=2 status=workspace_projection_committed .* workspace=1 visible_surfaces=1 focus=none$')"
    send_chord_and_wait meta_l+shift+3 '^sophia_live_wm schema=1 status=physical_action_committed action=' interaction-isolate-move
    if ! wait_for_new_evidence '^sophia_live_wm schema=2 status=workspace_projection_committed .* workspace=1 visible_surfaces=1 focus=none$' "$moved_projection_baseline"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=interaction_isolation_move_timeout" |
            tee -a "$EVIDENCE_FILE"
        return 1
    fi

    projection_baseline="$(evidence_count '^sophia_live_wm schema=2 status=workspace_projection_committed .* workspace=3 visible_surfaces=1 focus=none$')"
    send_chord_and_wait meta_l+3 '^sophia_live_wm schema=1 status=physical_action_committed action=' interaction-isolate-view
    if ! wait_for_new_evidence '^sophia_live_wm schema=2 status=workspace_projection_committed .* workspace=3 visible_surfaces=1 focus=none$' "$projection_baseline"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=interaction_isolation_view_timeout" |
            tee -a "$EVIDENCE_FILE"
        return 1
    fi

    projection_baseline="$(evidence_count '^sophia_live_wm schema=2 status=workspace_projection_committed .* workspace=3 visible_surfaces=1 focus=surface$')"
    focus_baseline="$(evidence_count '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$')"
    send_chord_and_wait meta_l+j '^sophia_live_wm schema=1 status=physical_action_committed action=' interaction-isolate-focus
    if ! wait_for_new_evidence '^sophia_live_wm schema=2 status=workspace_projection_committed .* workspace=3 visible_surfaces=1 focus=surface$' "$projection_baseline"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=interaction_isolation_focus_projection_timeout" |
            tee -a "$EVIDENCE_FILE"
        return 1
    fi
    if ! wait_for_new_evidence '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$' "$focus_baseline" 400; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=interaction_isolation_focus_timeout" |
            tee -a "$EVIDENCE_FILE"
        return 1
    fi
    "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" -4096 -4096 0
    "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" 32 0 0
    for _ in $(seq 1 8); do
        "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" 0 16 0
    done
    echo "sophia_qemu_xmonad_pointer schema=1 status=positioned anchor=isolated_page x_inset=32 y_steps=8x16" |
        tee -a "$EVIDENCE_FILE"
}

restore_focused_interaction_surface() {
    local projection_baseline
    projection_baseline="$(evidence_count '^sophia_live_wm schema=2 status=workspace_projection_committed .* workspace=1 .* focus=')"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+shift+1
    echo "sophia_qemu_xmonad_input schema=1 status=sent chord=meta_l+shift+1 phase=interaction-restore" |
        tee -a "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+1
    echo "sophia_qemu_xmonad_input schema=1 status=sent chord=meta_l+1 phase=interaction-restore" |
        tee -a "$EVIDENCE_FILE"
    if ! wait_for_new_evidence '^sophia_live_wm schema=2 status=workspace_projection_committed .* workspace=1 .* focus=\(surface\|none\)$' "$projection_baseline"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=interaction_restore_timeout" |
            tee -a "$EVIDENCE_FILE"
        return 1
    fi
}

run_firefox_m8_interactions() {
    local page_focus_baseline
    local keyboard_complete=false
    local clipboard_owner_baseline
    local clipboard_complete=false
    local primary_complete=false
    local scroll_complete=false
    local axis_route_baseline
    local wheel_notch
    local resize_complete=false
    local refocus_complete=false
    local dialog_complete=false
    wait_for_new_evidence '^sophia_firefox_m8 schema=1 status=page_ready ' 0 800
    for _ in $(seq 1 10); do
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" ctrl+l
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" f6
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" ctrl+a
        "$ROOT_DIR/tools/qemu_qmp_type.py" "$QMP_SOCKET" --no-return sophia
        if wait_for_new_evidence '^sophia_firefox_m8 schema=1 status=stage_complete stage=keyboard ' 0 80; then
            keyboard_complete=true
            break
        fi
        cycle_x11_focus_and_wait firefox-input-refocus || true
    done
    if [[ "$keyboard_complete" != true ]]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=firefox_stage_timeout stage=keyboard" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
    wait_for_firefox_stage keyboard
    for _ in $(seq 1 10); do
        # Re-enter the document after any focus cycle, then wait until the
        # asynchronous CLIPBOARD owner change is observable before requesting
        # its value. This keeps the conversion ordered by protocol evidence.
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" ctrl+l
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" f6
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" ctrl+a
        clipboard_owner_baseline="$(evidence_count '^sophia_firefox_m8 schema=1 status=selection_observed kind=owner_change ')"
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" ctrl+c
        if ! wait_for_new_evidence '^sophia_firefox_m8 schema=1 status=selection_observed kind=owner_change ' "$clipboard_owner_baseline" 80; then
            cycle_x11_focus_and_wait firefox-clipboard-refocus || true
            continue
        fi
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" tab
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" ctrl+v
        if wait_for_new_evidence '^sophia_firefox_m8 schema=1 status=stage_complete stage=clipboard ' 0 80; then
            clipboard_complete=true
            break
        fi
        cycle_x11_focus_and_wait firefox-clipboard-refocus || true
    done
    if [[ "$clipboard_complete" != true ]]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=firefox_stage_timeout stage=clipboard" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
    wait_for_firefox_stage clipboard
    isolate_focused_interaction_surface
    for _ in $(seq 1 10); do
        # Shift+Insert consumes PRIMARY without depending on a stale tile count
        # or pointer coordinate. The browser can expose more than one top-level,
        # so failed attempts rotate focus and retry. Keep the bounded middle
        # click sweep as an independent native pointer fallback.
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" ctrl+l
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" f6
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" ctrl+a
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" shift+insert
        if wait_for_new_evidence '^sophia_firefox_m8 schema=1 status=stage_complete stage=primary ' 0 80; then
            primary_complete=true
            break
        fi
        "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" -4096 -4096 0 middle
        for _row in $(seq 1 4); do
            "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" -4096 160 0 middle
            for _column in $(seq 1 4); do
                "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" 320 0 1 middle
            done
        done
        if wait_for_new_evidence '^sophia_firefox_m8 schema=1 status=stage_complete stage=primary ' 0 80; then
            primary_complete=true
            break
        fi
        cycle_x11_focus_and_wait firefox-primary-refocus || true
    done
    if [[ "$primary_complete" != true ]]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=firefox_stage_timeout stage=primary" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
    wait_for_firefox_stage primary
    local navigation_ready_baseline
    navigation_ready_baseline="$(evidence_count '^sophia_firefox_m8 schema=1 status=navigation_ready ')"
    "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" 0 0 1 left
    echo "sophia_qemu_xmonad_input schema=1 status=sent pointer=left phase=firefox-navigation" | tee -a "$EVIDENCE_FILE"
    if ! wait_for_new_evidence '^sophia_firefox_m8 schema=1 status=navigation_ready ' "$navigation_ready_baseline" 80; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=firefox_navigation_ready_timeout" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
    for wheel_notch in 1 2; do
        axis_route_baseline="$(axis_route_count)"
        "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" 0 0 1 wheel-down
        if ! wait_for_axis_route_count "$axis_route_baseline" 80; then
            echo "sophia_qemu_xmonad schema=1 status=failed reason=firefox_axis_route_timeout notch=$wheel_notch" | tee -a "$EVIDENCE_FILE"
            return 1
        fi
    done
    if wait_for_new_evidence '^sophia_firefox_m8 schema=1 status=stage_complete stage=scroll ' 0 80; then
        scroll_complete=true
    fi
    if [[ "$scroll_complete" != true ]]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=firefox_stage_timeout stage=scroll" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
    wait_for_firefox_stage scroll
    echo "sophia_qemu_firefox_m8 schema=4 status=scroll_complete source=wheel axis_routes=2 keyboard_fallback=false" | tee -a "$EVIDENCE_FILE"
    restore_focused_interaction_surface
    for _ in $(seq 1 10); do
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+spc
        echo "sophia_qemu_xmonad_input schema=1 status=sent chord=meta_l+spc phase=firefox-resize" | tee -a "$EVIDENCE_FILE"
        if wait_for_new_evidence '^sophia_firefox_m8 schema=1 status=stage_complete stage=resize ' 0 80; then
            resize_complete=true
            break
        fi
        cycle_x11_focus_and_wait firefox-resize-refocus || true
    done
    if [[ "$resize_complete" != true ]]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=firefox_stage_timeout stage=resize" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
    wait_for_firefox_stage resize
    for _ in $(seq 1 10); do
        page_focus_baseline="$(evidence_count '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$')"
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+j
        echo "sophia_qemu_xmonad_input schema=1 status=sent chord=meta_l+j phase=firefox-refocus-cycle" |
            tee -a "$EVIDENCE_FILE"
        if ! wait_for_new_evidence '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$' "$page_focus_baseline" 400; then
            continue
        fi
        if wait_for_new_evidence '^sophia_firefox_m8 schema=1 status=stage_complete stage=refocus ' 0 40; then
            refocus_complete=true
            break
        fi
    done
    if [[ "$refocus_complete" != true ]]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=firefox_stage_timeout stage=refocus" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
    wait_for_firefox_stage refocus
    isolate_focused_interaction_surface
    dialog_stage_baseline="$(evidence_count '^sophia_firefox_m8 schema=1 status=stage_complete stage=dialog ')"
    for _ in $(seq 1 10); do
        popup_ready_baseline="$(evidence_count '^sophia_firefox_m8 schema=1 status=dialog_ready content=redacted$')"
        "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" 0 0 1 left
        if ! wait_for_new_evidence '^sophia_firefox_m8 schema=1 status=dialog_ready content=redacted$' "$popup_ready_baseline" 80; then
            cycle_x11_focus_and_wait firefox-dialog-refocus || true
            continue
        fi
        echo "sophia_qemu_firefox_m8 schema=7 status=dialog_open surface_snapshot=false modality=dom" | tee -a "$EVIDENCE_FILE"
        if ! position_left_dialog_confirmation_pointer ||
            ! "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" 0 0 1 left; then
            echo "sophia_qemu_xmonad schema=1 status=failed reason=firefox_dialog_confirmation_send" |
                tee -a "$EVIDENCE_FILE"
            return 1
        fi
        echo "sophia_qemu_xmonad_input schema=1 status=sent pointer=left phase=firefox-dialog-confirmation" | tee -a "$EVIDENCE_FILE"
        if wait_for_new_evidence '^sophia_firefox_m8 schema=1 status=stage_complete stage=dialog ' "$dialog_stage_baseline" 800; then
            dialog_complete=true
            break
        fi
        echo "sophia_qemu_xmonad schema=1 status=failed reason=firefox_dialog_confirmation_timeout" | tee -a "$EVIDENCE_FILE"
        return 1
    done
    if [[ "$dialog_complete" != true ]]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=firefox_stage_timeout stage=dialog" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
    wait_for_firefox_stage dialog
    echo "sophia_qemu_firefox_m8 schema=7 status=dialog_closed surface_snapshot=false modality=dom" | tee -a "$EVIDENCE_FILE"
    echo "sophia_qemu_firefox_m8 schema=3 status=interactions_complete keyboard=true clipboard=true primary=true navigation=true scroll=true resize=true refocus=true pointer=true dialog=true" | tee -a "$EVIDENCE_FILE"
}

cleanup() {
    if [[ -n "$QEMU_PID" ]] && kill -0 "$QEMU_PID" 2>/dev/null; then
        kill "$QEMU_PID" 2>/dev/null || true
        wait "$QEMU_PID" 2>/dev/null || true
    fi
    if [[ -n "$LOGGER_PID" ]] && kill -0 "$LOGGER_PID" 2>/dev/null; then
        kill "$LOGGER_PID" 2>/dev/null || true
        wait "$LOGGER_PID" 2>/dev/null || true
    fi
    if [[ -n "$TRACE_LOGGER_PID" ]] && kill -0 "$TRACE_LOGGER_PID" 2>/dev/null; then
        kill "$TRACE_LOGGER_PID" 2>/dev/null || true
        wait "$TRACE_LOGGER_PID" 2>/dev/null || true
    fi
    if [[ -n "$VIEWER_PID" ]] && kill -0 "$VIEWER_PID" 2>/dev/null; then
        kill "$VIEWER_PID" 2>/dev/null || true
        wait "$VIEWER_PID" 2>/dev/null || true
    fi
    rm -f "$VNC_SOCKET" "$QMP_SOCKET" "$SERIAL_FIFO" "$INTERACTIVE_TRACE_FIFO"
}
trap cleanup EXIT

if ! command -v "$QEMU_BIN" >/dev/null 2>&1; then
    echo "missing qemu-system-x86_64; on Void install it with:" >&2
    echo "  sudo xbps-install -S qemu-system-amd64" >&2
    exit 1
fi
if ! command -v python3 >/dev/null 2>&1; then
    echo "missing python3; on Void install it with:" >&2
    echo "  sudo xbps-install -S python3" >&2
    exit 1
fi
if [[ "$SCENARIO" == "xmonad-interactive" ]]; then
    if [[ "$INTERACTIVE_VIEWER" != auto && "$INTERACTIVE_VIEWER" != none ]]; then
        echo "SOPHIA_QEMU_INTERACTIVE_VIEWER must be auto or none" >&2
        exit 1
    fi
    if [[ "$INTERACTIVE_VIEWER" == auto ]]; then
        command -v vncviewer >/dev/null 2>&1 || {
            echo "xmonad-interactive requires vncviewer or SOPHIA_QEMU_INTERACTIVE_VIEWER=none" >&2
            exit 1
        }
        if [[ -z "${DISPLAY:-}" && -z "${WAYLAND_DISPLAY:-}" ]]; then
            echo "automatic xmonad-interactive viewing requires a graphical session" >&2
            echo "set SOPHIA_QEMU_INTERACTIVE_VIEWER=none to attach from another session" >&2
            exit 1
        fi
    fi
fi
if [[ ! -r "$KERNEL_IMAGE" ]]; then
    echo "guest kernel is not readable: $KERNEL_IMAGE" >&2
    exit 1
fi
if [[ ! -r "$INITRAMFS" ]]; then
    echo "guest initramfs is not readable: $INITRAMFS" >&2
    echo "build it first with tools/build_qemu_session_initramfs.sh" >&2
    exit 1
fi
if [[ ! "$MEMORY_MIB" =~ ^[0-9]+$ ]] || (( MEMORY_MIB < 512 || MEMORY_MIB > 16384 )); then
    echo "SOPHIA_QEMU_MEMORY_MIB must be from 512 through 16384" >&2
    exit 1
fi
if [[ ! "$VIRTUAL_CPUS" =~ ^[0-9]+$ ]] || (( VIRTUAL_CPUS < 1 || VIRTUAL_CPUS > 16 )); then
    echo "SOPHIA_QEMU_CPUS must be from 1 through 16" >&2
    exit 1
fi
if [[ "$GPU_MODE" == virgl && ! -c "$RENDER_NODE" ]]; then
    echo "Virgl QEMU mode requires a DRM render node: $RENDER_NODE" >&2
    exit 1
fi

if [[ "$GPU_MODE" == virgl ]]; then
    # Virgl keeps guest producers and Sophia's renderer on one explicit host
    # render node; software QEMU remains the default.
    display_args=(-display "egl-headless,rendernode=$RENDER_NODE")
    gpu_args=(
        -device virtio-vga-gl,max_outputs=1
        -device virtio-gpu-pci,max_outputs=1
    )
else
    display_args=(-display none -vnc "unix:$VNC_SOCKET")
    gpu_args=(
        -device virtio-vga,max_outputs=1
        -device virtio-gpu-pci,max_outputs=1
    )
fi

machine="q35,accel=kvm:tcg"
if [[ "$SCENARIO" == "xmonad-interactive" ]]; then
    # Keep the interactive viewer on the declared virtio mouse. Q35's optional
    # vmmouse otherwise wins QEMU's pointer selection without a guest consumer.
    machine+=",vmport=off"
fi

mkdir -p "$(dirname "$EVIDENCE_FILE")"
: > "$EVIDENCE_FILE"
rm -f "$VNC_SOCKET" "$QMP_SOCKET" "$SERIAL_FIFO" "$INTERACTIVE_TRACE_FIFO"
mkfifo "$SERIAL_FIFO"
trace_args=()
if [[ "$SCENARIO" == "xmonad-interactive" ]]; then
    mkfifo "$INTERACTIVE_TRACE_FIFO"
    "$ROOT_DIR/tools/reduce_qemu_interactive_trace.sh" \
        "$INTERACTIVE_TRACE_FIFO" "$EVIDENCE_FILE" &
    TRACE_LOGGER_PID=$!
    trace_args=(
        -trace
        "events=$ROOT_DIR/tools/config/qemu_interactive_trace_events,file=$INTERACTIVE_TRACE_FIFO"
    )
fi

if [[ "$SCENARIO" == "emergency-recovery" ]]; then
    echo "sophia_qemu_recovery schema=1 status=starting isolation=headless control=qmp-unix host_drm=none host_vt=none keyboard=virtio chord=ctrl-alt-backspace" | tee -a "$EVIDENCE_FILE"
elif [[ "$SCENARIO" == gtk-* ]]; then
    echo "sophia_qemu_gtk schema=1 status=starting isolation=headless control=qmp-unix host_drm=none host_vt=none keyboard=virtio mouse=virtio scenario=$SCENARIO" | tee -a "$EVIDENCE_FILE"
elif [[ "$SCENARIO" == "xmonad-interactive" ]]; then
    echo "sophia_qemu_interactive schema=2 status=starting isolation=manual display_backend=vnc-unix control=qmp-unix pointer=virtio-relative vmport=off host_drm=none host_vt=none guest_network=none storage=none proof_watchdog=off fault_injection=off" | tee -a "$EVIDENCE_FILE"
elif [[ "$SCENARIO" == xmonad-* ]]; then
    if [[ "$SCENARIO" == "xmonad-idle-efficiency" || "$SCENARIO" == "xmonad-producer-overload" ]]; then
        echo "sophia_qemu_xmonad schema=2 status=starting isolation=headless control=qmp-unix profile=xmonad windows=2 gpu_mode=virgl host_render_node=explicit" | tee -a "$EVIDENCE_FILE"
    elif [[ "$GPU_MODE" == virgl ]]; then
        echo "sophia_qemu_xmonad schema=2 status=starting isolation=headless control=qmp-unix profile=xmonad windows=3 gpu_mode=virgl host_render_node=explicit" | tee -a "$EVIDENCE_FILE"
    else
        echo "sophia_qemu_xmonad schema=1 status=starting isolation=headless control=qmp-unix profile=xmonad windows=2" | tee -a "$EVIDENCE_FILE"
    fi
else
    echo "sophia_qemu_session schema=3 status=starting isolation=headless display_sink=vnc-unix control=qmp-unix host_drm=none host_vt=none guest_network=none storage=none gpu=virtio-gpu gpu_devices=2 gpu_heads=2 keyboard=virtio mouse=virtio ticks=300" | tee -a "$EVIDENCE_FILE"
fi

while IFS= read -r line || [[ -n "$line" ]]; do
    printf '%s\n' "${line%$'\r'}"
done < "$SERIAL_FIFO" | tee -a "$EVIDENCE_FILE" &
LOGGER_PID=$!

"$QEMU_BIN" \
    -machine "$machine" \
    -smp "$VIRTUAL_CPUS" \
    -m "$MEMORY_MIB" \
    -nodefaults \
    -no-reboot \
    "${display_args[@]}" \
    -monitor none \
    -qmp "unix:$QMP_SOCKET,server=on,wait=off" \
    "${trace_args[@]}" \
    -serial stdio \
    "${gpu_args[@]}" \
    -device virtio-keyboard-pci \
    -device virtio-mouse-pci \
    -kernel "$KERNEL_IMAGE" \
    -initrd "$INITRAMFS" \
    -append "console=ttyS0 quiet loglevel=3 rdinit=/sbin/sophia-qemu-init rd.driver.pre=virtio_pci rd.driver.pre=virtio_gpu rd.driver.pre=virtio_input panic=-1 sophia.scenario=$SCENARIO sophia.two_xterm=$TWO_XTERM" \
    > "$SERIAL_FIFO" 2>&1 &
QEMU_PID=$!

if [[ "$SCENARIO" == "xmonad-interactive" ]]; then
    if [[ "$INTERACTIVE_VIEWER" == auto ]]; then
        while [[ ! -S "$VNC_SOCKET" ]]; do
            kill -0 "$QEMU_PID" 2>/dev/null || break
            sleep 0.05
        done
        [[ -S "$VNC_SOCKET" ]] || {
            echo "sophia_qemu_interactive schema=1 status=failed reason=display_socket_missing" |
                tee -a "$EVIDENCE_FILE"
            exit 1
        }
        vncviewer "$VNC_SOCKET" >/dev/null 2>&1 &
        VIEWER_PID=$!
        echo "sophia_qemu_interactive schema=1 status=viewer_started program=vncviewer" |
            tee -a "$EVIDENCE_FILE"
    else
        echo "sophia_qemu_interactive schema=1 status=viewer_waiting socket=$VNC_SOCKET" |
            tee -a "$EVIDENCE_FILE"
        echo "Attach with: vncviewer $VNC_SOCKET"
    fi

    interactive_ready=false
    while kill -0 "$QEMU_PID" 2>/dev/null; do
        if grep -q '^sophia_live_wm schema=1 status=ready ' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_input_pipeline schema=1 status=focus_ready$' "$EVIDENCE_FILE" \
            && grep -Eq '^sophia_live_wm schema=2 status=workspace_projection_committed .*visible_surfaces=1 focus=surface$' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_startup schema=2 status=output_baseline_ready outputs=2/2$' "$EVIDENCE_FILE"; then
            interactive_ready=true
            break
        fi
        sleep 0.05
    done
    if [[ "$interactive_ready" == true ]]; then
        echo "sophia_qemu_interactive schema=1 status=ready actions=freeform shutdown=meta_l+shift+q" |
            tee -a "$EVIDENCE_FILE"
        echo "Interactive Sophia is ready; use it freely, then log out with Super+Shift+Q."
    fi

    set +e
    wait "$QEMU_PID"
    qemu_status=$?
    QEMU_PID=""
    wait "$LOGGER_PID"
    logger_status=$?
    LOGGER_PID=""
    wait "$TRACE_LOGGER_PID"
    trace_status=$?
    TRACE_LOGGER_PID=""
    if [[ -n "$VIEWER_PID" ]]; then
        kill "$VIEWER_PID" 2>/dev/null || true
        wait "$VIEWER_PID" 2>/dev/null
    fi
    VIEWER_PID=""
    set -e
    cleanup
    if [[ "$interactive_ready" != true || "$qemu_status" -ne 0 \
        || "$logger_status" -ne 0 || "$trace_status" -ne 0 ]]; then
        echo "sophia_qemu_interactive schema=1 status=failed reason=guest_exit ready=$interactive_ready qemu_exit=$qemu_status logger_exit=$logger_status trace_exit=$trace_status" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    "$ROOT_DIR/tools/verify_qemu_xmonad_interactive_evidence.sh" "$EVIDENCE_FILE"
    echo "sophia_qemu_interactive schema=1 status=complete qemu_exit=0" |
        tee -a "$EVIDENCE_FILE"
    exit 0
fi

if [[ "$SCENARIO" == "emergency-recovery" ]]; then
    guard_ready=false
    for _ in $(seq 1 600); do
        if grep -q '^sophia_session_input_guard schema=1 status=ready ' "$EVIDENCE_FILE"; then
            guard_ready=true
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then
            break
        fi
        sleep 0.05
    done
    if [[ "$guard_ready" != true ]]; then
        echo "sophia_qemu_recovery schema=1 status=failed reason=input_guard_readiness_timeout" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi

    if ! "$ROOT_DIR/tools/qemu_qmp_emergency_chord.py" "$QMP_SOCKET"; then
        echo "sophia_qemu_recovery schema=1 status=failed reason=qmp_arm_input_send" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    echo "sophia_qemu_recovery_input schema=1 status=sent phase=arm source=qmp device=virtio-keyboard chord=ctrl-alt-backspace events=6" | tee -a "$EVIDENCE_FILE"

    recovery_ready=false
    for _ in $(seq 1 600); do
        if grep -q '^sophia_session_input_guard schema=1 status=armed$' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_input_pipeline schema=2 status=poller_ready ' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_input_pipeline schema=1 status=focus_ready$' "$EVIDENCE_FILE"; then
            recovery_ready=true
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then
            break
        fi
        sleep 0.05
    done
    if [[ "$recovery_ready" != true ]]; then
        echo "sophia_qemu_recovery schema=1 status=failed reason=armed_session_readiness_timeout" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi

    if ! "$ROOT_DIR/tools/qemu_qmp_emergency_chord.py" "$QMP_SOCKET"; then
        echo "sophia_qemu_recovery schema=1 status=failed reason=qmp_trigger_input_send" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    echo "sophia_qemu_recovery_input schema=1 status=sent phase=trigger source=qmp device=virtio-keyboard chord=ctrl-alt-backspace events=6" | tee -a "$EVIDENCE_FILE"

    set +e
    wait "$QEMU_PID"
    qemu_status=$?
    QEMU_PID=""
    wait "$LOGGER_PID"
    logger_status=$?
    LOGGER_PID=""
    set -e
    cleanup

    if [[ "$qemu_status" -ne 0 ]]; then
        echo "sophia_qemu_recovery schema=1 status=failed qemu_exit=$qemu_status" | tee -a "$EVIDENCE_FILE"
        exit "$qemu_status"
    fi
    if [[ "$logger_status" -ne 0 ]]; then
        echo "sophia_qemu_recovery schema=1 status=failed serial_logger_exit=$logger_status" | tee -a "$EVIDENCE_FILE"
        exit "$logger_status"
    fi

    echo "sophia_qemu_recovery schema=1 status=complete qemu_exit=0" | tee -a "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/verify_qemu_emergency_recovery_evidence.sh" "$EVIDENCE_FILE"
    exit 0
fi

if [[ "$SCENARIO" == "xmonad-producer-overload" ]]; then
    ready=false
    for _ in $(seq 1 1600); do
        if grep -q '^sophia_live_wm schema=1 status=ready ' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_input_pipeline schema=1 status=focus_ready$' "$EVIDENCE_FILE" \
            && grep -Eq '^sophia_live_wm schema=2 status=workspace_projection_committed .*visible_surfaces=1 focus=surface$' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_startup schema=2 status=output_baseline_ready outputs=2/2$' "$EVIDENCE_FILE"; then
            ready=true
            break
        fi
        if grep -q '^sophia_live_wm schema=1 status=layout_timeout ' "$EVIDENCE_FILE" \
            || grep -Eq '^sophia_session_app schema=1 status=exited id=(cpu|gpu) ' "$EVIDENCE_FILE"; then
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$ready" != true ]]; then
        echo "sophia_qemu_producer_overload schema=1 status=failed reason=readiness_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi

    started_baseline="$(evidence_count '^sophia_session_app schema=2 status=started id=gpu source=action ')"
    admitted_baseline="$(evidence_count '^sophia_session_app schema=2 status=admitted source=action ')"
    echo "sophia_qemu_producer_overload schema=1 status=launch_begin chord=meta_l+p app=gpu" |
        tee -a "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+p
    if ! wait_for_new_evidence \
        '^sophia_session_app schema=2 status=started id=gpu source=action ' \
        "$started_baseline" 400 \
        || ! wait_for_new_evidence \
            '^sophia_session_app schema=2 status=admitted source=action ' \
            "$admitted_baseline" 1600; then
        echo "sophia_qemu_producer_overload schema=1 status=failed reason=producer_admission_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi

    overloaded=false
    for _ in $(seq 1 1600); do
        copies="$(present_progress_field complete_copy)"
        skips="$(present_progress_field complete_skip)"
        if grep -Eq '^sophia_live_wm schema=2 status=workspace_projection_committed .*visible_surfaces=2 focus=surface$' "$EVIDENCE_FILE" \
            && (( copies >= 20 && skips >= 1 )); then
            overloaded=true
            break
        fi
        if grep -q '^sophia_live_wm schema=1 status=layout_timeout ' "$EVIDENCE_FILE" \
            || grep -Eq '^sophia_session_app schema=1 status=exited id=(cpu|gpu) ' "$EVIDENCE_FILE"; then
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$overloaded" != true ]]; then
        echo "sophia_qemu_producer_overload schema=1 status=failed reason=overload_not_observed copies=${copies:-0} skips=${skips:-0}" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    echo "sophia_qemu_producer_overload schema=1 status=warmup_complete copies=$copies skips=$skips" |
        tee -a "$EVIDENCE_FILE"

    copy_baseline=$copies
    skip_baseline=$skips
    submit_baseline="$(evidence_count 'sophia_live_native_page_flip schema=1 status=submitted output=1 ')"
    retire_baseline="$(evidence_count 'sophia_live_native_page_flip schema=1 status=retired output=1 ')"
    echo "sophia_qemu_producer_overload schema=1 status=window_started duration_msec=10000 phases=2" |
        tee -a "$EVIDENCE_FILE"
    for phase in 1 2; do
        sleep 5
        current_copies="$(present_progress_field complete_copy)"
        current_skips="$(present_progress_field complete_skip)"
        current_submits="$(evidence_count 'sophia_live_native_page_flip schema=1 status=submitted output=1 ')"
        current_retires="$(evidence_count 'sophia_live_native_page_flip schema=1 status=retired output=1 ')"
        phase_copies=$((current_copies - copy_baseline))
        phase_skips=$((current_skips - skip_baseline))
        phase_submits=$((current_submits - submit_baseline))
        phase_retires=$((current_retires - retire_baseline))
        echo "sophia_qemu_producer_overload schema=1 status=phase_complete phase=$phase duration_msec=5000 copies=$phase_copies skips=$phase_skips submissions=$phase_submits retirements=$phase_retires" |
            tee -a "$EVIDENCE_FILE"
        if (( phase_copies < 20 || phase_skips < 1 \
            || phase_submits < 20 || phase_retires < 20 \
            || phase_retires > phase_submits + 1 \
            || phase_submits > phase_retires + 1 )); then
            echo "sophia_qemu_producer_overload schema=1 status=failed reason=phase_discipline phase=$phase" |
                tee -a "$EVIDENCE_FILE"
            exit 1
        fi
        copy_baseline=$current_copies
        skip_baseline=$current_skips
        submit_baseline=$current_submits
        retire_baseline=$current_retires
    done
    echo "sophia_qemu_producer_overload schema=1 status=window_complete duration_msec=10000 phases=2" |
        tee -a "$EVIDENCE_FILE"

    echo "sophia_qemu_producer_overload schema=1 status=logout_begin chord=meta_l+shift+q" |
        tee -a "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+shift+q
    echo "sophia_qemu_producer_overload schema=1 status=logout_sent chord=meta_l+shift+q" |
        tee -a "$EVIDENCE_FILE"

    set +e
    wait "$QEMU_PID"
    qemu_status=$?
    QEMU_PID=""
    wait "$LOGGER_PID"
    logger_status=$?
    LOGGER_PID=""
    set -e
    cleanup
    if [[ "$qemu_status" -ne 0 || "$logger_status" -ne 0 ]]; then
        echo "sophia_qemu_producer_overload schema=1 status=failed reason=guest_exit qemu_exit=$qemu_status logger_exit=$logger_status" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    "$ROOT_DIR/tools/verify_qemu_xmonad_producer_overload_evidence.sh" "$EVIDENCE_FILE"
    echo "sophia_qemu_xmonad schema=1 status=complete qemu_exit=0" |
        tee -a "$EVIDENCE_FILE"
    exit 0
fi

if [[ "$SCENARIO" == "xmonad-idle-efficiency" ]]; then
    ready=false
    for _ in $(seq 1 1600); do
        if grep -q '^sophia_live_wm schema=1 status=ready ' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_input_pipeline schema=1 status=focus_ready$' "$EVIDENCE_FILE" \
            && grep -Eq '^sophia_live_wm schema=2 status=workspace_projection_committed .*visible_surfaces=1 focus=surface$' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_startup schema=2 status=output_baseline_ready outputs=2/2$' "$EVIDENCE_FILE"; then
            ready=true
            break
        fi
        if grep -q '^sophia_live_wm schema=1 status=layout_timeout ' "$EVIDENCE_FILE" \
            || grep -Eq '^sophia_session_app schema=1 status=exited id=(cpu|gpu) ' "$EVIDENCE_FILE"; then
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$ready" != true ]]; then
        echo "sophia_qemu_idle_efficiency schema=1 status=failed reason=readiness_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi

    started_baseline="$(evidence_count '^sophia_session_app schema=2 status=started id=gpu source=action ')"
    admitted_baseline="$(evidence_count '^sophia_session_app schema=2 status=admitted source=action ')"
    echo "sophia_qemu_idle_efficiency schema=1 status=launch_begin chord=meta_l+p app=gpu" |
        tee -a "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+p
    if ! wait_for_new_evidence \
        '^sophia_session_app schema=2 status=started id=gpu source=action ' \
        "$started_baseline" 400 \
        || ! wait_for_new_evidence \
            '^sophia_session_app schema=2 status=admitted source=action ' \
            "$admitted_baseline" 1600; then
        echo "sophia_qemu_idle_efficiency schema=1 status=failed reason=producer_admission_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi

    frozen=false
    for _ in $(seq 1 1600); do
        retirements="$(evidence_count '^sophia_live_session_present schema=2 status=retired ')"
        if grep -Eq '^sophia_live_wm schema=2 status=workspace_projection_committed .*visible_surfaces=2 focus=surface$' "$EVIDENCE_FILE" \
            && grep -q '^sophia_qemu_idle_client schema=1 status=frozen producer=glxgears$' "$EVIDENCE_FILE" \
            && (( retirements >= 10 )) \
            && (( $(dmabuf_retirement_surface_count) == 1 )); then
            frozen=true
            break
        fi
        if grep -q '^sophia_live_wm schema=1 status=layout_timeout ' "$EVIDENCE_FILE" \
            || grep -Eq '^sophia_session_app schema=1 status=exited id=(cpu|gpu) ' "$EVIDENCE_FILE" \
            || grep -q '^sophia_qemu_idle_client schema=1 status=failed ' "$EVIDENCE_FILE"; then
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$frozen" != true ]]; then
        echo "sophia_qemu_idle_efficiency schema=1 status=failed reason=producer_freeze_timeout retirements=${retirements:-0}" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi

    # A frozen producer can leave already-admitted frames in the owner queue.
    # Begin the reuse phase only after that queue has stopped advancing.
    stable_ticks=0
    previous_retirements=-1
    while (( stable_ticks < 20 )); do
        retirements="$(evidence_count '^sophia_live_session_present schema=2 status=retired ')"
        if (( retirements == previous_retirements )); then
            stable_ticks=$((stable_ticks + 1))
        else
            previous_retirements=$retirements
            stable_ticks=0
        fi
        if grep -q '^sophia_live_wm schema=1 status=layout_timeout ' "$EVIDENCE_FILE" \
            || grep -Eq '^sophia_session_app schema=1 status=exited id=(cpu|gpu) ' "$EVIDENCE_FILE" \
            || ! kill -0 "$QEMU_PID" 2>/dev/null; then
            echo "sophia_qemu_idle_efficiency schema=1 status=failed reason=producer_quiescence_interrupted" |
                tee -a "$EVIDENCE_FILE"
            exit 1
        fi
        sleep 0.05
    done
    producer_retirements=$retirements
    echo "sophia_qemu_idle_efficiency schema=1 status=producer_quiescent surfaces=1 retirements=$producer_retirements stable_msec=1000" |
        tee -a "$EVIDENCE_FILE"

    focus_transitions="${SOPHIA_QEMU_IDLE_FOCUS_TRANSITIONS:-256}"
    if [[ ! "$focus_transitions" =~ ^[0-9]+$ ]] \
        || (( focus_transitions < 1 || focus_transitions > 512 )); then
        echo "SOPHIA_QEMU_IDLE_FOCUS_TRANSITIONS must be an integer from 1 through 512" >&2
        exit 1
    fi
    action_baseline="$(evidence_count '^sophia_live_wm schema=1 status=physical_action_committed action=')"
    repaint_baseline="$(evidence_count 'sophia_live_output_repaint schema=1 status=presented output=1 ')"
    partial_baseline="$(evidence_count 'sophia_live_output_repaint schema=1 status=presented output=1 mode=partial ')"
    flip_baseline="$(evidence_count 'sophia_live_native_page_flip schema=1 status=retired output=1 ')"
    echo "sophia_qemu_idle_efficiency schema=1 status=reuse_window_started focus_transitions=$focus_transitions" |
        tee -a "$EVIDENCE_FILE"
    for transition in $(seq 1 "$focus_transitions"); do
        action="$(evidence_count '^sophia_live_wm schema=1 status=physical_action_committed action=')"
        partial="$(evidence_count 'sophia_live_output_repaint schema=1 status=presented output=1 mode=partial ')"
        flip="$(evidence_count 'sophia_live_native_page_flip schema=1 status=retired output=1 ')"
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+j
        if ! wait_for_new_evidence \
            '^sophia_live_wm schema=1 status=physical_action_committed action=' \
            "$action" 400 \
            || ! wait_for_new_evidence \
                'sophia_live_output_repaint schema=1 status=presented output=1 mode=partial ' \
                "$partial" 400 \
            || ! wait_for_new_evidence \
                'sophia_live_native_page_flip schema=1 status=retired output=1 ' \
                "$flip" 400; then
            echo "sophia_qemu_idle_efficiency schema=1 status=failed reason=retained_repaint_timeout transition=$transition" |
                tee -a "$EVIDENCE_FILE"
            exit 1
        fi
    done
    actions=$(( $(evidence_count '^sophia_live_wm schema=1 status=physical_action_committed action=') - action_baseline ))
    repaints=$(( $(evidence_count 'sophia_live_output_repaint schema=1 status=presented output=1 ') - repaint_baseline ))
    partial_repaints=$(( $(evidence_count 'sophia_live_output_repaint schema=1 status=presented output=1 mode=partial ') - partial_baseline ))
    flips=$(( $(evidence_count 'sophia_live_native_page_flip schema=1 status=retired output=1 ') - flip_baseline ))
    current_retirements="$(evidence_count '^sophia_live_session_present schema=2 status=retired ')"
    if (( actions != focus_transitions \
        || partial_repaints < focus_transitions \
        || repaints != partial_repaints \
        || flips < focus_transitions \
        || current_retirements != producer_retirements )); then
        echo "sophia_qemu_idle_efficiency schema=1 status=failed reason=reuse_window_accounting actions=$actions repaints=$repaints partial_repaints=$partial_repaints flips=$flips producer_before=$producer_retirements producer_after=$current_retirements" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    echo "sophia_qemu_idle_efficiency schema=1 status=reuse_window_complete focus_transitions=$focus_transitions actions=$actions repaints=$repaints partial_repaints=$partial_repaints flips=$flips producer_retirements=$current_retirements" |
        tee -a "$EVIDENCE_FILE"

    idle_repaint_baseline="$(evidence_count 'sophia_live_output_repaint schema=1 status=presented output=')"
    idle_flip_baseline="$(evidence_count 'sophia_live_native_page_flip schema=1 status=retired output=')"
    idle_present_baseline="$(evidence_count '^sophia_live_session_present schema=2 status=retired ')"
    echo "sophia_qemu_idle_efficiency schema=1 status=idle_window_started duration_msec=2000" |
        tee -a "$EVIDENCE_FILE"
    sleep 2
    idle_repaints=$(( $(evidence_count 'sophia_live_output_repaint schema=1 status=presented output=') - idle_repaint_baseline ))
    idle_flips=$(( $(evidence_count 'sophia_live_native_page_flip schema=1 status=retired output=') - idle_flip_baseline ))
    idle_presents=$(( $(evidence_count '^sophia_live_session_present schema=2 status=retired ') - idle_present_baseline ))
    echo "sophia_qemu_idle_efficiency schema=1 status=idle_window_complete duration_msec=2000 repaints=$idle_repaints page_flips=$idle_flips client_presents=$idle_presents" |
        tee -a "$EVIDENCE_FILE"
    if (( idle_repaints != 0 || idle_flips != 0 || idle_presents != 0 )); then
        echo "sophia_qemu_idle_efficiency schema=1 status=failed reason=idle_work_observed repaints=$idle_repaints page_flips=$idle_flips client_presents=$idle_presents" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi

    echo "sophia_qemu_idle_efficiency schema=1 status=logout_begin chord=meta_l+shift+q" |
        tee -a "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+shift+q
    echo "sophia_qemu_idle_efficiency schema=1 status=logout_sent chord=meta_l+shift+q" |
        tee -a "$EVIDENCE_FILE"

    set +e
    wait "$QEMU_PID"
    qemu_status=$?
    QEMU_PID=""
    wait "$LOGGER_PID"
    logger_status=$?
    LOGGER_PID=""
    set -e
    cleanup
    if [[ "$qemu_status" -ne 0 || "$logger_status" -ne 0 ]]; then
        echo "sophia_qemu_idle_efficiency schema=1 status=failed reason=guest_exit qemu_exit=$qemu_status logger_exit=$logger_status" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    "$ROOT_DIR/tools/verify_qemu_xmonad_idle_efficiency_evidence.sh" "$EVIDENCE_FILE"
    echo "sophia_qemu_xmonad schema=1 status=complete qemu_exit=0" |
        tee -a "$EVIDENCE_FILE"
    exit 0
fi

if [[ "$SCENARIO" == "xmonad-render-contention" ]]; then
    ready=false
    for _ in $(seq 1 1600); do
        if grep -q '^sophia_live_wm schema=1 status=ready ' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_input_pipeline schema=1 status=focus_ready$' "$EVIDENCE_FILE" \
            && grep -Eq '^sophia_live_wm schema=2 status=workspace_projection_committed .*visible_surfaces=1 focus=surface$' "$EVIDENCE_FILE" \
            && grep -Eq '^sophia_live_work_area schema=1 status=reduced outputs=2 .*active_reservations=1$' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_startup schema=2 status=output_baseline_ready outputs=2/2$' "$EVIDENCE_FILE"; then
            ready=true
            break
        fi
        if grep -q '^sophia_live_wm schema=1 status=layout_timeout ' "$EVIDENCE_FILE"; then
            echo "sophia_qemu_render_contention schema=1 status=failed reason=startup_layout_timeout" |
                tee -a "$EVIDENCE_FILE"
            exit 1
        fi
        if grep -Eq '^sophia_session_app schema=1 status=exited id=(gpu1|gpu2|gpu3|statusbar) ' "$EVIDENCE_FILE"; then
            echo "sophia_qemu_render_contention schema=1 status=failed reason=startup_application_exit" |
                tee -a "$EVIDENCE_FILE"
            exit 1
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$ready" != true ]]; then
        echo "sophia_qemu_render_contention schema=1 status=failed reason=readiness_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    echo "sophia_qemu_render_contention schema=1 status=started producers=1 cpu_bar=xmobar gpu=virgl" |
        tee -a "$EVIDENCE_FILE"

    for launch in 'meta_l+ret gpu2 2' 'meta_l+p gpu3 3'; do
        read -r chord app visible <<<"$launch"
        started_baseline="$(evidence_count "^sophia_session_app schema=2 status=started id=$app source=action ")"
        admitted_baseline="$(evidence_count '^sophia_session_app schema=2 status=admitted source=action ')"
        echo "sophia_qemu_render_contention schema=1 status=launch_begin chord=$chord app=$app" |
            tee -a "$EVIDENCE_FILE"
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" "$chord"
        if ! wait_for_new_evidence \
            "^sophia_session_app schema=2 status=started id=$app source=action " \
            "$started_baseline" 400 \
            || ! wait_for_new_evidence \
                '^sophia_session_app schema=2 status=admitted source=action ' \
                "$admitted_baseline" 1600; then
            echo "sophia_qemu_render_contention schema=1 status=failed reason=producer_admission_timeout app=$app" |
                tee -a "$EVIDENCE_FILE"
            exit 1
        fi
        producer_ready=false
        for _ in $(seq 1 800); do
            if grep -Eq "^sophia_live_wm schema=2 status=workspace_projection_committed .*visible_surfaces=$visible focus=surface$" "$EVIDENCE_FILE" \
                && (( $(dmabuf_retirement_surface_count) == visible )); then
                producer_ready=true
                break
            fi
            if grep -q '^sophia_live_wm schema=1 status=layout_timeout ' "$EVIDENCE_FILE" \
                || grep -Eq '^sophia_session_app schema=1 status=exited id=(gpu1|gpu2|gpu3|statusbar) ' "$EVIDENCE_FILE"; then
                break
            fi
            if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
            sleep 0.05
        done
        if [[ "$producer_ready" != true ]]; then
            echo "sophia_qemu_render_contention schema=1 status=failed reason=producer_progress_timeout app=$app visible=$visible surfaces=$(dmabuf_retirement_surface_count)" |
                tee -a "$EVIDENCE_FILE"
            exit 1
        fi
        echo "sophia_qemu_render_contention schema=1 status=producer_ready app=$app producers=$visible" |
            tee -a "$EVIDENCE_FILE"
    done

    window_start_line="$(wc -l < "$EVIDENCE_FILE")"
    echo "sophia_qemu_render_contention schema=1 status=window_started producers=3 minimum_frames=30" |
        tee -a "$EVIDENCE_FILE"
    producers_ready=false
    for _ in $(seq 1 1200); do
        read -r surfaces minimum retirements <<<"$(dmabuf_retirement_window_stats "$window_start_line")"
        if (( surfaces == 3 && minimum >= 30 )); then
            producers_ready=true
            break
        fi
        if grep -q '^sophia_live_wm schema=1 status=layout_timeout ' "$EVIDENCE_FILE" \
            || grep -Eq '^sophia_session_app schema=1 status=exited id=(gpu1|gpu2|gpu3|statusbar) ' "$EVIDENCE_FILE"; then
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$producers_ready" != true ]]; then
        echo "sophia_qemu_render_contention schema=1 status=failed reason=dmabuf_progress_timeout retirements=${retirements:-0} surfaces=${surfaces:-0} minimum=${minimum:-0}" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    echo "sophia_qemu_render_contention schema=1 status=window_complete producers=3 dmabuf_surfaces=$surfaces minimum_retirements=$minimum retirements=$retirements" |
        tee -a "$EVIDENCE_FILE"
    sleep 1

    echo "sophia_qemu_render_contention schema=1 status=logout_begin chord=meta_l+shift+q" |
        tee -a "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+shift+q
    echo "sophia_qemu_render_contention schema=1 status=logout_sent chord=meta_l+shift+q" |
        tee -a "$EVIDENCE_FILE"

    set +e
    wait "$QEMU_PID"
    qemu_status=$?
    QEMU_PID=""
    wait "$LOGGER_PID"
    logger_status=$?
    LOGGER_PID=""
    set -e
    cleanup
    if [[ "$qemu_status" -ne 0 || "$logger_status" -ne 0 ]]; then
        echo "sophia_qemu_render_contention schema=1 status=failed reason=guest_exit qemu_exit=$qemu_status logger_exit=$logger_status" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    "$ROOT_DIR/tools/verify_qemu_xmonad_render_contention_evidence.sh" "$EVIDENCE_FILE"
    echo "sophia_qemu_xmonad schema=1 status=complete qemu_exit=0" |
        tee -a "$EVIDENCE_FILE"
    exit 0
fi

if [[ "$SCENARIO" == "xmonad-resize-storm" ]]; then
    ready=false
    for _ in $(seq 1 800); do
        if grep -q '^sophia_live_wm schema=1 status=ready ' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_input_pipeline schema=1 status=focus_ready$' "$EVIDENCE_FILE" \
            && grep -Eq '^sophia_live_wm schema=2 status=workspace_projection_committed .*visible_surfaces=1 focus=surface$' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_startup schema=2 status=output_baseline_ready outputs=2/2$' "$EVIDENCE_FILE"; then
            ready=true
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$ready" != true ]]; then
        echo "sophia_qemu_resize_storm schema=1 status=failed reason=readiness_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    echo "sophia_qemu_resize_storm schema=1 status=started steps=12 client=cpu-renderer" |
        tee -a "$EVIDENCE_FILE"
    if ! wait_for_evidence_count_at_least \
        '^sophia_live_resize schema=2 status=committed ' 12 2400 \
        || ! wait_for_new_evidence \
            '^sophia_live_resize_storm schema=1 status=complete steps=12 .* exact_pixels=true$' 0 400; then
        echo "sophia_qemu_resize_storm schema=1 status=failed reason=resize_sequence_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    retirement_baseline="$(evidence_count 'sophia_live_native_page_flip schema=1 status=retired output=')"
    if ! wait_for_new_evidence \
        'sophia_live_native_page_flip schema=1 status=retired output=' \
        "$retirement_baseline" 800; then
        echo "sophia_qemu_resize_storm schema=1 status=failed reason=post_storm_frame_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    echo "sophia_qemu_resize_storm schema=1 status=post_storm_frame_retired steps=12" |
        tee -a "$EVIDENCE_FILE"

    echo "sophia_qemu_resize_storm schema=1 status=logout_begin chord=meta_l+shift+q" |
        tee -a "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+shift+q
    echo "sophia_qemu_resize_storm schema=1 status=logout_sent chord=meta_l+shift+q" |
        tee -a "$EVIDENCE_FILE"

    set +e
    wait "$QEMU_PID"
    qemu_status=$?
    QEMU_PID=""
    wait "$LOGGER_PID"
    logger_status=$?
    LOGGER_PID=""
    set -e
    cleanup
    if [[ "$qemu_status" -ne 0 || "$logger_status" -ne 0 ]]; then
        echo "sophia_qemu_resize_storm schema=1 status=failed reason=guest_exit qemu_exit=$qemu_status logger_exit=$logger_status" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    "$ROOT_DIR/tools/verify_qemu_xmonad_resize_storm_evidence.sh" "$EVIDENCE_FILE"
    echo "sophia_qemu_xmonad schema=1 status=complete qemu_exit=0" |
        tee -a "$EVIDENCE_FILE"
    exit 0
fi

if [[ "$SCENARIO" == "xmonad-stale-response" ]]; then
    ready=false
    for _ in $(seq 1 800); do
        if grep -q '^sophia_live_wm schema=1 status=ready ' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_input_pipeline schema=1 status=focus_ready$' "$EVIDENCE_FILE" \
            && grep -Eq '^sophia_live_wm schema=2 status=workspace_projection_committed .*visible_surfaces=2 focus=surface$' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$' "$EVIDENCE_FILE"; then
            ready=true
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$ready" != true ]]; then
        echo "sophia_qemu_stale_response schema=1 status=failed reason=readiness_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi

    projection_baseline="$(evidence_count '^sophia_live_wm schema=2 status=workspace_projection_committed .*visible_surfaces=2 focus=surface$')"
    echo "sophia_qemu_stale_response schema=1 status=launch_begin chord=meta_l+ret" |
        tee -a "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+ret
    echo "sophia_qemu_stale_response schema=1 status=launch_sent chord=meta_l+ret" |
        tee -a "$EVIDENCE_FILE"
    if ! wait_for_new_evidence \
        '^sophia_session_app schema=2 status=started id=transient source=action ' 0 400 \
        || ! wait_for_new_evidence \
            '^sophia_session_app schema=2 status=completed id=transient source=action .* reason=normal_exit_after_surface ' 0 800 \
        || ! wait_for_new_evidence \
            '^sophia_live_wm schema=3 status=response_rejected reason=stale_layout .* source=manage removed_registered_surfaces=0$' 0 800 \
        || ! wait_for_new_evidence \
            '^sophia_live_wm schema=1 status=restarted restarts=1 preserved_layout=true$' 0 800; then
        echo "sophia_qemu_stale_response schema=1 status=failed reason=stale_recovery_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    restart_line="$(awk '/^sophia_live_wm schema=1 status=restarted restarts=1 preserved_layout=true$/ { print NR; exit }' "$EVIDENCE_FILE")"
    recovered=false
    for _ in $(seq 1 800); do
        if evidence_has_after_line \
            '^sophia_live_wm schema=4 status=reseed_queued phase=committed_layout request=relayout$' \
            "$restart_line" \
            && evidence_has_after_line \
                '^sophia_live_wm schema=2 status=workspace_projection_committed .*visible_surfaces=2 focus=surface$' \
                "$restart_line" \
            && (( $(evidence_count '^sophia_live_wm schema=2 status=workspace_projection_committed .*visible_surfaces=2 focus=surface$') > projection_baseline )); then
            recovered=true
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$recovered" != true ]]; then
        echo "sophia_qemu_stale_response schema=1 status=failed reason=reseed_projection_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    echo "sophia_qemu_stale_response schema=1 status=recovered restarts=1 visible_surfaces=2" |
        tee -a "$EVIDENCE_FILE"

    action_baseline="$(evidence_count '^sophia_live_wm schema=1 status=physical_action_committed action=')"
    focus_baseline="$(evidence_count '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$')"
    echo "sophia_qemu_stale_response schema=1 status=action_probe_begin chord=meta_l+j" |
        tee -a "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+j
    if ! wait_for_new_evidence \
        '^sophia_live_wm schema=1 status=physical_action_committed action=' \
        "$action_baseline" 400 \
        || ! wait_for_new_evidence \
            '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$' \
            "$focus_baseline" 400; then
        echo "sophia_qemu_stale_response schema=1 status=failed reason=post_restart_action_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    echo "sophia_qemu_stale_response schema=1 status=action_probe_committed chord=meta_l+j focus=applied" |
        tee -a "$EVIDENCE_FILE"

    echo "sophia_qemu_stale_response schema=1 status=logout_begin chord=meta_l+shift+q" |
        tee -a "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+shift+q
    echo "sophia_qemu_stale_response schema=1 status=logout_sent chord=meta_l+shift+q" |
        tee -a "$EVIDENCE_FILE"

    set +e
    wait "$QEMU_PID"
    qemu_status=$?
    QEMU_PID=""
    wait "$LOGGER_PID"
    logger_status=$?
    LOGGER_PID=""
    set -e
    cleanup
    if [[ "$qemu_status" -ne 0 || "$logger_status" -ne 0 ]]; then
        echo "sophia_qemu_stale_response schema=1 status=failed reason=guest_exit qemu_exit=$qemu_status logger_exit=$logger_status" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    "$ROOT_DIR/tools/verify_qemu_xmonad_stale_response_evidence.sh" "$EVIDENCE_FILE"
    echo "sophia_qemu_xmonad schema=1 status=complete qemu_exit=0" |
        tee -a "$EVIDENCE_FILE"
    exit 0
fi

if [[ "$SCENARIO" == "xmonad-launch-burst" ]]; then
    ready=false
    for _ in $(seq 1 800); do
        if grep -q '^sophia_live_wm schema=1 status=ready ' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_input_pipeline schema=1 status=focus_ready$' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_startup schema=2 status=output_baseline_ready outputs=2/2$' "$EVIDENCE_FILE"; then
            ready=true
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$ready" != true ]]; then
        echo "sophia_qemu_launch_burst schema=1 status=failed reason=readiness_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi

    managed_exit_baseline="$(evidence_count '^sophia_session_app schema=1 status=exited id=holder')"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+ret 32
    echo "sophia_qemu_launch_burst schema=1 status=sent chord=meta_l+ret requests=32" |
        tee -a "$EVIDENCE_FILE"
    if ! wait_for_evidence_count_at_least \
        '^sophia_session_app schema=2 status=admitted source=action ' 4 1600; then
        echo "sophia_qemu_launch_burst schema=1 status=failed reason=admission_drain_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    queued="$(evidence_count '^sophia_session_app schema=2 status=queued source=action ')"
    rejected="$(evidence_count '^sophia_session_app schema=2 status=rejected source=action .* reason=capacity$')"
    if (( queued != 4 || rejected < 20 || rejected > 28 )); then
        echo "sophia_qemu_launch_burst schema=1 status=failed reason=burst_accounting queued=$queued rejected=$rejected" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    echo "sophia_qemu_launch_burst schema=1 status=settled active_preload=12 queued=4 admitted=4 rejected=$rejected" |
        tee -a "$EVIDENCE_FILE"

    echo "sophia_qemu_launch_burst schema=1 status=capacity_release_wait source=managed_exit" |
        tee -a "$EVIDENCE_FILE"
    if ! wait_for_new_evidence \
        '^sophia_session_app schema=1 status=exited id=holder' \
        "$managed_exit_baseline" 800; then
        echo "sophia_qemu_launch_burst schema=1 status=failed reason=capacity_release_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    managed_exits="$(evidence_count '^sophia_session_app schema=1 status=exited id=holder')"
    echo "sophia_qemu_launch_burst schema=1 status=capacity_released managed_exits=$managed_exits" |
        tee -a "$EVIDENCE_FILE"

    queued_baseline="$(evidence_count '^sophia_session_app schema=2 status=queued source=action ')"
    admitted_baseline="$(evidence_count '^sophia_session_app schema=2 status=admitted source=action ')"
    recovery_focus_baseline="$(evidence_count '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$')"
    echo "sophia_qemu_launch_burst schema=1 status=recovery_launch_begin chord=meta_l+ret" |
        tee -a "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+ret
    echo "sophia_qemu_launch_burst schema=1 status=recovery_launch_sent chord=meta_l+ret" |
        tee -a "$EVIDENCE_FILE"
    if ! wait_for_new_evidence \
        '^sophia_session_app schema=2 status=queued source=action ' "$queued_baseline" 400 \
        || ! wait_for_new_evidence \
            '^sophia_session_app schema=2 status=admitted source=action ' "$admitted_baseline" 1600; then
        echo "sophia_qemu_launch_burst schema=1 status=failed reason=recovery_launch_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    echo "sophia_qemu_launch_burst schema=1 status=recovery_admitted queued=5 admitted=5" |
        tee -a "$EVIDENCE_FILE"
    if ! wait_for_new_evidence \
        '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$' \
        "$recovery_focus_baseline" 400; then
        echo "sophia_qemu_launch_burst schema=1 status=failed reason=recovery_focus_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    echo "sophia_qemu_launch_burst schema=1 status=recovery_focus_ready source=x11-control" |
        tee -a "$EVIDENCE_FILE"

    action_baseline="$(evidence_count '^sophia_live_wm schema=1 status=physical_action_committed action=')"
    action_focus_baseline="$(evidence_count '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$')"
    echo "sophia_qemu_launch_burst schema=1 status=action_probe_begin chord=meta_l+j" |
        tee -a "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+j
    if ! wait_for_new_evidence \
        '^sophia_live_wm schema=1 status=physical_action_committed action=' \
        "$action_baseline" 400; then
        echo "sophia_qemu_launch_burst schema=1 status=failed reason=post_burst_action_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    if ! wait_for_new_evidence \
        '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$' \
        "$action_focus_baseline" 400; then
        echo "sophia_qemu_launch_burst schema=1 status=failed reason=post_burst_focus_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    echo "sophia_qemu_launch_burst schema=1 status=action_probe_committed chord=meta_l+j focus=applied" |
        tee -a "$EVIDENCE_FILE"

    echo "sophia_qemu_launch_burst schema=1 status=logout_begin chord=meta_l+shift+q" |
        tee -a "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+shift+q
    echo "sophia_qemu_launch_burst schema=1 status=logout_sent chord=meta_l+shift+q" |
        tee -a "$EVIDENCE_FILE"

    set +e
    wait "$QEMU_PID"
    qemu_status=$?
    QEMU_PID=""
    wait "$LOGGER_PID"
    logger_status=$?
    LOGGER_PID=""
    set -e
    cleanup
    if [[ "$qemu_status" -ne 0 || "$logger_status" -ne 0 ]]; then
        echo "sophia_qemu_launch_burst schema=1 status=failed reason=guest_exit qemu_exit=$qemu_status logger_exit=$logger_status" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    "$ROOT_DIR/tools/verify_qemu_xmonad_launch_burst_evidence.sh" "$EVIDENCE_FILE"
    echo "sophia_qemu_xmonad schema=1 status=complete qemu_exit=0" |
        tee -a "$EVIDENCE_FILE"
    exit 0
fi

if [[ "$SCENARIO" == xmonad-* ]]; then
    ready=false
    for _ in $(seq 1 800); do
        if grep -q '^sophia_live_wm schema=1 status=ready ' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_input_pipeline schema=1 status=focus_ready$' "$EVIDENCE_FILE" \
            && grep -Eq '^sophia_live_wm schema=2 status=workspace_projection_committed .*visible_surfaces=2 focus=surface$' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$' "$EVIDENCE_FILE"; then
            ready=true
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$ready" != true ]]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=readiness_timeout" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi

    focus_baseline="$(evidence_count '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$')"
    send_chord_and_wait meta_l+j '^sophia_live_wm schema=1 status=physical_action_committed action=' focus-before-pointer
    if ! wait_for_new_evidence '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$' "$focus_baseline"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=pointer_setup_focus_timeout" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    run_pointer_focus_gesture click x

    focus_baseline="$(evidence_count '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$')"
    send_chord_and_wait meta_l+j '^sophia_live_wm schema=1 status=physical_action_committed action=' focus-before-pointer-drag
    if ! wait_for_new_evidence '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$' "$focus_baseline"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=pointer_drag_setup_focus_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    run_pointer_focus_gesture drag z

    pointer_edge_baseline="$(evidence_count '^sophia_live_session_pointer schema=7 status=output_edge_confined axis=horizontal side=maximum$')"
    if ! "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" 4096 0 0; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=qmp_pointer_edge_send" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    if ! wait_for_new_evidence '^sophia_live_session_pointer schema=7 status=output_edge_confined axis=horizontal side=maximum$' "$pointer_edge_baseline"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=pointer_edge_timeout" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    pointer_reverse_baseline="$(evidence_count '^sophia_live_session_pointer schema=7 status=edge_reverse_immediate axis=horizontal side=maximum$')"
    if ! "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" -96 0 0; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=qmp_pointer_reverse_send" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    if ! wait_for_new_evidence '^sophia_live_session_pointer schema=7 status=edge_reverse_immediate axis=horizontal side=maximum$' "$pointer_reverse_baseline"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=pointer_reverse_timeout" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    echo "sophia_qemu_xmonad_pointer schema=3 status=passed source=qmp device=virtio-mouse action=output_edge_reverse edge=right reverse_delta=96" | tee -a "$EVIDENCE_FILE"

    empty_workspace_chord=meta_l+2
    if [[ "$SCENARIO" == "xmonad-m8-mix" || "$SCENARIO" == "xmonad-m8-soak" ]]; then
        moved_vulkan_baseline="$(evidence_count '^sophia_live_wm schema=2 status=workspace_projection_committed .* workspace=1 visible_surfaces=1 focus=')"
        send_chord_and_wait meta_l+shift+2 '^sophia_live_wm schema=1 status=physical_action_committed action=' vulkan-workspace-move
        if ! wait_for_new_evidence '^sophia_live_wm schema=2 status=workspace_projection_committed .* workspace=1 visible_surfaces=1 focus=\(surface\|none\)$' "$moved_vulkan_baseline"; then
            echo "sophia_qemu_xmonad schema=1 status=failed reason=vulkan_workspace_move_timeout" |
                tee -a "$EVIDENCE_FILE"
            exit 1
        fi
        empty_workspace_chord=meta_l+3
    fi

    focus_baseline="$(evidence_count '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$')"
    focused_projection_baseline="$(evidence_count '^sophia_live_wm schema=2 status=workspace_projection_committed .* workspace=1 visible_surfaces=1 focus=surface$')"
    send_chord_and_wait meta_l+k '^sophia_live_wm schema=1 status=physical_action_committed action=' prelude-focus
    if ! wait_for_new_evidence '^sophia_live_wm schema=2 status=workspace_projection_committed .* workspace=1 visible_surfaces=1 focus=surface$' "$focused_projection_baseline" \
        || ! wait_for_new_evidence '^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$' "$focus_baseline"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=prelude_focus_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi

    layout_baseline="$(evidence_count '^sophia_live_wm schema=1 status=layout_committed ')"
    resized_projection_baseline="$(evidence_count '^sophia_live_wm schema=2 status=workspace_projection_committed .* workspace=1 visible_surfaces=1 focus=surface$')"
    send_chord_and_wait meta_l+spc '^sophia_live_wm schema=1 status=physical_action_committed action=' prelude-layout
    if ! wait_for_new_evidence '^sophia_live_wm schema=1 status=layout_committed ' "$layout_baseline" \
        || ! wait_for_new_evidence '^sophia_live_wm schema=2 status=workspace_projection_committed .* workspace=1 visible_surfaces=1 focus=surface$' "$resized_projection_baseline"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=prelude_layout_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi

    empty_workspace_baseline="$(evidence_count '^sophia_live_wm schema=2 status=workspace_projection_committed .* visible_surfaces=0 focus=none$')"
    empty_workspace_reached=false
    # Viewing a workspace is idempotent, so a bounded resend is safe if TCG
    # delays the first virtio-keyboard packet beyond the observation window.
    for _ in $(seq 1 4); do
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" "$empty_workspace_chord"
        echo "sophia_qemu_xmonad_input schema=1 status=sent chord=$empty_workspace_chord" |
            tee -a "$EVIDENCE_FILE"
        if wait_for_new_evidence '^sophia_live_wm schema=2 status=workspace_projection_committed .* visible_surfaces=0 focus=none$' "$empty_workspace_baseline" 80; then
            empty_workspace_reached=true
            break
        fi
    done
    if [[ "$empty_workspace_reached" != true ]]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=empty_workspace_projection_timeout" |
            tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    probe_empty_workspace_pointer
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+1
    echo "sophia_qemu_xmonad_input schema=1 status=sent chord=meta_l+1" | tee -a "$EVIDENCE_FILE"
    sleep 1
    restarted=false
    restart_line=0
    for _ in $(seq 1 800); do
        if grep -q '^sophia_live_wm schema=1 status=restarted .*preserved_layout=true' "$EVIDENCE_FILE"; then
            restarted=true
            restart_line="$(awk '/^sophia_live_wm schema=1 status=restarted .*preserved_layout=true/ { print NR; exit }' "$EVIDENCE_FILE")"
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$restarted" != true ]]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=restart_recovery_timeout" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    recovery_layout=false
    for _ in $(seq 1 400); do
        if evidence_has_after_line '^sophia_live_wm schema=1 status=layout_committed ' "$restart_line"; then
            recovery_layout=true
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$recovery_layout" != true ]] &&
        ! evidence_has_after_line '^sophia_live_wm schema=1 status=layout_timeout .*preserved_layout=true' "$restart_line"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=restart_layout_timeout" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    focus_state_recovered=false
    for _ in $(seq 1 400); do
        if evidence_has_after_line '^sophia_live_wm schema=1 status=focus_reconciled ' "$restart_line" ||
            evidence_has_after_line '^sophia_live_wm schema=1 status=hidden_focus_cleared ' "$restart_line"; then
            focus_state_recovered=true
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$focus_state_recovered" != true ]]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=restart_focus_state_timeout" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    if [[ "$SCENARIO" == "xmonad-m8-soak" ]]; then
        soak_started=$SECONDS
        cycles=0
        while (( SECONDS - soak_started < 1800 )); do
            for chord in meta_l+j meta_l+k meta_l+spc meta_l+3 meta_l+1; do
                "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" "$chord"
                echo "sophia_qemu_xmonad_input schema=1 status=sent chord=$chord" | tee -a "$EVIDENCE_FILE"
                sleep 1
            done
            send_launch_and_wait meta_l+ret '^sophia_session_app schema=1 status=started id=terminal source=action' terminal-launch
            send_close_and_wait terminal
            send_launch_and_wait meta_l+f '^sophia_session_app schema=1 status=started id=firefox source=action' firefox-launch
            if (( cycles == 0 )); then
                run_firefox_m8_interactions
            fi
            send_firefox_close_and_wait
            send_launch_and_wait meta_l+p '^sophia_session_app schema=1 status=started id=launcher source=action' launcher-launch
            send_close_and_wait launcher
            cycles=$((cycles + 1))
            echo "sophia_qemu_m8_soak schema=1 status=cycle_complete cycle=$cycles terminal_restarts=$cycles firefox_restarts=$cycles launcher_restarts=$cycles close_actions=$((cycles * 3))" | tee -a "$EVIDENCE_FILE"
            sleep 65
        done
        if (( cycles < 20 )); then
            echo "sophia_qemu_m8_soak schema=1 status=failed reason=insufficient_cycles cycles=$cycles" | tee -a "$EVIDENCE_FILE"
            exit 1
        fi
        chords=("meta_l+shift+q")
    elif [[ "$SCENARIO" == "xmonad-m8-mix" ]]; then
        send_launch_and_wait meta_l+ret '^sophia_session_app schema=1 status=started id=terminal source=action' terminal-launch
        send_close_and_wait terminal
        send_launch_and_wait meta_l+f '^sophia_session_app schema=1 status=started id=firefox source=action' firefox-launch
        run_firefox_m8_interactions
        send_firefox_close_and_wait
        send_launch_and_wait meta_l+p '^sophia_session_app schema=1 status=started id=launcher source=action' launcher-launch
        send_close_and_wait launcher
        chords=("meta_l+shift+q")
    else
        chords=("meta_l+ret" "meta_l+shift+c" "meta_l+shift+q")
    fi
    for chord in "${chords[@]}"; do
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" "$chord"
        echo "sophia_qemu_xmonad_input schema=1 status=sent chord=$chord" | tee -a "$EVIDENCE_FILE"
        sleep 1
    done


    set +e
    wait "$QEMU_PID"
    qemu_status=$?
    QEMU_PID=""
    wait "$LOGGER_PID"
    logger_status=$?
    LOGGER_PID=""
    set -e
    cleanup
    if [[ "$qemu_status" -ne 0 || "$logger_status" -ne 0 ]]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=guest_exit qemu_exit=$qemu_status logger_exit=$logger_status" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    case "$SCENARIO" in
        xmonad-m7) "$ROOT_DIR/tools/verify_qemu_xmonad_m7_evidence.sh" "$EVIDENCE_FILE" ;;
        xmonad-m8-launcher) "$ROOT_DIR/tools/verify_qemu_xmonad_m8_launcher_evidence.sh" "$EVIDENCE_FILE" ;;
        xmonad-m8-mix) "$ROOT_DIR/tools/verify_qemu_xmonad_m8_mix_evidence.sh" "$EVIDENCE_FILE" ;;
        xmonad-m8-soak) "$ROOT_DIR/tools/verify_qemu_xmonad_m8_soak_evidence.sh" "$EVIDENCE_FILE" ;;
    esac
    echo "sophia_qemu_xmonad schema=1 status=complete qemu_exit=0" | tee -a "$EVIDENCE_FILE"
    exit 0
fi
if [[ "$SCENARIO" == gtk-* ]]; then
    input_ready=false
    for _ in $(seq 1 600); do
        if grep -q '^sophia_live_session_input schema=1 status=ready source=physical text=sophia$' "$EVIDENCE_FILE"; then
            input_ready=true
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$input_ready" != true ]]; then
        echo "sophia_qemu_gtk schema=1 status=failed reason=input_readiness_timeout scenario=$SCENARIO" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi

    "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" 1 1 1
    echo "sophia_qemu_gtk_pointer schema=1 status=sent phase=entry_focus source=qmp clicks=1" | tee -a "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/qemu_qmp_type.py" "$QMP_SOCKET" sophia
    echo "sophia_qemu_gtk_input schema=1 status=sent source=qmp text=sophia events=14" | tee -a "$EVIDENCE_FILE"

    pointer_ready=false
    for _ in $(seq 1 200); do
        if grep -q '^sophia_live_session_pointer schema=1 status=ready source=physical action=select$' "$EVIDENCE_FILE"; then
            pointer_ready=true
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$pointer_ready" != true ]]; then
        echo "sophia_qemu_gtk schema=1 status=failed reason=pointer_readiness_timeout scenario=$SCENARIO" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi

    "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" 0 0 1
    echo "sophia_qemu_gtk_pointer schema=1 status=sent phase=focused_select source=qmp clicks=1" | tee -a "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/qemu_qmp_type.py" "$QMP_SOCKET"
    echo "sophia_qemu_gtk_input schema=1 status=sent source=qmp action=submit events=2" | tee -a "$EVIDENCE_FILE"

    set +e
    wait "$QEMU_PID"
    qemu_status=$?
    QEMU_PID=""
    wait "$LOGGER_PID"
    logger_status=$?
    LOGGER_PID=""
    set -e
    cleanup

    if [[ "$qemu_status" -ne 0 || "$logger_status" -ne 0 ]]; then
        echo "sophia_qemu_gtk schema=1 status=failed reason=guest_exit scenario=$SCENARIO qemu_exit=$qemu_status logger_exit=$logger_status" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    if ! grep -q "^sophia_qemu_guest schema=1 status=complete scenario=$SCENARIO$" "$EVIDENCE_FILE" \
        || ! grep -q '^sophia_x_application_session schema=1 status=passed class=gtk3_software client=zenity .*protocol_errors=0 first_error=none physical_text=true pointer_button=true surface_resize=committed buffer_path=cpu_shm native_presentation=enabled cleanup=clean$' "$EVIDENCE_FILE"; then
        echo "sophia_qemu_gtk schema=1 status=failed reason=semantic_evidence scenario=$SCENARIO" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    echo "sophia_qemu_gtk schema=1 status=complete scenario=$SCENARIO qemu_exit=0" | tee -a "$EVIDENCE_FILE"
    exit 0
fi

input_ready=false
for _ in $(seq 1 600); do
    if grep -q '^sophia_live_session_input schema=1 status=ready source=physical text=sophia$' "$EVIDENCE_FILE"; then
        input_ready=true
        break
    fi
    if ! kill -0 "$QEMU_PID" 2>/dev/null; then
        break
    fi
    sleep 0.05
done
if [[ "$input_ready" != true ]]; then
    echo "sophia_qemu_session schema=3 status=failed reason=input_readiness_timeout" | tee -a "$EVIDENCE_FILE"
    exit 1
fi

if ! "$ROOT_DIR/tools/qemu_qmp_type.py" "$QMP_SOCKET" sophia; then
    echo "sophia_qemu_session schema=3 status=failed reason=qmp_input_send" | tee -a "$EVIDENCE_FILE"
    exit 1
fi
echo "sophia_qemu_input schema=1 status=sent source=qmp device=virtio-keyboard text=sophia events=14" | tee -a "$EVIDENCE_FILE"

pointer_ready=false
for _ in $(seq 1 100); do
    if grep -q '^sophia_live_session_pointer schema=1 status=ready source=physical action=select$' "$EVIDENCE_FILE"; then
        pointer_ready=true
        break
    fi
    if ! kill -0 "$QEMU_PID" 2>/dev/null; then
        break
    fi
    sleep 0.05
done
if [[ "$pointer_ready" != true ]]; then
    echo "sophia_qemu_session schema=3 status=failed reason=pointer_readiness_timeout" | tee -a "$EVIDENCE_FILE"
    exit 1
fi

if ! "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET"; then
    echo "sophia_qemu_session schema=3 status=failed reason=qmp_pointer_send" | tee -a "$EVIDENCE_FILE"
    exit 1
fi
echo "sophia_qemu_pointer schema=1 status=sent source=qmp device=virtio-mouse action=select commands=5" | tee -a "$EVIDENCE_FILE"

set +e
wait "$QEMU_PID"
qemu_status=$?
QEMU_PID=""
wait "$LOGGER_PID"
logger_status=$?
LOGGER_PID=""
set -e
cleanup

if [[ "$qemu_status" -ne 0 ]]; then
    echo "sophia_qemu_session schema=3 status=failed qemu_exit=$qemu_status" | tee -a "$EVIDENCE_FILE"
    exit "$qemu_status"
fi
if [[ "$logger_status" -ne 0 ]]; then
    echo "sophia_qemu_session schema=3 status=failed serial_logger_exit=$logger_status" | tee -a "$EVIDENCE_FILE"
    exit "$logger_status"
fi

echo "sophia_qemu_session schema=3 status=complete qemu_exit=0" | tee -a "$EVIDENCE_FILE"
if [[ "$TWO_XTERM" == "1" ]]; then
    SOPHIA_QEMU_REQUIRE_TWO_XTERM=1 \
        "$ROOT_DIR/tools/verify_qemu_session_evidence.sh" "$EVIDENCE_FILE"
else
    "$ROOT_DIR/tools/verify_qemu_session_evidence.sh" "$EVIDENCE_FILE"
fi
