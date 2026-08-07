#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
RELEASE_DIR="$(cd "$(dirname "$SCRIPT_PATH")/.." && pwd)"
VERIFY_SOAK="${SOPHIA_VERIFY_SOAK_SESSION_BIN:-$RELEASE_DIR/bin/sophia-verify-soak-session}"
VERIFY_LOGIN="${SOPHIA_VERIFY_LOGIN_BIN:-$RELEASE_DIR/bin/sophia-verify-login-cycle}"
VERIFY_IDENTITY="${SOPHIA_VERIFY_IDENTITY_BIN:-$RELEASE_DIR/bin/sophia-verify-runtime-identity}"
VERIFY_LIFECYCLE="${SOPHIA_VERIFY_LIFECYCLE_BIN:-$RELEASE_DIR/bin/sophia-verify-lifecycle}"
[[ -x "$VERIFY_SOAK" ]] || VERIFY_SOAK="$RELEASE_DIR/tools/verify_installed_session_soak.sh"
[[ -x "$VERIFY_LOGIN" ]] || VERIFY_LOGIN="$RELEASE_DIR/tools/verify_installed_login_cycle.sh"
[[ -x "$VERIFY_IDENTITY" ]] || VERIFY_IDENTITY="$RELEASE_DIR/tools/verify_installed_runtime_identity.sh"
[[ -x "$VERIFY_LIFECYCLE" ]] || VERIFY_LIFECYCLE="$RELEASE_DIR/tools/verify_installed_session_lifecycle.sh"

STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
RUN_ROOT="${SOPHIA_PROMOTION_RUN_ROOT:-$STATE_HOME/sophia/promotion/runs}"
run=""
minimum_msec=7200000
minimum_terminals=10
minimum_firefox=5
case "${1:-}" in
    "")
        (( $# == 0 )) || {
            echo "usage: $0 [RUN_DIRECTORY] [MIN_MSEC [MIN_TERMINALS [MIN_FIREFOX]]]" >&2
            exit 1
        }
        ;;
    *[!0-9]*)
        (( $# <= 4 )) || {
            echo "usage: $0 [RUN_DIRECTORY] [MIN_MSEC [MIN_TERMINALS [MIN_FIREFOX]]]" >&2
            exit 1
        }
        run="$1"
        minimum_msec="${2:-$minimum_msec}"
        minimum_terminals="${3:-$minimum_terminals}"
        minimum_firefox="${4:-$minimum_firefox}"
        ;;
    *)
        (( $# <= 3 )) || {
            echo "usage: $0 [RUN_DIRECTORY] [MIN_MSEC [MIN_TERMINALS [MIN_FIREFOX]]]" >&2
            exit 1
        }
        minimum_msec="$1"
        minimum_terminals="${2:-$minimum_terminals}"
        minimum_firefox="${3:-$minimum_firefox}"
        ;;
esac
if [[ -z "$run" ]]; then
    run="$(
        find "$RUN_ROOT" -mindepth 1 -maxdepth 1 -type d 2>/dev/null |
            sort -V |
            tail -n 1 || true
    )"
fi
[[ -n "$run" && -d "$run" ]] || {
    echo "installed soak evidence is missing: ${run:-$RUN_ROOT}" >&2
    exit 1
}

(
    cd "$run"
    sha256sum -c SHA256SUMS
)
grep -Fxq 'sophia_installed_cycle schema=1 status=passed exit_status=0' \
    "$run/result.kdl" || {
    echo "installed soak attempt did not pass: $run" >&2
    exit 1
}
[[ "$(sed -n 's/^record_schema=//p' "$run/manifest")" == 4 \
    && "$(sed -n 's/^record_kind=//p' "$run/manifest")" == normal ]] || {
    echo "installed soak has no supported normal-run contract: $run" >&2
    exit 1
}

sophia_binary_sha256="$(sed -n 's/^sophia_binary_sha256=//p' "$run/manifest")"
"$VERIFY_LOGIN" \
    "$run/session.log" "$run/input-guard.log" "$run/recovery.log"
"$VERIFY_IDENTITY" "$run/runtime-identity.log" "$sophia_binary_sha256"
"$VERIFY_LIFECYCLE" "$run/lifecycle.log" normal

# A soak is a daily-driver gate, so unlike recovery-only archives it requires
# every exercised application binary to have an exact retained digest.
for application in sophia kitty firefox xmonad xmobar; do
    application_lines="$(
        grep -Ec "^sophia_runtime_identity schema=2 kind=application name=$application " \
            "$run/runtime-identity.log" || true
    )"
    exact_lines="$(
        grep -Ec "^sophia_runtime_identity schema=2 kind=application name=$application version=[^ ]+ digest=[0-9a-f]{64}$" \
            "$run/runtime-identity.log" || true
    )"
    (( application_lines == 1 && exact_lines == 1 )) || {
        echo "installed soak lacks one exact $application binary identity" >&2
        exit 1
    }
done

commit="$(sed -n 's/^commit=//p' "$run/manifest" | head -n 1)"
identity="$(tail -n 1 "$run/identity.log")"
[[ -n "$commit" \
    && "$identity" == "sophia_installed_session schema=1 status=starting "* \
    && " $identity " == *" profile=xmonad "* \
    && " $identity " == *" commit=$commit "* ]] || {
    echo "installed soak launch identity does not match its release: $run" >&2
    exit 1
}
started_at_utc="$(sed -n 's/^session_started_at_utc=//p' "$run/manifest")"
[[ -n "$started_at_utc" \
    && " $identity " == *" started_at_utc=$started_at_utc "* ]] || {
    echo "installed soak start time does not match its identity: $run" >&2
    exit 1
}
expected_identity_sha256="$(sed -n 's/^launch_identity_sha256=//p' "$run/manifest")"
observed_identity_sha256="$(sha256sum "$run/identity.log" | awk '{ print $1 }')"
[[ "$expected_identity_sha256" == "$observed_identity_sha256" ]] || {
    echo "installed soak launch-identity digest does not match: $run" >&2
    exit 1
}

"$VERIFY_SOAK" \
    "$run/session.log" "$minimum_msec" "$minimum_terminals" "$minimum_firefox"
echo "installed Sophia soak archive passed: run=$run commit=$commit"
