#!/usr/bin/env bash
set -euo pipefail

session_log="${1:-}"
minimum_msec="${2:-7200000}"
minimum_terminal_actions="${3:-10}"
minimum_firefox_actions="${4:-5}"
[[ -s "$session_log" ]] || {
    echo "usage: tools/verify_installed_session_soak.sh SESSION_LOG [MIN_MSEC [MIN_TERMINALS [MIN_FIREFOX]]]" >&2
    exit 1
}
for value in "$minimum_msec" "$minimum_terminal_actions" "$minimum_firefox_actions"; do
    [[ "$value" =~ ^[0-9]+$ ]] || {
        echo "soak thresholds must be nonnegative integers" >&2
        exit 1
    }
done
if grep -Eqi '(^|[[:space:]])(panic|error:|status=(failed|degraded))' "$session_log"; then
    echo "soak log contains an error, panic, or degraded status" >&2
    exit 1
fi
mapfile -t completions < <(
    grep -E '^sophia_live_session schema=14 status=bounded_complete ' "$session_log" || true
)
(( ${#completions[@]} == 1 )) || {
    echo "soak requires exactly one schema-14 completion; found ${#completions[@]}" >&2
    exit 1
}
completion="${completions[0]}"
field() {
    local token
    for token in $completion; do
        [[ "$token" == "$1="* ]] && {
            printf '%s\n' "${token#*=}"
            return
        }
    done
    return 1
}
elapsed="$(field elapsed_msec)"
[[ "$elapsed" =~ ^[0-9]+$ ]] && (( elapsed >= minimum_msec )) || {
    echo "soak duration ${elapsed:-missing}ms is below ${minimum_msec}ms" >&2
    exit 1
}
terminal_actions="$(
    grep -Ec '^sophia_session_app schema=1 status=started id=terminal source=action$' \
        "$session_log" || true
)"
firefox_actions="$(
    grep -Ec '^sophia_session_app schema=1 status=started id=firefox source=action$' \
        "$session_log" || true
)"
layout_commits="$(
    grep -Ec '^sophia_live_wm schema=1 status=layout_committed .* outcome=Committed$' \
        "$session_log" || true
)"
focus_commits="$(
    grep -Ec '^sophia_live_wm schema=1 status=focus_committed .* target=surface$' \
        "$session_log" || true
)"
close_actions="$(
    grep -Ec '^sophia_live_wm schema=1 status=session_action_committed .* action=CloseFocused$' \
        "$session_log" || true
)"
(( terminal_actions >= minimum_terminal_actions )) || {
    echo "soak has $terminal_actions terminal actions; $minimum_terminal_actions required" >&2
    exit 1
}
(( firefox_actions >= minimum_firefox_actions )) || {
    echo "soak has $firefox_actions Firefox actions; $minimum_firefox_actions required" >&2
    exit 1
}
(( layout_commits >= minimum_terminal_actions + minimum_firefox_actions )) || {
    echo "soak has $layout_commits layout commits; $((minimum_terminal_actions + minimum_firefox_actions)) required" >&2
    exit 1
}
(( focus_commits > 0 )) || {
    echo "soak has no committed focus changes" >&2
    exit 1
}
(( close_actions >= minimum_firefox_actions )) || {
    echo "soak has $close_actions close actions; $minimum_firefox_actions required" >&2
    exit 1
}
grep -Eq '^sophia_firefox_m8 schema=1 status=complete stages=6 selection_owner_changes=[2-9][0-9]* selection_conversions=[2-9][0-9]* content=redacted$' \
    "$session_log" || {
    echo "soak is missing the complete redacted Firefox interaction proof" >&2
    exit 1
}
grep -Eq '^sophia_live_session_protocol_errors schema=1 expected=[0-9]+ unexpected=0$' \
    "$session_log" || {
    echo "soak protocol-error summary is missing or nonzero" >&2
    exit 1
}
outputs="$(
    grep -Ec '^sophia_live_output schema=1 status=complete .*callbacks=[1-9][0-9]* .*nonzero_exports=[1-9][0-9]*$' \
        "$session_log" || true
)"
(( outputs >= 2 )) || {
    echo "soak has $outputs clean output summaries; two required" >&2
    exit 1
}
grep -Eq '^sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 namespace=revoked xauthority=removed$' \
    "$session_log" || {
    echo "soak final process/frontend cleanup is not clean" >&2
    exit 1
}
for positive in physical_events physical_keys_routed physical_pointer_events \
    physical_pointer_routed wm_requests wm_committed native_submissions \
    native_retirements native_frame_uploads; do
    actual="$(field "$positive")"
    [[ "$actual" =~ ^[0-9]+$ ]] && (( actual > 0 )) || {
        echo "soak completion has no $positive activity" >&2
        exit 1
    }
done
for assignment in \
    wm_degraded=false \
    native_submit_failures=0 \
    native_retire_failures=0 \
    native_callback_rejected=0 \
    native_in_flight=false \
    native_cleanup_pending=false \
    present_disconnect_failures=0 \
    present_live_sources=0 \
    present_live_fences=0 \
    present_live_transactions=0; do
    actual="$(field "${assignment%%=*}")"
    [[ "$actual" == "${assignment#*=}" ]] || {
        echo "soak completion violates $assignment (actual=${actual:-missing})" >&2
        exit 1
    }
done
grep -Eq '^sophia_live_session_health schema=1 status=clean .* pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false$' \
    "$session_log" || {
    echo "soak final health is not clean" >&2
    exit 1
}

echo "installed Sophia soak gate passed: elapsed_msec=$elapsed terminal_actions=$terminal_actions firefox_actions=$firefox_actions layout_commits=$layout_commits close_actions=$close_actions outputs=$outputs"
