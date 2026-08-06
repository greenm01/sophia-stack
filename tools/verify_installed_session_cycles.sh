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
(( $# <= 2 )) || {
    echo "usage: $0 POSITIVE_COUNT [THROUGH_RUN]" >&2
    exit 1
}
required="${1:-3}"
[[ "$required" =~ ^[1-9][0-9]*$ ]] || {
    echo "usage: $0 POSITIVE_COUNT [THROUGH_RUN]" >&2
    exit 1
}
through="${2:-}"
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
RUN_ROOT="${SOPHIA_PROMOTION_RUN_ROOT:-$STATE_HOME/sophia/promotion/runs}"
[[ -d "$RUN_ROOT" ]] || {
    echo "installed cycle evidence directory is missing: $RUN_ROOT" >&2
    exit 1
}
mapfile -t runs < <(find "$RUN_ROOT" -mindepth 1 -maxdepth 1 -type d | sort -V)
if [[ -n "$through" ]]; then
    # Resolve first so an alternate spelling cannot select outside the ledger.
    resolved_root="$(readlink -f "$RUN_ROOT")"
    if [[ "$through" == */* ]]; then
        resolved_through="$(readlink -f "$through")"
    else
        resolved_through="$(readlink -f "$RUN_ROOT/$through")"
    fi
    [[ -d "$resolved_through" \
        && "$(dirname "$resolved_through")" == "$resolved_root" ]] || {
        echo "historical cycle endpoint is not a direct ledger run: $through" >&2
        exit 1
    }
    through_index=-1
    for index in "${!runs[@]}"; do
        if [[ "$(readlink -f "${runs[$index]}")" == "$resolved_through" ]]; then
            through_index="$index"
            break
        fi
    done
    (( through_index >= 0 )) || {
        echo "historical cycle endpoint is not in the ledger: $through" >&2
        exit 1
    }
    (( through_index + 1 >= required )) || {
        echo "installed cycle gate has only $((through_index + 1)) runs through $through; $required required" >&2
        exit 1
    }
    start_index=$((through_index + 1 - required))
    # Keep the gate contiguous: the endpoint and its immediate predecessors.
    runs=("${runs[@]:start_index:required}")
else
    (( ${#runs[@]} >= required )) || {
        echo "installed cycle gate has ${#runs[@]} runs; $required required" >&2
        exit 1
    }
    runs=("${runs[@]: -required}")
fi
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
    sophia_binary_sha256="$(sed -n 's/^sophia_binary_sha256=//p' "$run/manifest")"
    "$VERIFY_IDENTITY" "$run/runtime-identity.log" "$sophia_binary_sha256"
    "$VERIFY_LIFECYCLE" "$run/lifecycle.log" normal
    record_schema="$(sed -n 's/^record_schema=//p' "$run/manifest")"
    case "$record_schema" in
        4)
            [[ "$(sed -n 's/^record_kind=//p' "$run/manifest")" == normal ]] || {
                echo "run is not a normal installed cycle: $run" >&2
                exit 1
            }
            ;;
        *)
            echo "run has no supported record schema: $run" >&2
            exit 1
            ;;
    esac
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

echo "installed Sophia cycle gate passed: runs=$required through=$(basename "${runs[-1]}") commit=$expected_commit"
