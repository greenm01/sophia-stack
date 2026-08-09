#!/bin/sh
set -eu

: "${SOPHIA_HAGIA_BIN:?set SOPHIA_HAGIA_BIN to the Hagia executable}"
: "${SOPHIA_HAGIA_RESTART_MARKER:?set SOPHIA_HAGIA_RESTART_MARKER}"

if [ ! -x "$SOPHIA_HAGIA_BIN" ]; then
    echo "Hagia executable is unavailable: $SOPHIA_HAGIA_BIN" >&2
    exit 2
fi

if [ -n "${SOPHIA_HAGIA_FAULT_AFTER:-}" ]; then
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
