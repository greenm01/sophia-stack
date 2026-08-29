#!/usr/bin/env bash
set -euo pipefail

# One GPU client filling one head, with nothing else drawn: the configuration
# in which a frame can be one opaque client DMA-BUF, which is what direct
# scanout requires.
#
# It differs from the vkcube isolation proof in two ways, both eligibility
# requirements rather than taste: a columns layout, so the single client covers
# the output exactly instead of sitting centred at its natural size with a
# background painted around it; and no focus ring, because a focus ring lowers
# to a Border command and any command that paints means the composed image is
# not the client's buffer. See `tools/fixtures/direct_scanout_sophia_wm.kdl`.
#
# The input-latency harness cannot answer this question: it proves input
# reaches a terminal, so it needs an input proof, and the session refuses
# `--terminal-exec` alongside one -- its client is always xterm, which draws
# through X core rendering and never presents a DMA-BUF at all.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=standalone
export SOPHIA_SESSION_VERBOSE_TRACE=true
export SOPHIA_ENABLE_DIRECT_SCANOUT=1
: "${SOPHIA_STANDALONE_WORKLOAD:=vkcube}"
export SOPHIA_STANDALONE_WORKLOAD

printf '%s\n' \
    'Direct scanout probe:' \
    "  Client: $SOPHIA_STANDALONE_WORKLOAD, filling the output, no chrome." \
    '  1. Confirm the client fills the screen edge to edge.' \
    '  2. Leave it running for a few seconds so frames accumulate.' \
    '  3. Press Super+Shift+Q for normal logout.' \
    '  4. Press Ctrl+Alt+Backspace only for emergency recovery.' \
    '  5. Back at tty3, run:' \
    '     tools/verify_direct_scanout_standalone.sh' \
    '' \
    'A run in which nothing was eligible is a result, not a failure of the' \
    'run: the verifier prints which verdict every frame received.'

exec "$ROOT_DIR/tools/start_sophia_tty3.sh" "$@"
