#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" && pwd)"
VERIFY_FIREFOX="${SOPHIA_VERIFY_FIREFOX_BIN:-$SCRIPT_DIR/sophia-verify-firefox-run}"
VERIFY_IDENTITY="${SOPHIA_VERIFY_IDENTITY_BIN:-$SCRIPT_DIR/sophia-verify-runtime-identity}"
VERIFY_LIFECYCLE="${SOPHIA_VERIFY_LIFECYCLE_BIN:-$SCRIPT_DIR/sophia-verify-lifecycle}"
if [[ ! -x "$VERIFY_FIREFOX" && -x "$SCRIPT_DIR/verify_sophia_firefox_physical.sh" ]]; then
    VERIFY_FIREFOX="$SCRIPT_DIR/verify_sophia_firefox_physical.sh"
fi
if [[ ! -x "$VERIFY_IDENTITY" && -x "$SCRIPT_DIR/verify_installed_runtime_identity.sh" ]]; then
    VERIFY_IDENTITY="$SCRIPT_DIR/verify_installed_runtime_identity.sh"
fi
if [[ ! -x "$VERIFY_LIFECYCLE" && -x "$SCRIPT_DIR/verify_installed_session_lifecycle.sh" ]]; then
    VERIFY_LIFECYCLE="$SCRIPT_DIR/verify_installed_session_lifecycle.sh"
fi
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
LOG_DIR="$STATE_HOME/sophia/xmonad-session"
RUN_ROOT="${SOPHIA_FIREFOX_RUN_ROOT:-$STATE_HOME/sophia/promotion/firefox-runs}"
PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"

"$VERIFY_FIREFOX" \
    "$LOG_DIR/session.log" "$LOG_DIR/input-guard.log" "$LOG_DIR/recovery.log"
"$VERIFY_LIFECYCLE" "$LOG_DIR/lifecycle.log" normal
install -d -m 700 "$RUN_ROOT"
sequence=1
while [[ -e "$RUN_ROOT/$(printf '%04d' "$sequence")" ]]; do
    sequence=$((sequence + 1))
done
run_dir="$RUN_ROOT/$(printf '%04d' "$sequence")"
install -d -m 700 "$run_dir"
install -m 600 "$LOG_DIR/session.log" "$run_dir/session.log"
install -m 600 "$LOG_DIR/input-guard.log" "$run_dir/input-guard.log"
install -m 600 "$LOG_DIR/lifecycle.log" "$run_dir/lifecycle.log"
grep -E '^sophia_tty_recovery schema=3 profile=xmonad ' \
    "$LOG_DIR/recovery.log" | tail -n 1 >"$run_dir/recovery.log"
if [[ -f "$PREFIX/current/manifest" ]]; then
    (cd "$PREFIX/current" && sha256sum -c SHA256SUMS)
    commit="$(sed -n 's/^commit=//p' "$PREFIX/current/manifest" | head -n 1)"
    install -m 600 "$PREFIX/current/manifest" "$run_dir/release-manifest"
    runtime_identity="$STATE_HOME/sophia/installed-session/runtime-identity.log"
    "$VERIFY_IDENTITY" "$runtime_identity"
    install -m 600 "$runtime_identity" "$run_dir/runtime-identity.log"
else
    commit="$(git -C "$ROOT_DIR" rev-parse HEAD)"
fi
printf 'schema=1\ncommit=%s\nrecorded_at_utc=%s\n' \
    "$commit" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    >"$run_dir/manifest"
(
    cd "$run_dir"
    checksum_files=(./*.log manifest)
    [[ ! -f release-manifest ]] || checksum_files+=(release-manifest)
    sha256sum "${checksum_files[@]}" >SHA256SUMS
)
echo "Recorded verified physical Firefox run: $run_dir"
