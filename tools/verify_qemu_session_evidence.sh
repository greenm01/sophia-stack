#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_FILE="${1:-${SOPHIA_QEMU_EVIDENCE:-/tmp/sophia-qemu-session.log}}"

"$ROOT_DIR/tools/verify_live_session_persistent_evidence.sh" "$EVIDENCE_FILE"

if [[ "$(grep -c '^sophia_qemu_session schema=3 status=starting isolation=headless display_sink=vnc-unix control=qmp-unix host_drm=none host_vt=none guest_network=none storage=none gpu=virtio-gpu gpu_devices=2 gpu_heads=2 keyboard=virtio mouse=virtio ticks=300$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing the isolated start marker" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_qemu_guest schema=1 status=complete ticks=300$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing the 300-tick guest completion marker" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_qemu_session schema=3 status=complete qemu_exit=0$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing the clean host completion marker" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_qemu_topology schema=1 status=observed requested_heads=2 connectors=2 connected=2$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing two connected virtual outputs" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_live_outputs schema=2 status=ready discovered=2 presentation=2 native_owned=2 multi_output_scanout=enabled layout=extended_horizontal$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing complete two-output native ownership" >&2
    exit 1
fi
mapfile -t output_lines < <(grep '^sophia_live_output schema=1 status=complete ' "$EVIDENCE_FILE" || true)
if [[ "${#output_lines[@]}" -ne 2 ]]; then
    echo "QEMU evidence must contain exactly two per-output completion records" >&2
    exit 1
fi
# This gate's whole claim is two independent logical outputs, and the record is
# emitted once per head rather than once per output. A mirror group would
# therefore produce two records that both name output 1 -- structurally two
# records, but one output. Counting distinct identities is what separates the
# two cases; identical content no longer can, because two independent outputs
# may legitimately present the same pixels.
declare -A output_ids=()
total_retirements=0
total_callbacks=0
for output_line in "${output_lines[@]}"; do
    output_id="$(sed -n 's/.* output=\([0-9][0-9]*\).*/\1/p' <<< "$output_line")"
    if [[ -z "$output_id" ]]; then
        echo "QEMU output evidence has no numeric output identity: $output_line" >&2
        exit 1
    fi
    output_ids[$output_id]=1
    for field in submissions nonzero_exports; do
        value="$(sed -n "s/.* ${field}=\([0-9][0-9]*\).*/\1/p" <<< "$output_line")"
        if [[ -z "$value" ]] || (( value == 0 )); then
            echo "QEMU output evidence has no $field: $output_line" >&2
            exit 1
        fi
    done
    retirements="$(sed -n 's/.* retirements=\([0-9][0-9]*\).*/\1/p' <<< "$output_line")"
    callbacks="$(sed -n 's/.* callbacks=\([0-9][0-9]*\).*/\1/p' <<< "$output_line")"
    if [[ -z "$retirements" || -z "$callbacks" ]]; then
        echo "QEMU output evidence is missing retirement counters: $output_line" >&2
        exit 1
    fi
    if [[ "${SOPHIA_QEMU_REQUIRE_TWO_XTERM:-0}" == "1" ]] \
        && (( retirements == 0 || callbacks == 0 )); then
        echo "QEMU two-xterm output evidence has no asynchronous retirement: $output_line" >&2
        exit 1
    fi
    total_retirements=$((total_retirements + retirements))
    total_callbacks=$((total_callbacks + callbacks))
    checksum="$(sed -n 's/.* checksum=\([0-9][0-9]*\) .*/\1/p' <<< "$output_line")"
    if [[ -z "$checksum" ]]; then
        echo "QEMU output evidence is missing its logical-content checksum" >&2
        exit 1
    fi
done
if (( ${#output_ids[@]} != 2 )); then
    echo "QEMU evidence names ${#output_ids[@]} distinct outputs; two required" >&2
    exit 1
fi
if (( total_retirements == 0 || total_callbacks == 0 )); then
    echo "QEMU evidence has no asynchronous page-flip retirement" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_live_vsync schema=1 status=complete outputs=2 overlap_rejections=0 phase_rejections=0 policy=page_flip_paced$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing the per-output fixed-refresh vsync gate" >&2
    exit 1
fi
if grep -q '^sophia_qemu_.* status=failed' "$EVIDENCE_FILE"; then
    echo "QEMU evidence contains a failure marker" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_live_session_input schema=1 status=ready source=physical text=sophia$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing physical-input readiness" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_qemu_input schema=1 status=sent source=qmp device=virtio-keyboard text=sophia events=14$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing QMP keyboard delivery" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_live_session_input schema=2 status=complete source=physical text=sophia expected_events=14 matched_events=14 pixel_change=true$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing the exact physical text completion record" >&2
    exit 1
fi
latency_line="$(grep '^sophia_live_input_latency schema=1 status=complete source=libinput_to_kernel_page_flip ' "$EVIDENCE_FILE" || true)"
if [[ "$(wc -l <<< "$latency_line")" -ne 1 ]]; then
    echo "QEMU evidence is missing the libinput-to-kernel-page-flip latency record" >&2
    exit 1
fi
for field in queue_dwell_msec dwell_to_submit_msec submit_to_page_flip_msec full_chain_msec; do
    value="$(sed -n "s/.* ${field}=\([0-9][0-9]*\).*/\1/p" <<< "$latency_line")"
    if [[ -z "$value" ]]; then
        echo "QEMU latency evidence is missing numeric $field" >&2
        exit 1
    fi
    printf -v "$field" '%s' "$value"
done
stage_total_msec=$((queue_dwell_msec + dwell_to_submit_msec + submit_to_page_flip_msec))
if (( stage_total_msec > full_chain_msec \
    || full_chain_msec - stage_total_msec > 2 \
    || full_chain_msec > 100 )); then
    echo "QEMU latency evidence has inconsistent stages or exceeds 100 ms" >&2
    exit 1
fi
clock_line="$(grep '^sophia_live_page_flip_clock schema=1 status=complete source=kernel_monotonic ' "$EVIDENCE_FILE" || true)"
if [[ "$(wc -l <<< "$clock_line")" -ne 1 ]] \
    || ! [[ "$clock_line" =~ timestamps=([1-9][0-9]*)\ fallbacks=0\ pending=0$ ]]; then
    echo "QEMU evidence lacks complete kernel page-flip timestamp coverage" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_live_session_pointer schema=1 status=ready source=physical action=select$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing physical-pointer readiness" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_qemu_pointer schema=1 status=sent source=qmp device=virtio-mouse action=select commands=5$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing QMP pointer delivery" >&2
    exit 1
fi

completion_line="$(grep -E '^sophia_live_session .*status=bounded_complete ' "$EVIDENCE_FILE")"
if [[ ! " $completion_line " =~ " schema=10 " ]] \
    && [[ ! " $completion_line " =~ " schema=11 " ]] \
    && [[ ! " $completion_line " =~ " schema=14 " ]] \
    && [[ ! " $completion_line " =~ " schema=15 " ]] \
    && [[ ! " $completion_line " =~ " schema=16 " ]]; then
    echo "QEMU evidence did not use the latency/resource schema" >&2
    exit 1
fi
if [[ ! " $completion_line " =~ " session_ticks=300 " ]]; then
    echo "QEMU evidence did not complete exactly 300 session ticks" >&2
    exit 1
fi
if [[ ! " $completion_line " =~ " physical_input=enabled " ]]; then
    echo "QEMU evidence did not open the virtual input path" >&2
    exit 1
fi
if [[ ! " $completion_line " =~ " injected_input=false " ]]; then
    echo "QEMU evidence used the internal X11 injection path" >&2
    exit 1
fi
if [[ "${SOPHIA_QEMU_REQUIRE_TWO_XTERM:-0}" == "1" ]]; then
    cpu_layers="$(sed -n 's/.* cpu_layers=\([0-9][0-9]*\) .*/\1/p' <<< "$completion_line")"
    if [[ ! "$cpu_layers" =~ ^[0-9]+$ ]] || (( cpu_layers < 2 )); then
        echo "QEMU two-xterm evidence did not compose two terminal layers" >&2
        exit 1
    fi
    if ! grep -Eq '^sophia_live_session schema=7 status=running .* secondary_terminal=enabled ' "$EVIDENCE_FILE"; then
        echo "QEMU two-xterm evidence did not enable the secondary terminal" >&2
        exit 1
    fi
fi
physical_keys="$(sed -n 's/.* physical_keys_routed=\([0-9][0-9]*\) .*/\1/p' <<< "$completion_line")"
if [[ -z "$physical_keys" ]] || (( physical_keys != 14 )); then
    echo "QEMU evidence did not route exactly 14 virtio keyboard events" >&2
    exit 1
fi
if [[ ! " $completion_line " =~ " pointer_proof=enabled " ]] || [[ ! " $completion_line " =~ " pointer_pixel_change=true " ]]; then
    echo "QEMU evidence did not prove visible pointer input" >&2
    exit 1
fi
physical_pointer="$(sed -n 's/.* physical_pointer_routed=\([0-9][0-9]*\) .*/\1/p' <<< "$completion_line")"
if [[ -z "$physical_pointer" ]] || (( physical_pointer == 0 )); then
    echo "QEMU evidence has no routed virtio mouse events" >&2
    exit 1
fi

for field in input_presented_latency_msec cpu_max_compose_msec \
    native_max_submit_to_page_flip_msec native_max_upload_msec \
    native_target_creations native_target_recreations native_pipeline_creations \
    native_frame_uploads; do
    value="$(sed -n "s/.* ${field}=\([0-9][0-9]*\).*/\1/p" <<< "$completion_line")"
    if [[ -z "$value" ]]; then
        echo "QEMU evidence is missing numeric $field" >&2
        exit 1
    fi
    printf -v "$field" '%s' "$value"
done
native_frame_surface_creations=0
if [[ " $completion_line " =~ " schema=15 " ]] \
    || [[ " $completion_line " =~ " schema=16 " ]]; then
    native_frame_surface_creations="$(
        sed -n 's/.* native_frame_surface_creations=\([0-9][0-9]*\).*/\1/p' \
            <<<"$completion_line"
    )"
    if [[ -z "$native_frame_surface_creations" ]]; then
        echo "QEMU evidence is missing numeric native_frame_surface_creations" >&2
        exit 1
    fi
fi
if (( input_presented_latency_msec > 100 || cpu_max_compose_msec > 25 \
    || native_max_submit_to_page_flip_msec > 100 || native_max_upload_msec > 100 )); then
    echo "QEMU evidence exceeded its input/rendering latency budget" >&2
    exit 1
fi
if (( native_target_recreations != 0 || native_frame_uploads < 2 )); then
    echo "QEMU evidence did not preserve bounded native upload resources" >&2
    exit 1
fi
if ! (( (native_target_creations == 0 && native_pipeline_creations == 0) \
    || (native_target_creations == 2 && native_pipeline_creations == 2) )); then
    echo "QEMU evidence has inconsistent direct-write/persistent-GL resource counters" >&2
    exit 1
fi
if { [[ " $completion_line " =~ " schema=15 " ]] \
    || [[ " $completion_line " =~ " schema=16 " ]]; } \
    && (( native_frame_surface_creations < native_target_creations )); then
    echo "QEMU evidence has fewer frame surfaces than renderer epochs" >&2
    exit 1
fi

echo "QEMU dual-output native presentation/QMP-input 300-tick evidence passed: $EVIDENCE_FILE"
