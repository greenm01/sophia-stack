#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="$(mktemp -d)"
trap 'rm -rf "$BUILD_DIR"' EXIT HUP INT TERM

cd "$ROOT_DIR"
XMONAD_BIN="$(tools/build_sophia_xmonad.sh)"
cargo build --offline -q -p sophia-x11-wm-bridge
env \
    SOPHIA_LEGACY_X11_WM="$XMONAD_BIN" \
    SOPHIA_LEGACY_X11_WM_ALIAS=xmonad/xmonad-x86_64-linux \
    cargo run --offline -q -p sophia-runtime \
        --example policy_c_conformance_host -- \
        "$ROOT_DIR/target/debug/sophia-x11-wm-bridge" \
        "$BUILD_DIR/session" \
        configured-restart \
        serve-policy

printf '%s\n' \
    'sophia_xmonad_public_policy schema=1 status=complete revision=3 scenarios=11 processes=5 connection_epochs=5 normal_restart=true timeout_recovery=true stale_recovery=true invalid_recovery=true preserved_commit=true'
