# Validation

**Role:** reproducible validation catalog.

The active product is native X11 with namespace admission, portals, external
WM policy, and Engine-owned CPU/DMA-BUF presentation. Retired compatibility
frontends are preserved under `research/` and are not validation gates.

Sophia's default validation path must not require native renderer libraries,
kernel devices, a display server, or network access. The default suite protects
the data model, protocol authorities, runtime reducers, renderer admission
records, and deterministic backend seams.
Default physical input validation uses `QueuedInputPoller`. Native libinput
coverage is feature-gated and opt-in; ordinary workspace validation must prove
physical input intake with deterministic queued packets and must not open
`/dev/input` devices.

Run before committing ordinary changes:

```sh
cargo fmt --check
tools/audit_source_layout.sh
cargo test --workspace --offline
```

For Sophia X Authority compatibility changes, also run the focused wire suite
and the real-client smoke that exercises the touched path. The
[X11 compatibility matrix](x11-compatibility-matrix.md) identifies each
probe's precise proven surface and next gate; do not treat this list as a full
X server conformance suite:

```sh
cargo test --offline -q -p sophia-protocol
cargo test --offline -q -p sophia-portal
cargo test --offline -q -p sophia-x-authority --test x11_wire
cargo test --offline -q -p sophia-x-authority --test x11_wire x_server_frontend_routes_selection_notify_to_the_requestor_client -- --exact
cargo test --offline -q -p sophia-x-authority --test x11_wire cross_namespace_executor_installs_property_and_notifies_requestor -- --exact
cargo test --offline -q -p sophia-portal --test socket
cargo run --offline -q -p sophia-cli -- x-authority-xclock-smoke
cargo run --offline -q -p sophia-cli -- x-authority-xeyes-smoke
cargo run --offline -q -p sophia-cli -- x-authority-xwininfo-root-smoke
cargo run --offline -q -p sophia-cli -- x-authority-xprop-root-smoke
cargo run --offline -q -p sophia-cli -- x-authority-xsetroot-name-smoke
cargo run --offline -q -p sophia-cli -- x-authority-xlogo-smoke
cargo run --offline -q -p sophia-cli -- x-authority-xmessage-smoke
cargo run --offline -q -p sophia-cli -- x-authority-xrandr-query-smoke
cargo run --offline -q -p sophia-cli -- x-authority-xcalc-smoke
cargo run --offline -q -p sophia-cli -- x-authority-xterm-smoke
cargo run --offline -q -p sophia-cli -- x-authority-xterm-render-smoke
cargo run --offline -q -p sophia-cli -- x-authority-xterm-input-smoke
cargo run --offline -q -p sophia-cli -- x-authority-xterm-two-client-smoke
cargo run --offline -q -p sophia-cli -- x-authority-kitty-input-smoke
dbus-run-session -- cargo run --offline -q -p sophia-cli -- x-authority-zenity-smoke
```

The Kitty input smoke is a strict promotion gate. It launches unconfigured
Kitty, waits for two DRI3/Present submissions, verifies the client-visible XKB
mapping, focuses the mapped surface, routes `ll` plus Return, and requires both
the exact shell result and a later Present. A failure is actionable evidence;
do not replace it with a wire-write-only assertion.

Milestone 5 uses one unattended local QEMU acceptance command. It boots a
diskless, networkless Linux guest that owns virtio DRM/KMS, guest console state,
libinput keyboard and pointer devices, and the Sophia session:

```sh
tools/qemu_milestone5_acceptance.sh
```

The runner rebuilds the initramfs, runs the strict two-xterm native-session
regression, exercises Ctrl-Alt-Backspace emergency recovery, then runs classic
shared-X and confined GTK3 Zenity profiles. The GTK profiles require exact
virtio `sophia` text, a routed pointer button before Return is accepted,
CPU/SHM pixels changed by a committed resize, native presentation on both
guest outputs, normal application exit, zero protocol errors, and clean
retirement. Evidence is retained below `.evidence/qemu-milestone5/`.

`tools/live_session_milestone5_gtk_hardware_proof.sh` remains an optional
compatibility diagnostic for any machine where direct DRM/KMS, VT, and physical
input behavior needs investigation. It is not a milestone gate. Its independent
input guard, bounded process-group termination, and KD/termios restoration remain
fail-closed safeguards for direct hardware use.

The fixture-backed GTK and TTY recovery verifier check remains available through
`tools/check_live_session_milestone5_verifier.sh`.

The real-client smokes are regression smokes, not full X server conformance
tests. Their reduced output must keep `first_error=none`, report the
proof-window outcome explicitly, and include request/opcode counters so future
client-driven regressions show which compatibility surface changed. The
external probe harness fails if it observes any client-visible X protocol
error, even after a drawing client has already produced authority transactions.
External probe binaries are resolved from `PATH`; set
`SOPHIA_XAUTHORITY_<LABEL>` to override a probe binary path for a local host.
`x-authority-xterm-smoke` is a setup/lifecycle regression, not a rendered
transaction proof; its reduced output is expected to report zero committed
runtime transactions. `x-authority-xterm-render-smoke` is the separate drawing
transaction and materialized CPU-pixel proof. The guarded session tools are the
separate Engine/KMS evidence.
`x-authority-zenity-smoke` is a GTK software-rendering regression. Prefer
running it under `dbus-run-session --` on TTY hosts so GTK reaches its DBus
startup path. It requires a committed surface, a copied nonzero `MIT-SHM`
buffer, and `first_error=none`. Pixel requirements are declarative probe policy;
the frontend does not branch on client names.
Parse-error details include a bounded request head so extension decode failures
show the concrete minor opcode that drove the next compatibility slice.

For live composition changes that connect X Authority transaction intake to
backend-live rendered scanout reporting, run the commands below. These validate
the backend-owned production runtime as well as the Engine, renderer, and
backend boundaries:

```sh
cargo test --offline -q -p sophia-backend-live --features libdrm-events live_session_composition
cargo run --offline -q -p sophia-cli --features atomic-scanout-live -- live-session-composition-smoke
cargo run --offline -q -p sophia-cli --features atomic-scanout-live -- sophia-live-session --proof --terminal=xterm
cargo run --offline -q -p sophia-cli --features atomic-scanout-live -- sophia-live-session --display=:177 --max-runtime-ms=6000 --inject-text=sophia
# Operator TTY proof: add --input-devices=/dev/input/by-path/...-event-kbd,
# type into xterm, and require physical_keys_routed>0 plus changed pixels.
tools/live_session_content_hardware_proof.sh
tools/live_session_persistent_hardware_proof.sh
tools/live_session_two_xterm_hardware_proof.sh
tools/live_session_milestone4_hardware_proof.sh
tools/operator_keyboard_hardware_proof.sh
tools/vrr_hardware_proof.sh
tools/build_qemu_session_initramfs.sh
tools/qemu_session_harness.sh
tools/run_sophia_input_latency_qemu.sh
SOPHIA_QEMU_SCENARIO=emergency-recovery tools/qemu_session_harness.sh
SOPHIA_QEMU_SCENARIO=gtk-classic tools/qemu_session_harness.sh
SOPHIA_QEMU_SCENARIO=gtk-confined tools/qemu_session_harness.sh
tools/audit_no_xlibre_runtime.sh
tools/audit_xcentric_runtime.sh
```

The Milestone 4 proof must pass both the software verifier and the strict
schema-14 GPU verifier. A DMA-BUF-only mixed-export diagnostic does not satisfy
the gate: the retained GPU run must include the CPU background layer, positive
acquire waiting, rejection recovery, Flip/Idle and idle-fence activity, and
zero live resources. On an AMDGPU command-stream rejection, capture
`sudo dmesg -T` immediately before another graphical session obscures the
kernel validator record.

Before repeating the full paired proof, run
`tools/native_egl_vkcube_mixed_smoke.sh` from the same dedicated text TTY. It
uses the real native-X `vkcube` DRI3 handoff, executes the CPU-plus-DMA-BUF
native EGL export in a watchdog child, stops before KMS submit, and verifies a
single `sophia_native_egl_mixed schema=1` line with positive CPU and DMA-BUF
layers plus zero live presentation resources. The full proof preauthorizes
`sudo` and retains `kernel-before.log`, `kernel-after.log`, and environment
identity beside the software and GPU session logs.

After changing deferred admission or production transaction intake, run the
real-client ordering preflight on a host with an openable DRM render node:

```sh
cargo run --offline -q -p sophia-cli -- x-authority-vkcube-admission-smoke
```

