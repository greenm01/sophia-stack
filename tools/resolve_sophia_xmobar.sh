#!/usr/bin/env bash
set -euo pipefail

if [[ -n "${SOPHIA_XMOBAR_BIN:-}" ]]; then
    [[ -x "$SOPHIA_XMOBAR_BIN" ]] || {
        echo "SOPHIA_XMOBAR_BIN is not executable: $SOPHIA_XMOBAR_BIN" >&2
        exit 1
    }
    printf '%s\n' "$SOPHIA_XMOBAR_BIN"
    exit 0
fi

if xmobar_bin="$(command -v xmobar 2>/dev/null)" && [[ -x "$xmobar_bin" ]]; then
    printf '%s\n' "$xmobar_bin"
    exit 0
fi

xmobar_source="${SOPHIA_XMOBAR_SOURCE:-${HOME}/src/xmobar}"
xmobar_build_dir="${SOPHIA_XMOBAR_BUILDDIR:-/tmp/sophia-xmobar-build}"
if [[ -f "$xmobar_source/xmobar.cabal" ]] && command -v cabal >/dev/null 2>&1; then
    resolve_source_binary() {
        cd "$xmobar_source"
        cabal list-bin \
            --offline \
            "--builddir=$xmobar_build_dir" \
            exe:xmobar 2>/dev/null || true
    }
    xmobar_bin="$(resolve_source_binary)"
    if [[ -n "$xmobar_bin" && -x "$xmobar_bin" ]]; then
        printf '%s\n' "$xmobar_bin"
        exit 0
    fi
    echo "Building unmodified xmobar source offline in $xmobar_build_dir" >&2
    (
        cd "$xmobar_source"
        cabal build \
            --offline \
            "--builddir=$xmobar_build_dir" \
            exe:xmobar >&2
    )
    xmobar_bin="$(resolve_source_binary)"
    if [[ -n "$xmobar_bin" && -x "$xmobar_bin" ]]; then
        printf '%s\n' "$xmobar_bin"
        exit 0
    fi
fi

echo "xmobar not found; set SOPHIA_XMOBAR_BIN or provide a Cabal build." >&2
exit 1
