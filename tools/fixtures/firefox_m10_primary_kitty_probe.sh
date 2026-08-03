#!/usr/bin/env bash
set -euo pipefail

probe_dir="${SOPHIA_FIREFOX_M10_KITTY_PROBE_DIR:-}"
if [[ -z "$probe_dir" || ! -d "$probe_dir" ]]; then
    echo 'Firefox PRIMARY probe directory is unavailable.' >&2
    exit 1
fi

set_redacted_title() {
    local length="$1" title
    printf -v title '%*s' "$length" ''
    title="${title// /0}"
    printf '\033]0;%s\007' "$title"
}

while true; do
    clear 2>/dev/null || true
    cat <<'INSTRUCTIONS'
Sophia Firefox PRIMARY-only proof

1. In Firefox, physically drag-select its entire token.
2. Return here and middle-click once.
INSTRUCTIONS
    printf 'PRIMARY token: '
    IFS= read -r reply
    if [[ "$reply" == sophia-firefox-primary ]]; then
        break
    fi
    echo 'The token was stale or incomplete; repeat the current Firefox step.'
done

set_redacted_title 253
: >"$probe_dir/checkpoint-primary-received"
cat <<'INSTRUCTIONS'

Firefox-to-Kitty PRIMARY passed.
Physically drag-select this entire return token:
sophia-kitty-primary

Cycle to Firefox and middle-click its full-page target.
When Firefox reports complete, press Super+Shift+Q.
INSTRUCTIONS

while IFS= read -r reply; do
    printf 'PRIMARY proof remains interactive: %s\n' "$reply"
done
