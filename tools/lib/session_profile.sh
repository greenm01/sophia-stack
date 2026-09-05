# The Hagia adapter pairs Engine-envelope validation with WM-owned semantics.
# Normal runtime still receives only Sophia's staged Policy fragment.
sophia_check_hagia_profile() {
    local sophia="$1" hagia="$2" profile="$3"
    [[ -x "$sophia" && -x "$hagia" && "$profile" == /* && -f "$profile" ]] || {
        echo "Hagia preflight requires executable Sophia/Hagia and an absolute desktop profile." >&2
        return 1
    }
    timeout 10s "$sophia" config check "--desktop-profile=$profile" || return
    timeout 10s "$hagia" config check "--config=$profile"
}
