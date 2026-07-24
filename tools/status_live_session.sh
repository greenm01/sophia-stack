#!/usr/bin/env bash
set -euo pipefail

PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"
current="$(readlink "$PREFIX/current" 2>/dev/null || true)"
previous="$(readlink "$PREFIX/previous" 2>/dev/null || true)"
printf 'sophia_install_status schema=1 prefix=%s current=%s previous=%s\n' \
    "$PREFIX" "${current:-none}" "${previous:-none}"
if [[ -n "$current" && -f "$PREFIX/$current/manifest" ]]; then
    sed -n '1,20p' "$PREFIX/$current/manifest"
    (
        cd "$PREFIX/$current"
        sha256sum -c SHA256SUMS
    )
fi
printf 'graphical_processes='
found_graphical=false
for process in sophia xmonad kitty firefox; do
    if pgrep -a -x "$process" 2>/dev/null; then
        found_graphical=true
    fi
done
[[ "$found_graphical" == true ]] || echo none
for profile in xmonad kitty; do
    state="${XDG_STATE_HOME:-$HOME/.local/state}/sophia/$profile-session"
    printf '%s_logs=%s\n' "$profile" "$state"
    tail -n 1 "$state/lifecycle.log" 2>/dev/null || true
    tail -n 1 "$state/recovery.log" 2>/dev/null || true
done
runtime_identity="${XDG_STATE_HOME:-$HOME/.local/state}/sophia/installed-session/runtime-identity.log"
printf 'runtime_identity=%s\n' "$runtime_identity"
[[ ! -s "$runtime_identity" ]] || cat "$runtime_identity"
