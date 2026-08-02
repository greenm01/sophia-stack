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

mark_checkpoint() {
    : >"$probe_dir/checkpoint-${terminal}-$1"
}

await_both_checkpoints() {
    local checkpoint="$1"
    mark_checkpoint "$checkpoint"
    echo "Waiting for the matching checkpoint in the other Kitty..."
    while [[ ! -e "$probe_dir/checkpoint-a-$checkpoint"
        || ! -e "$probe_dir/checkpoint-b-$checkpoint" ]]; do
        sleep 0.1
    done
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

await_selection_transfer() {
    local kind="$1" gesture="$2" reply
    while true; do
        printf '\nKitty B %s transfer: %s, then press Enter: ' "$kind" "$gesture"
        IFS= read -r reply
        if [[ "$reply" == sophia ]]; then
            return 0
        fi
        echo 'Expected SOPHIA; try again.'
    done
}

clear 2>/dev/null || true
printf 'Sophia Milestone 10 — Kitty %s\n\n' "${terminal^^}"
echo 'This window keeps every checkpoint visible; do not close it.'
await_token "${tokens[0]}"
set_redacted_title "${title_bytes[0]}"
mark_checkpoint 1
echo "Kitty ${terminal^^} is interactive before Firefox."
if [[ "$terminal" == a ]]; then
    echo 'Next: press Super+Enter. In the new Kitty, type B1.'
else
    await_both_checkpoints 1
    echo 'Both Kitty windows are ready. Press Super+F and follow the instructions inside Firefox.'
    await_selection_transfer CLIPBOARD 'press Ctrl+Shift+V once'
    set_redacted_title 202
    mark_checkpoint clipboard
    printf '\nCLIPBOARD received exactly. Select and copy this exact line with Ctrl+Shift+C:\n%s\n' sophia
    echo 'Cycle back to Firefox and paste it there. Return when Firefox shows its PRIMARY step.'
    await_selection_transfer PRIMARY 'middle-click once'
    set_redacted_title 203
    mark_checkpoint primary
    printf '\nPRIMARY received exactly. Select this exact line with the pointer:\n%s\n' sophia
    echo 'Cycle back to Firefox and middle-click its full-page PRIMARY target.'
fi

echo "Do not type ${tokens[1]^^} until Firefox completes and Ctrl+Q closes it."
await_token "${tokens[1]}"
set_redacted_title "${title_bytes[1]}"
echo "Kitty ${terminal^^} retained content and input after normal Firefox close."
await_both_checkpoints 2
if [[ "$terminal" == b ]]; then
    echo 'Both Kitty windows passed A2/B2. Press Super+F, wait for its page, then press Super+Shift+C.'
else
    echo 'Both Kitty windows passed A2/B2. Continue from Kitty B; do not launch Firefox here.'
fi

echo "Do not type ${tokens[2]^^} until the restarted Firefox window has closed."
await_token "${tokens[2]}"
set_redacted_title "${title_bytes[2]}"
echo "Kitty ${terminal^^} retained content and input after forced Firefox close."
await_both_checkpoints 3
if [[ "$terminal" == b ]]; then
    echo 'Both Kitty windows passed A3/B3. Press Super+Shift+Q to log out.'
else
    echo 'Both Kitty windows passed A3/B3. Continue from Kitty B; do not log out here.'
fi

while IFS= read -r reply; do
    printf 'Kitty %s remains interactive: %s\n' "${terminal^^}" "$reply"
done
