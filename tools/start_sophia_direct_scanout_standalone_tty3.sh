#!/usr/bin/env bash
set -euo pipefail

# One GPU client filling one head, with nothing else drawn: the configuration
# in which a frame can be one opaque client DMA-BUF, which is what direct
# scanout requires.
#
# It runs no window manager. It cannot -- `sophia-wm-demo` lost its serving
# mode in 83596bfc with the experimental WM API v7, and every session naming it
# as `--wm-process` now dies at startup with a usage string -- and it does not
# need one: a session without a WM honours the client's own geometry, so a
# client asked for the head's size fills it, and no WM means no focus ring and
# no border over the frame.
#
# The client is Kitty, because it is the one this stack is known to hand
# DMA-BUFs: every promoted Hagia archive carries hundreds of DMA-BUF frames
# from it, while vkcube on this machine presents through the software path --
# 389 Presents, every one a CPU layer, and direct scanout needs a client
# buffer.
#
# It is bounded and sized, so the run needs no operator beyond starting it: the
# shell inside it exits and `--exit-when-startup-exits` ends the session with
# it. That is the only ordinary exit. A session without a window manager has no
# shortcuts at all -- they are resolved against a policy client's configuration
# (`wm/public_policy.rs:2136-2145`) and there is no policy client here -- so
# Ctrl+Alt+Delete does nothing and Ctrl+Alt+Backspace is the only way out
# early.
#
# The input-latency harness cannot answer this question: it proves input
# reaches a terminal, so it needs an input proof, and the session refuses
# `--terminal-exec` alongside one -- its client is always xterm, which draws
# through X core rendering and never presents a DMA-BUF at all.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=standalone
export SOPHIA_SESSION_VERBOSE_TRACE=true
export SOPHIA_ENABLE_DIRECT_SCANOUT=1
: "${SOPHIA_STANDALONE_WORKLOAD:=kitty}"
: "${SOPHIA_STANDALONE_WIDTH:=2560}"
: "${SOPHIA_STANDALONE_HEIGHT:=1440}"
: "${SOPHIA_STANDALONE_HOLD_SECONDS:=20}"
export SOPHIA_STANDALONE_WORKLOAD SOPHIA_STANDALONE_WIDTH \
    SOPHIA_STANDALONE_HEIGHT SOPHIA_STANDALONE_HOLD_SECONDS

printf '%s\n' \
    'Direct scanout probe:' \
    "  Client: $SOPHIA_STANDALONE_WORKLOAD at ${SOPHIA_STANDALONE_WIDTH}x${SOPHIA_STANDALONE_HEIGHT}," \
    "  holding ${SOPHIA_STANDALONE_HOLD_SECONDS}s, no window manager, no chrome." \
    '' \
    '  1. Confirm the client fills the screen edge to edge.' \
    '  2. Wait; it exits on its own and the session ends with it.' \
    '  3. There is no logout shortcut: no window manager means no shortcuts.' \
    '     Ctrl+Alt+Backspace is the only way out early, and it is recorded' \
    '     as emergency recovery.' \
    '  4. Back at tty3, run:' \
    '     tools/verify_direct_scanout_standalone.sh' \
    '' \
    'If the client does not fill the screen, its size does not match the mode:' \
    'set SOPHIA_STANDALONE_WIDTH and SOPHIA_STANDALONE_HEIGHT to the head it' \
    'landed on. The verifier reports that as layer_not_head_sized.' \
    '' \
    'A run in which nothing was eligible is a result, not a failure of the' \
    'run: the verifier prints which verdict every frame received.'

exec "$ROOT_DIR/tools/start_sophia_tty3.sh" "$@"
