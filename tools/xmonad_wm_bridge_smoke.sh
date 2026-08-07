#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
xmonad_bin=${SOPHIA_XMONAD_BIN:-}

if [ -z "$xmonad_bin" ]; then
    xmonad_bin=$("$repo_root/tools/build_sophia_xmonad.sh")
fi

exec cargo run \
    --offline \
    --quiet \
    --manifest-path "$repo_root/Cargo.toml" \
    --package sophia-x11-wm-bridge \
    -- xmonad-smoke \
    "--xmonad=$xmonad_bin"
