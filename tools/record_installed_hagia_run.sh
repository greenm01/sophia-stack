#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
RELEASE_DIR="$(cd "$(dirname "$SCRIPT_PATH")/.." && pwd)"
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"

profile_mode="${SOPHIA_HAGIA_PROFILE_MODE:-}"
case "$profile_mode" in
    packaged-promotion)
        SOPHIA_ATTEMPT_KIND=hagia-promotion
        SOPHIA_ATTEMPT_RESULT_RECORD=sophia_installed_hagia_promotion
        SOPHIA_ATTEMPT_RUN_ROOT="${SOPHIA_HAGIA_PROMOTION_RUN_ROOT:-$STATE_HOME/sophia/promotion/hagia-promotion-runs}"
        ;;
    user|system|explicit|packaged-fallback)
        SOPHIA_ATTEMPT_KIND=hagia
        SOPHIA_ATTEMPT_RESULT_RECORD=sophia_installed_hagia
        SOPHIA_ATTEMPT_RUN_ROOT="${SOPHIA_HAGIA_RUN_ROOT:-$STATE_HOME/sophia/promotion/hagia-runs}"
        ;;
    *)
        echo "installed Hagia recording requires an exact profile mode" >&2
        exit 1
        ;;
esac
SOPHIA_ATTEMPT_PROFILE=hagia
SOPHIA_ATTEMPT_RECORD_SCHEMA=5
SOPHIA_ATTEMPT_SESSION_DIR="$STATE_HOME/sophia/hagia-session"
SOPHIA_ATTEMPT_IDENTITY_LOG="$STATE_HOME/sophia/installed-session/launch.log"
SOPHIA_ATTEMPT_RUNTIME_IDENTITY_LOG="$STATE_HOME/sophia/installed-session/runtime-identity.log"
SOPHIA_ATTEMPT_PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"
SOPHIA_ATTEMPT_VERIFY_SESSION="${SOPHIA_VERIFY_HAGIA_SESSION_BIN:-$RELEASE_DIR/bin/sophia-verify-hagia-session}"
SOPHIA_ATTEMPT_VERIFY_IDENTITY="${SOPHIA_VERIFY_IDENTITY_BIN:-$RELEASE_DIR/bin/sophia-verify-runtime-identity}"
SOPHIA_ATTEMPT_VERIFY_LIFECYCLE="${SOPHIA_VERIFY_LIFECYCLE_BIN:-$RELEASE_DIR/bin/sophia-verify-lifecycle}"
[[ -x "$SOPHIA_ATTEMPT_VERIFY_SESSION" ]] || SOPHIA_ATTEMPT_VERIFY_SESSION="$RELEASE_DIR/tools/verify_installed_hagia_session.sh"
[[ -x "$SOPHIA_ATTEMPT_VERIFY_IDENTITY" ]] || SOPHIA_ATTEMPT_VERIFY_IDENTITY="$RELEASE_DIR/tools/verify_installed_runtime_identity.sh"
[[ -x "$SOPHIA_ATTEMPT_VERIFY_LIFECYCLE" ]] || SOPHIA_ATTEMPT_VERIFY_LIFECYCLE="$RELEASE_DIR/tools/verify_installed_session_lifecycle.sh"
SOPHIA_ATTEMPT_AUXILIARY_BINARY_NAME=hagia
SOPHIA_ATTEMPT_AUXILIARY_BINARY_PATH="$SOPHIA_ATTEMPT_PREFIX/current/target/release/hagia"
SOPHIA_ATTEMPT_AUXILIARY_IDENTITY_NAME=hagia
coverage="$SOPHIA_ATTEMPT_SESSION_DIR/coverage.kdl"
profile_identity="$SOPHIA_ATTEMPT_SESSION_DIR/profile-identity.kdl"
source "$RELEASE_DIR/tools/lib/installed_hagia_evidence.sh"
declare -a SOPHIA_ATTEMPT_EXTRA_EVIDENCE_SOURCES=("$coverage" "$profile_identity")
declare -a SOPHIA_ATTEMPT_EXTRA_EVIDENCE_TARGETS=(coverage.kdl profile-identity.kdl)
if [[ "$profile_mode" == packaged-promotion ]]; then
    SOPHIA_ATTEMPT_EXTRA_EVIDENCE_SOURCES+=("${SOPHIA_DESKTOP_PROFILE:?}")
    SOPHIA_ATTEMPT_EXTRA_EVIDENCE_TARGETS+=(desktop-profile.kdl)
fi

if [[ "$#:${1:-}:${3:-}" == 3:finish:130 ]]; then
    SOPHIA_ATTEMPT_EXPECTED_EXIT_STATUS=130
    SOPHIA_ATTEMPT_SUCCESS_STATUS=recovered
    SOPHIA_ATTEMPT_LIFECYCLE_MODE=emergency
    SOPHIA_ATTEMPT_VERIFY_SESSION="${SOPHIA_VERIFY_HAGIA_RECOVERY_BIN:-$RELEASE_DIR/bin/sophia-verify-hagia-recovery}"
    [[ -x "$SOPHIA_ATTEMPT_VERIFY_SESSION" ]] || SOPHIA_ATTEMPT_VERIFY_SESSION="$RELEASE_DIR/tools/verify_installed_hagia_recovery.sh"
fi

case "$#:${1:-}" in
    0:|3:finish)
        if [[ -s "$SOPHIA_ATTEMPT_SESSION_DIR/session.log" ]]; then
            sophia_hagia_write_coverage "$SOPHIA_ATTEMPT_SESSION_DIR/session.log" "$coverage"
            sophia_hagia_write_profile_identity \
                "$SOPHIA_ATTEMPT_SESSION_DIR/session.log" "$profile_identity"
        fi
        ;;
esac

source "$RELEASE_DIR/tools/lib/installed_attempt_ledger.sh"
sophia_record_installed_attempt "$@"
