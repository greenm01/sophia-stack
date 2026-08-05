#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" && pwd)"
VERIFY_LOGIN="${SOPHIA_VERIFY_LOGIN_BIN:-$SCRIPT_DIR/sophia-verify-login-cycle}"
VERIFY_IDENTITY="${SOPHIA_VERIFY_IDENTITY_BIN:-$SCRIPT_DIR/sophia-verify-runtime-identity}"
VERIFY_LIFECYCLE="${SOPHIA_VERIFY_LIFECYCLE_BIN:-$SCRIPT_DIR/sophia-verify-lifecycle}"
if [[ ! -x "$VERIFY_LOGIN" && -x "$SCRIPT_DIR/verify_installed_login_cycle.sh" ]]; then
    VERIFY_LOGIN="$SCRIPT_DIR/verify_installed_login_cycle.sh"
fi
if [[ ! -x "$VERIFY_IDENTITY" && -x "$SCRIPT_DIR/verify_installed_runtime_identity.sh" ]]; then
    VERIFY_IDENTITY="$SCRIPT_DIR/verify_installed_runtime_identity.sh"
fi
if [[ ! -x "$VERIFY_LIFECYCLE" && -x "$SCRIPT_DIR/verify_installed_session_lifecycle.sh" ]]; then
    VERIFY_LIFECYCLE="$SCRIPT_DIR/verify_installed_session_lifecycle.sh"
fi
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
SESSION_DIR="$STATE_HOME/sophia/xmonad-session"
IDENTITY_LOG="$STATE_HOME/sophia/installed-session/launch.log"
RUNTIME_IDENTITY_LOG="$STATE_HOME/sophia/installed-session/runtime-identity.log"
RUN_ROOT="${SOPHIA_PROMOTION_RUN_ROOT:-$STATE_HOME/sophia/promotion/runs}"
PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"

identity=""
commit=""
started_at_utc=""
launch_identity_sha256=""

load_launch_identity() {
    [[ -s "$IDENTITY_LOG" ]] || {
        echo "installed session identity log is missing: $IDENTITY_LOG" >&2
        return 1
    }
    [[ -s "$RUNTIME_IDENTITY_LOG" ]] || {
        echo "installed runtime identity log is missing: $RUNTIME_IDENTITY_LOG" >&2
        return 1
    }
    identity="$(tail -n 1 "$IDENTITY_LOG")"
    [[ "$identity" == "sophia_installed_session schema=1 status=starting "* ]] || {
        echo "installed session identity is malformed" >&2
        return 1
    }
    commit="$(sed -n 's/^commit=//p' "$PREFIX/current/manifest" | head -n 1)"
    [[ -n "$commit" && " $identity " == *" commit=$commit "* ]] || {
        echo "installed session identity does not match $PREFIX/current" >&2
        return 1
    }
    started_at_utc=""
    for token in $identity; do
        [[ "$token" != started_at_utc=* ]] || started_at_utc="${token#*=}"
    done
    [[ -n "$started_at_utc" ]] || {
        echo "installed session identity has no start time" >&2
        return 1
    }
    launch_identity_sha256="$(sha256sum "$IDENTITY_LOG" | awk '{ print $1 }')"
    (
        cd "$PREFIX/current"
        sha256sum -c --status SHA256SUMS
    ) || {
        echo "installed release digest verification failed" >&2
        return 1
    }
}

