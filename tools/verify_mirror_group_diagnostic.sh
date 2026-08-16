#!/usr/bin/env bash
set -euo pipefail

evidence="${1:?usage: verify_mirror_group_diagnostic.sh EVIDENCE KERNEL_DELTA}"
kernel_delta="${2:?usage: verify_mirror_group_diagnostic.sh EVIDENCE KERNEL_DELTA}"

[[ -r "$evidence" && -r "$kernel_delta" ]] || {
    echo "mirror-group diagnostic inputs are not readable" >&2
    exit 1
}
[[ "$(grep -Ec '^sophia_mirror_group_gate schema=1 status=starting ' "$evidence")" == 1 ]] || {
    echo "mirror-group diagnostic needs exactly one run identity" >&2
    exit 1
}
identity="$(grep -E '^sophia_mirror_group_gate schema=1 status=starting ' "$evidence")"
[[ "$identity" =~ ^sophia_mirror_group_gate\ schema=1\ status=starting\ source_commit=[0-9a-f]{40}\ sophia_sha256=[0-9a-f]{64}\ profile_sha256=[0-9a-f]{64}$ ]] || {
    echo "mirror-group diagnostic has malformed run identity" >&2
    exit 1
}
[[ "$(grep -Ec '^sophia_mirror_group_kernel schema=1 status=captured ' "$evidence")" == 1 ]] || {
    echo "mirror-group diagnostic needs exactly one kernel capture result" >&2
    exit 1
}
kernel="$(grep -E '^sophia_mirror_group_kernel schema=1 status=captured ' "$evidence")"
[[ "$kernel" =~ ^sophia_mirror_group_kernel\ schema=1\ status=captured\ availability=(available|unavailable)\ continuity=(append|reset|unknown)\ lines=([0-9]+)\ total_lines=([0-9]+)\ truncated=(true|false)$ ]] || {
    echo "mirror-group diagnostic has malformed kernel capture metadata" >&2
    exit 1
}
availability="${BASH_REMATCH[1]}"
continuity="${BASH_REMATCH[2]}"
lines="${BASH_REMATCH[3]}"
total_lines="${BASH_REMATCH[4]}"
truncated="${BASH_REMATCH[5]}"
[[ "$(grep -Ec '^sophia_mirror_group_gate schema=1 status=failed ' "$evidence")" == 1 ]] || {
    echo "mirror-group diagnostic needs exactly one final failure result" >&2
    exit 1
}
failure="$(grep -E '^sophia_mirror_group_gate schema=1 status=failed ' "$evidence")"
[[ "$failure" =~ ^sophia_mirror_group_gate\ schema=1\ status=failed\ stage=(runtime|visual_confirmation|verification)\ exit=([0-9]+)\ signal=([0-9]+)\ kernel_capture=(available|unavailable)$ ]] || {
    echo "mirror-group diagnostic has malformed final failure result" >&2
    exit 1
}
stage="${BASH_REMATCH[1]}"
exit_status="${BASH_REMATCH[2]}"
signal="${BASH_REMATCH[3]}"
kernel_capture="${BASH_REMATCH[4]}"
(( exit_status > 0 && exit_status <= 255 )) || {
    echo "mirror-group diagnostic has invalid failure exit" >&2
    exit 1
}
expected_signal=0
if (( exit_status >= 128 )); then
    expected_signal=$((exit_status - 128))
fi
(( signal == expected_signal )) || {
    echo "mirror-group diagnostic signal does not match its exit" >&2
    exit 1
}
if [[ ( "$stage" == visual_confirmation || "$stage" == verification ) && "$exit_status" != 1 ]]; then
    echo "mirror-group operator/verifier diagnostic has an invalid exit" >&2
    exit 1
fi
[[ "$kernel_capture" == "$availability" ]] || {
    echo "mirror-group diagnostic kernel availability disagrees with its failure result" >&2
    exit 1
}
if [[ "$availability" == available ]]; then
    [[ "$continuity" == append || "$continuity" == reset ]] &&
        (( lines <= 4096 && total_lines >= lines )) || {
        echo "mirror-group diagnostic has invalid available kernel delta metadata" >&2
        exit 1
    }
    if [[ "$truncated" == true ]]; then
        (( total_lines > lines )) || {
            echo "mirror-group diagnostic claims an untruncated kernel delta was truncated" >&2
            exit 1
        }
    else
        (( total_lines == lines )) || {
            echo "mirror-group diagnostic omitted kernel lines without recording truncation" >&2
            exit 1
        }
    fi
else
    [[ "$continuity" == unknown && "$lines" == 0 && "$total_lines" == 0 && "$truncated" == false ]] || {
        echo "mirror-group diagnostic has invalid unavailable kernel metadata" >&2
        exit 1
    }
fi
[[ "$(wc -l <"$kernel_delta")" == "$lines" ]] || {
    echo "mirror-group diagnostic kernel delta length disagrees with its metadata" >&2
    exit 1
}
if grep -Eq '^sophia_mirror_group_gate schema=1 status=passed( |$)' "$evidence"; then
    echo "mirror-group diagnostic contains a promotion result" >&2
    exit 1
fi
visual_count="$(grep -Ec '^sophia_mirror_group_gate schema=1 status=visual_confirmed( |$)' "$evidence" || true)"
if [[ "$stage" == verification ]]; then
    [[ "$visual_count" == 1 ]] || {
        echo "mirror-group verifier diagnostic needs its operator confirmation" >&2
        exit 1
    }
elif (( visual_count != 0 )); then
    echo "mirror-group pre-verification diagnostic contains operator confirmation" >&2
    exit 1
fi

# A controlled owner-loop failure after native bootstrap has enough process
# lifetime to drain its physical owners. Treat omission of that evidence as a
# second failure, while leaving signal/abort diagnostics admissible because the
# process may not have had a cleanup opportunity.
if [[ "$stage" == runtime && "$signal" == 0 ]] \
    && grep -Eq 'sophia_live_(mirror_bootstrap schema=2|head_bootstrap schema=1) status=' "$evidence"; then
    [[ "$(grep -Ec '^sophia_live_session_runtime_fatal schema=1 status=cleaned .*native_suspend_reported=true native_drained=true abandoned_scanouts=0 .*presentations_shutdown=true cleanup_errors=0$' "$evidence" || true)" == 1 ]] || {
        echo "controlled native runtime failure lacks clean bounded-fatal evidence" >&2
        exit 1
    }
    [[ "$(grep -Ec '^sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 ' "$evidence" || true)" == 1 ]] || {
        echo "controlled native runtime failure lacks a clean suspend record" >&2
        exit 1
    }
    [[ "$(grep -Ec '^sophia_live_session_cleanup schema=1 status=clean .*frontend_workers=0 .*namespace=revoked xauthority=removed$' "$evidence" || true)" == 1 ]] || {
        echo "controlled native runtime failure lacks clean frontend teardown" >&2
        exit 1
    }
fi

echo "mirror-group diagnostic verified: stage=$stage exit=$exit_status signal=$signal kernel_capture=$availability"
