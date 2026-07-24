#!/usr/bin/env bash
set -euo pipefail

output="${1:-}"
release_dir="${2:-}"
[[ -n "$output" && -d "$release_dir" ]] || {
    echo "usage: capture-runtime-identity.sh OUTPUT RELEASE_DIR" >&2
    exit 1
}

normalize() {
    tr '\n\t ' '___' | tr -cd '[:alnum:]_.:+/@=-'
}
version_of() {
    local executable="$1"
    if [[ -x "$executable" ]]; then
        "$executable" --version 2>&1 | head -n 1 | normalize
    else
        printf 'unavailable'
    fi
}
command_path() {
    command -v "$1" 2>/dev/null || true
}
digest_or_unavailable() {
    if [[ -r "$1" ]]; then
        sha256sum "$1" | awk '{print $1}'
    else
        printf 'unavailable'
    fi
}

kitty="$(command_path kitty)"
firefox="$(command_path firefox)"
mesa="unavailable"
if command -v xbps-query >/dev/null 2>&1; then
    mesa="$(xbps-query -p pkgver mesa 2>/dev/null | normalize || true)"
    [[ -n "$mesa" ]] || mesa=unavailable
fi
input_identity="$(
    sed -n 's/^N: Name=//p' /proc/bus/input/devices 2>/dev/null |
        sha256sum |
        awk '{print $1}'
)"
[[ -n "$input_identity" ]] || input_identity=unavailable

{
    printf 'sophia_runtime_identity schema=1 kind=system kernel=%s mesa=%s\n' \
        "$(uname -r | normalize)" "$mesa"
    printf 'sophia_runtime_identity schema=1 kind=application name=kitty version=%s digest=%s\n' \
        "$(version_of "$kitty")" "$(digest_or_unavailable "$kitty")"
    printf 'sophia_runtime_identity schema=1 kind=application name=firefox version=%s digest=%s\n' \
        "$(version_of "$firefox")" "$(digest_or_unavailable "$firefox")"
    printf 'sophia_runtime_identity schema=1 kind=application name=xmonad version=packaged digest=%s\n' \
        "$(digest_or_unavailable "$release_dir/target/release/xmonad")"
    printf 'sophia_runtime_identity schema=1 kind=input seat=seat0 names_sha256=%s\n' \
        "$input_identity"
    shopt -s nullglob
    for connector in /sys/class/drm/card*-*/status; do
        connector_name="$(basename "$(dirname "$connector")")"
        status="$(<"$connector")"
        edid="$(dirname "$connector")/edid"
        printf 'sophia_runtime_identity schema=1 kind=output connector=%s status=%s edid_sha256=%s\n' \
            "$connector_name" "$(printf '%s' "$status" | normalize)" \
            "$(digest_or_unavailable "$edid")"
    done
} >"$output"
chmod 600 "$output"
