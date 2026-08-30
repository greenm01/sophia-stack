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

### Shader Sources

The renderer's GLSL lives in its own files under
`crates/sophia-renderer-native-egl/src/gl/shaders/`, embedded at compile time by
`include_str!`, so nothing is read at runtime and there is no asset to deploy.

The reason they are separate files is that a shader error is otherwise not
discoverable until a GPU refuses it, and that refusal is not fatal by design: the
pipeline records `status=unavailable`, falls back to the direct program, and the
session runs on with its filtering silently uncorrected. That is right at runtime
and a poor place to find a typo. A GLSL front end finds it first:

```sh
tools/check_shaders.sh          # or SOPHIA_GLSLANG=/path/to/glslangValidator
```

It refuses to run without a validator rather than passing, and refuses a run that
matched no shader sources rather than reporting success over nothing. It is a
front-end check only: it says the source is valid GLSL, not that a driver's
limits were respected or that a uniform was bound.

### Bounded Formal Transition Model

Milestone 12 adds unattended TLA+ gates for visual candidate preparation,
submission, output-scoped retirement, terminal settlement, resource release,
X11 admission recovery, and full-geometry feedback. They are not Milestone 11
installed-session requirements and add no physical operator steps.

The model and its action-to-Rust boundary map live under `validation/tla`.
Sophia pins the command-line TLA+ Tools v1.7.4 jar by SHA-256. Once that
artifact has been obtained, the check is entirely offline and leaves its TLC
state in a temporary directory:

```sh
SOPHIA_TLA2TOOLS_JAR=/absolute/path/to/tla2tools.jar tools/check_tla.sh
```

The bounded configurations explore retirement and supersession ordering,
exact PresentedBuffer selection through proactive or timeout recovery,
ownership of a software Present by one native frame, move/resize geometry
feedback, stale legacy-WM configure and focus after workspace replacement,
exact cached workspace assignment, quiet/fail-stop response collection,
pixel-silent first-admission retry,
public policy negotiation and transfer assembly, and atomic multi-output
projection.
`ShellWorkAreaCoordination` checks that a future ready shell reservation,
derived work area, and exact WM projection promote as one coherent generation;
normal shell or WM failure preserves the prior presented bundle. It is a target
pre-schema model and is not evidence of a production shell runtime.
`OutputTopologyLifecycle` checks the current native owner's rescan boundary:
replaceable hotplug hints, one routed-input epoch advance, old-scanout
retirement, complete multi-consumer publication, current policy settlement,
and presentation-before-input. No-output and bounded rebuild failure remain
recoverably quarantined.
The frame-ownership model permits an unrelated frame to submit and retire first
and proves that only the exact bound frame can emit feedback.
`GeometryFeedback` separates full rectangles from pixel readiness and proves
no-op silence plus convergence after late-target/FIFO rollback.
`PolicyConnection` requires the full client, connection-epoch, and transaction
identity for admitted work.
`PolicyProjection` requires proposals to answer an outstanding server-issued
request for the current scene generation. `LegacyWmProjection` permits delayed
private WM requests but proves that only exact active-workspace membership
crosses the compatibility bridge. `LegacyWmResponseBoundary` requires quiet
before success and process replacement after an error because legacy X
requests have no Sophia transaction identity. `PixelSilentAdmission` preserves
the owner and one bounded retry before withdrawal. They remain suitable for
routine validation. A TLC counterexample that changes implementation behavior must
become a deterministic Rust regression before the model or implementation is
corrected. The models are not refinement proofs and must not be weakened to
accept a known Rust shortcut.

Specula is an optional development audit, not part of the build or installed
session. Its commit pin, narrow runner, retained findings, and artifact policy
live under `validation/specula`.

### Complementary Architecture Models

The bounded Alloy and SMT-LIB2 gate complements TLA+ without translating or
duplicating the temporal models. Alloy checks finite authority,
protection-domain composition, action-capability, policy-operation binding,
and target topologies. Z3 checks target geometry/disclosure arithmetic,
policy presentation geometry, and consumes
schema-generated `sophia_wm_v1` widths and maxima for wire-bound proofs.
Every protected query is paired with a retained negative control that must
produce a counterexample or satisfiable witness.

The model inventory, scopes, correspondence, proof limits, official Alloy
archive hash, and optional Z3 5.x differential are documented under
`validation/architecture`. The stable unattended gate requires Alloy 6.2.0 and
Z3 4.16.0 and performs no network access:

```sh
SOPHIA_ALLOY_ARCHIVE=/absolute/path/to/alloy-6.2.0-linux-amd64.tar.gz \
  tools/check_architecture_models.sh
```

These models are bounded decision evidence, not Rust refinement proofs. The
target models remain pre-schema; their symbolic count, precision, and rate
budgets are not wire constants. Spin/Promela, dependency-policy automation,
and fuzzing remain candidates until they have retained models or corpora,
expected outcomes, and reproducible gates.

### Public Policy Wire

The draft `sophia_wm_v1` wire has one checked-in KDL schema and retained Rust,
C99, documentation, and golden-corpus outputs. Normal builds do not run the
generator. The gate first checks those outputs for byte-for-byte drift, then
runs the Rust codec and an independently compiled, allocation-free C99 codec
against the same valid and malformed frames. It then drives a standalone C99
client through the authenticated session transport and Engine reducer:

```sh
tools/check_policy_protocol.sh
cargo test --offline -q -p sophia-protocol --test policy_semantics
cargo test --offline -q -p sophia-runtime --test policy_ipc
cargo test --offline -q -p sophia-runtime --test policy_socket
cargo test --offline -q -p sophia-runtime --test policy_transport
cargo test --offline -q -p sophia-engine --test policy_projection
cargo test --offline -q -p sophia-wm-demo --test policy_v1
cargo test --offline -q -p sophia-x11-wm-bridge --test policy_projection_adapter
```

The first command proves generated envelope and record layouts across Rust and
C99. The focused Rust gates prove exact supervised-peer admission, negotiation,
bounded begin/chunk/end assembly, late-epoch discard, semantic record
conversion, atomic multi-output validation, and last-layout preservation. The
Rust reference client and generic X11 bridge then prove their policy output
through the same reducer. The xmonad compatibility launcher uses only this
public path; the removed compatibility transport cannot receive a downgrade.

