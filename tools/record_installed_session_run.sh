#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" && pwd)"
VERIFY_XMONAD="${SOPHIA_VERIFY_XMONAD_BIN:-$SCRIPT_DIR/sophia-verify-xmonad-run}"
VERIFY_IDENTITY="${SOPHIA_VERIFY_IDENTITY_BIN:-$SCRIPT_DIR/sophia-verify-runtime-identity}"
if [[ ! -x "$VERIFY_XMONAD" && -x "$SCRIPT_DIR/verify_sophia_xmonad_tty3.sh" ]]; then
    VERIFY_XMONAD="$SCRIPT_DIR/verify_sophia_xmonad_tty3.sh"
fi
if [[ ! -x "$VERIFY_IDENTITY" && -x "$SCRIPT_DIR/verify_installed_runtime_identity.sh" ]]; then
    VERIFY_IDENTITY="$SCRIPT_DIR/verify_installed_runtime_identity.sh"
fi
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
SESSION_DIR="$STATE_HOME/sophia/xmonad-session"
IDENTITY_LOG="$STATE_HOME/sophia/installed-session/launch.log"
RUNTIME_IDENTITY_LOG="$STATE_HOME/sophia/installed-session/runtime-identity.log"
RUN_ROOT="${SOPHIA_PROMOTION_RUN_ROOT:-$STATE_HOME/sophia/promotion/runs}"
PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"

"$VERIFY_XMONAD" \
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
install -m 600 "$IDENTITY_LOG" "$run_dir/identity.log"
install -m 600 "$RUNTIME_IDENTITY_LOG" "$run_dir/runtime-identity.log"
install -m 600 "$PREFIX/current/manifest" "$run_dir/manifest"
(
    cd "$run_dir"
    sha256sum ./*.log ./manifest >SHA256SUMS
)

echo "Recorded verified installed Sophia run: $run_dir"
