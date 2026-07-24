#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" && pwd)"
VERIFY_EMERGENCY="${SOPHIA_VERIFY_EMERGENCY_BIN:-$SCRIPT_DIR/sophia-verify-emergency-run}"
VERIFY_IDENTITY="${SOPHIA_VERIFY_IDENTITY_BIN:-$SCRIPT_DIR/sophia-verify-runtime-identity}"
VERIFY_LIFECYCLE="${SOPHIA_VERIFY_LIFECYCLE_BIN:-$SCRIPT_DIR/sophia-verify-lifecycle}"
if [[ ! -x "$VERIFY_EMERGENCY" && -x "$SCRIPT_DIR/verify_sophia_xmonad_emergency_tty3.sh" ]]; then
    VERIFY_EMERGENCY="$SCRIPT_DIR/verify_sophia_xmonad_emergency_tty3.sh"
fi
if [[ ! -x "$VERIFY_IDENTITY" && -x "$SCRIPT_DIR/verify_installed_runtime_identity.sh" ]]; then
    VERIFY_IDENTITY="$SCRIPT_DIR/verify_installed_runtime_identity.sh"
fi
if [[ ! -x "$VERIFY_LIFECYCLE" && -x "$SCRIPT_DIR/verify_installed_session_lifecycle.sh" ]]; then
    VERIFY_LIFECYCLE="$SCRIPT_DIR/verify_installed_session_lifecycle.sh"
fi

STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
SESSION_DIR="$STATE_HOME/sophia/xmonad-session"
IDENTITY_DIR="$STATE_HOME/sophia/installed-session"
RUN_ROOT="${SOPHIA_EMERGENCY_RUN_ROOT:-$STATE_HOME/sophia/promotion/emergency-runs}"
PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"

"$VERIFY_EMERGENCY" \
    "$SESSION_DIR/session.log" \
    "$SESSION_DIR/input-guard.log" \
    "$SESSION_DIR/recovery.log"
"$VERIFY_IDENTITY" "$IDENTITY_DIR/runtime-identity.log"
"$VERIFY_LIFECYCLE" "$SESSION_DIR/lifecycle.log" emergency
identity="$(tail -n 1 "$IDENTITY_DIR/launch.log")"
commit="$(sed -n 's/^commit=//p' "$PREFIX/current/manifest" | head -n 1)"
[[ -n "$commit" && " $identity " == *" commit=$commit "* ]] || {
    echo "installed emergency identity does not match $PREFIX/current" >&2
    exit 1
}
(
    cd "$PREFIX/current"
    sha256sum -c SHA256SUMS
)

install -d -m 700 "$RUN_ROOT"
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
install -m 600 "$SESSION_DIR/lifecycle.log" "$run_dir/lifecycle.log"
install -m 600 "$IDENTITY_DIR/launch.log" "$run_dir/identity.log"
install -m 600 "$IDENTITY_DIR/runtime-identity.log" "$run_dir/runtime-identity.log"
install -m 600 "$PREFIX/current/manifest" "$run_dir/manifest"
(
    cd "$run_dir"
    sha256sum ./*.log ./manifest >SHA256SUMS
)

echo "Recorded verified installed Sophia emergency run: $run_dir"
