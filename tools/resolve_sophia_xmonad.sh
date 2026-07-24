#!/usr/bin/env bash
set -euo pipefail

if [[ -n "${SOPHIA_XMONAD_BIN:-}" ]]; then
    [[ -x "$SOPHIA_XMONAD_BIN" ]] || {
        echo "SOPHIA_XMONAD_BIN is not executable: $SOPHIA_XMONAD_BIN" >&2
        exit 1
    }
    printf '%s\n' "$SOPHIA_XMONAD_BIN"
    exit 0
fi

if command -v xmonad >/dev/null 2>&1; then
    command -v xmonad
    exit 0
fi

xmonad_source="${SOPHIA_XMONAD_SOURCE:-$HOME/src/xmonad}"
existing_cabal_bin="$(
    find "$xmonad_source/dist-newstyle" -type f -perm -111 \
        -path '*/x/xmonad/build/xmonad/xmonad' -print -quit 2>/dev/null || true
)"
if [[ -n "$existing_cabal_bin" ]]; then
    printf '%s\n' "$existing_cabal_bin"
    exit 0
fi
if [[ -f "$xmonad_source/xmonad.cabal" ]] && command -v cabal >/dev/null 2>&1; then
    if cabal_path="$(cd "$xmonad_source" && cabal list-bin xmonad 2>/dev/null)" &&
        [[ -x "$cabal_path" ]]; then
        printf '%s\n' "$cabal_path"
        exit 0
    fi
fi

xmonad_out="${SOPHIA_XMONAD_NIX_OUT:-/tmp/sophia-xmonad}"
if [[ -x "$xmonad_out/bin/xmonad" ]]; then
    printf '%s\n' "$xmonad_out/bin/xmonad"
    exit 0
fi
if [[ -f "$xmonad_source/flake.nix" ]] && command -v nix >/dev/null 2>&1; then
    nix build "$xmonad_source#defaultPackage.x86_64-linux" --out-link "$xmonad_out" >&2
    printf '%s\n' "$xmonad_out/bin/xmonad"
    exit 0
fi

echo "xmonad not found; set SOPHIA_XMONAD_BIN or provide a Cabal/Nix build." >&2
exit 1
