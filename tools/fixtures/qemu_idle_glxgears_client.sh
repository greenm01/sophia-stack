#!/bin/sh
set -eu

producer_pid=

stop_producer() {
    trap - HUP INT TERM EXIT
    if [ -n "$producer_pid" ] && kill -0 "$producer_pid" 2>/dev/null; then
        # A stopped process cannot consume the session supervisor's TERM.
        # Resume it before forwarding shutdown so the process group drains.
        kill -CONT "$producer_pid" 2>/dev/null || true
        kill -TERM "$producer_pid" 2>/dev/null || true
        wait "$producer_pid" 2>/dev/null || true
    fi
}

trap 'stop_producer; exit 143' HUP INT TERM
trap stop_producer EXIT

/usr/bin/glxgears -swapinterval 1 -geometry 636x796 &
producer_pid=$!

# Give the unmodified client a bounded animation interval. The host gate proves
# that real DMA-BUF retirements occurred before accepting the frozen marker.
sleep 2
if ! kill -0 "$producer_pid" 2>/dev/null; then
    set +e
    wait "$producer_pid"
    producer_status=$?
    set -e
    echo "sophia_qemu_idle_client schema=1 status=failed reason=producer_exit exit_status=$producer_status"
    exit 1
fi

kill -STOP "$producer_pid"
echo "sophia_qemu_idle_client schema=1 status=frozen producer=glxgears"

while kill -0 "$producer_pid" 2>/dev/null; do
    sleep 1
done
set +e
wait "$producer_pid"
producer_status=$?
set -e
echo "sophia_qemu_idle_client schema=1 status=failed reason=unexpected_exit exit_status=$producer_status"
exit 1
