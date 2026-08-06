#!/usr/bin/env bash

sophia_session_rotate_log() {
    (( $# == 1 )) || return 2
    local log="$1" parent
    [[ -n "$log" && "$log" != / && ! -d "$log" ]] || return 2
    parent="$(dirname "$log")"
    [[ -d "$parent" && -w "$parent" ]] || return 2

    # Keep one diagnostic generation without allowing separate session
    # artifacts to accumulate on different retention schedules.
    if [[ -e "$log" || -L "$log" ]]; then
        mv -f -- "$log" "$log.previous"
    fi
    : >"$log"
    chmod 600 "$log"
}

sophia_session_record_failure() {
    local lifecycle_log="$1"
    local phase="$2"
    local installed="$3"
    local version="$4"
    local commit="$5"
    local exit_status="$6"

    case "$phase" in
        preflight | input_guard | graphics_takeover | session | handoff) ;;
        *) return 2 ;;
    esac
    [[ "$installed" == true || "$installed" == false ]] || return 2
    [[ "$version" =~ ^[0-9A-Za-z._-]+$ ]] || return 2
    [[ "$commit" =~ ^[0-9A-Za-z._-]+$ ]] || return 2
    [[ "$exit_status" =~ ^[0-9]+$ ]] || return 2

    printf 'sophia_session_diagnostic schema=1 status=failed phase=%s installed=%s version=%s commit=%s exit_status=%s\n' \
        "$phase" "$installed" "$version" "$commit" "$exit_status" \
        >>"$lifecycle_log"
}