begin_run() {
    load_launch_identity
    install -d -m 700 "$RUN_ROOT"
    mapfile -t duplicate_runs < <(
        grep -rlFx --include=manifest \
            "launch_identity_sha256=$launch_identity_sha256" "$RUN_ROOT" 2>/dev/null || true
    )
    (( ${#duplicate_runs[@]} == 0 )) || {
        echo "installed session was already recorded: ${duplicate_runs[0]%/manifest}" >&2
        return 1
    }

    local sequence=1 run_dir
    while true; do
        run_dir="$RUN_ROOT/$(printf '%04d' "$sequence")"
        if mkdir -m 700 "$run_dir" 2>/dev/null; then
            break
        fi
        sequence=$((sequence + 1))
    done
    install -m 600 "$IDENTITY_LOG" "$run_dir/identity.log"
    install -m 600 "$RUNTIME_IDENTITY_LOG" "$run_dir/runtime-identity.log"
    install -m 600 "$PREFIX/current/manifest" "$run_dir/manifest"
    printf 'record_schema=2\nsession_started_at_utc=%s\nlaunch_identity_sha256=%s\n' \
        "$started_at_utc" "$launch_identity_sha256" >>"$run_dir/manifest"
    printf 'sophia_installed_cycle schema=1 status=pending exit_status=none\n' \
        >"$run_dir/result.kdl"
    printf '%s\n' "$run_dir"
}

write_checksums() {
    local run_dir="$1"
    (
        cd "$run_dir"
        local files=(manifest result.kdl identity.log runtime-identity.log)
        for candidate in session.log input-guard.log recovery.log lifecycle.log; do
            [[ ! -f "$candidate" ]] || files+=("$candidate")
        done
        sha256sum "${files[@]}" >SHA256SUMS
    )
}

finish_run() {
    local run_dir="$1" session_status="$2"
    [[ "$session_status" =~ ^[0-9]+$ && "$session_status" -le 255 ]] || {
        echo "session exit status must be an integer from 0 through 255" >&2
        return 1
    }
    local resolved_root resolved_run expected_identity
    resolved_root="$(readlink -f "$RUN_ROOT")"
    resolved_run="$(readlink -f "$run_dir")"
    [[ -d "$resolved_run" && "$(dirname "$resolved_run")" == "$resolved_root" ]] || {
        echo "run is not a direct child of the installed cycle ledger: $run_dir" >&2
        return 1
    }
    run_dir="$resolved_run"

    local failure=none source target
    for target in session.log input-guard.log recovery.log lifecycle.log; do
        source="$SESSION_DIR/$target"
        if [[ -s "$source" ]]; then
            install -m 600 "$source" "$run_dir/$target"
        else
            failure=missing_evidence
        fi
    done
    load_launch_identity || failure=identity_mismatch
    expected_identity="$(sed -n 's/^launch_identity_sha256=//p' "$run_dir/manifest")"
    [[ -n "$expected_identity" && "$launch_identity_sha256" == "$expected_identity" ]] ||
        failure=identity_mismatch

    if [[ "$session_status" != 0 ]]; then
        failure=session_exit
    elif [[ "$failure" == none ]]; then
        "$VERIFY_LOGIN" \
            "$run_dir/session.log" "$run_dir/input-guard.log" "$run_dir/recovery.log" ||
            failure=login_verification
        if [[ "$failure" == none ]]; then
            "$VERIFY_IDENTITY" "$run_dir/runtime-identity.log" || failure=identity_verification
        fi
        if [[ "$failure" == none ]]; then
            "$VERIFY_LIFECYCLE" "$run_dir/lifecycle.log" normal ||
                failure=lifecycle_verification
        fi
    fi

    if [[ "$failure" == none ]]; then
        printf 'sophia_installed_cycle schema=1 status=passed exit_status=0\n' \
            >"$run_dir/result.kdl"
        write_checksums "$run_dir"
        echo "Recorded verified installed Sophia run: $run_dir"
        return 0
    fi
    printf 'sophia_installed_cycle schema=1 status=failed exit_status=%s reason=%s\n' \
        "$session_status" "$failure" >"$run_dir/result.kdl"
    write_checksums "$run_dir"
    echo "Recorded failed installed Sophia attempt: $run_dir reason=$failure" >&2
    return 1
}

case "$#:${1:-}" in
    0:)
        run_dir="$(begin_run)"
        finish_run "$run_dir" 0
        ;;
    1:begin)
        begin_run
        ;;
    3:finish)
        finish_run "$2" "$3"
        ;;
    *)
        echo "usage: $0 [begin | finish RUN_DIR SESSION_EXIT_STATUS]" >&2
        exit 1
        ;;
esac
