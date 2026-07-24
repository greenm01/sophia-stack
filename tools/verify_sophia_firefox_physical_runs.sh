#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" && pwd)"
VERIFY_FIREFOX="${SOPHIA_VERIFY_FIREFOX_BIN:-$SCRIPT_DIR/sophia-verify-firefox-run}"
VERIFY_IDENTITY="${SOPHIA_VERIFY_IDENTITY_BIN:-$SCRIPT_DIR/sophia-verify-runtime-identity}"
if [[ ! -x "$VERIFY_FIREFOX" && -x "$SCRIPT_DIR/verify_sophia_firefox_physical.sh" ]]; then
    VERIFY_FIREFOX="$SCRIPT_DIR/verify_sophia_firefox_physical.sh"
fi
if [[ ! -x "$VERIFY_IDENTITY" && -x "$SCRIPT_DIR/verify_installed_runtime_identity.sh" ]]; then
    VERIFY_IDENTITY="$SCRIPT_DIR/verify_installed_runtime_identity.sh"
fi
required="${1:-3}"
[[ "$required" =~ ^[1-9][0-9]*$ ]] || {
    echo "usage: verify_sophia_firefox_physical_runs.sh POSITIVE_COUNT" >&2
    exit 1
}
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
RUN_ROOT="${SOPHIA_FIREFOX_RUN_ROOT:-$STATE_HOME/sophia/promotion/firefox-runs}"
[[ -d "$RUN_ROOT" ]] || {
    echo "physical Firefox evidence directory is missing: $RUN_ROOT" >&2
    exit 1
}
mapfile -t runs < <(find "$RUN_ROOT" -mindepth 1 -maxdepth 1 -type d | sort)
(( ${#runs[@]} >= required )) || {
    echo "physical Firefox gate has ${#runs[@]} runs; $required required" >&2
    exit 1
}
runs=("${runs[@]: -required}")
expected_commit=""
for run in "${runs[@]}"; do
    (cd "$run" && sha256sum -c SHA256SUMS)
    "$VERIFY_FIREFOX" \
        "$run/session.log" "$run/input-guard.log" "$run/recovery.log"
    [[ ! -f "$run/runtime-identity.log" ]] ||
        "$VERIFY_IDENTITY" "$run/runtime-identity.log"
    commit="$(sed -n 's/^commit=//p' "$run/manifest" | head -n 1)"
    [[ "$commit" =~ ^[0-9a-f]{40}$ ]] || {
        echo "physical Firefox run has an invalid commit: $run" >&2
        exit 1
    }
    if [[ -z "$expected_commit" ]]; then
        expected_commit="$commit"
    elif [[ "$commit" != "$expected_commit" ]]; then
        echo "physical Firefox runs span multiple commits" >&2
        exit 1
    fi
done
echo "physical Firefox three-run gate passed: runs=$required commit=$expected_commit"
