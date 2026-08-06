# Shared immutable-attempt ledger for installed Sophia session profiles.

sophia_installed_attempt_load_identity() {
    [[ -s "$SOPHIA_ATTEMPT_IDENTITY_LOG" ]] || {
        echo "installed session identity log is missing: $SOPHIA_ATTEMPT_IDENTITY_LOG" >&2
        return 1
    }
    [[ -s "$SOPHIA_ATTEMPT_RUNTIME_IDENTITY_LOG" ]] || {
        echo "installed runtime identity log is missing: $SOPHIA_ATTEMPT_RUNTIME_IDENTITY_LOG" >&2
        return 1
    }
    sophia_attempt_identity="$(tail -n 1 "$SOPHIA_ATTEMPT_IDENTITY_LOG")"
    [[ "$sophia_attempt_identity" == "sophia_installed_session schema=1 status=starting "* \
        && " $sophia_attempt_identity " == *" profile=$SOPHIA_ATTEMPT_PROFILE "* ]] || {
        echo "installed session identity is malformed or has the wrong profile" >&2
        return 1
    }
    sophia_attempt_commit="$(
        sed -n 's/^commit=//p' "$SOPHIA_ATTEMPT_PREFIX/current/manifest" | head -n 1
    )"
    [[ -n "$sophia_attempt_commit" \
        && " $sophia_attempt_identity " == *" commit=$sophia_attempt_commit "* ]] || {
        echo "installed session identity does not match $SOPHIA_ATTEMPT_PREFIX/current" >&2
        return 1
    }
    sophia_attempt_started_at_utc=""
    local token
    for token in $sophia_attempt_identity; do
        [[ "$token" != started_at_utc=* ]] ||
            sophia_attempt_started_at_utc="${token#*=}"
    done
    [[ -n "$sophia_attempt_started_at_utc" ]] || {
        echo "installed session identity has no start time" >&2
        return 1
    }
    sophia_attempt_identity_sha256="$(
        sha256sum "$SOPHIA_ATTEMPT_IDENTITY_LOG" | awk '{ print $1 }'
    )"
    (
        cd "$SOPHIA_ATTEMPT_PREFIX/current"
        sha256sum -c --status SHA256SUMS
    ) || {
        echo "installed release digest verification failed" >&2
        return 1
    }
}

