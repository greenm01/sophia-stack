# Shared guard for read-only DRM checks that still need atomic validation.
#
# Atomic commits require DRM master even when they carry TEST_ONLY, so a check run
# while a compositor holds the card reports "not master" and concludes nothing. A
# run that proves nothing is worse than no run, because its output looks like a
# result. Refusing up front is the cheaper failure.

# Names the display servers that would hold the card. Not exhaustive by design:
# the DISPLAY and WAYLAND_DISPLAY checks catch what this list misses, and an
# unknown compositor still fails safely as MasterUnavailable at the ioctl.
declare -p SOPHIA_DRM_MASTER_HOLDERS >/dev/null 2>&1 ||
    declare -a SOPHIA_DRM_MASTER_HOLDERS=(
        Xorg X Xwayland xlibre-server sophia hyprland sway niri weston kwin_wayland
        gnome-shell
    )

# Prints one line per apparent holder. Empty output means the card looks free.
sophia_drm_master_blockers() {
    [[ -n "${DISPLAY:-}" ]] && echo "DISPLAY=${DISPLAY} is set"
    [[ -n "${WAYLAND_DISPLAY:-}" ]] && echo "WAYLAND_DISPLAY=${WAYLAND_DISPLAY} is set"
    local name
    for name in "${SOPHIA_DRM_MASTER_HOLDERS[@]}"; do
        if pgrep -x "$name" >/dev/null 2>&1; then
            echo "$name is running"
        fi
    done
    return 0
}

# Refuses unless the card looks free. `$1` names the override variable a caller
# honors, so the message points at the right escape hatch.
sophia_require_drm_master_available() {
    local override="${1:-SOPHIA_DRM_MASTER_FORCE}"
    local blockers
    blockers="$(sophia_drm_master_blockers)"

    if [[ ! -d /dev/dri ]]; then
        echo "/dev/dri is missing; no primary card node to use." >&2
        return 1
    fi
    if [[ -z "$blockers" ]]; then
        return 0
    fi

    echo "A display server appears to hold the card:" >&2
    sed 's/^/  - /' <<<"$blockers" >&2
    echo >&2
    if [[ "${!override:-0}" == "1" ]]; then
        echo "$override=1; running anyway." >&2
        echo >&2
        return 0
    fi
    echo "Atomic commits need DRM master even to validate, so this run would report" >&2
    echo "MasterUnavailable and conclude nothing. Switch to a bare TTY with no" >&2
    echo "compositor, or set $override=1 to run anyway." >&2
    return 1
}
