#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" && pwd)"
VERIFY_XMONAD="${SOPHIA_VERIFY_XMONAD_BIN:-$SCRIPT_DIR/sophia-verify-xmonad-run}"
if [[ ! -x "$VERIFY_XMONAD" && -x "$SCRIPT_DIR/verify_sophia_xmonad_tty3.sh" ]]; then
    VERIFY_XMONAD="$SCRIPT_DIR/verify_sophia_xmonad_tty3.sh"
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
for run in "${runs[@]}"; do
    (
        cd "$run"
        sha256sum -c SHA256SUMS
    )
    "$VERIFY_XMONAD" \
        "$run/session.log" "$run/input-guard.log" "$run/recovery.log"
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
done

echo "installed Sophia cycle gate passed: runs=$required commit=$expected_commit"