It keeps policy-managed mapping deferred, delivers only the generic
`AdmitSurface` control, and requires continued DRI3 import plus two exact
Present Complete/Idle round trips. This is a transport/admission regression;
it does not replace visible native KMS proof.

Before involving Kitty, xmonad, xmobar, or the X11 WM compatibility bridge, run
the visible single-client isolation proof from tty3:

```sh
tools/start_sophia_vkcube_standalone_tty3.sh
```

Sophia launches default `vkcube --wsi xcb` directly. The external reference WM
uses its generic `natural` layout policy: it receives only the opaque node,
centers the node's natural allocation, and requests no policy resize. The
ordinary policy-managed deferred admission, X11 Present, renderer, and KMS
paths remain active. The launcher installs its checked-in KDL2 policy template
into the owner-only runtime directory as mode `0600`, so config safety does not
depend on repository checkout permissions. After visually confirming the
spinning cube, use Super-Shift-Q for normal logout and run:

```sh
tools/verify_sophia_standalone_vkcube.sh
```

The verifier requires exactly one presented-frame admission candidate, its
exact visual-admission completion, nonzero scanout pixels, normal logout, zero
protocol/resource debt, and clean teardown. A DMA-BUF candidate must have its
exact page-flip retirement. A software candidate must advance through at least
three authority transactions and produce positive Present Complete, Idle, and
idle-fence-trigger evidence, so a visible but frozen first frame cannot pass.
Presented-frame evidence may use imported DMA-BUF storage or an immutable CPU
snapshot materialized from a software/MIT-SHM Present request. An unresolved
X pixmap, unrelated backing snapshot, blank bordered window, process-only
success, or emergency exit cannot pass. The launcher knows that the validation
client is `vkcube`; Engine, the X authority, and the natural layout reducer
contain no application identity branches.

The staged CPU-buffer regression deliberately separates the first immutable
buffer update from the later released transaction. It must retain the
renderer-private buffer while Engine has no committed surface, then compose
visual detail and route Complete-before-Idle after release. It also separates
an update-only replacement from a later patch, proves late unrooted patches
are disposable, and requires post-reduction committed surfaces to retain a
materialized renderer buffer:

```sh
cargo test --offline -q -p sophia-backend-live --all-features \
  --test software_present_feedback
```

Startup failure evidence reports staged and resident CPU-buffer counts,
resident bytes, missing committed buffers, and software Present submissions.
`layout_pending` identifies blocked convergence; `cpu_buffer_missing`
identifies a broken renderer residency root. Do not increase the startup
timeout to make either failure disappear.

To measure the software-Present path after correctness passes, run the bounded
benchmark from the same dedicated tty:

```sh
tools/benchmark_sophia_vkcube_tty3.sh
```

It runs an explicit 500-by-500, 900-frame, FIFO `vkcube` workload, exits with
the startup application, and reports `sophia_rendering_performance schema=2`.
The report derives FPS and p95 frame
cadence from routed Present Flip timestamps rather than process wall time. It
also joins the session's CPU replacement/patch counts, patch rectangles and
payload bytes, exact-versus-damage-scoped metric counts, native composition
target reuse, maximum CPU composition time, maximum native upload time, and
retirement count. The raw report can be regenerated without another graphical
run:

```sh
tools/report_sophia_rendering_performance.sh
```

The terminal CPU-path workload is a separate bounded standalone proof:

```sh
tools/check_bounded_xterm_geometry.sh
tools/check_sophia_terminal_performance_reporter.sh
tools/run_sophia_terminal_gate_tty3.sh
```

Before the physical command, require `sudo sv status socklog-unix nanoklogd`
to report both services running and confirm
`/var/log/socklog/kernel/current` is nonempty. The commit-pinned runner enforces
those checks, refuses a dirty worktree, and archives the session, launcher,
guard, recovery, performance report, and kernel-log delta under
`$XDG_STATE_HOME/sophia/rendering-benchmarks/<commit>/terminal-cpu/`. Run it
from a logged-in local TTY3, arm Ctrl-Alt-Backspace when prompted, and confirm
the centered xterm scrolls continuously. The default 20-second, 500-by-500
pixel intent resolves against the pinned `6x13` font rather than being passed
to xterm as character cells. It emits eight lines every 16 ms, avoiding an
unbounded producer burst while keeping the software-Present path continuously
active. An inner process-external timer bounds the producer even when terminal
backpressure blocks a write, and its incremental count preserves completed
bursts before xterm's process-level safety timeout. The independent 30-second
watchdog bounds the complete session. Let the xterm exit automatically; the
logout shortcut intentionally produces an incomplete benchmark.

The trailing `sophia_terminal_performance schema=3` report requires positive
immutable CPU patch traffic, damage-driven partial repaint, no unexpected X11
or native failure, clean resource drain, and
`cpu_max_compose_msec <= cpu_compose_budget_msec`. The default composition
budget is 25 ms; `SOPHIA_TERMINAL_COMPOSE_BUDGET_MSEC` accepts only a positive
integer and is reserved for a separately documented gate. The report also
requires the client to reproduce the declared line-batch and interval metadata.
The raw report can be regenerated from the retained standalone session log:

```sh
tools/report_sophia_terminal_performance.sh
```

If the machine locks or the report fails, retain the standalone session,
launcher, input-guard, recovery, lifecycle, and protected kernel logs. Do not
repeat the physical takeover until that evidence is diagnosed. This benchmark
does not establish Xserver parity: Copy-based xterm redraw has no equivalent
per-frame flip cadence yet.

After greetd restores the normal Xorg or XLibre session, open a terminal in
that session and run:

```sh
tools/benchmark_xserver_graphics.sh
```

The Xserver runner compiles a bounded XCB observer, launches the identical
`vkcube` command, and measures the server's Present Complete timestamps. It
does not infer cadence from process wall time. The final comparison refuses
different workload geometry, frame count, Vulkan present mode, Vulkan provider,
or output pixel count. The default gate requires at least 90% of Xserver FPS
and permits at most `xserver_p95 / 0.90`. Override
`SOPHIA_RENDER_MIN_BASELINE_RATIO` only when a documented milestone sets a
different threshold.

The X Present completion path is an observed result, not a comparability input.
An unredirected Xserver may report `Flip`; a composited desktop may report
`Copy`. Both carry advancing FIFO UST/MSC cadence, but `Copy` can complete
before the desktop compositor's eventual scanout. The comparison therefore
labels unlike paths `comparability=cadence_only` and never promotes their p95
ratio to an end-to-end scanout- or input-latency claim. Sophia's mixed/CPU
composition record is post-KMS `Copy`. `Flip` is reserved for a future frame
that reaches direct scanout without composition.

If `glxgears` is installed, the Xserver runner also records a bounded mean-FPS
sample as `role=compatibility_probe`. On Void Linux it is supplied by
`mesa-demos`. This result establishes the reference Xserver's GLX/OpenGL
cadence and exposes gross reference-path regressions. It is not a renderer
benchmark and never supplies Sophia's Vulkan parity threshold. Set
`SOPHIA_XSERVER_GLXGEARS=false` to skip it or `true` to require the binary.

The paired Sophia-side compatibility proof is one command from the dedicated
TTY:

```sh
tools/benchmark_sophia_glxgears_tty3.sh
```

It starts `glxgears` directly in the standalone natural-layout profile, without
Kitty, xmonad, or xmobar. The default 500-by-500, swap-interval-one workload
runs for 20 seconds and exits automatically. Before graphics takeover, a
bounded external-client preflight must reach classic visual discovery, direct
context creation, DRI3 import, and Present submission. Confirm
that the centered window shows three smoothly rotating gears. The trailing
`sophia_glxgears_performance` record reports the client's sampled FPS
separately from Sophia's routed post-KMS Copy FPS and p95 interval. It also
requires an identified GL renderer, positive DRI3/mixed-composition evidence,
Present idle-fence progress, at least one retained-image cache hit, zero
descriptor mismatch or cache-capacity rejection, no submission or retirement
failure, and clean resource drain. This remains a GLX compatibility diagnostic
rather than a substitute for the fixed Vulkan acceptance workload.

The session log must not contain a CPU submission between the first mixed
Present retirement and its successor, a repeated cold import of one live image
generation, nor an AMD `context is guilty` recovery. The first run that
rendered only a flash of gears violated these invariants: stale CPU fallback
blanked the composed output, and a focus repaint recreated the current
DMA-BUF's EGLImage/texture. That second import blocked in `glFinish` until AMD
recovered the guilty context. These are generic mixed-presentation lifecycle
failures, not GLX workload failures.

The raw reports and comparison can be regenerated without rerunning either
graphical session:

