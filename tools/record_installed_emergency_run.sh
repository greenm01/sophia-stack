#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
RELEASE_DIR="$(cd "$(dirname "$SCRIPT_PATH")/.." && pwd)"
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"

SOPHIA_ATTEMPT_KIND=emergency
SOPHIA_ATTEMPT_PROFILE=xmonad
SOPHIA_ATTEMPT_RESULT_RECORD=sophia_installed_emergency
SOPHIA_ATTEMPT_RUN_ROOT="${SOPHIA_EMERGENCY_RUN_ROOT:-$STATE_HOME/sophia/promotion/emergency-runs}"
SOPHIA_ATTEMPT_SESSION_DIR="$STATE_HOME/sophia/xmonad-session"
SOPHIA_ATTEMPT_IDENTITY_LOG="$STATE_HOME/sophia/installed-session/launch.log"
SOPHIA_ATTEMPT_RUNTIME_IDENTITY_LOG="$STATE_HOME/sophia/installed-session/runtime-identity.log"
SOPHIA_ATTEMPT_PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"
SOPHIA_ATTEMPT_VERIFY_SESSION="${SOPHIA_VERIFY_EMERGENCY_BIN:-$RELEASE_DIR/bin/sophia-verify-emergency-run}"
SOPHIA_ATTEMPT_VERIFY_IDENTITY="${SOPHIA_VERIFY_IDENTITY_BIN:-$RELEASE_DIR/bin/sophia-verify-runtime-identity}"
SOPHIA_ATTEMPT_VERIFY_LIFECYCLE="${SOPHIA_VERIFY_LIFECYCLE_BIN:-$RELEASE_DIR/bin/sophia-verify-lifecycle}"
SOPHIA_ATTEMPT_EXPECTED_EXIT_STATUS=130
SOPHIA_ATTEMPT_LIFECYCLE_MODE=emergency
if [[ ! -x "$SOPHIA_ATTEMPT_VERIFY_SESSION" ]]; then
    SOPHIA_ATTEMPT_VERIFY_SESSION="$RELEASE_DIR/tools/verify_sophia_xmonad_emergency_tty3.sh"
fi
if [[ ! -x "$SOPHIA_ATTEMPT_VERIFY_IDENTITY" ]]; then
    SOPHIA_ATTEMPT_VERIFY_IDENTITY="$RELEASE_DIR/tools/verify_installed_runtime_identity.sh"
fi
if [[ ! -x "$SOPHIA_ATTEMPT_VERIFY_LIFECYCLE" ]]; then
    SOPHIA_ATTEMPT_VERIFY_LIFECYCLE="$RELEASE_DIR/tools/verify_installed_session_lifecycle.sh"
fi

source "$RELEASE_DIR/tools/lib/installed_attempt_ledger.sh"
sophia_record_installed_attempt "$@"
