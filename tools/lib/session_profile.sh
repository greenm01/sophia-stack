# The Hagia adapter pairs Engine-envelope validation with WM-owned semantics.
# Normal runtime still receives only Sophia's staged Policy fragment.
sophia_check_hagia_profile() {
    local sophia="$1" hagia="$2" profile="$3"
    [[ -x "$sophia" && "$profile" == /* && -f "$profile" ]] || {
        echo "Desktop preflight requires executable Sophia and an absolute desktop profile." >&2
        return 1
    }
    timeout 10s "$sophia" config check "--desktop-profile=$profile" || return
    local selected shell_client policy status
    selected="$(timeout 10s "$sophia" config print-component \
        "--desktop-profile=$profile" --component=window-manager)" || return
    shell_client="$(timeout 10s "$sophia" config print-component \
        "--desktop-profile=$profile" --component=shell-client)" || return
    if [[ -n "$shell_client" && ! -x "$shell_client" ]]; then
        echo "Selected native shell is not executable: $shell_client" >&2
        return 1
    fi
    if [[ -n "$selected" && ! "$selected" -ef "$hagia" ]]; then
        [[ -x "$selected" ]] || {
            echo "Selected WM is not executable: $selected" >&2
            return 1
        }
        # A different WM owns its vocabulary. Its protocol activation is the
        # semantic gate; Hagia's parser cannot validate another WM's policy.
        echo "Selected WM will validate its policy during session activation."
        return 0
    fi
    [[ -x "$hagia" ]] || {
        echo "Default Hagia policy client is not executable: $hagia" >&2
        return 1
    }
    policy="$(mktemp)" || return
    chmod 600 "$policy"
    if timeout 10s "$sophia" config print-policy "--desktop-profile=$profile" >"$policy"; then
        if timeout 10s "$hagia" config check "--config=$policy"; then
            status=0
        else
            status=$?
        fi
    else
        status=$?
    fi
    rm -f -- "$policy"
    return "$status"
}