```sh
tools/report_sophia_rendering_performance.sh
tools/report_xserver_rendering_performance.sh
tools/compare_sophia_xserver_rendering.sh
```

Never compare a hardware Xserver run to a Sophia Lavapipe run; provider
mismatch measures the Vulkan implementation, not the compositor pipeline. The
comparison rejects that mismatch by hashing `vkcube`'s provider description.
Offline regressions are
`tools/check_sophia_glxgears_performance_reporter.sh`,
`tools/check_sophia_rendering_performance_reporter.sh`,
`tools/check_sophia_terminal_performance_reporter.sh`, and
`tools/check_xserver_rendering_performance_reporter.sh`.

For the visible xmonad/KMS proof, run
`tools/start_sophia_xmonad_vkcube_recovery_tty3.sh`, launch
`vkcube --wsi xcb`, exit normally, then run:

```sh
tools/verify_sophia_xmonad_vkcube_recovery.sh
```

The verifier joins every armed visual admission to the same surface and
transaction at both Engine admission completion and native page-flip
retirement. Before the admission may arm, the same transaction must appear as
an Engine-selected `PresentedBuffer` visual candidate with a concrete natural
extent. This rejects the historical failure where a later tile-sized software
backing clear replaced the Vulkan frame as recovery evidence. The verifier also
requires bounded clean teardown and rejects malformed, overflowed, mismatched,
or degraded intake.

The retained two-xterm hardware proof must preserve its 2,000 ms startup,
25 ms maximum-composition, 100 ms input-to-presentation, complete event-flush,
two-layer, and clean-teardown gates. Its current retained result is 1,487 ms,
10 ms, 23 ms, all 14 events flushed, and no cleanup debt. It remains
`hardware`, not `session`, evidence until the compatibility-matrix promotion
gate passes.

The emergency-recovery QEMU scenario starts the independent input guard, sends
one complete Ctrl-Alt-Backspace chord to arm it, waits for the live virtual
libinput path and committed focus, then sends a second chord. It requires both
the guard trigger and an `emergency_exit`, followed by bounded live-session and
native-scanout cleanup. It is the non-destructive prerequisite for the
installed-session recovery test. QEMU does not exercise the host VT or DRM
device, so it cannot replace the final guarded hardware restoration gate.

`live-session-composition-smoke` is non-destructive. Its reduced output must
report `status=Passed`, one or more drained authority batches, committed runtime
transactions, applied runtime surfaces, and
`rendered_scanout_submit=SubmittedWaitingForPageFlip`,
`rendered_scanout_retire=RetiredAfterPageFlip`,
`runtime_scanout_state=Retired`, and `cleanup_pending=false`.

Use `tools/qemu_session_harness.sh` for repeated native GBM/KMS development.
The harness waits for the guest's physical-input readiness marker, types
`sophia` through QMP and virtio-keyboard, and requires all press/release events
to pass through libinput and Engine focus before changed xterm pixels count as
input proof. QMP also performs a bounded virtio-mouse double-click selection,
and the verifier requires nonzero routed pointer events plus a second xterm
pixel change. The guest requires two connected virtual KMS outputs. Engine
discovers both, owns both connector/CRTC/primary-plane chains, and presents an
extended horizontal desktop. The verifier requires per-output nonzero
submissions, callbacks, retirements, exports, distinct checksums, and a
page-flip-paced vsync record with zero overlap or phase rejection.
It boots an isolated direct-kernel initramfs with virtio-gpu and verifies
exactly 300 session ticks without host DRM, input-device, VT, disk, or guest
network access. The QEMU evidence verifier also rejects native submit/retire
failure, rejected callbacks, in-flight ownership, cleanup debt, and a tick
count other than 300. QEMU does not claim VRR because virtio-gpu does not expose
the physical property contract. Keep the physical TTY proof for the AMD
multi-connector/VRR gates and operator-typed input evidence.

The `xmonad-m7` scenario additionally waits for two visible managed surfaces
and proves plain-click focus and click-drag focus as independent sequences.
Before each gesture, Super-J establishes keyboard focus away from the target
tile. The verifier then requires pointer focus request, Engine focus commit,
focused-border composition and damage, X11 focus acknowledgment, retained
handoff delivery, and a following virtio-keyboard event routed to the same
opaque surface, in that order. The click requires both button edges; the drag
requires press, motion, and release. The scenario then drives the virtio mouse
beyond the right edge of the two-output union and sends one reverse delta. The
verifier requires an Engine confinement observation, an immediate-reversal
observation after it, and only then the completed QMP sequence marker.

The same scenario switches to an authoritative empty workspace before the WM
restart exercise. It sends one primary click and requires both edges to be
suppressed with `reason=no_target`, with no pointer-focus request or client
route. It returns with Super-1 because no focused surface exists to move. This
guards against initial-focus reconciliation selecting a committed surface that
the external WM has projected out. Together these checks prove the stateful
raw-to-logical pointer path and hidden-surface exclusion without claiming
physical cursor visibility or the required all-edge hardware exercise.

### X11 Live-Session Stability Diagnostics

After an allocator abort or a flushed-input pixel timeout, do not repeatedly run
the combined milestone gate. From a dedicated local text TTY with no graphical
compositor, first run the automated synthetic-input stability workload:

```sh
tools/run_x11_live_session_stability.sh --runs 10
```

The workload uses the same persistent X11 CPU-upload and Engine-owned KMS path
without requiring operator typing. It retains independent logs under
`~/.local/state/sophia/x11-live-session-stability/`. To capture the native
allocator seam, use exactly one diagnostic mode:

```sh
tools/run_x11_live_session_stability.sh --diagnostic
tools/run_x11_live_session_stability.sh --trace
tools/run_x11_live_session_stability.sh --core
```

GDB follows the Sophia parent and detaches from xterm, builds the release binary
with full Rust debuginfo, enables glibc and Mesa diagnostics, and records all
thread stacks. Core mode caps the dump at 128 MiB. Trace mode records reduced
EGL/GBM lifecycle stages without native handles. A fix is not hardware-stable
until ten automated runs report exact terminal text plus changed pixels, three
physical keyboard/pixel runs do the same, and the complete milestone helper
passes without allocator output, stale generations, missing pixels, callback
rejection, in-flight ownership, or cleanup debt.

The two remaining physical gates are deliberately separate from the QEMU
regression loop. On a dedicated TTY, after the graphical compositor has
released DRM master, run:

```sh
tools/finish_milestones_1_2.sh
```

The historical combined helper auto-selects the keyboard when exactly one stable
keyboard path exists and executes the retained physical diagnostics. It
refuses to proceed while River, Niri, Sway, Hyprland, KWin, or Xorg is active,
so invoking it accidentally from the live graphical session cannot take DRM
master. Set `SOPHIA_OPERATOR_KEYBOARD` only when more than one keyboard path is
listed. When `keyd` is active, the interactive helper uses `sudo sv down keyd`
before opening the physical keyboard and an EXIT trap restores it with
`sudo sv up keyd` after success, failure, or interruption. There is no separate
confirmation prompt whose Return could enter the exact input proof.

The supported physical TTY3 input path is currently the guarded Kitty-only
profile:

```sh
tools/start_sophia_kitty_tty3.sh
```

Run it after switching to `/dev/tty3`. It stops the active runit display
manager, drives every connected output, starts one automatically focused
Kitty, and restores the display manager on exit. The Kitty profile removes
Wayland display variables and disables desktop-service bus activation so
portal, notification, and settings helpers cannot delay X11 window creation.
It also uses Kitty's `--config NONE` profile, forces opaque X11 rendering
because no compositing manager is present, and leaves an interactive shell
running under a fixed diagnostic title.
Startup must reach a focused,
visibly presented surface within eight seconds or the wrapper fails closed
and restores the TTY. Launcher output is retained in
`/tmp/sophia-kitty-tty3-launch.log`.

The initial classic hardware cursor is centered on the primary output before
the first application surface. Sophia prefers atomic cursor planes, detaches
inherited ownership before attaching its canonical X11 `left_ptr`, and uses
the DRM legacy hardware-cursor ioctl only on devices such as virtio-gpu that
expose no compatible atomic cursor plane. Both paths retain one backend-owned
hardware cursor; neither bakes a second cursor into primary-plane pixels. Move
it across both outputs, click and drag to select terminal text, type into
Kitty, and close Kitty normally. xmonad and Super-Enter remain outside this
gate.
Ctrl-Alt-Backspace remains the independent emergency recovery chord.

