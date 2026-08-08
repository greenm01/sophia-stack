#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
RELEASE_DIR="$(cd "$(dirname "$SCRIPT_PATH")/.." && pwd)"
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"

SOPHIA_ATTEMPT_KIND=normal
SOPHIA_ATTEMPT_PROFILE=xmonad
SOPHIA_ATTEMPT_RESULT_RECORD=sophia_installed_cycle
SOPHIA_ATTEMPT_RUN_ROOT="${SOPHIA_PROMOTION_RUN_ROOT:-$STATE_HOME/sophia/promotion/runs}"
SOPHIA_ATTEMPT_SESSION_DIR="$STATE_HOME/sophia/xmonad-session"
SOPHIA_ATTEMPT_IDENTITY_LOG="$STATE_HOME/sophia/installed-session/launch.log"
SOPHIA_ATTEMPT_RUNTIME_IDENTITY_LOG="$STATE_HOME/sophia/installed-session/runtime-identity.log"
SOPHIA_ATTEMPT_SOAK_SUMMARY="$SOPHIA_ATTEMPT_SESSION_DIR/soak-summary.kdl"
SOPHIA_ATTEMPT_PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"
SOPHIA_ATTEMPT_VERIFY_SESSION="${SOPHIA_VERIFY_LOGIN_BIN:-$RELEASE_DIR/bin/sophia-verify-login-cycle}"
SOPHIA_ATTEMPT_VERIFY_IDENTITY="${SOPHIA_VERIFY_IDENTITY_BIN:-$RELEASE_DIR/bin/sophia-verify-runtime-identity}"
SOPHIA_ATTEMPT_VERIFY_LIFECYCLE="${SOPHIA_VERIFY_LIFECYCLE_BIN:-$RELEASE_DIR/bin/sophia-verify-lifecycle}"
if [[ ! -x "$SOPHIA_ATTEMPT_VERIFY_SESSION" ]]; then
    SOPHIA_ATTEMPT_VERIFY_SESSION="$RELEASE_DIR/tools/verify_installed_login_cycle.sh"
fi
if [[ ! -x "$SOPHIA_ATTEMPT_VERIFY_IDENTITY" ]]; then
    SOPHIA_ATTEMPT_VERIFY_IDENTITY="$RELEASE_DIR/tools/verify_installed_runtime_identity.sh"
fi
if [[ ! -x "$SOPHIA_ATTEMPT_VERIFY_LIFECYCLE" ]]; then
    SOPHIA_ATTEMPT_VERIFY_LIFECYCLE="$RELEASE_DIR/tools/verify_installed_session_lifecycle.sh"
fi

source "$RELEASE_DIR/tools/lib/installed_soak_evidence.sh"
declare -a SOPHIA_ATTEMPT_EXTRA_EVIDENCE_SOURCES=(
    "$SOPHIA_ATTEMPT_SOAK_SUMMARY"
)
declare -a SOPHIA_ATTEMPT_EXTRA_EVIDENCE_TARGETS=(soak-summary.kdl)

case "$#:${1:-}" in
    0:|3:finish)
        if [[ -s "$SOPHIA_ATTEMPT_SESSION_DIR/session.log" ]]; then
            sophia_soak_write_summary \
                "$SOPHIA_ATTEMPT_SESSION_DIR/session.log" \
                "$SOPHIA_ATTEMPT_SOAK_SUMMARY"
        fi
        ;;
esac

source "$RELEASE_DIR/tools/lib/installed_attempt_ledger.sh"
sophia_record_installed_attempt "$@"
