#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
RELEASE_DIR="$(cd "$(dirname "$SCRIPT_PATH")/.." && pwd)"
VERIFY_SESSION="${SOPHIA_VERIFY_NATIVE_CHROME_SESSION_BIN:-$RELEASE_DIR/bin/sophia-verify-native-chrome-session}"
VERIFY_IDENTITY="${SOPHIA_VERIFY_IDENTITY_BIN:-$RELEASE_DIR/bin/sophia-verify-runtime-identity}"
VERIFY_LIFECYCLE="${SOPHIA_VERIFY_LIFECYCLE_BIN:-$RELEASE_DIR/bin/sophia-verify-lifecycle}"
if [[ ! -x "$VERIFY_SESSION" ]]; then
    VERIFY_SESSION="$RELEASE_DIR/tools/verify_installed_native_chrome_session.sh"
fi
if [[ ! -x "$VERIFY_IDENTITY" ]]; then
    VERIFY_IDENTITY="$RELEASE_DIR/tools/verify_installed_runtime_identity.sh"
fi
if [[ ! -x "$VERIFY_LIFECYCLE" ]]; then
    VERIFY_LIFECYCLE="$RELEASE_DIR/tools/verify_installed_session_lifecycle.sh"
fi

STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
RUN_ROOT="${SOPHIA_NATIVE_CHROME_RUN_ROOT:-$STATE_HOME/sophia/promotion/native-chrome-runs}"
(( $# <= 1 )) || {
    echo "usage: $0 [RUN_DIRECTORY]" >&2
    exit 1
}
run="${1:-}"
if [[ -z "$run" ]]; then
    run="$(find "$RUN_ROOT" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort -V | tail -n 1 || true)"
fi
[[ -n "$run" && -d "$run" ]] || {
    echo "installed native-chrome evidence is missing: ${run:-$RUN_ROOT}" >&2
    exit 1
}

(
    cd "$run"
    sha256sum -c SHA256SUMS
)
grep -Fxq 'sophia_installed_native_chrome schema=1 status=passed exit_status=0' \
    "$run/result.kdl" || {
    echo "installed native-chrome attempt did not pass: $run" >&2
    exit 1
}
"$VERIFY_SESSION" \
    "$run/session.log" "$run/sequence.log" \
    "$run/input-guard.log" "$run/recovery.log"
sophia_binary_sha256="$(sed -n 's/^sophia_binary_sha256=//p' "$run/manifest")"
"$VERIFY_IDENTITY" "$run/runtime-identity.log" "$sophia_binary_sha256"
native_wm_binary_sha256="$(sed -n 's/^native_wm_binary_sha256=//p' "$run/manifest")"
[[ "$native_wm_binary_sha256" =~ ^[0-9a-f]{64}$ ]] || {
    echo "installed native-chrome archive has no native-WM digest: $run" >&2
    exit 1
}
grep -Eq "^sophia_runtime_identity schema=2 kind=application name=sophia-wm-demo version=[^[:space:]]+ digest=$native_wm_binary_sha256$" \
    "$run/runtime-identity.log" || {
    echo "installed native-chrome runtime identity has the wrong native-WM digest: $run" >&2
    exit 1
}
"$VERIFY_LIFECYCLE" "$run/lifecycle.log" normal
[[ "$(sed -n 's/^record_schema=//p' "$run/manifest")" == 4 \
    && "$(sed -n 's/^record_kind=//p' "$run/manifest")" == native-chrome ]] || {
    echo "installed native-chrome archive has no supported record contract: $run" >&2
    exit 1
}
commit="$(sed -n 's/^commit=//p' "$run/manifest" | head -n 1)"
identity="$(tail -n 1 "$run/identity.log")"
[[ -n "$commit" \
    && "$identity" == "sophia_installed_session schema=1 status=starting "* \
    && " $identity " == *" profile=native "* \
    && " $identity " == *" commit=$commit "* \
    && "$(head -n 1 "$run/sequence.log")" == "commit=$commit" ]] || {
    echo "installed native-chrome identity does not match its release: $run" >&2
    exit 1
}
started_at_utc="$(sed -n 's/^session_started_at_utc=//p' "$run/manifest")"
[[ -n "$started_at_utc" \
    && " $identity " == *" started_at_utc=$started_at_utc "* ]] || {
    echo "installed native-chrome start time does not match its identity: $run" >&2
    exit 1
}
expected_sha256="$(sed -n 's/^launch_identity_sha256=//p' "$run/manifest")"
observed_sha256="$(sha256sum "$run/identity.log" | awk '{ print $1 }')"
[[ "$expected_sha256" == "$observed_sha256" ]] || {
    echo "installed native-chrome launch identity digest does not match: $run" >&2
    exit 1
}

echo "installed Sophia native-chrome archive passed: run=$run commit=$commit"