sophia_installed_attempt_begin() {
    sophia_installed_attempt_load_identity
    install -d -m 700 "$SOPHIA_ATTEMPT_RUN_ROOT"
    local -a duplicate_runs
    mapfile -t duplicate_runs < <(
        grep -rlFx --include=manifest \
            "launch_identity_sha256=$sophia_attempt_identity_sha256" \
            "$SOPHIA_ATTEMPT_RUN_ROOT" 2>/dev/null || true
    )
    (( ${#duplicate_runs[@]} == 0 )) || {
        echo "installed session was already recorded: ${duplicate_runs[0]%/manifest}" >&2
        return 1
    }

    local sequence=1 run_dir
    while true; do
        run_dir="$SOPHIA_ATTEMPT_RUN_ROOT/$(printf '%04d' "$sequence")"
        if mkdir -m 700 "$run_dir" 2>/dev/null; then
            break
        fi
        sequence=$((sequence + 1))
    done
    printf '%s schema=1 status=pending exit_status=none\n' \
        "$SOPHIA_ATTEMPT_RESULT_RECORD" >"$run_dir/result.kdl"
    install -m 600 "$SOPHIA_ATTEMPT_IDENTITY_LOG" "$run_dir/identity.log"
    install -m 600 "$SOPHIA_ATTEMPT_RUNTIME_IDENTITY_LOG" \
        "$run_dir/runtime-identity.log"
    install -m 600 "$SOPHIA_ATTEMPT_PREFIX/current/manifest" "$run_dir/manifest"
    printf 'record_schema=3\nrecord_kind=%s\nsession_started_at_utc=%s\nlaunch_identity_sha256=%s\n' \
        "$SOPHIA_ATTEMPT_KIND" "$sophia_attempt_started_at_utc" \
        "$sophia_attempt_identity_sha256" >>"$run_dir/manifest"
    printf '%s\n' "$run_dir"
}

sophia_installed_attempt_write_checksums() {
    local run_dir="$1"
    (
        cd "$run_dir"
        local files=(manifest result.kdl identity.log runtime-identity.log)
        local candidate
        for candidate in session.log input-guard.log recovery.log lifecycle.log; do
            [[ ! -f "$candidate" ]] || files+=("$candidate")
        done
        sha256sum "${files[@]}" >SHA256SUMS
    )
}

sophia_installed_attempt_finish() {
    local run_dir="$1" session_status="$2"
    [[ "$session_status" =~ ^[0-9]+$ && "$session_status" -le 255 ]] || {
        echo "session exit status must be an integer from 0 through 255" >&2
        return 1
    }
    local resolved_root resolved_run expected_identity recorded_kind
    resolved_root="$(readlink -f "$SOPHIA_ATTEMPT_RUN_ROOT")"
    resolved_run="$(readlink -f "$run_dir")"
    [[ -d "$resolved_run" && "$(dirname "$resolved_run")" == "$resolved_root" ]] || {
        echo "run is not a direct child of the installed attempt ledger: $run_dir" >&2
        return 1
    }
    run_dir="$resolved_run"
    recorded_kind="$(sed -n 's/^record_kind=//p' "$run_dir/manifest")"
    [[ "$recorded_kind" == "$SOPHIA_ATTEMPT_KIND" ]] || {
        echo "installed attempt has the wrong record kind: $run_dir" >&2
        return 1
    }

    local failure=none source target
    for target in session.log input-guard.log recovery.log lifecycle.log; do
        source="$SOPHIA_ATTEMPT_SESSION_DIR/$target"
        if [[ -s "$source" ]]; then
            install -m 600 "$source" "$run_dir/$target"
        else
            failure=missing_evidence
        fi
    done
    sophia_installed_attempt_load_identity || failure=identity_mismatch
    expected_identity="$(
        sed -n 's/^launch_identity_sha256=//p' "$run_dir/manifest"
    )"
    [[ -n "$expected_identity" \
        && "$sophia_attempt_identity_sha256" == "$expected_identity" ]] ||
        failure=identity_mismatch

    if [[ "$session_status" != 0 ]]; then
        failure=session_exit
    elif [[ "$failure" == none ]]; then
        "$SOPHIA_ATTEMPT_VERIFY_SESSION" \
            "$run_dir/session.log" "$run_dir/input-guard.log" "$run_dir/recovery.log" ||
            failure=session_verification
        if [[ "$failure" == none ]]; then
            "$SOPHIA_ATTEMPT_VERIFY_IDENTITY" "$run_dir/runtime-identity.log" ||
                failure=identity_verification
        fi
        if [[ "$failure" == none ]]; then
            "$SOPHIA_ATTEMPT_VERIFY_LIFECYCLE" "$run_dir/lifecycle.log" normal ||
                failure=lifecycle_verification
        fi
    fi

    if [[ "$failure" == none ]]; then
        printf '%s schema=1 status=passed exit_status=0\n' \
            "$SOPHIA_ATTEMPT_RESULT_RECORD" >"$run_dir/result.kdl"
        sophia_installed_attempt_write_checksums "$run_dir"
        echo "Recorded verified installed $SOPHIA_ATTEMPT_KIND run: $run_dir"
        return 0
    fi
    printf '%s schema=1 status=failed exit_status=%s reason=%s\n' \
        "$SOPHIA_ATTEMPT_RESULT_RECORD" "$session_status" "$failure" \
        >"$run_dir/result.kdl"
    sophia_installed_attempt_write_checksums "$run_dir"
    echo "Recorded failed installed $SOPHIA_ATTEMPT_KIND attempt: $run_dir reason=$failure" >&2
    return 1
}

sophia_record_installed_attempt() {
    : "${SOPHIA_ATTEMPT_KIND:?}"
    : "${SOPHIA_ATTEMPT_PROFILE:?}"
    : "${SOPHIA_ATTEMPT_RESULT_RECORD:?}"
    : "${SOPHIA_ATTEMPT_RUN_ROOT:?}"
    : "${SOPHIA_ATTEMPT_SESSION_DIR:?}"
    : "${SOPHIA_ATTEMPT_IDENTITY_LOG:?}"
    : "${SOPHIA_ATTEMPT_RUNTIME_IDENTITY_LOG:?}"
    : "${SOPHIA_ATTEMPT_PREFIX:?}"
    : "${SOPHIA_ATTEMPT_VERIFY_SESSION:?}"
    : "${SOPHIA_ATTEMPT_VERIFY_IDENTITY:?}"
    : "${SOPHIA_ATTEMPT_VERIFY_LIFECYCLE:?}"

    local run_dir
    case "$#:${1:-}" in
        0:)
            run_dir="$(sophia_installed_attempt_begin)"
            sophia_installed_attempt_finish "$run_dir" 0
            ;;
        1:begin)
            sophia_installed_attempt_begin
            ;;
        3:finish)
            sophia_installed_attempt_finish "$2" "$3"
            ;;
        *)
            echo "usage: $0 [begin | finish RUN_DIR SESSION_EXIT_STATUS]" >&2
            return 1
            ;;
    esac
}
