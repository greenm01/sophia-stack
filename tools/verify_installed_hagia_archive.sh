#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
RELEASE_DIR="$(cd "$(dirname "$SCRIPT_PATH")/.." && pwd)"
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
run_root="${SOPHIA_HAGIA_RUN_ROOT:-$STATE_HOME/sophia/promotion/hagia-runs}"
run="${1:-}"
if [[ -z "$run" ]]; then
    run="$(find "$run_root" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort -V | tail -n 1 || true)"
fi
[[ -n "$run" && -s "$run/SHA256SUMS" ]] || {
    echo "installed Hagia evidence is missing: ${run:-$run_root}" >&2
    exit 1
}
(cd "$run" && sha256sum -c --status SHA256SUMS) || {
    echo "installed Hagia archive checksum verification failed: $run" >&2
    exit 1
}
[[ "$(sed -n 's/^record_kind=//p' "$run/manifest")" == hagia ]] || {
    echo "installed Hagia archive has the wrong record kind: $run" >&2
    exit 1
}
result="$(tail -n 1 "$run/result.kdl")"
status="$(sed -n 's/.* status=\([^ ]*\).*/\1/p' <<<"$result")"
case "$status" in
    passed) session_verifier="$RELEASE_DIR/bin/sophia-verify-hagia-session"; lifecycle_mode=normal ;;
    recovered) session_verifier="$RELEASE_DIR/bin/sophia-verify-hagia-recovery"; lifecycle_mode=emergency ;;
    *) echo "latest installed Hagia attempt is not healthy: $result" >&2; exit 1 ;;
esac
[[ -x "$session_verifier" ]] || {
    if [[ "$status" == passed ]]; then
        session_verifier="$RELEASE_DIR/tools/verify_installed_hagia_session.sh"
    else
        session_verifier="$RELEASE_DIR/tools/verify_installed_hagia_recovery.sh"
    fi
}
identity_verifier="${SOPHIA_VERIFY_IDENTITY_BIN:-$RELEASE_DIR/bin/sophia-verify-runtime-identity}"
lifecycle_verifier="${SOPHIA_VERIFY_LIFECYCLE_BIN:-$RELEASE_DIR/bin/sophia-verify-lifecycle}"
[[ -x "$identity_verifier" ]] || identity_verifier="$RELEASE_DIR/tools/verify_installed_runtime_identity.sh"
[[ -x "$lifecycle_verifier" ]] || lifecycle_verifier="$RELEASE_DIR/tools/verify_installed_session_lifecycle.sh"
sophia_digest="$(sed -n 's/^sophia_binary_sha256=//p' "$run/manifest")"
hagia_digest="$(sed -n 's/^hagia_binary_sha256=//p' "$run/manifest")"
"$identity_verifier" "$run/runtime-identity.log" "$sophia_digest" hagia "$hagia_digest"
"$session_verifier" "$run/session.log" "$run/input-guard.log" "$run/recovery.log"
"$lifecycle_verifier" "$run/lifecycle.log" "$lifecycle_mode"
source "$RELEASE_DIR/tools/lib/installed_hagia_evidence.sh"
expected_coverage="$(sophia_hagia_emit_coverage "$run/session.log")"
observed_coverage="$(cat "$run/coverage.kdl")"
[[ "$observed_coverage" == "$expected_coverage" ]] || {
    echo "installed Hagia coverage does not match raw evidence: $run" >&2
    exit 1
}

echo "installed Hagia archive verified: status=$status run=$run"