The launcher uses libinput's udev backend on `seat0`. It discovers every
keyboard, pointer, and touchpad assigned to that seat and follows device
add/remove events. `SOPHIA_OPERATOR_INPUT_SEAT` selects another seat.
`SOPHIA_OPERATOR_INPUT_DEVICES` remains an explicit comma-separated path
override for deterministic diagnostics; ordinary sessions do not enumerate
`/dev/input/by-id` in shell.

The xmonad path remains a guarded promotion candidate. Run it only from
`/dev/tty3`:

```sh
tools/start_sophia_xmonad_tty3.sh
```

Before a physical run, the bounded real-client resize gate must pass:

```sh
tools/xmonad_live_session_smoke.sh
```

It launches the installed Kitty binary through the normal application
registry, supervises the generic bridge and real xmonad, waits for visible
startup, commits a 960x640 resize with ConfigureNotify acknowledgement and
later pixels, routes exact shell input only after that commit, and requires the
shell's semantic result plus another changed frame, zero unexpected protocol
errors, and clean frontend worker teardown. It retains evidence at
`/tmp/sophia-xmonad-live-session.log`.

The launcher shares the proven Kitty takeover and recovery lifecycle, but adds
the generic bridge as `sophia-live-session`'s supervised WM process and uses
xmonad only as the selected policy client. Kitty remains a normal application
in the bounded session registry; neither Engine nor X Authority contains
Kitty-specific window-management behavior.

The former normal-promotion workflow combined repeat, clipboard, pointer-edge,
VT, workspace, close, and layout gestures into one long operator script. It is
retained only as an exhaustive focused diagnostic:

```sh
tools/start_sophia_xmonad_tty3.sh
tools/verify_sophia_xmonad_tty3.sh
```

The verifier requires physical keyboard routing, at least two XKB-derived
held-key repeat pulses with a fully drained repeat ledger, click-drag pointer
transitions, at least one clipboard owner change and selection conversion,
startup and action-launched Kitty processes, committed focus,
layout, workspace-away/workspace-return, close, and logout actions, independent
page-flip retirement on two outputs, clean native retirement, an armed but
untriggered guard, and exact KD mode and termios restoration. Its default evidence is under
`~/.local/state/sophia/xmonad-session/`; launcher output is retained at
`/tmp/sophia-xmonad-tty3-launch.log`. A failed condition names the missing
proof.

The promotion ledger instead launches the short hardware smoke through
`tools/start_sophia_xmonad_hardware_smoke_tty3.sh`. Its complete sequence is
printed on screen: create four stable Kitty tiles, confirm click focus and
typing, make one TTY2/TTY3 round-trip, and log out normally. The verifier also
runs the strict four-Kitty geometry, latency, resource-lifetime, and teardown
checks against that same session log.

For the standalone focused four-Kitty Tall regression, use:

```sh
tools/start_sophia_xmonad_four_kitty_tty3.sh
tools/verify_sophia_xmonad_four_kitty.sh
```

Follow the sequence printed by the launcher and use normal xmonad logout.
The verifier waits briefly for completion, correlates the four-window held
resize epoch with a matching atomic commit of all three or four changed
surfaces, derives the Tall layout from the Engine work area applied to that
output, requires one pixel-matched work-area-height pane plus three stack panes
that exactly cover the remainder, and rejects empty per-output
submissions, forced native detach, abandoned scanouts, callback imbalance, or
cleanup debt. It also requires at least 32 mixed exports, one complete
composition target and frame surface per safe export, zero recovery
replacement, zero launch-admission timeout, and at most 100 ms for input queue
dwell, CPU upload,
and submit-to-page-flip latency. Each
output must also retain one
`sophia_live_native_startup_output ... proof=synchronous_modeset` record; later
asynchronous submissions still require matching callbacks. The separate
`sophia_live_native_resources schema=2` completion record contains only
reduced creation and replacement counts; it does not expose native handles or
application metadata. While physical input is active, the session owner polls
its X authority channel on the one-millisecond budget so idle X traffic cannot
consume the input dwell allowance.

The focused gate also requires one
`sophia_live_owner_timing schema=2 status=complete` record. Child reaping and
physical-input routing must each remain within 100 ms. Native retirement runs
before shortcut routing.

It also requires one
`sophia_live_wm_transport schema=1 status=complete` record. The bounded
transport must drain with zero pending or capacity-rejected requests, peak at
no more than sixteen owner entries, reject no more than sixteen stale
responses, and keep queue dwell and socket round trip within the external
policy timeout of 500 ms. The socket round trip is intentionally not charged
to the owner-thread 100 ms budget: the typed worker waits while the owner
continues input, rendering, and page-flip service.

Validate verifier changes with:

```sh
tools/check_sophia_xmonad_four_kitty_verifier.sh
```

Use the focused physical click and click-drag gate from TTY3:

```sh
tools/start_sophia_xmonad_pointer_focus_tty3.sh
```

The wrapper prints the complete interaction sequence, runs the normal xmonad
session, returns to the originating TTY after normal logout, and automatically
verifies the retained log. It first requires a plain primary click and
following key on an unfocused tile, then requires focus to move away before a
primary click-drag and separate following key on that tile. Both handoffs must
contain the blind-WM request, Engine commit, X-frontend acknowledgment, ordered
release, and key delivery to the selected opaque surface. The drag handoff must
retain at least one motion record in addition to press and release.

For an already captured log, run:

```sh
tools/verify_sophia_xmonad_pointer_focus_pair.sh
```

The older `tools/verify_sophia_xmonad_pointer_focus.sh` remains a useful
single-handoff diagnostic. Validate both verifier shapes with:

```sh
tools/check_sophia_xmonad_pointer_focus_verifier.sh
tools/check_sophia_xmonad_pointer_focus_pair_verifier.sh
```

For the exhaustive unmodified-xmobar work-area diagnostic, use:

```sh
tools/start_sophia_xmonad_xmobar_tty3.sh
tools/verify_sophia_xmonad_xmobar.sh
```

Follow the launcher's exact click, scroll, and normal-logout
sequence. The verifier requires one active reservation reduced across both
outputs, exact top-edge work-area geometry, pixel-matched managed presentation
below the bar, and both button and axis routing to a generic
`ClientPositioned` surface. The short promotion verifier also requires clean
native drain, an untriggered emergency guard, and exact TTY restoration. The
older strict verifier retains workspace/VT assertions for focused regression
work after those domains change. The role evidence is application-agnostic:
the Engine and live input path do not identify xmobar or any other bar
implementation.

Validate changes to this focused verifier with:

```sh
tools/check_sophia_xmonad_xmobar_verifier.sh
```

The xmobar verifier also requires one aggregate compositor-chrome record with
one eligible managed Kitty, one focused frame, one focus ring, and no second
eligible surface. This proves the generic `ClientPositioned` role excludes the
bar without naming xmobar inside Engine or the renderer.

## Commit-pinned Milestone 9 promotion

After committing a candidate, run the first gate from any text terminal:

```sh
tools/sophia_m9_promotion.sh next
```

Gate zero is unattended. It runs the canonical offline local regression suite,
the two-output M7 xmonad and M8 mixed-application QEMU scenarios, and the
isolated QEMU libinput-to-kernel-page-flip latency regression. The retained
evidence covers focus click and drag, hidden-workspace input suppression,
layout/workspace actions, launch/close/logout, clipboard and PRIMARY,
Firefox/Kitty/Vulkan interaction, resize, dialog, bridge restart, compositor
damage, exact input/frame clock correlation, and clean teardown. It is tied to
the exact candidate commit.
The M8 browser scroll stage first requires a newly observed and routed physical
axis event, then uses a focused Space key to advance the deterministic local
page. It proves Engine axis routing but does not claim that the current X
frontend produces native Firefox DOM `wheel` events.

After that gate passes, switch to a logged-in TTY3 and run the same command
once for each remaining pending gate. The command refuses a dirty worktree,
selects exactly one gate, prints its complete short interaction sequence,
archives session/guard/recovery/sequence evidence under
`$XDG_STATE_HOME/sophia/m9-promotion/<commit>/`, and advances only after its
fail-closed verifier succeeds. The ordered gates are:

1. Unattended QEMU semantic gate.
2. Native chrome and hot-reload hardware proof.
3. Four-Kitty keyboard, pointer-focus, two-output, and one-VT hardware smoke.
4. Short xmobar geometry, pointer, and retained-keyboard-focus hardware smoke.
5. Independent emergency recovery.

QEMU does not certify the physical GPU/KMS path, actual keyboard and mouse,
monitor pixels, libseat VT ownership, greetd transition, or emergency TTY
recovery. Those remain physical. Conversely, the physical gates do not ask the
operator to manually replay deterministic clipboard, workspace, application,
and protocol state machines already exercised by QEMU.

