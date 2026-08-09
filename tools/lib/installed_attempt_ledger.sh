# Shared immutable-attempt ledger for installed Sophia session profiles.

# Proof profiles may add bounded evidence without forking reservation, identity,
# finalization, or checksum semantics.
declare -p SOPHIA_ATTEMPT_EXTRA_EVIDENCE_SOURCES >/dev/null 2>&1 ||
    declare -a SOPHIA_ATTEMPT_EXTRA_EVIDENCE_SOURCES=()
declare -p SOPHIA_ATTEMPT_EXTRA_EVIDENCE_TARGETS >/dev/null 2>&1 ||
    declare -a SOPHIA_ATTEMPT_EXTRA_EVIDENCE_TARGETS=()
declare -p SOPHIA_ATTEMPT_SESSION_EVIDENCE >/dev/null 2>&1 ||
    declare -a SOPHIA_ATTEMPT_SESSION_EVIDENCE=(
        session.log input-guard.log recovery.log
    )
SOPHIA_ATTEMPT_AUXILIARY_BINARY_NAME="${SOPHIA_ATTEMPT_AUXILIARY_BINARY_NAME:-}"
SOPHIA_ATTEMPT_AUXILIARY_BINARY_PATH="${SOPHIA_ATTEMPT_AUXILIARY_BINARY_PATH:-}"
SOPHIA_ATTEMPT_AUXILIARY_IDENTITY_NAME="${SOPHIA_ATTEMPT_AUXILIARY_IDENTITY_NAME:-}"

sophia_installed_attempt_validate_evidence_contract() {
    (( ${#SOPHIA_ATTEMPT_EXTRA_EVIDENCE_SOURCES[@]} ==
        ${#SOPHIA_ATTEMPT_EXTRA_EVIDENCE_TARGETS[@]} )) || {
        echo "installed attempt extra-evidence arrays have different lengths" >&2
        return 1
    }
    local target
    declare -A evidence_targets=(
        [session.log]=1
        [input-guard.log]=1
        [recovery.log]=1
        [lifecycle.log]=1
    )
    for target in "${SOPHIA_ATTEMPT_EXTRA_EVIDENCE_TARGETS[@]}"; do
        [[ "$target" =~ ^[0-9A-Za-z._-]+$ ]] || {
            echo "installed attempt has an unsafe evidence name: $target" >&2
            return 1
        }
        [[ -z "${evidence_targets[$target]:-}" ]] || {
            echo "installed attempt has a duplicate evidence name: $target" >&2
            return 1
        }
        evidence_targets[$target]=1
    done
    for target in "${SOPHIA_ATTEMPT_SESSION_EVIDENCE[@]}"; do
        [[ "$target" =~ ^[0-9A-Za-z._-]+$ \
            && -n "${evidence_targets[$target]:-}" ]] || {
            echo "installed attempt has an unknown verifier input: $target" >&2
            return 1
        }
    done
    if [[ -n "$SOPHIA_ATTEMPT_AUXILIARY_BINARY_NAME" \
        || -n "$SOPHIA_ATTEMPT_AUXILIARY_BINARY_PATH" ]]; then
        [[ "$SOPHIA_ATTEMPT_AUXILIARY_BINARY_NAME" =~ ^[a-z][a-z0-9_]*$ \
            && -f "$SOPHIA_ATTEMPT_AUXILIARY_BINARY_PATH" ]] || {
            echo "installed attempt has an invalid auxiliary binary contract" >&2
            return 1
        }
        local release_root resolved_binary
        release_root="$(readlink -f "$SOPHIA_ATTEMPT_PREFIX/current")"
        resolved_binary="$(readlink -f "$SOPHIA_ATTEMPT_AUXILIARY_BINARY_PATH")"
        [[ "$resolved_binary" == "$release_root/"* ]] || {
            echo "installed attempt auxiliary binary is outside the release" >&2
            return 1
        }
    fi
    if [[ -n "$SOPHIA_ATTEMPT_AUXILIARY_IDENTITY_NAME" ]]; then
        [[ -n "$SOPHIA_ATTEMPT_AUXILIARY_BINARY_NAME" \
            && "$SOPHIA_ATTEMPT_AUXILIARY_IDENTITY_NAME" =~ ^[a-z][a-z0-9_-]*$ ]] || {
            echo "installed attempt has an invalid auxiliary identity contract" >&2
            return 1
        }
    fi
}

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
    sophia_attempt_binary_sha256="$(
        sha256sum "$SOPHIA_ATTEMPT_PREFIX/current/target/release/sophia" |
            awk '{ print $1 }'
    )"
    [[ "$sophia_attempt_binary_sha256" =~ ^[0-9a-f]{64}$ ]] || {
        echo "installed Sophia binary digest is unavailable" >&2
        return 1
    }
    sophia_attempt_auxiliary_binary_sha256=""
    if [[ -n "$SOPHIA_ATTEMPT_AUXILIARY_BINARY_PATH" ]]; then
        sophia_attempt_auxiliary_binary_sha256="$(
            sha256sum "$SOPHIA_ATTEMPT_AUXILIARY_BINARY_PATH" |
                awk '{ print $1 }'
        )"
        [[ "$sophia_attempt_auxiliary_binary_sha256" =~ ^[0-9a-f]{64}$ ]] || {
            echo "installed attempt auxiliary binary digest is unavailable" >&2
            return 1
        }
    fi
    local -a identity_args=(
        "$SOPHIA_ATTEMPT_RUNTIME_IDENTITY_LOG"
        "$sophia_attempt_binary_sha256"
    )
    if [[ -n "$SOPHIA_ATTEMPT_AUXILIARY_IDENTITY_NAME" ]]; then
        identity_args+=(
            "$SOPHIA_ATTEMPT_AUXILIARY_IDENTITY_NAME"
            "$sophia_attempt_auxiliary_binary_sha256"
        )
    fi
    if ! "$SOPHIA_ATTEMPT_VERIFY_IDENTITY" "${identity_args[@]}" >/dev/null; then
        echo "runtime identity does not match the installed Sophia binary" >&2
        return 1
    fi
}

