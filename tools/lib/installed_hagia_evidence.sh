# Read-only coverage reduction for ordinary installed Hagia sessions.

sophia_hagia_count() {
    local log="$1" pattern="$2"
    grep -Ec "$pattern" "$log" 2>/dev/null || true
}

sophia_hagia_emit_coverage() {
    local log="$1"
    printf 'sophia_hagia_coverage schema=1 terminal_starts=%s firefox_starts=%s physical_actions=%s session_actions=%s pointer_moves=%s pointer_resizes=%s checkpoints=%s reconciliations=%s output_changes=%s topology_changes=%s\n' \
        "$(sophia_hagia_count "$log" '^sophia_session_app schema=(1|2) status=started id=terminal ')" \
        "$(sophia_hagia_count "$log" '^sophia_session_app schema=(1|2) status=started id=firefox ')" \
        "$(sophia_hagia_count "$log" '^sophia_live_wm schema=1 status=physical_action_committed ')" \
        "$(sophia_hagia_count "$log" '^sophia_live_wm schema=1 status=session_action_committed ')" \
        "$(sophia_hagia_count "$log" '^sophia_live_wm schema=4 status=pointer_gesture_committed mode=move$')" \
        "$(sophia_hagia_count "$log" '^sophia_live_wm schema=4 status=pointer_gesture_committed mode=resize$')" \
        "$(sophia_hagia_count "$log" '(^hagia_policy_checkpoint schema=1 status=saved | event=checkpoint status=saved )')" \
        "$(sophia_hagia_count "$log" '(^hagia_policy_checkpoint schema=1 status=reconciled | event=checkpoint status=reconciled )')" \
        "$(sophia_hagia_count "$log" '(^hagia_policy_projection schema=1 status=active_output_changed$| event=projection status=active_output_changed detail=$)')" \
        "$(sophia_hagia_count "$log" '^sophia_live_output_topology .*status=(changed|removed|restored)')"
}

sophia_hagia_write_coverage() {
    local log="$1" output="$2" temporary
    temporary="${output}.tmp.$$"
    sophia_hagia_emit_coverage "$log" >"$temporary"
    chmod 600 "$temporary"
    mv -f "$temporary" "$output"
}
