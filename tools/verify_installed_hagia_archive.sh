#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
RELEASE_DIR="$(cd "$(dirname "$SCRIPT_PATH")/.." && pwd)"
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
command_name="$(basename "$0")"
if [[ "$command_name" == sophia-verify-hagia-promotion \
    || "${SOPHIA_VERIFY_HAGIA_PROMOTION:-false}" == true ]]; then
    expected_kind=hagia-promotion
    run_root="${SOPHIA_HAGIA_PROMOTION_RUN_ROOT:-$STATE_HOME/sophia/promotion/hagia-promotion-runs}"
else
    expected_kind=hagia
    run_root="${SOPHIA_HAGIA_RUN_ROOT:-$STATE_HOME/sophia/promotion/hagia-runs}"
fi
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
record_schema="$(sed -n 's/^record_schema=//p' "$run/manifest")"
record_kind="$(sed -n 's/^record_kind=//p' "$run/manifest")"
[[ "$record_kind" == "$expected_kind" ]] || {
    echo "installed Hagia archive has the wrong record kind: $run" >&2
    exit 1
}
case "$record_schema" in
    4)
        [[ "$expected_kind" == hagia ]] || {
            echo "Hagia promotion requires profile-aware record schema 5: $run" >&2
            exit 1
        }
        ;;
    5)
        profile_identity="$(cat "$run/profile-identity.kdl" 2>/dev/null || true)"
        [[ "$profile_identity" =~ ^sophia_live_desktop_profile\ schema=1\ status=loaded\ mode=(user|system|explicit|packaged-fallback|packaged-promotion)\ generation=[1-9][0-9]*\ digest=[0-9a-f]{64}\ root_sha256=([0-9a-f]{64})\ sources=[1-9][0-9]*$ ]] || {
            echo "installed Hagia archive has no exact profile identity: $run" >&2
            exit 1
        }
        profile_mode="${BASH_REMATCH[1]}"
        profile_sha256="${BASH_REMATCH[2]}"
        [[ "$(grep -Fxc "$profile_identity" "$run/session.log")" == 1 ]] || {
            echo "installed Hagia profile identity does not match raw evidence: $run" >&2
            exit 1
        }
        launch_identity="$(tail -n 1 "$run/identity.log")"
        [[ " $launch_identity " == *" desktop_profile_mode=$profile_mode "* \
            && " $launch_identity " == *" desktop_profile_sha256=$profile_sha256 "* ]] || {
            echo "installed Hagia launch and activation profile identities disagree: $run" >&2
            exit 1
        }
        if [[ "$expected_kind" == hagia-promotion ]]; then
            [[ "$profile_mode" == packaged-promotion ]] || {
                echo "Hagia promotion archive did not use packaged-promotion mode: $run" >&2
                exit 1
            }
            release_schema="$(sed -n 's/^schema=//p' "$run/manifest" | head -n 1)"
            packaged_sha256="$(sed -n 's/^hagia_default_profile_sha256=//p' "$run/manifest")"
            [[ "$release_schema" == 5 && "$packaged_sha256" == "$profile_sha256" \
                && -f "$run/desktop-profile.kdl" \
                && "$(sha256sum "$run/desktop-profile.kdl" | awk '{ print $1 }')" == "$packaged_sha256" ]] || {
                echo "Hagia promotion profile does not match the packaged default: $run" >&2
                exit 1
            }
        else
            [[ "$profile_mode" != packaged-promotion && ! -e "$run/desktop-profile.kdl" ]] || {
                echo "daily Hagia evidence crossed the promotion profile boundary: $run" >&2
                exit 1
            }
        fi
        ;;
    *)
        echo "installed Hagia archive has an unsupported record schema: $run" >&2
        exit 1
        ;;
esac
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

echo "installed Hagia archive verified: kind=$record_kind status=$status run=$run"
