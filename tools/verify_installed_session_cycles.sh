#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" && pwd)"
VERIFY_LOGIN="${SOPHIA_VERIFY_LOGIN_BIN:-$SCRIPT_DIR/sophia-verify-login-cycle}"
VERIFY_IDENTITY="${SOPHIA_VERIFY_IDENTITY_BIN:-$SCRIPT_DIR/sophia-verify-runtime-identity}"
VERIFY_LIFECYCLE="${SOPHIA_VERIFY_LIFECYCLE_BIN:-$SCRIPT_DIR/sophia-verify-lifecycle}"
if [[ ! -x "$VERIFY_LOGIN" && -x "$SCRIPT_DIR/verify_installed_login_cycle.sh" ]]; then
    VERIFY_LOGIN="$SCRIPT_DIR/verify_installed_login_cycle.sh"
fi
if [[ ! -x "$VERIFY_IDENTITY" && -x "$SCRIPT_DIR/verify_installed_runtime_identity.sh" ]]; then
    VERIFY_IDENTITY="$SCRIPT_DIR/verify_installed_runtime_identity.sh"
fi
if [[ ! -x "$VERIFY_LIFECYCLE" && -x "$SCRIPT_DIR/verify_installed_session_lifecycle.sh" ]]; then
    VERIFY_LIFECYCLE="$SCRIPT_DIR/verify_installed_session_lifecycle.sh"
fi
required="${1:-3}"
[[ "$required" =~ ^[1-9][0-9]*$ ]] || {
    echo "usage: tools/verify_installed_session_cycles.sh POSITIVE_COUNT" >&2
    exit 1
}
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
RUN_ROOT="${SOPHIA_PROMOTION_RUN_ROOT:-$STATE_HOME/sophia/promotion/runs}"
[[ -d "$RUN_ROOT" ]] || {
    echo "installed cycle evidence directory is missing: $RUN_ROOT" >&2
    exit 1
}
mapfile -t runs < <(find "$RUN_ROOT" -mindepth 1 -maxdepth 1 -type d | sort)
(( ${#runs[@]} >= required )) || {
    echo "installed cycle gate has ${#runs[@]} runs; $required required" >&2
    exit 1
}
runs=("${runs[@]: -required}")
expected_commit=""
declare -A seen_launch_identities=()
for run in "${runs[@]}"; do
    (
        cd "$run"
        sha256sum -c SHA256SUMS
    )
    grep -Fxq 'sophia_installed_cycle schema=1 status=passed exit_status=0' \
        "$run/result.kdl" || {
        echo "installed cycle attempt did not pass: $run" >&2
        exit 1
    }
    "$VERIFY_LOGIN" \
        "$run/session.log" "$run/input-guard.log" "$run/recovery.log"
    "$VERIFY_IDENTITY" "$run/runtime-identity.log"
    "$VERIFY_LIFECYCLE" "$run/lifecycle.log" normal
    [[ "$(sed -n 's/^record_schema=//p' "$run/manifest")" == 2 ]] || {
        echo "run has no supported record schema: $run" >&2
        exit 1
    }
    commit="$(sed -n 's/^commit=//p' "$run/manifest" | head -n 1)"
    [[ -n "$commit" ]] || {
        echo "run has no release commit: $run" >&2
        exit 1
    }
    if [[ -z "$expected_commit" ]]; then
        expected_commit="$commit"
    elif [[ "$commit" != "$expected_commit" ]]; then
        echo "installed cycle gate spans multiple commits" >&2
        exit 1
    fi
    identity="$(tail -n 1 "$run/identity.log")"
    [[ "$identity" == "sophia_installed_session schema=1 status=starting "* \
        && " $identity " == *" commit=$commit "* ]] || {
        echo "run identity does not match its release manifest: $run" >&2
        exit 1
    }
    started_at_utc="$(sed -n 's/^session_started_at_utc=//p' "$run/manifest")"
    [[ -n "$started_at_utc" && " $identity " == *" started_at_utc=$started_at_utc "* ]] || {
        echo "run start time does not match its launch identity: $run" >&2
        exit 1
    }
    launch_identity_sha256="$(sed -n 's/^launch_identity_sha256=//p' "$run/manifest")"
    observed_sha256="$(sha256sum "$run/identity.log" | awk '{ print $1 }')"
    [[ "$launch_identity_sha256" == "$observed_sha256" ]] || {
        echo "run launch-identity digest does not match: $run" >&2
        exit 1
    }
    [[ -z "${seen_launch_identities[$launch_identity_sha256]:-}" ]] || {
        echo "installed cycle gate contains a duplicate launch identity" >&2
        exit 1
    }
    seen_launch_identities[$launch_identity_sha256]="$run"
done

echo "installed Sophia cycle gate passed: runs=$required commit=$expected_commit"
