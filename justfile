# Sophia's human entry points.
#
# `just --list` is the optional, memorable surface someone runs by hand.
# Recipes contain no workflow logic: they delegate to `cargo xtask`, which is
# also the canonical CI interface. Installed sessions invoke Sophia directly;
# neither production nor repository scripts depend on `just`.

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
    @cargo --quiet xtask conformance run direct-scanout "{{ width }}" "{{ height }}" "{{ hold }}" "{{ workload }}"

# Binds a signed identity, runs the probe, verifies and archives it. Run from
# tty3. The probe recipe above is the same session without the archive.

# Promote a direct-scanout run as immutable evidence. Run from tty3.
direct-scanout-gate:
    @cargo --quiet xtask conformance gate direct-scanout

# Re-verify an archived direct-scanout run, newest by default.
direct-scanout-archive run='':
    @cargo --quiet xtask conformance verify direct-scanout-archive "{{ run }}"

# Read what the last direct-scanout probe measured.
direct-scanout-verify log='':
    @cargo --quiet xtask conformance verify direct-scanout-standalone "{{ log }}"

# The check none of the four failed physical runs had: three died assembling a
# vector nothing validated until the display manager was already down.

# Build and validate every tool profile's session argument vector.
check-profiles:
    @cargo --quiet xtask profile check

# Fail if the exact source-layout debt differs from its reviewed ledger.
check-layout:
    @cargo --quiet xtask check layout

# Run the canonical offline, non-hardware repository gate.
check:
    @cargo --quiet xtask check
