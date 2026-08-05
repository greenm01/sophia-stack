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
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
SESSION_DIR="$STATE_HOME/sophia/xmonad-session"
IDENTITY_LOG="$STATE_HOME/sophia/installed-session/launch.log"
RUNTIME_IDENTITY_LOG="$STATE_HOME/sophia/installed-session/runtime-identity.log"
RUN_ROOT="${SOPHIA_PROMOTION_RUN_ROOT:-$STATE_HOME/sophia/promotion/runs}"
PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"

"$VERIFY_LOGIN" \
    "$SESSION_DIR/session.log" \
    "$SESSION_DIR/input-guard.log" \
    "$SESSION_DIR/recovery.log"
[[ -s "$IDENTITY_LOG" ]] || {
    echo "installed session identity log is missing: $IDENTITY_LOG" >&2
    exit 1
}
[[ -s "$RUNTIME_IDENTITY_LOG" ]] || {
    echo "installed runtime identity log is missing: $RUNTIME_IDENTITY_LOG" >&2
    exit 1
}
"$VERIFY_IDENTITY" "$RUNTIME_IDENTITY_LOG"
"$VERIFY_LIFECYCLE" "$SESSION_DIR/lifecycle.log" normal
identity="$(tail -n 1 "$IDENTITY_LOG")"
[[ "$identity" == "sophia_installed_session schema=1 status=starting "* ]] || {
    echo "installed session identity is malformed" >&2
    exit 1
}
commit="$(sed -n 's/^commit=//p' "$PREFIX/current/manifest" | head -n 1)"
[[ -n "$commit" && " $identity " == *" commit=$commit "* ]] || {
    echo "installed session identity does not match $PREFIX/current" >&2
    exit 1
}
started_at_utc=""
for token in $identity; do
    [[ "$token" != started_at_utc=* ]] || started_at_utc="${token#*=}"
done
[[ -n "$started_at_utc" ]] || {
    echo "installed session identity has no start time" >&2
    exit 1
}
launch_identity_sha256="$(sha256sum "$IDENTITY_LOG" | awk '{ print $1 }')"
(
    cd "$PREFIX/current"
    sha256sum -c SHA256SUMS
)

install -d -m 700 "$RUN_ROOT"
mapfile -t duplicate_runs < <(
    grep -rlFx --include=manifest \
        "launch_identity_sha256=$launch_identity_sha256" "$RUN_ROOT" 2>/dev/null || true
)
(( ${#duplicate_runs[@]} == 0 )) || {
    echo "installed session was already recorded: ${duplicate_runs[0]%/manifest}" >&2
    exit 1
}
sequence=1
while [[ -e "$RUN_ROOT/$(printf '%04d' "$sequence")" ]]; do
    sequence=$((sequence + 1))
done
run_dir="$RUN_ROOT/$(printf '%04d' "$sequence")"
install -d -m 700 "$run_dir"
install -m 600 "$SESSION_DIR/session.log" "$run_dir/session.log"
install -m 600 "$SESSION_DIR/input-guard.log" "$run_dir/input-guard.log"
grep -E '^sophia_tty_recovery schema=3 profile=xmonad ' \
    "$SESSION_DIR/recovery.log" | tail -n 1 >"$run_dir/recovery.log"
install -m 600 "$IDENTITY_LOG" "$run_dir/identity.log"
install -m 600 "$RUNTIME_IDENTITY_LOG" "$run_dir/runtime-identity.log"
install -m 600 "$SESSION_DIR/lifecycle.log" "$run_dir/lifecycle.log"
install -m 600 "$PREFIX/current/manifest" "$run_dir/manifest"
printf 'record_schema=1\nsession_started_at_utc=%s\nlaunch_identity_sha256=%s\n' \
    "$started_at_utc" "$launch_identity_sha256" >>"$run_dir/manifest"
(
    cd "$run_dir"
    sha256sum ./*.log ./manifest >SHA256SUMS
)

echo "Recorded verified installed Sophia run: $run_dir"
