# Sophia's human entry points.
#
# `just --list` is the surface someone runs by hand; the ~320 scripts under
# tools/ are what those entries call. Recipes here carry parameters and the
# guidance that belongs with them, and nothing else: a recipe body is shell,
# so moving logic into one buys nothing that a script does not already give.
#
# The typed work lives in `cargo xtask` -- session arguments and evidence
# verification -- because that is where the Rust/shell seams are.

_default:
    @just --list --unsorted

# One GPU client filling one head with nothing else drawn: the configuration in
# which a frame can be one opaque client DMA-BUF, which is what direct scanout
# requires.
#
# It runs no window manager. It cannot -- `sophia-wm-demo` lost its serving
# mode in 83596bfc with the experimental WM API v7 -- and it does not need one:
# a session without a WM honours the client's own geometry, so a client asked
# for the head's size fills it, and no WM means no focus ring and no border
# over the frame.
#
# The client is Kitty, because it is the one this stack is known to hand
# DMA-BUFs: every promoted Hagia archive carries hundreds of DMA-BUF frames
# from it, while vkcube on this machine presents through the software path --
# 389 Presents, every one a CPU layer.
#
# The input-latency harness cannot answer this question. It proves input
# reaches a terminal, so it needs an input proof, and the session refuses
# `--terminal-exec` alongside one; its client is always xterm, which draws
# through X core rendering and never presents a DMA-BUF at all.

# Prove a client's own buffer can reach the plane uncomposed. Run from tty3.
direct-scanout-probe width='2560' height='1440' hold='20' workload='kitty':
    #!/usr/bin/env bash
    set -euo pipefail
    # Positional, not named: `just direct-scanout-probe 1920 1080` is the form,
    # and `width=1920` would otherwise become the width itself. Checked rather
    # than documented, because a silently wrong size is a run that reports
    # layer_not_head_sized for a reason that has nothing to do with the row.
    for pair in 'width {{ width }}' 'height {{ height }}' 'hold {{ hold }}'; do
        name="${pair%% *}"
        value="${pair#* }"
        if [[ ! "$value" =~ ^[1-9][0-9]*$ ]]; then
            echo "$name must be a positive integer, got '$value'" >&2
            echo "arguments are positional: just direct-scanout-probe WIDTH HEIGHT HOLD WORKLOAD" >&2
            exit 2
        fi
    done
    case '{{ workload }}' in
        kitty | glxgears | vkcube | xterm) ;;
        *)
            echo "workload must be kitty, glxgears, vkcube, or xterm, got '{{ workload }}'" >&2
            exit 2
            ;;
    esac
    printf '%s\n' \
        'Direct scanout probe:' \
        "  Client: {{ workload }} at {{ width }}x{{ height }}," \
        "  holding {{ hold }}s, no window manager, no chrome." \
        '' \
        '  1. Confirm the client fills the screen edge to edge.' \
        '  2. Wait; it exits on its own and the session ends with it.' \
        '  3. There is no logout shortcut: no window manager means no' \
        '     shortcuts at all, because they are resolved against a policy' \
        "     client's configuration and none runs here. Ctrl+Alt+Backspace" \
        '     is the only way out early, and it is recorded as emergency' \
        '     recovery.' \
        '  4. Back at tty3, run: just direct-scanout-verify' \
        '' \
        'If the client does not fill the screen, its size does not match the' \
        'mode: pass width= and height= for the head it landed on. The' \
        'verifier reports that as layer_not_head_sized.' \
        '' \
        'A run in which nothing was eligible is a result, not a failure of' \
        'the run: the verifier prints which verdict every frame received.'
    export SOPHIA_TTY_PROFILE=standalone
    export SOPHIA_SESSION_VERBOSE_TRACE=true
    export SOPHIA_ENABLE_DIRECT_SCANOUT=1
    export SOPHIA_STANDALONE_WORKLOAD='{{ workload }}'
    export SOPHIA_STANDALONE_WIDTH='{{ width }}'
    export SOPHIA_STANDALONE_HEIGHT='{{ height }}'
    export SOPHIA_STANDALONE_HOLD_SECONDS='{{ hold }}'
    exec tools/start_sophia_tty3.sh

# Binds a signed identity, runs the probe, verifies and archives it. Run from
# tty3. The probe recipe above is the same session without the archive.

# Promote a direct-scanout run as immutable evidence. Run from tty3.
direct-scanout-gate:
    @tools/direct_scanout_gate.sh

# Re-verify an archived direct-scanout run, newest by default.
direct-scanout-archive run='':
    @tools/verify_direct_scanout_archive.sh {{ run }}

# Read what the last direct-scanout probe measured.
direct-scanout-verify log='':
    @tools/verify_direct_scanout_standalone.sh {{ log }}

# The check none of the four failed physical runs had: three died assembling a
# vector nothing validated until the display manager was already down.

# Build and validate every tool profile's session argument vector.
check-profiles:
    @cargo --quiet xtask check-profiles

# The source-layout audit is known-red: 35 files carry debt with no reviewed
# cohesion exception. Printing all of them on every run buries a new one, so
# this asserts the count and shows the difference when it moves.
layout_baseline := '35'

# Fail if the source-layout debt has grown past its recorded baseline.
check-layout:
    #!/usr/bin/env bash
    set -euo pipefail
    observed="$(tools/audit_source_layout.sh 2>&1 | grep -c '^error:' || true)"
    if (( observed > {{ layout_baseline }} )); then
        echo "source-layout debt grew from {{ layout_baseline }} to $observed:" >&2
        tools/audit_source_layout.sh 2>&1 | grep '^error:' >&2
        exit 1
    fi
    echo "source layout at its baseline: $observed of {{ layout_baseline }}"

# The offline gates, in the order a failure is cheapest to read.
check:
    cargo test --workspace --all-features
    cargo clippy --workspace --all-features --all-targets
    just check-profiles
    just check-layout
    tools/check_direct_scanout_verifier.sh
    tools/check_direct_scanout_archive_verifier.sh
    tools/check_sophia_standalone_vkcube_verifier.sh
    tools/check_hagia_native_matchers.sh
    tools/check_mirror_group_physical_verifier.sh
