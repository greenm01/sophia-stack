#!/usr/bin/env bash
set -euo pipefail

printf '\033]0;Sophia TrueColor ANSI Proof\007'
printf '\033[2J\033[H\033[1;37mSophia Kitty 24-bit ANSI proof\033[0m\n\n'
printf '\033[48;2;255;0;0m    \033[0m red      '
printf '\033[48;2;0;255;0m      \033[0m green    '
printf '\033[48;2;0;0;255m        \033[0m blue\n'
printf '\033[48;2;255;255;0m          \033[0m yellow   '
printf '\033[48;2;0;255;255m            \033[0m cyan     '
printf '\033[48;2;255;0;255m              \033[0m magenta\n\n'
printf 'Exit the proof with Super+Shift+Q.\n'

# Keep the real Kitty client mapped until the WM ends the proof session.
while :; do
    sleep 3600
done
