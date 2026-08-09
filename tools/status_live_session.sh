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
for process in sophia hagia xmonad sophia-wm-demo kitty firefox xterm; do
    if pgrep -a -x "$process" 2>/dev/null; then
        found_graphical=true
    fi
done
[[ "$found_graphical" == true ]] || echo none
for profile in hagia xmonad kitty native; do
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

print_latest_attempt() {
    local label="$1" root="$2" latest
    printf '%s_attempts=%s\n' "$label" "$root"
    latest="$(
        find "$root" -mindepth 1 -maxdepth 1 -type d 2>/dev/null |
        sort -V |
        tail -n 1 || true
    )"
    if [[ -n "$latest" ]]; then
        printf 'latest_%s=%s\n' "$label" "$latest"
        tail -n 1 "$latest/result.kdl" 2>/dev/null || true
    else
        printf 'latest_%s=none\n' "$label"
    fi
}

print_latest_attempt installed_cycle "$STATE_HOME/sophia/promotion/runs"
print_latest_attempt installed_firefox \
    "$STATE_HOME/sophia/promotion/firefox-runs"
print_latest_attempt installed_xterm \
    "$STATE_HOME/sophia/promotion/xterm-runs"
print_latest_attempt installed_truecolor \
    "$STATE_HOME/sophia/promotion/truecolor-runs"
print_latest_attempt installed_fallback \
    "$STATE_HOME/sophia/promotion/fallback-runs"
print_latest_attempt installed_emergency \
    "$STATE_HOME/sophia/promotion/emergency-runs"
print_latest_attempt installed_watchdog \
    "$STATE_HOME/sophia/promotion/watchdog-runs"
print_latest_attempt installed_native_chrome \
    "$STATE_HOME/sophia/promotion/native-chrome-runs"
print_latest_attempt installed_hagia \
    "$STATE_HOME/sophia/promotion/hagia-runs"

hagia_root="$STATE_HOME/sophia/promotion/hagia-runs"
printf 'hagia_coverage_root=%s\n' "$hagia_root"
for outcome in passed recovered failed pending; do
    count="$({
        grep -rlE "^sophia_installed_hagia schema=1 status=$outcome([[:space:]]|$)" \
            "$hagia_root" --include=result.kdl 2>/dev/null || true
    } | wc -l)"
    printf 'hagia_%s_runs=%s\n' "$outcome" "$count"
done
mapfile -t hagia_coverage_files < <(
    find "$hagia_root" -mindepth 2 -maxdepth 2 -type f -name coverage.kdl \
        2>/dev/null | sort -V || true
)
for field in terminal_starts firefox_starts physical_actions session_actions \
    pointer_moves pointer_resizes checkpoints reconciliations output_changes \
    topology_changes; do
    if (( ${#hagia_coverage_files[@]} == 0 )); then
        observed=0
    else
        observed="$(awk -v field="$field" '
            {
                for (i = 1; i <= NF; i++) {
                    split($i, pair, "=")
                    if (pair[1] == field && pair[2] ~ /^[0-9]+$/ && pair[2] > 0) {
                        seen++
                        break
                    }
                }
            }
            END { print seen + 0 }
        ' "${hagia_coverage_files[@]}")"
    fi
    printf 'hagia_scenario_%s_sessions=%s\n' "$field" "$observed"
done