The emergency guard intentionally returns launcher status 130 after taking
control. The promotion driver accepts that status only for the emergency gate,
then requires the archived session to have drained routed client keys and
native state, exited successfully, and restored the originating TTY. Status
130 alone never satisfies the gate.

Inspect progress from any text terminal with:

```sh
tools/sophia_m9_promotion.sh status
```

Evidence from another commit cannot implicitly satisfy the current ledger.
Failed runs are retained in timestamped directories but do not advance the
next gate.

If a promotion-tool-only commit immediately follows a candidate whose native
chrome gate already passed, this explicit command may adopt that one parent
gate:

```sh
tools/sophia_m9_promotion.sh adopt-parent-native
```

Adoption succeeds only when the parent result still verifies and Git proves
that Engine/runtime crates plus every native gate dependency are unchanged.
The copied result records both the current commit and its source commit.

If the immediately preceding candidate completed the hardware smoke but an
outdated verifier rejected otherwise valid evidence, correct and commit only
the hardware-smoke verifier, its regression fixtures, the promotion driver,
and their documentation. Then reverify and adopt that retained run with:

```sh
tools/sophia_m9_promotion.sh adopt-parent-hardware
```

This command accepts only a parent result rejected by verification, not a
launcher or runtime failure. It uses a closed verifier-only path allowlist,
reruns both hardware-smoke verifiers against the archived session, guard, and
recovery evidence, and records the parent as the evidence source. Any runtime
or gate-input change rejects adoption and requires a new physical run.

If the first four gates pass and emergency recovery then exposes a defect
confined to acknowledgement-barrier retirement, commit that narrow fix and
run the new commit's unattended semantic gate. The already-passing physical
evidence can then be reverified and adopted with:

```sh
tools/sophia_m9_promotion.sh adopt-parent-pre-emergency
```

This exception accepts only the owner-loop delivery reducer, its focused test,
and promotion documentation/tooling. It rejects renderer, frontend, WM,
launcher, verifier, input-routing, and general runtime changes. Native,
hardware-smoke, and xmobar evidence is reverified before source-commit results
are written. Emergency evidence is never adopted.

The exhaustive keyboard/VT runner remains a focused diagnostic after input,
XKB, seat, or VT changes. It logs only the count of 21 shifted printable key
positions and 12 VT targets; typed content remains redacted:

```sh
tools/check_sophia_xmonad_keyboard_vt_verifier.sh
```

For the bounded Super-Enter overload regression, use:

```sh
tools/start_sophia_xmonad_launch_burst_tty3.sh
tools/verify_sophia_xmonad_launch_burst.sh
```

Press Super-Enter rapidly at least four times, including before the startup
prompt if possible; use twenty presses to exercise overflow. Session policy
admits one opaque application surface at a time, retains at most sixteen
active-plus-queued action launches, and rejects excess requests without
terminating Sophia. The verifier requires every output's callback-bearing
startup baseline before the first action application starts, two to sixteen
complete sequential admissions without timeout, post-burst keyboard routing,
normal logout, and clean presentation teardown.
Validate verifier mutations with:

```sh
tools/check_sophia_xmonad_launch_burst_verifier.sh
```

Run emergency recovery as a separate capture:

```sh
tools/start_sophia_xmonad_emergency_tty3.sh
```

After the guard returns the session to the originating TTY, verify it with:

```sh
tools/verify_sophia_xmonad_emergency_tty3.sh
```

This verifier requires both the independent guard and live owner loop to
observe the chord. It rejects process-group `TERM` fallback as successful
promotion evidence and requires a bounded session completion with drained
input, no native scanout or Present debt, exact KD-mode restoration, and exact
termios restoration.

For an installed release, run the same verifier from the immutable current
artifact:

```sh
/opt/sophia/current/bin/sophia-verify-emergency-run
```

### Physical Firefox workflow

After the normal xmonad input gate passes, run the content-redacted physical
Firefox workflow from TTY3:

```sh
tools/start_sophia_xmonad_firefox_proof_tty3.sh
```

The launcher prints the exact interaction sequence. It uses the offline fixture
and requires keyboard, `CLIPBOARD`, `PRIMARY`, physical scroll, resize,
focus-away/focus-return, and a pointer-opened dialog, two Firefox launches with
status-zero exits, an independently interactive second Kitty, and normal
xmonad logout. Verify and retain the run with:

```sh
tools/verify_sophia_firefox_physical.sh
tools/record_sophia_firefox_physical_run.sh
tools/verify_sophia_firefox_physical_runs.sh 3
```

The installed artifact exposes the same workflow as the `Sophia Firefox Proof`
greetd entry. After each installed run use `sophia-record-firefox-run`; after
three runs use `sophia-verify-firefox-runs 3`. Those commands verify immutable
release digests and privacy-safe runtime identities in addition to the browser,
WM, input, renderer, and teardown evidence.

## Installed Daily-Driver Candidate

An installed release is built and frozen separately from login:

```sh
tools/install_current_live_session.sh
```

Packaging refuses a dirty worktree, builds optimized Sophia and the generic WM
bridge, resolves the tested xmonad binary, and records SHA-256 digests plus the
exact Git commit in an immutable release directory. Installation verifies
those digests, copies the release below `/opt/sophia/releases/`, atomically
updates `/opt/sophia/current`, preserves the former target as
`/opt/sophia/previous`, and installs the Sophia greetd session entries below
`/usr/share/wayland-sessions/`, the directory configured for greetd/tuigreet
session discovery.

The installed Kitty baseline is `Sophia Kitty (Baseline)`; the integrated
policy candidate is `Sophia xmonad (Experimental)`. Both enter through
`/usr/local/bin/sophia-session`, which performs no
source build, repository lookup, display-manager takeover, or privileged
service control. It emits the installed version and commit before entering the
same guarded session lifecycle. It fails closed unless greetd supplies
an existing, absolute, user-owned `XDG_RUNTIME_DIR` and a real local Linux VT.
The lifecycle log records ordered preflight, input-guard, graphics-takeover,
session, and bounded display-manager-handoff phases without application
content. Inspect the latest phase or roll back the installation with:

```sh
sophia-status
sophia-stop
sudo sophia-rollback
```

`sophia-stop` requests bounded cleanup from an independently logged-in control
TTY; Ctrl-Alt-Backspace remains the independent local emergency chord.
Because the graphical owner deliberately places the kernel keyboard in
off-mode, Sophia recognizes Ctrl-Alt-F1 through Ctrl-Alt-F12 and explicitly
requests the target Linux VT through libseat. Seat disable pauses physical
input, drains KMS ownership, and preserves the X clients and Engine scene;
seat enable reacquires devices and repaints retained content. Shifted US
punctuation is covered by the
same `evdev`/`pc105`/`us` XKB state used for client-visible core and XKB events.

The artifact also installs repository-independent evidence commands:

```sh
# After each complete normal physical run:
sophia-record-run

# After one separate Ctrl-Alt-Backspace recovery run:
sophia-record-emergency-run

# Require the latest three recorded runs to be clean and from one commit:
sophia-verify-cycles 3

# Verify a completed two-hour session with at least ten terminal and five
# Firefox action launches:
sophia-verify-soak \
  ~/.local/state/sophia/xmonad-session/session.log 7200000 10 5

# Ten clean installed login cycles:
sophia-verify-cycles 10

# Full eight-hour workday, with proportionally broader app exercise:
sophia-verify-soak \
  ~/.local/state/sophia/xmonad-session/session.log 28800000 30 15
```

Run the long gates through the installed `Sophia Firefox Proof` entry so the
same log also contains the redacted keyboard, `CLIPBOARD`, `PRIMARY`, resize,
and dialog contract. `sophia-record-run` first applies the strict physical
xmonad verifier, checks
the running release identity and every packaged SHA-256 digest, and only then
copies the session, guard, recovery, identity, and release manifest into a
numbered, checksummed promotion-run directory. It also requires the ordered
installed lifecycle and display-manager handoff. `sophia-record-emergency-run`
applies the independent guard/owner recovery verifier and archives the
emergency lifecycle separately. `sophia-verify-cycles` rechecks each archived
lifecycle and rejects mixed commits, modified evidence, emergency exits,
incomplete input/WM/native cleanup, or fewer than the requested number of runs.
`sophia-verify-soak`
requires one clean schema-14 completion, the requested elapsed time and action
counts, and zero WM, Present, callback, native, or cleanup debt.
Each installed launch also rotates a content-free runtime identity record with
the kernel and Mesa versions, Kitty/Firefox/xmonad binary digests, a hashed
input-device identity, and connector plus EDID hashes. The verifier rejects
missing identities and fields that could contain typed, clipboard, or window
content.