`tools/check_policy_protocol.sh` additionally runs the Rust reference,
independent C, and immutable archived revision-3 C clients through one
authenticated, eleven-cycle revision-3
behavior corpus. The retained connection observes constrained single-output
layout, two-output partitioning, output loss with surface migration, and the
same raw output returning at a new generation, then an ordered focus action, a
timed-out candidate, a stale candidate superseded by a newer scene, an invalid
candidate, and a successful recovery after each rejection. Committed replies
must pass the canonical reducer without losing an assigned surface or changing
the declared active output; rejected work must not poison later cycles. Each
client also runs the corpus across two supervised processes and fresh
connection epochs; the host pins the last committed projection across the
replacement boundary. Hagia's check below runs both exact sequences.

The authenticated black-box host covers four direct `sophia_wm_v1` peers:
Rust, C, Hagia, and the configured xmonad compatibility bridge. The xmonad gate
launches the real checked-in xmonad configuration behind its private synthetic
X server, consumes the same eleven scenes, and uses five fresh processes: one
normal replacement plus timeout, stale, and invalid replacement/recovery
boundaries. `tools/check_archived_policy_client.sh` separately verifies fixed
digests before compiling the frozen C99 codec/client snapshot and running it
against the current server. Shared restart and archived-client freeze coverage
are therefore closed. The separately authorized physical output apply/rollback
archive described below supplies the hardware evidence.

The separate, standalone Hagia checkout verifies its independently written Nim
decoder against the same retained corpus, then runs its proof client through
the authenticated Sophia transport and canonical reducer with:

```sh
cd ~/dev/hagia
SOPHIA_STACK_ROOT=~/dev/sophia-stack tools/check_sophia_policy.sh
```

The focused live recovery gate runs real Kitty under the public session path,
terminates the first supervised Hagia process, admits only its replacement,
and requires epoch advancement, startup readiness, retained layout, and clean
session/layout health:

`tools/hagia_client_lifecycle_fault_smoke.sh` applies the same replacement
requirements to explicitly armed post-negotiation and complete-snapshot client
faults. Session-operation client faults remain outside that gate until a
deterministic opaque-operation activation driver is retained.

```sh
SOPHIA_HAGIA_BIN=~/dev/hagia/hagia tools/hagia_live_session_smoke.sh
```

This is bounded offline integration evidence. The installed physical output
apply/rollback gate below supplies the separate freeze evidence.

The dynamic-output physical gate is separately armed because it takes
exclusive DRM/input ownership and asks the operator to disconnect and reconnect
one of the connected outputs:

```sh
tools/run_output_topology_gate_tty4.sh
```

Run it from `/dev/tty4` with at least two connected physical outputs. It supplies
the arm and `seat0` defaults, builds Hagia from the adjacent clean signed
checkout so its policy wire matches current Sophia, builds the clean signed
Sophia revision, and preserves timestamped evidence under `/tmp`. Environment
variables remain available for nonstandard rigs. The gate requires one
security-epoch barrier per change, complete `N - 1` loss and `N` return
publications with advancing generations, matching policy settlements, later
page-flip retirements, a surviving Kitty input proof, and clean
non-quarantined shutdown.

Revision 3's final output-authority proof is a separate two-phase TTY4 gate:

```sh
SOPHIA_FRAME_FED_OUTPUT_ARM=1 tools/run_frame_fed_output_gate_tty4.sh
```

It is reference-rig-specific and refuses anything except connected `DP-1`
2560×1440 and `DP-2` 1920×1080. Before taking DRM or input ownership it requires
clean, signed Sophia and Hagia HEADs equal to their locally known
`origin/master`, then builds and hashes the exact release binaries. The first
normal public-Hagia session applies, first-presents, and publishes the checked-in
profile. The second applies the same startup candidate and forces reverse-card
rollback after final KMS acceptance but before candidate installation. Both
require distinct physical text confirmation and clean teardown. A verified pair
is archived under
`$XDG_STATE_HOME/sophia/promotion/frame-fed-output-runs/`; duplicate evidence,
configuration outside the signed commit, identity drift, forbidden rollback
publication, or checksum drift is refused. This gate changes real output state
and must not be run without explicit operator authorization.

The retained run is frame-fed archive `0001`. It binds Sophia
`870ba46ae231081220b982ecc3a5a95517df7a90`, Hagia
`a83c8fa022a4ceff5d8b96a01c46052bbd8ba64a`, success evidence
`7dbcc54326d48168df930edf88d81f5cf64fb64251f3b2a9b150e159a37431e5`, and
rollback evidence
`267f8b11cc3de692708ee4c634efe6a09b6eb31da992483566e3ba520114f69d`.
Independent archive verification reports `status=passed`, boundary
`after_apply`, and two phases. This closes the hardware gate for stable
interface major 1, wire revision 3.

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
cargo run --offline -q -p sophia-cli --features native-session -- live-session-composition-smoke
cargo run --offline -q -p sophia-cli --features native-session -- session run --proof --terminal=xterm
cargo run --offline -q -p sophia-cli --features native-session -- session run --display=:177 --max-runtime-ms=6000 --inject-text=sophia
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
visual detail and route Copy Idle-before-Complete after release. It also separates
an update-only replacement from a later patch, bounds the recent update
working set at 16 handles, and requires post-reduction committed surfaces to
retain a materialized renderer buffer:

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
cadence from routed displayed Present timestamps rather than process wall time. It
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

The wrapper retains and retries up to eight page-flip stalls only when schema-2
attribution proves the completion event never crossed the card descriptor:
the poller must be empty, routed, last read `WouldBlock`, and report zero decoded
or rejected callbacks. Every failed attempt remains under `attempt-NNN/`; only
the final attempt is promoted to the archive root and evaluated. A pending or
rejected callback, another benchmark failure, or exhaustion of the bounded
budget fails immediately. `SOPHIA_TERMINAL_MAX_STALL_RETRIES` may lower the
budget or raise it no higher than 32.

The trailing `sophia_terminal_performance schema=4` report retains those
resource, patch, damage, client-metadata, failure, drain, and composition-budget
checks and additionally requires exactly one
`sophia_live_cpu_visual_progress schema=1 status=complete` record. Post-readiness
updates must balance exactly as presented plus superseded with zero pending or
discarded updates. At least three content-changing primary retirements must
occur; the first update and first changed retirement must follow readiness
within one second; source and display gaps may not exceed one second; and the
last update and last changed retirement must precede completion by no more than
one second. The largest accepted-update-to-exact-retirement latency may not
exceed two periods of the session's reported refresh plus one millisecond of
integer tolerance. A startup-only burst cannot pass.

