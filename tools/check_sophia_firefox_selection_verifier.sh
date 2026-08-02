#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE="$ROOT_DIR/tools/fixtures/physical_firefox_selection_pass.log"
TEMP_FILE="$(mktemp)"
trap 'rm -f -- "$TEMP_FILE"' EXIT

"$ROOT_DIR/tools/verify_sophia_firefox_selection_physical.sh" "$FIXTURE"
for pattern in 'checkpoint=clipboard_peer' 'kind=owner_change count=3' 'kind=conversion count=4' 'stage=primary'; do
    grep -Fv "$pattern" "$FIXTURE" >"$TEMP_FILE"
    if "$ROOT_DIR/tools/verify_sophia_firefox_selection_physical.sh" "$TEMP_FILE"; then
        echo "focused selection verifier accepted missing evidence: $pattern" >&2
        exit 1
    fi
done
echo 'focused Firefox selection verifier fixtures passed'
