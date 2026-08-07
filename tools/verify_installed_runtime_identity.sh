#!/usr/bin/env bash
set -euo pipefail

identity="${1:-}"
expected_sophia_digest="${2:-}"
(( $# >= 1 && $# <= 2 )) && [[ -s "$identity" ]] || {
    echo "usage: tools/verify_installed_runtime_identity.sh IDENTITY [SOPHIA_SHA256]" >&2
    exit 1
}
if [[ -n "$expected_sophia_digest" \
    && ! "$expected_sophia_digest" =~ ^[0-9a-f]{64}$ ]]; then
    echo "expected Sophia digest is not a SHA-256 value" >&2
    exit 1
fi
require_line() {
    grep -Eq "$1" "$identity" || {
        echo "installed runtime identity is missing $2" >&2
        exit 1
    }
}
require_line '^sophia_runtime_identity schema=2 kind=system kernel=[^ ]+ mesa=[^ ]+$' \
    "kernel/Mesa identity"
mapfile -t sophia_lines < <(
    grep -E '^sophia_runtime_identity schema=2 kind=application name=sophia version=[^ ]+ digest=[0-9a-f]{64}$' \
        "$identity" || true
)
(( ${#sophia_lines[@]} == 1 )) || {
    echo "installed runtime identity requires one Sophia executable identity" >&2
    exit 1
}
observed_sophia_digest="${sophia_lines[0]##* digest=}"
if [[ -n "$expected_sophia_digest" \
    && "$observed_sophia_digest" != "$expected_sophia_digest" ]]; then
    echo "installed runtime identity has the wrong Sophia executable digest" >&2
    exit 1
fi
for application in kitty firefox xmonad xmobar; do
    require_line "^sophia_runtime_identity schema=2 kind=application name=$application version=[^ ]+ digest=([0-9a-f]{64}|unavailable)$" \
        "$application identity"
done
require_line '^sophia_runtime_identity schema=2 kind=input seat=seat0 names_sha256=[0-9a-f]{64}$' \
    "input-seat identity"
connected="$(
    grep -Ec '^sophia_runtime_identity schema=2 kind=output connector=[^ ]+ status=connected edid_sha256=[0-9a-f]{64}$' \
        "$identity" || true
)"
(( connected > 0 )) || {
    echo "installed runtime identity has no connected output" >&2
    exit 1
}
if grep -Eqi '(clipboard|window_title|typed_content|payload)=' "$identity"; then
    echo "installed runtime identity contains forbidden application content" >&2
    exit 1
fi

echo "installed runtime identity verified: connected_outputs=$connected"
