#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE="$ROOT_DIR/tools/fixtures/physical_firefox_rendering_pass.log"
TEMP_FILE="$(mktemp)"
trap 'rm -f -- "$TEMP_FILE"' EXIT

"$ROOT_DIR/tools/verify_sophia_firefox_rendering_physical.sh" "$FIXTURE"
grep -Ev 'status=recovery_extent_(retained|cleared)|source=standing_target_recovery' \
    "$FIXTURE" >"$TEMP_FILE"
"$ROOT_DIR/tools/verify_sophia_firefox_rendering_physical.sh" "$TEMP_FILE"
for pattern in \
    'status=surface_observed source=action' \
    'status=page_ready title_bytes=249' \
    'status=recovery_extent_cleared' \
    'source=standing_target_recovery' \
    'sophia_native_composition_region_frame' \
    'sophia_live_head_content_geometry' \
    'sophia_live_head_composition_queue' \
    'sophia_live_native_head_page_flip' \
    'status=complete page_ready=true' \
    'status=clean protocol_errors=0' \
    'status=clean app_groups=0'; do
    grep -Fv "$pattern" "$FIXTURE" >"$TEMP_FILE"
    if "$ROOT_DIR/tools/verify_sophia_firefox_rendering_physical.sh" "$TEMP_FILE"; then
        echo "rendering canary verifier accepted missing evidence: $pattern" >&2
        exit 1
    fi
done

# Both current storage paths are valid; metadata, stale pixels, other surfaces
# and other frames must never substitute for the browser's scanout evidence.
for mutation in \
    's/nonzero_rgb_pixels=1700000/nonzero_rgb_pixels=0/g' \
    's/nonzero_rgb_pixels=1700000/nonzero_rgb_pixels=16/g' \
    's/checksum=987654321098765432/checksum=123456789012345678/g' \
    '/sophia_live_head_content_geometry/s/surface=6291459/surface=999/g' \
    '/sophia_native_composition_region_frame/s/scene_generation=90/scene_generation=80/g' \
    '/sophia_native_composition_region_frame/s/head=1/head=2/g' \
    '/sophia_live_native_head_page_flip/s/frame=4/frame=5/g' \
    '/sophia_native_composition_region_frame/s/1285_23/2_16/g' \
    '/sophia_native_composition_region_frame/s/region_pixels=1782528/region_pixels=1/g' \
    's/clip=1266x1408_1285_23/clip=1266x100_1285_23/g'; do
    sed -e "$mutation" "$FIXTURE" >"$TEMP_FILE"
    if "$ROOT_DIR/tools/verify_sophia_firefox_rendering_physical.sh" "$TEMP_FILE"; then
        echo "rendering verifier accepted invalid pixels/identity: $mutation" >&2
        exit 1
    fi
done
# Geometry and tracing prefixes are observed data, not fixed fixture strings.
sed -e 's/1266x1408/800x600/g' -e 's/1782528/480000/g' -e 's/1700000/400000/g' \
    -e 's/1285_23/50_30/g' -e 's/^/2026-09-04 INFO module: /' "$FIXTURE" >"$TEMP_FILE"
"$ROOT_DIR/tools/verify_sophia_firefox_rendering_physical.sh" "$TEMP_FILE"

awk '/status=restarted/ { print; print; next } { print }' "$FIXTURE" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_rendering_physical.sh" "$TEMP_FILE"; then
    echo 'rendering canary verifier accepted repeated WM recovery' >&2
    exit 1
fi
# Visible pixels and eventual cleanup cannot turn a failed normal drain into
# a pass. This models the EOF-stranded final batch from the physical canary.
awk '{ print } END {
    print "sophia_live_session_quiescence schema=2 status=timed_out reason=logout_complete elapsed_msec=2000 authority_pending=1 cpu_pending=0 native_pending=false coordinator_pending=0 oldest_authority_transaction=42"
}' "$FIXTURE" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_rendering_physical.sh" "$TEMP_FILE"; then
    echo 'rendering canary verifier accepted a shutdown timeout' >&2
    exit 1
fi
echo 'Firefox rendering canary verifier fixtures passed'
