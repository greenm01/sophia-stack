#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
RELEASE_DIR="$(cd "$(dirname "$SCRIPT_PATH")/.." && pwd)"
VERIFY_WATCHDOG="${SOPHIA_VERIFY_WATCHDOG_BIN:-$RELEASE_DIR/bin/sophia-verify-watchdog-run}"
VERIFY_IDENTITY="${SOPHIA_VERIFY_IDENTITY_BIN:-$RELEASE_DIR/bin/sophia-verify-runtime-identity}"
if [[ ! -x "$VERIFY_WATCHDOG" ]]; then
    VERIFY_WATCHDOG="$RELEASE_DIR/tools/verify_installed_watchdog_recovery.sh"
fi
if [[ ! -x "$VERIFY_IDENTITY" ]]; then
    VERIFY_IDENTITY="$RELEASE_DIR/tools/verify_installed_runtime_identity.sh"
fi

STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
RUN_ROOT="${SOPHIA_WATCHDOG_RUN_ROOT:-$STATE_HOME/sophia/promotion/watchdog-runs}"
if (( $# > 1 )); then
    echo "usage: $0 [RUN_DIRECTORY]" >&2
    exit 1
fi
run="${1:-}"
if [[ -z "$run" ]]; then
    run="$(
        find "$RUN_ROOT" -mindepth 1 -maxdepth 1 -type d 2>/dev/null |
            sort -V |
            tail -n 1 || true
    )"
fi
[[ -n "$run" && -d "$run" ]] || {
    echo "installed watchdog evidence is missing: ${run:-$RUN_ROOT}" >&2
    exit 1
}

(
    cd "$run"
    sha256sum -c SHA256SUMS
)
grep -Fxq 'sophia_installed_watchdog schema=1 status=passed exit_status=124' \
    "$run/result.kdl" || {
    echo "installed watchdog attempt did not pass: $run" >&2
    exit 1
}
"$VERIFY_WATCHDOG" \
    "$run/session.log" \
    "$run/input-guard.log" \
    "$run/recovery.log" \
    "$run/lifecycle.log"
sophia_binary_sha256="$(sed -n 's/^sophia_binary_sha256=//p' "$run/manifest")"
"$VERIFY_IDENTITY" "$run/runtime-identity.log" "$sophia_binary_sha256"
[[ "$(sed -n 's/^record_schema=//p' "$run/manifest")" == 4 \
    && "$(sed -n 's/^record_kind=//p' "$run/manifest")" == watchdog ]] || {
    echo "installed watchdog has no supported record contract: $run" >&2
    exit 1
}
commit="$(sed -n 's/^commit=//p' "$run/manifest" | head -n 1)"
identity="$(tail -n 1 "$run/identity.log")"
[[ -n "$commit" \
    && "$identity" == "sophia_installed_session schema=1 status=starting "* \
    && " $identity " == *" profile=xmonad "* \
    && " $identity " == *" commit=$commit "* ]] || {
    echo "installed watchdog identity does not match its release: $run" >&2
    exit 1
}
started_at_utc="$(sed -n 's/^session_started_at_utc=//p' "$run/manifest")"
[[ -n "$started_at_utc" \
    && " $identity " == *" started_at_utc=$started_at_utc "* ]] || {
    echo "installed watchdog start time does not match its identity: $run" >&2
    exit 1
}
expected_sha256="$(sed -n 's/^launch_identity_sha256=//p' "$run/manifest")"
observed_sha256="$(sha256sum "$run/identity.log" | awk '{ print $1 }')"
[[ "$expected_sha256" == "$observed_sha256" ]] || {
    echo "installed watchdog launch identity digest does not match: $run" >&2
    exit 1
}

echo "installed Sophia watchdog archive passed: run=$run commit=$commit"
