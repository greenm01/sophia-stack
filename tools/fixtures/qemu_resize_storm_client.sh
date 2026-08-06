#!/bin/sh
set -eu

iteration=0
while [ "$iteration" -lt 9000 ]; do
    printf 'sophia resize storm %04d 0123456789 abcdefghijklmnopqrstuvwxyz\n' "$iteration"
    iteration=$((iteration + 1))
    sleep 0.016
done
