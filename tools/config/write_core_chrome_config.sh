#!/usr/bin/env bash
set -euo pipefail

if (($# < 7 || $# > 8)); then
    echo "usage: $0 ABSOLUTE_PATH RING_ENABLED RING_WIDTH FRAME_ENABLED FRAME_WIDTH NAMESPACE VERBOSE [EXTRA_NODE]" >&2
    exit 2
fi

path=$1
ring_enabled=$2
ring_width=$3
frame_enabled=$4
frame_width=$5
namespace_profile=$6
verbose=$7
extra=${8:-}

[[ "$path" == /* ]] || {
    echo "core config destination must be absolute" >&2
    exit 2
}
[[ "$ring_enabled" =~ ^(true|false)$ && "$frame_enabled" =~ ^(true|false)$ &&
    "$verbose" =~ ^(true|false)$ ]] || {
    echo "enabled and verbose values must be true or false" >&2
    exit 2
}
[[ "$ring_width" =~ ^[0-9]+$ && "$frame_width" =~ ^[0-9]+$ ]] || {
    echo "chrome widths must be nonnegative integers" >&2
    exit 2
}
[[ "$namespace_profile" =~ ^(classic|classic-shared|confined)$ ]] || {
    echo "unsupported namespace profile: $namespace_profile" >&2
    exit 2
}

{
    printf '%s\n' \
        '/- kdl-version 2' \
        'schema 2' \
        'input {' \
        '    seat "seat0"' \
        '    keyboard rules="evdev" model="pc105" layout="us" variant="" options=""' \
        '    repeat delay-ms=660 interval-ms=25' \
        '}' \
        'compositor {' \
        '    chrome-fallback {' \
        "        focus-ring enabled=#$ring_enabled width=$ring_width color=\"#70b7ff\"" \
        "        frame enabled=#$frame_enabled width=$frame_width focused-color=\"#70b7ff\" unfocused-color=\"#303030\"" \
        '    }' \
        '    chrome-limits max-width=64' \
        '}' \
        "namespace profile=\"$namespace_profile\"" \
        "diagnostics verbose=#$verbose"
    [[ -z "$extra" ]] || printf '%s\n' "$extra"
} >"$path"
chmod 600 "$path"
