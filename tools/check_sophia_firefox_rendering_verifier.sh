#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE="$ROOT_DIR/tools/fixtures/physical_firefox_rendering_pass.log"
TEMP_FILE="$(mktemp)"
CPU_FIXTURE="$(mktemp)"
trap 'rm -f -- "$TEMP_FILE" "$CPU_FIXTURE"' EXIT

# Model the production launch emitter: schema 2 followed by its schema-1
# compatibility echo. Keep legacy-only and schema-2-only readers covered too.
launch_fixture() {
    awk -v mode="$1" '
        /status=started id=browser source=action/ {
            print "sophia_session_app schema=2 status=started id=browser source=action transaction=2"
            if (mode == "modern") next
        }
        { print }
    ' "${2:-$FIXTURE}"
}

expect_failure() {
    if "$ROOT_DIR/tools/verify_sophia_firefox_rendering_physical.sh" "$TEMP_FILE"; then
        echo "rendering verifier accepted $1" >&2
        exit 1
    fi
}

for mode in paired modern; do
    launch_fixture "$mode" >"$TEMP_FILE"
    "$ROOT_DIR/tools/verify_sophia_firefox_rendering_physical.sh" "$TEMP_FILE"
done

# CPU backing admission commits directly; it does not require a synthetic
# standing-target Present successor. Pixel/native-retirement proof is unchanged.
awk '
    /status=recovery_extent_cleared/ {
        sub(/reason=admission_present_retired/, "reason=cpu_admission_committed")
        print
        print "sophia_live_visual_admission schema=1 status=committed transaction=1200 surface=6291459 source=cpu_backing_snapshot"
        next
    }
    /source=standing_target_recovery/ || /status=visual_committed transaction=1201/ { next }
    { print }
' "$FIXTURE" >"$CPU_FIXTURE"
"$ROOT_DIR/tools/verify_sophia_firefox_rendering_physical.sh" "$CPU_FIXTURE"
launch_fixture paired "$CPU_FIXTURE" >"$TEMP_FILE"
"$ROOT_DIR/tools/verify_sophia_firefox_rendering_physical.sh" "$TEMP_FILE"
# Tracing from other owners can appear between the two launch records.
launch_fixture paired "$CPU_FIXTURE" | sed \
    -e '/schema=2 status=started/a unrelated renderer diagnostic' \
    -e 's/^/2026-09-04 INFO module: /' >"$TEMP_FILE"
"$ROOT_DIR/tools/verify_sophia_firefox_rendering_physical.sh" "$TEMP_FILE"

for mutation in \
    '/sophia_live_visual_admission/d' \
    '/sophia_live_visual_admission/s/surface=6291459/surface=999/' \
    '/sophia_live_visual_admission/s/transaction=1200/transaction=0/' \
    's/source=cpu_backing_snapshot/source=unknown/' \
    's/reason=cpu_admission_committed/reason=unknown/' \
    '/sophia_native_composition_region_frame/d' \
    '/sophia_live_native_head_page_flip/d' \
    '/status=recovery_extent_cleared/p'; do
    sed -e "$mutation" "$CPU_FIXTURE" >"$TEMP_FILE"
    expect_failure "invalid CPU admission: $mutation"
done
# Admission before the clear cannot settle a later recovery obligation.
awk '/sophia_live_visual_admission/ { admission = $0; next }
    /status=recovery_extent_cleared/ { clear = $0; next }
    /sophia_live_head_content_geometry/ && clear { print admission; print clear; clear = "" }
    { print }' "$CPU_FIXTURE" >"$TEMP_FILE"
expect_failure 'CPU admission preceding its recovery clear'

for mutation in \
    '/schema=2 status=started/s/transaction=2/transaction=3/' \
    '/schema=2 status=started/s/transaction=2/transaction=0/' \
    '/schema=2 status=started/s/schema=2/schema=99/' \
    '/schema=2 status=started/p' \
    '/schema=2 status=started/ { p; s/transaction=2/transaction=3/; }' \
    '/schema=1 status=started id=browser/p'; do
    launch_fixture paired | sed -e "$mutation" >"$TEMP_FILE"
    expect_failure "invalid or repeated launch: $mutation"
done

"$ROOT_DIR/tools/verify_sophia_firefox_rendering_physical.sh" "$FIXTURE"
grep -Ev 'status=recovery_extent_(retained|cleared)|source=standing_target_recovery' \
    "$FIXTURE" >"$TEMP_FILE"
"$ROOT_DIR/tools/verify_sophia_firefox_rendering_physical.sh" "$TEMP_FILE"
for pattern in \
    'status=surface_observed source=action' \
    'status=page_ready title_bytes=249' \
    'status=recovery_extent_cleared' \
    'source=standing_target_recovery' \
    'status=visual_committed transaction=1201' \
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

# The same opaque frame IDs in different native owners cannot complete an old
# geometry/readback join. A boundary between two complete joins stays valid.
owner_close='sophia_live_native_owner schema=1 status=closed epoch=1 reason=seat_release settled=true settlement_failures=0'
awk -v boundary="$owner_close" '/scene_generation=901 surface=/ { print boundary } { print }' "$FIXTURE" >"$TEMP_FILE"
"$ROOT_DIR/tools/verify_sophia_firefox_rendering_physical.sh" "$TEMP_FILE"
awk -v boundary="$owner_close" '/status=retired output=1 head=1 submission=25/ { print boundary } { print }' "$FIXTURE" >"$TEMP_FILE"
expect_failure 'a native retirement joined across owner epochs'