sophia_installed_attempt_begin() {
    sophia_installed_attempt_load_identity
    local lifecycle_mode="${SOPHIA_ATTEMPT_LIFECYCLE_MODE:-normal}"
    [[ "$lifecycle_mode" == normal || "$lifecycle_mode" == cycle \
        || "$lifecycle_mode" == emergency || "$lifecycle_mode" == watchdog ]] || {
        echo "installed attempt has an invalid lifecycle mode: $lifecycle_mode" >&2
        return 1
    }
    local append_auxiliary_binary=true auxiliary_binary_field=""
    local -a recorded_auxiliary_binaries=()
    if [[ -n "$SOPHIA_ATTEMPT_AUXILIARY_BINARY_NAME" ]]; then
        auxiliary_binary_field="${SOPHIA_ATTEMPT_AUXILIARY_BINARY_NAME}_binary_sha256"
        mapfile -t recorded_auxiliary_binaries < <(
            sed -n "s/^${auxiliary_binary_field}=//p" \
                "$SOPHIA_ATTEMPT_PREFIX/current/manifest"
        )
        case "${#recorded_auxiliary_binaries[@]}" in
            0)
                ;;
            1)
                [[ "${recorded_auxiliary_binaries[0]}" =~ ^[0-9a-f]{64}$ \
                    && "${recorded_auxiliary_binaries[0]}" == \
                        "$sophia_attempt_auxiliary_binary_sha256" ]] || {
                    echo "installed release auxiliary binary digest does not match the measured binary" >&2
                    return 1
                }
                append_auxiliary_binary=false
                ;;
            *)
                echo "installed release has duplicate auxiliary binary digests" >&2
                return 1
                ;;
        esac
    fi
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
    printf 'record_schema=4\nrecord_kind=%s\nlifecycle_mode=%s\nsession_started_at_utc=%s\nlaunch_identity_sha256=%s\nsophia_binary_sha256=%s\n' \
        "$SOPHIA_ATTEMPT_KIND" "$lifecycle_mode" "$sophia_attempt_started_at_utc" \
        "$sophia_attempt_identity_sha256" "$sophia_attempt_binary_sha256" \
        >>"$run_dir/manifest"
    if [[ -n "$SOPHIA_ATTEMPT_AUXILIARY_BINARY_NAME" \
        && "$append_auxiliary_binary" == true ]]; then
        printf '%s_binary_sha256=%s\n' \
            "$SOPHIA_ATTEMPT_AUXILIARY_BINARY_NAME" \
            "$sophia_attempt_auxiliary_binary_sha256" >>"$run_dir/manifest"
    fi
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
        for candidate in "${SOPHIA_ATTEMPT_EXTRA_EVIDENCE_TARGETS[@]}"; do
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
    local resolved_root resolved_run expected_identity recorded_binary recorded_kind
    local recorded_auxiliary_binary
    local -a recorded_auxiliary_binaries=()
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

    local expected_status="${SOPHIA_ATTEMPT_EXPECTED_EXIT_STATUS:-0}"
    [[ "$expected_status" =~ ^[0-9]+$ && "$expected_status" -le 255 ]] || {
        echo "expected session exit status must be an integer from 0 through 255" >&2
        return 1
    }

    local failure=none source target index
    for target in session.log input-guard.log recovery.log lifecycle.log; do
        source="$SOPHIA_ATTEMPT_SESSION_DIR/$target"
        if [[ -s "$source" ]]; then
            install -m 600 "$source" "$run_dir/$target"
        else
            failure=missing_evidence
        fi
    done
    for index in "${!SOPHIA_ATTEMPT_EXTRA_EVIDENCE_SOURCES[@]}"; do
        source="${SOPHIA_ATTEMPT_EXTRA_EVIDENCE_SOURCES[$index]}"
        target="${SOPHIA_ATTEMPT_EXTRA_EVIDENCE_TARGETS[$index]}"
        if [[ -s "$source" ]]; then
            install -m 600 "$source" "$run_dir/$target"
        else
            failure=missing_evidence
        fi
    done
    sophia_attempt_identity_sha256=""
    sophia_attempt_binary_sha256=""
    sophia_attempt_auxiliary_binary_sha256=""
    sophia_installed_attempt_load_identity || failure=identity_mismatch
    expected_identity="$(
        sed -n 's/^launch_identity_sha256=//p' "$run_dir/manifest"
    )"
    [[ -n "$expected_identity" \
        && "$sophia_attempt_identity_sha256" == "$expected_identity" ]] ||
        failure=identity_mismatch
    recorded_binary="$(sed -n 's/^sophia_binary_sha256=//p' "$run_dir/manifest")"
    [[ "$recorded_binary" =~ ^[0-9a-f]{64}$ \
        && "$sophia_attempt_binary_sha256" == "$recorded_binary" ]] ||
        failure=identity_mismatch
    if [[ -n "$SOPHIA_ATTEMPT_AUXILIARY_BINARY_NAME" ]]; then
        mapfile -t recorded_auxiliary_binaries < <(
            sed -n "s/^${SOPHIA_ATTEMPT_AUXILIARY_BINARY_NAME}_binary_sha256=//p" \
                "$run_dir/manifest"
        )
        if (( ${#recorded_auxiliary_binaries[@]} == 1 )); then
            recorded_auxiliary_binary="${recorded_auxiliary_binaries[0]}"
        else
            recorded_auxiliary_binary=""
        fi
        [[ "${#recorded_auxiliary_binaries[@]}" == 1 \
            && "$recorded_auxiliary_binary" =~ ^[0-9a-f]{64}$ \
            && "$sophia_attempt_auxiliary_binary_sha256" == \
                "$recorded_auxiliary_binary" ]] || failure=identity_mismatch
    fi

    if [[ "$session_status" != "$expected_status" ]]; then
        failure=session_exit
    elif [[ "$failure" == none ]]; then
        local -a session_evidence=()
        for target in "${SOPHIA_ATTEMPT_SESSION_EVIDENCE[@]}"; do
            session_evidence+=("$run_dir/$target")
        done
        if [[ "${SOPHIA_ATTEMPT_SESSION_INCLUDES_LIFECYCLE:-false}" == true ]]; then
            session_evidence+=("$run_dir/lifecycle.log")
        fi
        "$SOPHIA_ATTEMPT_VERIFY_SESSION" "${session_evidence[@]}" ||
            failure=session_verification
        if [[ "$failure" == none ]]; then
            local -a archived_identity_args=(
                "$run_dir/runtime-identity.log" "$recorded_binary"
            )
            if [[ -n "$SOPHIA_ATTEMPT_AUXILIARY_IDENTITY_NAME" ]]; then
                archived_identity_args+=(
                    "$SOPHIA_ATTEMPT_AUXILIARY_IDENTITY_NAME"
                    "$recorded_auxiliary_binary"
                )
            fi
            "$SOPHIA_ATTEMPT_VERIFY_IDENTITY" \
                "${archived_identity_args[@]}" ||
                failure=identity_verification
        fi
        if [[ "$failure" == none ]]; then
            "$SOPHIA_ATTEMPT_VERIFY_LIFECYCLE" "$run_dir/lifecycle.log" \
                "${SOPHIA_ATTEMPT_LIFECYCLE_MODE:-normal}" ||
                failure=lifecycle_verification
        fi
    fi

    if [[ "$failure" == none ]]; then
        local success_status="${SOPHIA_ATTEMPT_SUCCESS_STATUS:-passed}"
        [[ "$success_status" == passed || "$success_status" == recovered ]] || {
            echo "installed attempt has an invalid success status: $success_status" >&2
            return 1
        }
        printf '%s schema=1 status=%s exit_status=%s\n' \
            "$SOPHIA_ATTEMPT_RESULT_RECORD" "$success_status" "$session_status" \
            >"$run_dir/result.kdl"
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
    sophia_installed_attempt_validate_evidence_contract

    local run_dir
    case "$#:${1:-}" in
        0:)
            run_dir="$(sophia_installed_attempt_begin)"
            sophia_installed_attempt_finish "$run_dir" \
                "${SOPHIA_ATTEMPT_EXPECTED_EXIT_STATUS:-0}"
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
