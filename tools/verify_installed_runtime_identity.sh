#!/usr/bin/env bash
set -euo pipefail

identity="${1:-}"
expected_sophia_digest="${2:-}"
auxiliary_name="${3:-}"
expected_auxiliary_digest="${4:-}"
(( $# == 1 || $# == 2 || $# == 4 )) && [[ -s "$identity" ]] || {
    echo "usage: tools/verify_installed_runtime_identity.sh IDENTITY [SOPHIA_SHA256 [AUXILIARY_NAME AUXILIARY_SHA256]]" >&2
    exit 1
}
if [[ -n "$expected_sophia_digest" \
    && ! "$expected_sophia_digest" =~ ^[0-9a-f]{64}$ ]]; then
    echo "expected Sophia digest is not a SHA-256 value" >&2
    exit 1
fi
if (( $# == 4 )); then
    [[ "$auxiliary_name" =~ ^[a-z][a-z0-9_-]*$ \
        && "$expected_auxiliary_digest" =~ ^[0-9a-f]{64}$ ]] || {
        echo "expected auxiliary identity is malformed" >&2
        exit 1
    }
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
for application in kitty firefox xterm sophia-wm-demo hagia narthex; do
    require_line "^sophia_runtime_identity schema=2 kind=application name=$application version=[^ ]+ digest=([0-9a-f]{64}|unavailable)$" \
        "$application identity"
done
if (( $# == 4 )); then
    mapfile -t auxiliary_lines < <(
        grep -E "^sophia_runtime_identity schema=2 kind=application name=$auxiliary_name version=[^ ]+ digest=[0-9a-f]{64}$" \
            "$identity" || true
    )
    (( ${#auxiliary_lines[@]} == 1 )) || {
        echo "installed runtime identity requires one $auxiliary_name executable identity" >&2
        exit 1
    }
    observed_auxiliary_digest="${auxiliary_lines[0]##* digest=}"
    [[ "$observed_auxiliary_digest" == "$expected_auxiliary_digest" ]] || {
        echo "installed runtime identity has the wrong $auxiliary_name executable digest" >&2
        exit 1
    }
fi
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
