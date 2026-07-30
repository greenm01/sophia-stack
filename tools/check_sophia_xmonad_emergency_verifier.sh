#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SESSION_SOURCE="$ROOT_DIR/tools/fixtures/physical_firefox_session_pass.log"
GUARD="$ROOT_DIR/tools/fixtures/physical_xmonad_guard_emergency_pass.log"
RECOVERY="$ROOT_DIR/tools/fixtures/physical_xmonad_recovery_emergency_pass.log"
SESSION_TEMP="$(mktemp)"
MUTATION_TEMP="$(mktemp)"
trap 'rm -f -- "$SESSION_TEMP" "$MUTATION_TEMP"' EXIT

sed -e 's/^sophia_live_session schema=14 status=bounded_complete /sophia_live_session schema=16 status=bounded_complete /' \
    -e '1i sophia_live_session_input_pipeline schema=1 status=emergency_exit\
sophia_live_session_keys schema=1 status=released reason=emergency scope=all count=2' \
    "$SESSION_SOURCE" >"$SESSION_TEMP"
"$ROOT_DIR/tools/verify_sophia_xmonad_emergency_tty3.sh" \
    "$SESSION_TEMP" "$GUARD" "$RECOVERY"

grep -Fv 'status=emergency_exit' "$SESSION_TEMP" >"$MUTATION_TEMP"
if "$ROOT_DIR/tools/verify_sophia_xmonad_emergency_tty3.sh" \
    "$MUTATION_TEMP" "$GUARD" "$RECOVERY"; then
    echo "emergency verifier accepted a session without owner-loop recovery" >&2
    exit 1
fi

grep -Fv 'reason=emergency scope=all' "$SESSION_TEMP" >"$MUTATION_TEMP"
if "$ROOT_DIR/tools/verify_sophia_xmonad_emergency_tty3.sh" \
    "$MUTATION_TEMP" "$GUARD" "$RECOVERY"; then
    echo "emergency verifier accepted an undrained emergency chord" >&2
    exit 1
fi

sed 's/status=complete pending=0/status=complete pending=2/' \
    "$SESSION_TEMP" >"$MUTATION_TEMP"
if "$ROOT_DIR/tools/verify_sophia_xmonad_emergency_tty3.sh" \
    "$MUTATION_TEMP" "$GUARD" "$RECOVERY"; then
    echo "emergency verifier accepted pending client keys" >&2
    exit 1
fi

sed 's/native_cleanup_pending=false/native_cleanup_pending=true/' \
    "$SESSION_TEMP" >"$MUTATION_TEMP"
if "$ROOT_DIR/tools/verify_sophia_xmonad_emergency_tty3.sh" \
    "$MUTATION_TEMP" "$GUARD" "$RECOVERY"; then
    echo "emergency verifier accepted pending native cleanup" >&2
    exit 1
fi

grep -Fv 'status=triggered' "$GUARD" >"$MUTATION_TEMP"
if "$ROOT_DIR/tools/verify_sophia_xmonad_emergency_tty3.sh" \
    "$SESSION_TEMP" "$MUTATION_TEMP" "$RECOVERY"; then
    echo "emergency verifier accepted an untriggered independent guard" >&2
    exit 1
fi

sed 's/session_shutdown=graceful/session_shutdown=fallback_term/' \
    "$RECOVERY" >"$MUTATION_TEMP"
if "$ROOT_DIR/tools/verify_sophia_xmonad_emergency_tty3.sh" \
    "$SESSION_TEMP" "$GUARD" "$MUTATION_TEMP"; then
    echo "emergency verifier accepted TERM fallback as graceful cleanup" >&2
    exit 1
fi

echo "xmonad emergency verifier fixtures passed"
