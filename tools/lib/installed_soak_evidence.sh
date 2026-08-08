# Shared read-only reduction for the installed xmonad soak gate.

readonly SOPHIA_SOAK_PRACTICAL_ACTION_IDS=(1 2 3 4 5 6 7 8 9 10 11 12 13 14)
readonly SOPHIA_SOAK_PRACTICAL_ACTION_NAMES=(
    focus-next focus-previous next-layout toggle-floating reset-layout
    focus-master swap-master swap-down swap-up shrink expand sink
    increase-master-count decrease-master-count
)

sophia_soak_count() {
    local log="$1" pattern="$2"
    grep -Ec "$pattern" "$log" 2>/dev/null || true
}

sophia_soak_line_field() {
    local line="$1" name="$2" token
    for token in $line; do
        if [[ "$token" == "$name="* ]]; then
            printf '%s\n' "${token#*=}"
            return 0
        fi
    done
    return 1
}

sophia_soak_completion() {
    grep -E '^sophia_live_session schema=(14|15|16) status=bounded_complete ' \
        "$1" 2>/dev/null || true
}

sophia_soak_action_count() {
    local log="$1" action="$2"
    sophia_soak_count "$log" \
        "^sophia_live_wm schema=1 status=physical_action_committed action=${action}$"
}

sophia_soak_action_name() {
    local wanted="$1" index
    for index in "${!SOPHIA_SOAK_PRACTICAL_ACTION_IDS[@]}"; do
        if [[ "${SOPHIA_SOAK_PRACTICAL_ACTION_IDS[$index]}" == "$wanted" ]]; then
            printf '%s\n' "${SOPHIA_SOAK_PRACTICAL_ACTION_NAMES[$index]}"
            return 0
        fi
    done
    return 1
}

sophia_soak_practical_complete_count() {
    local log="$1" action complete=0
    for action in "${SOPHIA_SOAK_PRACTICAL_ACTION_IDS[@]}"; do
        (( $(sophia_soak_action_count "$log" "$action") > 0 )) &&
            complete=$((complete + 1))
    done
    printf '%s\n' "$complete"
}

sophia_soak_missing_actions() {
    local log="$1" action name missing=""
    for action in "${SOPHIA_SOAK_PRACTICAL_ACTION_IDS[@]}"; do
        if (( $(sophia_soak_action_count "$log" "$action") == 0 )); then
            name="$(sophia_soak_action_name "$action")"
            missing="${missing:+$missing,}$name"
        fi
    done
    printf '%s\n' "${missing:-none}"
}

sophia_soak_workspace_view_count() {
    awk '
        /^sophia_live_wm schema=1 status=physical_action_committed action=/ {
            split($NF, field, "="); if (field[2] >= 257 && field[2] <= 265) count++
        }
        END { print count + 0 }
    ' "$1"
}

sophia_soak_workspace_move_count() {
    awk '
        /^sophia_live_wm schema=1 status=physical_action_committed action=/ {
            split($NF, field, "="); if (field[2] >= 513 && field[2] <= 521) count++
        }
        END { print count + 0 }
    ' "$1"
}

sophia_soak_emit_summary() {
    local log="$1" completion elapsed practical action
    completion="$(sophia_soak_completion "$log" | tail -n 1)"
    elapsed="$(sophia_soak_line_field "$completion" elapsed_msec 2>/dev/null || true)"
    [[ "$elapsed" =~ ^[0-9]+$ ]] || elapsed=0
    practical="$(sophia_soak_practical_complete_count "$log")"
    printf 'sophia_soak_summary schema=1 elapsed_msec=%s terminal_actions=%s firefox_actions=%s close_actions=%s workspace_views=%s workspace_moves=%s pointer_moves=%s pointer_resizes=%s practical_complete=%s practical_total=%s missing="%s"' \
        "$elapsed" \
        "$(sophia_soak_count "$log" '^sophia_session_app schema=(1|2) status=started id=terminal source=action( |$)')" \
        "$(sophia_soak_count "$log" '^sophia_session_app schema=(1|2) status=started id=firefox source=action( |$)')" \
        "$(sophia_soak_count "$log" '^sophia_live_wm schema=1 status=session_action_committed .* action=CloseFocused$')" \
        "$(sophia_soak_workspace_view_count "$log")" \
        "$(sophia_soak_workspace_move_count "$log")" \
        "$(sophia_soak_count "$log" '^sophia_live_wm schema=4 status=pointer_gesture_committed mode=move$')" \
        "$(sophia_soak_count "$log" '^sophia_live_wm schema=4 status=pointer_gesture_committed mode=resize$')" \
        "$practical" "${#SOPHIA_SOAK_PRACTICAL_ACTION_IDS[@]}" \
        "$(sophia_soak_missing_actions "$log")"
    for action in "${SOPHIA_SOAK_PRACTICAL_ACTION_IDS[@]}"; do
        printf ' action_%s=%s' "$action" "$(sophia_soak_action_count "$log" "$action")"
    done
    printf '\n'
}

sophia_soak_write_summary() {
    local log="$1" output="$2" temporary
    temporary="${output}.tmp.$$"
    umask 077
    sophia_soak_emit_summary "$log" >"$temporary"
    mv "$temporary" "$output"
}