`SOPHIA_INSTALL_PREFIX`, `SOPHIA_SESSION_DIR`, and `SOPHIA_COMMAND_DIR` allow a
non-system staging installation for verifier tests. A release is not promoted
to daily-driver status merely because installation succeeds; the three-login,
recovery, fallback, repeated-cycle, and soak gates remain mandatory.

If an interactive physical run loses visible terminal content after typing,
capture a bounded exact-input diagnostic instead of repeating an open-ended
session:

```sh
tools/start_sophia_xmonad_input_proof_tty3.sh
```

Wait for Kitty to finish its initial xmonad resize and show a prompt. Type
`sophia`, press Return, wait one second, then move the pointer and click once.
The command exits automatically only after the exact keyboard sequence reaches
the terminal, a later native frame is presented, and physical pointer motion
plus a button are routed. The diagnostic uses the ordinary reduced session
evidence; verbose per-frame EGL/X11 tracing remains disabled so observation
does not perturb the timing under test.

The generic development runner still supports
`tools/run_sophia_session.sh --wm=xmonad`; `--wm=native` selects the Sophia WM
API demo and `--wm=none` runs without external policy. The unattended
`tools/qemu_xmonad_m8_launcher_acceptance.sh` gate proves startup, xmonad-driven
terminal launch and close, logout, bridge restart recovery, and clean two-output
shutdown. The frozen Milestone 7 regression remains
`tools/qemu_xmonad_m7_acceptance.sh`.

The deterministic native-presentation lifecycle gate is:

```sh
cargo test --offline -q -p sophia-cli \
  --test resize_transaction --test session_startup
cargo test --offline -q -p sophia-renderer-live \
  --test buffer_registry --test presentation_boundary
cargo test --offline -q -p sophia-backend-live --all-features \
  --test startup --test presentation_feature --test libdrm_events_feature
```

This set retains compensating resize rollback and abandoned-pixel fencing,
output/target size replacement and stale-allocation removal, stale prepared
page-flip settlement, displayed-buffer replacement, cleanup retry, and
duplicate-retirement rejection. The relevant tests assert that an accepted
replacement retires only its predecessor, final cleanup retires the displayed
owner, repeated cleanup becomes a no-op, and stale callbacks cannot consume a
new owner. The gate is deterministic lifecycle evidence; balanced target,
pipeline, frame-surface, callback, and shutdown counts in a physical session
remain required.

The M7 gate independently drives two pointer-focus sequences against an
unfocused visible surface. Plain click queues and releases the primary press
and release; click-drag additionally queues motion between them. Each sequence
must commit WM-selected focus, receive X-frontend acknowledgment, release its
ordered handoff, and route its own following key to that same opaque surface.
Each committed pointer focus must also produce a four-band focus ring
and retire nonzero compositor plus combined output damage before the sequence
completes. The matching safe repaint may be `partial` when only small regions
changed or `full` when the transaction also changed client generations,
stacking, or sufficient coverage. The complete run must observe focus rings on
at least two distinct focus targets and initial display-list baselines on both
outputs. The verifier remains virtual-input evidence; it does not replace
physical DRM, libinput, resize, workspace, or VT confirmation.

After a normal physical xmonad capture that exercises two focus targets, one
focused resize, an empty workspace round-trip, and one VT round-trip, run:

```sh
tools/verify_sophia_xmonad_focused_border.sh
```

The verifier correlates each ring with committed opaque focus, requires four
bands, requires a geometry/style generation change without using client pixel
generations, checks ring restoration after workspace and VT resume,
requires nonzero mixed composition, and rejects degraded or unclean teardown.
Its own fail-closed fixtures run through:

```sh
tools/check_sophia_xmonad_focused_border_verifier.sh
```

Milestone 8 adds two explicit application scenarios:

```sh
cargo run --offline -q -p sophia-cli -- x-authority-firefox-smoke
tools/qemu_xmonad_m8_mix_acceptance.sh
tools/qemu_xmonad_m8_soak_acceptance.sh
```

The M8 image profile is isolated, offline, and storage-free. It requires
Firefox, `vkcube`, and Mesa Lavapipe; set `SOPHIA_FIREFOX_BIN`,
`SOPHIA_VKCUBE_BIN`, or `SOPHIA_LVP_ICD` when they are not installed in the
standard Void paths. The mix verifier requires four managed application IDs,
two independently retired outputs, workspace and bridge-recovery evidence,
zero normal-session protocol errors, and final frontend/application cleanup.
It also requires each close shortcut to clear its client key ledger without a
delivery barrier, a fully drained control queue, and zero control timeouts.
The soak reuses that scenario for at least 1,800,000 ms and twenty complete
interaction cycles. Application launch and close steps are evidence-driven:
the harness waits for the matching managed-process start, committed layout,
close acknowledgement, and zero-status managed-process exit before advancing,
so a slow Firefox startup or stale xmonad focus cannot be hidden by fixed
sleeps. The Firefox stage record contains only redacted title byte lengths and
bounded selection-operation counts; title content and X object identities stay
inside the frontend. The soak verifier additionally requires twenty terminal,
Firefox, and launcher restarts plus sixty committed close actions.
The installed GTK4 Zenity probe runs through the retained Cairo-renderer
wrapper so it maps without an EGL-capable guest display and reports explicit
child-reaped and wrapper-complete evidence. Close/launch actions use bounded
retries around busy WM layout proposals and require the matching managed
process to exit successfully.

The older bounded proof wrapper can pass `--max-runtime-ms=10000`,
`--inject-text=sophia`, and `--exit-after-input-proof`; passing evidence requires one
WM request, one acknowledged resize, one committed layout with a moved surface,
the proof shell to receive exactly `sophia`, and a later terminal pixel change.
Schema-11 live evidence records `input_text_match=true`; changed pixels without
the semantic completion record fail verification. The separate xterm raster
smoke also requires an exact readable ASCII glyph match. The final physical gate uses the wrapper
without injection and requires operator-visible typing on the scanned-out xterm.

The keyboard helper presents `type sophia then Return:` inside the scanned-out
xterm and waits up to 15 seconds for that exact press/release sequence. A fresh
five-second pixel deadline begins only after Return is released. The proof fails
on an unexpected key, modifier, repeat, missing release, missing Return, or if
later xterm pixels do not change. Accepted evidence records all 14 expected and
matched events after Engine focus routing and core-X translation. The VRR helper
is destructive and requires a connector reporting `vrr_capable=1` plus
the selected CRTC's `VRR_ENABLED` property. It submits an Engine-eligible
opaque fullscreen modeset with `VRR_ENABLED=true`, waits for presentation, then
submits an overlay/ineligible fixed-refresh fallback with `VRR_ENABLED=false`
and waits for its page flip and retirement. Both reduced phase lines and the
underlying atomic scanout lines must pass their strict verifiers.

Use `atomic-vrr-inspect` for a non-destructive reduced view of the selected
connector/CRTC IDs, discovery status, capability value, enable-property
presence, and bounded property-name lists. A connector reporting
`vrr_capable=0` is valid unsupported hardware and cannot satisfy the activation
gate even when its CRTC exposes `VRR_ENABLED`.

The optional renderer-native features have extra local checks:

```sh
cargo test --offline -p sophia-renderer-live --features gbm-probe
cargo test --offline -p sophia-backend-live --features gbm-probe
cargo test --offline -p sophia-renderer-live --features egl-probe
cargo test --offline -p sophia-backend-live --features egl-probe
cargo test --offline -p sophia-backend-live --features gbm-probe,egl-probe
cargo test --offline -p sophia-backend-live --features libdrm-events
cargo test --offline -p sophia-backend-live --features libinput-events
cargo test --offline -p sophia-backend-live --features libdrm-events,libinput-events
```

The `gbm-probe` feature admits the safe `gbm` crate behind an optional feature.
It exercises fake and native GBM capability records while keeping the public
boundary reduced to capability health. This command must remain optional, and
the default workspace suite must continue to pass without native renderer
feature flags.

The `egl-probe` feature admits `khronos-egl` through the internal
`sophia-renderer-native-egl` adapter crate. That crate owns the unavoidable
unsafe dynamic EGL calls. Public renderer-live and backend-live tests assert
only reduced EGL startup and draw-smoke status.

