#!/bin/sh
set -eu

: "${SOPHIA_HAGIA_BIN:?set SOPHIA_HAGIA_BIN to the Hagia executable}"
: "${SOPHIA_HAGIA_RESTART_MARKER:?set SOPHIA_HAGIA_RESTART_MARKER}"

if [ ! -x "$SOPHIA_HAGIA_BIN" ]; then
    echo "Hagia executable is unavailable: $SOPHIA_HAGIA_BIN" >&2
    exit 2
fi

if [ ! -e "$SOPHIA_HAGIA_RESTART_MARKER" ]; then
    : >"$SOPHIA_HAGIA_RESTART_MARKER"
    target_pid=$$
    (
        sleep 2
        kill -TERM "$target_pid"
    ) &
fi

exec "$SOPHIA_HAGIA_BIN" "$@"
