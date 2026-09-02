#!/usr/bin/env bash
# Shared installed command/session reconciliation for activation and rollback.

sophia_surface_select_entries() {
    local release="$1" hagia_included

    hagia_included="$(awk -F= '$1 == "hagia_included" { print $2; exit }' \
        "$release/manifest")"
    case "$hagia_included" in
        true|false) ;;
        *)
            echo "Release has an invalid hagia_included value." >&2
            return 1
            ;;
    esac

    SOPHIA_SURFACE_HAGIA_INCLUDED="$hagia_included"
    SOPHIA_SURFACE_HAGIA_COMMANDS=(
        sophia-hagia-session
        sophia-hagia-promotion-session
        sophia-record-hagia-run
        sophia-verify-hagia
        sophia-verify-hagia-promotion
    )
    SOPHIA_SURFACE_HAGIA_DESKTOPS=(
        sophia-hagia
        sophia-hagia-promotion
    )
    SOPHIA_SURFACE_HAGIA_DESKTOP_COMMANDS=(
        sophia-hagia-session
        sophia-hagia-promotion-session
    )
    SOPHIA_SURFACE_COMMANDS=(
        sophia-session
        sophia-kitty-session
        sophia-firefox-proof
        sophia-xterm-proof
        sophia-truecolor-proof
        sophia-recovery-proof
        sophia-native-chrome-proof
        sophia-status
        sophia-stop
        sophia-soak-progress
        sophia-rollback
        sophia-run-cycles
        sophia-setup-uinput
        sophia-record-run
        sophia-record-fallback-run
        sophia-record-emergency-run
        sophia-record-watchdog-run
        sophia-record-native-chrome-run
        sophia-record-firefox-run
        sophia-verify-login-cycle
        sophia-verify-cycles
        sophia-verify-emergency
        sophia-verify-fallback
        sophia-verify-watchdog
        sophia-verify-native-chrome
        sophia-verify-firefox-runs
        sophia-verify-xterm-runs
        sophia-verify-truecolor-runs
        sophia-verify-xmobar-work-area
        sophia-verify-soak
    )
    SOPHIA_SURFACE_DESKTOPS=(
        sophia
        sophia-kitty
        sophia-firefox-proof
        sophia-recovery-proof
        sophia-native-chrome-proof
        sophia-cycle-proof
    )
    if [[ "$hagia_included" == true ]]; then
        SOPHIA_SURFACE_COMMANDS+=("${SOPHIA_SURFACE_HAGIA_COMMANDS[@]}")
        SOPHIA_SURFACE_DESKTOPS+=("${SOPHIA_SURFACE_HAGIA_DESKTOPS[@]}")
    fi
}

sophia_surface_validate_entries() {
    local release="$1" command desktop

    sophia_surface_select_entries "$release" || return
    for command in "${SOPHIA_SURFACE_COMMANDS[@]}"; do
        [[ -x "$release/bin/$command" ]] || {
            echo "Release is missing operator command: $command" >&2
            return 1
        }
    done
    for desktop in "${SOPHIA_SURFACE_DESKTOPS[@]}"; do
        [[ -f "$release/share/wayland-sessions/$desktop.desktop" ]] || {
            echo "Release is missing session entry: $desktop.desktop" >&2
            return 1
        }
    done
}

sophia_surface_remove_absent_hagia() {
    local prefix="$1" session_dir="$2" command_dir="$3"
    local command link desktop entry expected index

    [[ "$SOPHIA_SURFACE_HAGIA_INCLUDED" == false ]] || return 0
    for command in "${SOPHIA_SURFACE_HAGIA_COMMANDS[@]}"; do
        link="$command_dir/$command"
        if [[ -L "$link" ]] \
            && [[ "$(readlink "$link")" == "$prefix/current/bin/$command" ]]; then
            rm -f -- "$link"
        fi
    done
    for index in "${!SOPHIA_SURFACE_HAGIA_DESKTOPS[@]}"; do
        desktop="${SOPHIA_SURFACE_HAGIA_DESKTOPS[$index]}"
        command="${SOPHIA_SURFACE_HAGIA_DESKTOP_COMMANDS[$index]}"
        entry="$session_dir/$desktop.desktop"
        expected="Exec=$prefix/current/bin/$command"
        if [[ -f "$entry" && ! -L "$entry" ]] \
            && grep -Fqx "$expected" "$entry"; then
            rm -f -- "$entry"
        fi
    done
}

sophia_surface_install() {
    local release="$1" prefix="$2" session_dir="$3" command_dir="$4"
    local command desktop desktop_temp sed_prefix

    # Validate the complete target before changing the installed surface.
    sophia_surface_validate_entries "$release" || return
    install -d -m 755 "$session_dir" "$command_dir"
    sophia_surface_remove_absent_hagia "$prefix" "$session_dir" "$command_dir"
    for command in "${SOPHIA_SURFACE_COMMANDS[@]}"; do
        ln -sfn "$prefix/current/bin/$command" "$command_dir/$command"
    done

    sed_prefix="${prefix//\\/\\\\}"
    sed_prefix="${sed_prefix//&/\\&}"
    sed_prefix="${sed_prefix//|/\\|}"
    for desktop in "${SOPHIA_SURFACE_DESKTOPS[@]}"; do
        desktop_temp="$(mktemp "$session_dir/.$desktop.desktop.XXXXXX")"
        if ! sed "s|@SOPHIA_INSTALL_PREFIX@|$sed_prefix|g" \
            "$release/share/wayland-sessions/$desktop.desktop" \
            >"$desktop_temp"; then
            rm -f -- "$desktop_temp"
            return 1
        fi
        chmod 644 "$desktop_temp"
        mv -f "$desktop_temp" "$session_dir/$desktop.desktop"
    done
}
