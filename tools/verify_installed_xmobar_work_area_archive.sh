#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" && pwd)"
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
RUN_ROOT="${SOPHIA_XTERM_RUN_ROOT:-$STATE_HOME/sophia/promotion/xterm-runs}"
run="${1:-}"
if [[ -z "$run" ]]; then
    mapfile -t runs < <(find "$RUN_ROOT" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort)
    (( ${#runs[@]} > 0 )) || {
        echo "xmobar/work-area archive verification failed: no xterm archives in $RUN_ROOT" >&2
        exit 1
    }
    run="${runs[-1]}"
fi
run="$(readlink -f "$run")"
[[ -d "$run" ]] || {
    echo "xmobar/work-area archive verification failed: missing archive: $run" >&2
    exit 1
}

VERIFY_XTERM="${SOPHIA_VERIFY_XTERM_BIN:-$SCRIPT_DIR/sophia-verify-xterm-run}"
VERIFY_XMOBAR="${SOPHIA_VERIFY_XMOBAR_WORK_AREA_SESSION_BIN:-$SCRIPT_DIR/sophia-verify-xmobar-work-area-session}"
VERIFY_IDENTITY="${SOPHIA_VERIFY_IDENTITY_BIN:-$SCRIPT_DIR/sophia-verify-runtime-identity}"
VERIFY_LIFECYCLE="${SOPHIA_VERIFY_LIFECYCLE_BIN:-$SCRIPT_DIR/sophia-verify-lifecycle}"
[[ -x "$VERIFY_XTERM" ]] || VERIFY_XTERM="$SCRIPT_DIR/verify_installed_xterm_session.sh"
[[ -x "$VERIFY_XMOBAR" ]] || VERIFY_XMOBAR="$SCRIPT_DIR/verify_sophia_xmobar_work_area_session.sh"
[[ -x "$VERIFY_IDENTITY" ]] || VERIFY_IDENTITY="$SCRIPT_DIR/verify_installed_runtime_identity.sh"
[[ -x "$VERIFY_LIFECYCLE" ]] || VERIFY_LIFECYCLE="$SCRIPT_DIR/verify_installed_session_lifecycle.sh"

(cd "$run" && sha256sum -c SHA256SUMS)
[[ "$(sed -n 's/^record_schema=//p' "$run/manifest")" == 4 \
    && "$(sed -n 's/^record_kind=//p' "$run/manifest")" == xterm ]] || {
    echo "xmobar/work-area archive verification failed: archive is not an automatic xterm record" >&2
    exit 1
}
grep -Fxq 'sophia_installed_xterm schema=1 status=passed exit_status=0' \
    "$run/result.kdl" || {
    echo "xmobar/work-area archive verification failed: xterm attempt did not pass" >&2
    exit 1
}
"$VERIFY_XTERM" "$run/session.log" "$run/input-guard.log" "$run/recovery.log"
"$VERIFY_XMOBAR" "$run/session.log"
recorded_binary="$(sed -n 's/^sophia_binary_sha256=//p' "$run/manifest")"
"$VERIFY_IDENTITY" "$run/runtime-identity.log" "$recorded_binary"
"$VERIFY_LIFECYCLE" "$run/lifecycle.log" normal
grep -Eq '^sophia_runtime_identity schema=2 kind=application name=xmobar version=[^ ]+ digest=[0-9a-f]{64}$' \
    "$run/runtime-identity.log" || {
    echo "xmobar/work-area archive verification failed: xmobar executable identity is missing" >&2
    exit 1
}

commit="$(sed -n 's/^commit=//p' "$run/manifest" | head -n 1)"
echo "installed xmobar/work-area archive gate passed: run=$run commit=$commit"
