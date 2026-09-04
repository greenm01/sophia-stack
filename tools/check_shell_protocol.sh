#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
narthex_root=${SOPHIA_NARTHEX_ROOT:-"$(dirname -- "$root")/narthex"}
build_dir=$(mktemp -d)
trap 'rm -rf "$build_dir"' EXIT HUP INT TERM

cd "$root"
cargo run --offline -q -p sophia-protocol --example shell_v1_corpus -- --valid \
    >"$build_dir/sophia-shell-v1.frames"
cargo run --offline -q -p sophia-protocol --example shell_v1_corpus -- --malformed \
    >"$build_dir/sophia-shell-v1-malformed.frames"
cargo run --offline -q -p sophia-protocol --example shell_tab_corpus >"$build_dir/sophia-shell-tabs.frames"
cmp "$build_dir/sophia-shell-tabs.frames" protocol/golden/sophia-shell-tabs.frames
cmp "$build_dir/sophia-shell-v1.frames" protocol/golden/sophia-shell-v1.frames
cmp "$build_dir/sophia-shell-v1-malformed.frames" \
    protocol/golden/sophia-shell-v1-malformed.frames
cargo test --offline -q -p sophia-protocol --test shell_wire
cargo test --offline -q -p sophia-protocol --test shell_tabs
cargo test --offline -q -p sophia-runtime --test shell_transport

${CC:-cc} -std=c99 -Wall -Wextra -Werror -pedantic \
    bindings/c/tests/sophia_shell_v1_client.c \
    -o "$build_dir/sophia-shell-v1-c-client"
cargo run --offline -q -p sophia-runtime \
    --example shell_descriptor_conformance_host -- \
    "$build_dir/sophia-shell-v1-c-client"

if [ ! -f "$narthex_root/src/narthex.nim" ]; then
    echo "Narthex checkout not found at $narthex_root" >&2
    exit 2
fi
cd "$narthex_root"
SOPHIA_STACK_ROOT="$root" nim c -r --hints:off --path:src \
    --nimcache:"$build_dir/nimcache-test" \
    -o:"$build_dir/tshell-v1" tests/tshell_v1.nim
SOPHIA_STACK_ROOT="$root" nim c -r --hints:off --path:src --nimcache:"$build_dir/nimcache-tabs" -o:"$build_dir/tshell-tabs" tests/tshell_tabs.nim
nim c --hints:off --path:src --nimcache:"$build_dir/nimcache-client" \
    -o:"$build_dir/narthex" src/narthex.nim
cd "$root"
cargo run --offline -q -p sophia-runtime \
    --example shell_descriptor_conformance_host -- "$build_dir/narthex"
cargo run --offline -q -p sophia-runtime \
    --example shell_descriptor_conformance_host -- "$build_dir/narthex" --serve
# The reservation half: the real Nim shell claims a bottom strip, Engine's
# coordinator admits it, and the work area shrinks only once the bundle
# commits. Driving it here keeps the claim honest offline, where a wrong band
# costs seconds instead of a rig session.
cargo run --offline -q -p sophia-runtime \
    --example shell_descriptor_conformance_host -- "$build_dir/narthex" --bar-proof

printf '%s\n' \
    'sophia_shell_behavior_corpus schema=1 status=complete clients=rust,c,nim protected=true live_serve=true descriptors=2 activations=1 withdrawn=true reservations=1'
