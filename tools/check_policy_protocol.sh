#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
build_dir=$(mktemp -d)
trap 'rm -rf "$build_dir"' EXIT HUP INT TERM

cd "$root"
cargo run --offline -q -p sophia-policy-protocol-gen -- --check
cargo test --offline -q -p sophia-protocol --test policy_wire
${CC:-cc} -std=c99 -Wall -Wextra -Werror -pedantic \
    -Ibindings/c \
    bindings/c/sophia_wm_v1.c \
    bindings/c/tests/sophia_wm_v1_conformance.c \
    -o "$build_dir/sophia-wm-v1-conformance"
"$build_dir/sophia-wm-v1-conformance" \
    protocol/golden/sophia-wm-v1.frames \
    protocol/golden/sophia-wm-v1-malformed.frames \
    protocol/golden/sophia-wm-v1.records
${CC:-cc} -std=c99 -Wall -Wextra -Werror -pedantic \
    -Ibindings/c \
    bindings/c/sophia_wm_v1.c \
    bindings/c/tests/sophia_wm_v1_client.c \
    -o "$build_dir/sophia-wm-v1-client"
cargo run --offline -q -p sophia-runtime --example policy_c_conformance_host -- \
    "$build_dir/sophia-wm-v1-client" "$build_dir/session"
