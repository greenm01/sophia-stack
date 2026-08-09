#!/bin/sh
set -eu

: "${SOPHIA_HAGIA_BIN:?set SOPHIA_HAGIA_BIN to the Hagia executable}"
: "${SOPHIA_HAGIA_RESTART_MARKER:?set SOPHIA_HAGIA_RESTART_MARKER}"

if [ ! -x "$SOPHIA_HAGIA_BIN" ]; then
    echo "Hagia executable is unavailable: $SOPHIA_HAGIA_BIN" >&2
    exit 2
fi

if [ -n "${SOPHIA_HAGIA_RESTART_AFTER_ACTION:-}" ] \
    && [ ! -e "$SOPHIA_HAGIA_RESTART_MARKER" ]; then
    : "${SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE:?set evidence for an action-correlated restart}"
    case "$SOPHIA_HAGIA_RESTART_AFTER_ACTION" in
        *[!0-9]*|'')
            echo "SOPHIA_HAGIA_RESTART_AFTER_ACTION must be an action number" >&2
            exit 2
            ;;
    esac
    case "${SOPHIA_HAGIA_RESTART_REQUIRES_ACTION:-}" in
        *[!0-9]*)
            echo "SOPHIA_HAGIA_RESTART_REQUIRES_ACTION must be an action number" >&2
            exit 2
            ;;
    esac

    target_pid=$$
    (
        while kill -0 "$target_pid" 2>/dev/null; do
            trigger_line="$(awk -v action="$SOPHIA_HAGIA_RESTART_AFTER_ACTION" '
                $0 == "sophia_live_wm schema=1 status=physical_action_committed action=" action {
                    print NR
                    exit
                }
            ' "$SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE" 2>/dev/null || true)"
            prerequisite_line="0"
            if [ -n "${SOPHIA_HAGIA_RESTART_REQUIRES_ACTION:-}" ]; then
                prerequisite_line="$(awk -v action="$SOPHIA_HAGIA_RESTART_REQUIRES_ACTION" '
                    $0 == "sophia_live_wm schema=1 status=physical_action_committed action=" action {
                        print NR
                        exit
                    }
                ' "$SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE" 2>/dev/null || true)"
            fi
            if [ -n "$trigger_line" ] \
                && [ -n "$prerequisite_line" ] \
                && [ "$prerequisite_line" -lt "$trigger_line" ] \
                && awk -v trigger="$trigger_line" '
                    NR > trigger && (
                        /^hagia_policy_checkpoint schema=1 status=saved candidate_nonempty=true$/ ||
                        / event=checkpoint status=saved detail="candidate_nonempty=true"$/
                    ) {
                        found = 1
                        exit
                    }
                    END { exit found ? 0 : 1 }
                ' "$SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE" 2>/dev/null; then
                : >"$SOPHIA_HAGIA_RESTART_MARKER"
                kill -KILL "$target_pid" 2>/dev/null || true
                exit 0
            fi
            sleep 0.05
        done
    ) &
elif [ -n "${SOPHIA_HAGIA_FAULT_AFTER:-}" ]; then
    export HAGIA_POLICY_FAULT_AFTER="$SOPHIA_HAGIA_FAULT_AFTER"
    export HAGIA_POLICY_FAULT_MARKER="$SOPHIA_HAGIA_RESTART_MARKER"
    export HAGIA_POLICY_FAULT_OCCURRENCE="${SOPHIA_HAGIA_FAULT_OCCURRENCE:-1}"
    export HAGIA_POLICY_FAULT_DELAY_MSEC="${SOPHIA_HAGIA_FAULT_DELAY_MSEC:-0}"
elif [ ! -e "$SOPHIA_HAGIA_RESTART_MARKER" ]; then
    : >"$SOPHIA_HAGIA_RESTART_MARKER"
    target_pid=$$
    (
        sleep 2
        kill -TERM "$target_pid"
    ) &
fi

exec "$SOPHIA_HAGIA_BIN" "$@"