The default composition budget remains 25 ms;
`SOPHIA_TERMINAL_COMPOSE_BUDGET_MSEC` accepts only a positive integer and is
reserved for a separately documented gate. The raw report can be regenerated
from the retained standalone session log:

```sh
tools/report_sophia_terminal_performance.sh
```

If the machine locks or the report fails, retain the standalone session,
launcher, input-guard, recovery, lifecycle, and protected kernel logs. The
wrapper may repeat only the attributed below-process stall above; do not repeat
any other failed physical takeover until its evidence is diagnosed. This
benchmark does not establish Xserver parity. Optional X Present cadence remains a
diagnostic; the gate's screen-progress authority is the session's exact primary
composition and KMS-retirement evidence.

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
context creation, DRI3 import, and Present submission. Move the pointer over
the centered window and confirm that the three gears remain smooth, then let
the bounded client end the session automatically; Super-Shift-Q intentionally
preempts the benchmark completion. The trailing
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

Milestone 14's broader same-hardware comparison is a separate diagnostic matrix
owned by typed conformance code:

```sh
cargo xtask conformance desktop-comparison prepare RUN KERNEL MESA GPU
cargo xtask conformance desktop-comparison run RUN SAMPLE_LOG
cargo xtask conformance desktop-comparison verify RUN
cargo xtask conformance desktop-comparison report RUN
```

Preparation refuses a dirty or unsigned Sophia candidate. It hashes the
repository-owned stack configurations and local Firefox fixture, pins the
candidate and reference-stack identities, and records the common two-output
topology plus operator-supplied kernel, Mesa, and GPU identities. The schedule
rotates Sophia, XLibre+xmonad, and niri across three repetitions of Kitty 60 s,
the offline Firefox fixture, resize, and a 16-Kitty launch burst, then requires
one two-hour soak for each stack: 39 raw samples total.

Each native-stack adapter emits exactly one
`desktop_comparison_sample schema=1 status=complete` record. `run` verifies its
schedule position, backend, topology, executable versions, duration, resource
and frame populations, crash count, and sample-loss count before copying the
raw log under a deterministic path and extending its checksum ledger. `verify`
requires the exact complete matrix; `report` emits stack/workload means with
`verdict=none`. Reference performance is never a Sophia correctness threshold.
The repository-owned inputs and adapter boundary are documented in
`validation/desktop-comparison/README.md`.

For the visible xmonad/KMS proof, run
`tools/start_sophia_xmonad_vkcube_recovery_tty3.sh`, launch
`vkcube --wsi xcb`, exit normally, then run:

```sh
tools/verify_sophia_xmonad_vkcube_recovery.sh
```

The verifier selects an armed visual admission only when its exact transaction
and surface later produce a schema-4 software retirement. Unrelated imported
DMA-BUF admissions, including the startup Kitty, cannot satisfy this recovery
proof. Before the admission may arm, its transaction, surface, and target
buffer must appear as the Engine-selected `PresentedBuffer` candidate with a
concrete natural extent. The target identity may originate in imported DMA-BUF
storage or an immutable CPU materialization, but it may not bypass retirement
as a backing snapshot.

At least three increasing software Present transactions must carry nonzero
UST/MSC and exact native frame/submission ownership. The native submission must
be an independent CPU or retained-mixed frame, so feedback cannot steal a newer
DMA Present frame. Diagnostic logs join each transaction to routed Complete and
Idle events. Ordinary installed-session logs suppress those verbose lines; for
them, the verifier requires aggregate Copy, Idle, and idle-fence counts to cover
every exact software retirement. These checks reject the historical
backing-clear substitution, mixed-batch registration omission, callback and
successor confusion, and static first-frame success. The verifier also requires
bounded clean teardown and rejects malformed, overflowed, mismatched, or
degraded intake.

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
submissions, callbacks, retirements, exports, logical-content checksums, and a
page-flip-paced vsync record with zero overlap or phase rejection. Checksums are
diagnostic content evidence, not output identity: unrelated outputs may show
identical content. Two distinct output identities are what the gate counts,
because the per-output completion record is emitted once per head -- a mirror
group would otherwise satisfy a two-output claim with one output. Physical
heads in one mirror group carry the same scene
generation and logical-content checksum; optional native head-pixel checksums
may differ. Connector-qualified lifecycle records prove that both scanout chains
presented the cohort.
It boots an isolated direct-kernel initramfs with virtio-gpu and verifies
exactly 300 session ticks without host DRM, input-device, VT, disk, or guest
network access. The QEMU evidence verifier also rejects native submit/retire
failure, rejected callbacks, in-flight ownership, cleanup debt, and a tick
count other than 300. QEMU does not claim VRR because virtio-gpu does not expose
the physical property contract. Keep the physical TTY proof for the AMD
multi-connector/VRR gates and operator-typed input evidence.

For exploratory interaction, use the dedicated one-shot guest instead of the
acceptance soak:

```sh
tools/qemu_xmonad_interactive.sh
```

Run it from a graphical terminal. It rebuilds the isolated M8 image, opens its
Unix-domain VNC display, and leaves the terminal, Vulkan, Firefox, launcher,
xmonad, and two-output session under operator control. It has no proof clock,
scheduled bridge restart, automated input, disk, network, host DRM access, or
host VT access. Use the session freely and finish with Super-Shift-Q; closing
the viewer does not impersonate a clean logout. Set
`SOPHIA_QEMU_INTERACTIVE_VIEWER=none` only when a viewer will attach from
another graphical session; the harness prints the private socket path.

The interactive path enables QEMU's VNC and input-core tracepoints only into a
FIFO. `tools/reduce_qemu_interactive_trace.sh` discards their values and
retains boundary-kind markers and bounded keyboard-count checkpoints. Q35's
legacy `vmmouse` path is disabled so the viewer and guest share the declared
relative virtio mouse. The final verifier correlates those host markers with
virtio discovery, Engine key/button/motion intake and routing,
focused-client and output projection, a manually launched terminal, later
typed input, a focus change, close, normal logout, native drain, and clean
resource release. It rejects raw trace names so keycodes, pointer coordinates,
and button values cannot enter retained evidence.

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