The `libdrm-events` feature admits the Rust `drm` crate as an optional
backend-live dependency. It checks only the reduced dependency-admission report,
private native adapter skeleton, page-flip event polling adapter shape, and
deterministic fake poller that feeds the runtime-owned bounded callback queue.
Native page-flip values must be reduced before they reach runtime observation.
The native-shaped reader contract is still deterministic: tests feed reduced
native callback facts through a bounded reader before the poller decodes them
through backend-local output routes.
Real libdrm event validation is gated by
`SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE=1`. Without that variable, future hardware
smokes must return a reduced skipped report and avoid opening DRM device nodes.
Until a concrete native page-flip reader exists, the reduced smoke report fails
closed as `BackendUnavailable` when this gate is requested.

The `libinput-events` feature admits the safe Rust `input` wrapper as the
concrete libinput dependency. It defines the reduced live input event reader and
poller shape, proves that the poller implements Sophia Engine's non-blocking
input contract, and smoke-tests an empty path-based libinput context without
opening devices. The reader reduces pointer motion, pointer button, and
keyboard key events through a reduced seat/device map without changing runtime
reports.
Real libinput validation is gated by
`SOPHIA_RUN_REAL_LIBINPUT_EVENTS_SMOKE=1`. Without that variable, future
hardware smokes must return a reduced skipped report and avoid opening input
devices or reporting device paths, seat names, file descriptors, or libinput
error strings. Until device-opening hardware smoke is admitted, the reduced
smoke report fails closed as `BackendUnavailable` when this gate is requested.

The backend-live GBM feature suite includes an opt-in real-device smoke. Set
`SOPHIA_RUN_REAL_GBM_SMOKE=1` to let the test look for an openable
`/dev/dri/renderD*` node, route that backend-owned fd-like authority through the
GBM probe, and assert only reduced startup status. Without that environment
variable, the smoke returns early. This keeps CI deterministic and avoids
letting native driver crashes fail ordinary validation.

The combined `gbm-probe,egl-probe` backend suite uses the same environment gate
for the GBM-backed EGL path. When `SOPHIA_RUN_REAL_GBM_SMOKE=1` is set and an
openable render node exists, the test requires the private GBM/EGL draw smoke to
reach `ClearColorReady` and the offscreen presentation smoke to reach `Ready`.
It still exposes no render-node path, fd, GBM object, EGL object, pixel, driver
error, or KMS identity through Sophia's public reports. The real GBM/EGL smoke
runs the native path in a child test process so a driver crash reports as an
opt-in validation failure instead of terminating ordinary deterministic tests.

The combined `libdrm-events,gbm-probe` backend suite also includes an opt-in
atomic scanout smoke. Run `tools/atomic_scanout_preflight.sh` first when the
host state is unknown. That preflight drives the feature-gated
`sophia atomic-scanout-preflight` CLI command instead of a test filter. It does
not request DRM master, does not modeset hardware, and emits exactly one reduced
`sophia_atomic_scanout_preflight` line: schema version, validation target,
readiness status, capped primary card count, capped read/write-openable primary
card count, capped atomic-capability-admitted primary card count, capped KMS
scanout-target primary card count, and capped atomic-property-ready primary card
count. It does not expose device paths, file descriptors, native errors,
permissions, or KMS object identity. The helper verifies the captured log before
exiting and fails unless the host is smoke-ready. Use
`tools/verify_atomic_scanout_preflight.sh` directly when you need to check an
existing capture; the verifier requires
`CandidatePrimaryCardsAtomicReady` and at least one primary card node that
admits the `UniversalPlanes` and `Atomic` DRM client capabilities, exposes a
reduced KMS connector/CRTC/primary-plane target, and has the atomic property
handles needed for the primary-plane request. Under `libdrm-events`, preflight
and `select_real_atomic_scanout_card` use the same reduced readiness probe, so
the non-modesetting gate and the destructive selector do not drift.
Real card-selection and page-flip-session setup failures reduce themselves into
`LibdrmNativeAtomicScanoutSmokeEvidence`, so setup evidence stays consistent
outside the smoke harness as well.

The real DRM-master hardware proof needs a machine and login session with these
properties:

- Rust 1.96 or newer is active. Rustup users should pick up
  `rust-toolchain.toml`; distro-package users must install an equivalent
  compiler before running the proof scripts.
- Linux exposes at least one primary `/dev/dri/card*` node and the active
  kernel driver supports atomic KMS.
- At least one display connector is physically connected and has a usable mode.
- The operator has read/write access to the selected primary card node.
- The run can become DRM master. In practice, this usually means using a local
  TTY or a dedicated test session after stopping the active display manager or
  compositor.
- Modeset disruption is acceptable. The destructive smoke is allowed to change
  scanout state while it proves initial modeset and steady page-flip behavior.
- The machine can build the feature-gated backend-live CLI commands offline
  from the checked-out dependency set.

Run `tools/atomic_scanout_smoke.sh` only from a session that may take DRM master
on a primary `/dev/dri/card*` node. The helper verifies preflight first, then
runs the feature-gated `sophia atomic-scanout-smoke` CLI command with
`SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1`. The CLI parent spawns a child process
for the destructive proof and emits reduced `SmokeChildTimeout` evidence if the
child fails to produce page-flip evidence within the bounded deadline. The
helper forwards optional CLI targeting arguments: `--slot`, `--output`,
`--authority`, `--page-flip-timeout-ms`, and `--child-timeout-ms`. The page-flip
timeout bounds native callback waiting inside the smoke child; the child timeout
bounds the parent watchdog around the destructive child process and defaults to
30 seconds. The native page-flip wait defaults to 8 seconds.
Backend-live first uses the production `select_real_atomic_scanout_card` seam to
choose an opaque card owner that opens read/write, admits UniversalPlanes and
Atomic client capabilities, exposes a reduced KMS primary-plane scanout target,
and has the atomic property handles needed for primary-plane commit. The
selected card is then promoted into a page-flip session owner that keeps the
submit card, cloned event reader, and routed poller together. With
`libinput-events` enabled, that same owner can drive one backend runtime tick
through the native GBM rendered-primary-plane scanout path and native page-flip
poller, so callers do not have to split fd ownership apart. That same page-flip
session owner also owns the destructive hardware proof phases. The smoke child
creates a persistent backend-live GBM/EGL rendered-scanout exporter, then asks
the session to run `InitialModeset` and `SteadyPageFlip` proof phases. The
session clears a GBM surface, locks the rendered front buffer through the normal
runtime export seam, submits a primary-plane atomic modeset, waits for reduced
page-flip evidence, and retires the submitted framebuffer resources. It then
exports a second rendered front buffer and submits it through the steady-state
page-flip policy, proving the post-modeset path without `ALLOW_MODESET`. Each
submitted phase waits within a bounded deadline for native page-flip evidence
before reducing the final smoke record.
The real card fd is opened nonblocking, so missing callbacks reduce as missing
evidence instead of hanging inside the DRM event read.
Without verified preflight and that environment variable, the destructive path
never opens or modesets hardware.
The stable evidence shape for that run is the
`sophia_atomic_scanout_evidence` line pair: schema version, phase, overall
status, rendered context status, GBM export status, primary-plane property
discovery status, scanout-buffer import status, reduced GBM export detail,
reduced scanout-buffer format, modifier, and plane-count shape, native resource
creation status, atomic request build status, primary-plane submit status,
reduced request scope, page-flip poll status, reduced commit flags, reduced
page-flip wait outcome, page-flip event status, retirement status, retire-time
resource destroy status, and retire-time cleanup-pending status only. A passing
capture must contain both `InitialModeset` and
`SteadyPageFlip`, and both phases must report `page_flip_wait=Retired`. Failed
captures
reduce the stop point without native identity: smoke-child timeout, primary-card
open, DRM client capability setup, KMS target selection, rendered-context
creation, GBM export stage, retained-resource ownership, scanout-buffer import,
property discovery, resource creation, request build, atomic submit,
request-shape mismatch, page-flip reader setup, page-flip delivery,
page-flip wait state, waiting-retire state, and resource retirement are reported
separately.
Runtime rendered-primary-plane submits can also be captured as reduced
`sophia_runtime_rendered_scanout_submit` lines. Those lines are not a substitute
for the two-phase hardware smoke evidence, but they are useful when inspecting a
running production loop: schema 6 includes the reduced submit status, scanout
target, reduced output size, frame target, reduced frame-target size, GBM export,
scanout-buffer validation, reduced scanout-buffer format, modifier, and
plane-count shape, reduced plane format-table presence, native submit stages,
framebuffer-creation detail, atomic commit flags, commit submit result, runtime
scanout state, and in-flight age, plus whether native cleanup debt is pending,
without exposing DRM object IDs, file descriptors, GEM handles, pitch/offset
arrays, exact modifier values, property blob IDs, or native driver errors.
Runtime retirement and cleanup retries can be captured as
`sophia_runtime_rendered_scanout_retire` and
`sophia_runtime_rendered_scanout_cleanup` lines. They record reduced retirement
status, destroy status, runtime scanout state, in-flight age, and cleanup debt,
so a live loop can distinguish clean retirement, stale callback waits, and
cleanup retry failures.
If the runtime proof producer cannot reach a submit-to-retire observation, it
emits `sophia_runtime_rendered_scanout_failure` with a reduced reason such as
`InitialTickFailed`, `SubmitReportMissing`, `RetireTickFailed`, or
`RetireTimedOut`. Failure lines are useful diagnostics, but they are never valid
clean proof evidence.
Use `tools/verify_runtime_rendered_scanout_evidence.sh` for a narrow clean
runtime proof. It expects exactly one submitted rendered-primary-plane scanout
line and exactly one clean retired line, rejects cleanup retry and failure
lines, and rejects unknown, duplicate, or malformed fields. This verifier proves a
single-frame runtime submit-to-retire observation; the destructive two-phase
hardware proof still comes from `tools/verify_atomic_scanout_evidence.sh`.
To capture that runtime proof on real hardware, run
`tools/runtime_rendered_scanout_evidence.sh` from a session where DRM master and
modeset disruption are acceptable. The helper runs atomic preflight, executes
the feature-gated `sophia atomic-scanout-runtime-evidence` command with
`SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1`, captures the reduced runtime evidence
log, and verifies it with `tools/verify_runtime_rendered_scanout_evidence.sh`.
The stable evidence shape for the GBM/EGL renderer smoke is
`LiveRealGbmSmokeEvidence`: status, draw status, presentation status, and
frame-target allocation status only.

