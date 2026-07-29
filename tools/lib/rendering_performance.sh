#!/usr/bin/env bash

rendering_performance_field() {
    local line="$1" key="$2" token
    for token in $line; do
        if [[ "$token" == "$key="* ]]; then
            printf '%s\n' "${token#*=}"
            return 0
        fi
    done
    return 1
}

rendering_performance_cadence() {
    local timestamps_file="$1" intervals_file="$2"
    local timestamp_count interval_count p95_index fps p95_msec

    timestamp_count="$(wc -l <"$timestamps_file")"
    ((timestamp_count >= 3)) || return 1
    awk '
        NR == 1 { previous = $1; next }
        $1 <= previous { exit 1 }
        { previous = $1 }
    ' "$timestamps_file" || return 1

    awk '
        NR == 1 { previous = $1; first = $1; next }
        {
            if ($1 > previous) {
                print ($1 - previous) / 1000
            }
            previous = $1
            last = $1
        }
        END {
            if (last > first) {
                printf "%.3f\n", (NR - 1) * 1000000 / (last - first) > "/dev/stderr"
            }
        }
    ' "$timestamps_file" 2>"$intervals_file.fps" | sort -n >"$intervals_file"

    fps="$(<"$intervals_file.fps")"
    rm -f "$intervals_file.fps"
    [[ -n "$fps" ]] || return 1
    interval_count="$(wc -l <"$intervals_file")"
    ((interval_count > 0)) || return 1
    p95_index="$(
        awk -v count="$interval_count" 'BEGIN { print int((count * 95 + 99) / 100) }'
    )"
    p95_msec="$(sed -n "${p95_index}p" "$intervals_file")"
    [[ -n "$p95_msec" ]] || return 1

    printf '%s %s %s\n' "$timestamp_count" "$fps" "$p95_msec"
}