The compatibility-bridge regression also retains a synthetic window across an
unmap, admits a new surface on the next workspace, and deliberately emits late
geometry for both. Translation must produce configure, render, and focus
commands only for the mapped window. The live admission reducer separately
primes a new policy-managed surface with its strongest complete observed pixel
extent, keeps the blind WM's different size as a standing target, and releases
that temporary constraint only through the ordinary visual-retirement path.
These checks prevent a private legacy layout from resizing hidden clients and
prevent a first-launch extent mismatch from consuming the timeout/restart path.

The unattended launch-capacity gate is:

```sh
tools/qemu_xmonad_launch_burst_acceptance.sh
```

Its isolated profile keeps one visible startup Xterm and preloads twelve
managed `sleep` children, leaving four slots in the active-plus-pending limit.
The harness sends 32 Super-Enter chords through one QMP connection. Because the
WM transport may validly coalesce an identical action while one request is in
flight, the verifier requires exactly four burst admissions, 20 through 28
capacity rejections, and balanced committed launch actions rather than an
exact rejection count. It then waits for a managed child to release capacity,
requires one recovery launch and its X11 focus acknowledgment, commits a
Super-J focus transition, and logs out normally. Acceptance also requires two
unique startup-output baselines, asynchronous native retirement, zero admission
timeouts or WM restarts, clean session health, and drained native and
application ownership. `tools/check_qemu_xmonad_launch_burst_verifier.sh`
mutation-tests this evidence contract and runs from the local regression gate.

The short-lived-surface recovery gate is:

```sh
tools/qemu_xmonad_stale_response_acceptance.sh
```

Its isolated profile keeps two persistent Xterms while a Super-Enter action
maps an Xterm whose child exits inside the xmonad bridge's response quiet
period. The verifier requires the action surface to be observed before normal
exit, rejects the resulting stale `ManageSurface` reply, and requires exactly
one transport restart plus a committed-layout reseed. Both persistent surfaces
must survive, a post-restart Super-J focus transition must reach X11, and
normal logout must drain application, transport, input, presentation, and
native ownership. `UnknownSurface`, a second restart, incorrect stale-response
accounting, or missing causal ordering fails the gate. The mutation checker
`tools/check_qemu_xmonad_stale_response_verifier.sh` runs from the local
regression gate.

The unattended resize-under-render gate is:

```sh
tools/qemu_xmonad_resize_storm_acceptance.sh
```

Its isolated profile continuously redraws one CPU/SHM Xterm while the session
applies 12 bounded policy resizes. A later target cannot start until the prior
transaction has delivered one configure and committed matching geometry and
pixels. The verifier orders each request, blind-WM layout commit, resize-epoch
commit, and exact-pixel completion; it rejects rollback, timeout, mismatched
dimensions, or authority loss. A partial-damage page flip after the final
resize proves that rendering did not stall. Normal logout must balance every
renderer-worker request and completion, drain native presentation, and leave no
WM, application, snapshot, import-cache, or cleanup debt. This gate covers the
software-present resize path; it does not claim concurrent DMA-BUF coverage.
`tools/check_qemu_xmonad_resize_storm_verifier.sh` mutation-tests the evidence
contract and runs from the local regression gate.

The unattended single-output DMA-BUF contention gate is:

```sh
tools/qemu_xmonad_render_contention_acceptance.sh
```

This host-render-node profile uses virgl to give three unmodified `glxgears`
clients real DRI3 DMA-BUFs. Sophia's bounded application-admission FIFO starts
the second and third clients only after the preceding client is stable; the
measured window begins only after all three producers and one unmodified xmobar
are present. The verifier requires at least 30 exact retirements from each
producer with no more than two frames of service skew. It also requires
exactly one active output, active CPU patch composition, positive import-cache
traffic, balanced renderer-worker ownership, a 100 ms worker-request bound, a
balanced 100 ms frontend-control bound, monotonic Present cadence, and clean
layout, native, protocol, and process teardown.

This is deliberately a single-active-output proof. QEMU still exposes two
connected outputs and both retain valid completion records, but only one must
carry the bounded workload. The gate must not be cited as inter-output fairness
evidence until producers are assigned to both outputs of one render-device
group. `tools/check_qemu_xmonad_render_contention_verifier.sh` mutation-tests
the evidence contract and runs from the local regression gate.

The unattended idle and partial-damage efficiency gate is:

```sh
tools/qemu_xmonad_idle_efficiency_acceptance.sh
```

This explicit-host-render-node profile runs a static CPU/SHM Xterm beside a
real virgl `glxgears` DMA-BUF producer, freezes the producer after at least ten
retirements, and waits for one second of stable producer state. It then sends
256 Super-J focus transitions. Every transition must commit one physical
action and produce a partial, page-flip-retired `RetainedMixed` submission from
the cached client image, with no later client Present and no CPU submission in
the marked reuse window. A subsequent two-second idle window must contain no
repaint, page flip, or client Present.

Completion requires more import-cache hits than imports, exact final eviction,
balanced renderer-worker ownership, bounded worker and frontend-control
latency, one active output plus one baseline-only output, a two- or three-frame
startup upload baseline, and clean native, application, protocol, and cache
teardown. The upload total is deliberately bounded rather than exact because
the initial static Xterm frame may coalesce with startup; the causal reuse
window independently rejects every non-retained submission. Run
`tools/check_qemu_xmonad_idle_efficiency_verifier.sh` to mutation-test the
evidence contract. Both checks run from the local regression gate.

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
the first application surface. Sophia queries the DRM driver's cursor-width
and cursor-height capabilities, allocates its canonical X11 `left_ptr` at that
size, detaches inherited atomic cursor-plane state once, and then uses the
legacy `set_cursor2`/`move_cursor` ioctls for steady motion. This deliberately
keeps cursor motion out of the primary-plane atomic page-flip queue. The future
all-atomic path must combine primary and cursor state under one per-output KMS
transaction owner; independent cursor-only atomic commits are not a supported
fallback. Move the cursor across both outputs, click and drag to select
terminal text, type into Kitty, and close Kitty normally. xmonad and
Super-Enter remain outside this gate.
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
the generic bridge as `sophia session run`'s supervised WM process and uses
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
`sophia_live_wm_transport schema=2 status=complete` record. The bounded
transport must drain with zero pending or capacity-rejected requests, peak at
no more than sixteen owner entries, reject no more than sixteen stale
responses, report at least one ordered physical action, report zero coalesced
physical actions, and keep queue dwell and socket round trip within the
external policy timeout of 500 ms. Repeated opaque action tokens remain
distinct FIFO activations and are rebuilt against the latest committed state
when they reach the transport head. Capacity saturation consumes the shortcut,
fails closed, and emits at most sixteen per-event diagnostics; the completion
record retains the saturating rejection count. The socket round trip is
intentionally not charged to the owner-thread 100 ms budget: the typed worker
waits while the owner continues input, rendering, and page-flip service.

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
The M8 browser scroll stage uses Firefox's native XI2 path. The deterministic
page first performs a real local-document navigation, then requires two
vertical DOM wheel events and nonzero document displacement before it advances.
The gate orders two newly routed axis packets between PRIMARY and scroll; there
is no keyboard fallback.

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

