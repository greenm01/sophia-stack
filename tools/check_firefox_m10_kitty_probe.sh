#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROBE="$ROOT_DIR/tools/fixtures/firefox_m10_kitty_probe.sh"
TEMP_DIR="$(mktemp -d)"
LIFECYCLE_TEMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TEMP_DIR" "$LIFECYCLE_TEMP_DIR"' EXIT

printf 'a1\na2\na3\n' |
    env SOPHIA_FIREFOX_M10_KITTY_PROBE_DIR="$TEMP_DIR" "$PROBE" \
        >"$TEMP_DIR/first.out" &
first_pid=$!
for _ in $(seq 1 100); do
    [[ -d "$TEMP_DIR/kitty-a" ]] && break
    sleep 0.01
done
[[ -d "$TEMP_DIR/kitty-a" ]]
printf 'b1\nwrong\nsophia\nwrong\nsophia-firefox-primary\nb2\nb3\n' |
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

printf 'a1\na2\na3\n' |
    env SOPHIA_FIREFOX_M10_KITTY_PROBE_DIR="$LIFECYCLE_TEMP_DIR" \
        SOPHIA_FIREFOX_M10_PROOF_SLICE=lifecycle "$PROBE" \
        >"$LIFECYCLE_TEMP_DIR/first.out" &
lifecycle_first_pid=$!
for _ in $(seq 1 100); do
    [[ -d "$LIFECYCLE_TEMP_DIR/kitty-a" ]] && break
    sleep 0.01
done
printf 'b1\nb2\nb3\n' |
    env SOPHIA_FIREFOX_M10_KITTY_PROBE_DIR="$LIFECYCLE_TEMP_DIR" \
        SOPHIA_FIREFOX_M10_PROOF_SLICE=lifecycle "$PROBE" \
        >"$LIFECYCLE_TEMP_DIR/second.out" &
lifecycle_second_pid=$!
wait "$lifecycle_first_pid"
wait "$lifecycle_second_pid"
[[ "$(grep -hc 'transfer:' "$LIFECYCLE_TEMP_DIR"/*.out | awk '{ sum += $1 } END { print sum + 0 }')" == 0 ]]
grep -Fq 'Lifecycle slice' "$LIFECYCLE_TEMP_DIR/second.out"

echo 'Firefox M10 Kitty checkpoint coordinator passed'
