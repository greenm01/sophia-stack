#!/usr/bin/env bash
set -euo pipefail

session_log="${1:-}"
guard_log="${2:-}"
recovery_log="${3:-}"
[[ -s "$session_log" && -s "$guard_log" && -s "$recovery_log" ]] || {
    echo "usage: tools/verify_installed_hagia_recovery.sh SESSION_LOG GUARD_LOG RECOVERY_LOG" >&2
    exit 1
}
fail() { echo "installed Hagia recovery verification failed: $*" >&2; exit 1; }
grep -Eq '^sophia_session_input_guard schema=1 status=armed$' "$guard_log" || fail "input guard was not armed"
grep -Eq '^sophia_session_input_guard schema=1 status=triggered$' "$guard_log" || fail "emergency recovery was not triggered"
recovery="$(grep -E '^sophia_tty_recovery schema=3 profile=hagia ' "$recovery_log" | tail -n 1 || true)"
[[ -n "$recovery" \
    && " $recovery " == *' termios_restored=true '* \
    && " $recovery " == *' emergency=true '* \
    && " $recovery " == *' session_exit_status='* ]] || fail "TTY recovery is incomplete"
kd_before="$(sed -n 's/.* kd_mode_before=\([^ ]*\).*/\1/p' <<<"$recovery")"
kd_after="$(sed -n 's/.* kd_mode_after=\([^ ]*\).*/\1/p' <<<"$recovery")"
[[ -n "$kd_before" && "$kd_before" == "$kd_after" ]] || fail "KD mode was not restored"

echo "installed Hagia emergency recovery verified"
