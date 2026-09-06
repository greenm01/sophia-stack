# Quickshell X11 panel development check

This is an opt-in X11 compatibility client for CP-14.3. It exercises a 32-pixel
panel per output, a clock, a button, and an anchored local counter popout.
Quickshell speaks X11 here; it does not acquire `sophia_shell_v1` authority.
Narthex remains the native shell. DMS integration and native shell content are
later work; no Sophia wire protocol or Engine API depends on this toolkit.

X Authority translates docks, struts, popup ownership and X11 input. Engine
owns commits, composition, physical input and work-area reduction. Hagia
receives generic geometry and returns spatial policy. The fixture owns its
clock, counter and local widget interactions. The selected shell provides the
panel: Sophia no longer adds its old 14-pixel workspace bar. With this fixture
stopped and no other reservations, the full output work area is available.
Narthex's switcher and explicit tab descriptors remain independent.

## Launch in the current live Sophia session

From a terminal **inside the Sophia session**, run:

```sh
cd ~/dev/sophia-stack && cargo xtask panel --quickshell="$HOME/src/quickshell/build/sophia-baseline/src/quickshell"
```

The launcher inherits DISPLAY and XAUTHORITY, forces Qt's XCB platform, and
requests OpenGL by default. It prints a private evidence directory containing
the exact QML copy, binary fingerprints, version, requested environment and
`session.log`. Inspect Qt's backend/device lines there: requesting OpenGL does
not prove hardware acceleration, and llvmpipe/softpipe must not be accepted as
the GPU result. Use `--renderer=software` for an explicit comparison.
`SOPHIA_QUICKSHELL_BIN` can supply the binary instead of the argument.
`--output=/absolute/new/directory` chooses the evidence location.

This command attaches a development client; it does not install a profile,
replace Narthex, switch VTs or acquire DRM ownership. Ctrl+C stops it. Do not
run the interactive fixture alongside the standalone Quickshell trace smoke.

Perform this short check, then continue ordinary work:

1. Confirm a panel and ticking clock on both outputs. Check windows remain
   below it and the native shell still works.
2. Open the popout on each output, increment the counter and close it. Check
   anchoring, hit targets and that terminal keyboard focus remains usable.
3. Stop and relaunch the client. Check work-area restoration, no ghost popout,
   no stuck input and no native-shell interruption.

Retain the evidence directory and report which output/action failed. Physical
input, both-output behavior, hotplug/scaling and GPU presentation remain
pending until observed in this live check. There is no 36-row prerequisite.

## Optional login startup

The normal Hagia launcher honors `session { startup "terminal" "quickshell-panel"; }`
in the desktop profile. Register `quickshell-panel` separately in Sophia's core
configuration; executable paths and renderer environment belong to the trusted
session, not Hagia. The native shell remains enabled. See
[configuration ownership](configuration.md) for startup precedence.

For this development setup, the personal core registry invokes `/usr/bin/env`
with the XCB/OpenGL environment, the downstream Quickshell binary and
`~/.config/quickshell/sophia-panel/shell.qml`. The fixture copy has no automatic
exercise enabled. Qt diagnostics join the ordinary session log. Updating the
repository fixture does not replace this personal copy automatically.

## Isolated automated software probe

Build Sophia, then run the same launcher with explicit probe mode:

```sh
cargo build --offline -p sophia-cli --features native-session
cargo xtask panel --probe --renderer=software \
  --quickshell="$HOME/src/quickshell/build/sophia-baseline/src/quickshell" \
  --wm="$HOME/dev/hagia/hagia"
```

This uses real `sophia session run --session-mode=normal` startup, a deterministic
1280×720 output, Hagia, and an xterm witness. `/usr/bin/xterm`, `/usr/bin/firefox`
and `/bin/sh` must exist. Firefox is registered to satisfy Hagia's advertised
browser action but never started. The witness uses Sophia's established
`-cm -dc -fn 6x13` flags and exits after 12 seconds. The QML opens, updates and
closes its popout, hides/reopens the panel, then exits after 11 seconds. The
session has a 15-second outer deadline. A new display number is selected from
the launcher PID; `--display=:299` overrides it, and a collision is an error.

Configuration copies are private because Sophia rejects group-writable
profiles. `--no-input` prevents opening physical devices; it is rejected with
native scanout or input overrides. The isolated profile disables native shell
startup only for this test. Live mode leaves installed settings intact.

Probe success requires CPU surfaces with visual detail and advancing buffer
generations for the panel and popout, the expected popup anchor, a terminal
below the reservation, reservation release/reacquisition/final release, zero
protocol errors and clean session cleanup. A zero-transaction trace cannot
pass. The verifier is in `sophia-conformance`, separate from production:

```sh
cargo xtask conformance verify panel /path/to/evidence/session.log
```

`diagnostics verbose=#true` emits at most 60 CPU surface samples, one per second,
with at most 32 surfaces per sample. Truncated evidence is rejected. These
records describe committed content; they do not prove pointer input, physical
presentation or exact rendered pixels. Existing standalone Quickshell smokes
remain extension/trace observations, including their possible unmapped dock;
they have not been promoted to mapped-content proofs.

## Retained findings

`cargo xtask check` passes, including 2,430 Rust test executions, Clippy,
source-layout and decorated-record reader gates, archive/verifier checks and
host buffer-age pixel equivalence. The independent latter proof does not
promote this panel's CPU probe to GPU acceptance.

The accepted local run is `/tmp/sophia-panel-probe-v7`: changing panel and popup
content, a complete reservation cycle, 12/12 delivered controls, zero pending
controls, zero protocol errors and clean namespace teardown. Binary/QML hashes
are in its `identity.txt`; `/tmp` is development evidence, not a promoted archive.

Two Sophia lifecycle defects were repaired during this exercise: runtime input
and output setup read activated profile payloads after public-policy admission;
a resize proposal made stale by a work-area change now recovers and requests a
fresh projection instead of terminating the session.

A separate forced-deadline run (`/tmp/sophia-panel-probe-v5`) stopped while clients
were still connected and retained one unacknowledged session control. That
teardown race remains open. Normal client exit before the safety deadline
passes; it does not close the forced-deadline finding. Xterm without the usual
compatibility flags also exposed unsupported LookupColor (opcode 92), recorded
in `/tmp/sophia-panel-probe-v3`; this fixture does not expand the color protocol.
