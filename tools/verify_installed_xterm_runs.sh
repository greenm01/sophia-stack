#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" && pwd)"
VERIFY_XTERM="${SOPHIA_VERIFY_XTERM_BIN:-$SCRIPT_DIR/sophia-verify-xterm-run}"
VERIFY_IDENTITY="${SOPHIA_VERIFY_IDENTITY_BIN:-$SCRIPT_DIR/sophia-verify-runtime-identity}"
VERIFY_LIFECYCLE="${SOPHIA_VERIFY_LIFECYCLE_BIN:-$SCRIPT_DIR/sophia-verify-lifecycle}"
[[ -x "$VERIFY_XTERM" ]] || VERIFY_XTERM="$SCRIPT_DIR/verify_installed_xterm_session.sh"
[[ -x "$VERIFY_IDENTITY" ]] || VERIFY_IDENTITY="$SCRIPT_DIR/verify_installed_runtime_identity.sh"
[[ -x "$VERIFY_LIFECYCLE" ]] || VERIFY_LIFECYCLE="$SCRIPT_DIR/verify_installed_session_lifecycle.sh"
required="${1:-1}"
[[ "$required" =~ ^[1-9][0-9]*$ ]] || {
    echo "usage: verify_installed_xterm_runs.sh POSITIVE_COUNT" >&2
    exit 1
}
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
RUN_ROOT="${SOPHIA_XTERM_RUN_ROOT:-$STATE_HOME/sophia/promotion/xterm-runs}"
[[ -d "$RUN_ROOT" ]] || {
    echo "installed xterm evidence directory is missing: $RUN_ROOT" >&2
    exit 1
}
mapfile -t runs < <(find "$RUN_ROOT" -mindepth 1 -maxdepth 1 -type d | sort)
(( ${#runs[@]} >= required )) || {
    echo "installed xterm gate has ${#runs[@]} runs; $required required" >&2
    exit 1
}
runs=("${runs[@]: -required}")
expected_commit=""
for run in "${runs[@]}"; do
    (cd "$run" && sha256sum -c SHA256SUMS)
    [[ "$(sed -n 's/^record_schema=//p' "$run/manifest")" == 4 \
        && "$(sed -n 's/^record_kind=//p' "$run/manifest")" == xterm ]] || {
        echo "installed xterm run has the wrong automatic record kind: $run" >&2
        exit 1
    }
    grep -Fxq 'sophia_installed_xterm schema=1 status=passed exit_status=0' \
        "$run/result.kdl" || {
        echo "installed xterm attempt did not pass: $run" >&2
        exit 1
    }
    "$VERIFY_XTERM" "$run/session.log" "$run/input-guard.log" "$run/recovery.log"
    recorded_binary="$(sed -n 's/^sophia_binary_sha256=//p' "$run/manifest")"
    [[ "$recorded_binary" =~ ^[0-9a-f]{64}$ ]] || {
        echo "installed xterm run has no Sophia executable digest: $run" >&2
        exit 1
    }
    "$VERIFY_IDENTITY" "$run/runtime-identity.log" "$recorded_binary"
    "$VERIFY_LIFECYCLE" "$run/lifecycle.log" normal
    require_xterm='^sophia_runtime_identity schema=2 kind=application name=xterm version=[^ ]+ digest=[0-9a-f]{64}$'
    grep -Eq "$require_xterm" "$run/runtime-identity.log" || {
        echo "installed xterm run has no executable identity: $run" >&2
        exit 1
    }
    commit="$(sed -n 's/^commit=//p' "$run/manifest" | head -n 1)"
    [[ "$commit" =~ ^[0-9a-f]{40}$ ]] || {
        echo "installed xterm run has an invalid commit: $run" >&2
        exit 1
    }
    identity="$(tail -n 1 "$run/identity.log")"
    [[ "$identity" == "sophia_installed_session schema=1 status=starting profile=hagia "* \
        && " $identity " == *" commit=$commit "* ]] || {
        echo "installed xterm launch identity does not match its manifest: $run" >&2
        exit 1
    }
    launch_digest="$(sha256sum "$run/identity.log" | awk '{ print $1 }')"
    [[ "$launch_digest" == \
        "$(sed -n 's/^launch_identity_sha256=//p' "$run/manifest")" ]] || {
        echo "installed xterm launch identity digest does not match: $run" >&2
        exit 1
    }
    if [[ -z "$expected_commit" ]]; then
        expected_commit="$commit"
    elif [[ "$commit" != "$expected_commit" ]]; then
        echo "installed xterm runs span multiple commits" >&2
        exit 1
    fi
done
echo "installed xterm run-set gate passed: runs=$required commit=$expected_commit"
