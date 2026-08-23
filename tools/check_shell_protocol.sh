#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
hagia_root=${SOPHIA_HAGIA_ROOT:-"$(dirname -- "$root")/hagia"}
build_dir=$(mktemp -d)
trap 'rm -rf "$build_dir"' EXIT HUP INT TERM

cd "$root"
cargo run --offline -q -p sophia-protocol --example shell_v1_corpus -- --valid \
    >"$build_dir/sophia-shell-v1.frames"
cargo run --offline -q -p sophia-protocol --example shell_v1_corpus -- --malformed \
    >"$build_dir/sophia-shell-v1-malformed.frames"
cmp "$build_dir/sophia-shell-v1.frames" protocol/golden/sophia-shell-v1.frames
cmp "$build_dir/sophia-shell-v1-malformed.frames" \
    protocol/golden/sophia-shell-v1-malformed.frames
cargo test --offline -q -p sophia-protocol --test shell_wire
cargo test --offline -q -p sophia-runtime --test shell_transport

${CC:-cc} -std=c99 -Wall -Wextra -Werror -pedantic \
    bindings/c/tests/sophia_shell_v1_client.c \
    -o "$build_dir/sophia-shell-v1-c-client"
cargo run --offline -q -p sophia-runtime \
    --example shell_descriptor_conformance_host -- \
    "$build_dir/sophia-shell-v1-c-client"

if [ ! -f "$hagia_root/src/hagia_shell.nim" ]; then
    echo "Hagia checkout not found at $hagia_root" >&2
    exit 2
fi
cd "$hagia_root"
SOPHIA_STACK_ROOT="$root" nim c -r --hints:off --path:src \
    --nimcache:"$build_dir/nimcache-test" \
    -o:"$build_dir/tsophia-shell-v1" tests/tsophia_shell_v1.nim
nim c --hints:off --path:src --nimcache:"$build_dir/nimcache-client" \
    -o:"$build_dir/hagia-shell" src/hagia_shell.nim
cd "$root"
cargo run --offline -q -p sophia-runtime \
    --example shell_descriptor_conformance_host -- "$build_dir/hagia-shell"

printf '%s\n' \
    'sophia_shell_behavior_corpus schema=1 status=complete clients=rust,c,nim protected=true descriptors=2 activations=1 withdrawn=true'
