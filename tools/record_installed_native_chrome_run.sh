#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
RELEASE_DIR="$(cd "$(dirname "$SCRIPT_PATH")/.." && pwd)"
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
RUNTIME_ROOT="${XDG_RUNTIME_DIR:-/tmp}"

SOPHIA_ATTEMPT_KIND=native-chrome
SOPHIA_ATTEMPT_PROFILE=native
SOPHIA_ATTEMPT_RESULT_RECORD=sophia_installed_native_chrome
SOPHIA_ATTEMPT_RUN_ROOT="${SOPHIA_NATIVE_CHROME_RUN_ROOT:-$STATE_HOME/sophia/promotion/native-chrome-runs}"
SOPHIA_ATTEMPT_SESSION_DIR="$STATE_HOME/sophia/native-session"
SOPHIA_ATTEMPT_IDENTITY_LOG="$STATE_HOME/sophia/installed-session/launch.log"
SOPHIA_ATTEMPT_RUNTIME_IDENTITY_LOG="$STATE_HOME/sophia/installed-session/runtime-identity.log"
SOPHIA_ATTEMPT_PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"
SOPHIA_ATTEMPT_VERIFY_SESSION="${SOPHIA_VERIFY_NATIVE_CHROME_SESSION_BIN:-$RELEASE_DIR/bin/sophia-verify-native-chrome-session}"
SOPHIA_ATTEMPT_VERIFY_IDENTITY="${SOPHIA_VERIFY_IDENTITY_BIN:-$RELEASE_DIR/bin/sophia-verify-runtime-identity}"
SOPHIA_ATTEMPT_VERIFY_LIFECYCLE="${SOPHIA_VERIFY_LIFECYCLE_BIN:-$RELEASE_DIR/bin/sophia-verify-lifecycle}"
sequence_log="${SOPHIA_NATIVE_CHROME_SEQUENCE_LOG:-$RUNTIME_ROOT/sophia-native-hot-reload-${UID}/sequence.log}"
SOPHIA_ATTEMPT_EXTRA_EVIDENCE_SOURCES=("$sequence_log")
SOPHIA_ATTEMPT_EXTRA_EVIDENCE_TARGETS=(sequence.log)
SOPHIA_ATTEMPT_SESSION_EVIDENCE=(
    session.log sequence.log input-guard.log recovery.log
)
SOPHIA_ATTEMPT_AUXILIARY_BINARY_NAME=native_wm
SOPHIA_ATTEMPT_AUXILIARY_BINARY_PATH="$SOPHIA_ATTEMPT_PREFIX/current/target/release/sophia-wm-demo"

source "$RELEASE_DIR/tools/lib/installed_attempt_ledger.sh"
sophia_record_installed_attempt "$@"
