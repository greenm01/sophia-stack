#!/usr/bin/env bash
set -euo pipefail

identity="${1:-}"
[[ -s "$identity" ]] || {
    echo "installed runtime identity is missing: ${identity:-unspecified}" >&2
    exit 1
}
require_line() {
    grep -Eq "$1" "$identity" || {
        echo "installed runtime identity is missing $2" >&2
        exit 1
    }
}
require_line '^sophia_runtime_identity schema=1 kind=system kernel=[^ ]+ mesa=[^ ]+$' \
    "kernel/Mesa identity"
for application in kitty firefox xmonad; do
    require_line "^sophia_runtime_identity schema=1 kind=application name=$application version=[^ ]+ digest=([0-9a-f]{64}|unavailable)$" \
        "$application identity"
done
require_line '^sophia_runtime_identity schema=1 kind=input seat=seat0 names_sha256=[0-9a-f]{64}$' \
    "input-seat identity"
connected="$(
    grep -Ec '^sophia_runtime_identity schema=1 kind=output connector=[^ ]+ status=connected edid_sha256=[0-9a-f]{64}$' \
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
