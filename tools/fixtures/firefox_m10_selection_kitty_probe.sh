#!/usr/bin/env bash
set -euo pipefail

probe_dir="${SOPHIA_FIREFOX_M10_KITTY_PROBE_DIR:-}"
if [[ -z "$probe_dir" || ! -d "$probe_dir" ]]; then
    echo 'Firefox selection probe directory is unavailable.' >&2
    exit 1
fi

set_redacted_title() {
    local length="$1" title
    printf -v title '%*s' "$length" ''
    title="${title// /0}"
    printf '\033]0;%s\007' "$title"
}

await_transfer() {
    local kind="$1" gesture="$2" expected="$3" reply
    while true; do
        printf '\n%s: %s, then press Enter: ' "$kind" "$gesture"
        IFS= read -r reply
        if [[ "$reply" == "$expected" ]]; then
            return 0
        fi
        echo 'The token was stale or incomplete; follow the current Firefox step and retry.'
    done
}

clear 2>/dev/null || true
echo 'Sophia focused Firefox selection proof'
echo 'Press Super+F, type sophia in Firefox, and follow its current selection step.'
set_redacted_title 241
: >"$probe_dir/checkpoint-selection-before"

await_transfer CLIPBOARD 'press Ctrl+Shift+V once' sophia
set_redacted_title 242
: >"$probe_dir/checkpoint-selection-clipboard"
printf '\nSelect and copy this exact return token with Ctrl+Shift+C:\n%s\n' sophia-kitty-clipboard
echo 'Return to Firefox, paste it, and physically select its PRIMARY token.'

await_transfer PRIMARY 'middle-click once' sophia-firefox-primary
set_redacted_title 243
: >"$probe_dir/checkpoint-selection-primary"
printf '\nSelect this exact return token with the pointer:\n%s\n' sophia-kitty-primary
echo 'Return to Firefox, middle-click its target, then use Super+Shift+Q.'

while IFS= read -r reply; do
    printf 'Selection proof remains interactive: %s\n' "$reply"
done
