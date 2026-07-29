#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
LOG_DIR="${SOPHIA_STANDALONE_LOG_DIR:-$STATE_HOME/sophia/standalone-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"
BASELINE_FPS="${SOPHIA_RENDER_BASELINE_FPS:-}"
BASELINE_P95_MSEC="${SOPHIA_RENDER_BASELINE_P95_MSEC:-}"
MIN_BASELINE_RATIO="${SOPHIA_RENDER_MIN_BASELINE_RATIO:-0.90}"

fail() {
    echo "Sophia rendering performance report failed: $*" >&2
    exit 1
}

field() {
    local line="$1" key="$2" token
    for token in $line; do
        if [[ "$token" == "$key="* ]]; then
            printf '%s\n' "${token#*=}"
            return 0
        fi
    done
    return 1
}

[[ -s "$SESSION_LOG" ]] || fail "missing session log: $SESSION_LOG"

completion="$(
    grep -E '^sophia_live_session schema=15 status=bounded_complete ' "$SESSION_LOG" |
        tail -n 1
)"
[[ -n "$completion" ]] || fail "missing bounded session completion"
efficiency="$(
    grep -E '^sophia_live_rendering_efficiency schema=1 status=complete ' "$SESSION_LOG" |
        tail -n 1
)"
[[ -n "$efficiency" ]] || fail "missing rendering-efficiency evidence"

TIMESTAMPS_FILE="$(mktemp)"
INTERVALS_FILE="$(mktemp)"
trap 'rm -f "$TIMESTAMPS_FILE" "$INTERVALS_FILE"' EXIT

awk '
    /^sophia_live_session_present_feedback schema=1 kind=complete / &&
        / routed=true / && / mode=Flip / {
        for (field_index = 1; field_index <= NF; field_index++) {
            if ($field_index ~ /^ust=[0-9]+$/) {
                split($field_index, pair, "=")
                print pair[2]
            }
        }
    }
' "$SESSION_LOG" >"$TIMESTAMPS_FILE"

timestamp_count="$(wc -l <"$TIMESTAMPS_FILE")"
((timestamp_count >= 3)) ||
    fail "need at least three routed Flip timestamps, observed $timestamp_count"

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
' "$TIMESTAMPS_FILE" 2>"$INTERVALS_FILE.fps" | sort -n >"$INTERVALS_FILE"

fps="$(cat "$INTERVALS_FILE.fps")"
rm -f "$INTERVALS_FILE.fps"
[[ -n "$fps" ]] || fail "Present timestamps did not advance"
interval_count="$(wc -l <"$INTERVALS_FILE")"
((interval_count > 0)) || fail "Present timestamps contained no positive interval"
p95_index="$(
    awk -v count="$interval_count" 'BEGIN { print int((count * 95 + 99) / 100) }'
)"
p95_msec="$(sed -n "${p95_index}p" "$INTERVALS_FILE")"

native_retirements="$(field "$completion" native_retirements)" ||
    fail "completion lacks native_retirements"
native_max_upload_msec="$(field "$completion" native_max_upload_msec)" ||
    fail "completion lacks native_max_upload_msec"
cpu_max_compose_msec="$(field "$completion" cpu_max_compose_msec)" ||
    fail "completion lacks cpu_max_compose_msec"
cpu_replacements="$(field "$efficiency" cpu_replacements)" ||
    fail "efficiency evidence lacks cpu_replacements"
cpu_patch_updates="$(field "$efficiency" cpu_patch_updates)" ||
    fail "efficiency evidence lacks cpu_patch_updates"
cpu_patch_rects="$(field "$efficiency" cpu_patch_rects)" ||
    fail "efficiency evidence lacks cpu_patch_rects"
cpu_payload_bytes="$(field "$efficiency" cpu_payload_bytes)" ||
    fail "efficiency evidence lacks cpu_payload_bytes"
target_reuses="$(field "$efficiency" composition_target_reuses)" ||
    fail "efficiency evidence lacks composition_target_reuses"

status=pass
if [[ -n "$BASELINE_FPS" ]]; then
    awk -v observed="$fps" -v baseline="$BASELINE_FPS" -v ratio="$MIN_BASELINE_RATIO" \
        'BEGIN { exit !(observed >= baseline * ratio) }' ||
        status=below_baseline
fi
if [[ -n "$BASELINE_P95_MSEC" ]]; then
    awk -v observed="$p95_msec" -v baseline="$BASELINE_P95_MSEC" \
        -v ratio="$MIN_BASELINE_RATIO" \
        'BEGIN { exit !(observed <= baseline / ratio) }' ||
        status=below_baseline
fi

printf '%s\n' \
    "sophia_rendering_performance schema=1 status=$status flip_samples=$timestamp_count fps=$fps p95_frame_msec=$p95_msec native_retirements=$native_retirements cpu_max_compose_msec=$cpu_max_compose_msec native_max_upload_msec=$native_max_upload_msec cpu_replacements=$cpu_replacements cpu_patch_updates=$cpu_patch_updates cpu_patch_rects=$cpu_patch_rects cpu_payload_bytes=$cpu_payload_bytes composition_target_reuses=$target_reuses baseline_fps=${BASELINE_FPS:-none} baseline_p95_msec=${BASELINE_P95_MSEC:-none} min_baseline_ratio=$MIN_BASELINE_RATIO"

[[ "$status" == pass ]] || fail "observed cadence is below the configured baseline gate"
