#!/usr/bin/env bash
set -euo pipefail

profile="${1:-}"
case "$profile" in
    kitty|native|xmonad) ;;
    *)
        echo "usage: tools/stop_sophia_session.sh kitty|native|xmonad" >&2
        exit 2
        ;;
esac

state_dir="${XDG_RUNTIME_DIR:-/tmp}/sophia-${profile}-session-${UID}"
pid_file="$state_dir/wrapper.pid"

if [[ ! -s "$pid_file" ]]; then
    echo "No Sophia $profile session is recorded."
    exit 0
fi
wrapper_pid="$(<"$pid_file")"
if [[ ! "$wrapper_pid" =~ ^[0-9]+$ ]]; then
    echo "Invalid Sophia $profile session state: $pid_file" >&2
    exit 1
fi
if ! kill -0 "$wrapper_pid" 2>/dev/null; then
    rm -f "$pid_file"
    echo "Removed stale Sophia $profile session state."
    exit 0
fi

echo "Stopping Sophia $profile session (wrapper PID $wrapper_pid)..."
kill -TERM "$wrapper_pid"
for _ in {1..50}; do
    if ! kill -0 "$wrapper_pid" 2>/dev/null; then
        echo "Sophia $profile session stopped."
        exit 0
    fi
    sleep 0.1
done

echo "Sophia $profile wrapper did not stop within five seconds." >&2
echo "Inspect it with: ps -o pid,ppid,pgid,tty,stat,args -p $wrapper_pid" >&2
exit 1
