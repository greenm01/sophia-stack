#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
RELEASE_DIR="$(cd "$(dirname "$SCRIPT_PATH")/.." && pwd)"
VERIFY_SESSION="${SOPHIA_VERIFY_FALLBACK_SESSION_BIN:-$RELEASE_DIR/bin/sophia-verify-fallback-session}"
VERIFY_IDENTITY="${SOPHIA_VERIFY_IDENTITY_BIN:-$RELEASE_DIR/bin/sophia-verify-runtime-identity}"
VERIFY_LIFECYCLE="${SOPHIA_VERIFY_LIFECYCLE_BIN:-$RELEASE_DIR/bin/sophia-verify-lifecycle}"
if [[ ! -x "$VERIFY_SESSION" ]]; then
    VERIFY_SESSION="$RELEASE_DIR/tools/verify_installed_fallback_session.sh"
fi
if [[ ! -x "$VERIFY_IDENTITY" ]]; then
    VERIFY_IDENTITY="$RELEASE_DIR/tools/verify_installed_runtime_identity.sh"
fi
if [[ ! -x "$VERIFY_LIFECYCLE" ]]; then
    VERIFY_LIFECYCLE="$RELEASE_DIR/tools/verify_installed_session_lifecycle.sh"
fi

STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
RUN_ROOT="${SOPHIA_FALLBACK_RUN_ROOT:-$STATE_HOME/sophia/promotion/fallback-runs}"
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
    echo "installed fallback evidence is missing: ${run:-$RUN_ROOT}" >&2
    exit 1
}

(
    cd "$run"
    sha256sum -c SHA256SUMS
)
grep -Fxq 'sophia_installed_fallback schema=1 status=passed exit_status=0' \
    "$run/result.kdl" || {
    echo "installed fallback attempt did not pass: $run" >&2
    exit 1
}
"$VERIFY_SESSION" "$run/session.log" "$run/input-guard.log" "$run/recovery.log"
sophia_binary_sha256="$(sed -n 's/^sophia_binary_sha256=//p' "$run/manifest")"
"$VERIFY_IDENTITY" "$run/runtime-identity.log" "$sophia_binary_sha256"
"$VERIFY_LIFECYCLE" "$run/lifecycle.log" normal
[[ "$(sed -n 's/^record_schema=//p' "$run/manifest")" == 4 \
    && "$(sed -n 's/^record_kind=//p' "$run/manifest")" == fallback ]] || {
    echo "installed fallback has no supported record contract: $run" >&2
    exit 1
}
commit="$(sed -n 's/^commit=//p' "$run/manifest" | head -n 1)"
identity="$(tail -n 1 "$run/identity.log")"
[[ -n "$commit" \
    && "$identity" == "sophia_installed_session schema=1 status=starting "* \
    && " $identity " == *" profile=kitty "* \
    && " $identity " == *" commit=$commit "* ]] || {
    echo "installed fallback identity does not match its release: $run" >&2
    exit 1
}
started_at_utc="$(sed -n 's/^session_started_at_utc=//p' "$run/manifest")"
[[ -n "$started_at_utc" \
    && " $identity " == *" started_at_utc=$started_at_utc "* ]] || {
    echo "installed fallback start time does not match its identity: $run" >&2
    exit 1
}
expected_sha256="$(sed -n 's/^launch_identity_sha256=//p' "$run/manifest")"
observed_sha256="$(sha256sum "$run/identity.log" | awk '{ print $1 }')"
[[ "$expected_sha256" == "$observed_sha256" ]] || {
    echo "installed fallback launch identity digest does not match: $run" >&2
    exit 1
}

echo "installed Sophia fallback archive passed: run=$run commit=$commit"