When touching renderer-native code, run both paths:

```sh
cargo test --workspace --offline
cargo test --offline -p sophia-renderer-live --features gbm-probe
cargo test --offline -p sophia-backend-live --features gbm-probe
cargo test --offline -p sophia-renderer-live --features egl-probe
cargo test --offline -p sophia-backend-live --features egl-probe
cargo test --offline -p sophia-backend-live --features gbm-probe,egl-probe
cargo test --offline -p sophia-backend-live --features libdrm-events
cargo test --offline -p sophia-backend-live --features libinput-events
cargo test --offline -p sophia-backend-live --features libdrm-events,libinput-events
```

For the current atomic scanout backend work, the local non-hardware gate is:

```sh
tools/check_atomic_scanout_local.sh
```

It runs formatting, the GBM/EGL renderer checks, the backend-live
libdrm/libinput scanout feature checks, and the reduced verifier fixture
checks. It does not request DRM master or modeset hardware.

Run the opt-in local hardware smoke only when you want real render-node
coverage:

```sh
SOPHIA_RUN_REAL_GBM_SMOKE=1 cargo test --offline -p sophia-backend-live --features gbm-probe,egl-probe
```

The libdrm and libinput real-hardware gates are defined before their concrete
native readers are admitted:

```sh
SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE=1 cargo test --offline -p sophia-backend-live --features libdrm-events
SOPHIA_RUN_REAL_LIBINPUT_EVENTS_SMOKE=1 cargo test --offline -p sophia-backend-live --features libinput-events
```

Until those readers exist, these variables only document the future opt-in
shape. The deterministic feature tests must continue to pass without them.

Run the atomic scanout hardware smoke only from a local session where modeset
and DRM master disruption are acceptable. The helper captures the reduced
preflight log, verifies host readiness, captures the reduced evidence log, and
runs only the opt-in atomic scanout CLI smoke:

```sh
tools/atomic_scanout_smoke.sh
SOPHIA_ATOMIC_SCANOUT_EVIDENCE=/tmp/sophia-atomic-smoke.log tools/atomic_scanout_smoke.sh
tools/atomic_scanout_smoke.sh --slot=1 --output=1 --authority=1 --page-flip-timeout-ms=8000 --child-timeout-ms=30000
```

The helper runs the verified preflight before the smoke and
`tools/verify_atomic_scanout_evidence.sh` after a successful smoke. Set
`SOPHIA_ATOMIC_SCANOUT_SKIP_PREFLIGHT=1` only when preflight is known to be
wrong for the host and a modesetting smoke is still intentional.

For the full hardware proof, prefer `tools/atomic_scanout_hardware_proof.sh`.
It runs preflight once, captures the destructive two-phase atomic evidence,
captures the runtime rendered-scanout submit-to-retire evidence, and verifies
all three reduced logs:

```sh
tools/atomic_scanout_hardware_proof.sh --slot=1 --output=1 --authority=1 --page-flip-timeout-ms=8000 --child-timeout-ms=30000
```

To verify captured logs without rerunning hardware:

```sh
tools/verify_atomic_scanout_evidence.sh /tmp/sophia-atomic-smoke.log
tools/verify_atomic_scanout_preflight.sh /tmp/sophia-atomic-scanout-preflight.log
tools/verify_runtime_rendered_scanout_evidence.sh /tmp/sophia-runtime-rendered-scanout.log
```

To inspect the default hardware-proof logs and local device visibility in one
place, use:

```sh
tools/atomic_scanout_hardware_status.sh
```

This helper does not request DRM master and does not modeset hardware. It may
print local diagnostic facts such as toolchain availability and `/dev/dri` node
counts, so its output is for the operator. The reduced proof artifacts remain
the three verifier-accepted logs.

The verifier accepts only reduced evidence that proves a rendered GBM
front-buffer export, primary-plane property discovery, native resource
creation, atomic request build, primary-plane atomic submit, nonblocking
page-flip commit flags, native page-flip delivery, and explicit resource
retirement for both the initial modeset and steady-state page-flip phases. It
also requires the current evidence schema and rejects duplicate or unknown
fields, so a passing capture cannot smuggle native object identity into the
reduced log. A clean capture may report any reduced created-framebuffer path:
no-modifier AddFB2, modifier-aware AddFB2, or legacy AddFB fallback.

The verifier fixtures can be checked without hardware:

```sh
tools/check_atomic_scanout_verifiers.sh
```

That script proves the preflight verifier accepts only an atomic-ready reduced
host record, rejects impossible count relationships, and rejects native host
identity fields. It also proves the scanout evidence verifier rejects missing
steady-state page-flip evidence, the wrong steady-state request scope, and
native identity fields.

## Retiring `DEFAULT_DISPLAY`

The `DEFAULT_DISPLAY` EGL smoke is temporary, but it is not removable merely
because the GBM-backed path exists. It can be retired only after the opt-in real
render-node validation is repeatably green and the reduced public boundary is
unchanged.

Current decision: keep `DEFAULT_DISPLAY` for now as a host compatibility smoke.
The real GBM/EGL path has passed repeated local validation on the current
machine, but one host is not enough evidence to remove a broad compatibility
check. `DEFAULT_DISPLAY` remains non-production-shaped; it must not be used as
the compositor platform boundary.

Before removing it, record evidence that:

- `SOPHIA_RUN_REAL_GBM_SMOKE=1` passes after a clean build;
- the same command passes in repeated local runs on the target development
  machine;
- the GBM-backed draw smoke reaches `ClearColorReady`;
- the offscreen presentation smoke reaches `Ready`;
- the reduced frame-target allocation smoke reaches `Ready`;
- `LiveRealGbmSmokeEvidence` records `Passed` without exposing native identity;
- driver crashes remain isolated to child-process validation failures;
- no public report exposes render-node paths, file descriptors, GBM/EGL objects,
  native errors, pixels, KMS framebuffer IDs, connector IDs, CRTC IDs, or plane
  IDs.

If any condition fails, keep `DEFAULT_DISPLAY` as a host compatibility smoke and
continue treating GBM-backed EGL as the production-shaped path under
development.

Minimum host/device matrix before retirement:

- one Intel integrated GPU machine;
- one AMD integrated or discrete GPU machine;
- one machine where `/dev/dri/renderD*` exists but GBM/EGL degrades cleanly;
- one headless or restricted environment where the real smoke is skipped or
  unavailable without failing default validation;
- repeated clean-build runs on the primary development machine.

Each matrix entry must record only reduced evidence: command, pass/fail status,
draw status, presentation status, and whether a child-process crash was
contained. Do not record render-node paths, fd numbers, GBM/EGL handles, driver
error strings, pixels, or KMS object identity.
