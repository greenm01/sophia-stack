#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROBE="$ROOT_DIR/tools/fixtures/firefox_m10_kitty_probe.sh"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TEMP_DIR"' EXIT

printf 'a1\na2\na3\n' |
    env SOPHIA_FIREFOX_M10_KITTY_PROBE_DIR="$TEMP_DIR" "$PROBE" \
        >"$TEMP_DIR/first.out" &
first_pid=$!
for _ in $(seq 1 100); do
    [[ -d "$TEMP_DIR/kitty-a" ]] && break
    sleep 0.01
done
[[ -d "$TEMP_DIR/kitty-a" ]]
printf 'b1\nwrong\nsophia\nwrong\nsophia\nb2\nb3\n' |
    env SOPHIA_FIREFOX_M10_KITTY_PROBE_DIR="$TEMP_DIR" "$PROBE" \
        >"$TEMP_DIR/second.out" &
second_pid=$!
wait "$first_pid"
wait "$second_pid"

a_output="$(grep -l 'Kitty A checkpoint' "$TEMP_DIR"/*.out)"
b_output="$(grep -l 'Kitty B checkpoint' "$TEMP_DIR"/*.out)"
[[ -n "$a_output" && -n "$b_output" && "$a_output" != "$b_output" ]]
[[ "$(grep -hc 'Press Super+F' "$a_output")" == 0 ]]
[[ "$(grep -hc 'Press Super+F' "$b_output")" == 2 ]]
[[ "$(grep -hc 'Press Super+Shift+Q' "$a_output")" == 0 ]]
[[ "$(grep -hc 'Press Super+Shift+Q' "$b_output")" == 1 ]]
for checkpoint in 1 2 3; do
    [[ -e "$TEMP_DIR/checkpoint-a-$checkpoint" ]]
    [[ -e "$TEMP_DIR/checkpoint-b-$checkpoint" ]]
done
[[ -e "$TEMP_DIR/checkpoint-b-clipboard" ]]
[[ -e "$TEMP_DIR/checkpoint-b-primary" ]]
[[ "$(grep -Fc 'Expected SOPHIA; try again.' "$b_output")" == 2 ]]
[[ "$(grep -Fc 'received exactly' "$b_output")" == 2 ]]

echo 'Firefox M10 Kitty checkpoint coordinator passed'