Use the focused source-tree slices while diagnosing one Firefox boundary. They
run the same Sophia/X-authority path and isolated Firefox profile as promotion,
but stop as soon as the affected contract is proved:

```sh
# One Kitty, one Firefox, and no page interaction. This is the default canary.
tools/start_sophia_xmonad_firefox_rendering_tty3.sh
tools/verify_sophia_firefox_rendering_physical.sh

# One Kitty, one Firefox, and two trusted clicks through the DOM modal.
tools/start_sophia_xmonad_firefox_dialog_tty3.sh
tools/verify_sophia_firefox_dialog_physical.sh

# One Kitty, one Firefox, and only the two remaining PRIMARY transfers.
tools/start_sophia_xmonad_firefox_primary_tty3.sh
tools/verify_sophia_firefox_primary_physical.sh

# Legacy combined selection diagnostic; not required after CLIPBOARD passes.
tools/start_sophia_xmonad_firefox_selection_tty3.sh
tools/verify_sophia_firefox_selection_physical.sh

# Two Kitty windows, normal Firefox close, restart, and WM-forced close.
tools/start_sophia_xmonad_firefox_lifecycle_tty3.sh
tools/verify_sophia_firefox_lifecycle_physical.sh
```

The rendering canary requires only `Super+F`, a visual check, and
`Super+Shift+Q`. Its verifier binds the action-created Firefox surface to a
complete full-height left-column retirement. Direct admission passes; fallback
admission must clear its temporary extent through the exact standing target
and may restart the WM at most once. It contains no clipboard, PRIMARY,
navigation, resize, focus-cycle, or dialog steps.

The dialog canary starts only after Firefox has a complete full-height frame.
Click the full-page `Open proof dialog` button and then `Confirm Sophia dialog`.
Its verifier orders an independently routed pointer batch and a complete native
Firefox retirement around each redacted DOM checkpoint. It also rejects a new
X11 toplevel, post-admission WM restart, layout timeout, recovery extent, short
clip, GDK freeze error, or dirty teardown. The `<dialog>` remains part of the
existing Firefox toplevel; genuine X11 transient/floating windows retain their
separate wire and policy regressions.

Cross-window `CLIPBOARD` has already passed on physical hardware. Do not rerun
the combined selection slice for rendering changes or PRIMARY closure. The
PRIMARY-only slice starts at the Firefox source token, requires a trusted full-
field pointer selection, and accepts only exact direction-specific tokens. Its
three checkpoints begin only once the source is armed, then record Kitty's
exact-token receipt and Firefox's exact-token confirmation. Its verifier orders
one owner change and one conversion inside each peer interval and rejects any
replayed CLIPBOARD checkpoint. The lifecycle slice
removes clipboard, scroll, resize, refocus, and dialog work; its verifier still
requires two independent Kitty processes, two status-zero Firefox exits, an
ordered `CloseFocused`, clean health, and clean frontend teardown. That slice
passed physically on 2026-08-03 with all six Kitty checkpoints, both status-zero
Firefox exits, and clean session/layout/frontend teardown. These slices are
diagnostic evidence, not promotion runs.

After the affected slice passes and the normal xmonad input gate remains clean,
run the content-redacted physical Firefox promotion workflow from TTY3:

```sh
tools/start_sophia_xmonad_firefox_proof_tty3.sh
```

The promotion launcher uses the offline fixture and keeps the current
instruction visible inside the active window; the operator does not need to
memorize a sequence before graphics takeover. It creates a run-local Firefox
profile and uses the same native-X, single-process, XI2 configuration as the
passing QEMU workload,
so the proof neither inherits the operator's normal Firefox profile nor changes
it. Kitty A and Kitty B each accept three short,
content-redacted checkpoints before Firefox, after its normal `Ctrl+Q` exit,
and after its restarted window is closed through xmonad. The Firefox page
requires keyboard input, real navigation followed by document scrolling,
a layout change, focus-away/focus-return, and a pointer-opened dialog.
`CLIPBOARD` and `PRIMARY` are excluded because their focused physical gates and
bidirectional wire regressions are authoritative. The strict verifier rejects
replayed selection stages, orders the six Kitty checkpoints around the two
status-zero Firefox exits, requires replacement-document readiness followed by
at least one routed axis packet before DOM scroll completion, and rejects pending
input/actions, protocol errors, native/frontend/authority cleanup debt, or
retained temporary layout constraints or standing-target debt. Its layout
checkpoint requires exactly one Super+Space action, a committed layout with
nonzero movement, exact retirement for every affected surface, a subsequent
Present from the launch-correlated Firefox surface, and all three managed
surfaces remaining visible. The configured first cycle is `ThreeColMid` to
`Tall`, matching the user's established policy; the proof deliberately accepts
an outer position or size change instead of requiring Firefox itself to resize.
The real-xmonad smoke locks that order down. Its dialog checkpoint
requires popup-document readiness, a five-surface layout snapshot,
confirmation, and a return to the four-surface physical xmobar baseline before
Firefox exits.
During resize, a descendant Present may be smaller than its policy-managed
outer surface only when X Authority reports that exact child content extent.
The renderer applies the accumulated child offset and clips, but never scales;
native retirement must match the content extent before it commits the outer
layout extent. A click on a
client-positioned Firefox render child must hand focus to its containing
policy-managed Firefox surface while the X event still reaches the selected
descendant window. A `WM_TRANSIENT_FOR` popup remains attached to that managed
owner, follows its workspace visibility, and must disappear from composition
on unmap or owner removal without entering blind-WM admission.
Verify and retain the run with:

```sh
tools/verify_sophia_firefox_physical.sh
tools/record_sophia_firefox_physical_run.sh
tools/verify_sophia_firefox_physical_runs.sh 1
```

