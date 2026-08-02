#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROBE="$ROOT_DIR/tools/fixtures/firefox_m10_selection_kitty_probe.sh"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TEMP_DIR"' EXIT

printf 'wrong\nsophia\nwrong\nsophia-firefox-primary\n' |
    env SOPHIA_FIREFOX_M10_KITTY_PROBE_DIR="$TEMP_DIR" "$PROBE" >"$TEMP_DIR/probe.out"

for checkpoint in before clipboard primary; do
    [[ -e "$TEMP_DIR/checkpoint-selection-$checkpoint" ]]
done
[[ "$(grep -Fc 'stale or incomplete' "$TEMP_DIR/probe.out")" == 2 ]]
grep -Fq 'sophia-kitty-clipboard' "$TEMP_DIR/probe.out"
grep -Fq 'sophia-kitty-primary' "$TEMP_DIR/probe.out"
echo 'focused Firefox selection Kitty coordinator passed'
