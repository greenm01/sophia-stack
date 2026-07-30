#!/usr/bin/env bash
set -euo pipefail

# Terminal CPU-path throughput reporter. Reduces a bounded xterm standalone
# session log to one fail-closed sophia_terminal_performance schema=1 line.
# Unlike the vkcube/glxgears reporters (GPU DRI3 flip path), this asserts the
# software-Present (CPU) evidence: positive immutable patch-batch traffic,
# damage-driven partial repaint, bounded CPU compose time, and clean teardown.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=tools/lib/rendering_performance.sh
source "$ROOT_DIR/tools/lib/rendering_performance.sh"

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
LOG_DIR="${SOPHIA_STANDALONE_LOG_DIR:-$STATE_HOME/sophia/standalone-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"

fail() {
    echo "Sophia terminal performance report failed: $*" >&2
    exit 1
}

positive_field() {
    local line="$1" key="$2" value
    value="$(rendering_performance_field "$line" "$key")" ||
        fail "line lacks $key"
    [[ "$value" =~ ^[0-9]+$ ]] || fail "$key is not an integer"
    ((value > 0)) || fail "$key is not positive"
    printf '%s\n' "$value"
}

nonnegative_field() {
    local line="$1" key="$2" value
    value="$(rendering_performance_field "$line" "$key")" ||
        fail "line lacks $key"
    [[ "$value" =~ ^[0-9]+$ ]] || fail "$key is not a nonnegative integer"
    printf '%s\n' "$value"
}

[[ -s "$SESSION_LOG" ]] || fail "missing session log: $SESSION_LOG"

if grep -Eqi \
    '(^Error:|panicked at|admission_group_(invalid|overflowed)|mismatched.transaction|status=(failed|degraded)([[:space:]]|$))' \
    "$SESSION_LOG"; then
    fail "session contains an error, invalid admission group, or degraded status"
fi

benchmark="$(
    grep -E '^sophia_terminal_benchmark schema=1 workload=xterm-cpu ' "$SESSION_LOG" |
        tail -n 1 || true
)"
[[ -n "$benchmark" ]] || fail "missing terminal benchmark metadata"
client="$(
    grep -E '^sophia_xterm_client schema=1 status=complete ' "$SESSION_LOG" |
        tail -n 1 || true
)"
[[ -n "$client" ]] || fail "missing bounded xterm client completion"
completion="$(
    grep -E '^sophia_live_session schema=16 status=bounded_complete ' "$SESSION_LOG" |
        tail -n 1 || true
)"
[[ -n "$completion" ]] || fail "missing bounded Sophia session completion"
grep -Eq '^sophia_live_session_protocol_errors schema=1 expected=[0-9]+ unexpected=0$' \
    "$SESSION_LOG" || fail "session contains unexpected X11 protocol errors"
grep -Eq '^sophia_live_session_cleanup schema=1 status=clean ' "$SESSION_LOG" ||
    fail "session cleanup was not clean"

for assignment in \
    native_presentation=enabled \
    native_submit_failures=0 \
    native_retire_failures=0 \
    native_in_flight=false \
    native_cleanup_pending=false \
    wm_degraded=false \
    present_live_sources=0 \
    present_live_fences=0 \
    present_live_transactions=0; do
    [[ " $completion " == *" $assignment "* ]] ||
        fail "completion does not contain $assignment"
done

# Benchmark metadata / client throughput.
duration_seconds="$(positive_field "$benchmark" duration_seconds)"
surface_width="$(positive_field "$benchmark" surface_width)"
surface_height="$(positive_field "$benchmark" surface_height)"
client_lines="$(positive_field "$client" lines)"
client_iterations="$(positive_field "$client" iterations)"
client_timed_exit="$(rendering_performance_field "$client" timed_exit)" ||
    fail "client completion lacks timed_exit"
[[ "$client_timed_exit" == true ]] || fail "xterm client did not complete its bounded window"

# CPU software-Present evidence: the patch-batch path must have been exercised,
# not whole-pixmap replacement every present.
efficiency="$(
    grep -E '^sophia_live_rendering_efficiency schema=1 status=complete ' "$SESSION_LOG" |
        tail -n 1 || true
)"
[[ -n "$efficiency" ]] || fail "missing rendering-efficiency evidence"
cpu_updates="$(positive_field "$efficiency" cpu_updates)"
cpu_patch_updates="$(positive_field "$efficiency" cpu_patch_updates)"
cpu_payload_bytes="$(positive_field "$efficiency" cpu_payload_bytes)"
cpu_patch_rects="$(positive_field "$efficiency" cpu_patch_rects)"
cpu_replacements="$(nonnegative_field "$efficiency" cpu_replacements)"
composition_target_reuses="$(nonnegative_field "$efficiency" composition_target_reuses)"
cpu_max_compose_msec="$(
    rendering_performance_field "$completion" cpu_max_compose_msec
)" || fail "completion lacks cpu_max_compose_msec"
[[ "$cpu_max_compose_msec" =~ ^[0-9]+$ ]] ||
    fail "cpu_max_compose_msec is not an integer"

# Damage-driven repaint proof: at least one partial repaint, not a full frame
# on every present.
partial_repaints="$(
    grep -Ec 'sophia_live_output_repaint schema=1 .* mode=partial ' "$SESSION_LOG" || true
)"
full_repaints="$(
    grep -Ec 'sophia_live_output_repaint schema=1 .* mode=full ' "$SESSION_LOG" || true
)"
((partial_repaints > 0)) ||
    fail "no damage-driven partial repaint; the CPU path repainted full frames"

# Optional presentation cadence, if the session summarized it.
cadence="$(
    grep -E '^sophia_live_present_cadence schema=1 status=complete ' "$SESSION_LOG" |
        tail -n 1 || true
)"
present_samples=0
present_fps=none
p95_frame_msec=none
if [[ -n "$cadence" ]]; then
    present_samples="$(rendering_performance_field "$cadence" samples)" ||
        fail "cadence summary lacks samples"
    present_fps="$(rendering_performance_field "$cadence" mean_fps)" ||
        fail "cadence summary lacks mean_fps"
    p95_frame_msec="$(rendering_performance_field "$cadence" p95_frame_msec)" ||
        fail "cadence summary lacks p95_frame_msec"
fi

native_retirements="$(positive_field "$completion" native_retirements)"

printf '%s\n' \
    "sophia_terminal_performance schema=1 status=pass workload=xterm-cpu duration_seconds=$duration_seconds surface_width=$surface_width surface_height=$surface_height client_lines=$client_lines client_iterations=$client_iterations native_retirements=$native_retirements cpu_updates=$cpu_updates cpu_replacements=$cpu_replacements cpu_patch_updates=$cpu_patch_updates cpu_patch_rects=$cpu_patch_rects cpu_payload_bytes=$cpu_payload_bytes cpu_max_compose_msec=$cpu_max_compose_msec composition_target_reuses=$composition_target_reuses partial_repaints=$partial_repaints full_repaints=$full_repaints present_samples=$present_samples present_fps=$present_fps p95_frame_msec=$p95_frame_msec"