The installed artifact exposes the same workflow as the `Sophia Firefox Proof`
greetd entry. That entry reserves an immutable Firefox attempt before graphics
takeover, archives it under `sophia/promotion/firefox-runs/`, and applies the
dedicated Firefox and lifecycle verifiers on exit. Use
`sophia-verify-firefox-runs 1` to verify the newest archive. The compatibility
command `sophia-record-firefox-run` remains available for a source-tree run
that was not launched through the installed entry. Both paths verify immutable
release digests and privacy-safe runtime identities in addition to the browser,
WM, input, renderer, and teardown evidence.

The installed xterm/work-area gate stays out of the greetd menu. Run
`sophia-xterm-proof` from a local text VT, switch away and back after xterm is
visible, and log out with `Super+Shift+Q`. The wrapper records a schema-4
`record_kind=xterm` archive under `sophia/promotion/xterm-runs/`. Its verifier
binds xterm's executable identity to the run and requires two reduced work
areas, an atomic CPU-snapshot geometry commit inside the primary work area,
ordered zero-image handoff, retained-scene rehydration, post-resume native
retirement, normal logout, clean X11 and process ownership, and exact TTY
restoration. Verify the newest archive with
`sophia-verify-xterm-runs 1`.

The retained xterm archive also carries the focused status-bar boundary.
`sophia-verify-xmobar-work-area` requires its immutable checksums, exact
14-pixel reservations on both outputs, at least three exact 2560-by-14 primary
repaints, matching native retirement, packaged xmobar identity, normal logout,
and clean recovery. It does not require another operator sequence.

The installed physical color gate also stays out of the greetd menu. Run
`sophia-truecolor-proof` from a local text VT, wait for the Kitty ANSI sample
and asymmetric X11 palette, then use `Super+Shift+Q`. Its automatic
`record_kind=truecolor` archive requires an exact core X11 color and image
round trip, exact per-channel populations from the final primary-output palette
rectangle, a chromatic final Kitty DMA-BUF rectangle, matching output-1 KMS
retirements, the independent output-2 startup baseline, clean protocol and
process ownership, and exact TTY restoration. This gate does not claim active
cross-output client projection, which remains outside the current classical-WM
compatibility boundary.
Reverify it with `sophia-verify-truecolor-runs 1`. Repeated final-region
readback is scoped to this explicit proof environment and is not enabled for
ordinary sessions.

Run-set verification treats the checksummed evidence as the durable input. An
exit-zero archive whose original result is exactly
`reason=session_verification` may pass under a corrected current verifier; the
archive remains unchanged, and the result reports `reverified=1`. Any nonzero
session exit, different failure reason, checksum change, or current semantic
failure remains ineligible. This permits verifier repair without manufacturing
another physical sequence or weakening the fail-closed evidence contract.

## Installed Daily-Driver Candidate

The packaged [installed operations runbook](operations.md) is the canonical
reference for the supported host boundary, status and log locations, normal
stop, independent recovery, fallback login, and rollback.

An installed release is built and frozen separately from login:

```sh
tools/install_live_session.sh
```

With no argument, this one-shot command resolves the current Git commit,
packages it when necessary, verifies the exact artifact and every digest,
requests privilege only for the system installation, and verifies the installed
commit. An explicit artifact directory remains available for staged tests and
recovery tooling. Packaging refuses a dirty worktree, builds optimized Sophia,
the generic WM bridge, and the native WM, resolves the tested xmonad binary,
and records SHA-256 digests plus the exact Git commit in an immutable release
directory. Installation verifies
those digests, copies the release below `/opt/sophia/releases/`, atomically
updates `/opt/sophia/current`, preserves the former target as
`/opt/sophia/previous`, and installs the Sophia greetd session entries below
`/usr/share/wayland-sessions/`, the directory configured for greetd/tuigreet
session discovery.

The installed Kitty fallback is `Sophia Kitty (Baseline)`; the integrated
policy candidate is `Sophia xmonad (Experimental)`. The bounded containment
gate is `Sophia Recovery Proof`, and `Sophia Native Chrome Proof` is the
automated physical ring/frame gate. All four enter through versioned commands
below `/opt/sophia/current`; the ordinary sessions ultimately use
`sophia-session`, which performs no
source build, repository lookup, display-manager takeover, or privileged
service control. It emits the installed version and commit before entering the
same guarded session lifecycle. It fails closed unless greetd supplies
an existing, absolute, user-owned `XDG_RUNTIME_DIR` and a real local Linux VT.
The lifecycle log records ordered preflight, input-guard, graphics-takeover,
session, and bounded display-manager-handoff phases without application
content. A failed launch adds exactly one `sophia_session_diagnostic` record
with the installed version and commit, exit status, and exact `preflight`,
`input_guard`, `graphics_takeover`, `session`, or `handoff` phase. It contains
no command arguments, application identity, typed content, or window metadata.
`sophia-status` prints that record once alongside the verified current manifest
and final lifecycle outcome. Inspect the latest phase or roll back the
installation with:

```sh
sophia-status
sophia-stop
sudo sophia-rollback
```

`sophia-stop` requests bounded cleanup from an independently logged-in control
TTY; Ctrl-Alt-Backspace remains the independent local emergency chord.
Neither mechanism imposes a lifetime on a healthy desktop. `Sophia Recovery
Proof` is the only installed entry that opts into the outer launcher's
45-second wall-clock deadline. The deadline process remains outside Sophia's
process group; when it expires, it terminates that group and leaves the outer
launcher to restore the saved keyboard, KD, and terminal modes before greetd
resumes.

One installed watchdog proof is sufficient for a release. Select `Sophia
Recovery Proof` in greetd, press and release Ctrl-Alt-Backspace once when asked
to arm the guard, and then leave the visible xmonad session running. Greetd
must return automatically after 45 seconds. Log into `Sophia Kitty (Baseline)`
once, exercise physical keyboard input, and exit Kitty normally. That entry
automatically retains its own attempt. Verify the fallback and the
automatically retained watchdog attempt with:

```sh
sophia-verify-fallback
sophia-verify-watchdog
```

