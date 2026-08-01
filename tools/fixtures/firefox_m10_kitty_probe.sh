#!/usr/bin/env bash
set -euo pipefail

probe_dir="${SOPHIA_FIREFOX_M10_KITTY_PROBE_DIR:-}"
if [[ -z "$probe_dir" || ! -d "$probe_dir" ]]; then
    echo "Firefox M10 Kitty probe directory is unavailable." >&2
    exit 1
fi

if mkdir "$probe_dir/kitty-a" 2>/dev/null; then
    terminal=a
    tokens=(a1 a2 a3)
    title_bytes=(193 211 229)
elif mkdir "$probe_dir/kitty-b" 2>/dev/null; then
    terminal=b
    tokens=(b1 b2 b3)
    title_bytes=(194 212 230)
else
    echo "Firefox M10 uses exactly two Kitty probe windows." >&2
    exit 1
fi

set_redacted_title() {
    local length="$1" title
    printf -v title '%*s' "$length" ''
    title="${title// /0}"
    printf '\033]0;%s\007' "$title"
}

await_token() {
    local token="$1" reply
    while true; do
        printf '\nKitty %s checkpoint: type %s and press Enter: ' \
            "${terminal^^}" "${token^^}"
        IFS= read -r reply
        if [[ "${reply,,}" == "$token" ]]; then
            return 0
        fi
        echo "Expected ${token^^}; try again."
    done
}

clear
printf 'Sophia Milestone 10 — Kitty %s\n\n' "${terminal^^}"
echo 'This window keeps every checkpoint visible; do not close it.'
await_token "${tokens[0]}"
set_redacted_title "${title_bytes[0]}"
echo "Kitty ${terminal^^} is interactive before Firefox."
if [[ "$terminal" == a ]]; then
    echo 'Next: press Super+Enter. In the new Kitty, type B1.'
else
    echo 'Next: press Super+F and follow the instructions inside Firefox.'
fi

await_token "${tokens[1]}"
set_redacted_title "${title_bytes[1]}"
echo "Kitty ${terminal^^} retained content and input after normal Firefox close."
echo 'After both A2 and B2: press Super+F, wait for its page, then press Super+Shift+C.'

await_token "${tokens[2]}"
set_redacted_title "${title_bytes[2]}"
echo "Kitty ${terminal^^} retained content and input after forced Firefox close."
echo 'After both A3 and B3 are complete, press Super+Shift+Q to log out.'

while IFS= read -r reply; do
    printf 'Kitty %s remains interactive: %s\n' "${terminal^^}" "$reply"
done
