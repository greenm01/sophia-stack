#!/usr/bin/env bash
set -euo pipefail

# Verifies that direct scanout actually engaged, and engaged lawfully, across
# the session logs of a `--direct` input-latency run.
#
# The counters alone cannot distinguish "the path was off" from "the scene was
# never eligible" from "the proof is wrong", so a run that flipped nothing
# fails with the eligibility verdict histogram printed rather than passing
# quietly on zeros. That histogram is the whole diagnostic value of a first
# run on a machine nobody has measured yet.
#
# Model: `validation/tla/PresentFlipOwnership.tla`.

fail() {
    printf 'direct scanout verification failed: %s\n' "$1" >&2
    exit 1
}

(( $# > 0 )) || fail "no session logs were given"

field() {
    local line="$1" key="$2" value
    value="$(printf '%s\n' "$line" | grep -oE "(^| )$key=[^ ]*" | tail -n1 || true)"
    value="${value#* }"
    value="${value#"$key="}"
    [[ -n "$value" ]] || return 1
    printf '%s\n' "$value"
}

total_attempts=0
total_flips=0
total_tests=0
total_rejections=0
total_refusals=0
total_fallbacks=0
total_eligible=0
sessions=0
verdict_names=()
verdict_totals=()
episode_sessions=0

for log in "$@"; do
    [[ -s "$log" ]] || continue
    sessions=$((sessions + 1))
    resources="$(grep -E '^sophia_live_native_resources schema=11 status=complete ' "$log" | tail -n1 || true)"
    [[ -n "$resources" ]] ||
        fail "session reported no schema-11 resource record, so it did not run this build: $log"
    for key in direct_scanout_attempts direct_scanout_flips direct_scanout_tests \
        direct_scanout_test_rejections direct_scanout_refusals direct_scanout_fallbacks; do
        value="$(field "$resources" "$key")" ||
            fail "resource record is missing $key: $log"
        [[ "$value" =~ ^[0-9]+$ ]] || fail "$key is not numeric: $value"
    done
    attempts="$(field "$resources" direct_scanout_attempts)"
    flips="$(field "$resources" direct_scanout_flips)"
    tests="$(field "$resources" direct_scanout_tests)"
    rejections="$(field "$resources" direct_scanout_test_rejections)"
    refusals="$(field "$resources" direct_scanout_refusals)"
    fallbacks="$(field "$resources" direct_scanout_fallbacks)"

    # A refusal is Engine's proof disagreeing with the pixels that proof was
    # computed from. There is no benign nonzero value: an ineligible frame
    # never becomes an attempt at all.
    (( refusals == 0 )) ||
        fail "the eligibility proof disagreed with the frame it lowered ($refusals times): $log"
    (( flips + fallbacks <= attempts )) ||
        fail "more direct attempts settled than were made: $log"
    (( rejections <= tests )) ||
        fail "more validating commits were refused than issued: $log"
    # A client buffer reaching a plane without the driver having been asked is
    # the one failure this row exists to make impossible.
    if (( flips > 0 )); then
        (( tests > 0 )) ||
            fail "a client buffer reached a plane with no validating commit: $log"
    fi

    verdicts="$(grep -E '^sophia_live_direct_scanout_verdicts schema=2 status=complete ' "$log" | tail -n1 || true)"
    [[ -n "$verdicts" ]] || fail "session reported no eligibility verdicts: $log"
    index=0
    for pair in $verdicts; do
        [[ "$pair" == *=* ]] || continue
        name="${pair%%=*}"
        count="${pair#*=}"
        [[ "$name" != schema && "$name" != status && "$name" != output && "$name" != head ]] ||
            continue
        [[ "$count" =~ ^[0-9]+$ ]] || fail "verdict $name is not numeric: $count"
        if (( sessions == 1 )); then
            verdict_names+=("$name")
            verdict_totals+=(0)
        fi
        [[ "${verdict_names[index]:-}" == "$name" ]] ||
            fail "sessions disagree on the verdict histogram's shape: $log"
        verdict_totals[index]=$(( verdict_totals[index] + count ))
        index=$((index + 1))
    done
    (( index == ${#verdict_names[@]} )) ||
        fail "sessions disagree on the verdict histogram's width: $log"

    # The episode records say the steps happened in a lawful order. A flip for
    # a scene that was never exported, or exported without a passing test, is
    # a sequence the counters cannot rule out on their own.
    if grep -q '^sophia_live_direct_scanout schema=1 ' "$log"; then
        episode_sessions=$((episode_sessions + 1))
        exported=0
        tested=0
        while read -r status; do
            case "$status" in
                exported) exported=1 ;;
                test_passed) (( exported == 1 )) ||
                    fail "a validating commit passed for a scene never exported: $log"
                    tested=1 ;;
                test_rejected) tested=0 ;;
                flipped)
                    (( exported == 1 )) ||
                        fail "a direct flip happened for a scene never exported: $log"
                    exported=0 ;;
                fell_back) exported=0; tested=0 ;;
                refused) : ;;
            esac
        done < <(grep -oE '^sophia_live_direct_scanout schema=1 status=[a-z_]+' "$log" |
            sed 's/.*status=//')
        # An episode still open at session end is ordinary: the last frame's
        # settlement can fall outside the bounded run.
        : "$tested"
    fi

    total_attempts=$((total_attempts + attempts))
    total_flips=$((total_flips + flips))
    total_tests=$((total_tests + tests))
    total_rejections=$((total_rejections + rejections))
    total_refusals=$((total_refusals + refusals))
    total_fallbacks=$((total_fallbacks + fallbacks))
done

(( sessions > 0 )) || fail "no session produced evidence"

histogram=""
for index in "${!verdict_names[@]}"; do
    histogram+=" ${verdict_names[index]}=${verdict_totals[index]}"
    if [[ "${verdict_names[index]}" == eligible ]]; then
        total_eligible="${verdict_totals[index]}"
    fi
done

printf 'sophia_direct_scanout_gate schema=1 sessions=%s attempts=%s flips=%s tests=%s test_rejections=%s refusals=%s fallbacks=%s episode_sessions=%s\n' \
    "$sessions" "$total_attempts" "$total_flips" "$total_tests" \
    "$total_rejections" "$total_refusals" "$total_fallbacks" "$episode_sessions"
printf 'sophia_direct_scanout_verdicts schema=1 sessions=%s%s\n' "$sessions" "$histogram"

if (( total_flips == 0 )); then
    printf 'No client buffer reached a plane. Per head:\n' >&2
    for log in "$@"; do
        grep -hE '^sophia_live_direct_scanout_verdicts schema=2 status=head ' "$log" 2>/dev/null |
            sed 's/^sophia_live_direct_scanout_verdicts schema=2 status=head /  /' >&2 || true
    done
    for log in "$@"; do
        grep -hE '^sophia_live_direct_scanout_geometry schema=1 ' "$log" 2>/dev/null |
            sed 's/^sophia_live_direct_scanout_geometry schema=1 /  measured: /' >&2 || true
    done
    printf 'The totals above say why:\n' >&2
    printf '  eligible=%s of %s lowered frames across %s sessions.\n' \
        "$total_eligible" \
        "$(( $(IFS=+; echo "$(printf '%s+' "${verdict_totals[@]}")0") ))" \
        "$sessions" >&2
    fail "direct scanout never engaged"
fi

printf 'direct scanout verification passed: %s sessions, %s flips\n' "$sessions" "$total_flips"