The recovery entry reserves its ledger slot before takeover and finalizes it
after the status-124 handoff. Its verifier requires visible startup readiness,
exactly one process-group deadline, an armed but untriggered local input guard,
exact TTY restoration, the installed release identity and digests, and a
display-manager handoff with status 124. Watchdog attempts remain separate;
they never count as normal login cycles or as graceful Ctrl-Alt-Backspace
recovery.
Because the graphical owner deliberately places the kernel keyboard in
off-mode, Sophia recognizes Ctrl-Alt-F1 through Ctrl-Alt-F12 and explicitly
requests the target Linux VT through libseat. Seat disable pauses physical
input, drains KMS ownership, and preserves the X clients and Engine scene;
seat enable reacquires devices and repaints retained content. Shifted US
punctuation is covered by the
same `evdev`/`pc105`/`us` XKB state used for client-visible core and XKB events.

The artifact also installs repository-independent evidence commands:

```sh
# Check only the current installed login/startup/logout cycle:
sophia-verify-login-cycle

# After one separate Ctrl-Alt-Backspace recovery run:
sophia-verify-emergency

# After the one dedicated Sophia Recovery Proof run:
sophia-verify-watchdog

# Require the latest three recorded runs to be clean and from one commit:
sophia-verify-cycles 3

# Reverify a retained three-run gate after later recovery attempts:
sophia-verify-cycles 3 0005

# Verify the latest automatic Kitty fallback attempt:
sophia-verify-fallback

# Verify the latest automatic native ring/frame proof:
sophia-verify-native-chrome

# Verify the latest installed Hagia session or exact emergency recovery. This
# has no duration or action-count threshold:
sophia-verify-hagia

# Show identities, rollback target, outcomes, and cumulative scenario coverage:
sophia-status

# Run and then reverify ten clean installed lifecycle cycles:
sophia-run-cycles
sophia-verify-cycles 10

# Historical xmonad soak artifacts remain reproducible through the explicit
# archive policy and are not current promotion gates:
sophia-verify-soak
```

The raw current-candidate soak verifier defaults to the `current` sample policy:

```sh
tools/verify_installed_session_soak.sh SESSION_LOG 7200000 10 5 current
```

Current evidence requires exactly one nonsaturated five-second resource series,
at least 120 samples, an exact declared sample count, contiguous sequence numbers
starting at one, and advancing uptime. After dropping the warmup quarter, the
later settled half may not exceed the earlier half's peak for CPU registry
buffers/bytes, native frame slots, snapshots, or imported-image cache entries.
RSS alone has a 64 MiB allocator-arena tolerance. Totals such as COW splits may
rise and are not treated as resident-resource leaks. The two-hour duration and
ordinary lifecycle/application/action gates still apply.

`tools/verify_installed_soak_archive.sh` passes `archive` explicitly so an old
checksummed run that predates resource sampling remains reproducible. Archive
policy permits an absent series but applies the complete current rules when a
series is present; neither an archive nor a synthetic fixture is fresh milestone
evidence.

The opt-in checkpoint/restart and state-transition proof runs from a logged-in
TTY 4 and restores the originating display manager and VT on every exit:

```sh
tools/hagia-proof
```

Follow its on-screen steps once and do not enter the final phrase early:
the phrase plus Enter terminates the session immediately. The proof keeps
client-bound keys ordered across asynchronous Engine/frontend focus
acknowledgement, but still rejects a
wrong routed character, retargeted sequence, stale surface, or missing physical
presentation. A restart line or a returned TTY alone is partial evidence; the
command must print `Hagia physical policy gate passed`.

Every passing run is rechecked by
`tools/verify_hagia_policy_physical.sh` and archived under
`${XDG_STATE_HOME:-$HOME/.local/state}/sophia/promotion/hagia-policy-runs` with
both source commits, exact Sophia, Hagia, and Hagia Shell executable digests,
the immutable raw log, and checksums. The verifier also requires the protected
switcher lifecycle on both sides of its fresh-epoch restart.
`tools/archive_hagia_policy_physical_run.sh` rejects a duplicate evidence digest
instead of creating a second proof record.

The native session proof runs the bounded product workflow -- three terminal
launches, a visible focus-next, one close, and a normal logout -- with no xmonad
compatibility bridge in the session. It also runs from a logged-in TTY 4:

```sh
tools/hagia-native-proof
```

Unlike the switcher proof above, this one ends with a normal logout rather than
the proof phrase: type the phrase when the first screen asks, then follow the
remaining steps. The command must print `Hagia native session gate passed`.

The gate runs its session through the ordinary `hagia` runner profile, so exact
TTY restoration is recorded by the same component that performs it. Evidence is
rechecked by `tools/verify_hagia_native_session.sh` and archived under
`${XDG_STATE_HOME:-$HOME/.local/state}/sophia/promotion/hagia-native-runs` with
both source commits, all three executable digests, the digest of the desktop
profile the session actually loaded, the raw log, and checksums. Beyond ordered
projection commits, shell-role separation, bounded session-control latency,
clean health, and exact TTY recovery, the verifier requires the schema-7
frame-slot block to balance and to report no slot still leased at completion --
the evidence that promotes the three native frame slots.

Run `tools/check_hagia_native_matchers.sh` before any physical attempt. It
drives the real guide across synthesized passing evidence, deletes each required
line in turn, injects a leaked lease, a stale release, an unbalanced worker
ledger, a latency overrun, a reordered workflow, bridge activity, a misreported
profile, and an emergency exit, and requires each to be rejected for its own
stated reason.

