#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_DIR="$ROOT_DIR/tools/config/sophia-xmonad"
BUILD_DIR="${SOPHIA_XMONAD_BUILDDIR:-$ROOT_DIR/target/sophia-xmonad}"

command -v cabal >/dev/null 2>&1 || {
    echo "Building Sophia's configured xmonad requires cabal-install." >&2
    exit 1
}

(
    cd "$SOURCE_DIR"
    cabal build --offline "--builddir=$BUILD_DIR" exe:sophia-xmonad >&2
    cabal list-bin --offline "--builddir=$BUILD_DIR" exe:sophia-xmonad
)
