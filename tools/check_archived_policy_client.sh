#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
archive="$root/protocol/archive/sophia-wm-v1-r3"
build_dir=$(mktemp -d)
trap 'rm -rf "$build_dir"' EXIT HUP INT TERM

cd "$archive"
sha256sum -c SHA256SUMS
${CC:-cc} -std=c99 -Wall -Wextra -Werror -pedantic \
    -I. \
    sophia_wm_v1.c \
    client.c \
    -o "$build_dir/sophia-wm-v1-r3-client"

cd "$root"
cargo run --offline -q -p sophia-runtime --example policy_c_conformance_host -- \
    "$build_dir/sophia-wm-v1-r3-client" "$build_dir/all" all
cargo run --offline -q -p sophia-runtime --example policy_c_conformance_host -- \
    "$build_dir/sophia-wm-v1-r3-client" "$build_dir/restart" restart

printf '%s\n' \
    'sophia_policy_archived_client schema=1 status=complete interface_major=1 revision=3 language=c99 scenarios=11 reconnect_restart=true preserved_commit=true immutable_digests=true'
