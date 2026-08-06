#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 2 ]]; then
    echo "usage: $0 TRACE_FIFO EVIDENCE_FILE" >&2
    exit 2
fi

trace_fifo="$1"
evidence_file="$2"

# QEMU's raw input trace contains key and pointer values. Reduce the stream
# before persistence so interactive evidence records only boundary crossings.
awk '
    /vnc_client_connect/ && !display {
        print "sophia_qemu_interactive schema=1 status=display_attached backend=vnc-unix"
        fflush()
        display = 1
    }
    /vnc_msg_client_(ext_)?key_event/ {
        host_keys++
        if (host_keys == 1 || host_keys % 8 == 0) {
            print "sophia_qemu_interactive schema=2 status=host_input_delivered kind=keyboard count=" host_keys
            fflush()
        }
    }
    /vnc_msg_client_pointer_event/ && !host_pointer {
        print "sophia_qemu_interactive schema=1 status=host_input_delivered kind=pointer"
        fflush()
        host_pointer = 1
    }
    /input_event_key_(number|qcode)/ {
        guest_keys++
        if (guest_keys == 1 || guest_keys % 8 == 0) {
            print "sophia_qemu_interactive schema=2 status=qemu_input_delivered kind=keyboard count=" guest_keys
            fflush()
        }
    }
    /input_event_(rel|abs)/ && !guest_motion {
        print "sophia_qemu_interactive schema=1 status=qemu_input_delivered kind=motion"
        fflush()
        guest_motion = 1
    }
    /input_event_btn/ && !guest_button {
        print "sophia_qemu_interactive schema=1 status=qemu_input_delivered kind=button"
        fflush()
        guest_button = 1
    }
' "$trace_fifo" | tee -a "$evidence_file"
