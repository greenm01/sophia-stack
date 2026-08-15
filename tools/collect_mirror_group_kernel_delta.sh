#!/usr/bin/env bash
set -euo pipefail

before="${1:?usage: collect_mirror_group_kernel_delta.sh BEFORE AFTER DELTA [MAX_LINES]}"
after="${2:?usage: collect_mirror_group_kernel_delta.sh BEFORE AFTER DELTA [MAX_LINES]}"
delta="${3:?usage: collect_mirror_group_kernel_delta.sh BEFORE AFTER DELTA [MAX_LINES]}"
max_lines="${4:-256}"

[[ -r "$before" && -r "$after" ]] || {
    echo "mirror-group kernel snapshots are not readable" >&2
    exit 1
}
[[ "$max_lines" =~ ^[1-9][0-9]*$ ]] && (( max_lines <= 4096 )) || {
    echo "mirror-group kernel delta limit must be between 1 and 4096 lines" >&2
    exit 1
}

work="$(mktemp)"
trap 'rm -f -- "$work"' EXIT
before_lines="$(wc -l <"$before")"
after_lines="$(wc -l <"$after")"
continuity=reset
if (( after_lines >= before_lines )) &&
    cmp -s "$before" <(head -n "$before_lines" "$after"); then
    continuity=append
    tail -n "+$((before_lines + 1))" "$after" >"$work"
else
    cp "$after" "$work"
fi

total_lines="$(wc -l <"$work")"
truncated=false
if (( total_lines > max_lines )); then
    truncated=true
fi
tail -n "$max_lines" "$work" >"$delta"
lines="$(wc -l <"$delta")"
printf 'availability=available continuity=%s lines=%s total_lines=%s truncated=%s\n' \
    "$continuity" "$lines" "$total_lines" "$truncated"
