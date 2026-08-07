#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_DIR="${SOPHIA_XMOBAR_SOURCE:-$HOME/src/xmobar}"
BUILD_DIR="${SOPHIA_XMOBAR_BUILDDIR:-$ROOT_DIR/target/sophia-xmobar}"

command -v cabal >/dev/null 2>&1 || {
    echo "Building Sophia's packaged xmobar requires cabal-install." >&2
    exit 1
}
[[ -f "$SOURCE_DIR/xmobar.cabal" ]] || {
    echo "Xmobar source is missing from $SOURCE_DIR." >&2
    exit 1
}
[[ -z "$(git -C "$SOURCE_DIR" status --short)" ]] || {
    echo "Refusing to package a dirty xmobar source tree: $SOURCE_DIR" >&2
    exit 1
}

(
    cd "$SOURCE_DIR"
    cabal build --offline "--builddir=$BUILD_DIR" exe:xmobar >&2
    cabal list-bin --offline "--builddir=$BUILD_DIR" exe:xmobar
)
