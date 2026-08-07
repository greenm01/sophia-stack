#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" && pwd)"
VERIFY_TRUECOLOR="${SOPHIA_VERIFY_TRUECOLOR_BIN:-$SCRIPT_DIR/sophia-verify-truecolor-run}"
VERIFY_IDENTITY="${SOPHIA_VERIFY_IDENTITY_BIN:-$SCRIPT_DIR/sophia-verify-runtime-identity}"
VERIFY_LIFECYCLE="${SOPHIA_VERIFY_LIFECYCLE_BIN:-$SCRIPT_DIR/sophia-verify-lifecycle}"
[[ -x "$VERIFY_TRUECOLOR" ]] || VERIFY_TRUECOLOR="$SCRIPT_DIR/verify_installed_truecolor_session.sh"
[[ -x "$VERIFY_IDENTITY" ]] || VERIFY_IDENTITY="$SCRIPT_DIR/verify_installed_runtime_identity.sh"
[[ -x "$VERIFY_LIFECYCLE" ]] || VERIFY_LIFECYCLE="$SCRIPT_DIR/verify_installed_session_lifecycle.sh"
required="${1:-1}"
[[ "$required" =~ ^[1-9][0-9]*$ ]] || {
    echo "usage: verify_installed_truecolor_runs.sh POSITIVE_COUNT" >&2
    exit 1
}
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
RUN_ROOT="${SOPHIA_TRUECOLOR_RUN_ROOT:-$STATE_HOME/sophia/promotion/truecolor-runs}"
[[ -d "$RUN_ROOT" ]] || {
    echo "installed TrueColor evidence directory is missing: $RUN_ROOT" >&2
    exit 1
}
mapfile -t runs < <(find "$RUN_ROOT" -mindepth 1 -maxdepth 1 -type d | sort)
(( ${#runs[@]} >= required )) || {
    echo "installed TrueColor gate has ${#runs[@]} runs; $required required" >&2
    exit 1
}
runs=("${runs[@]: -required}")
expected_commit=""
reverified=0
for run in "${runs[@]}"; do
    (cd "$run" && sha256sum -c SHA256SUMS)
    [[ "$(sed -n 's/^record_schema=//p' "$run/manifest")" == 4 \
        && "$(sed -n 's/^record_kind=//p' "$run/manifest")" == truecolor ]] || {
        echo "installed TrueColor run has the wrong automatic record kind: $run" >&2
        exit 1
    }
    result="$(<"$run/result.kdl")"
    case "$result" in
        'sophia_installed_truecolor schema=1 status=passed exit_status=0') ;;
        'sophia_installed_truecolor schema=1 status=failed exit_status=0 reason=session_verification')
            reverified=$((reverified + 1))
            ;;
        *)
            echo "installed TrueColor attempt did not complete normally: $run" >&2
            exit 1
            ;;
    esac
    "$VERIFY_TRUECOLOR" "$run/session.log" "$run/input-guard.log" "$run/recovery.log"
    recorded_binary="$(sed -n 's/^sophia_binary_sha256=//p' "$run/manifest")"
    [[ "$recorded_binary" =~ ^[0-9a-f]{64}$ ]] || {
        echo "installed TrueColor run has no Sophia executable digest: $run" >&2
        exit 1
    }
    "$VERIFY_IDENTITY" "$run/runtime-identity.log" "$recorded_binary"
    "$VERIFY_LIFECYCLE" "$run/lifecycle.log" normal
    for application in kitty xmobar; do
        grep -Eq "^sophia_runtime_identity schema=2 kind=application name=${application} version=[^ ]+ digest=[0-9a-f]{64}$" \
            "$run/runtime-identity.log" || {
            echo "installed TrueColor run has no $application executable identity: $run" >&2
            exit 1
        }
    done
    commit="$(sed -n 's/^commit=//p' "$run/manifest" | head -n 1)"
    [[ "$commit" =~ ^[0-9a-f]{40}$ ]] || {
        echo "installed TrueColor run has an invalid commit: $run" >&2
        exit 1
    }
    identity="$(tail -n 1 "$run/identity.log")"
    [[ "$identity" == "sophia_installed_session schema=1 status=starting profile=xmonad "* \
        && " $identity " == *" commit=$commit "* ]] || {
        echo "installed TrueColor launch identity does not match its manifest: $run" >&2
        exit 1
    }
    if [[ -z "$expected_commit" ]]; then
        expected_commit="$commit"
    elif [[ "$commit" != "$expected_commit" ]]; then
        echo "installed TrueColor runs span multiple commits" >&2
        exit 1
    fi
done
echo "installed TrueColor run-set gate passed: runs=$required reverified=$reverified commit=$expected_commit"
