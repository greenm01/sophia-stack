#!/usr/bin/env bash
set -euo pipefail

PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
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
operator_guide="$PREFIX/current/share/doc/sophia/operations.md"
printf 'operator_guide=%s\n' "$operator_guide"
printf 'graphical_processes='
found_graphical=false
for process in sophia xmonad kitty firefox; do
    if pgrep -a -x "$process" 2>/dev/null; then
        found_graphical=true
    fi
done
[[ "$found_graphical" == true ]] || echo none
for profile in xmonad kitty; do
    state="$STATE_HOME/sophia/$profile-session"
    lifecycle="$state/lifecycle.log"
    printf '%s_logs=%s\n' "$profile" "$state"
    latest_diagnostic="$(
        grep '^sophia_session_diagnostic schema=1 ' "$lifecycle" 2>/dev/null |
            tail -n 1 || true
    )"
    latest_lifecycle="$(tail -n 1 "$lifecycle" 2>/dev/null || true)"
    [[ -z "$latest_diagnostic" ]] || printf '%s\n' "$latest_diagnostic"
    if [[ -n "$latest_lifecycle" && "$latest_lifecycle" != "$latest_diagnostic" ]]; then
        printf '%s\n' "$latest_lifecycle"
    fi
    tail -n 1 "$state/recovery.log" 2>/dev/null || true
done
runtime_identity="$STATE_HOME/sophia/installed-session/runtime-identity.log"
printf 'runtime_identity=%s\n' "$runtime_identity"
[[ ! -s "$runtime_identity" ]] || cat "$runtime_identity"

attempt_root="$STATE_HOME/sophia/promotion/runs"
printf 'installed_cycle_attempts=%s\n' "$attempt_root"
latest_attempt="$(
    find "$attempt_root" -mindepth 1 -maxdepth 1 -type d 2>/dev/null |
        sort -V |
        tail -n 1 || true
)"
if [[ -n "$latest_attempt" ]]; then
    printf 'latest_installed_cycle=%s\n' "$latest_attempt"
    tail -n 1 "$latest_attempt/result.kdl" 2>/dev/null || true
else
    echo 'latest_installed_cycle=none'
fi
