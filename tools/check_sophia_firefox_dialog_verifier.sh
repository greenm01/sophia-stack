#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE="$ROOT_DIR/tools/fixtures/physical_firefox_dialog_pass.log"
TEMP_FILE="$(mktemp)"
trap 'rm -f -- "$TEMP_FILE"' EXIT

"$ROOT_DIR/tools/verify_sophia_firefox_dialog_physical.sh" "$FIXTURE"
grep -Ev 'status=restarted|status=recovery_extent_(retained|cleared)|source=standing_target_recovery' \
    "$FIXTURE" >"$TEMP_FILE"
"$ROOT_DIR/tools/verify_sophia_firefox_dialog_physical.sh" "$TEMP_FILE"
awk '/status=started id=firefox source=action transaction=2/ {
    print "sophia_session_app schema=2 status=started id=terminal source=action transaction=1"
    print "sophia_session_app schema=2 status=surface_observed source=action transaction=1 surface=4194307"
} { print }' "$FIXTURE" >"$TEMP_FILE"
"$ROOT_DIR/tools/verify_sophia_firefox_dialog_physical.sh" "$TEMP_FILE"

for pattern in \
    'checkpoint=page_ready' \
    'transaction=1271 surface=6291459' \
    'status=pointer_batch routed=2 total=2' \
    'checkpoint=modal_ready' \
    'transaction=1281 surface=6291459' \
    'status=pointer_batch routed=2 total=4' \
    'checkpoint=confirmed' \
    'transaction=1294 surface=6291459' \
    'status=recovery_extent_cleared' \
    'source=standing_target_recovery' \
    'status=complete checkpoints=3' \
    'status=clean protocol_errors=0' \
    'status=clean recovery_extents=0' \
    'status=clean app_groups=0'; do
    grep -Fv "$pattern" "$FIXTURE" >"$TEMP_FILE"
    if "$ROOT_DIR/tools/verify_sophia_firefox_dialog_physical.sh" "$TEMP_FILE"; then
        echo "dialog canary verifier accepted missing evidence: $pattern" >&2
        exit 1
    fi
done

sed 's/transaction=1281 surface=6291459 source=1276x1422 target=1276x1422_2_16 clip=1276x1422_2_16/transaction=1281 surface=6291459 source=1276x1422 target=1276x1422_2_16 clip=1276x1040_2_16/' \
    "$FIXTURE" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_dialog_physical.sh" "$TEMP_FILE"; then
    echo 'dialog canary verifier accepted an incomplete modal frame' >&2
    exit 1
fi
awk '/checkpoint=modal_ready/ { print; print "sophia_live_wm schema=1 status=restarted restarts=2 preserved_layout=true"; next } { print }' \
    "$FIXTURE" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_dialog_physical.sh" "$TEMP_FILE"; then
    echo 'dialog canary verifier accepted a modal-era WM restart' >&2
    exit 1
fi
awk '/checkpoint=modal_ready/ { print; print "sophia_live_surface_admission schema=1 status=frontend_admitted transaction=9 surface=7340035"; next } { print }' \
    "$FIXTURE" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_dialog_physical.sh" "$TEMP_FILE"; then
    echo 'dialog canary verifier accepted an unexpected modal toplevel' >&2
    exit 1
fi
sed 's/pointer_buttons=4/pointer_buttons=2/' "$FIXTURE" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_dialog_physical.sh" "$TEMP_FILE"; then
    echo 'dialog canary verifier accepted one physical click' >&2
    exit 1
fi
awk '/checkpoint=modal_ready/ { print; print "Gdk-CRITICAL **: gdk_window_thaw_toplevel_updates: assertion failed"; next } { print }' \
    "$FIXTURE" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_dialog_physical.sh" "$TEMP_FILE"; then
    echo 'dialog canary verifier accepted a GDK freeze failure' >&2
    exit 1
fi
echo 'Firefox dialog canary verifier fixtures passed'
