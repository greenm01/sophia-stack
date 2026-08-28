#!/usr/bin/env bash
set -euo pipefail

# Captures the kernel's side of a page-flip stall into the latency evidence.
#
# Nine flips went missing in one day and the stall record now attributes the
# Sophia side; this collects the other half. Run it from any terminal after a
# latency run -- it finds the newest evidence directory itself, so nothing has
# to be copied between TTYs. sudo will prompt once: the socklog kernel log and
# the kernel ring buffer are root-readable only on this host.
#
# Usage: tools/collect_sophia_kernel_stall_log.sh [EVIDENCE_DIR]

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
SOCKLOG_KERNEL=/var/log/socklog/kernel/current

evidence="${1:-}"
if [[ -z "$evidence" ]]; then
    evidence="$(ls -dt "$STATE_HOME"/sophia/rendering-benchmarks/*/input-latency/.*.pending \
        "$STATE_HOME"/sophia/rendering-benchmarks/*/input-latency/2* 2>/dev/null | head -1 || true)"
fi
[[ -n "$evidence" && -d "$evidence" ]] || {
    echo "no input-latency evidence directory found; pass one explicitly" >&2
    exit 1
}
echo "Evidence: $evidence"

# The stalls this capture is for, so the correlation is visible immediately.
stall_lines="$(grep -rhoE \
    'sophia_live_native_page_flip_stall schema=[0-9]+ status=hard_stall[^"]*' \
    "$evidence" 2>/dev/null | sort -u || true)"
if [[ -n "$stall_lines" ]]; then
    echo "Stalls recorded in this run:"
    printf '%s\n' "$stall_lines" | sed 's/^/  /'
else
    echo "No stall records in this evidence; capturing the kernel log anyway."
fi

capture="$evidence/kernel.log"
: >"$capture"
chmod 600 "$capture"

captured_any=false
if sudo test -r "$SOCKLOG_KERNEL"; then
    {
        echo "=== source=$SOCKLOG_KERNEL captured=$(date -u +%Y%m%dT%H%M%SZ) ==="
        if command -v tai64nlocal >/dev/null; then
            sudo cat "$SOCKLOG_KERNEL" | tai64nlocal
        else
            sudo cat "$SOCKLOG_KERNEL"
        fi
    } >>"$capture"
    captured_any=true
else
    echo "socklog kernel log is not readable even with sudo; skipping" >&2
fi
if sudo dmesg >/dev/null 2>&1; then
    {
        echo "=== source=dmesg captured=$(date -u +%Y%m%dT%H%M%SZ) ==="
        sudo dmesg --time-format iso 2>/dev/null || sudo dmesg
    } >>"$capture"
    captured_any=true
else
    echo "kernel ring buffer is not readable even with sudo; skipping" >&2
fi
"$captured_any" || {
    echo "no kernel source could be read; nothing captured" >&2
    exit 1
}

lines="$(wc -l <"$capture")"
echo "Captured $lines kernel log lines to $capture"
echo
echo "Display-related tail:"
grep -iE 'drm|amdgpu|flip|vblank|link|dp-|displayport' "$capture" | tail -40 |
    sed 's/^/  /' || echo "  (no display-related lines matched)"
