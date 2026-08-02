#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE="$ROOT_DIR/tools/fixtures/physical_firefox_lifecycle_pass.log"
TEMP_FILE="$(mktemp)"
trap 'rm -f -- "$TEMP_FILE"' EXIT

"$ROOT_DIR/tools/verify_sophia_firefox_lifecycle_physical.sh" "$FIXTURE"
for pattern in 'checkpoint=after_normal_close' 'action=CloseFocused' 'checkpoint=after_forced_close' 'status=complete page_ready=true'; do
    grep -Fv "$pattern" "$FIXTURE" >"$TEMP_FILE"
    if "$ROOT_DIR/tools/verify_sophia_firefox_lifecycle_physical.sh" "$TEMP_FILE"; then
        echo "focused lifecycle verifier accepted missing evidence: $pattern" >&2
        exit 1
    fi
done
echo 'focused Firefox lifecycle verifier fixtures passed'
