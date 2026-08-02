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
    'clip=1276x1422_2_16' \
    'status=complete page_ready=true' \
    'status=clean protocol_errors=0' \
    'status=clean app_groups=0'; do
    grep -Fv "$pattern" "$FIXTURE" >"$TEMP_FILE"
    if "$ROOT_DIR/tools/verify_sophia_firefox_rendering_physical.sh" "$TEMP_FILE"; then
        echo "rendering canary verifier accepted missing evidence: $pattern" >&2
        exit 1
    fi
done

awk '/status=restarted/ { print; print; next } { print }' "$FIXTURE" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_rendering_physical.sh" "$TEMP_FILE"; then
    echo 'rendering canary verifier accepted repeated WM recovery' >&2
    exit 1
fi
echo 'Firefox rendering canary verifier fixtures passed'
