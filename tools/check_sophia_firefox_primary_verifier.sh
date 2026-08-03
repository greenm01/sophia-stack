#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE="$ROOT_DIR/tools/fixtures/physical_firefox_primary_pass.log"
TEMP_FILE="$(mktemp)"
trap 'rm -f -- "$TEMP_FILE"' EXIT

"$ROOT_DIR/tools/verify_sophia_firefox_primary_physical.sh" "$FIXTURE"
for pattern in \
    'checkpoint=page_ready' \
    'checkpoint=source_armed' \
    'kind=conversion count=1' \
    'kind=notify client=1' \
    'checkpoint=kitty_received' \
    'kind=owner_change count=2' \
    'kind=conversion count=2' \
    'kind=notify client=3' \
    'checkpoint=confirmed' \
    'status=complete checkpoints=4' \
    'status=clean protocol_errors=0' \
    'status=clean recovery_extents=0' \
    'status=clean app_groups=0'; do
    grep -Fv "$pattern" "$FIXTURE" >"$TEMP_FILE"
    if "$ROOT_DIR/tools/verify_sophia_firefox_primary_physical.sh" "$TEMP_FILE"; then
        echo "PRIMARY verifier accepted missing evidence: $pattern" >&2
        exit 1
    fi
done

awk '
    /kind=conversion count=1/ { next }
    /checkpoint=kitty_received/ {
        print
        print "sophia_firefox_m8 schema=1 status=selection_observed kind=conversion count=1 content=redacted"
        next
    }
    { print }
' "$FIXTURE" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_primary_physical.sh" "$TEMP_FILE"; then
    echo 'PRIMARY verifier accepted a conversion outside its checkpoint interval' >&2
    exit 1
fi
awk '/checkpoint=page_ready/ { print; print "sophia_firefox_selection schema=1 status=kitty_checkpoint checkpoint=clipboard_peer content=redacted"; next } { print }' \
    "$FIXTURE" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_sophia_firefox_primary_physical.sh" "$TEMP_FILE"; then
    echo 'PRIMARY verifier accepted replayed CLIPBOARD work' >&2
    exit 1
fi
echo 'Firefox PRIMARY verifier fixtures passed'
