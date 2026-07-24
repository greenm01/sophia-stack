#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" && pwd)"
VERIFY_XMONAD="${SOPHIA_VERIFY_XMONAD_BIN:-$SCRIPT_DIR/sophia-verify-xmonad-run}"
if [[ ! -x "$VERIFY_XMONAD" && -x "$SCRIPT_DIR/verify_sophia_xmonad_tty3.sh" ]]; then
    VERIFY_XMONAD="$SCRIPT_DIR/verify_sophia_xmonad_tty3.sh"
fi
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
LOG_DIR="${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"
GUARD_LOG="${2:-$LOG_DIR/input-guard.log}"
RECOVERY_LOG="${3:-$LOG_DIR/recovery.log}"

"$VERIFY_XMONAD" \
    "$SESSION_LOG" "$GUARD_LOG" "$RECOVERY_LOG"
fail() {
    echo "physical Firefox verification failed: $*" >&2
    exit 1
}
count() {
    grep -Ec "$1" "$SESSION_LOG" || true
}
(( $(count '^sophia_session_app schema=1 status=started id=firefox source=action$') >= 2 )) ||
    fail "Firefox was not action-launched twice"
(( $(count '^sophia_session_app schema=1 status=exited id=firefox source=managed exit_status=exit status: 0$') >= 2 )) ||
    fail "Firefox did not exit successfully twice"
grep -Eq '^sophia_firefox_m8 schema=1 status=page_ready .* content=redacted$' \
    "$SESSION_LOG" || fail "offline Firefox page never became ready"
for stage in loaded keyboard clipboard primary resize dialog; do
    [[ "$(count "^sophia_firefox_m8 schema=1 status=stage_complete stage=$stage ")" == 1 ]] ||
        fail "Firefox stage did not complete exactly once: $stage"
done
complete="$(
    grep -E '^sophia_firefox_m8 schema=1 status=complete stages=6 ' "$SESSION_LOG" |
        tail -n 1
)"
[[ -n "$complete" ]] || fail "Firefox six-stage proof did not complete"
for field in selection_owner_changes selection_conversions; do
    value="$(
        for token in $complete; do
            if [[ "$token" == "$field="* ]]; then
                printf '%s\n' "${token#*=}"
                break
            fi
        done
    )"
    [[ "$value" =~ ^[0-9]+$ ]] && (( value >= 2 )) ||
        fail "$field is below two"
done
grep -Eq '^sophia_live_session_protocol_errors schema=1 expected=[0-9]+ unexpected=0$' \
    "$SESSION_LOG" || fail "protocol-error summary is missing or nonzero"

echo "physical Firefox workflow verified: $SESSION_LOG"