Use `Sophia Hagia (Native Policy)` for ordinary development after its bounded
preflight passes. Every launch reserves a Hagia ledger entry before takeover;
normal logout passes, operator emergency return is marked recovered, and
unexpected exit or invalid health fails. The checksummed coverage summary
records scenarios actually observed without turning absent scenarios or wall
time into a failure. `sophia-record-hagia-run` binds every attempt to exact
Sophia and Hagia digests and `sophia-verify-hagia` rechecks the structural
health or emergency-recovery outcome. The legacy xmonad wrapper applies its
focused login verifier automatically through `sophia-record-run`. Before xmonad
takeover, it reserves a numbered attempt with the release and launch identities.
After handoff it finalizes that attempt with the session, guard, recovery, and
lifecycle evidence. A clean attempt requires automatic Kitty startup,
two-output readiness within eight seconds, a native retirement, normal logout,
clean protocol and session health, drained native and application state, an
untriggered guard, and exact TTY restoration. Failed and interrupted attempts
remain failed or pending ledger entries, so they cannot disappear between
later clean cycles. Each archive records the launch timestamp and a SHA-256
digest of the immutable launch identity; duplicate identities fail. The
broader xmonad and Firefox interaction proofs remain independent gates and are
not repeated merely to count a login cycle. The no-argument
`sophia-record-run` form remains a compatibility importer for a completed
unrecorded run; ordinary installed logins require no recording command.
The Kitty entry reserves and finalizes a separate immutable fallback attempt
the same way. `sophia-verify-fallback` rejects a failed or pending latest
attempt, modified archive, wrong profile or commit, external WM, missing
per-output startup presentation, no asynchronous scene retirement, absent
physical input, emergency-guard use, dirty shutdown, or incomplete VT
restoration. A damage-idle output retains its proven synchronous startup
modeset instead of issuing a redundant page flip.
The native-chrome entry packages the native WM and the guarded hot-reload
driver. It records the ordered ring-only, frame-only, and combined transitions
with the installed launch, binary, runtime, lifecycle, guard, and recovery
identities. `sophia-verify-native-chrome` rejects missing or reordered resize
and presentation boundaries, a mismatched sequence commit, absent physical
key routing, output or native debt, emergency recovery, and modified evidence.
The dedicated recovery entry likewise reserves and finalizes a watchdog
attempt automatically. `sophia-verify-watchdog` rejects an unexpected exit,
local emergency trigger, changed deadline, modified archive, wrong profile or
commit, incomplete lifecycle, or incomplete VT restoration. The no-argument
`sophia-record-watchdog-run` form remains a compatibility importer for an
unrecorded status-124 proof.
When an ordinary installed session returns status 130, its wrapper applies the
independent guard/owner recovery verifier and archives the emergency lifecycle
automatically. The pre-reserved normal attempt remains failed, so an emergency
cannot count toward consecutive clean cycles. `sophia-verify-emergency`
rechecks the separate archive, while the no-argument
`sophia-record-emergency-run` command remains a compatibility importer.
`sophia-verify-cycles` rechecks each archived
lifecycle and rejects mixed commits, modified evidence, duplicate launch
identities, any failed or pending attempt among the latest requested entries,
emergency exits, incomplete input/WM/native cleanup, or fewer than the
requested number of runs. An optional ending-run name selects that immutable
archive and its direct predecessors, retaining the same no-gap fail-closed
contract after later attempts enter the ledger.
`sophia-verify-hagia` selects the latest Hagia archive by default, rechecks its
checksums, profile, Sophia/Hagia digests, runtime identity, session or recovery
semantics, lifecycle order, and recomputed coverage. It accepts only `passed`
or `recovered`; failed and pending attempts remain visible. The retained
`sophia-verify-soak` command still validates historical xmonad artifacts and
their original thresholds but is not an active promotion requirement.
Each installed launch rotates every active reduced log to one `.previous`
generation, including the recovery log, and captures a content-free schema-2
runtime identity with the kernel and Mesa versions, the packaged Sophia
digest, Kitty/Firefox/xmonad/Hagia binary digests, a hashed input-device identity,
and connector plus EDID hashes. The schema-4 attempt manifest repeats the
Sophia digest, and every recorder and archive verifier requires it to match
the captured identity. A later installation therefore cannot erase which
binary produced an older run. The verifier rejects missing or mismatched
identities and fields that could contain typed, clipboard, or window content.

`SOPHIA_INSTALL_PREFIX`, `SOPHIA_SESSION_DIR`, and `SOPHIA_COMMAND_DIR` allow a
non-system staging installation for verifier tests. Installation alone does
did not freeze the policy protocol or authorize removal of API v7. The
deterministic reconnect/restart corpus, archived revision-3 client, applicable
bounded physical scenarios, and zero unresolved critical Hagia defects closed
those gates before removal; they did not require a wall-clock soak.

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

The buffer-age damage boundary has its own captured-pixel proof:

```sh
tools/check_buffer_age_equivalence.sh
```

It renders an identical twelve-frame mixed sequence damage-limited and
forced-full on this host's real GPU through a render node -- no DRM master, no
display takeover, safe inside a live desktop session -- and requires identical
captured checksums frame by frame, with at least one real partial repaint and
a negative control proving a lying damage table is caught. It refuses rather
than skips when no render node is writable, because the gated tests skip
silently and a skipped proof is indistinguishable from a passing one.

The two-head mirror gate is a separate destructive TTY4 proof:

```sh
tools/run_mirror_group_gate_tty4.sh
```

The runner refuses a dirty worktree or a HEAD without a valid cryptographic
commit signature before it builds. It pins xterm to the `6x13` face with white
text on black, scrolls 40 deterministic mixed-case rows, and leaves the exact
`Sophia Mirror AaZz 0123456789` marker visible for comparison on both heads.
Acceptance means identical logical content and letter case with stable
legibility on each native per-head projection; different physical modes do not
and cannot produce pixel-identical glyph rasters. The runner rechecks the clean,
signed source identity after compilation, and promotion archive verification
revalidates the recorded commit signature.
Only a status-zero session followed by the operator's exact `yes` pixel
confirmation and successful candidate verification enters
`$XDG_STATE_HOME/sophia/promotion/mirror-group-runs/`. A failed runtime or
rejected visual confirmation or evidence verification instead enters
`$XDG_STATE_HOME/sophia/diagnostics/mirror-group-runs/`; it cannot satisfy the
physical promotion verifier. The diagnostic record pins the source commit,
binary, and profile, and records the failure stage, process exit, derived
signal, and a checksum-protected kernel-log delta. The delta retains at most
256 newest lines by default. The runner first tries unprivileged `dmesg`, then
non-interactive `sudo -n dmesg`; if neither is already permitted, it records
the capture as unavailable and never prompts for new privilege. Override the
bound only with `SOPHIA_MIRROR_KERNEL_MAX_LINES` between 1 and 4096.

Passing evidence must also contain one common logical frame with connector-specific
`submitted`, `callback_accepted`, and `retired` records for both DP-1 and DP-2.
Positive aggregate counters alone are insufficient: they can describe unrelated
head generations and therefore do not prove a mirror join. That same frame must
have exactly one positive `sophia_live_mirror_head_damage` record per connector
in the connector's physical mode; any `OutputMismatch` damage rejection fails
the proof. Completed native-head schema 2 evidence names that scene generation
and its shared `logical_content_checksum`; `head_pixel_checksum` is optional and
never compared for equality. Causal no-op generations with `mode=skip` do not mask a later valid
projected generation. The runner verifies this candidate before it appends the
sole `status=passed` promotion result.

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
