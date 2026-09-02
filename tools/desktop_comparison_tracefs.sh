#!/bin/sh
# Narrow root adapter for one local desktop-comparison kernel timing capture.
set -eu

if [ "$#" -ne 4 ]; then
    echo "usage: desktop_comparison_tracefs.sh OUTPUT READY START STOP" >&2
    exit 64
fi
if [ "$(id -u)" -ne 0 ] || [ -z "${SUDO_UID:-}" ]; then
    echo "desktop comparison tracefs adapter must run through sudo" >&2
    exit 77
fi

output=$1
ready=$2
start=$3
stop=$4
parent=$(readlink -f "$(dirname "$output")")
if [ "$(readlink -f "$(dirname "$ready")")" != "$parent" ] \
    || [ "$(readlink -f "$(dirname "$start")")" != "$parent" ] \
    || [ "$(readlink -f "$(dirname "$stop")")" != "$parent" ]; then
    echo "trace adapter paths must share one resolved attempt directory" >&2
    exit 65
fi
if [ "$(basename "$output")" != kernel-trace.raw ] \
    || [ "$(basename "$ready")" != trace.ready ] \
    || [ "$(basename "$start")" != trace.start ] \
    || [ "$(basename "$stop")" != trace.stop ]; then
    echo "trace adapter received an unexpected artifact name" >&2
    exit 65
fi
if [ ! -d "$parent" ] || [ -L "$parent" ] \
    || [ "$(stat -c %u "$parent")" != "$SUDO_UID" ]; then
    echo "trace attempt directory is not owned by the invoking user" >&2
    exit 77
fi
case "$(stat -c %a "$parent")" in
    *[2367][0-9]|*[0-9][2367])
        echo "trace attempt directory is group/world writable" >&2
        exit 77
        ;;
esac
if [ -e "$output" ] || [ -e "$ready" ] || [ -e "$start" ] || [ -e "$stop" ]; then
    echo "trace adapter refuses pre-existing artifacts" >&2
    exit 73
fi

trace_root=
for candidate in /sys/kernel/tracing /sys/kernel/debug/tracing; do
    if [ -f "$candidate/events/drm/drm_vblank_event_delivered/enable" ]; then
        trace_root=$candidate
        break
    fi
done
if [ -z "$trace_root" ]; then
    echo "drm_vblank_event_delivered tracepoint is unavailable" >&2
    exit 69
fi

instance="$trace_root/instances/sophia-desktop-comparison-$SUDO_UID-$$"
reader=
cleanup() {
    if [ -d "$instance" ]; then
        printf '0\n' >"$instance/tracing_on" 2>/dev/null || true
        printf '0\n' >"$instance/events/drm/drm_vblank_event_delivered/enable" 2>/dev/null || true
        if [ -f "$instance/events/drm/drm_vblank_event_queued/enable" ]; then
            printf '0\n' >"$instance/events/drm/drm_vblank_event_queued/enable" 2>/dev/null || true
        fi
    fi
    if [ -n "$reader" ]; then
        kill "$reader" 2>/dev/null || true
        wait "$reader" 2>/dev/null || true
    fi
    if [ -d "$instance" ]; then
        rmdir "$instance" 2>/dev/null || true
    fi
}
trap cleanup EXIT HUP INT TERM

mkdir "$instance"
umask 022
: >"$output"
printf '0\n' >"$instance/tracing_on"
printf '\n' >"$instance/trace"
printf '1\n' >"$instance/events/drm/drm_vblank_event_delivered/enable"
if [ -f "$instance/events/drm/drm_vblank_event_queued/enable" ]; then
    printf '1\n' >"$instance/events/drm/drm_vblank_event_queued/enable"
fi
cat "$instance/trace_pipe" >>"$output" &
reader=$!
touch "$ready"

while [ ! -f "$start" ]; do
    if [ -f "$stop" ]; then
        exit 70
    fi
    sleep 0.02
done
printf '1\n' >"$instance/tracing_on"
while [ ! -f "$stop" ]; do
    sleep 0.02
done
printf '0\n' >"$instance/tracing_on"
sleep 0.05
