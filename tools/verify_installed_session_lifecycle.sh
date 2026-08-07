#!/usr/bin/env bash
set -euo pipefail

lifecycle="${1:-}"
mode="${2:-normal}"
[[ -s "$lifecycle" \
    && ( "$mode" == normal || "$mode" == cycle \
        || "$mode" == emergency || "$mode" == watchdog ) ]] || {
    echo "usage: tools/verify_installed_session_lifecycle.sh LIFECYCLE_LOG [normal|cycle|emergency|watchdog]" >&2
    exit 1
}
fail() {
    echo "installed lifecycle verification failed: $*" >&2
    exit 1
}
line_number() {
    grep -nFm1 "$1" "$lifecycle" | cut -d: -f1
}
require_phase() {
    local status="$1" phase="$2"
    grep -Fq "sophia_session_lifecycle schema=1 status=$status phase=$phase installed=true build=false manual_service=false runtime=owner vt=local" \
        "$lifecycle" || fail "missing installed $status/$phase phase"
}

require_phase entering preflight
require_phase complete preflight
require_phase entering input_guard
require_phase complete input_guard
require_phase entering graphics_takeover
require_phase complete graphics_takeover
require_phase entering session

previous=0
for marker in \
    'status=entering phase=preflight ' \
    'status=complete phase=preflight ' \
    'status=entering phase=input_guard ' \
    'status=complete phase=input_guard ' \
    'status=entering phase=graphics_takeover ' \
    'status=complete phase=graphics_takeover ' \
    'status=entering phase=session '; do
    current="$(line_number "$marker")"
    [[ "$current" =~ ^[0-9]+$ ]] || fail "missing lifecycle marker: $marker"
    (( current > previous )) || fail "lifecycle phases are out of order"
    previous="$current"
done

case "$mode" in
    normal)
        returned='sophia_session_lifecycle schema=1 status=returned phase=handoff installed=true exit_status=0 emergency=false handoff=display_manager'
        ;;
    cycle)
        returned='sophia_session_lifecycle schema=1 status=returned phase=handoff installed=true exit_status=0 emergency=false handoff=cycle_runner'
        ;;
    emergency)
        returned='sophia_session_lifecycle schema=1 status=returned phase=handoff installed=true exit_status=130 emergency=true handoff=display_manager'
        ;;
    watchdog)
        returned='sophia_session_lifecycle schema=1 status=returned phase=handoff installed=true exit_status=124 emergency=true handoff=display_manager'
        grep -Eq '^sophia_session_diagnostic schema=1 status=failed phase=session installed=true version=[0-9A-Za-z._-]+ commit=[0-9A-Za-z._-]+ exit_status=124$' \
            "$lifecycle" || fail "watchdog deadline diagnostic is missing"
        ;;
esac
grep -Fxq "$returned" "$lifecycle" || fail "handoff does not match $mode recovery"
returned_line="$(line_number "$returned")"
(( returned_line > previous )) || fail "handoff preceded the session"

echo "installed Sophia lifecycle verified: mode=$mode log=$lifecycle"
