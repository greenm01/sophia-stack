# Terminal adapters own client-specific arguments; the session launcher owns
# only the application role and lifecycle.

sophia_resolve_session_terminal_kind() {
    local executable="$1" requested="${2:-}" resolved
    if [[ -n "$requested" ]]; then
        resolved="$requested"
    else
        resolved="$(basename "$(readlink -f "$executable")")"
    fi
    case "$resolved" in
        kitty | xterm) printf '%s\n' "$resolved" ;;
        *)
            echo "Unsupported Sophia terminal kind: $resolved" >&2
            return 1
            ;;
    esac
}

sophia_append_session_terminal_base_args() {
    local destination="$1" kind="$2" executable="$3"
    local -n arguments="$destination"
    arguments+=("--session-app=terminal=$executable" --session-start=terminal)
    case "$kind" in
        kitty)
            arguments+=(
                --session-app-arg=terminal=--config
                --session-app-arg=terminal=NONE
                --session-app-arg=terminal=--override
                --session-app-arg=terminal=linux_display_server=x11
                --session-app-arg=terminal=--override
                --session-app-arg=terminal=background_opacity=1
            )
            ;;
        xterm)
            arguments+=(
                --session-app-arg=terminal=-cm
                --session-app-arg=terminal=-dc
            )
            ;;
        *)
            echo "Unsupported Sophia terminal kind: $kind" >&2
            return 1
            ;;
    esac
}

sophia_append_session_terminal_title_args() {
    local destination="$1" kind="$2" title="$3"
    local -n arguments="$destination"
    case "$kind" in
        kitty) arguments+=(--session-app-arg=terminal=--title) ;;
        xterm) arguments+=(--session-app-arg=terminal=-title) ;;
        *)
            echo "Unsupported Sophia terminal kind: $kind" >&2
            return 1
            ;;
    esac
    arguments+=("--session-app-arg=terminal=$title")
}
