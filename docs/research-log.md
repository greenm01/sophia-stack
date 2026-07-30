# Active Research Log

This file records decisions and unresolved questions for the active milestone.
Completed evidence is archived in `research-log-archive.md`.

## 2026-07-30: Unattended QEMU input-latency regression

- **Promotion coverage.** The commit-pinned Milestone 9 semantic gate now
  archives and verifies this isolated regression after its M7 and M8 scenarios.
  A candidate cannot pass gate zero with only application semantics while
  omitting the current libinput/page-flip correlation contract.
- **Local gate hygiene.** The promotion rerun exposed a stale generic-QEMU
  pass fixture and two unreviewed source-layout overages before virtualization
  began. The fixture now carries the latency and kernel-clock records, session
  configuration tests have their own domain file, and layout support is split
  from the persistent layout owner. The complete local gate passes again.
- **Host uinput is no longer required for development validation.** The QEMU
  session injects QMP keys through the guest virtio keyboard, evdev, and the
  normal threaded libinput poller. `tools/run_sophia_input_latency_qemu.sh`
  rebuilds the guest by default and retains commit-pinned evidence under the
  user's state directory.
- **No-WM admission deadlock fixed.** Presentation-intent quarantine is an
  external-WM ownership boundary. Proof sessions without a WM now commit
  policy-managed X11 pixels directly instead of waiting forever for an absent
  policy process to admit them.
- **Software scanout is correlated directly.** CPU-composed xterm frames do
  not create a GPU Present retirement record. The native head now retains the
  accepted kernel page-flip UST as well as its submission UST, allowing the
  input proof to select the changed post-ingress software frame exactly.
- **Retained result.** The isolated two-output guest reached startup readiness
  in 74 ms, routed and flushed all 14 keyboard events, matched `sophia`, proved
  the pointer path, and reported a 6 ms full chain with 8 kernel timestamps,
  zero fallbacks, and zero pending correlations. QEMU validates the clock and
  correlation plumbing; it does not replace the physical 20-sample p95 gate.

## 2026-07-30: Input-to-photon clock provenance

- **Kernel presentation timestamp preserved.** The native DRM event adapter now
  retains `PageFlipEvent::duration` as microseconds on its private callback,
  carries it beside the public callback through bounded polling, and correlates
  it by output/frame serial before presentation retirement. Production
  retirement therefore uses the DRM kernel's monotonic page-flip UST instead of
  `presentation_started.elapsed()`.
- **Fallback is observable, not silent.** Synthetic presentation timestamps
  remain available for fake and non-kernel callback sources, but production
  completion now emits `sophia_live_page_flip_clock` with kernel timestamp,
  fallback, and pending counts. The physical latency gate must require positive
  kernel timestamps with zero fallbacks and zero pending correlations.
- **Raw ingress is now separately injectable.**
  `tools/probes/uinput_text_injector.py` creates a bounded Linux virtual
  keyboard and publishes its event node so the session opens it through the
  same threaded libinput path as hardware. The poller retains a private
  per-event timing sidecar (serial, kernel event time, queue dwell); protocol
  packets remain passive and unchanged. `--inject-text` remains synthetic and
  is never counted as this proof.
- **Exact-frame correlation and gate.** The physical proof anchors on the last
  routed key press, waits for X delivery, requires a changed output frame
  submitted after that raw ingress, and computes its retirement from the
  matching kernel page-flip UST. GPU Present surfaces retain the stable-surface
  proof, while software-composed frames correlate on the native output
  submission/page-flip pair. Completion reports queue dwell,
  dwell-to-submit, submit-to-page-flip, and full-chain latency.
  `tools/run_sophia_input_latency_tty3.sh` collects 20 independent
  commit-pinned uinput/libinput samples, rejects any fallback/pending page-flip
  timestamps, and requires full-chain p95 below the configured refresh period.
  `tools/setup_sophia_uinput.sh` installs the persistent Void/udev module,
  device-node, and `input`-group policy required by that unprivileged runner.
  The code path and injector ABI self-test pass offline; physical p95 evidence
  is still required before closing the todo item.

## 2026-07-30: Terminal CPU benchmark hard-locked the machine — xterm geometry char/pixel confusion

The first physical run of the 9.4 terminal CPU-path benchmark
(`tools/benchmark_sophia_terminal_tty3.sh`) hard-locked the machine and
required a power reset. Recovered evidence from
`~/.local/state/sophia/standalone-session/` isolated two distinct problems.

- **Functional root cause (fixed).** `tools/probes/run_bounded_xterm.sh` passed
  the intended *pixel* size (`SOPHIA_XTERM_WIDTH/HEIGHT=500`) straight into
  xterm's `-geometry`, but xterm reads `-geometry` in *character cells*. With
  the default font that requested a 4004x5004 px window. `apply_text_draw`
  backs each CPU window with one immutable software buffer bounded by
  `X_AUTHORITY_SOFTWARE_BUFFER_MAX_BYTES` (64 MiB, `sophia-x-authority`
  `software.rs`); 4004x5004x4 ≈ 80 MB overran it, so `draw_text` returned
  `None`, the `ImageText8` was rejected `BadWindow`, xterm aborted (exit 83),
  and the session ended with `"live session ended without a committed external
  WM layout"` (exit 1). The X authority failed *closed* here — it refused the
  buffer rather than allocating it.
- **Deterministic offline reproduction.** Driving the committed
  `x-authority-xterm-input-smoke` at `500x500` reproduced the crash
  bit-identically (opcode 76 `X_ImageText8`, resource `0x200014`, serial `362`)
  with no KMS involved. `40x8`/`100x50`/`200x150` pass. The authority runs the
  same dispatch path offline, so this class of bug is debuggable without a
  physical takeover. Note the `sophia_terminal_performance_pass.log` fixture was
  aspirational: the benchmark had never had a real green run.
- **Fix.** The probe now converts the pixel intent to a character geometry
  against a pinned fixed-metric core font (`-fn 6x13 -b 2`) and clamps the pixel
  intent well under the cap. Default 500 → `82x38` cells → 496x498 px → 988 KB;
  the worst-case clamp (2048 px) stays at 16.7 MB. `SOPHIA_XTERM_WIDTH/HEIGHT`
  remain the reported pixel intent on the `sophia_terminal_benchmark` line.
- **Fail-closed compose budget.** The terminal performance report rejects
  `cpu_max_compose_msec` above 25 ms, matching the established
  CPU-composition gate used by the retained two-xterm and QEMU evidence. It
  records `cpu_compose_budget_msec` beside the observed maximum; malformed or
  zero overrides fail before evidence is accepted.
- **Commit-pinned physical runner.**
  `tools/run_sophia_terminal_gate_tty3.sh` refuses a dirty worktree, requires
  both persistent logging services and a nonempty kernel log before takeover,
  and archives the source commit, benchmark/report results, session/guard/TTY
  recovery, launcher handback, and the exact appended kernel-log bytes. A
  rotated log or new AMDGPU rejection/reset/timeout fails closed.
- **First post-geometry-fix physical result: bounded transport overload.** The
  run on commit `d7fbcff` passed offline preflight, armed the input guard,
  acquired both outputs, completed the synchronous output baseline, and then
  stopped the X frontend at transaction 650:
  `X authority observed transaction channel is full`. The probe's tight loop
  wrote 200 lines per iteration with no interval; a few bursts filled the
  intentional 256-batch frontend-to-owner queue before ordered visual facts
  could drain. X authority correctly refused to allocate or drop facts, xterm
  exited 84, native suspend drained, and TTY3/greetd handback completed. This
  is neither the prior geometry failure nor evidence for enlarging an
  eventually finite queue.
- **Paced workload decision.** The terminal probe now defaults to eight lines
  every 16 ms and carries both values through schema-2 benchmark/client records
  into the schema-3 performance report. The reporter rejects mismatched cadence
  or inconsistent line totals. Zero and over-one-second intervals are invalid,
  so an override cannot silently restore the unbounded producer. The gate
  runner also leaves a structured `interrupted` result and copies available
  session artifacts from its exit trap.
- **First paced physical results: rendering passed, controller completion
  fixed.** On commit `839d21a`, the first run was manually logged out at 14.3
  seconds after visually confirming the scrolling xterm and responsive pointer;
  it drained native scanout and restored TTY3/greetd cleanly but correctly had
  no 20-second client record. The next run was left for the full window:
  authority batches dropped `0`, unexpected protocol errors were `0`,
  `cpu_max_compose_msec=6`, native failures were `0`, the kernel delta was
  complete and clean, and handback was clean. It still failed the report
  because xterm backpressure held the producer inside `seq(1)` past its
  wall-clock loop test; the outer 25-second safety timeout ended xterm before
  the producer's final-only count write. The probe controller now runs the
  producer under an independent bounded timer and records each completed burst
  incrementally. A stalled-pty offline regression exercises that path. A real
  software-only Sophia session then emitted the client completion and cleaned
  up normally even though xterm lingered until its process safety timeout.
  The standalone launcher now explicitly tells bounded-xterm operators to let
  it exit automatically instead of showing the generic logout hint.
- **Native path is not the lock cause.** Audit: CPU-layer GL textures are
  reallocated to the incoming layer size (`sophia-renderer-native-egl` `gl.rs`),
  but that layer *is* the ≤64 MiB software buffer (≤ ~4096², inside RDNA3's
  16384 GL limit); DMA-BUF layers are client-bounded; scanout framebuffers are
  output-sized. In the crashed run the oversized CPU buffer was refused, so
  nothing oversized ever reached the GPU — only one output-sized blank frame
  presented.
- **The hard lock is downstream of the abnormal early exit at KMS handback.**
  There is no DRM/VT code in Rust; the launcher stops greetd/lightdm, Sophia
  becomes DRM master implicitly, and on exit drops master by closing the fd
  while the launcher restarts the display manager. Teardown
  (`detach_native_scanout`, `production_visual_runtime/native.rs`) drains
  in-flight scanouts, rejects pending presents, and rebuilds the output set
  without scanout — but it does **not** disable the CRTC or restore the prior
  mode. Sophia leaves its last framebuffer active on the CRTC and drops master;
  greetd/Xorg then re-modesets the RX 7900 GRE from that state. This teardown
  path is identical for normal exits, which hand back cleanly, so the trigger
  was the *early* abnormal exit (session aborted right after the first blank
  frame, before the steady-state page-flip loop) hitting the RDNA3 re-take in a
  fragile transient state.
- **Decision.** The probe fix removes the trigger; the benchmark should now
  reach steady state and hand back like every other clean run. Optional
  hardening for the handback (deferred to a validated physical change): issue an
  explicit CRTC-disable / mode-restore atomic commit before dropping master.
- **Environment (not in-repo).** Host is Void Linux (runit, no journald), so
  the prior-boot kernel dmesg was lost on reset. Persistent logging is now
  enabled via `socklog-void` (`/var/log/socklog/kernel/current`); setup helper
  is `~/sophia-amdgpu-logging-setup.sh`. GPU: Radeon RX 7900 GRE (Navi 31,
  RDNA3, `03:00.0`) plus a Raphael iGPU (`16:00.0`); `amdgpu` `gpu_recovery`,
  `lockup_timeout`, `reset_method`, `runpm` are all auto (`-1`).
- **Controller-fixed physical gate passed.** Two commit-pinned runs on
  `4cb4f5f` completed the 20-second producer and emitted passing schema-3
  reports. Both recorded 6,648 lines / 831 completed iterations, positive
  immutable CPU patch traffic, damage-driven partial repaint, zero authority
  drops, zero unexpected protocol/native failures, clean kernel deltas, and
  clean TTY3/greetd handback. Maximum CPU composition was 7 ms against the
  25 ms budget. The runner archives retained `visual-confirmed=false` because
  the local prompt did not record `yes`; the operator had separately observed
  the expected scrolling-number surface and responsive pointer in the paced
  session. That prompt metadata is not rewritten. The named automated
  acceptance criteria are complete; no Xserver parity claim is made.

## 2026-07-30: GLX Animation Passed; Startup Evidence Lost a Valid Early Frame

The bounded six-second physical rerun visibly animated. Radeonsi reported
52.956 client FPS on the RX 7900 GRE. Sophia completed 242 renderer requests
with 242 completions, no worker failure, soft stall, hard stall, or
release-queue failure; native teardown drained without an abandoned scanout,
and X protocol errors remained zero. Neither the 500 ms page-flip watchdog nor
the independent 11-second session deadline fired.

The run still exited status 1 because the completion gate claimed startup
readiness was never reached. Transaction 46 had in fact produced 70,988
nonzero RGB pixels, retired through KMS, committed visual admission, and was
logged as a stable mixed scanout. That happened immediately before the X focus
control acknowledgement pinned the startup surface. The startup reducer
correctly rejected pre-pin evidence to prevent another client from satisfying
the gate, but the owner retained only the latest transaction per surface.
Continuous animation made every later retirement overlap a newer pending
frame, so the already-proved stable frame could not be reconstructed.

Startup presentation evidence is now retained monotonically in a
surface-keyed map until readiness or native recovery. It records only a frame
that was stable at its actual KMS retirement and preserves the maximum observed
nonzero count. Once focus pins the startup surface, only evidence with that
exact surface identity can supply visual-detail and stable-presentation
events. A status bar or another client therefore cannot satisfy the startup
application, while asynchronous focus and presentation ordering no longer
creates a false failure.

The next physical rerun showed that the map alone was insufficient. It
retained transaction 46 correctly, but the consumer redundantly required a
current base committed-surface record. Removing the earlier `BufferSource`
subtype check had left this surrounding membership gate in place. DRI3 Present
content is a presentation-layer lease; after admission, the base committed
surface may legitimately be absent. A third bounded run still animated at
53.542 client FPS, completed 242/242 worker requests with no stalls or failures,
and drained all imports, while this gate alone produced the false startup
failure. The stable nonzero KMS retirement already proves both GPU content and
surface identity, so startup visual-detail reduction now uses that evidence
even when no base committed record exists. A regression covers that exact
state instead of approximating it with a DMA-BUF-backed base surface.

The following run again animated normally at 52.981 client FPS and drained
241/241 renderer requests, but exposed the underlying control-flow split.
Transaction 46 retired while the owner was waiting for the next authority
batch. That authority-wait path logged the same stable scanout as the ordinary
lifecycle path but did not update startup presentation evidence. The two
nearly identical retirement blocks had drifted semantically. Retirement is now
recorded by one shared function used by both service sites; it owns admission
retirement, surface-keyed startup evidence, reducer input, and the structured
retirement/scanout records. Phase-local input-proof bookkeeping remains at the
call sites.

The corrected physical run reached GPU content readiness and full startup
readiness in 178 ms without native recovery. It visibly animated at 59.088
client FPS, accepted 352/352 renderer requests, completed 351 Present Flips and
Idle notifications, reported zero protocol errors, drained every imported
image, and exited status 0 with clean session and TTY recovery.

That successful run exposed a final benchmark-only mismatch: verbose tracing
was intentionally disabled, but the cadence reporter still parsed per-frame
Present diagnostic lines. Presentation cadence is now accumulated in bounded
owner state from routed retained-buffer UST values and emitted once at
completion. The report keeps exact sample/interval counts, nonadvancing and
overflow flags, mean FPS, and p95 frame time without adding per-frame logging
overhead. Reporter regressions reject insufficient, nonadvancing, or overflowed
summaries.

The final six-second run validated the aggregate path end to end. Startup
reached readiness in 161 ms without recovery. The client reported 59.197 FPS;
352 routed retained-buffer samples produced 351 advancing UST intervals, zero
nonadvancing observations, 59.953 presentation FPS, and 17.324 ms p95 frame
time. Sophia completed 353/353 renderer requests, 352 exact Flip/Idle pairs,
zero protocol errors, zero live imports, and clean exit/TTY recovery. The
schema-3 performance reporter returned `status=pass`.

Teardown also exposed stale worker metrics: `ClearImages` was queued without an
acknowledgement, then the owner immediately sampled the previous cache state.
The maintenance command now returns its eviction result and updated persistent
statistics over a one-slot bounded channel with a one-second deadline.
Changing GLX buffers legitimately produced 242 imports and zero cache hits:
Idle releases each generation before the client may mutate and reuse it. The
report therefore requires positive imports and clean zero-debt teardown, while
retaining cache hits as an informative counter rather than inventing reuse.
The benchmark no longer forces verbose per-stage tracing. Normal structured
presentation and resource summaries remain enabled, while diagnostic tracing
can be opted into explicitly, so the performance probe does not measure its
own high-volume logging.

## 2026-07-29: GLX Freeze Was a Retained-Buffer Race on the Owner Thread

The failed physical `glxgears` run completed and retired two mixed frames. On
the next repaint Sophia emitted X Present `Copy` completion for a DMA-BUF it
continued to retain and sample, then entered a production `glFinish`. That
finish blocked the session owner for 10.097 seconds. Radeonsi reported a
cancelled command stream, lost context, and guilty hard recovery; during the
same interval the owner could not service the hardware cursor, physical input,
VT control, or X traffic. The static gears, frozen pointer, and session abort
were one failure, not separate input and animation bugs.

Xserver's Present implementation confirms that `Copy` means the server has
copied the pixmap and may idle it. Sophia had used that protocol result for a
zero-copy retained compositor lease. The protocol-neutral backend contract now
uses `Retained`, `Copied`, and `Skipped`. The X authority maps `Retained` to
Present `Flip`; software snapshots alone use `Copy`. Idle remains behind exact
KMS retirement.

Production rendering no longer executes `glFinish` in composition, cache
eviction, cache clear, or full-screen DMA-BUF drawing. EGL swap and KMS
retirement carry the normal same-GPU ordering; diagnostic readback is bounded
to the initial nonzero proof.

The stronger containment boundary moves production EGL/GL/GBM work to a
bounded renderer worker after the initial modeset. The worker receives an
owned immutable frame and retains the native BO under a lease ID. The session
owner receives only duplicated DMA-BUF FDs and a descriptor, so a driver stall
cannot stop input or cursor service. A request becomes a deferred scanout while
pending, reports a soft stall at 100 ms, and is quarantined after 1 second
without fake Present feedback. Completion metrics now include worker requests,
completions, failures, stalls, release-queue failures, and maximum request age.
The remaining multi-output optimization is to share one worker among outputs
that use the same render device.

The first worker-enabled physical rerun remained static but did not reproduce
the 10-second owner-thread stall. It exposed two asynchronous-boundary defects.
While the mixed Present was rendering, a CPU repaint replaced the output's
pending content label; the resulting KMS retirement was therefore recorded as
CPU and could not satisfy the exact visual-admission transaction. A second CPU
fallback then produced a valid GEM-backed scanout buffer whose DMA-BUF FD
export was unavailable. The old synchronous submit path correctly used the GEM
descriptor in that case, but the worker had incorrectly made FD export
mandatory and rejected the frame.

The first repair preserved mixed content ownership against CPU replacement
while the GPU frame was in flight. It also treated PRIME export as optional
and retained a descriptor fallback. That transport choice was provisional;
the later lockup evidence below showed that PRIME and shared-file descriptors
must be mutually exclusive topology modes, not preferred/fallback paths.

The next physical run proved those repairs: the seat and DRM device became
active, mixed transaction 46 reached KMS, and its page flip retired. The
concurrent elogind `failed to add session ... to hash map: File exists`
message was therefore nonfatal session-manager noise, not the presentation
failure. Admission still remained `not_committed` because the Present
scheduler had only queued and submitted states. A deferred renderer-worker
export was popped from the queue as if it had failed; when KMS later accepted
and retired that exact frame, no scheduler record remained to connect the
retirement to its prepared Engine commit.

The scheduler now retains one mutually exclusive in-flight record with
`Rendering` and `Submitted` variants. The full immutable Present record moves
`queued -> rendering` when the worker accepts it, `rendering -> submitted`
only when KMS accepts the returned buffer, and leaves the scheduler only on
retirement or controlled failure. A newer client frame may remain queued, but
cannot replace the prepared transaction, displayed layer, or resource
ownership of the frame already crossing the asynchronous boundary.

The following physical run proved admission and protocol feedback through two
advancing mixed Flip retirements, then froze the graphical session. Its log
identified two deeper ownership defects. Output content and damage still had
only pending/submitted slots, so newer compositor work relabelled transaction
51's worker result as `RetainedMixed`. More seriously, the worker and KMS use
duplicated descriptors for the same DRM file, but submission preferred a PRIME
round trip. Importing that exported DMA-BUF back into the same DRM file may
return GBM's existing GEM handle; KMS resource cleanup then closes a handle
still owned by the renderer. The observed sequence was framebuffer resource
creation failure, `DmaBufImageCreateFailed`, repeated EGL target replacement,
then a submitted page flip that did not retire before emergency recovery.

Output damage and native content now carry their own `Rendering` slot beside
`Pending`, `Submitted`, and `Presented`. A worker request moves the exact
snapshot and content into that slot; worker completion promotes that same
record even if newer work is queued. Scanout transport now declares its DRM
file topology explicitly. The current shared-file production path submits GEM
descriptors directly and never PRIME-imports its own buffers. A future
independent render-node path must provide PRIME FDs and fails closed if they
are unavailable; the two modes are not runtime fallbacks for one another.
A 500 ms page-flip watchdog terminates the session so a lost DRM event cannot
leave the graphical seat waiting indefinitely.

The operator reported the resulting graphical freeze as a complete system
lock. The process-local page-flip watchdog is necessary but cannot be the only
recovery boundary because it still depends on the session owner being
scheduled. The development TTY launcher therefore accepts an independent
wall-clock deadline. Its separate shell process records the deadline, sends
TERM and then KILL to the complete Sophia session process group, and lets the
existing parent cleanup restore keyboard, console graphics mode, keyd, and the
display manager. The bounded `glxgears` proof enables that deadline at workload
duration plus five seconds. This is containment, not evidence that the
rendering defect is fixed, and it cannot recover a kernel-wide scheduler
failure.

## 2026-07-28: GLX Compatibility Uses the Generic Standalone Workload Slot

Sophia's bounded GLX proof now launches `glxgears` through the same standalone
application lifecycle used by the fixed Vulkan proof. Workload selection and
bounded-process policy remain operator tooling concerns: Engine receives an
ordinary X11 client and retains no `glxgears`, Kitty, xmonad, or benchmark
identity. This preserves the frontend-neutral scene and presentation model.

The probe runs at a fixed 500-by-500 geometry with swap interval one and exits
after 20 seconds. Its report fails closed unless the log identifies the OpenGL
renderer, shows client animation samples, advancing routed post-KMS Flip
timestamps, positive native and mixed exports, Present idle-fence progress,
and a clean resource drain. Client-reported FPS and Sophia's actual
presentation cadence are separate fields because they describe different
boundaries.

This proof diagnoses direct GLX bootstrap and the DRI3/Present compatibility
path. It cannot replace the deterministic Vulkan parity workload, and no
GLX-specific fast path has been added to Engine or the renderer. Physical
metrics remain pending until the dedicated-TTY run is retained.

The first physical attempt failed before creating a window:
`glxgears` could not select an RGB double-buffered visual. Kitty had exercised
the modern FBConfig/context path, while the classic catalog advertised zero
depth bits and the authority did not decode legacy visual-based context
creation. The repair makes both GLX entry paths explicit data variants,
advertises a 24-bit depth buffer in matching visuals and FBConfigs, normalizes
legacy visuals to the bounded FBConfig runtime identity, and validates direct
MakeCurrent context/drawable pairs. A bounded external-client preflight must
now reach visual discovery, direct context creation, DRI3 import, and Present
before the script takes over the TTY. The real-Mesa preflight passes those
stages without an X protocol error. Mesa keeps MakeCurrent local for this
direct-rendering workload, so that supported request is not a required wire
observation.

The first run that reached native output exposed two coupled generic
presentation-lifetime faults. The initial gears frame was imported, composed,
and retired successfully, but an unrelated software cycle had queued a stale
CPU frame while that mixed Present was in flight. It replaced the visible
gears with a blank output. Sophia then retained the completed client DMA-BUF
until the successor's KMS retirement; Mesa needed that idle fence while the
successor was being imported, and radeonsi eventually reported a guilty
context hard recovery.

Production frame reduction now preserves an already-submitted GPU Present
instead of queuing CPU fallback behind it. When an acquired mixed Present
replaces the same surface, the runtime retires the prior composited source and
signals its idle fence before importing the successor. Present Complete remains
tied to the prior KMS page flip, so protocol timing and client-buffer reuse are
separate facts. Focused regressions require both the no-superseding-frame
policy and actual xshmfence progress while the successor remains ready.

## 2026-07-28: Xserver Parity Uses Present Evidence, Not Demo Wall Time

The software-Present optimization reached an observed 59.947 FPS with a
17.564 ms p95 interval on a 459-sample physical run, but the tree retained no
same-machine Xorg or XLibre baseline. Treating an ideal 60 Hz interval as that
baseline would hide provider, output, mode, and server-side presentation
differences.

The paired reference runner now launches the same fixed 500-by-500,
900-frame, FIFO `vkcube` workload under an Xserver. A bounded XCB probe attaches
only to newly created matching top-levels and records actual Present Complete
UST/MSC values. The shared cadence reducer computes FPS and p95 for both
systems. The comparison fails closed when workload, requested frames, Vulkan
provider, Vulkan/X Present mode, or output pixel count differ, then applies the
existing 90% rate and inverse-p95 gate.

`glxgears` is retained only as an optional Xserver GLX/OpenGL reference and
gross cadence probe. Its workload and driver behavior are not representative
enough to become Sophia's rendering acceptance metric, so its record is kept
separate and cannot affect the Vulkan parity decision. A bounded Sophia-side
direct-GLX/DRI3/Present pair is tracked explicitly rather than treating the
Xserver-only number as Sophia evidence.

The first retained Xserver capture used XFCE's composited Xorg path. It
completed 898 of 898 observed frames with monotonically advancing UST and MSC,
but every completion mode was `Copy`; Sophia's corresponding completion is
post-KMS `Flip`. Rejecting all non-Flip samples incorrectly classified healthy
FIFO client cadence as missing evidence. The reference reporter now admits
advancing Pixmap `Flip`, `Copy`, and `SuboptimalCopy` completions while
recording the path. A mismatch is explicitly `cadence_only`: it can gate
throughput and cadence, but cannot establish final scanout latency. An
unredirected Xserver Flip capture remains the stronger follow-up if that claim
is required.

The paired physical Vulkan gate passes. Both fixed workloads produced 898
observed completions with the same llvmpipe provider, 500-by-500 surface,
FIFO Vulkan mode, and 2560-by-1440 target output. Sophia measured 59.953 FPS
and 17.155 ms p95; composited Xorg measured 59.950 FPS and 16.686 ms p95. The
rate ratio is 1.0001 and the inverse-p95 ratio is 0.9727, above the required
0.90 in both dimensions. Sophia's maximum CPU composition was 6 ms, maximum
native upload was 3 ms, and native submission failures remained zero. This
closes the software-Present cadence gate while preserving the separate
unredirected-Flip latency follow-up.

## 2026-07-28: Software Present Optimization Keeps Ownership Boundaries Intact

The first correct standalone software-vkcube result retired 487 frames in
17,755 ms, about 27.4 FPS. Its retained maxima identified two independent
costs: CPU composition reached 17 ms and native upload reached 10 ms. The hot
path also cloned the source pixmap, copied a full immutable snapshot across the
authority boundary, allocated and checksummed a 2560-by-1440 output, cloned
that output for reporting, copied it again before GBM write, and recreated
mixed EGL/GBM render targets. Correct Present feedback had exposed throughput
rather than another policy or lifecycle fault.

The optimized path keeps the architectural boundaries unchanged. X authority
retains a bounded read-only SysV mapping, resolves XFixes regions, copies only
clipped update rows into its logical-window backing, and emits a fixed-capacity
immutable patch batch with a stable handle and monotonic generation. Renderer
intake prevalidates the complete batch before applying it, so malformed suffix
data cannot expose a partially updated frame. Engine sees only ordinary buffer
generations and damage; the WM remains blind to storage and protocol details.

Output composition now owns reference-counted bytes, reclaims the allocation
when no downstream lease remains, performs three exact startup pixel proofs,
and then derives bounded evidence from display-list generations and geometry.
The same-stride native CPU upload borrows those bytes instead of making another
full-frame vector. Mixed CPU/DMA-BUF composition retains its native target and
frame surface across same-size frames. Metrics expose replacement versus patch
traffic, payload bytes, evidence mode, and target reuse so a physical result
can prove that the intended path actually ran.

`tools/benchmark_sophia_vkcube_tty3.sh` supplies a fixed 900-frame workload.
`tools/report_sophia_rendering_performance.sh` computes FPS and p95 cadence from
Present UST values and can enforce a same-provider Xorg parity gate. Physical
results are pending. A retirement-fed three-slot CPU scanout pool is deliberately
conditional: per-frame GBM allocation should be replaced only if the new
measurements show it remains material, because recycling a scanout BO before
KMS retirement would violate the existing ownership proof.

The first bounded benchmark exposed a scheduling regression before it could
collect performance data. Sophia selected the 500-by-500 PresentedBuffer and
admitted its frontend surface, but submitted an unchanged empty CPU frame
between output-baseline readiness and visual-admission commit. That page flip
retired successfully; the visual transaction itself never committed, two
layout epochs timed out, and the eight-second `no_surface` startup guard
performed a clean shutdown. There was no panic, protocol error, native submit
failure, or emergency recovery.

The cause was an overloaded checksum contract. The first three compositions
used an exact output-pixel checksum, while later compositions used bounded
generation/damage evidence. Switching metric modes therefore changed the value
for identical pixels, and native scheduling interpreted the proof-algorithm
change as new content. Scheduling identity is now always derived from immutable
buffer generations, geometry, compositor primitives, and cursor state; the
metric mode changes only how nonzero output is counted. Regressions prove
identical display lists retain one identity across the warm-up boundary and
that an immutable generation change still advances it. A new physical
benchmark result remains pending.

The first retest removed that false page flip but still reached `no_surface`.
It exposed the underlying admission-order dependency more clearly. The
PresentedBuffer candidate was selected before the blind WM staged its pending
layout; the layout correctly retained that selected transaction, then
`AdmitSurface` completed. Control completion advanced Engine from
`ControlPending` to `AwaitingPixels`, but production resolved pending layout
only while processing a later authority batch. The software client was already
blocked waiting for Present feedback, so no later batch existed and the two
sides deadlocked until the startup guard fired. In the historical successful
run, the candidate happened to arrive after control completion and authority
processing called the reducer immediately.

The owner loop now runs one shared layout-progress service after every event
class that can unlock a pending epoch. Admission-control acknowledgement
advances only Engine admission state; authority observation, WM staging, and
control completion all invoke the same idempotent reconciliation. A ready
layout remains pending when the WM-update slot is occupied and commits after
that slot drains rather than failing or overwriting the older update. Thus
candidate-before-control and control-before-candidate converge without a
synthetic authority wakeup or hidden commit side effect.

That change advanced the next physical run through visual admission, layout,
focus, and eight CPU compositions, then exposed a second ownership defect as
`no_visual_detail`. Renderer CPU intake installed the quarantined snapshot and
immediately reclaimed every buffer absent from Engine's committed surface
snapshot. Absence was intentional before admission, so the later released
transaction referenced a buffer the renderer had already discarded. The
unchanged empty output correctly produced no new native submission.

CPU update application and residency reclamation are now separate operations.
The live layout emits a sorted, bounded handle snapshot for CPU buffers
referenced by pre-admission or release-pending transaction groups. Backend-live
joins those handles with committed surfaces and the current production batch;
renderer-live retains exactly that complete root set. Staged pixels may reside
in renderer-private storage but cannot become visible until their exact Engine
transaction commits. Removal, withdrawal, supersession, or timeout drops the
root and reclaims the buffer on the next cycle. This avoids another pixel copy
and keeps X resources, application identity, and layout policy out of Engine
and renderer state.

The console's contemporaneous elogind message was not causal. Session 214 was a
valid `_greeter` session on tty7 created after Sophia's clean tty recovery; the
Sophia run had already acquired input devices, modeset both outputs, and
executed for eight seconds. Its leader and `.ref` FIFO were live during
diagnosis, and the runtime record later disappeared normally. The warning
belongs to greetd/elogind handoff diagnostics, not rendering or admission.

## 2026-07-28: Visible Vulkan Diagnosis Starts With One Natural-Size Client

The first physical xmonad run after evidence-ranked admission produced no
visible change: default vkcube still opened a blank bordered surface. Further
changes made only inside the combined Kitty/xmonad path would not distinguish
the X11 Present/rendering fault from compatibility-bridge layout behavior.

Sophia now has a dedicated single-client production profile. It launches
`vkcube --wsi xcb` directly, omits Kitty, xmonad, xmobar, and the X11 WM
compatibility bridge, and uses the external reference WM's new generic
`natural` layout policy. That policy sees only an opaque layout node, preserves
its natural allocation, centers it within output bounds, and emits no policy
resize. It is usable for any single-purpose session; neither its reducer nor
Engine branches on vkcube identity.

The profile deliberately retains policy-managed deferred mapping, the X
authority's DRI3/Present stream, exact visual-candidate admission, renderer
composition, and KMS page-flip retirement. It therefore draws a useful fault
boundary: a blank standalone window localizes the defect below the xmonad
bridge, while a visible cube localizes the remaining defect to full-desktop
policy/configure integration. The strict verifier requires one
PresentedBuffer candidate, exact armed/presented/retired identity, nonzero
scanout pixels, normal logout, and zero live presentation resources. Physical
evidence is pending.

## 2026-07-28: Admission Recovery Requires Evidence, Not Latest Extent

The first physical run after staged-pixel recovery still produced the same
blank bordered window. The retained trace disproved another readiness-only
fix. The new policy-managed surface reached frontend admission and recovery
transaction 3 committed its geometry and focus ring, but no
`visual_admission status=armed` or native Present retirement belonged to that
surface. Every retired GPU frame still belonged to the existing terminal.

The causal defect was the scalar `safe_sizes` table plus reverse transaction
scan. Every accepted authority transaction overwrote the surface's safe extent.
A full-size software backing clear created after the blind WM's tiled configure
therefore replaced the earlier natural client-frame extent. Recovery published
that policy-sized blank extent as an exact constraint, selected its CPU
transaction, and treated admission as synchronous. The real Present group
remained quarantined, so the application could not advance its frame loop.
This single ordering error explained both the missing cube and failure to
produce a fixed-size floating placement.

River's separate scheduled/configured/rendering states are the closest policy
and timeout model for Sophia. Xserver supplies the X11 content rule: Present,
clears, and core drawing belong to one ordered logical window stream. Niri
supplies only the policy precedent that exact client constraints may open
floating. Sophia combines those lessons without adopting another project's
protocol or shell architecture.

Engine now retains a passive `SafeSurfaceObservation` containing source
transaction, extent, evidence class, and Engine observation sequence. During
admission, a complete Present/XPixmap buffer outranks an accumulated software
backing snapshot regardless of arrival order; equal evidence remains
newest-first. The admission stage must select the exact transaction named by
that record. A selected complete Present supersedes older covered groups,
newer groups remain fenced until retirement, and only the matching page flip
publishes managed state and focus. The X authority still owns X semantics, the
blind WM still owns floating placement, and no application identity crosses
either boundary.

Offline reducers reproduce the physical 500-by-500 Present followed by a
1276-by-1422 blank backing snapshot and prove recovery retains the Present
transaction and extent. The physical verifier now requires presented-buffer
candidate evidence before every armed admission. A fresh hardware run remains
required before this becomes retained session evidence.

## 2026-07-27: Recovery Cannot Substitute Extent History for Pixels

The first physical vkcube run after retirement-gated DMA-BUF admission still
showed a small tiled border with no cube. The retained log proved that this was
not an XMonad floating decision: vkcube surface 6291456 reached generic
frontend admission, the first two-surface resize timed out, and recovery
transaction 3 published a three-layer layout and focus ring. It never emitted
`sophia_live_visual_admission status=armed`, and no Present retirement belonged
to that surface. All observed GPU retirements remained Kitty surface 2097168.

The escape was the recovery readiness reducer. It allowed
`layout_epochs.committed_size == requested_size` to satisfy a pending
admission, even when the proposal owned no staged transaction for that
surface. A WM proposal could also carry a bufferless planning node without a
size change, leaving the surface outside `admission_surfaces`. The layout then
published geometry, chrome, and focus from extent history alone.

Every `PolicyPending`, `ControlPending`, or `AwaitingPixels` layer now becomes
an explicit admission target whether or not the WM changed its size. Retained
size state remains valid recovery guidance only; admission readiness requires
the proposal's exact staged concrete transaction. The regression recreates an
acknowledged recovery surface with matching retained extent but no pixels and
proves that the proposal stays held, committed layers remain empty, and focus
is not published. A client that never supplies matching pixels is withdrawn by
the existing bounded timeout path instead of receiving an empty compositor
frame.

## 2026-07-27: DMA-BUF Admission Completes at Exact Retirement

The follow-up physical trace removed the earlier mixed-transaction rejection
but exposed the remaining visual-lifecycle defect: vkcube surface 6291456 was
admitted, laid out, focused, and framed without any retired Present for that
surface. All 51 retired Presents belonged to the existing Kitty surface. The
admission commit had treated a released DMA-BUF transaction with no causally
paired Present as a synchronous visual commit, while the X mapped snapshot
could independently disable quarantine.

DMA-BUF admission now enters `AwaitingRetirement` with the exact selected
visual transaction. It becomes managed and eligible for deferred focus only
when that surface and transaction retire from KMS. Admission quarantine no
longer consults the mutable X mapped bit. Both quarantine and production intake
require one-to-one surface/buffer pairing between DMA-BUF transactions and
Present submissions. Resource release is held while a quarantined group
references its DMA-BUF or fences, and backend intake registers and begins
presentation ownership before applying release. A Present-bearing GPU cycle
also cannot queue a retained CPU frame ahead of its candidate.

Offline Engine, CLI, and backend regressions cover exact retirement matching,
mapped-bit independence, malformed-group rejection, deferred resource release,
and deferred focus. The retained session log predates its source commit and is
diagnostic rather than proof; the new physical verifier requires matching
`armed`, `presented`, and native `retired` records plus clean teardown.

## 2026-07-27: Admission Release Preserves Atomic Transaction Groups

The latest physical vkcube trace exposed a second identity defect after
per-Present scene ownership was corrected. The admission quarantine retained
vkcube transaction 858, then appended it to the next ordinary frontend batch
whose envelope transaction was 367. Engine correctly rejected the manufactured
two-surface batch with `expected_transaction=367 actual_transaction=858`.
Transport, Kitty, and KMS remained alive, but the admitted vkcube surface never
crossed into committed visual state.

The envelope and the atomic unit are now explicit separate data shapes.
`LiveProductionAuthorityBatch` owns envelope-scoped DMA-BUF and fence lifetime
facts plus ordered `LiveProductionAuthorityGroup` records. Each group owns one
transaction ID and validates every surface transaction and Present submission
against it before scheduler or Engine intake. The pre-admission path retains
the same complete groups in a fixed 256-entry FIFO, reprojects and rebases them
only at accepted admission, and releases them beside—never inside—the current
frontend group. Mixed identity and capacity exhaustion fail the session closed.

Offline regressions reproduce transactions 367 and 858, validate both groups,
and commit them independently through the production coordinator. A routed
deferred-map vkcube smoke additionally requires real map intent, generic
`AdmitSurface` delivery, DRI3 import, and two exact Present Complete/Idle
round trips. The restricted sandbox has no `/dev/dri`; an unrestricted local
attempt reached intent and admission, but this environment's vkcube selected
llvmpipe and emitted no DRI3/Present handoff. The command therefore remains a
hardware-Vulkan preflight rather than retained proof. Visible vkcube pixels and
native KMS retirement remain the short physical roadmap gate.

## 2026-07-27: Present Requests Are Not Persistent Scene State

The first physical run after truthful deferred admission reached
`frontend_admitted`, but every Kitty Present was rejected before rendering.
The live visual runtime had stored xmobar and Kitty as historical
`SurfaceTransaction` values, cloned the entire table for each Present, and
asked Engine to prepare it under Kitty's newest transaction ID. Engine
correctly rejected mixed batches such as expected transaction 403 with actual
transaction 198. Kitty never entered committed state or focus, and the startup
watchdog exited at `stage=not_focused`; KMS and protocol transport remained
healthy.

Xserver's Present implementation keeps each queued request's window, pixmap,
serial, fences, and timing separate from persistent window state. Niri likewise
uses client transactions as readiness blockers and builds output render
elements from current compositor state. Sophia retains its stronger
`PreparedSurfaceCommit` contract but applies the same ownership lesson: a
queued Present owns exactly one matching surface transaction, while unrelated
surfaces come from Engine's committed baseline.

The production coordinator now prepares one Present candidate and rebases only
its causal Engine generation. Backend input and compositor projections derive
from committed state rather than pending transactions, and page-flip retirement
promotes the candidate before successful feedback. The Present hot path no
longer clones and validates every historical scene transaction. Mixed
xmobar/Kitty identity, malformed-candidate rejection, generation preservation,
and exact retirement are covered by offline regressions; physical startup
reproof remains open.

## 2026-07-26: X Hierarchy Defines the WM Admission Boundary

The first physical pre-pixel-admission candidate still exited with
`stage=not_focused`: native scanout and page flips remained healthy, but the WM
never received a manage request. A retained real-xterm authority trace exposed
the exact ordering. xterm issues `MapSubwindows` for descendants of its
toplevel, then issues `MapWindow` for the toplevel itself. Sophia had discarded
the requested parent at `CreateWindow` and implemented `MapSubwindows` as
"map every window in this namespace." That prematurely moved the toplevel out
of its deferred state, so the later real map request could not emit the
presentation intent required by Engine and the blind WM.

The authority window table now owns parent links as passive X protocol state.
`QueryTree` projects that state, reparenting validates cycles, and
`MapSubwindows` affects only direct children. A non-override-redirect root
child is a policy-managed toplevel; descendants and override-redirect windows
are client-positioned. Deferred map policy therefore applies only at the X
root boundary. The retained wire regression reproduces xterm's real opcode
sequence and proves that mapping a child does not admit its parent, while the
following toplevel `MapWindow` emits exactly one managed presentation request.

Core software drawing follows the same tree reduction. Descendant buffers stay
X-authority resources, while their translated damage is accumulated into one
immutable toplevel presentation buffer and one Sophia surface generation.
The concrete presentation extent grows only with observed descendant coverage
and is capped by the toplevel geometry; a configure alone therefore cannot
manufacture a full-size buffer that would satisfy admission. This gives xterm's
shell/content window split one visual identity without leaking X children into
the WM.

The audit also closed a visual-boundary gap. Drawing to an unmapped managed
window is valid X11 traffic, but it is not permission to enter Sophia's
committed scene. The live layout now keeps at most one latest pre-admission
transaction per surface in a bounded data table. It records the safe extent
for planning and recovery, excludes the transaction from renderer intake, and
releases it exactly once—rebased to the first Engine visual generation and the
accepted WM geometry—after frontend admission is acknowledged and matching
pixels are ready. Withdrawal, removal, and terminal timeout erase the retained
record. Early Present submissions follow the same boundary in a fixed
256-record queue; overflow fails the session closed instead of leaking a GPU
submission or growing memory without bound. No client identity or X resource
enters Engine or WM policy.

## 2026-07-26: Managed X11 Mapping Requires Pre-Pixel Admission

The repeated blank `vkcube` frame was an ordering defect, not evidence that the
application required a hard-coded floating or fixed-size rule. The live X
frontend fulfilled `MapWindow` immediately and emitted `MapNotify`/`Expose`
before the blind WM had an opaque node to manage. Sophia could create that node
only after observing a pixel-backed transaction, while the application was
already reconfiguring its swapchain from the initial 500-by-500 window to the
tiled allocation. The latest trace registered buffers and fences but retired no
cube Present at the accepted layout.

X server map-redirection semantics supplied the useful reference model: keep a
policy-managed window unmapped while policy decides its geometry, then configure
and map it as one admitted transition. Sophia implements that invariant through
its own protocol-neutral boundaries. The frontend emits
`SurfacePresentationIntent`; Engine retains passive admission facts; the WM
plans an opaque bufferless node; and `AdmitSurface` configures and maps only
after proposal validation. Matching authority pixels, not the control
acknowledgement, establish committed visual truth. No X server code or
application-specific fact enters Sophia.

The same audit found an overly broad Present barrier. Any pending layout had
blocked every queued Present, including unrelated stable surfaces. Each
submission now carries an immutable disposition: immediate, staged for one
layout epoch, or rejected for a known size mismatch. The bounded scheduler can
continue unrelated work, shares one immutable CPU-layer batch, and releases
staged submissions when the epoch resolves. Wrong-size pending pixels update
only the safe-observation record and cannot leak into the visible layer table.

Reducer, wire, admission, WM pre-pixel planning, and mixed Present scheduling
tests cover the new ordering. Default `vkcube --wsi xcb` remains the physical
AMDGPU proof; until that retained run succeeds, the roadmap item stays open.

## 2026-07-26: Resize Timeout Recovery Is An Engine Replan

Launching a non-cooperative Vulkan top-level exposed an architectural failure,
not an application quirk. This recovery model remains valid, but the later
pre-map admission audit above supersedes it as the root-cause diagnosis for the
blank initial `vkcube` window. Xmonad proposed a tiled epoch, existing clients
and the new surface did not all publish the exact requested extents before the
deadline, and the CLI compensated back to old sizes. Preselecting startup
dimensions would still have encoded application policy.

Resize/admission recovery now belongs to a protocol-neutral Engine coordinator.
It retains safe authority content extents, fences pixels from abandoned sizes,
and stores declared constraints separately from temporary exact recovery
constraints. A timed-out first admission is retried once with
`min_size == max_size == safe_extent` and `resizable = false`; the blind WM
still chooses placement. The unsafe slow-client option that synthesized visual
truth from timed-out pending pixels was replaced by a replan-at-committed-extent
decision.

The legacy compatibility bridge generically exposes effective constraints as
synthetic ICCCM `WM_NORMAL_HINTS`. When manage-time constraints change it
remanages the private synthetic window so stock xmonad reevaluates fixed-size
policy. No Vulkan, Kitty, xmonad-client, XID, namespace, title, class, or PID
fact enters Engine policy. Default `vkcube --wsi xcb` is the physical
compatibility proof, not an implementation branch.

The real unmodified-xmonad bridge smoke now follows its sequential three-window
tiling proof with a manage-time transition of one opaque node to an exact
500-by-500 constraint. Xmonad returned that floating placement after the bridge
remanaged the private synthetic window, proving the ICCCM path without client
metadata. Physical authority/presentation admission remains the open roadmap
gate.

The first physical run then found one remaining unit-boundary error. Recovery
retained a 500-by-500 client buffer but sent 500-by-500 as the WM's outer
constraint. The active two-pixel clearance correctly inset that allocation to
a 496-by-496 client configure, which the application did not satisfy. Engine
now owns the inverse conversion as well: committed geometry and content
constraints become 504-by-504 outer facts before the WM boundary, and the
existing inset returns exactly 500-by-500 to the authority. Focus ring/frame
width is therefore handled generically rather than encoded in recovery policy.

## 2026-07-27: Delivery Acknowledgement Retires Both Input Ledgers

The second emergency capture proved that both synthetic modifier releases
reached the frontend: all 547 expected input deliveries flushed, the pressed
key ledger reached zero, repeat was inactive, and Engine/native teardown was
clean. Completion still failed because the two release IDs remained in a
separate control-ordering barrier. Normal loops eventually pruned that set
during another control-service pass; emergency completion exits immediately
after the input-delivery reducer settles and exposed the split ownership.

Input-delivery acknowledgement now atomically retires an ID from both the
general pending set and the client-key release barrier. A focused reducer test
locks that invariant. The pre-emergency physical gates on the parent candidate
remain valid for this isolated bookkeeping correction. After the new commit
runs its unattended semantic gate, a closed-path adoption command reverifies
the parent native, hardware-smoke, and xmobar evidence and records provenance.
It cannot adopt emergency evidence or accept broader runtime changes.

## 2026-07-26: Emergency Shutdown Owns Routed-Key Drain

The first commit-pinned emergency gate triggered both the live owner and the
independent guard, drained native presentation, restored the TTY, and returned
control, but the inner session failed its final key-ledger invariant. Ctrl and
Alt had already been routed to the focused client before Backspace completed
the emergency chord. The loop waited for existing authority deliveries but
did not synthesize releases for those two routed presses, leaving the client
ledger nonempty at completion.

Emergency shutdown now snapshots the complete bounded pressed-key ledger,
cancels active repeat, routes releases through the normal X authority path,
and adds those delivery IDs to the existing acknowledgement barrier. This is
session-wide input ownership, not Kitty, xmonad, or chord-specific client
policy. Surface-scoped focus and logout flushes share the same release reducer.

The run also exposed an outer-control-plane mismatch. The independent guard
intentionally exits its launcher with status 130 after recovery, while the
promotion driver previously rejected every nonzero launcher status before
running the emergency verifier. Gate policy now admits 130 only for emergency
evidence. The verifier remains authoritative: it requires the inner session
to exit zero with drained key, control, native, and Present state plus exact
TTY restoration.

## 2026-07-26: Physical Evidence Exposed Verifier Assumptions

The first short hardware-smoke promotion run was healthy but its verifiers
rejected four assumptions that were stricter than the runtime contract. A
physical click can queue only its press before focus applies; its release may
then route normally, so one or more queued handoff events is valid. Managed
Present geometry is the WM allocation inset once by negotiated Engine chrome,
not the outer allocation. A damage-driven compositor may idle stable windows,
so mixed-export ledgers require positive balanced work rather than an
arbitrary minimum frame count. Finally, a physically connected output with no
moving client retains its synchronous baseline buffer without producing an
asynchronous page-flip retirement.

The corrected proof checks data-oriented end-state invariants: ordered focus
application and queued input delivery, negotiated clearance and exact inset
geometry, positive mixed exports with balanced target/pipeline/frame-surface
accounting, and per-output completion with one retained displayed buffer.
The retained run satisfies those invariants on both outputs, completed one
drained VT round-trip and normal logout, and reported clean protocol, control,
native, namespace, and TTY teardown.

Promotion can explicitly adopt this one parent run after a verifier-only
correction. The adoption path accepts only a verifier-rejected result, uses a
closed changed-path allowlist, and reruns both hardware verifiers before
writing source-commit provenance. Runtime changes or launcher failures still
require fresh physical evidence.

## 2026-07-26: Promotion Separates Semantic Automation From Hardware Proof

The accumulated Milestone 9 operator sequence had become both tedious and
weakly reproducible. It asked one person to remember clipboard, workspace,
focus, repeat, launch, close, VT, bar, and teardown gestures across many
physical sessions even though most of those state machines already have
deterministic two-output QEMU or reducer coverage. A missed gesture could fail
the ledger without identifying a product defect.

Promotion now begins with one commit-pinned unattended semantic gate. It runs
the canonical offline local regression suite and the retained M7 xmonad and M8
mixed-application QEMU scenarios, then verifies their exact evidence. Policy, protocol,
application, focus, workspace, clipboard, damage, and teardown semantics are
therefore machine-driven and replayable. The promotion driver can run this
first gate outside TTY3.

Physical evidence is reduced to facts virtualization cannot establish:
native chrome/hot reload, real KMS pixels and lifetime, actual keyboard and
pointer routing, one libseat VT round-trip, client-positioned bar geometry and
pointer behavior, normal TTY recovery, and independent emergency recovery.
The four-Kitty and bar proofs each have a short dedicated verifier. Exhaustive
keyboard/VT, pointer-edge, launch-burst, clipboard, and long xmobar workflows
remain focused diagnostics after their owning subsystem changes; they are no
longer memorization-heavy per-candidate rituals.

This is an evidence-boundary change, not a runtime exception. QEMU never
substitutes for AMDGPU, libinput, monitor, greetd, or emergency-recovery proof.
The Engine, X frontend, WM bridge, and renderer remain unaware of promotion
policy.

## 2026-07-26: Real M7/M8 QEMU Runs Found Cross-Layer Regressions

Running the retained scenarios as real guests, rather than relying only on
fixture verifiers, exposed five independent defects. The M7 click probe landed
on newly reserved compositor clearance at the output edge; the harness now
resets to the edge and moves 32 pixels into client content. The M8 Vulkan
fixture assumed its fixed startup size would survive xmonad relayout; it now
uses a deterministic software-rendered size on a separate workspace so the
test measures mixed presentation rather than toolkit resize timing.

Firefox then reached an unimplemented XSync counter request after Sophia
advertised XSync 3.1. The frontend now implements bounded generic counter
create, set, change, query, destroy, list, resource ownership, and namespace
cleanup. No browser policy entered Engine. Firefox also creates more than one
top-level surface and its software-rendered content does not retire through the
Present ledger. Launch admission therefore tracks a bounded fixed set of
observed surfaces and settles on any committed stable visual surface, whether
stability is proven by CPU visual detail or retired Present.

The final GTK launcher close exposed a teardown ordering defect. Shortcut
prefix modifiers had entered the per-client pressed-key ledger, but the
launcher did not select keyboard events. Sophia waited for synthetic release
acknowledgements before sending close, so the close timed out without being
dispatched. Closing surfaces now clear those keys through the state-only
authority path and dispatch close immediately. Focus handoff and VT suspension
still deliver ordered releases because those clients remain alive. The real
M8 run completed all eight browser stages, delivered all 48 controls with zero
timeouts, reaped the launcher, drained input and WM state, and revoked the
namespace and X authority cleanly.

The guest also established a precise compatibility boundary. Physical wheel
axis input is observed, hit-tested, and routed to Firefox, but the current
reduced core-button translation does not produce Firefox DOM `wheel` events.
The fixture advances its scroll stage with a focused Space key only after a new
axis route is observed. This proves generic Engine axis routing and continued
browser interaction; it does not claim native Firefox wheel compatibility.
That work remains in the X frontend compatibility milestone.

## 2026-07-26: Client-Positioned Clicks Must Bypass WM Focus

The first external-config promotion run completed every ordered reload phase,
then terminated when the operator clicked the status bar. Hit testing correctly
selected the `ClientPositioned` surface, but the generic primary-button path
started a managed focus handoff because the bar differed from keyboard focus.
The blind WM correctly had no workspace registration for that excluded
surface; treating its rejection as fatal ended the session.

Physical input now consults the existing protocol-neutral presentation-role
table before starting a focus handoff. A `ClientPositioned` target receives
button input directly while the current keyboard focus is retained.
`PolicyManaged` targets keep the ordered WM/Engine/frontend focus handoff.
This is role policy rather than a status-bar or xmobar exception.

## 2026-07-26: Physical Promotion Is A Commit-Pinned Ledger

Milestone 9 previously depended on several correct but independently retained
physical captures. That could not prove that normal, stress, chrome, status-bar,
VT, and emergency behavior all belonged to one candidate build. The promotion
driver now keys an append-only evidence directory by the full Git commit,
refuses a dirty tree, selects one ordered gate per invocation, archives the
source logs before verification, and advances only when the gate-specific
fail-closed verifier succeeds. Failed evidence is retained but never counted.

The same evidence boundary stays data-oriented and privacy preserving. Engine
reduces a compositor display list to aggregate frame/ring/primitive counts and
clearance rather than exposing surface identities. Physical keyboard coverage
reduces to 21 shifted printable positions and 12 VT targets without logging
typed characters. Protocol-specific authorities remain unaware of compositor
chrome, and the xmonad compatibility bridge explicitly reports that it did not
negotiate native chrome policy.

## 2026-07-26: Chrome Uses Stable Allocation Clearance

The first focused-border prototype painted four inside-edge solids over the
committed client rectangle. Increasing its width therefore hid client pixels.
The durable model now distinguishes a focused ring from an optional
focused/unfocused frame. Engine treats WM geometry as an outer allocation and
derives client content by one checked inset equal to the maximum enabled chrome
width. Focus changes only repaint; width changes prepare matching client
buffers before the new visual style becomes active.

The renderer-neutral display list carries one semantic border node per
surface/role. Damage and backend lowering share one fixed four-band expansion,
eliminating per-edge policy records and temporary edge vectors. Client-
positioned surfaces are excluded before display-list construction. KDL schema
2 and WM API v6 name focus-ring and frame policy explicitly; schema 1 is
rejected with migration guidance. This follows Niri's useful separation of
focus indicators and frames without importing its Wayland authority model or
making Sophia's Engine dependent on a particular WM, frontend, or renderer.

Chrome ownership is capability-gated rather than inferred from the presence of
a WM process. A native WM may negotiate the policy; the X11 compatibility
bridge deliberately does not, so xmonad and other external WMs use the
Engine/compositor fallback. Both sources reduce into the same candidate versus
visually committed state, keeping width changes behind one relayout boundary.

## 2026-07-26: Click And Drag Now Have Independent Focus Proofs

The first unattended pointer-focus proof used only a click-drag gesture. That
exercised the generic ordered handoff but left plain click-to-focus as an
inference. The M7 harness now creates two independent focus transitions against
an unfocused visible surface. The first sends primary press/release and a
following key. It then moves focus away through the WM and sends primary
press/motion/release plus a different following key.

Both real QEMU sequences passed through virtio input, Engine hit testing, the
blind-WM focus request, X-frontend acknowledgment, ordered deferred-input
release, focused-border composition, combined output-damage retirement, and
keyboard delivery to the selected opaque surface. The click released two
records and the drag released three. Completion retained two output baselines,
clean bridge recovery and logout, zero stale WM responses, zero protocol
errors, and no native cleanup debt.

The verifier treats each gesture as its own bounded state machine. It rejects a
missing or overlapping gesture, an incomplete handoff, insufficient click or
drag records, a missing key-probe boundary, a key routed to another surface,
missing target border/damage/repaint evidence, or a missing completion marker.
Physical libinput and visual confirmation remain required because QEMU cannot
prove the actual mouse, display, or TTY path.

The physical operator gate now has the same two-sequence shape through
`tools/start_sophia_xmonad_pointer_focus_tty3.sh`. The wrapper guides a plain
click and key, moves focus away, guides a click-drag and different key, then
automatically checks both ordered handoffs after normal logout. Its verifier
requires two independent requests and at least press/release for the click
plus press/motion/release for the drag. It does not infer visual success: the
operator still confirms pointer selection, border movement, and text delivery
on the physical outputs.

## 2026-07-26: Native Lifetime Regressions Form One Named Gate

The remaining Milestone 9.1 regression item did not require a new lifecycle
owner. Its constituent failures already reduce through focused deterministic
tests at the boundaries that own them:

- CLI resize coordination tests compensating rollback, abandoned-pixel
  fencing, disconnect cleanup, and stale prepared-frame settlement.
- Backend startup tests output/target size mismatch, replacement, stale target
  allocation removal, and reduced target retirement.
- Backend presentation and scanout tests accepted replacement, stale callback
  rejection, cleanup retry, final displayed-owner cleanup, and repeated
  retirement as a no-op.
- Renderer lifetime tests stale CPU retirement and reusable DMA-BUF retirement
  without exposing renderer-native handles.

Validation now gives these tests one named command set. This preserves DRY
ownership: rollback remains transaction policy, target replacement remains
backend readiness, and native resource destruction remains KMS/backend state.
The gate closes the deterministic Milestone 9.1 item, but it does not replace
the same-commit physical xmonad run, whose balanced complete-target,
frame-surface, page-flip, and teardown counts remain authoritative.

## 2026-07-26: One Snapshot Now Accounts For All Primary-Frame Pixels

Compositor damage could not safely become output scheduling authority while
client and software-cursor changes remained separate. Sophia now builds one
bounded immutable output-damage snapshot for every CPU and mixed CPU/DMA-BUF
frame. It contains only pixel-relevant Engine facts: output shape and scale,
ordered opaque surface IDs, committed generations, geometry, buffer identity,
the compositor display list, and optional software-cursor bounds. It contains
no XIDs, application metadata, WM facts, protocol objects, or renderer-native
resources.

The reducer damages old and new client extents for generation, geometry, or
buffer changes; damages all involved client extents for stacking, creation, or
removal; includes old/new compositor nodes; and includes old/new software
cursor bounds. Initial presentation and output shape/scale changes force full
output. Hardware cursors keep their independent plane lifecycle. Snapshots are
bounded to 1,024 client nodes and revalidated before reduction so public record
mutation cannot bypass ID, ordering, capacity, or output invariants.

CPU and mixed frames retain this snapshot through cloning, latest-frame-wins
queueing, native export, accepted KMS submission, and page-flip retirement.
QEMU established full initial plans on both outputs and emitted separate
compositor, combined-output, and repaint records for every retired primary
frame. During pointer focus, the matching combined plan correctly became full
when the same transaction also advanced client generations; requiring a
chrome-only partial plan would have hidden client damage. The verifier now
requires the compositor record, combined record, and safe partial-or-full
decision from the same retired frame before the following key.

The planner still does not authorize partial drawing. Native destination
buffers are not yet proven preserved or reconstructed for the reported region.
The next optimization step is explicit destination-buffer age/history and a
full fallback whenever that proof is absent.

## 2026-07-26: Repaint Planning Fails Safe Before Native Optimization

The retirement-safe display-list region is not, by itself, authority to skip a
frame: client-buffer, software-cursor, output, and future animation damage must
also participate. Sophia therefore adds the next optimization boundary without
prematurely changing pixels. Engine now reduces compositor damage to an
output-local `skip`, `partial`, or `full` repaint plan. It clips every rectangle
to output bounds, coalesces only unions that remain exact rectangles, and
computes a bounded pixel count. More than 2,048 raw rectangles, more than 32
partial rectangles, or at least 60 percent output coverage fails safe to a
full-output plan. Invalid output dimensions and policies are rejected.

Deterministic tests cover clipping, exact versus L-shaped coalescing, pixel
accounting, coverage fallback, fragmentation fallback, raw-capacity fallback,
invalid inputs, and attachment to the in-flight display-list lifecycle. The
two-output QEMU xmonad run then observed empty `skip` baselines, four-rectangle
partial border creation, partial old/new focus damage, partial border removal,
and zero compositor repaint work for stable client-only frames. The M7 verifier
now rejects a focus proof without a nonempty retired partial plan.

This is planning evidence, not a claim that rendering is partial. The next
implementation stage must combine client, compositor, cursor, and output damage
into one frame plan, preserve or reconstruct the destination buffer correctly,
and use full rendering whenever buffer age or native capability is uncertain.

## 2026-07-26: Compositor Damage Follows KMS Retirement

Display-list damage previously described the difference between two lists but
had no temporal identity in the production scanout path. Advancing a single
“last list” at composition time would be incorrect: latest-frame-wins queueing
can supersede pixels, native export can fail, and a submitted buffer remains
in flight until its page-flip callback.

Engine now owns a bounded per-output display-list presentation reducer with
pending, submitted, and presented slots. CPU and mixed CPU/DMA-BUF frames carry
the immutable list that generated their pixels. A queued list compares against
the submitted list when a flip is in flight and otherwise against the
presented list. Superseding or rejecting pending work cannot change the
presented baseline. Only an accepted KMS submit advances pending to submitted;
only the corresponding accepted callback advances submitted to presented.
Legacy frames without display-list identity explicitly clear pending chrome
state rather than inheriting an unrelated list.

The two-output QEMU xmonad gate passed this lifecycle. Both outputs established
empty initial baselines. Focus creation retired four border rectangles, focus
changes retired eight old/new rectangles, border removal retired four, and
stable client-only frames retired zero compositor rectangles. The gate now
rejects missing secondary-output initialization or missing nonzero retired
damage during the click-drag focus proof. Partial redraw, frame suppression,
and KMS damage-clip submission remain later scheduling optimizations; the new
ledger supplies the retirement-safe region those optimizations must consume.

## 2026-07-26: Focus Chrome Uses The Ordinary Engine Display List

Sophia now has the first production compositor-owned visual: a minimal focused
surface border. Engine builds one bounded immutable display list from opaque
surface order, committed focus, and committed surface state. Four stable
inside-edge nodes follow the focused surface in that list. Node identity,
generation, geometry, and color drive deterministic old/new damage; unchanged
nodes produce no compositor damage. The external WM and X frontend receive no
chrome records, graphics parameters, renderer handles, or application facts.

The CPU reference path consumes the ordered surface and solid commands
directly, including clipping and exact XRGB byte output. The native mixed path
uses the same list to interleave CPU buffers, DMA-BUF buffers, and compositor
solids. EGL lowers each solid to a scissored opaque clear, avoiding texture
allocation in the frame hot path. A prepared GPU Present uses the candidate
surface state associated with those pixels; retained and CPU composition use
committed state, so a border cannot move ahead of matching client geometry.

The two-output QEMU xmonad gate passed with the new path. It observed four
border primitives on both focus targets. During the click-drag proof, WM focus,
Engine focus, and X frontend focus acknowledgment completed first; the retained
pointer records were then released, the new border frame was composed for that
same opaque target, and the following key reached it. The verifier rejects a
missing border, the wrong target, fewer than four primitives, or border
evidence that arrives after the following key. Physical DRM confirmation across
focus, resize, workspace, VT, and mixed CPU/DMA-BUF presentation remains open.

Border generation hashes only facts that change compositor pixels: geometry,
thickness, and color. Client buffer commits therefore do not create false
compositor damage or repeated evidence. Hiding focus clears retained
observation state so restoring the workspace proves a new border composition;
VT and native-recovery repaints explicitly re-emit the reduced border fact even
when geometry is unchanged. A dedicated physical verifier now requires those
workspace and VT sequences, two focus targets, a focused geometry-generation
change, nonzero mixed exports, and clean shutdown. Its pass and mutation
fixtures fail closed without claiming physical completion.

## 2026-07-26: Click Focus Requires A Cross-Authority Input Barrier

Physical pointer events were already hit-tested against Engine scene truth and
routed to opaque surfaces, but a primary press never asked spatial policy to
change focus. Consequently clicking or click-dragging another xmonad tile
could move the cursor and deliver pointer input without changing Engine, WM,
or X11 focus.

WM API v4 adds `FocusRequested`, containing only surface, output, and
workspace. The reference WM returns `FocusSurface`; the metadata-blind legacy
bridge translates the target into a private synthetic primary-button gesture
so xmonad updates its own focus stack before returning the same opaque Sophia
surface. No XID, namespace, application metadata, or raw input payload crosses
the WM boundary.

Engine now owns a bounded pointer-focus handoff. The initial press and following
motion/release records remain ordered against the selected surface, including
drag coordinates outside its geometry, until the X frontend acknowledges that
same focus. A 256-record capacity and two-second timeout fail closed. Protocol,
reference-policy, bridge, route, ordering, and timeout regressions pass. The
real unmodified-xmonad smoke also focused a requested opaque surface. Xmonad's
focus refresh re-emitted unchanged configure requests, so the compatibility
runtime discards those only for a focus-only request; the resulting Sophia
transaction contains `FocusSurface` and no placement. Physical TTY confirmation
remains the promotion evidence.

The local QEMU xmonad gate then exposed a harness-only seat regression before
WM negotiation: its minimal initramfs provides neither logind nor a seatd
daemon, so automatic libseat discovery returned `Function not implemented`.
The guest now explicitly selects libseat's direct `noop` backend. Production
sessions keep ordinary backend discovery and remain libseat-owned.

After cursor recovery, the M7 harness exposed a stale restart assertion. Its
input sequence intentionally left workspace 2 active and empty, but the host
required a new focused-surface record after restarting the compatibility
bridge. Preserving that workspace correctly produces `hidden_focus_cleared`.
The harness now accepts exactly the two valid reduced recovery states: a new
focus reconciliation for a focused projection or a new clear-focus record for
an empty projection.

The next M7 pass reached post-restart launch/close and exposed a separate
supervision classification error. Proof mode treated every secondary child
exit as fatal, including an approved action-launched terminal carrying a
launch transaction. Fixed proof witnesses still must remain alive, but
transaction-correlated action children may now exit normally in both proof and
normal sessions.

Repeated two-window startup exposed a stateful WM queue ordering defect.
Sophia treated geometry changes and earlier committed workspace state as
reasons to discard a later xmonad response. Xmonad had already processed that
request, so the rejection desynchronized the two state machines and a
following action failed with `UnknownSurface`. Response lifetime checks now
track only the opaque surfaces in the request. Each ordered response is reduced
against the latest committed workspace state, with a `ManageSurface` target
added to that current planning projection. Engine transaction validation
continues to own geometry correctness.

The same QEMU gate found that startup readiness was reduced only when an
optional timeout was configured, although bounded completion always required
the readiness result. Readiness reduction is now unconditional; the option
only supplies a failure deadline. Its post-detail frame barrier requires a new
submission only on outputs intersecting the focused surface while retaining
the initial presentation baseline for every owned output. The final M7 run
completed two-head startup in 187 milliseconds, processed 14 WM requests with
13 commits and zero stale responses, recovered one compatibility-bridge
restart, launched and cleanly closed the action terminal, logged out with zero
protocol errors or pending work, and drained both outputs.

The M7 verifier previously required at least two CPU layers in the final frame.
That contradicted the scripted empty workspace at logout. Its three-surface
peak remains independently required by committed-layout evidence; bounded
completion now proves clean external-WM lifecycle without requiring closed or
hidden windows to remain in the shutdown frame.

The unattended M7 workflow now exercises the pointer-focus barrier itself
through the existing virtio mouse. After two xterm surfaces settle, a keyboard
focus action selects the non-master tile, relative motion clamps the pointer to
the unfocused master tile, and QMP emits primary press, drag motion, and
release. The run recorded `FocusRequested` for the other opaque surface,
committed and acknowledged that focus, released all three retained pointer
records only afterward, and routed a following ordinary key to the same
surface. The final ledger contained two routed buttons, four routed pointer
events, zero stale WM responses, zero protocol errors, and no pending input.

The shared physical verifier now requires that following keyboard record as
well as request, Engine commit, X frontend acknowledgment, and retained
press/motion/release order. QEMU proves the complete software and virtual-input
path reproducibly; the real libinput device, physical DRM, and operator
interaction remain the final promotion evidence.

## 2026-07-26: Pending Pixels Must Not Replace WM Geometry

The latest normal xmonad capture exposed a temporal geometry defect that the
four-Kitty end-state verifier did not cover. After ManageSurface transaction 3
committed the two-window layout, the new surface continued to present at its
admission staging position, `1280x1426_80_60`. The existing surface presented
at its correct right-hand tile. A later Super-J focus transaction moved the new
surface to `1280x1426_0_14` without a configure, after which the geometry
remained stable. The three-window sequence reproduced the same pattern.

A strengthened real-xmonad smoke now manages three opaque layout nodes
sequentially with the physical `2560x1426_0_14` work area. It requires the
exact full-height master and two `1280x713` stack panes. The smoke passes,
proving that xmonad and the compatibility bridge return every committed
placement.

The first correction made Present admission consume the same immutable Engine
presentation layout used for composition. This prevents a newly queued buffer
from missing reprojection merely because it did not exist at the beginning of
the cycle. A following physical run proved that correction for the second
window, then reproduced the defect with the third window on workspace 2.

That topology exposed the underlying resize-quarantine race. The third window
already had the requested full-height pixel size, while the two existing
windows needed resize redraws. An ordinary third-window Present arriving while
those redraws were pending was therefore an unrequested layout observation.
The pending-layout reducer replaced the whole proposed layer with that
observation, including its `(80,60)` staging geometry and old stack rank. It
preserved new pixels by silently discarding the WM placement.

Unrequested policy-managed observations now merge only authority-owned visual
content into an existing pending layer: source, damage, generation, identity,
opacity, crop, transform, and resize capability. WM-owned geometry and stack
rank remain from the proposal. Client-positioned surfaces explicitly retain
authority-owned geometry, so an xmobar update during a resize epoch is not
lost. Deterministic regressions cover both authority assignments.

The following 77-second physical run confirmed the authority split across
seven action-launched Kitty surfaces on workspaces 1 and 2. Each ManageSurface
commit moved exactly the projected surface count, and the new surface's first
retired Present used the work-area master geometry. Three-window layouts
presented one `1280x1426` master and two exact `1280x713` stack panes;
four-window layouts presented the master plus `1280x475`, `1280x475`, and
`1280x476` stack panes. No target used `(80,60)`, and every following layout
transaction reported `moved_surfaces=0`. The run balanced 524 mixed target,
pipeline, and frame-surface lifetimes, held input dwell to 11 ms and
submit-to-page-flip observation to 47 ms, recorded no unexpected protocol
error or WM degradation, and completed cleanly.

The work-area-aware four-Kitty verifier now checks this temporal boundary for
every action-launched surface it observes. It correlates launch observation,
ManageSurface commit, active-workspace projection, first retired Present, and
the following stability transaction. Mutation fixtures reject staging
geometry, mismatched pixel dimensions, and a second geometry change.

Focused borders remain a separate compositor-chrome concern. They must derive
from committed focus and committed surface geometry, enter the ordinary Engine
frame and damage lifecycle, and never compensate for or conceal a placement
error. The renderer-neutral chrome model in `docs/compositor-graphics.md`
already defines that boundary; implementation follows physical geometry
stability.

## 2026-07-26: Clipboard Routing Is Workspace-Blind And Namespace-Explicit

The next physical workflow reported that Ctrl-Shift-C did not work on
workspace 3. The retained session did not show a swallowed chord or lost
focus: workspace 3 kept a focused surface, six selection-owner changes and 21
conversions reached the X authority, and the session exited cleanly. Kitty did,
however, report four failed selection conversions around workspace
transitions. The normal-session verifier previously rejected ownership failure
but could still accept this conversion failure.

The socket audit found a protocol-routing defect independent of Kitty and
xmonad. A client writing a property on another client's selection requestor
received the resulting `PropertyNotify` itself. X11 requires delivery to each
client that selected `PropertyChangeMask` on that window. The routed frontend
now retains bounded per-client core event subscriptions, routes property
changes to those subscribers, removes subscriptions with their client or
window, and preserves synchronous reply/event order when the requester is also
the subscriber. This state remains inside the X frontend; Engine, workspaces,
the WM, and the compositor never receive selection objects or payloads.

The same audit tightened namespace resolution. A request first resolves an
owner in its own admitted namespace and uses ordinary X11 transfer semantics.
Only the absence of a local owner permits a cross-namespace portal request.
Owner generations are globally monotonic, and portal source capture plus
target execution revalidate the exact source namespace and generation instead
of whichever namespace changed most recently. Explicit owner clear and
disconnect cleanup also retain that namespace instead of clearing an
arbitrarily ordered owner. Policy still sees only bounded facts; the runtime
executor alone handles correlated clipboard bytes.

The same-namespace socket regression now follows Kitty's material request
shape: distinct target and property atoms, requester-side property
subscription, `AnyPropertyType`, deletion, and a maximum `long_length`. The
cross-namespace regression uses the same distinct target/property and
requester subscription while proving broker-mediated payload capture and
handoff. The strict physical verifier now rejects Kitty's conversion-failure
diagnostic as well as ownership failure and requires at least two ownership
changes plus two conversions. The operator sequence now copies workspace 1 to
workspace 3 and back before continuing the normal promotion workflow. A new
physical pass remains required before promotion.

## 2026-07-26: GetProperty Length Is A Ceiling, Not A Payload

A physical same-namespace Kitty copy/paste attempt terminated the receiving
Kitty with three opcode-20 X errors. The session used the `classic_shared`
namespace profile, emitted no `CloseFocused` action, and never entered the
portal path. Kitty 0.48.0's X11 selection receiver calls `XGetWindowProperty`
with `long_length=LONG_MAX` and `delete=True` to read the complete property.
Sophia incorrectly multiplied that request ceiling by four, compared it with
the 256 KiB retained-property limit, and returned `BadValue`. Xlib's default
error handling then terminated the client.

Core X11 semantics instead define the returned length as the minimum of the
stored remainder and four times the requested length. The property reducer now
saturates the request conversion and clamps it to already bounded retained
bytes; it never allocates from the request ceiling. The 256 KiB per-value and
4 MiB table limits remain unchanged. An offset beyond the actual property
still fails with `BadValue`.

`GetProperty(delete=True)` is now one explicit authority transition. A
complete type-matching read returns the reply, removes the property, and emits
the existing deletion notification; partial reads, type mismatches, missing
properties, and failed reads preserve state. Reducer, core-dispatch,
same-namespace multi-client, and cross-namespace portal regressions all use the
maximum wire length and prove exact bytes, no protocol error, and post-reply
deletion. No Kitty, xmonad, compositor, namespace-policy, or portal special
case was added. Physical copy/paste and the strict normal-session verifier
remain the promotion gate.

## 2026-07-26: Four-Kitty Geometry Follows The Engine Work Area

The first four-Kitty run after the output-scoped frame-service change rendered
and exited cleanly, but its verifier rejected the missing pre-bar target
`1280x1440_0_0`. The retained layout proved that the expectation was stale:
the active status-bar reservation correctly changed the primary work area to
`2560x1426_0_14`. Xmonad and Engine produced one `1280x1426` master pane and
three `1280x475`, `1280x475`, and `1280x476` stack panes that covered the work
area exactly through `y=1440`.

The verifier now correlates the four-surface atomic resize transaction with its
workspace projection, obtains that output's applied Engine work area, and
derives the expected Tall geometry from the bounded rectangle. It no longer
assumes a particular output mode, origin, or absence of reservations.
Mutation fixtures use the 14-pixel reservation and reject an incomplete
projection or a one-pixel work-area/tile mismatch.

The corrected strict gate passes the retained physical run: 259 mixed exports
matched 259 target, pipeline, frame-surface creation and retirement lifetimes;
generation and recovery replacement remained zero; maximum input dwell was
20 ms, render time 20 ms, and submit-to-page-flip observation 32 ms. Both
outputs retained their valid startup lifecycle, the WM and session-control
ledgers drained, and cleanup recorded zero unexpected protocol errors. The
complete normal xmonad gate and one more consecutive four-Kitty cycle remain
before the frame-service lifecycle slice is promoted.

## 2026-07-26: Output-Scoped Frame Service Passes The Xmobar Gate

The first physical capture from frame-service commit `9b14ea9` passed the
focused unmodified-xmobar verifier. One 14-pixel top reservation reduced both
output work areas exactly; managed Kitty pixels began at `y=14`, and the bar
remained in the full-output client-positioned scene. Both button and axis
packets routed to that generic role without changing Kitty keyboard focus.
Workspace 2/1 and three VT suspend/resume cycles preserved the bar, managed
pixels, pointer, and keyboard.

The run created and retired 50 mixed composition targets, pipelines, and frame
surfaces with zero generation or recovery replacement. Both startup outputs
retained synchronous modeset proof, the WM worker drained with no pending or
rejected requests, and native suspension, session health, protocol accounting,
cleanup, input guard, and TTY restoration were clean. This closes the focused
status-bar gate and provides the first physical confirmation of the
output-scoped reducer. The four-Kitty and complete normal xmonad captures
remain required from the same commit before the lifecycle change is promoted.

## 2026-07-25: Frame Service Is An Output-Scoped Engine Reduction

The status-bar work-area run completed 162 mixed exports with balanced target,
pipeline, and frame-surface counts, but the native service still selected work
from aggregate booleans such as “some output has retirement” and “some output
has a pending frame.” That representation discarded output identity and made
fairness, primary reservation, callback stalls, and exact effect ordering
implicit in backend control flow.

The local Niri source reinforces two reusable boundaries without changing
Sophia's product architecture: frame state belongs to each output, and
scheduling policy should be testable independently from renderer and KMS
execution. Sophia does not adopt Niri's Wayland state graph, compositor
authority, protocol objects, or damage implementation. Damage history and
leased buffer pools remain deferred until the post-soak efficiency milestone.

Engine now reduces a bounded immutable observation for every output into named
effects: poll one output's retirement, submit the queued primary presentation,
or submit one output's pending frame. It validates a unique stable output set
and exactly one stable primary, orders retirement before new submission,
reserves a queued primary presentation without starving ready secondary
outputs, reobserves backend state after every effect, never reissues an effect
that failed to advance within the pass, and fails closed at a derived effect
budget. Backend-live maps those effects to mechanism and contains no runtime
selection policy.

Deterministic Engine tests cover idle, mixed retirement/presentation/pending
ordering, secondary fairness, primary reservation, stalled observation,
invalid identity sets, mutation during a pass, and budget exhaustion. The
physical promotion boundary remains unchanged: the focused xmobar, four-Kitty,
and normal xmonad gates must pass from this same lifecycle commit before the
architecture is promoted.

## 2026-07-25: Synchronous Initial Modeset Is Startup Presentation Proof

The first crash-free four-Kitty capture still failed verification because
startup forced-detached one secondary scanout after 750 ms. The trace showed
that both initial modesets had completed successfully, but startup immediately
queued an identical event-bearing framebuffer on each output and required an
asynchronous callback. The primary produced that callback; the secondary
driver did not emit an event for its redundant commit. Recovery then abandoned
a healthy buffer, recreated the DRM session, and repeated the same baseline.

Initial modeset is a synchronous KMS operation. A successful return already
proves that output's first framebuffer was committed and is not an in-flight
page flip. Native output state now records that fact explicitly. Startup
readiness accepts either this synchronous proof or an accepted asynchronous
page-flip callback, and unchanged-frame suppression no longer schedules a
redundant baseline solely to manufacture an event. Reduced per-output evidence
names `proof=synchronous_modeset`; the physical verifier requires exactly one
record for each of the two outputs. Callback requirements remain unchanged for
subsequent asynchronous commits.

The first physical run confirmed this correction: both outputs emitted
synchronous proof, startup performed no recovery or forced detach, and maximum
submit-to-page-flip latency fell from 119 ms to 36 ms. It also exposed four
previously opaque native submit failures later in the workflow. Export counts
showed three CPU attempts and 159 successful mixed exports, but only 158 total
submissions including the two initial modesets. Native submission now emits a
reduced failure record containing the output, submit stage, and generic content
class so the next capture can distinguish export, framebuffer, atomic request,
and commit failure without native handles.

The next capture recorded zero native submit failures and clean native drain,
but final completion still applied the old callback-only independence check to
the unchanged secondary output. The completion invariant now accepts exactly
two balanced forms: one successful synchronous submission with no callback or
retirement, or an asynchronous stream with equal callbacks and retirements and
one retained displayed submission. Both forms still require a nonzero export;
mixed, incomplete, or callback-imbalanced lifecycles fail closed.

The following capture completed that lifecycle cleanly. Startup reported both
outputs ready with no recovery, input queue dwell fell within budget at 91 ms,
and every native submission, callback, retirement, and cleanup balanced. The
remaining verifier failure was one 202 ms submit-to-callback interval while a
four-to-three-window close synchronously waited for two X Authority configure
delivery acknowledgements. Adjacent page flips completed in roughly 25 ms.
The kernel path is therefore not the demonstrated source of this outlier; the
owner thread stopped polling DRM events while WM control delivery blocked.
The next latency correction must make configure acknowledgement intake
incremental or otherwise keep native event servicing live during that wait.

## 2026-07-25: Persistent Native Targets Are Isolated By Render Class

The latest physical four-Kitty evidence showed that a stable output epoch
created a GBM/EGL target and GL pipeline for every mixed frame. That avoided an
older AMDGPU command-stream rejection by destroying mixed DMA-BUF state before
the next CPU upload, but replaced the hazard with frame-rate resource churn,
185 ms page-flip latency, and 291 ms input queue dwell.

Native scanout now keeps separate persistent CPU, DMA-BUF, and composition
targets for each output context. Render classes cannot leak EGL/GL state into
one another, while each class reuses its target across a stable
size/format/modifier epoch. An exported scanout buffer retains a reference to
the persistent native surface, so buffer release still precedes surface
destruction without forcing the context or pipeline to be rebuilt per frame.
An epoch change retires the affected target; a bounded retry may replace a
target after an explicitly classified EGL, GL, upload, or composition failure.

Reduced completion evidence reports total and per-class target creation plus
epoch- and recovery-driven replacement counts. The four-Kitty verifier now
requires sustained mixed composition, class-consistent creation counts, zero
replacement in the stable workload, zero launch-admission timeout, and bounded
input, upload, and page-flip latency. Local compilation and verifier mutation
coverage establish the ownership model; the physical workload and recovery
retirement proof remain open.

The first physical run of that implementation crashed on its third mixed frame
with an AMDGPU command-stream rejection. A bounded two-target experiment then
crashed at the same point: target A rendered frame one, target B rendered frame
two, and the driver rejected target A's first reuse after its exported KMS
lease had retired. This disproves both cross-class contamination and an
in-flight front-buffer lease as the complete cause. The retained Mesa/AMDGPU
path cannot safely reuse a composition EGL context after the current DMA-BUF
import sequence.

Mixed composition has therefore returned to the previously proven fail-safe:
destroy its context, pipeline, and target after every exported frame. CPU and
direct DMA-BUF targets remain class-isolated and persistent. Reduced evidence
requires composition creation and retirement counts to match mixed exports,
while epoch and recovery replacement of the other classes remains zero. This
restores startup correctness without falsely closing the lifetime milestone.
The next optimization must change the import/synchronization architecture, not
extend an unsafe context pool.

The same milestone now names click-to-focus separately from pointer motion and
client click-drag delivery. A primary click on an unfocused visible surface
must first select that surface through the blind WM interface, then deliver
ordered client input to the newly focused target. This is pending work; no
xmonad- or application-specific policy belongs in Engine.

## 2026-07-25: Native Service Must Preserve The Resize Epoch Barrier

The first physical four-Kitty run isolated the remaining corruption. Xmonad
requested three 1280-by-480 stack buffers, and Kitty produced them, but the
asynchronous native service scheduled the queued Presents while the WM layout
was still pending. The per-batch defer flag did not survive that service
boundary. Those buffers were compared with the old 1280-by-720 or
1280-by-1440 geometry and rejected; the 300 ms policy deadline then expired,
and the fourth surface remained visible at its 80-by-60 staging offset.

Presentation scheduling is now persistently blocked for the lifetime of the
pending layout. KMS retirement continues, but queued Presents cannot enter
scanout until the layout commits. A timeout rejects the complete quarantined
queue, preserves the prior displayed surfaces, does not focus the uncommitted
surface, and leaves admission eligible for one retry after rollback pixels
arrive. The xmonad bridge uses the existing bounded two-second maximum so a
multi-client resize is not failed merely because three clients repaint
serially. This state is surface- and transaction-based; no application-specific
branch was added.

The first validation run exposed a necessary refinement: the startup buffer
may precede the first WM configure. Holding that wrong-size Present withheld
the feedback Kitty needed before allocating the configured buffer, producing a
startup deadlock. A Present whose pixels conflict with the pending requested
size is therefore completed as a controlled skip immediately. Only matching
pixels, or pixels for a moved-only surface with no size request, enter the
quarantine.

The successful four-window transition also invalidated cloning the primary
mixed frame onto every output. The primary composition was 2560 by 1440 while
the secondary scanout target was 1920 by 1080, producing one controlled export
failure per primary Present and a final teardown error. Mixed X11 composition
now stays on its owning primary output; other outputs retain their own
output-sized frames until Engine has an explicit per-output scene projection.

## 2026-07-25: Present Configure And Pixel Size Define Resize Readiness

The physical three- and four-Kitty trace disproved the earlier placement-only
resize fix. Xmonad requested 1280-by-720 and 1280-by-480 tiles, but every
retired Kitty source remained 1280-by-1440. The frontend emitted core
`ConfigureNotify` only, although Mesa's DRI3 loader selects Present
`ConfigureNotify` and uses it to update drawable dimensions. The session then
mistook the already-updated window geometry for matching pixels and committed
the layout before the client had allocated a resized DMA-BUF.

Engine-driven X11 resize now emits the standard mask-selected Present
configuration event in addition to core configure and expose delivery. Live
resize readiness resolves the actual DMA-BUF or CPU-buffer dimensions by
handle; target window geometry is no longer pixel evidence. Present submissions
for a partial multi-surface resize remain queued while the layout is pending,
and a mismatched source is rejected rather than clipped and reported as a
successful resize. Configure evidence is named delivery rather than
acknowledgement because X11 clients do not acknowledge core configure events.

The same trace exposed an independent output-service defect: the primary output
continued retiring mixed frames while output 2 retained only its startup
submission. Mixed and retained-resume frames are now queued on every active
output so each KMS head participates in the presentation lifecycle.

## 2026-07-25: Retained Frames Follow One Atomic Layout Snapshot

A physical three-Kitty xmonad run kept submitting and retiring KMS frames but
showed a blinking third tile, then only two visible tiles after a TTY
round-trip. The trace proved that client DMA-BUF sources remained valid while
individual Present records crossed a two-to-three-window layout transition with
different geometry. Per-surface page-flip health was therefore not sufficient
full-state evidence.

The production runtime keeps retained frame ownership separate from
placement. Every cycle projects queued and displayed Presents through one
stack-ordered WM layout snapshot before composition. DMA-BUF pixels remain
unscaled; clipping is not accepted as proof that a client completed a resize.
Present source offsets remain frontend facts carried to the generic backend. The first displayed
buffer stays busy until replacement instead of receiving an immediate Idle.
Native resume also queues a complete retained mixed frame, so quiet clients do
not have to repaint merely to recover visible contents after a VT switch.

This is protocol-neutral visual state. The Engine and renderer contain no
Kitty or xmonad policy. Physical promotion remains open until three windows
stay visible before and after a TTY2/TTY3 round-trip.

## 2026-07-24: Empty Desktop Input Remains Engine-Owned

The first physical xmonad run after controlled stale-Present retirement proved
that Kitty exited normally and both outputs received correction frames, but the
remaining cursor appeared frozen. The live-session policy selected
`ShortcutsOnly` when the startup child had exited and both focus and proof
surface were absent. That mode intentionally discarded pointer events even
though no client surface remained.

An empty desktop now uses the ordinary full physical-input path. Engine still
consumes global shortcuts first, keyboard focus rejects application keys, and
scene hit-testing produces no client pointer target; only the session-owned
hardware cursor moves. This is protocol- and application-neutral and adds no
empty-desktop object or Kitty branch.

The same run exposed four `RANDR:GetOutputProperty` `BadValue` errors that also
occurred in the Kitty-only profile. Conventional RandR property atoms are now
created with the X server's shared atom table. Valid outputs return an empty
property for unavailable hardware identity and `CARDINAL(0)` for
`non-desktop`; Sophia does not invent EDID or connector data. Invalid outputs
and atoms remain protocol errors. A two-output real-Kitty smoke retains this
compatibility evidence without weakening normal-session error policy.

## 2026-07-24: Stale Present Retirement Is A Controlled Settlement

An xmonad physical run exposed a normal client-exit race: Kitty exited after
Present transaction 778 entered KMS but before its page-flip callback. The
surface-removal batch advanced Engine state, so retirement correctly rejected
the prepared candidate as `RejectedStaleSurface`; the live backend incorrectly
promoted that controlled result into a fatal session error.

Prepared retirement now remains ordered through the production coordinator:
Engine revalidates first, then the backend maps a committed result to
Present `Flip` and a rejected result to `Skip`, followed by `Idle` and exact
resource release. A rejected retirement never becomes a stable or focusable
surface and the current Engine snapshot is projected unchanged. Missing or
duplicate presentation resources remain fatal because they indicate broken
ownership rather than an ordinary asynchronous race.

CPU-frame preservation now follows the post-batch active transaction set
instead of a removed pre-batch DMA-BUF surface. Removing the last GPU surface
therefore queues the current CPU snapshot behind the in-flight frame, allowing
the asynchronous service to replace exited-client pixels after retirement.
Focused regressions reproduce the removal-before-retirement ordering without
physical hardware and require `Skip`, `Idle`, unchanged Engine state, and
exactly-once resource cleanup. This supersedes the earlier wording that stale
prepared retirement invokes no feedback callback: it invokes no successful
`Flip`, but it must still settle backend and protocol lifetimes.

## 2026-07-24: Architecture Conformance First Slice

- Audited production and test source layout against `docs/style-guide.md` and
  `docs/dod.md`; added an executable exact-path exception ledger so existing
  debt is visible and new unreviewed violations fail validation.
- Advanced the blind WM contract to API v3. Launch requests now carry only an
  opaque nonzero `SessionApplicationId`; executable names and application roles
  remain session-owned. Existing CLI evidence labels remain stable.
- Removed the application-protocol name from the renderer's built-in default
  cursor without changing its dimensions, hotspot, or pixels.
- Removed allocation and sorting from Engine input hit testing and cached the
  backend visual runtime's input-layer projection at authority-update
  boundaries.
- Split the production visual runtime into a 795-line transaction/presentation
  facade plus focused native-scanout and asynchronous-service modules. Present
  feedback now crosses the backend/session boundary through a bounded owned
  queue, and KMS retirement performs explicit Engine commit, protocol feedback,
  and output projection steps instead of callback-owned mutation.
- Removed visual-state seeding from backend and CLI startup. The runtime begins
  empty, accepts initial generation zero only through normal Engine authority
  commit, and rejects a forged nonzero initial generation.
- Centralized authority-transaction layer templates under Engine, preserving
  namespace identity and stack order for both production and deterministic
  backend paths. Moved protocol cursor coverage to the crate boundary and
  removed implementation-only legacy-WM builder and atomic-helper inline tests.
- Replaced direct native-scanout library printing with `tracing` while
  preserving the stable evidence message bodies emitted through CLI-installed
  subscribers.
- Converted native EGL/GBM pixel and lifecycle diagnostics to `tracing` and
  removed the process ID from DMA-BUF lifecycle output. Pixel evidence message
  bodies remain compatible with the existing verifier.
- Converted X authority dispatch, socket-write, close, and input diagnostics to
  `tracing`. Request-byte prefixes, file descriptors, raw XIDs, and key details
  are now redacted; diagnostics retain only opaque client IDs, protocol
  opcodes/counts, routing decisions, and bounded timing.
- Converted the private legacy-WM opcode trace to `tracing`; the source-layout
  ledger now has no direct-library-printing exceptions.
- All workspace targets compile with all features. Focused protocol, Engine,
  renderer, backend, WM, and bridge tests pass. Strict workspace Clippy remains
  a tracked migration gate because pre-existing native renderer argument
  bundles and style warnings are not yet clean.

## 2026-07-19: X-Centric Product Direction And Wayland Retirement

Sophia is an X-centric product built on a protocol-neutral architecture. X11
is the sole supported application protocol and the native X Server Frontend is
the product vehicle. Engine transactions, routed input, namespaces, portals,
rendering, and presentation remain independent of X11 object identity so a
future translator can be evaluated without moving authority.

The Smithay-backed Wayland frontend is retired from the workspace, CLI,
launcher, dependencies, documentation contracts, and validation gates. Its
source, tools, fixtures, last Kitty SHM evidence, and controlled linear
DMA-BUF evidence are frozen under `research/wayland`. Those results proved that
the Engine boundary was not X-shaped; they do not create an ongoing Wayland
compatibility promise.

Future application protocols are not deferred backlog. A translator or native
Sophia interface requires named product evidence, an explicit specification
amendment, existing authority boundaries, and bounded maintenance cost. Sophia
will not import another protocol ecosystem's shell, workspace, input,
presentation, or compositor-extension architecture.

## 2026-07-19: Milestone 8 Close, Sequence, And Soak Results

Firefox's intermittent XCB abort was an output-order race: asynchronous writers
could snapshot sequence N, wait behind a reply for N+1, then emit an event
carrying N. Request-sequence publication and every protocol, control, and input
event snapshot are now serialized by the output socket lock.

Close routing selects an exact `WM_DELETE_WINDOW` target, its nearest
protocol-advertising ancestor, or the only unambiguous protocol window. A
client with no cooperative target follows the bounded terminate path. The soak
also established bounded launch/focus settlement, close retries during busy
layout proposals, stale-surface WM resynchronization, and explicit discard of
the terminal logout chord's undeliverable release batch.

The final unattended QEMU run lasted 1,891,936 ms and completed 22 terminal,
Firefox, and GTK launcher cycles with 66 closes, 11 recovered bridge restarts,
all six Firefox semantic stages, zero unexpected protocol errors, no pending
WM/action/input state, and clean application, frontend, namespace, Xauthority,
and native presentation teardown. Three consecutive mixed-application runs
also passed before the soak.

## 2026-07-18: Semantic Firefox Gate And Xmonad Focus Reconciliation

The M8 page proof no longer treats changing pixels or fixed host sleeps as
evidence for browser behavior. The X frontend reduces title changes to property
name and byte length only; XIDs, namespaces, and title bytes remain inside the
authority. Six monotonically sized witnesses prove load, keyboard, clipboard,
primary selection, resize, and dialog stages. Separate reduced counters require
at least two selection-owner changes and two conversions before the session can
publish a complete Firefox record.

The first evidence-driven close attempt exposed a bridge synchronization bug.
Xmonad could focus a newly managed synthetic window after producing its layout,
while the bridge returned without reconciling the private X server's current
focus. Engine therefore retained the previous opaque surface and a subsequent
close-focused action targeted the wrong client. The bridge now queries its own
synthetic focus after each quiet layout cycle and translates that opaque window
back to `FocusSurface`; no XID or client metadata crosses the WM boundary.

Application launch waits for both process start and a committed layout. Close
waits for the WM action acknowledgement and a zero-status managed-process exit.
The soak repeats those evidence-driven terminal, Firefox, and launcher restarts
instead of fixed-delay chords, and its verifier requires twenty restarts of
each application, sixty committed closes, the semantic Firefox proof, zero
unexpected protocol errors, and clean final ownership. Offline verifier
regressions and the complete all-features test suite pass. Real mix and soak
promotion remain pending their unattended QEMU runs.

## 2026-07-18: Milestone 8 Evidence And Offline Application Shape

Normal sessions now identify every startup and action-launched application by
its approved registry ID, terminate the complete process group when its leader
exits, and treat every reduced X protocol error as fatal. Separate health and
post-teardown records preserve schema-14 while exposing pending WM/action/input
state and confirming frontend, namespace, Xauthority, and application cleanup.

The retained Firefox probe forces native X11, uses an isolated profile and
bounded process-group termination, and requires nonzero pixels with
`first_error=none`. A real host run now passes after adding bounded
`MIT-SHM CreatePixmap`, core `GetImage` and `ReparentWindow`, XKB `GetControls`,
larger bounded image/property payloads, and semantic classification of the
window-zero probes Firefox deliberately tolerates. The offline
page advances visible content and monotonically sized title witnesses after
keyboard, clipboard, primary-selection, resize, and dialog stages without
putting title bytes in evidence. Explicit `xmonad-m8-mix` and
`xmonad-m8-soak` scenarios use an optional Firefox/vkcube/Lavapipe image profile
and have strict positive plus negative fixture verifiers. The self-contained
image builder now resolves an installed xmonad and recursively includes and
validates Firefox's ELF dependency closure. The host Firefox proof completed
396 requests across 45 opcodes, two committed runtime surfaces, 5,971,968 CPU
buffer bytes, nonzero pixels, and no unexpected protocol error.

The first real mixed QEMU run reached xterm and Lavapipe but exposed an
application-size/recovery seam: `vkcube` retained its 500x500 default while the
post-bar xmonad layout requested 640x720. The guest now starts it at the stable
tiled size, and the host harness waits for per-application start/exit evidence
instead of fixed sleeps. The mix and 30-minute soak remain open until those
revised real gates pass; fixture verifiers and the complete offline test suite
are green.

## 2026-07-14: Explicit Final Scanout Retirement

The post-completion X11 allocator failure exposed a teardown ownership gap.
Persistent presentation deliberately retained the last displayed submission,
but the bounded session drained only the in-flight submission. Returning from
the loop therefore dropped the last GBM owner implicitly without first
retiring its framebuffer, mode blob, and imported GEM handles through the live
DRM device.

Persistent runtime shutdown now explicitly retires that displayed submission,
retries any reduced cleanup while the DRM device and renderer context are
still alive, and refuses clean completion if either in-flight or cleanup state
remains. Lifecycle diagnostics bracket the terminal retirement without logging
native handles. On X13, the focused backend regression and native-feature CLI
build pass, followed by ten of ten uninstrumented exact-text native stability
runs with clean evidence and no allocator diagnostic. The three operator-typed
runs remain the physical acceptance gate.

## 2026-07-14: Portal Requests And Grants Are Separate State

Portal policy decisions no longer need to double as execution authority. A
generic I/O-free lifecycle now retains deadline-bound request facts separately
from single-use grants. Allowed requests create active grants bound to source
generation and broker generation; completion, executor failure, expiry,
namespace disconnect, owner change, and broker restart have explicit terminal
transitions. A caller supplies monotonic time, the active set is capped at 64,
and no payload or operating-system handle enters this state. The first broker
IPC slice will use this reducer for every portal kind while clipboard remains
the first concrete executor.

## 2026-07-13: Core Keyboard Map Offsets And Semantic Input Evidence

The X13 stability workload exposed a false-positive input proof: xterm visibly
echoed repeated `^@` control notation, never printed its `received:` result, but
the session passed because fourteen events flushed and later pixels changed.
The `GetKeyboardMapping` decoder read the request padding byte as
`first_keycode` and the real first-keycode byte as `count`. Xterm therefore
cached keysyms for keycodes 0 through 7 while Sophia delivered normal core
keycodes such as 39 for `s`; Xlib translated every delivered key to NUL.

The decoder now reads the protocol body fields at bytes 4 and 5. Both wire byte
orders have regression coverage, and the real-xterm input smoke requires its
shell to receive exactly `sophia`. The live proof likewise uses an owner-only
result channel and emits schema 11 only after exact terminal bytes, flushed
delivery tokens, changed focused-surface pixels, and presentation all agree.
Pixel change alone is no longer input evidence. Kitty remains a separate
Wayland client proof; its X11 mode needs modern extension coverage beyond this
core-keyboard regression.

On X13, the standalone real-xterm smoke and all ten native repetitions reported
exact six-byte `sophia` receipt with no `^@` substitution. Nine native runs
exited cleanly. The tenth emitted complete schema-11 presentation and cleanup
records, then glibc reported `corrupted size vs. prev_size` during process
teardown. That preserves the allocator lifetime issue as a separate unresolved
bug; it does not weaken the now-semantic keyboard result.

## 2026-07-13: X11 Input Target Race And GBM Owner Drop Order

Two fresh dedicated-X13 milestone attempts stopped at different seams. The first
routed and flushed all fourteen physical key events but received no later xterm
pixels. The second reached terminal content and Engine-applied focus, then
aborted with `free(): invalid pointer` before input readiness. Both runs restored
`keyd`, released DRM ownership, and left no Sophia process or core file.

Inspection found that Engine `FocusSurface` control and client-selected keyboard
delivery shared one atomic X window. A late focus command could replace xterm's
VT child with its top-level surface window after the child selected key events.
Those states are now separate: focus control updates only the surface window,
while key delivery retains the latest client window selecting key events and
uses the focused surface only as a fallback.

The native CPU-upload path keeps each locked GBM front buffer and its originating
surface alive through KMS retirement. That owner's destruction order is now
explicit: release the front-buffer lock first, then release the surface. A
shared persistent GBM/EGL surface was tested and rejected after it reproduced a
pre-input guest crash; the proven per-frame EGL surface path remains in place.

`tools/run_x11_live_session_stability.sh` adds bounded normal, lifecycle-trace,
GDB, and core-capture modes. Timeout evidence now distinguishes no client
update, stale buffer generation, unchanged composition, and missing native
presentation. An X13 QEMU rerun now reaches changed pixels for all fourteen
physical key events and exits without the allocator abort. Its independent
pointer-selection proof still times out with observed-but-unrouted events, so
repeated local TTY runs and the full physical milestone gate remain required.

## 2026-07-13: Explicit Portal Taxonomy

The portal milestone began by removing two ambiguous protocol encodings.
`Screenshot` had represented both still capture and recording, while URI-open
requests were labeled as notifications and distinguished only by a type hint.
`PortalTransferKind` now has explicit clipboard, drag-and-drop, file-handoff,
screen-capture, screen-recording, URI-open, and notification values. Each maps
directly to its namespace capability. Reducer and codec regressions cover every
kind; established codec numbers for the five existing values remain stable,
with recording and URI-open using new tags. Request/grant lifecycle separation
is the next portal slice.

## 2026-07-13: Explicit X Session Profiles And Map Isolation

The live X launcher now selects `classic` or `confined` explicitly. Classic
remains the default shared-X group. A confined run receives a fresh registry
namespace with explicit zero portal capabilities, and those immutable facts
flow through every connection admission. The session status record exposes the
selected profile and directional capability bitsets without exposing namespace
identity.

The first simultaneous confined socket proof assigned two clients distinct
namespaces and exposed a real leak: `MapWindow` changed lifecycle state without
checking the runtime resource table. The runtime now performs namespace-aware
window lookup before mapping, so the second client receives native `BadAccess`;
classic same-namespace mapping remains valid. The following socket expansion
closes properties, selections, metadata, event selection, and routed input.

The next socket expansion found the same missing-boundary pattern in property
and selection paths. `ChangeProperty` previously keyed a foreign XID under the
requester's namespace and could emit a metadata candidate without checking the
window owner. Selection ownership and conversion likewise trusted the owner or
requestor XID instead of the admitted namespace. Runtime/dispatch now validate
all three before mutation or portal construction. The wire proof requires
`BadAccess` for foreign property and owner changes, normal
`SelectionNotify(property=None)` for foreign conversion, and zero metadata
candidates.

The final confinement expansion found that the socket bridge updated its
authority-local keyboard target from `CWEventMask` before dispatch authorization.
A rejected foreign event subscription could therefore redirect later input in
the requester's private worker to another namespace's XID. Event-target changes
now occur only after namespace validation. The drawable validator also
classifies a resource once so a foreign window's `CrossNamespaceDenied` is not
overwritten by a failed pixmap fallback. A routed simultaneous-client proof
requires native `BadAccess`, sends a broker-addressed key to the requester, and
verifies that its event target remains the local root; the broker's separate
queue regressions prove delivery stays client-specific. This completes the
bounded Milestone 1 confinement matrix; full XKB, XI2, focus, and grab semantics
remain Milestone 3 work.

The final admission-lifecycle gap was targeted supervisor revocation. Concurrent
workers now report only their session-issued `ClientAdmissionId` to frontend
supervision and retain a cloned socket solely as a disconnect handle. A
`RevokeAdmission` service command shuts down that one socket; the worker still
owns writer shutdown, private-route removal, connection-ledger cleanup, surface
removal observation, and admission-lease revocation in that order. A
pre-admission command is retained until the matching worker attaches, closing
the allocation/worker-registration race. A simultaneous classic-client
regression revokes admission 1, observes its surface removal and inaccessible
old window, then creates another window through the uninterrupted peer. This
completes the namespace/admission foundation and makes the portal broker plus
X11 clipboard the active milestone.

## 2026-07-13: Live Xauthority Ownership

The live X session no longer relies on an unauthenticated owner-only socket. Its
supervisor obtains a fresh 128-bit cookie from the kernel for every run, writes
a standard `FamilyLocal` `MIT-MAGIC-COOKIE-1` record with mode `0600`, syncs the
complete record before exposing its path, passes `XAUTHORITY` to both launched
terminals, and removes the file through explicit and drop cleanup. A private,
owner-only `XDG_RUNTIME_DIR` is preferred; the random, create-new owner-only
file remains safe when the system temporary directory is the fallback.

The frontend validates that cookie before invoking session admission. Policy
sees only `MitMagicCookie1` provenance and kernel peer credentials, never the
secret. A regression proves bad cookies do not invoke policy, while the accepted
connection is admitted once and revoked once. Fresh per-session generation is
the rotation boundary; confined launch credentials remain future policy work.

## 2026-07-13: Per-Connection X Admission Boundary

The native X frontend now calls a protocol-neutral session policy after setup
authentication and before allocating X client or resource-range identity. The
policy receives only the bounded authentication method and kernel Unix peer
credentials; it never receives raw cookie bytes. A successful decision returns
an immutable `ClientAdmissionContext` retained in an admission lease. Native
X11 setup failure represents denial, and teardown or any early worker error
revokes the lease after route and resource cleanup.

The live classic session backs that policy with its session-owned
`NamespaceRegistry`: it requires a peer UID matching the effective session UID,
allocates a distinct admission per connection, and intentionally assigns those
admissions the same classic-shared namespace. This removes the listener-wide
identity shortcut without weakening classic X semantics. Confined launch and
targeted supervisor revocation are now implemented as described above.

## 2026-07-13: X11-First Namespace And Portal Critical Path

Sophia's next architecture work is the native X Server Frontend, not broader
Wayland protocol or DMA-BUF coverage. The two-xterm frontend already proves
bounded concurrent workers, client-attributed transactions, targeted input,
Engine composition, and KMS presentation. Its next risk is no longer basic
visibility; it is admitting clients into the correct trust domain before more
X11 semantics depend on a hardcoded listener namespace.

The chosen dependency order is session-owned namespace admission, then a portal
broker with X11 `CLIPBOARD`/`PRIMARY` as its first complete adapter, then XKB,
grabs, Engine-derived output/resize, and standard presentation semantics.
Classic shared-X intentionally retains same-namespace resource visibility.
Confined sessions use distinct namespaces and explicit capabilities; XID ranges
remain creation/cleanup ledgers rather than access-control lists.

At this stage Wayland/Smithay stayed supported under maintenance gates. The
2026-07-19 retirement decision above supersedes that status. XLibre remained
frozen historical evidence and a possible future provider only if measured
native-X gaps later justified its authority and maintenance cost.

## 2026-07-13: Kitty DMA-BUF Direct-Scanout Boundary

Enabling the experimental DMA-BUF global for guarded Kitty failed before a
usable native presentation and surfaced the misleading scheduler invariant
`native frame was neither submitted nor retained for a later submit`,
disconnecting Kitty. Sophia's current DMA-BUF route is direct KMS scanout,
whose exporter requires the client buffer to match the physical output exactly.
An arbitrary Kitty toplevel therefore cannot be a valid client for that route,
regardless of the exact buffer that reached the failed run.

This is an architecture boundary, not evidence that Kitty can use the current
direct path. The controlled full-output XRGB producer remains the direct
DMA-BUF lifetime proof. The interactive Kitty harness now deliberately does
not advertise DMA-BUF and continues to prove native SHM composition, input,
recovery, and latency. The next DMA-BUF milestone is GPU composition: import
an arbitrary window-sized client DMA-BUF, scale/blend it into a Sophia-owned
output-sized render target, retain it through the target page-flip retirement,
and only then release the client buffer. Only that route can support Kitty
without requiring fullscreen, output-sized buffers.

## 2026-07-12: Controlled DMA-BUF First-Frame Heap Corruption

The first real controlled DMA-BUF run reached Sophia's full-size 1920x1200
client frame and recorded `sophia_wayland_frame` with `buffer=dmabuf`. The
native process then aborted with `corrupted size vs. prev_size`, disconnecting
the producer before presentation retirement or buffer release could be proven.
This is a renderer/resource-ownership safety failure, not DMA-BUF evidence.

The producer itself now follows the compositor's initial xdg configure rather
than assuming 640x480, and uses a driver-supported explicit linear GBM
allocation. Those corrections moved the test past allocation and target-size
rejection; they did not make the native import/presentation path safe. The
300-frame lifecycle and three-Kitty promotion gates remain blocked pending
allocator/lifetime diagnosis.

The next controlled rerun uses a GDB-backed diagnostic mode with explicit
DMA-BUF stages. The importer now detaches the EGLImage from its GL texture and
finishes that detach before destroying the EGLImage and dropping the imported
client FD. This makes input-image teardown independent from the retained GBM
front-buffer owner.

The GDB-backed three-frame rerun passed: each 1920x1200 frame completed EGL
image creation, rendering, texture detach, image destruction, KMS submission,
page-flip observation, scanout retirement, and client buffer release. The
session exited normally with three imports, three retirements, three callbacks,
no cleanup debt, and a 14 ms maximum submit-to-page-flip interval. The
GDB-backed 300-frame lifetime proof then completed with 300 imports,
submissions, page flips, and retirements, no allocator diagnostic or cleanup
debt, and the same 14 ms maximum submit-to-page-flip interval. A subsequent
normal release 300-frame run nevertheless aborted with `corrupted size vs.
prev_size` after frame 8 (an earlier normal run reached frame 13). This makes
the fault timing-sensitive: the GDB result is diagnostic evidence, not a
completed lifecycle gate. A release-timing trace then completed all 300 frames
with ordered ownership stages and an 18 ms maximum submit-to-page-flip interval.
One uninstrumented rerun and then three separately retained uninstrumented
300-frame runs all completed normally: each reported 300 imports, 300 callbacks
and retirements, no cleanup debt, no surviving process, and a 14 ms maximum
submit-to-page-flip interval. The next full promotion preflight nevertheless
aborted on its first uninstrumented DMA-BUF frame with `free(): invalid pointer`,
before Kitty started. A later post-repair normal run also aborted after frame 2.
The persistent CPU-upload texture was therefore isolated from imported images:
each import now gets a transient per-frame texture, which is deleted after
`glFinish` before EGLImage destruction. The repaired three-frame proof passed
with three imports, three retirements, and a 16 ms maximum interval. A normal
core-capture 300-frame run and three separate uninstrumented normal 300-frame
runs then all completed: every run had 300 imports, callbacks, and retirements,
no cleanup debt, no surviving process, and 14–16 ms maximum latency. This meets
the bounded controlled gate, while retaining the normal-stability wrapper as a
regression guard for the earlier intermittent abort. The next required evidence
is three guarded native-SHM Kitty runs; a later GPU-composition milestone must
precede any real-Kitty DMA-BUF runs.

## 2026-07-12: DMA-BUF Performance Gate and Renderer Safety Boundary

The current native Wayland/Kitty presentation route is SHM-backed and stable
enough to serve as the production fallback, but the latest hardware result was
about 110 ms input-to-presentation and therefore missed the 100 ms budget.
DMA-BUF descriptors are admitted only as a bounded single-plane linear subset;
their native import and presentation path remains explicitly experimental.
There is no passing real-hardware DMA-BUF result at this point.

A controlled external Wayland producer now allocates linear XRGB8888 GBM
buffers, alternates them only after `wl_buffer.release`, and waits for each
frame callback. The first hardware gate uses three frames; the second uses 300
frames to exercise import, presentation, feedback, and retirement lifetime.
Only after both pass may the three independent guarded real-Kitty runs begin.
Those acceptance runs remain on SHM until GPU composition exists. DMA-BUF stays
non-default until a real-Kitty GPU-composition log proves input, recovery,
presentation, and the 100 ms budget.

The current CPU composition copy and 2 ms native idle cadence are a safety
boundary, not merely a tuning choice. Removing the copy or tightening that loop
has reproduced native renderer/exporter heap corruption on hardware. Further
latency work must isolate that ownership fault before changing either setting.

## 2026-07-12: Native Wayland Replaces The Kitty Compatibility Runtime

Sophia's production Kitty path now terminates a private Wayland socket through
the Smithay-backed Sophia Wayland Authority. Engine input routes and layer
records are protocol-neutral; keyboard focus and pointer hit-testing remain in
Engine, while the authority translates accepted routes into `wl_keyboard` and
`wl_pointer` delivery. A real Kitty 0.47.4 process completes the headless smoke
with `DISPLAY` removed, changing nonzero SHM frames, and no X server process.

The installed launcher now uses the native Wayland/KMS session and retains the
independent Ctrl-Alt-Backspace recovery interlock. XLibre is excluded from the
production dependency graph and launcher; its frozen crate, CLI, patches,
scripts, fixtures, and notes live under `research/xlibre`.

The native-scanout session advertises a bounded single-plane linear/implicit
XRGB8888/ARGB8888 DMA-BUF subset. Accepted buffers cross the renderer boundary
as owned descriptors. Their experimental native import/presentation route is
now gated by the controlled first-frame/lifetime proof; arbitrary Kitty buffers
need GPU composition before they can enter this route. It is not yet recorded
as passing hardware evidence. Wayland
presentation and buffer-release feedback must remain withheld until the
matching KMS submission is observed as presented. The next evidence gate is the
controlled proof, followed by text, navigation, pointer, resize, sub-100 ms
presentation, clean exit, and TTY recovery in real Kitty.

## 2026-07-12: Installed-Session Input Recovery Interlock

The first installed Kitty operator run exposed a control-plane failure: scanout
reached a visible terminal, but normal keyboard delivery failed and the session
had no reliable local escape. The wrapper had stopped `keyd`, placed the TTY in
graphics/raw mode, and disabled XLibre VT switching while relying on the same
live input path for `exit`. A reboot then removed the runtime-directory logs.

The installed launcher now refuses graphics takeover until a separate libinput
guard observes one complete Ctrl-Alt-Backspace chord. A second chord requests a
graceful in-session exit without depending on focus, then lets the independent
guard force bounded wrapper cleanup if the live loop is wedged. Session groups
and XLibre receive TERM followed by a bounded KILL fallback; KD mode, termios,
and the previous `keyd` state are restored afterward. Full Ctrl-Alt-Fn switching
remains deferred until Sophia owns a correct VT/DRM suspend-and-resume cycle.

The input path now emits privacy-safe one-shot stages for poller readiness,
committed focus, observed keys, authority routing, and XTEST injection. Bounded
synthetic text also traverses Engine focus and evdev mapping instead of entering
the authority channel directly. Logs retain the latest and previous run in the
user state directory so a reboot does not erase the failing seam. The dedicated
TTY physical typing and deliberately wedged-session recovery run remain the
hardware acceptance gate.

Guard bring-up also exposed that libinput's path context can reject stable
`/dev/input/by-path` aliases before invoking Sophia's open callback. Native
input admission now canonicalizes configured absolute paths and honors the
requested read/write access mode without logging either path. The rebuilt QEMU
guest passes the 300-tick dual-output gate with all 14 keyboard events routed,
five pointer events routed, changed pixels, 112 native submissions, clean
retirement, and no callback rejection or cleanup debt.

The isolated recovery scenario now exercises the guard contract directly with
a virtio keyboard: one complete Ctrl-Alt-Backspace chord arms the independent
guard, a second chord triggers both the guard and the live loop, and both
virtual outputs drain with no in-flight scanout or cleanup debt before poweroff.
This is deliberately not recorded as host VT restoration evidence. A separate
same-build rerun of the default physical-text QEMU scenario routed all 14 key
events but timed out waiting for the expected xterm pixel change, so that
content regression remains open independently of the recovery gate.

The first guarded physical-TTY rerun then proved the interlock on hardware: the
first chord armed before takeover and the second returned to the text TTY
without rebooting. Persistent logs showed physical poller readiness, committed
focus, observed and routed keys, and XTEST injection, but Kitty received no
usable input. The installed XTEST protocol definition identifies the stopped
seam: FakeInput's `time` field is a delivery delay in milliseconds, while the
adapter had supplied libinput's monotonic event timestamp. The compatibility
injector now requests zero-delay delivery and synchronously checks each XTEST
request instead of treating a queued unchecked request as successful.

That hardware rerun also exposed the compatibility renderer's full-frame cost:
45 frames in 25.9 seconds, 8.74 MB read and hashed per frame, a 397 ms libinput
lag warning, and a maximum 798 ms submit-to-page-flip interval. The live bridge
now maintains X Damage trackers and a CPU-buffer base, emits one clipped packed
patch per damaged surface, and falls back to replacement only for initial,
resized, missing-base, or at-least-half-surface updates. XTEST uses an
independent connection and worker so capture cannot block channel draining.
The optimized dummy-XLibre proof presented routed xterm input in 17 ms with
three steady patches, 1.26 MB total readback, and a 9 ms maximum capture.

## 2026-07-11: Isolated Virtio-GPU Session Evidence

Sophia now has a direct-kernel QEMU initramfs builder and a headless session
harness. The guest has no storage or network device, uses serial control and an
unconnected Unix-domain VNC display sink, and owns emulated virtio-gpu and
virtio-keyboard devices. It starts udev, mounts devpts, launches real xterm,
opens the virtual input nodes through libinput, and runs persistent native
scanout for an exact `--max-ticks=300` budget without host DRM or VT access.

The passing run completed 300 session ticks, 42 native submissions, 41 steady
retirements, 41 accepted page-flip callbacks, two nonzero terminal exports,
injected terminal pixel change, and zero submit failures, retire failures,
rejected callbacks, saturated callback queues, in-flight frames, or cleanup
debt. The strict verifier accepted `/tmp/sophia-qemu-session.log`.

Guest bring-up exposed two real cross-driver defects. AddFB2 fallback passed a
linear modifier while clearing `DRM_MODE_FB_MODIFIERS`, which violated the DRM
crate's flag/value invariant; the implicit fallback now wraps the same planes
with `modifier=None`. Virtio-gpu also reports repeated zero page-flip sequence
values. Native CRTC routes now normalize driver values into strictly increasing
Sophia-local serials, preserving stale-event rejection across repeated values
and 32-bit sequence wrap. Focused regressions cover both fixes.

The guest virtual keyboard is present and opens through libinput, but the
current proof uses Sophia's bounded X key injection for the pixel-change check.
QMP-driven virtual-key input remains the next isolated input proof.

## 2026-07-10: Roadmap And Documentation Review

The xterm compatibility stream currently reaches `ImageText8`, emits four
ready `SurfaceTransaction` values, commits them through runtime, and passes the
deterministic composition/scanout lifecycle proof. Core drawing now updates
bounded XRGB8888 software buffers, renderer-live composes those bytes, and the
native EGL adapter can upload the composed frame into a GBM front buffer.
The TTY3 content proof now exports an exact composed xterm checksum through the
native GL/GBM path, submits that buffer to KMS, observes accepted page-flip
retirement, and drains cleanup. Requested and exported checksum evidence match;
the remaining presentation work is persistent-session ownership, not pixel
upload correctness.

The active milestone is therefore persistent session ownership, hardware
terminal-content presentation, and physical keyboard delivery. Injected core
key events already produce changed pixels in a real xterm. Pixel bytes remain
outside Sophia Engine and the blind WM protocol.

The persistent launcher now owns an explicit local display, one xterm, the X
Authority server, one live backend runtime, and the latest composed CPU scene.
A bounded real-xterm run passes repeated authority/runtime ticks and injected
pixel change. Building it exposed and fixed static drawing generations: X
Authority now advances a window generation after each emitted visual
transaction, so long-running Engine commits remain contiguous. Native scanout
now joins this owner behind `--native-scanout`: the same loop queues composed
CPU frames for GL/GBM export, polls native page flips, retires tracked KMS
submissions, and drains cleanup. Reduced schema 6 evidence records successful
submits, deferrals and failures, submit-to-page-flip latency, maximum in-flight
age, callback pressure, nonzero exports, authority drops, and cleanup debt.
The non-native repeated-xterm regression and strict verifier fixtures pass.

The strict persistent hardware proof now passes. Corrected counters first
exposed River ownership and then two real lifetime defects: the runtime retired
the newly displayed framebuffer instead of the previously displayed one, and
the shutdown loop retired a frame while immediately submitting another. The
persistent mode now performs a blocking initial modeset without waiting for an
event, retains the displayed owner until a later accepted page flip replaces
it, and has a retire-only idle/shutdown path. A 30-second TTY3 run completed 46
submissions with 45 steady retirements, six nonzero exports, zero dropped
authority batches, zero rejected callbacks, zero transition failures, and no
in-flight or cleanup debt. A subsequent bounded run also reports nonzero
submit-to-page-flip latency after fixing timestamp association.

Host iteration remains unnecessarily disruptive because River must release the
only DRM card. The next stability harness should boot Sophia in a headless QEMU
guest with `virtio-gpu`, serial control, virtual keyboard input, and no guest
compositor. Use that guest for the 300-tick and repeated-session proofs; retain
the AMD TTY run for final driver and modifier evidence.

Physical keyboard plumbing now enters the persistent owner through explicit
libinput event nodes. `InputFocusState` in Sophia Engine validates a seat's
focused surface against committed visual state before X Authority maps evdev
keycodes and modifiers to core events. The TTY keyboard node opens in a bounded
run, but the noninteractive validation process observed zero physical events;
an operator typing run is still the evidence gate.

Wayland was gated behind the operator-grade X session. Before considering it,
Sophia would connect the live session to the documented generic X11 WM bridge. The
bridge is an embedded minimal X server with synthetic windows; a configured
legacy WM is layout policy only and receives no physical input, raw metadata,
namespaces, or real client XIDs. Sophia Engine speaks only its generic WM IPC
and does not know which legacy WM the bridge supervises.

The xmonad source is cloned at `~/src/xmonad` commit `a9a8b5c` as the first
compatibility reference. It is not vendored and is not a Sophia runtime
dependency. The embedded server and real two-window xmonad process proof pass.
The remaining gate is to feed opaque live-session surface snapshots through the
same generic bridge socket and apply validated proposals to presented surfaces.

## Active Questions

- What is the smallest immutable admission record that lets the supervisor bind
  listener policy, peer credentials, cookie validation, session generation,
  namespace profile, and capabilities without exporting secrets?
- What bounded broker IPC keeps portal decisions and grant lifecycle pure while
  allowing runtime executors to retain X selection context, payload bytes, and
  later OS handles?
- Which XKB, grab, RandR, resize, and presentation gaps must close before the
  proven two-xterm hardware path can honestly become `session` evidence?

These questions remain probe-driven: implement the first observed missing path,
then rerun the relevant real-client smoke.

## 2026-07-11: Readable And Resizable Xmonad Session Path

The first physical xmonad attempt exposed that the earlier pixel-change proof
was not a visual proof: unimplemented core drawing painted damage bounds white,
and the partial fixed glyph table rendered ordinary punctuation as question
marks. The operator stopped the session without treating that run as milestone
evidence.

X Authority now preserves GC raster values, executes the xterm-used text,
fill, line, clear, copy, and image paths against bounded XRGB buffers, and
covers printable ASCII in its deterministic fixed raster. The real-xterm smoke
scans materialized pixels for the expected `Sophia` glyph sequence and reports
`ascii_marker_match=true`; nonzero bytes alone no longer pass it.

CPU drawing publication now distinguishes full replacements from tightly
packed damage patches. Resize keeps a buffer handle's generation monotonic,
publishes one correctly sized replacement, and applies later patches in order.
With fixed size constraints removed, the real-xmonad headless session completes
one configure acknowledgement, commits the resized layout and focus, and
observes changed pixels after injected input without authority backpressure.
The dedicated-TTY visual rerun remains the final gate.

## 2026-07-11: Real Xterm Through Generic Xmonad Policy

`sophia-live-session` now supervises an arbitrary Sophia WM socket process via
generic executable/argument flags. With `sophia-x11-wm-bridge` selected as that
process, the live session sends one opaque xterm surface to real xmonad, Engine
validates the response, and the committed placement drives composition,
hit-testing, backend visual state, and scanout. A headless integrated run proves
one moved surface, committed Engine focus, and a later injected terminal pixel
change. No xmonad identity enters Engine or the live-session policy path.

The Engine-to-X-Authority control seam now supports bounded configure/focus
commands and reduced acknowledgements keyed by `SurfaceId`. Probing arbitrary
full-output xterm resizing exposed a repaint loop in the core-drawing path, so
the first real one-client gate pins min/max size to the established live buffer
and uses xmonad for placement, stacking, and focus. Removing that constraint is
tracked explicitly rather than overstating resize compatibility.

## 2026-07-11: QMP Keyboard Proof And Presentation Boundaries

The isolated session no longer uses Sophia's internal core-X key injector for
its input claim. The guest announces readiness only after xterm pixels and
Engine-owned focus are stable. The host then sends `sophia` and Return through
QMP `input-send-event`, virtio-keyboard, the kernel input path, libinput, Engine
focus validation, and X Authority. The passing run observed and routed all 14
press/release events, changed later xterm pixels, completed exactly 300 session
ticks, submitted 46 native frames, retired 45 steady page flips, and drained
without rejected callbacks, failed transitions, or cleanup debt. Tick counting
pauses for a bounded five-second physical-input window so readiness at the last
scheduled tick cannot race QMP delivery.

The guest also exposes virtio-mouse and libinput maps pointer events to a
separate Engine device ID. The completed pointer slice performs QMP word
selection in the typed xterm. Five motion/button events pass through libinput,
Engine surface-only hit-testing/focus, and core X MotionNotify/Button events;
all five route and a second terminal pixel change is observed. The first drag
attempt exposed that targeting the last mapped X window was insufficient even
though all input reached Engine. Pointer events now carry only the routed
Sophia surface, and X Authority resolves that surface through its internal
surface/window table. This preserves the authority boundary: Engine never
receives or interprets the client XID.

Native presentation now has independent per-output scanout ownership, damage,
frame clocks, in-flight state, and retirement, proved with two QEMU heads. The
physical multi-connector AMD gate remains. Fixed-refresh evidence requires each
output to follow its own page-flip timeline without overlapping submission. VRR
remains a hardware proof gate: the property contract and Engine eligibility
policy exist, default off, but activation and fixed-refresh fallback still need
capable hardware evidence.

## 2026-07-11: Bounded Per-Output Timelines And Two-Connector QEMU Topology

Engine output discovery is now bounded to 16 descriptors. Backend assembly no
longer advances one global deterministic clock: it seeds an independent clock
for each discovered output using that output's fixed refresh rate. A separate
presentation registry tracks pending damage, one in-flight frame, exact
retirement, and the last retired serial per output. Two-output regressions prove
that 60 Hz and 120 Hz timelines advance independently, one output cannot submit
over an unretired frame, and a mismatched retirement cannot clear ownership.
These are scheduling invariants; the clock is not yet driven by DRM vblank.

A single virtio-gpu device configured with two scanouts exposed two connector
objects but only one connected connector, so it was rejected as multi-monitor
evidence. The accepted harness uses two isolated virtio GPU devices with one
scanout each. The guest reports two connectors and both connected; Engine
discovers two and creates two presentation timelines. That topology was the
prerequisite for native multi-output ownership, which is recorded below.

## 2026-07-11: Dual-Output Native Presentation And Fixed-Refresh Vsync

The persistent runtime now owns a bounded table of output-scoped frame targets,
callback intake, scanout submissions, displayed buffers, cleanup debt, and
retirement state. Native selection deterministically assigns disjoint
connector/CRTC/primary-plane chains, groups page-flip routes by DRM card, and
supports explicit selections so one card cannot silently resubmit its first
connector for every output.

The isolated QEMU session owns both virtio GPU outputs. Output 1 presents the
terminal while output 2 presents a deterministic Engine proof marker in the
extended desktop region; their checksums must differ. The 300-tick gate requires
nonzero per-output exports, submissions, callbacks, and retirements, plus zero
callback rejection, cleanup debt, overlapping submission, or non-monotonic
page-flip phase. Keyboard and pointer proofs remain mandatory in the same run.

VRR property discovery recognizes connector `VRR_CAPABLE` and CRTC
`VRR_ENABLED`. The Engine decision defaults off and permits enable only for one
opaque, unoccluded fullscreen surface without overlays or required composition.
Atomic page-flip request construction fails closed if VRR is requested without
the enable property. Activation and fallback remain an AMD hardware gate;
virtio-gpu is not accepted as VRR evidence.

The physical VRR gate now has a dedicated two-phase runner and strict reduced
evidence verifier. During implementation, the proof exposed that the native
page-flip builder carried `VRR_ENABLED`, but the modeset branch ignored the
same policy request. Modeset request construction now supports the property and
fails closed when its handle is absent. `tools/vrr_hardware_proof.sh` derives an
Enabled decision for one opaque, unoccluded fullscreen surface and commits
`VRR_ENABLED=true`, then derives an Ineligible decision for an overlay-present
scene and commits the fixed-refresh `false` fallback. It requires presented and
retired callbacks for both phases. The destructive AMD run is still pending
because it must be performed from the dedicated TTY, not the active graphical
session.

`tools/operator_keyboard_hardware_proof.sh` similarly packages the remaining
operator gate without guessing an input node. The operator supplies a stable
`...-event-kbd` path, waits for the physical-input readiness marker, and types
the expected lowercase proof text. Existing persistent-session evidence rejects
the run unless physical keys route through Engine focus and later xterm pixels
change.

## 2026-07-11: Exact Operator Input Evidence

The first AMD operator attempts exposed two proof-harness defects rather than a
new authority boundary. `keyd` exclusively owned the AT keyboard, so opening its
physical event node succeeded while libinput observed zero events. With `keyd`
stopped, Engine routed 27 events, but the original five-second deadline still
expired before a later xterm transaction changed the composed checksum.

The combined helper now detects an active `keyd`, stops it through an explicit
interactive `sudo sv down keyd`, and installs an EXIT trap that restores the
service. It has no separate Enter-to-begin prompt, preventing that confirmation
key from becoming the first exact-proof event.
The physical proof requires exact press/release pairs for the configured
lowercase text plus Return after Engine focus routing and evdev-to-core-X
translation. It gives the operator 15 seconds to complete that bounded sequence,
then starts a separate five-second pixel-settle deadline. The scanned-out xterm
shows the expected input, and readiness is withheld until those nonzero prompt
pixels are page-flip-confirmed on the primary output. Keyboard delivery freezes
after the exact Return release so operator retries cannot weaken the evidence.
Schema 2 input evidence records
expected events, matched events, and the later pixel change. The AMD acceptance
run now passes: all 14 events matched, xterm pixels changed, and the one-output
native session completed 62 submissions, 61 callbacks/retirements, 22 nonzero
exports, and zero overlap, phase, callback, transition, or cleanup failures.

Requiring a nonzero prompt baseline then exposed that the earlier QMP pixel
claim was a false positive: late prompt drawing changed the initial blank frame,
while routed key events still targeted the last mapped top-level X window.
Xterm selects core key events on its VT child through `CWEventMask`. Sophia X
Authority now parses that bounded value from `CreateWindow` and
`ChangeWindowAttributes`, retains the selected X window inside the authority,
and uses the last mapped window only as a fallback. Input readiness also
requires 500 milliseconds of quiescence after nonzero prompt pixels so event
selection cannot race the external sender. A rebuilt strict QEMU run
then matched all 14 events and changed pixels after the fully drawn prompt before
continuing through pointer and dual-output evidence. Raw X window identity never
crosses into Engine or WM state.

The first AMD VRR attempt then exposed a property-name mismatch. The connector
advertises the kernel-standard lowercase `vrr_capable`, while the selected CRTC
does expose uppercase `VRR_ENABLED`. Discovery had searched for uppercase
`VRR_CAPABLE` and therefore rejected capable hardware before building an atomic
request. The lookup now uses `vrr_capable` with the old uppercase spelling kept
only as a compatibility fallback for deterministic fixtures.

Non-destructive inspection after that correction reports connector 100, CRTC
86, `VRR_ENABLED` present, and `vrr_capable=0`. The current eDP panel is not VRR
capable, so activation/fallback evidence cannot be produced on this hardware.
The gate remains open for a connector reporting capability `1`; Sophia does not
override the value or treat a property contract without capability as proof.

## 2026-07-12: Temporary XLibre Compatibility Provider For Kitty

Kitty's installed X11 backend requires XKB and a working OpenGL context, while
Sophia X Authority deliberately does not yet advertise XKB, GLX, DRI3, or
Present. Pretending that Kitty was another core-drawing probe would therefore
produce a launcher that connected but could never render.

The first usable compatibility checkpoint instead reactivates the historical
XLibre bridge as an explicitly temporary protocol authority. XLibre runs on the
dummy video driver with software GL, no physical input devices, no TCP listener,
and a private MIT cookie. A persistent XComposite adapter owns the XIDs and
named pixmaps, converts readbacks into opaque `XLibrePrototype` surface
transactions, and never exposes client identity to Engine or the WM. Engine
continues to own physical input, focus routing, composition, frame scheduling,
and KMS. Core key events return through a bridge-private XTEST adapter until the
Sophia-owned X Authority has native GPU-buffer coverage.

The first real headless run used Kitty 0.47.4 against XLibre 1.25.1.8. It
materialized one 925 KB nonzero Kitty surface. Capture checksum deduplication
reduced a four-second run from 29 repeated batches to six actual pixel changes;
injected `sophia` plus Return then changed the composed checksum and completed
in 2.6 seconds. Native TTY presentation remains the operator gate.

The first installed-session input proof then showed that capture correctness
alone was insufficient: Kitty echoed typed characters several seconds late.
The launcher had used a debug build, the session cloned and repeatedly scanned
each 1280x720 frame, physical input was polled only after rendering, and native
export recreated its EGL/GL setup for every frame. The launcher now runs the
release binary; XLibre sessions acquire libinput on a bounded worker; the main
loop drains input before waiting for X transactions and again before composing;
CPU composition borrows source storage, row-copies clipped spans, and computes
its checksum/nonzero count in one pass; and the native renderer reuses one EGL
context and GL pipeline per output. KMS still receives a fully completed GL
frame because the atomic path does not yet provide an explicit native fence.

Schema 9 records the maximum composition, input-dispatch gap, queue depth and
dwell, upload, and persistent-resource counts. The final Kitty dummy rerun
presents input in 40 milliseconds with 8-millisecond CPU composition and
11-millisecond MIT-SHM capture. The stricter QEMU final-key-to-primary-output
measurement is 37 milliseconds. The dual-output QEMU proof
creates exactly two native targets and pipelines with zero recreations, drains
155 page flips without cleanup debt, and confirms that PRIME GEM cleanup treats
the driver's already-closed `EINVAL` result as idempotent success. Degraded
XGetImage remains operational but is rejected for interactive evidence.

The next operator run exposed a keymap mismatch hidden by ordinary typing.
Sophia correctly translated Linux input codes with the evdev `+8` convention,
but device-less dummy XLibre had selected its legacy `xfree86` keycode table.
Letter positions overlap between those tables; navigation positions do not, so
evdev keycode 111 (`Up`) arrived as `Print`. The private server now loads the
evdev XKB rules before launching a client and fails startup unless Up, Left,
Right, and Down resolve at keycodes 111, 113, 114, and 116. Sophia X Authority's
minimal core map now advertises the same navigation keysyms for direct clients.

## 2026-07-14: Engine Topology, Authority XKB, And Resize Quarantine

Milestone 3 now has three explicit boundaries. First, live Engine output records
become a validated, generation-bearing, at-most-16-output snapshot; X setup and
populated RandR CRTC/output/mode replies derive from it without exposing KMS
object identity. Dynamic RandR subscriptions and events remain separate work.

Second, Engine sends physical input as a `RoutedInputRequest` containing its
selected Sophia surface and global/local coordinates. The X frontend resolves
the owning worker, then a dedicated authority thread owns per-seat xkbcommon
state using a bounded explicit RMLVO configuration. `XKEYBOARD` remains
unadvertised until its map/name/state request surface is implemented.

Third, an X resize transaction whose pixels match a pending requested size is
quarantined with its CPU update. Neither can mutate the committed scene while
the old geometry is active. When every requested surface is ready, the staged
geometry and pixels replay together; timeout discards them and retains the last
committed scene. This closes the path that could display a large white drawing
update at the old top-left geometry, but hardware resize promotion still needs
an operator proof and rollback evidence.

## 2026-07-14: Probe-Backed GTK Startup And Real SHM Pixels

The native frontend now advertises the measured XKB and XI2 startup subset,
including XI version/device discovery, client pointer/focus queries, event
selection, and optional device-property reads. Zenity consequently advances
through normal window creation and software drawing with no X protocol error.

That probe exposed the blank-block cause: `ShmPutImage` validated the segment
but discarded its offset and payload, then materialized a zero-filled damage
buffer. A narrow SysV SHM adapter now validates segment size with `IPC_STAT`,
attaches read-only, copies only a bounded image range, and detaches immediately.
The generic pixel-proof policy now records one 310-by-233 committed surface,
288,920 nonzero bytes, and `first_error=none`. This is software-pixel evidence,
not completion of interactive GTK input, the full XI2/grab contract, or
Milestone 3 hardware promotion.

## 2026-07-14: One Session XKB Description And Stable RandR Identity

The native frontend now compiles one immutable xkbcommon snapshot from the
session RMLVO. Core `GetKeyboardMapping`, XKB `GetMap`, and per-seat event
translation consume that configuration instead of combining a handwritten US
wire map with an independently compiled state machine. The live command accepts
bounded `--xkb-rules`, `--xkb-model`, `--xkb-layout`, `--xkb-variant`, and
`--xkb-options` overrides; a German-layout regression proves that core and XKB
views change together.

RandR CRTC and output identities now derive from Engine `OutputId`, while mode
identity derives from the mode tuple. Reordering a topology snapshot therefore
does not renumber an unchanged output. Focus state is also namespace-local and
window destruction resets only its namespace. Dynamic RandR event diffs,
complete XKB state/name notifications, grabs, and XI2 event delivery remain
Milestone 3 work.

The follow-up dynamic path now acknowledges newer Engine snapshots, populates
`GetMonitors`, and sends mask-selected RandR screen, CRTC, output, and resource
notifications through each client's bounded protocol queue. A deterministic
`--inject-output-size=WIDTHxHEIGHT` live-session hook applies a validated
generation update after client startup, so update behavior can be retained as
evidence without requiring a physical connector hotplug.

The live resize rollback fence is now an exported coordinator rather than
private layout bookkeeping. It owns committed sizes, monotonic compensating
transaction IDs, abandoned-size filtering, and disconnect cleanup. Integration
tests cover successful advancement, timeout rollback construction, rejection of
late abandoned pixels until the old size is confirmed, and cleanup while a
rollback is pending. The live layout uses this coordinator for its existing
geometry-plus-pixels quarantine and compensating configure path.

Core input grabs now have connection identity and namespace-scoped authority
state instead of validation-only request handling. Active pointer/keyboard,
passive key/button with Any detail/modifier conflict checks, implicit button,
owner-events routing, synchronous freeze with bounded deferred input and
`AllowEvents`, ungrabs, and namespace-local `GrabServer` ownership all clean up
on disconnect. Engine still chooses the ordinary target surface and local
coordinates; the authority redirects only when X grab semantics require it.
XI2 generic-event delivery remains the next input-compatibility boundary.

That XI2 boundary now advertises XGE 1.0 and XI 2.0, reports master pointer
button/valuator classes plus the master keyboard key class, retains bounded
per-client selection masks, and emits selected Key, Button, Motion,
Enter/Leave, and Focus generic events. Device events preserve Engine-provided
root/local coordinates as FP16.16 values and follow core grab redirection. One
input delivery acknowledgement is returned only after the writer flushes the
core event and every selected XI2 record generated from it. Raw, touch, and
gesture events remain deliberately outside Milestone 3.

## 2026-07-14: XKB State, Names, And Subscriptions

The X authority now implements the generic XKEYBOARD 1.0 state/name path rather
than a toolkit-specific startup exception. `GetState` reports the last
authority-translated effective modifier state, `GetNames` publishes interned
component atoms derived from the configured session RMLVO, and bounded
`SelectEvents` parsing persists each client's StateNotify detail mask. Modifier
transitions emit the standard 32-byte StateNotify record only when the selected
state detail changed. Focus/hierarchy policy and retained classic/confined
session evidence remain separate open gates.

Window input routing no longer treats event-mask update order as focus. The
connection records CreateWindow parent links, mapped state, and ConfigureWindow
sibling/stack modes. Engine-selected target surfaces begin core propagation at
their owning window, ancestor selection is bounded against malformed cycles,
and root focus resolves through the current mapped stacking order. Scene-level
restack acknowledgement remains an Engine integration/evidence gate.

Retained live-session completion is now schema 12. It binds each completion to
its `classic_shared` or `confined` namespace profile and records whether the
deterministic Engine topology update was applied. The paired Milestone 3 runner
executes the same guarded two-xterm proof once per profile; its verifier requires
the confined startup record to have zero request and publish capabilities, both
runs to include an applied output update, and both to satisfy the existing
startup, composition, input-flush, presentation, resize, and cleanup checks.
The output-update acknowledgement now also carries the number of RandR records
queued to live subscribers. Schema 12 retains that count, and promotion rejects
an accepted topology update that reached no X11 client.

The paired runner now also requests a deterministic one-shot X11 surface
resize after both terminal surfaces have published. The live layout sends the
client-targeted ConfigureSurface command, validates the matching control
acknowledgement, and keeps the new geometry quarantined until a transaction
with matching resized pixels arrives. Schema 12 reports `surface_resize` only
after that commit; the promotion verifier requires the configure acknowledgement
and pixels marker in both namespace profiles.

The topology path now opens a dedicated authenticated RandR witness before the
Engine update, uses a reply-producing core request as a subscription barrier,
and reads back the resized ScreenChangeNotify record. This replaced the earlier
timing-dependent assumption that xterm itself would subscribe. The witness is
closed before frontend drain; a two-xterm headless live smoke then completed
with four queued RandR records, a matching wire event, committed resized
pixels, and clean process teardown.

Milestone 3 promotion no longer accepts the synthetic-input default. The paired
runner requires readable physical keyboard and pointer event nodes, exact
physical `sophia` plus Return input, flushed delivery, presented text pixels,
and a pointer-driven pixel change in both profiles. Schema 13 separates
automated terminal-content readiness from total operator interaction time, so
the two-second startup budget measures startup rather than typing speed.

## 2026-07-14: Retained Paired Milestone 3 Session

Fresh X13 runs under classic shared-X and a newly allocated zero-capability
confined namespace passed `tools/verify_live_session_milestone3_evidence.sh`.
Both schema-13 completions retained two live CPU layers, exact physical
`sophia` plus Return delivery, pointer-routed pixel changes, matching accepted
authority/runtime transaction counts, four authenticated RandR notifications,
committed configure-plus-pixels resize, native presentation, and no in-flight
or cleanup-pending KMS state.

Classic completed with 94 ms startup readiness, 13 ms maximum composition,
22/22 routed deliveries flushed, and 0 ms measured input-to-presentation.
Confined completed with 90 ms startup readiness, 13 ms maximum composition,
38/38 routed deliveries flushed, and 0 ms measured input-to-presentation. The
operator-bounded elapsed times include deliberate physical interaction and do
not replace the schema-13 startup metric. The ignored retained logs live at
`.evidence/remote-target/tmp/sophia-milestone3-{classic,confined}.log`.

## 2026-07-14: Milestone 4 Buffer Lifetime Foundation

Milestone 4 now has an explicit reduced buffer boundary. Protocol-visible
DMA-BUF descriptors use opaque buffer and fence identities, admit at most four
planes, accept only bounded XRGB8888/ARGB8888 dimensions and byte ranges, and
contain no native renderer objects or file descriptors. Native plane and acquire
fence FDs enter a renderer-private registry. That registry refuses duplicate or
malformed registrations, blocks submission behind an unsignaled acquire fence,
and releases ownership only after page-flip retirement, rejection, or disconnect.

X software drawing now publishes a new immutable handle for every accepted
generation rather than patching a buffer already visible to Engine. External
tests retain the earlier generation and prove its bytes do not change. A matching
CPU lifetime reducer keeps the last committed handle through stale retirement and
rejection, releases the previous handle only after the replacement page flip,
and drains disconnect ownership exactly once.

MIT-SHM now advertises a real extension event base and encodes the standard
Completion event layout when PutImage requested notification and the request was
accepted. The completion is not emitted for rejected window updates. The
workspace all-feature test gate passes offline on X13. Standard DRI3/Present
SCM_RIGHTS transport, client-visible Present feedback, mixed GPU/CPU composition,
and retained Vulkan hardware evidence remain open; the private SOPHIA-PRESENT
prototype is not promoted by this checkpoint.

## 2026-07-14: Standard DRI3/Present Transport Checkpoint

The X authority now negotiates the standard `DRI3` and `Present` extension
names at version 1.2 without routing them through the private
`SOPHIA-PRESENT` opcode. The Unix request reader uses `recvmsg` for the fixed
X11 header, captures up to four SCM_RIGHTS descriptors with close-on-exec set,
then completes the ordinary request payload read. Descriptor ownership is
RAII-bound and unexpected FD arity terminates the malformed connection instead
of leaking or guessing ownership.

The first admitted standard DMA-BUF request is DRI3 `PixmapFromBuffer`. Its
wire decoder preserves the pixmap/drawable and bounded storage metadata, marks
the request as requiring exactly one FD, accepts only 32-bpp XRGB8888/ARGB8888
shapes whose stride and declared storage cover the image, and records only the
authority-owned pixmap identity. The native FD remains borrowed through the
socket trace seam for renderer-side duplication and never enters authority
runtime state. DRI3 fences, PresentPixmap/events, and the live renderer handoff
remain the next transport checkpoint.

## 2026-07-15: DRI3 1.2 Vulkan Transport Proof

The X11 socket output boundary now sends a bounded byte record plus up to four
SCM_RIGHTS descriptors. Standard DRI3 `Open` obtains a duplicated render-device
FD only from the live backend provider and returns it in a one-FD reply; neither
the authority runtime nor Engine stores a device path or native handle.

Mesa's DRI3 1.2 startup required `GetSupportedModifiers`, modifier-bearing
`PixmapFromBuffers`, and the small XFIXES region lifecycle used by Present. The
portable modifier reply advertises linear plus the implicit-modifier sentinel,
and the multi-buffer decoder retains bounded plane strides, offsets, and the
wire modifier in the reduced DMA-BUF descriptor.

The first Vulkan failures were caused by Unix-stream FD association rather than
an AMD modifier. A single `sendmsg` can attach descriptors to bytes preceding
the X11 request that consumes them. The server now queues ancillary FDs in
stream order, leaves them pending across no-FD requests, and drains exactly the
declared arity for each later FD-bearing request. A deterministic regression
sends two descriptors alongside an earlier no-FD XFIXES request and proves that
the following DRI3 pixmap and fence requests consume one each.

On the Void Linux X13 with Mesa RADV, the bounded DRI3 1.2 `vkcube` run remained
healthy for its eight-second proof window: 68 requests, three imported pixmaps
and fences, one accepted standard Present transaction, one committed runtime
surface, and `first_error=none`. This proves Vulkan transport into the Engine
transaction seam; it does not yet claim native KMS presentation of the Vulkan
pixels.

## 2026-07-15: Reusable Renderer-Private DMA-BUF Sources

The renderer lifetime boundary now distinguishes a persistent DRI3 pixmap
source from one in-flight presentation. Plane FDs remain renderer-private and
reusable across Presents, while every presentation receives duplicated plane
and acquire-fence ownership in the existing bounded registry. Page-flip
retirement removes only the in-flight ownership; explicit source removal or
disconnect releases each persistent source once.

External tests use a real xshmfence to prove that an unsignaled acquire fence
holds submission, a trigger makes the presentation ready, page-flip retirement
allows the same source to be presented again, an in-use source cannot be
removed, and disconnect cleanup is idempotent. The complete offline all-feature
workspace suite passes with this reusable lifetime model. Live-session import,
mixed CPU/GPU composition, and page-flip-driven Present feedback remain open.
The X frontend also exposes a cloneable protocol-only feedback router that can
emit Present Complete and Idle after the broker moves into its service thread.
It is intentionally not attached to the current CPU fallback submission: doing
so would acknowledge a page flip that did not contain the imported Vulkan
pixels.

## 2026-07-15: Milestone 4 Live-Presentation Handoff

Commit `11f93ee` leaves Milestone 4 at the boundary between proven protocol
transport and unimplemented native GPU presentation. The frontend publishes
DMA-BUF registrations, fence registrations, and Present submissions through
`XAuthorityObservedTransactionBatch`. `LiveDmaBufPresentationRegistry` owns the
reusable source and per-Present FD model, and
`XServerFrontendProtocolRouter` owns protocol-only completion delivery. No
persistent-session consumer currently connects those pieces, so the bounded
`vkcube` result remains Engine-transaction evidence rather than proof that its
Vulkan pixels reached KMS.

The current live-session assembly is also an explicit architecture debt.
`PersistentNativeScanout` and `PersistentCpuScene` remain in the CLI command;
the latter retains a CPU-only `SurfaceId` projection outside the normative
Engine scene owner. Moving the entire session loop before proving GPU
presentation would broaden the active milestone, while wiring more durable
scene and renderer authority directly into the CLI would deepen the debt.

The chosen continuation is a narrow hybrid extraction. Establish an
Engine/backend-owned live-presentation seam, then move only DMA-BUF import,
acquire-fence polling, mixed CPU/GPU composition, KMS submission correlation,
and page-flip retirement through it. Source and fence FDs transfer immediately
into renderer-private ownership. Engine preserves the last committed
geometry-plus-pixels state while a presentation is pending or rejected. Only a
real page flip containing the imported pixels may route Present Complete, then
Idle, trigger the idle fence, and retire the presentation exactly once. Broader
CLI session-loop extraction and Milestone 5 compatibility work remain deferred
until the software-plus-`vkcube` native KMS matrix passes.

## 2026-07-15: Milestone 4 Mixed-Presentation Implementation

The narrow handoff is now implemented without moving protocol or native object
ownership into Engine. The X frontend assigns typed buffer/fence handles and
routes feedback by exact `TransactionId`. `LivePresentationResourceSession`
immediately duplicates frontend registrations into renderer-private ownership,
polls xshmfences, builds mixed CPU/DMA-BUF frames, and retains reusable DRI3
sources separately from individual Present lifetimes. The native EGL path
supports one-to-four-plane EGLImages, clipped placement, alpha blending, and a
single persistent output composition pass.

Engine now exposes a prepared surface commit for asynchronous presentation.
Preparation does not mutate committed state. Page-flip application revalidates
only surfaces touched by the prepared transaction, which prevents stale GPU
callbacks from overwriting a newer version of the same surface while allowing
unrelated CPU surfaces to continue committing. Rejection and disconnect drop
the candidate. Successful native feedback applies the candidate, routes Present
Complete with Flip mode, retires the renderer presentation and idle fence, then
routes Idle. Teardown converts remaining queued work to Skip/Idle and asserts
that no source, fence, presentation, or cleanup debt remains.

The offline all-feature workspace suite passes, including prepared-commit
merge/stale regressions, real xshmfence wait/trigger tests, repeated-pixmap and
deferred-release tests, mixed-frame backend ownership, multi-plane renderer
validation, and exact transaction routing. The schema-14 session evidence adds
mixed-export, acquire-wait, completion, idle-fence, and live-resource counters.
`tools/live_session_milestone4_hardware_proof.sh` pairs the established software
resize proof with a `vkcube`/CPU mixed session, controlled first acquire delay,
one rejected Present, required later Flip recovery, and strict teardown checks.
Its verifier passes positive and missing-mixed-export fixtures. The exclusive
TTY X13 run is deliberately still unclaimed and is the remaining Milestone 4
exit action.

## 2026-07-15: Milestone 4 Hardware Checkpoint And AMDGPU Mixed-Draw Blocker

The paired X13 gate now proves the software half after a real renderer defect
was isolated. Reusing mixed-composition GL state for the legacy full-screen CPU
upload eventually lost the AMD context. The persistent upload path now restores
its fixed full-screen quad and completes independently; the retained software
run committed the 800x600 configure-plus-pixels resize, flushed all 14 semantic
input events, reported exact text and changed pixels, submitted 36 native
frames, retired 35, and drained with no failure or cleanup debt.

The Vulkan attempt also exposed a transaction-domain mismatch. Present request
generations continue across controlled Skip, while Engine visual generations
advance only on accepted commits. Full-state Present snapshots are now rebased
to the current Engine committed generation immediately before preparation;
external regressions cover the empty baseline and a later post-Skip baseline.
This removed the stale-candidate flood and allowed the real imported image to
reach the renderer.

The remaining failure is specifically the required two-layer native EGL draw.
With the CPU background removed only for diagnosis, the same real `vkcube`
session completed 86 mixed exports and Flip completions, one controlled Skip,
87 matching Idle events and idle-fence triggers, 121 native submissions, 120
retirements, and zero live sources, fences, transactions, or KMS cleanup debt.
Restoring the CPU layer aborts Sophia inside Radeon `glFinish` with
`amdgpu: The CS has been rejected, see dmesg for more information (-2)` before
the first mixed KMS submission.

The failure survived `RADV_DEBUG=nodcc`, explicit CPU/import completion
boundaries, frame-local CPU textures, frame-local vertex buffers, and a
diagnostic layer-order reversal. Those experiments were removed; the retained
implementation keeps only the proven full-screen upload isolation, Present
generation rebase, and EGLImage sampling lifetime order. The next session
should capture the privileged kernel validator message immediately after one
failure, then reduce CPU-texture-plus-imported-image composition to a focused
native-EGL hardware regression before changing more session code. Retained
ignored evidence is under
`.evidence/remote-target/tmp/sophia-milestone4/` and
`.evidence/remote-target/tmp/sophia-milestone4-dmabuf-only/`; neither GPU log
is promotion evidence until the normal paired verifier passes.

## 2026-07-15: Milestone 4 Native-EGL Reduction

The remaining mixed-draw failure now has a bounded reproduction below KMS.
`native-egl-vkcube-mixed-smoke` launches the real native-X `vkcube` transport,
transfers its DRI3 planes through `LivePresentationResourceSession`, combines
them with the full-output CPU background, and invokes the persistent mixed
exporter directly. A watchdog parent reports child exit or timeout, while the
successful child emits schema-1 evidence only after disconnect drains all live
sources, fences, and presentations. Fixture-backed verification rejects a
missing CPU layer or cleanup debt.

Native composition now reports CPU upload, EGLImage create/bind, draw, finish,
and destroy failures separately. The mixed CPU background uses dedicated,
fixed-size texture storage and sub-image updates rather than reallocating the
fullscreen upload texture in the command stream that samples the imported
EGLImage. This is locally covered and the full offline all-feature suite passes,
but it is not hardware promotion evidence until the focused X13 smoke and the
normal schema-14 paired proof both pass. The paired wrapper now retains
privileged before/after kernel logs and driver environment identity so another
AMDGPU rejection cannot lose its validator record.

## 2026-07-15: Milestone 4 Mixed-Presentation Hardware Exit

The isolated mixed exporter passed while the persistent session aborted in the
Radeon command-submission thread. Lifecycle tracing and a GDB backtrace showed
that one CPU-plus-DMA-BUF frame drew, swapped, submitted, page-flipped, and
retired successfully; the asynchronous rejection surfaced in the following
ordinary CPU upload. Reusing the GL context across that imported-image boundary
was the distinguishing lifetime.

The native renderer now destroys the mixed frame's GL context after
`glFinish`, swap, and front-buffer lock. The returned GBM owner retains the
scanout surface independently through KMS submission and page-flip retirement;
the following CPU frame receives a fresh context. DRI3 `Open` also returns an
independently opened same-GPU render node instead of the compositor's primary
KMS node, preserving the protocol/backend ownership boundary.

The retained X13 schema-14 proof passed its strict verifier with 76 mixed Flip
completions, one controlled Skip, 77 matching Idle events and idle-fence
triggers, nine acquire-gate waits, zero submit/retire failures, and zero live
sources, fences, or transactions. The established software xterm/resize half
also passes, completing the Milestone 4 hardware exit.

## 2026-07-16: GTK3 Application Promotion Contract

Milestone 5 uses a direct, bounded Sophia-X client launcher instead of wrapping
applications in xterm. Application evidence correlates the existing live
session record with `sophia_x_application_session schema=1`: exact bounded
stdout, normal exit, reduced zero-error protocol observations, physical text
and pointer-button delivery, resize/redraw, native presentation, and clean
teardown are all mandatory. Error-only X dispatches cross the frontend boundary
as reduced code/opcode/sequence facts but never create empty Engine commits.
Zenity entry dialogs run under fresh classic shared-X and confined sessions;
the operator types without Return and completes the action with a physical OK
click.

## 2026-07-16: Zenity Probe-Driven RandR And XFixes Gaps

The current GTK3 Zenity engine probe exposed two bounded requests after its
package became available locally: RandR `GetOutputProperty` for EDID and XFixes
`SelectSelectionInput`. Sophia now returns a valid empty output-property reply
when no EDID payload is retained and validates the selection window, atom, and
three-bit event mask. The same probe showed that advertising DRI3 without a
render-device provider creates an avoidable `BadImplementation`; socket
advertisement now withholds DRI3 in that configuration so GTK selects MIT-SHM.
The repeated probe commits one surface with 288,920 nonzero software bytes and
`first_error=none`; no broader RandR property store or XFixes event expansion
was inferred.

## 2026-07-16: Sophia X TTY Recovery Is An Acceptance Gate

The first GTK hardware attempt could leave the active text VT black until a
power cycle because the X proof called the raw persistent KMS runner without
the guarded TTY lifecycle already used by native Wayland. The GTK runner now
builds and preflights before takeover, requires an independent
Ctrl-Alt-Backspace guard to arm, saves KD and termios state, runs each Sophia
session in a bounded process group, restores keyd and the console on every exit
path, and records a strict durable recovery line.

The isolated QEMU emergency gate then exposed five modifier deliveries queued
before the final Backspace trigger. Emergency completion now waits for those
deliveries to flush before frontend teardown. The repeated gate proves guard
arm/trigger, exact five-of-five settlement, clean two-head KMS retirement, zero
native cleanup debt, and clean guest shutdown.

## 2026-07-16: Milestone 5 Native Zenity Blocker Retained

Two guarded X13 classic shared-X attempts reached one Engine-owned native KMS
output and then showed a blank screen because Zenity aborted before presenting
a dialog. Both retained logs report GTK thaw-update assertions followed by
`BadRequest` at serial 304, request code 139 (`XFIXES`), minor code 0
(`QueryVersion`). The confined profile never started. The second emergency
chord restored KD mode 0, the exact termios state, keyd, and all Sophia
processes; the recovery record is complete. The earlier false keyd failure was
a service-start race, so the runner now waits boundedly for keyd after `sv up`.

The retained diagnosis was incomplete because wire-parse errors discarded the extension minor opcode and always encoded minor zero. A raw-minor trace on the X13 render-provider path reproduced XFixes request 11 (`SetRegion`) immediately after `CreateRegion`; `QueryVersion` had already succeeded. Sophia now retains extension minor codes, owns namespace-scoped XFixes region lifecycle, validates Present region references, and reclaims regions with the client resource range.

The first corrected run exposed a separate sentinel bug: raw region zero was converted with generation one and compared structurally to the generation-zero `NONE` value. Validity-based optional-resource checks fixed that rejection. The exact X13 sequence now accepts CreateRegion, SetRegion, DRI3 pixmap and fence resources, and Present with `first_error=none`. The non-KMS render-provider smoke reaches an Engine transaction but has no scanout consumer, so its remaining pixel-proof failure is expected and is not session evidence. Fresh guarded classic and confined hardware captures remain required before GTK promotion.

## 2026-07-17: GTK Input Stall Split From Scanout Throughput

The latest guarded X13 classic run presented the Zenity entry dialog but
accepted only five physical key presses before input stopped. The retained
15-second interval contained 984 X requests, including 252 outputless requests,
62 MIT-SHM PutImage requests, 31 CPU compositions, and 30 native submissions.
That showed both avoidable redraw work and socket-output lock contention, but
not a KMS deadlock: presentation continued while keyboard progress stopped.

Physical libinput collection now runs on a bounded worker instead of the
authority loop. Outputless X requests skip the shared output-stream lock,
software-only authority batches may coalesce their CPU composition while every
Engine transaction is still applied in order, and cursor-only movement produces
a composed native frame. During the pointer acceptance phase, physical Return
press and release are suppressed and reported instead of aborting the session.
Raw X request tracing and native lifecycle tracing are no longer enabled by the
normal GTK hardware runner; `SOPHIA_M5_GTK_DIAGNOSTIC=1` opts into both.

A bounded local Zenity entry proof then routed and flushed all fourteen
synthetic press/release events for `sophia` plus Return. GTK continued issuing
geometry, property, and SHM redraw requests but never exited or produced the
expected stdout before the semantic timeout. The throughput and lock fixes are
therefore retained, while GTK entry submission remains an explicit Milestone 5 compatibility gap.

## 2026-07-17: Unattended GTK Input Acceptance In QEMU

A direct-kernel, diskless, networkless QEMU guest now runs the real Zenity
entry dialog under both `classic_shared` and `confined` namespace profiles.
The host harness uses QMP only to drive virtio keyboard and mouse devices; the
guest receives those events through the normal physical-input poller. Both
profiles type exact `sophia`, observe changed pixels, route a physical OK-button
click, match Zenity stdout, exit normally, and cleanly retire both virtio-gpu
outputs with `protocol_errors=0`.

The trace-driven compatibility slices added core ChangeGC and CreateCursor,
XIChangeCursor, bounded opaque non-input SendEvent delivery, XIUngrabDevice,
and a protocol-shaped XIQueryPointer reply. It also exposed a proof-loop bug:
Return suppression was scoped to the entire pointer-proof run rather than only
the pre-selection phase. Suppression now ends when pointer selection becomes
ready, and an application proof cannot complete before its primary child exits.
The QEMU result closes the deterministic semantic gap; guarded target-hardware
classic/confined captures with resize remain the promotion gate.


## 2026-07-17: Presented Cursor Gate And Production-Loop Review

The final GTK QEMU regression corrected the earlier pointer-proof conclusion.
Pointer readiness now follows a centered cursor composition and matching native
presentation. Return remains suppressed until a physical pointer button routes,
not merely until pointer readiness. If the application proof surface disappears
before selection, the session exits through bounded cleanup. Both classic and
confined Zenity guests pass the click-then-submit sequence with normal exit and
clean two-output retirement. Fresh paired X13 evidence remains required.

A concurrent architecture review found that the authority boundaries are
implemented, but production orchestration remains duplicated. The 6,500-line
live-session command retains `PersistentCpuScene`, `PersistentBackendRuntime`,
and `PersistentNativeScanout` while Engine, runtime, and backend crates each
carry partial loop abstractions. The next architecture milestone after GTK
promotion is one protocol-neutral coordinator in
`sophia-engine::runtime_driver`: bounded authority intake, Engine
commit/preparation, composition from committed state, backend-private KMS
submission/retirement, then exact protocol feedback. CLI proof logic remains
an observer and supervisor rather than a visual-state owner.


## 2026-07-17: GTK Submit Deadlock Removed

A fresh local QEMU run reproduced the apparent post-submit blank screen after
Zenity had accepted exact physical text and pointer input. The client process
exited, but the CLI session loop synchronously called `read_to_end` on its
piped stdout. An inherited writer could therefore block the visual coordinator
forever, bypassing the 30-second session deadline and preventing native
retirement and console recovery. Application stdout now targets a private
mode-0700 capture directory and mode-0600 file. Once the child exits, the loop
reads at most 4,097 bytes from the regular file without waiting for every
inherited descriptor to close; a regression keeps a writer open while proving
the bounded read completes.

The rebuilt X13-hosted QEMU image passed both GTK profiles. Classic completed
in 4,617 ms and confined in 4,633 ms; both matched `sophia\n`, routed a
physical pointer button, reported `first_error=none`, retired both virtio-gpu
outputs, and ended with zero native cleanup debt. The initramfs builder also
requires xterm explicitly now: it can no longer silently produce a nominally
successful image whose default session scenario fails at boot-time readiness.
The guarded physical X13 resize captures remain the Milestone 5 promotion gate.


## 2026-07-17: Input Delivery Settlement Restores Bounded Sessions

The default xterm QEMU gate exposed a second post-input stall after keyboard and
pointer evidence had already succeeded. Phase tracing proved cursor composition,
KMS submission, and page-flip retirement all returned. The loop instead kept
`input_delivery_wait_started_at` populated after the exact key deliveries
settled. Because ordinary proof sessions advance `--max-ticks` only outside an
active delivery wait, a successful input proof made the session immortal; GTK
was unaffected only because its application-specific proof exits immediately.

Delivery settlement now consumes the wait timestamp exactly once. Later pointer
or emergency batches start their own bounded delivery wait and clear it after
settlement, while the initial key-flush record remains tied to the complete
14-event sequence. A regression covers the consume-once transition. The QEMU
verifier now recognizes current schema 14 and validates either native CPU export
mode: zero GL resources for the preferred direct linear GBM write, or exactly
one reusable GL target/pipeline per output for the fallback. Mixed counters,
recreation, missing uploads, latency violations, and cleanup debt still fail.

The rebuilt X13-hosted image passed every unattended profile. The strict
two-xterm run completed 300 ticks in 6,971 ms with two CPU layers, 8 ms input
presentation, 11 ms maximum composition, 40 submissions, 38 retirements, and
zero cleanup debt. Classic and confined GTK completed normally with exact
stdout, `first_error=none`, pointer selection, and clean two-output retirement.
The emergency profile armed and triggered Ctrl-Alt-Backspace, flushed all five
routed deliveries, and shut down cleanly in 187 ms.


## 2026-07-17: Backend Snapshot Ownership Moves Into Production Coordinator

`HeadlessCompositorBackendAssembly` no longer stores an independent
`Vec<CommittedSurfaceState>`. It owns a `ProductionSessionCoordinator`, and the
existing deterministic and live runtime adapters now receive and return the
coordinator-owned snapshot through one split Engine/state borrow. Public
`with_committed_surfaces`, replacement, input routing, rendering, and runtime
reports retain their behavior, but the live backend has one fewer visual-state
owner before the remaining CLI scene and native sequencing migration.

The focused Engine, all-feature live-backend, and live CLI suites pass. A rebuilt
X13 QEMU image also passed the strict two-xterm gate with two CPU layers, exact
keyboard and pointer routing, 8 ms input presentation, 40 submissions, 38
retirements, and zero cleanup debt. The confined GTK gate passed normal Zenity
exit, exact stdout, `first_error=none`, clean two-output retirement, and zero
native debt. This is an ownership migration, not the Milestone 6 exit: the
legacy runtime adapter still sequences commits and the CLI still owns
`PersistentCpuScene` and `PersistentNativeScanout`.


## 2026-07-17: CPU Pixel Storage Leaves The CLI

Renderer-live now owns a protocol-neutral `LiveCpuBufferRegistry`. It accepts
immutable replacements and packed damage patches, rejects stale generations,
missing bases, metadata changes, invalid bounds, and malformed byte lengths,
and retires unreferenced handles. The X frontend remains responsible for
read-only MIT-SHM admission and emits its existing immutable updates; the CLI
only converts those packets at the renderer boundary. `PersistentCpuScene` no
longer contains a CPU buffer map or applies pixel patches itself.

Four focused registry regressions cover replacement/patch ordering, stale
generation rejection, fail-closed malformed replacement and patch behavior,
and resource retention. The live CLI suite passes. On the rebuilt X13-hosted
image, strict two-xterm QEMU completed 300 ticks with two CPU layers, 7 ms input
presentation, 40 submissions, 38 retirements, and zero cleanup debt. Confined
GTK passed its high-volume SHM redraw path, exact text/pointer proof, normal
exit, `first_error=none`, and clean two-output retirement. The remaining
Milestone 6 scene gap is narrower but explicit: CLI still projects a
`SurfaceId` to geometry/handle table because commit and composition have not yet
been split into coordinator phases.


## 2026-07-17: Authority Commits Once Before Per-Output Projection

The persistent live backend no longer creates one authority inbox per output or
replays the same X transaction through every output Engine. The production
coordinator exposes a bounded commit phase; the primary runtime commits each
batch once, then every output consumes the same immutable committed snapshot and
the same precomputed commit observations. The late-client generation bridge now
runs once at this boundary, before the single commit, rather than priming every
output ahead of replay. A two-output regression proves both output assemblies
end on generation 6 while the runtime records one committed transaction.

The full offline all-feature suite passes. The rebuilt X13-hosted QEMU image
passed the strict two-xterm profile in 7,104 ms with 114 of 114 authority
transactions applied, two CPU layers, 7 ms input presentation, 4 ms maximum
composition, 40 submissions, 38 retirements, and zero cleanup debt. An initial
run without the centralized late-discovery bridge correctly rejected the second
xterm generations as stale; retaining that fail-closed evidence drove the fix.
Confined GTK then passed 54 SHM transactions, exact text and pointer evidence,
normal exit, `first_error=none`, 108 submissions, 106 retirements, and clean
resource shutdown. Classic GTK passed the same application contract, and the
emergency profile flushed all five routed chord deliveries before clean shutdown
in 178 ms. Composition still uses the CLI scene table and runs before
this commit phase; the next slice must compose from the coordinator snapshot.


## 2026-07-18: CPU Composition Consumes Engine Committed State

The persistent CPU path now splits authority preparation from per-output runtime
ticks. Each batch commits once, renderer pixel updates are reconciled against the
resulting immutable `CommittedSurfaceState` slice, and composition resolves
geometry, buffer handles, stacking, readiness, and proof generations from that
slice before any KMS submission. `PersistentCpuScene` retains only the renderer
buffer registry and composition evidence; its independent `SurfaceId` table and
raised-surface state are deleted. Native runtime construction also no longer
requires a pre-commit frame or a blank modeset: KMS initializes with the first
frame composed from committed state.

The full offline all-feature suite passes. A rebuilt X13-hosted QEMU image passed
the strict two-xterm 300-tick profile in 6,824 ms with 123 of 123 authority
transactions applied, two CPU layers, 8 ms input presentation, 2 ms maximum
composition, 40 submissions, 38 retirements, and zero cleanup debt. Confined GTK
passed 56 committed SHM transactions, exact text and pointer evidence, normal
exit, `first_error=none`, 107 submissions, 105 retirements, and clean shutdown.
Classic GTK also passed the final image with exact application evidence and clean
retirement. The emergency profile flushed all five routed chord deliveries and
shut down without native debt in 151 ms. The duplicate
scene milestone item is complete, while CLI ownership of runtime/scanout and
feedback sequencing remains open.


## 2026-07-18: Production Feedback Waits For Asynchronous Retirement

The initial production adapter incorrectly modeled KMS submission and retirement
as one synchronous callback. That shape could not own the real live path without
either blocking a cycle or treating submission as page-flip completion. The
contract now has separate `submit_frame` and `poll_retirements` phases. A
`ProductionRetirement` carries its originating cycle, and protocol feedback is
routed only for records returned by the retirement poll. Submission evidence and
zero or more feedback records remain distinct in each cycle report.

Engine regressions cover ordered immediate retirement, retirement-poll failure
with no feedback, feedback failure after retirement, and a frame held across one
cycle then retired on a later poll. The live closure adapter exposes the same four
callbacks. The full offline all-feature suite passes. This establishes the
correct state-machine seam for moving `PersistentNativeScanout` and Present
Complete/Idle timing out of the CLI; the live path is not yet wired through it,
so the Milestone 6 coordinator and sequencing items remain open.


## 2026-07-18: Backend Owns Page-Flip Retirement Correlation

`PersistentNativeScanout` no longer owns an Engine presentation registry, a
per-head scheduled frame slot, or a reduced UST/MSC feedback queue. The
protocol-neutral `LiveProductionPageFlipTracker` in backend-live now schedules
each submitted output against a production cycle, rejects overlap, validates
monotonic page-flip sequence and timestamp evidence, retires the exact scheduled
frame, and emits a `ProductionRetirement<LiveProductionPageFlipRetirement>` only
after all those gates succeed. Per-output take and discard operations preserve
the existing CPU-frame versus Present-frame separation without exposing backend
state to the frontend.

Regressions prove no retirement exists at submit time, a matching accepted flip
retains the originating cycle and reduced UST/MSC, overlap fails closed, and
non-monotonic callbacks produce no retirement. The full offline all-feature
suite passes. On the rebuilt X13-hosted QEMU image, strict two-xterm completed
300 ticks in 6,970 ms with 120 of 120 transactions applied, 8 ms input
presentation, 40 submissions, 38 retirements, and zero phase or cleanup debt.
Confined GTK passed 57 SHM transactions, exact text and pointer evidence, normal
exit, `first_error=none`, 113 submissions, 111 retirements, and clean shutdown.
Present protocol routing still resides in the CLI and is the next ownership seam.


## 2026-07-18: Present Complete And Idle Follow Backend Resource Retirement

`LiveProductionPresentFeedbackCoordinator` now owns the presentation resource
session and produces paired, protocol-neutral Complete/Idle outcomes only after
the matching page flip or controlled rejection retires the live presentation.
Missing or already-retired transactions return an explicit error and emit no
feedback. The live CLI translates the reduced Flip/Skip mode to X wire events
and observes counters, but no longer orders resource retirement, Complete, Idle,
or idle-fence accounting independently. Diagnostic abort still tears resources
down without falsely producing client feedback.

Tests prove a client-released DMA-BUF source and presentation retire before the
Flip/Idle outcome, a second completion fails closed, and an unknown Skip emits
nothing. The full offline all-feature suite passes. The guarded X13 native EGL
vkcube diagnostic passed with one CPU layer, one DMA-BUF layer, and zero live
sources, fences, or transactions. A rebuilt strict two-xterm QEMU image completed
300 ticks in 6,919 ms with 123 of 123 transactions applied, 7 ms input
presentation, 40 submissions, 38 retirements, and zero phase or cleanup debt.
The full real-KMS Milestone 4 proof could not run unattended because X13 sudo
requested a password before any modeset; it remains a later interactive gate.
Prepared Engine commit application and runtime/scanout invocation are still in
`live_session.rs` and remain the next production ownership migration.


## 2026-07-18: GPU Present Uses One Engine Snapshot Across Outputs

The live Present path no longer prepares and applies the same GPU transaction once per
output assembly. It prepares against the primary production coordinator snapshot, applies
that prepared commit exactly once after the matching page flip, and projects the resulting
immutable committed snapshot to the remaining outputs. This removes a multi-output visual
authority fork while preserving per-output scanout state.

A focused coordinator regression proves that applying a prepared Present mutates the
coordinator-owned snapshot, and the full offline all-feature suite passes. The rebuilt X13
QEMU image passed strict two-xterm in 6,907 ms with 120 of 120 authority transactions,
5 ms input presentation, 38 submissions, 36 retirements, and zero phase or cleanup debt.
Classic and confined GTK passed exact physical text and pointer selection, normal Zenity
exit, `first_error=none`, and clean two-output retirement. Emergency recovery flushed all
five routed chord deliveries and shut down cleanly in 187 ms. Runtime/scanout
invocation and the retirement-to-commit trigger still reside in `live_session.rs`; moving
that sequencing behind the production adapter remains the next Milestone 6 boundary.


## 2026-07-18: Coordinator Completes Retired Present Atomically

A matched GPU page flip now enters one `ProductionSessionCoordinator` operation that
applies the prepared Engine commit, captures the resulting immutable snapshot, retires
the backend Present resources, and produces the reduced Complete/Idle outcome. The CLI
requests that operation and translates its outcome, but no longer orders Engine commit
and backend feedback retirement itself. If the prepared baseline is stale, the coordinator
preserves the current snapshot and never invokes the feedback retirement callback.

Regressions prove commit-before-feedback on success and zero feedback calls for a stale
baseline. The full offline all-feature suite passes, the X13 release build succeeds, and
the guarded native EGL/vkcube diagnostic exports one CPU plus one DMA-BUF layer with zero
live sources, fences, or transactions afterward. The retained two-xterm, GTK classic, GTK
confined, and emergency QEMU gates already passed the immediately preceding snapshot; the
remaining production-loop gap is ownership of live runtime/scanout invocation itself.


## 2026-07-18: One Session Coordinator Owns Visual State

`PersistentBackendRuntime` now owns one session-level `ProductionSessionCoordinator`.
Authority commits, Present preparation, retired Present completion, and public committed
state all use that owner. Per-output backend assemblies receive immutable snapshot
projections for rendering and scanout; they are no longer selected as a primary authority.
A regression deliberately changes the first output projection to generation 99, then
proves the session coordinator independently commits generation 5 to 6 exactly once and
overwrites both output projections with its result.

The full offline all-feature suite passes. On the rebuilt X13 QEMU image, strict two-xterm
completed 300 ticks in 7,013 ms with 117 of 117 authority transactions, 7 ms input
presentation, 42 submissions, 40 retirements, and zero phase or cleanup debt. Confined GTK
committed 58 SHM transactions, accepted exact physical text and pointer selection, exited
normally with `first_error=none`, and retired both outputs cleanly. Live runtime and native
scanout method invocation still need to move behind the production live adapter before the
Milestone 6 coordinator item can close.


## 2026-07-18: Production Output Fanout Owns Runtime And Scanout Order

Engine now defines a protocol-neutral `ProductionOutputRuntimeAdapter`, and backend-live
provides its bounded callback implementation. The session coordinator projects its single
committed snapshot and enumerates outputs. Steady CPU ticks, committed-snapshot ticks, GPU
Present submission, native idle submission, page-flip retirement and cleanup, and displayed
buffer teardown all enter through that fanout. During the audit, retired Present completion
was also corrected to use the session coordinator directly and to project its result to
every output; it no longer mutates the former primary output coordinator.

Engine and backend regressions prove one snapshot reaches every output and projection plus
runtime invocation remain one adapter callback. The full offline all-feature suite passes.
The rebuilt X13 QEMU image passed strict two-xterm in 6,897 ms with 120 of 120 transactions,
7 ms input presentation, 40 submissions, 38 retirements, and zero phase or cleanup debt.
Classic and confined GTK accepted exact physical text and pointer selection, exited normally
with `first_error=none`, and cleanly retired both outputs. Emergency recovery flushed all
five routed chord deliveries and shut down cleanly in 189 ms. The concrete closures still
live beside `PersistentNativeScanout` in `live_session.rs`; extracting that implementation
into backend-live is the remaining runtime/scanout ownership step.


## 2026-07-18: Native Scanout Ownership Leaves The CLI

The concrete native output owner is now backend-live's `LiveProductionNativeScanout`.
It owns real atomic card/session groups, per-head GBM exporters, native callback queues,
page-flip correlation, submission and retirement counters, mixed-frame export, and the
production composed-frame record. The implementation is feature-gated in its own backend
domain module. `live_session.rs` no longer defines or directly names real atomic sessions,
GBM exporter types, or the page-flip tracker, and shrank by roughly 570 lines. The existing
opt-in native lifecycle diagnostic is preserved verbatim at the backend boundary.

Default and all-feature backend builds pass, as does the full offline all-feature suite.
The guarded X13 native EGL/vkcube diagnostic exported one CPU and one DMA-BUF layer with
zero live resources afterward. The rebuilt QEMU image passed strict two-xterm in 6,973 ms
with 120 of 120 transactions, 3 ms input presentation, 38 submissions, 36 retirements, and
zero phase or cleanup debt. Classic and confined GTK passed exact physical text/pointer
selection, normal exit, `first_error=none`, and clean two-output retirement. Emergency
recovery flushed all five routed chord deliveries and shut down cleanly in 164 ms. The
remaining Milestone 6 ownership gap is `PersistentBackendRuntime` plus the CPU composition
callbacks still implemented in the CLI.


## 2026-07-18: CPU Composition State Moves Into Renderer-Live

Renderer-live now owns `LiveProductionCpuScene`: the CPU buffer registry, immutable update
admission, committed-surface handle retention, focus-aware stacking, cursor composition,
focused-surface visual-detail inspection, composition evidence, and per-output composed
frame creation. The X boundary only converts authority SHM records into neutral
`LiveCpuBufferUpdate` values and observes reduced reports. Backend-live also owns composed
frame records and the high-level page-flip cleanup/retry and displayed-output release
operations, so CLI callbacks no longer invoke low-level native cleanup APIs or lifecycle
logging. The protocol-neutral authority, renderer, scanout, and feedback adapter roadmap
item is complete.

The full offline all-feature suite passes. On the rebuilt X13 QEMU image, strict two-xterm
completed in 6,995 ms with 117 of 117 transactions, 7 ms input presentation, 40 submissions,
38 retirements, and zero phase or cleanup debt. The guarded native mixed diagnostic exported
one CPU and one DMA-BUF layer with zero live resources. Classic and confined GTK accepted
exact physical text and pointer selection, exited normally with `first_error=none`, and
retired both outputs cleanly. Emergency recovery flushed all five routed chord deliveries
and shut down cleanly in 162 ms. `PersistentBackendRuntime` remains the last large CLI
visual-control wrapper; its X routing and proof observations must be separated from the
protocol-neutral production state machine before the Milestone 6 ownership item can close.


## 2026-07-18: CPU And Present Batches Enter Production Runtime Cycles

CPU authority batches now enter `ProductionSessionCoordinator::run_cycle`, which owns the
commit, immutable-snapshot composition, output submission, retirement poll, and feedback
order. The X session loop supplies translated authority updates and observes only the reduced
submission report. Present batches now likewise cross a single runtime-owned GPU production
entry point: CPU-background composition, per-output frame creation, native initialization,
and Present scheduling no longer occur in the outer CLI loop. Present remains asynchronous;
its prepared Engine state is committed only after the matching page flip through the existing
coordinator retirement gate.

The full offline all-feature suite passes. The rebuilt X13 target passed the guarded mixed
CPU-plus-DMA-BUF diagnostic with zero live sources, fences, or transactions. Its QEMU image
passed strict two-xterm in 6,966 ms with 117 of 117 authority transactions, 8 ms input
presentation, 40 submissions, 38 retirements, and zero cleanup debt. Classic and confined
GTK accepted exact physical text and pointer selection, exited normally with
`first_error=none`, and retired both outputs cleanly. Emergency recovery flushed all five
routed chord deliveries and shut down cleanly in 161 ms. The remaining Milestone 6 boundary
is structural: extract the runtime-owned visual control object from the X router, proof
counters, and process supervision, then delete the legacy committed-snapshot entry points.


## 2026-07-18: X Present Routing Leaves The Visual Runtime

The production visual runtime no longer stores an `XServerFrontendProtocolRouter` or any X
Present completion, idle, fence, or disconnect proof counters. It emits reduced
`LivePresentFeedbackOutcome` values through an injected protocol-neutral sink. The separate
`XPresentSessionObserver` translates those records to X wire events and owns all session-proof
accounting; shutdown consumes the renderer disconnect report outside visual control. A direct
regression proves the runtime sink receives the paired reduced outcome unchanged.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm
in 6,988 ms with 117 of 117 authority transactions, 8 ms input presentation, 40 submissions,
38 retirements, and zero cleanup debt. Classic GTK accepted exact physical text and pointer
selection, exited normally with `first_error=none`, and retired both outputs cleanly. The
remaining structural extraction is X authority-batch and resource translation plus session
supervision around the protocol-neutral runtime, followed by deletion of legacy committed-
snapshot entry points then shared with the Wayland maintenance path.


## 2026-07-18: X Authority Batches Stop At The Production Boundary

The X session loop now translates each projected authority batch once into a protocol-neutral
production record containing Engine transactions and surface removals, renderer DMA-BUF and
fence registrations, Present submissions, and release handles. X resource IDs, client IDs,
protocol errors, and authority-specific CPU update records do not cross that boundary.
`PersistentBackendRuntime` no longer accepts `XAuthorityObservedTransactionBatch`; CPU and GPU
production entry points consume only the reduced production batch plus renderer updates.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm
in 7,008 ms with 117 of 117 authority transactions, 7 ms input presentation, 42 submissions,
40 retirements, and zero cleanup debt. The guarded native mixed diagnostic translated and
exported one CPU plus one DMA-BUF layer with zero live sources, fences, or transactions. The
remaining Milestone 6 ownership work is moving the now-neutral visual control implementation
out of the CLI module and retiring its legacy committed-snapshot APIs.


## 2026-07-18: Backend Owns Production Intake Records

Backend-live now defines and exports the neutral authority batch, DMA-BUF registration, fence
registration, and Present submission records consumed by production visual control. The CLI
retains only the X-to-production translation function; it no longer defines the records or
their file-descriptor ownership shape. This makes the next runtime extraction a movement of
behavior behind an already backend-owned input contract rather than another protocol rewrite.
The full offline all-feature suite passes; runtime behavior is unchanged from the immediately
preceding strict QEMU and guarded native mixed evidence.


## 2026-07-18: Present Rebase Policy Moves Into Engine

The full-state Present generation rebase now lives beside `ProductionSessionCoordinator` in
`sophia-engine::runtime_driver`. The visual runtime no longer reaches back into a CLI library
module to reconcile skipped authority generations with the last visible Engine generation.
The former CLI module is only a compatibility re-export for its retained tests. The full
offline all-feature suite passes; this is a dependency-boundary change with no runtime
behavior change.


## 2026-07-18: Backend Owns The CPU Production Adapter

Backend-live now owns `LiveProductionCpuCycleAdapter`. It applies renderer updates after the
Engine commit, composes or coalesces from the immutable committed snapshot, creates native
frames for every output, invokes one narrow output-runtime callback, and returns reduced
composition timing and evidence. The CLI no longer implements `ProductionPresentationAdapter`
or defines the CPU production frame record; its remaining callback projects the snapshot and
invokes backend runtime/scanout objects pending their final owner extraction.

The full offline all-feature suite passes. On the rebuilt X13 QEMU image, strict two-xterm
completed in 6,971 ms with 117 of 117 authority transactions, 7 ms input presentation, 40
submissions, 38 retirements, and zero cleanup debt. Classic and confined GTK accepted exact
physical text and pointer selection, exited normally with `first_error=none`, and retired both
outputs cleanly after 54-56 CPU compositions. The next extraction is GPU scheduling and the
concrete per-output runtime owner; legacy committed-snapshot entry points remain only for the
then-active Wayland maintenance path and tests.


## 2026-07-18: Backend Owns Present Resource Admission

`LiveProductionPresentFeedbackCoordinator` now consumes the backend-owned production batch
directly to register DMA-BUF sources and fences and to process source/fence releases. The CLI
visual wrapper no longer clones file descriptors or sequences presentation-resource lifetime
admission. The full offline all-feature suite passes; behavior is covered by the immediately
preceding guarded native mixed and strict QEMU evidence.


## 2026-07-18: Backend Owns Present Scheduling State

Backend-live now owns `LiveProductionPresentScheduler`: queued and submitted Present state,
first-frame acquire delay, fence polling, bounded timeout rejection, controlled-rejection
proof policy, diagnostic triggering, and acquire/rejection counters. The CLI visual wrapper
asks for a reduced gate decision and supplies native scanout availability; it no longer owns
the scheduling tables or timing state. A backend regression proves delayed acquire admission
and one-shot controlled rejection with a registered DMA-BUF presentation.

The full offline all-feature suite passes. The rebuilt X13 guarded native mixed diagnostic
crossed the new scheduler, exported one CPU plus one DMA-BUF layer, and ended with zero live
sources, fences, or transactions. The remaining central Milestone 6 extraction is the
concrete per-output runtime owner and the legacy committed-snapshot APIs shared with Wayland.


## 2026-07-18: GTK QEMU Gate Now Proves Resize Redraw

The retained classic and confined GTK QEMU profiles previously passed input and native
presentation while reporting `surface_resize=disabled`, even though Milestone 5 requires a
CPU\/SHM redraw after an Engine-owned resize. Both guest profiles now request 640x360, and
the host harness rejects evidence unless the application record carries the complete semantic
tail: zero protocol errors, exact physical text, routed pointer selection, committed resize,
CPU\/SHM buffer path, native presentation, and clean teardown.

On the rebuilt X13 QEMU image, classic and confined Zenity each committed the resize with a
configure acknowledgement and changed pixels, accepted exact `sophia` input plus pointer
selection, exited normally with `first_error=none`, and retired both virtio-gpu outputs with
zero cleanup debt. Strict two-xterm also passed in 6,989 ms with 117 of 117 authority
transactions, 40 submissions, 38 retirements, and zero phase or cleanup debt. The remaining
Milestone 5 promotion gate is the deliberately operator-driven paired physical X13 capture.


## 2026-07-18: Production X Cursor Repaint Stops Replacing Engine State

The physical-pointer cursor repaint no longer composes frames in the outer X session loop or
calls the legacy committed-snapshot replacement entry point. A visual-runtime repaint method
reads the production coordinator snapshot, asks renderer-live to compose the cursor, creates
per-output frames, and submits them through the backend-owned output runtime set. The remaining
snapshot replacement API was named and called only by the Wayland maintenance adapter and its
regression; production X had no caller. The 2026-07-19 retirement removed that adapter.

The full CLI all-feature suite passes. On the rebuilt X13 QEMU image, strict two-xterm completed
in 6,941 ms with 120 of 120 transactions, exact keyboard and pointer proofs, 42 submissions,
40 retirements, and zero cleanup debt. Resize-enabled classic and confined GTK accepted exact
text and pointer selection, committed 640x360 CPU\/SHM redraws, exited normally with
`first_error=none`, and cleanly retired both outputs. The guarded native mixed diagnostic
exported one CPU and one DMA-BUF layer with zero live sources, fences, or transactions.


## 2026-07-18: Mixed Diagnostic Contract Moves Behind Backend Boundary

The native mixed-export completion record and its reduced evidence schema now live in
backend-live beside the native scanout diagnostic that produces them. The CLI only downcasts
the backend error, prints the reduced record, and applies command-level pass criteria; it no
longer defines a renderer\/scanout result type inside session supervision. A backend regression
freezes the exact schema. The rebuilt guarded X13 diagnostic still exported one CPU and one
DMA-BUF layer and retired all sources, fences, and transactions. This removes one CLI-specific
dependency that pinned the remaining neutral visual-control implementation to
`live_session.rs`.


## 2026-07-18: Visual Runtime Intermediate Records Move To Backend

Backend-live now owns the prepared-authority record and reduced CPU production submission
record used between visual-control phases. The CLI no longer defines internal records carrying
Engine commits, active transactions, backend ticks, renderer composition evidence, or compose
timing. Together with the backend-owned mixed diagnostic contract, this leaves the visual
control implementation dependent only on types already owned by engine, renderer-live, and
backend-live, preparing the concrete wrapper movement without changing runtime behavior.


## 2026-07-18: Concrete Visual Control Leaves The CLI

Backend-live now owns `LiveProductionVisualRuntime`, including the production coordinator,
per-output runtime set, renderer transaction projection, CPU and GPU cycle entry points, Present
resource admission and scheduling, native submission and retirement, cleanup, and reduced
feedback routing. The `PersistentBackendRuntime` type and roughly 950 lines of implementation
are gone from `live_session.rs`. CLI code constructs the runtime, translates X batches, supervises
clients, records proof evidence, and requests high-level service; it no longer defines visual
state-machine behavior. A reduced diagnostics snapshot replaces direct CLI access to Present
resource and scheduler internals.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm in
6,916 ms with 120 of 120 transactions, 40 submissions, 38 retirements, exact keyboard and pointer
proofs, and zero cleanup debt. Resize-enabled classic and confined GTK passed exact text, pointer
selection, committed 640x360 CPU\/SHM redraw, normal exit, `first_error=none`, native presentation,
and clean teardown. Emergency recovery flushed all five routed chord events and completed in 167
ms. The guarded native mixed diagnostic exported one CPU and one DMA-BUF layer with zero live
resources. Milestone 6 ownership, fail-closed, and retained-gate migration items are complete.
The remaining exit item is moving asynchronous GPU-service and retirement trigger timing from
the CLI event loop into `sophia-engine::runtime_driver`.


## 2026-07-18: Production X Uses One Native Service Poll

`LiveProductionVisualRuntime::service_native` now owns the asynchronous native service order:
page-flip retirement and cleanup first, eligible queued Present work second, and pending native
frames last. It returns one reduced report with the optional backend tick and phase observations.
The production X event loop no longer inspects pending exporter frames or separately invokes
retirement, GPU scheduling, and native idle submission. Wayland retains its specialized
maintenance service because it correlates client buffer release to its own submission counters.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm in
6,986 ms with 117 of 117 transactions, 40 submissions, 38 retirements, and zero cleanup debt.
Resize-enabled classic and confined GTK passed exact input, pointer selection, committed resize
redraw, normal exit, `first_error=none`, native presentation, and clean teardown. The remaining
Milestone 6 exit gap is exact rather than structural: GPU Present prepare\/retire sequencing still
lives in backend visual control and must enter `sophia-engine::runtime_driver` before that module
is the only production visual coordinator.


## 2026-07-18: Full-State Present Preparation Enters Runtime Driver

`ProductionSessionCoordinator::prepare_full_state_present` now owns authority-generation rebasing
and Engine preparation against its committed snapshot. Backend visual control no longer reaches
through `coordinator.engine()` or independently selects the preparation baseline. The same
coordinator already owns matching-retirement application and suppresses feedback when that
baseline is stale, so both sides of the asynchronous prepared-commit gate now remain in
`runtime_driver`. The external regression deliberately supplies generation 99 and proves the
coordinator rebases and commits it against the visible generation.

The full offline all-feature suite passes. The rebuilt guarded X13 mixed path crossed the new
coordinator entry point, exported one CPU plus one DMA-BUF layer, and retired all sources, fences,
and transactions. The remaining Milestone 6 coordinator gap is asynchronous KMS service adapter
shape, not Engine Present preparation or retirement ownership.


## 2026-07-18: Runtime Driver Owns Asynchronous KMS Phase Order

`ProductionAsyncServiceCoordinator` consumes reduced in-flight, cleanup, queued-Present, and
pending-frame observations and requests at most the ordered Retire, SchedulePresent, and
SubmitPendingFrame phases. Backend-live executes those requests and feeds updated observations
back after each action; it no longer encodes asynchronous phase order. A runtime-driver regression
proves dynamic behavior: retirement unlocks Present scheduling, and a resulting in-flight frame
suppresses pending submission in the same service pass. Together with coordinator-owned CPU
cycles and full-state Present prepare\/retire gates, this closes the last Milestone 6 architecture
checkbox.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm in
6,973 ms with 117 of 117 transactions, 40 submissions, 38 retirements, and zero cleanup debt.
Resize-enabled classic and confined GTK passed exact input, pointer selection, committed resize
redraw, normal exit, `first_error=none`, native presentation, and clean teardown. The guarded
native mixed path exported one CPU and one DMA-BUF layer with zero live resources. Milestone 6
implementation is complete; its overall promotion remains coupled to the operator-driven paired
physical X13 GTK evidence still required by Milestone 5.


## 2026-07-18: Post-Milestone-6 Native Stability And Physical Evidence Audit

The documented unattended X13 native stability gate passed 10 of 10 release runs against the
runtime-driver-owned phase state machine. Every retained record passed exact terminal text,
changed pixels, native presentation, callback validation, and zero in-flight or cleanup debt.

The durable Milestone 5 physical GTK store was audited rather than assumed valid. Its classic
record ends at pointer readiness with zero routed pointer events and has no application-session
completion; its confined record is empty; recovery records `emergency=true`. Those artifacts
cannot satisfy the current paired verifier. The remaining daily-driver promotion action is a
fresh local-TTY run of `tools/live_session_milestone5_gtk_hardware_proof.sh`, followed by the
three-class aggregate verifier. It requires a person to arm the independent guard, type exact
text, and physically click each dialog, so it cannot be completed through unattended SSH or
QEMU without weakening the stated acceptance criterion.


## 2026-07-18: GTK Client Exit Hang And Post-Proof Completion Watchdog

The first X13 run with routed pointer buttons accepted exact physical `sophia` text, routed
the OK click, and presented the surface-removal frame, then held a blank screen until the
emergency chord restored the TTY. Reduction found a completion-phase deadline vacuum: once
the text proof completes and a button routes, the keyboard-sequence and pointer-selection
deadlines are disarmed and the global runtime budget intentionally stays out of input proofs,
so any stall after the proof loops without a bound. The loop exit requires the primary
client's reaped exit status; a toolkit that destroys its window but never exits leaves that
term false forever. On a bare text TTY without a session bus address, GTK finalization is the
prime suspect for the missing exit.

The session now bounds the post-removal wait: when the application proof surface is gone and
the client has not exited within five seconds, the loop fails closed with the exact exit-term
states instead of presenting blank frames. Application-proof clients launch under
`dbus-run-session --` when no bus address exists and the runner resolves on `PATH`, giving
the toolkit a bounded per-client bus that exits with the client. The first watchdog draft
armed on proof-complete-plus-button and falsely expired inside the QEMU click-then-submit
sequence; the retained trigger is surface removal, which is the actual abnormal state.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm,
and resize-enabled classic and confined GTK passed exact text, routed button selection,
committed 640x360 redraw, normal exit, `first_error=none`, native presentation, and clean
two-output retirement. Fresh paired physical X13 evidence remains the acceptance gate; if the
watchdog fires there, its record names the missing exit term.


## 2026-07-18: X13 GTK Blank Session Reduced To Tap Policy And Pointer Deadline

A fresh classic hardware run accepted exact physical `sophia` input, committed the 640x360
GTK resize, presented the software cursor, and routed sustained touchpad motion. It emitted no
pointer-button record, no application-session record, and no bounded-completion record before
the independent emergency chord restored the TTY cleanly. X13's libinput report confirmed that
the ELAN touchpad supports tapping but defaults tap-to-click to disabled.

The native path-based libinput owner now enables tap-to-click for every tap-capable admitted
device, verifies the applied state, and exports only reduced device/tap counts. The proof loop
now distinguishes motion observed/routed from button observed/routed. Its selection deadline
remains armed after cursor pixels change and ends only after both a routed button and pointer
pixel evidence; this closes the prior unbounded state where motion canceled the only pointer
deadline. Cursor repaint also fails closed if an application proof produces no composed layer
or only the bounded software-cursor footprint.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm
in 6,880 ms with 19 of 19 input deliveries, 40 submissions, 38 retirements, and zero native
debt. Resize-enabled classic and confined GTK passed exact text, routed button selection,
normal exit, `first_error=none`, and clean two-output retirement. A bounded non-KMS smoke
against the real ELAN path reported `devices=2 tap_capable=1 tap_enabled=1` and completed its
xterm pixel proof. Paired physical X13 evidence remains the acceptance gate before GTK3
software promotion.

## 2026-07-18: Milestone 5 Uses Unattended QEMU Acceptance

Machine-specific X13 capture is no longer an application-promotion gate. The
repeatable acceptance boundary is a diskless, networkless QEMU guest that owns
virtio DRM/KMS, guest console state, and libinput-backed virtio keyboard and
pointer devices. Direct hardware runners remain optional compatibility
diagnostics.

`tools/qemu_milestone5_acceptance.sh` rebuilds the guest and runs strict
two-xterm presentation/input, emergency Ctrl-Alt-Backspace recovery, and classic
plus confined GTK3 profiles without operator input. The first aggregate run
exposed a stale schema-1 poller assertion in emergency recovery; updating the
harness, verifier, and fixtures to the schema-2 tap-policy record closed it. The
rerun passed all four scenarios. The strict three-class baseline then passed

## 2026-07-18: WM API v2 Foundation

Milestones 7 and 8 split interactive policy enablement from daily-driver
promotion. The normative WM contract now fixes Engine ownership of physical
input, nine workspace slots, named session actions, opaque metadata, and
one-visible-workspace-per-output semantics.

The protocol carries a versioned hello, bounded binding registrations, session
descriptor, opaque action activation, workspace activation, and named session
action requests. Engine rejects unsupported capabilities, duplicate bindings,
invalid action/key values, and Ctrl-Alt-Backspace. Its shortcut registry
consumes matching press/release pairs and suppresses repeats without leaking raw
input. The native demo WM performs the startup handshake and exercises focus,
workspace, and terminal actions. Engine now owns per-seat physical modifier
state, consumes registered chords after the emergency chord check, and sends
opaque action activations through the live WM transport. A nine-slot workspace
policy validates workspace swaps, surface moves, visible focus, layout commands,
and advertised session tokens. The profiled legacy bridge registers the bundled
xmonad chord set and translates bounded workspace and named-action requests.
The full offline all-feature suite passes. Atomic delayed-commit persistence,
named-action execution, xmonad focus/layout synthesis, and QEMU remain open.

## 2026-07-18: Normal xmonad Session Launcher

Milestone 7 is archived with its unattended QEMU gate retained as a frozen
regression. Its verifier now has a positive fixture plus negative mutations and
fails explicitly when any guest failure marker is present.

`sophia-live-session --session-mode=normal` now owns a bounded application
registry with explicit startup and named-action mappings. Applications are
spawned without a shell in dedicated process groups; normal exit is nonfatal,
and shutdown sends TERM to the group before a bounded KILL fallback. The
operator launcher selects xmonad compatibility policy, native Sophia WM policy,
or no external WM without placing a WM identity in Engine.

The first Milestone 8 QEMU gate starts one registered xterm, survives an
intentional bridge/xmonad restart with layout preserved, launches a second
registered xterm through xmonad's terminal action, closes it, logs out, and
retires two-output native presentation without cleanup debt. The frozen
Milestone 7 two-xterm gate also passes against the same build.
## 2026-07-22: Xmonad TTY3 Requires Independent Local Recovery

The first physical native-xmonad TTY3 operator attempt produced blank scanout.
Sophia did not provide VT suspend/resume, the wrapper had no independent input
guard or explicit KD/termios restoration, and its documented recovery path
incorrectly depended on switching to another TTY. The operator had to reboot,
which also erased the `/tmp` launcher log.

The xmonad wrapper now applies the established guarded TTY lifecycle before
graphics takeover: one Ctrl-Alt-Backspace chord must arm the independent input
guard, a second chord stops the supervised session even if Sophia input routing
is wedged, and cleanup restores KD mode, termios, and keyd. Guard and recovery
records are durable under the user's state directory. Ctrl-Alt-Fn remains
unsupported until Engine owns a correct VT/DRM suspend-and-resume boundary.

The guarded rerun proved DRM ownership, two-output presentation, WM bridge
startup, and complete emergency restoration, but also retained
`startup_apps=0` and no live `key_observed` marker after Super-Enter. The normal
TTY wrapper had omitted `--session-start=terminal`. Independently, a blank
normal session represented its missing primary child as already exited; the
post-exit proof guard then compared two absent surfaces and suppressed the
entire physical-input poll, including global WM shortcuts. The session now
distinguishes full application routing, shortcut-only polling, and complete
suppression. Empty desktops admit emergency and registered WM chords without
delivering ordinary keys or pointer events to an unfocused client, and the
physical launcher starts Kitty by default.

## 2026-07-22: Kitty Requires A Direct-Mesa GLX Bootstrap

The guarded physical rerun reached xmonad and input recovery, then Kitty 0.48.0
failed with `failed to create GLFW window`. Sophia advertised GLX 1.4 but no
FBConfigs and omitted `GLX_EXT_libglvnd`; libglvnd therefore selected its
indirect vendor instead of Mesa and never reached the already implemented
DRI3/Present path.

Sophia now exposes a depth-32 ARGB TrueColor visual, maps GLVND's vendor query
to `mesa`, and implements a bounded direct-rendering GLX bootstrap: visual and
FBConfig catalogs, client-info negotiation, direct context lifecycle, GLX
window aliases, and drawable attributes. The catalog deliberately contains
only XRGB linear, ARGB linear, and ARGB sRGB configurations with depth/stencil
zero. Indirect GLX rendering, server-side Render/RenderLarge, and GLX
SwapBuffers remain unsupported; Mesa renders client-side and submits through
DRI3/Present. The first live trace additionally showed Mesa using X Sync to
destroy DRI3 fences and GLFW freeing its ARGB colormap, so the bounded Sync
initialization/fence teardown and core colormap teardown paths are retained as
part of the same compatibility slice.

`x-authority-kitty-smoke` now proves the exact live sequence on an AMD render
node: GLVND vendor selection, FBConfig discovery, two direct context/window
lifecycles, depth-32 modifier negotiation, ARGB DRI3 import, one accepted
Present transaction, one committed runtime surface, and clean protocol state
with `first_error=none`. The guarded physical TTY3 capture remains the session
gate; the standalone smoke deliberately terminates its proof window after the
first committed frame because it has no live renderer feedback loop.

## 2026-07-23: Concurrent Present Identity and Physical Cursor Follow-up

The next guarded TTY3 run proved that Super-Enter launched a second terminal,
then the persistent frontend exited with `X11 route queue is full for client
9`. Each X client worker had derived the Engine transaction identity from its
connection-local 16-bit X sequence. Two clients could therefore submit the
same global `TransactionId`; the pending Present registry treated that
collision as queue pressure, and worker completion propagated the client error
to the whole frontend supervisor.

The listener now allocates monotonically increasing 64-bit transaction
identities shared by all of its workers while preserving the X wire sequence
as a client-local `u16`. Duplicate transaction identity and real per-client
pressure are distinct failures. A full Present queue waits for bounded
Complete/Idle progress for up to two seconds, and removal wakes waiters; no
feedback is dropped or reordered. A client-local Present failure disconnects
only that worker, while panics, poisoned shared state, and supervisor failures
remain service-fatal.

The same capture exposed the private legacy-WM server's 1280x720 setup fixture
on physical 2560x1440 and 1920x1080 outputs. Production bridge startup is now
lazy until the first Engine layout request, advertises those actual bounds in
X setup, and emits root ConfigureNotify on later changes. The compatibility
fixture remains available only to standalone smoke callers.

Cursor ownership remains protocol-neutral. A bounded passive snapshot models
visibility, global position, hotspot, ARGB image, and generation without an X
or application identity. The current software path now coalesces motion,
avoids repaint submission while native scanout is in flight, polls at 1 ms
while motion is pending, and reports maximum motion-to-submit latency.
Client-selected classic X cursor shapes and optional KMS cursor-plane
presentation remain separate follow-ups; the Engine contains no Kitty-specific
branch.

## 2026-07-23: Kitty-Only Physical Input Gate

The first run after the Present identity fix appeared completely locked, but
the durable log proved two physical mouse motions were observed and routed and
Super-Enter launched a second Kitty. Both xmonad layouts timed out, and the
cursor path could replace a DMA-BUF application frame with a CPU-only scene.
This was a visible presentation failure, not lost libinput events.

The supported TTY3 gate now starts one automatically focused Kitty with no
external WM while retaining both physical outputs and the independent recovery
guard. Xmonad, Super-Enter, and resize synchronization are deferred until
keyboard, pointer motion, button delivery, drag selection, and clean shutdown
pass together.

DMA-BUF sessions no longer invoke the CPU cursor repaint. Backend-live owns a
64x64 ARGB dumb buffer containing a classic left pointer and uses the DRM
hardware-cursor interface to install it on the active CRTC, move it without a
primary-plane repaint, and transfer it across the extended-horizontal output
layout. Motion is coalesced, presentation latency and hardware update failures
are reduced into the session evidence, and CPU repaint remains available only
for CPU-buffer regression clients.

## 2026-07-23: Production Input Uses Libinput Seat Discovery

The guarded launcher previously guessed one keyboard and the first distinct
mouse from `/dev/input/by-id` and `/dev/input/by-path`. That could not correctly
represent composite receivers, multiple keyboards, touchpads, or hotplug.

Backend-live now supports libinput's udev context and assigns `seat0` by
default. Device-added events classify keyboard, pointer, and touch
capabilities, apply tap-to-click where supported, and update bounded policy
evidence; device-removed events maintain active counts without exposing device
names or paths. Both the independent recovery guard and the Sophia session use
seat discovery. Explicit `--input-devices` remains mutually exclusive with
`--input-seat` and is retained only for deterministic hardware/QEMU proofs.

## 2026-07-23: Kitty Startup Is Bounded Before the First Surface

The first Kitty-only `seat0` run acquired both outputs and discovered fourteen
libinput devices, but remained blank until the independent guard restored the
TTY. The session log contained no focused or committed application surface.
Kitty's desktop-settings request failed after 10.389 seconds because a bare TTY
had no usable portal service; emergency recovery was requested at the same
boundary. The separate non-modesetting Kitty trace still passed 207 X requests,
one DRI3/Present transaction, one runtime surface, and `first_error=none`, so
the direct-Mesa GLX path was not the failing boundary.

The first private-bus attempt activated the host notification and XFCE settings
services without a usable desktop display, adding another nondeterministic
startup path. The Kitty gate now matches the passing standalone trace instead:
Wayland variables are removed, desktop-service bus activation is disabled, and
the no-WM profile forces opaque X11 rendering. The live session accepts a
generic bounded startup deadline and succeeds only after a focused CPU-detail
or DRI3/Present surface crosses actual native presentation. A missing surface,
uncommitted surface, missing visual content, or unpresented frame reports a
distinct reduced stage and returns through the normal TTY cleanup path after
eight seconds.

Native normal sessions initialize an empty output runtime immediately. Physical
pointer motion is polled before a client surface exists, the compositor-owned
classic hardware cursor begins at the primary-output center, and unfocused
keyboard and pointer-button events remain unrouted. This removes the prior
first-surface dependency from cursor feedback without introducing an
application-specific branch in Engine.

The first centered-cursor repair was insufficient. A physical rerun still
showed a frozen inherited pointer beside Sophia's moving pointer because the
atomic display owner attempted to clear cursor state through deprecated legacy
cursor ioctls and discarded every error. Backend-live now discovers cursor
planes compatible with every selected CRTC, atomically detaches them before
first use, retains an ARGB cursor framebuffer, and performs coalesced atomic
attach/move commits. Hardware and software paths share one canonical classic
X11 pointer raster.

The same rerun proved that input discovery was not the keyboard failure:
libinput observed a key and routed pointer motion plus a button. Kitty created
and mapped a surface and submitted one DRI3/Present frame, but no later
authority batch arrived after asynchronous KMS retirement. Initial focus and
startup-content reconciliation lived only in the authority-batch branch, so
the retired surface never gained focus and keyboard delivery remained gated.
Present retirement now carries its transaction and surface back to the session
loop; the shared reconciliation path runs after both authority work and KMS
service, sends X11 focus control, and recognizes the retired frame without
requiring a second Present. Startup diagnostics now report actual committed
surfaces, focus/control state, Present retirement, native submission failures,
callbacks, and per-output in-flight state.

Finally, the physical gate had not actually matched the passing Kitty smoke:
it still loaded the normal user configuration. The guarded profile now uses
`--config NONE` with only the forced X11, opaque-background, and diagnostic
title arguments. Normal Kitty configuration compatibility remains outside the
minimal input gate.

## 2026-07-23: Native Scanout Uses One Frame And One Cursor Owner

The next physical Kitty-only run proved that accepted KMS commits and routed
input were not sufficient visual evidence. Kitty created a 2540x1390 window,
completed DRI3/Present, and received keyboard and pointer routing, while the
operator still saw a frozen pointer beside the moving hardware cursor and no
Kitty content.

The exporter had independent pending CPU, DMA-BUF, and mixed-frame slots.
Consequently a mixed Kitty frame could export first while an older CPU frame
remained eligible for the idle service, and later CPU-only authority work could
replace retained DMA-BUF content. Native CPU composition also continued to
receive the pointer position even though the atomic cursor plane owned cursor
presentation.

Native pending scanout is now a single latest-wins slot per output. A mixed
frame supersedes its unsubmitted CPU precursor, and authority work preserves
the last GPU scanout whenever a committed or pending DMA-BUF layer cannot be
recomposed safely. Physical native sessions select hardware cursor ownership
explicitly and never bake cursor pixels into CPU backgrounds. Mixed submission
resolves the registered primary output by identity, and startup becomes ready
only when the exact Present transaction is the stable displayed content with
no newer primary frame queued or submitted. The Kitty-only physical capture
remains required before restoring xmonad.

## 2026-07-23: Secondary Scanout Cannot Block Primary Present

The first physical run with stable Present provenance showed one correct
hardware cursor but no Kitty pixels. Kitty created a 2540x1390 window and
issued Present, yet startup ended with zero committed surfaces. The primary
output was idle while the secondary output still had an unrelated CPU frame in
flight. The async service used one global in-flight bit, so the secondary
retirement prevented the primary mixed frame from ever being scheduled.

Native async service decisions are now output-scoped. Retirement still polls
every in-flight or cleanup-pending output, but a queued mixed Present is blocked
only by the registered primary output. Pending background frames are submitted
only on outputs that are individually idle, so one blocked output cannot cause
another exporter to run or prevent useful work. Submit, callback, and
retirement diagnostics now carry output identity and submission provenance.
The next guarded Kitty-only capture must prove the primary Present commits
while secondary retirement remains independent.

## 2026-07-24: Present Driver And VT Must Share Output-Scoped Ownership

The next physical run still showed only the hardware cursor. Output-correlated
logs proved the async coordinator retired the primary independently, but the
Present driver retained a second global in-flight early return. The secondary
output therefore continued to veto mixed composition after the coordinator had
correctly admitted it.

The same run left typed `ll` in tty3's input queue, where it appeared after
greetd returned. The Kitty launcher saved and restored KD and termios state but
never entered KD graphics or raw/no-echo mode. That left the console line
discipline active underneath native scanout.

The Present driver now consumes the same tested output-state reduction as the
service coordinator and blocks only on primary in-flight or cleanup state.
After the independent emergency guard is armed and immediately before starting
Sophia, the launcher switches to KD graphics and `stty raw -echo`; its existing
cleanup restores the exact saved KD and termios state on normal, failed, signal,
and emergency exits. Regression tests retain the guard-before-takeover order
and exact restoration commands.

## 2026-07-24: Queued Present Must Reserve The Primary Output

The first physical run after VT takeover showed one working hardware cursor but
no Kitty pixels. The capture proved Kitty created and mapped its 2540x1390
window, submitted Present transaction 1, and routed pointer motion and button
events. It also proved exact KD and termios recovery. Mixed scanout never
submitted: after a primary CPU frame retired, the async service repeatedly
submitted another CPU frame until the startup watchdog expired.

The async coordinator orders Present before pending frames, but a Present phase
is an attempt rather than proof of submission. If that attempt observes a
transiently blocked primary, advancing to the pending-frame phase can fill the
same primary immediately and starve Present indefinitely. A queued Present now
reserves the primary from pending CPU submission. Idle secondary outputs remain
eligible, preserving independent multi-output progress. Regression coverage
proves both the primary reservation and secondary eligibility.

## 2026-07-24: The Opaque sRGB Hypothesis Was Disproved

The next physical run proved mixed scanout and input were functional despite
the display still appearing black. Present transaction 202 reached mixed KMS
scanout, retired, became stable in 708 milliseconds, and accepted the user's
`exit` input. The failure was therefore the content of the imported client
layer rather than scheduling, focus, or VT ownership.

Adding an opaque sRGB framebuffer configuration did not change the client
selection: the next retained request stream still selected FBConfig 3, created
depth-32 windows, and exported ARGB8888 pixmaps. Both physical outputs remained
blank even though the mixed frame retired and keyboard input reached the
client. The speculative configuration and its compatibility claim are removed.

The remaining boundary is pixel content. Native mixed composition now has an
opt-in, one-shot readback that reports only aggregate counts and checksums after
the CPU background and after the DMA-BUF layer. The verifier distinguishes an
unchanged framebuffer, no visible RGB delta, and a visibly changed client
layer. ARGB composition also uses premultiplied source-over blending (`ONE`,
`ONE_MINUS_SRC_ALPHA`) instead of multiplying source RGB by alpha twice. These
changes remain application-agnostic and preserve Engine's protocol-neutral
authority boundary.

The first attempted diagnostic capture contained no pixel-evidence records, so
it could not classify the blank frame. Until the Kitty-only physical gate
passes, that profile now enables the bounded one-shot trace directly rather
than depending on an operator choosing a separate wrapper. An explicit
`status=enabled` record distinguishes missing activation from a failed GL
readback.

The resulting physical capture localized the blank screen further. The CPU
background and the framebuffer after the first Kitty DMA-BUF layer had the
same checksum and zero nonzero RGB pixels, while scanout retired the mixed
transaction normally. Kitty submitted that initial Present before mapping its
top-level window, then allocated a second DRI3 pixmap but submitted no second
Present. This is consistent with the client waiting for Present
Complete/Idle feedback before exposing its rendered window, not with a KMS,
cursor, keyboard-routing, or terminal-shell failure. The Kitty gate now traces
whether each backend completion was actually routed into the X frontend so the
next physical capture can distinguish feedback routing from client-side
consumption.

The next physical capture made that distinction: Complete and Idle were both
generated for the retired frame, but both reported `routed=false`. Standard
Present decode retained the X server frontend's globally allocated transaction
for its pending-feedback registry, while dispatch replaced the transaction
sent to Engine with the client's 16-bit request sequence. Retirement therefore
could not match the pending entry. Standard Present now carries the global
transaction explicitly through decode and dispatch, matching the existing
feedback registry key and avoiding cross-client sequence collisions. A wire
regression uses deliberately different request-sequence and global-transaction
values and requires the resulting surface transaction to retain the global
value.

The first capture after that change still reported `routed=false`, proving the
single-client run did not depend on the transaction distinction. The request
order exposed the actual feedback loss: Kitty selected Present events for a
bootstrap window, selected them again for its main window, then cleared the
bootstrap selection. The frontend stored only one selection per client, so
clearing the old event ID removed the newer main-window selection. Complete
marked the pending presentation finished but found no subscriber; Idle then
removed it without notifying Kitty. Kitty consequently never mapped its main
window or submitted the rendered follow-up pixmap.

Present selections are now retained per client and event ID, and a zero mask
removes only that selection. Complete and Idle route to every matching
window/mask selection. A regression preserves Kitty's observed bootstrap/main/
clear ordering and requires both events on the main event ID.

The same capture confirmed exact KD and termios restoration but returned to
greetd's VT because restarting the service activated its console. The guarded
TTY3 launcher now records its originating VT, restores the display manager,
reactivates that VT with `chvt`, and records the resulting active VT for both
normal and emergency cleanup.

The next physical run reached a visible Kitty command line, routed typed keys,
and continued retiring rendered Present frames with Complete/Idle feedback.
The session terminated only after pointer motion attempted a nonblocking atomic
cursor-plane update while another KMS commit was still in flight. Linux
returned `EBUSY`; the cursor path recognized only `EAGAIN`/`WouldBlock` as
transient and escalated `EBUSY` into a fatal session error. Nonblocking cursor
attach, move, and detach now classify both results as deferred and retry from
the existing dirty-cursor loop after scanout progresses. Other atomic errors
remain fatal.

The following run stayed healthy but did not echo `ll`. Its ordering identified
a separate startup race: Sophia declared focus ready and forwarded the physical
keys roughly one second before GLFW changed the mapped window's core event mask
to include KeyPress and KeyRelease. The X frontend previously fell back to the
focused window even when no window in its ancestor chain had selected keyboard
events, so the client ignored those early records. The input writer now keeps a
physical key boundedly pending for up to five seconds while the focused route
has no keyboard selection, then targets the selected window as soon as the
client installs its mask. This is based solely on standard X11 event selection;
it contains no Kitty-specific policy.

The next operator observation showed `ll` on tty3 only after emergency teardown.
That proves the VT input queue was still receiving the physical keyboard:
`stty raw -echo` disabled canonical processing and echo but did not disconnect
the Linux console keyboard from the VT. The guarded launcher now saves the
console keyboard mode with `KDGKBMODE`, selects `K_OFF` while Sophia owns the
graphical VT, and restores the exact saved mode during every cleanup path.
Evdev remains available to libinput and the independent emergency guard.

## 2026-07-24: Kitty input is blocked after wire delivery

`x-authority-kitty-input-smoke` now reproduces the interactive failure without
owning a VT. It launches Kitty 0.48.0 with `--config NONE`, uses the routed
frontend and render node, waits for two DRI3/Present submissions, snapshots the
client-visible XKB map, focuses the mapped surface, and injects `ll` plus
Return. The gate only passes when the proof shell writes exactly `ll` and Kitty
submits another Present.

The retained diagnostic boundary is narrower than the earlier hardware
hypotheses: keycode 46 resolves to `l`; Kitty's mapped window selected core
KeyPress/KeyRelease; focus control is delivered; all six events are flushed to
the client socket; and Present Complete/Idle routing succeeds. The shell still
receives zero bytes and Kitty submits no post-input frame, while the existing
xterm input smoke passes the same brokered route. Physical libinput discovery,
console echo, Kitty configuration, and xmonad are therefore not the current
root boundary. Promotion remains blocked on Kitty consuming the X11 stream
across Present synchronization.

## 2026-07-24: Kitty Keyboard Root Cause Was Extension Event Aliasing

The strict real-Kitty input gate now passes. Physical input discovery, routing,
focus, event selection, and XCB receipt had all been working. An instrumented
libX11 showed that each core KeyPress/KeyRelease reached its queue and was then
rejected by the installed wire converter. Sophia advertised GLX with traditional
event base zero, so libGLX registered its seventeen extension converters over
core event numbers 0 through 16, including KeyPress 2 and KeyRelease 3.

Sophia now assigns non-core, mutually disjoint traditional event ranges to
RANDR, XFIXES, SYNC, GLX, XKEYBOARD, XInputExtension, and MIT-SHM. The XKB
names reply also reports level-name counts consistent with the two levels
advertised by XkbGetMap. Installed Kitty 0.48.0 consumes routed `ll` plus
Return, writes the exact shell result, and submits three later Presents. This
is protocol-level, application-agnostic behavior; no Kitty policy exists in
the engine.

The subsequent guarded TTY3 run provided the physical promotion proof. Kitty
became visibly ready in 798 ms; physical keyboard input and two pointer-button
transitions were routed; cursor motion-to-submit remained bounded at 13 ms;
Kitty exited with status zero; protocol health was clean; and the originating
TTY modes were restored without emergency recovery. A separate report-field
bug falsely rejected that successful run because the stable-Present readiness
path logged readiness without persisting its elapsed time; both paths now
populate the same readiness measurement.

## 2026-07-24: Architecture Debt Moves Behind Stable Facades

The conformance pass now treats file size as evidence of mixed ownership rather
than a mechanical target. X authority dispatch observations are owned,
value-free records; routing packets and frontend admission/provider contracts
live in separate data modules while the existing crate facade remains stable.
The live-session command now delegates secure Xauthority files, proof
artifacts, X frontend adapters, and process-group supervision to their owning
modules. Its owner loop and physical-input poll boundary receive explicit
channel/resource/startup records instead of 18- and 16-argument call sites.

Native scanout now separates passive DMA-BUF and composition records from
GBM/EGL execution, and the legacy WM bridge separates wire framing from runtime
supervision. X transport reduction tests moved from production source to an
integration test through the public owned-observation API. Focused all-feature
checks and tests pass for the affected crates. The remaining work is the
mutable registry/worker and protocol-family extraction, the live owner/WM/input
split, scanout lifetime/composition execution, legacy bridge server/dispatch,
and the remaining oversized integration fixtures.

## 2026-07-24: Continuous Present Starved WM And GPU Input Readiness

The installed xmonad proof still appeared to lose both keyboard and pointer
input after tracing was disabled. The retained session showed fourteen active
libinput devices, stable mixed Present retirement, a visible hardware cursor,
and Engine focus, but no WM layout commit, applied X11 focus, or physical-input
readiness marker. Disabling tracing changed timing only; it could not repair
either missing state transition.

Initial WM management waited for 500 milliseconds without any authority work.
Kitty's continuing Presents reset that global timer, so xmonad could remain
ready yet never receive its first opaque surface. Unmanaged surfaces are now
submitted whenever no WM transaction is pending, one at a time, independent of
application frame cadence. Startup and proof readiness also track the surface
whose X11 focus was actually acknowledged instead of treating the presence of
an external WM as proof that focus was applied.

The same stable DMA-BUF retirement that satisfied visible startup did not set
the older CPU-only terminal-content flag. The proof therefore never armed and
the session deliberately skipped libinput polling. Focused content readiness
now accepts either CPU visual detail or a stable retired Present belonging to
the focused surface. Libinput is always drained: before proof readiness,
pointer motion updates only the compositor-owned cursor while ordinary keys
and buttons are discarded without entering the exact-text matcher or X11
route. The installed gate now requires exact `sophia` input followed by routed
pointer motion and one button. All decisions use opaque surfaces and generic
presentation facts; Engine contains no Kitty-specific behavior.

The first installed rerun confirmed the WM, focus, cursor polling, and stable
Present changes, but physical input still remained in cursor-only mode. The
proof had a second baseline predicate requiring nonzero CPU-scene pixels even
after the focused stable DMA-BUF surface was accepted. GPU-only Kitty therefore
reached content readiness without satisfying the duplicate CPU gate. Baseline
readiness now consumes the same focused-content fact used by startup and input
arming; CPU composition remains an alternative source rather than an additional
requirement. A regression fixes the GPU-ready/CPU-empty combination.

The next installed run proved that exact physical `sophia` input reached the
shell and all fourteen X11 events flushed. Thirty later authority batches and
stable native Presents followed, but the post-input verifier still required a
CPU-buffer checksum or generation change and timed out a GPU-only terminal.
A stable retired Present on the exact proof surface after input delivery now
provides the corresponding GPU presentation evidence.

That run also exposed a separate cursor ordering error. Once text input was
ready, pointer motion entered full routing mode, but application pointer
delivery remained gated until the later pointer-proof phase. Cursor placement
was incorrectly behind that delivery gate, freezing the compositor-owned
hardware cursor. Placement now occurs before the application-delivery decision,
so cursor motion remains responsive without prematurely routing pointer events.

The following physical run displayed a blinking prompt but did not echo input.
Its retained evidence contained `key_observed` without `key_routed` and ended at
`focus_control_pending`. The WM layout committed before the corresponding
surface entered Engine's committed-surface set; the first focus attempt returned
`UnknownSurface`, but the owner loop consumed the one-shot focus request anyway.
WM focus requests now remain pending on that transient result and are consumed
only after Engine focus and X11 client focus both succeed.

The retry run then committed focus, armed physical input in 800 milliseconds,
and routed keys. It stopped only because the proof matcher required strict
press-release pairs and rejected ordinary keyboard rollover when `a` was
pressed before `i` was released. The proof now validates exact character press
order while independently tracking balanced releases, accepting natural
overlap without weakening modifier, repeat, unexpected-release, or submit-key
checks.

## 2026-07-24: Owner-Loop State And Oversized Tests Split By Domain

The remaining live-session owner-loop state now has explicit delivery,
observation, cursor-update, and metrics records. Input-delivery draining is an
owned phase with one state boundary instead of a macro mutating seven ambient
delivery variables. The 168-line owner-loop facade initializes resources and
state, then delegates lifecycle, authority, input proof, physical input, and
completion to bounded phase owners.

Oversized test programs were split along real ownership seams: live-session
presentation, Engine rendering transactions, runtime process supervision,
atomic-scanout retirement, and native page-flip decoding. The source-layout
audit no longer reports any test program at 800 lines. The split modules reuse
their parent fixtures and preserve the same behavioral assertions; focused
CLI, Engine, runtime, and all-feature backend suites pass.

## 2026-07-24: Physical Promotion Needs Action And Output Evidence

The original physical xmonad verifier proved that xmonad committed some layout
and focus work, that two Kitty processes started, and that the session retired
native frames. Those aggregate facts could not distinguish the requested
focus, layout, workspace, close, click-drag, and two-output workflow from a
shorter interaction that happened to increment the same counters.

The live-session boundary now records the opaque action number after a
physically initiated WM proposal commits. This remains application- and
protocol-neutral: the Engine sees the same opaque policy action and no Kitty
identity. The physical verifier requires the focus-next, next-layout,
workspace-away, workspace-return, and close actions, two pointer-button
transitions, two terminal launches, two native outputs, and an independently
retired page flip on each output. Fixture mutations prove that missing
workspace, output-retirement, cursor, or click-drag evidence is rejected.

These records prove that the requested control path committed; they do not by
themselves prove that a hidden surface received no routed input. That
isolation claim remains a physical gate until delivery evidence correlates
input with the focused visible surface before and after workspace changes.

## 2026-07-24: Independent Recovery Must Not Preempt Owner Cleanup

The independent input guard previously exited 250 milliseconds after detecting
the emergency chord. The TTY wrapper's `wait -n` then returned and cleanup
immediately sent `TERM` to the graphical process group. That could preempt the
live owner loop even when it had independently observed the same chord and was
draining routed input, native scanout, and Present state.

After a guard trigger, the wrapper now gives the live session a bounded
five-second window to finish its in-process emergency path. A schema-3 recovery
record distinguishes `graceful` completion from `fallback_term` and retains the
session exit status alongside KD and termios restoration. The physical
emergency verifier requires guard and owner observations, a status-zero
graceful exit, fully drained input, no native or Present debt, and exact TTY
restoration. Fixture mutations ensure that a TERM fallback cannot be promoted
as a successful emergency capture.

## 2026-07-24: Scroll And Hidden Focus Were Missing Input Semantics

The Firefox promotion audit found that libinput wheel events were dropped
before entering Sophia packets. Input now carries signed, protocol-neutral
horizontal and vertical v120 units through Engine hit-testing. Only the X
frontend maps those units to core X11 scroll buttons, emitting a bounded
press/release pair. The deterministic Firefox workload now proves scroll,
focus-away/focus-return, and a pointer-opened dialog in addition to keyboard,
CLIPBOARD, PRIMARY, resize, and normal exit.

The same audit found that workspace policy cleared its hidden-focus record
without clearing the Engine seat or X frontend focus. A hidden but still
committed surface could therefore remain the keyboard target. Workspace-away
now issues a surface-scoped clear-focus control, waits for X authority
acknowledgement, clears Engine focus, and records the transition. A harmless
key typed before workspace return must be explicitly suppressed for lack of
focus. The physical verifier requires the ordered sequence: workspace away,
focus cleared, key suppressed, workspace returned.

Adding axis routing pushed the mutable route registry over the cohesion
threshold. Resolved-input selection and frozen-input draining now live with
the existing routing-input owner, returning the registry below 1000 lines
without changing its public facade.

## 2026-07-24: Installed Login Lifecycle Is An Evidence Boundary

The development TTY wrapper previously allowed `/tmp` as its runtime root and
did not distinguish build, input-guard, graphics-takeover, session, and return
phases in retained evidence. That was acceptable for bounded development but
could not prove an installed greetd login was independent of repository
fallbacks or identify the exact phase that failed.

The installed entry now declares its stricter contract explicitly: no source
build, no manual service control, an existing absolute user-owned
`XDG_RUNTIME_DIR`, and a real local Linux VT. The shared wrapper records ordered,
content-free lifecycle records from preflight through display-manager handoff.
A fixture-backed verifier rejects temporary runtime state, missing or reordered
phases, and emergency recovery presented as normal logout.

Normal, Firefox, and emergency recorders now retain that lifecycle beside the
release manifest, runtime identity, recovery, guard, and session evidence.
Repeated-cycle verification rechecks the lifecycle of every archived run.
This proves the installed-path contract and makes failures diagnosable; it does
not replace the remaining physical three-login, fallback-session, emergency,
ten-cycle, or soak captures.

## 2026-07-24: Installed Session Entries Follow Greetd Discovery

The first immutable lifecycle release installed its desktop entries below
`/usr/local/share/wayland-sessions`, but this host's explicit tuigreet command
scans `/usr/share/wayland-sessions`. The files were valid yet could not appear
in the menu. The system installer and current-release verifier now use
`/usr/share/wayland-sessions`, matching the configured greetd discovery
boundary. Staging tests continue to override the directory explicitly.

## 2026-07-24: Installed Kitty Baseline Remains Separate From xmonad

The first greetd-launched xmonad capture proved device discovery, 48 routed
keys, focused X authority, repeated Kitty Present retirement, and a responsive
hardware cursor, but the operator could not see usable terminal content. This
is not the earlier missing-device failure: the evidence points to an
xmonad-managed presentation/layout defect after the first configure.

The installed menu now exposes `Sophia Kitty (Baseline)` separately from
`Sophia xmonad (Experimental)`. Both use the same immutable binary, runtime
identity, VT guard, KMS renderer, X frontend, and cleanup path; only the opaque
session profile differs. Re-proving the known-good Kitty profile isolates the
shared installed/greetd boundary before changing xmonad layout behavior. It
does not promote or conceal the failed xmonad capture.

## 2026-07-24: XKB StateNotify Uses Post-Event Modifiers

The installed Kitty baseline exposed two keyboard gaps. First, Helix did not
recognize `:`. A strengthened real-Kitty smoke reproduced it: Kitty's keyboard
trace saw Shift press with no effective modifier, semicolon press as `;`, then
Shift only on semicolon release. Core key events correctly carry pre-event
modifier state, but Sophia incorrectly reused that state in XKB `StateNotify`,
whose effective modifiers describe the post-event state.

Routed key records now carry both values explicitly. Core and XI events retain
pre-event state while XKB notifications use the post-event state produced by
the per-seat xkbcommon machine. The real Kitty gate now types exact `:ll` and
requires shell receipt plus later Presents. A complete pc105 US symbol-table
regression covers printable base/shift pairs and F1 through F12.

Second, kernel VT shortcuts cannot operate while the graphical owner has set
the console keyboard to `K_OFF`; leaving translated console input enabled would
reintroduce typed bytes on the hidden TTY. The protocol-neutral session input
owner now recognizes Ctrl-Alt-F1 through Ctrl-Alt-F12, consumes the function-key
edges, and asks the controlling VT to activate the selected terminal. This is a
session-control action, not an application or X11 shortcut.

The first physical switch attempt exposed a launcher boundary: the graphical
owner is deliberately started with `setsid`, so `/dev/tty` is unavailable
inside it. The helper failed with `ENXIO`; treating that failed control action
as fatal correctly returned to greetd. The wrapper now passes the exact
originating `/dev/ttyN` as `SOPHIA_SESSION_TTY`, and the detached owner opens
that explicit device for VT activation. A launcher regression requires the
device handoff to precede `setsid`.

The next physical run refined that diagnosis: the path was correct, but
reopening `/dev/tty7` after detachment failed with `EACCES`. Device paths are
not durable capabilities across display-manager ownership transitions. The
launcher now duplicates its already-authorized controlling-TTY descriptor
before takeover and passes the descriptor number as
`SOPHIA_SESSION_TTY_FD`. Session-control helpers issue VT ioctls directly on
that inherited descriptor; the path remains only a compatibility fallback.
This keeps VT control in the session-control boundary without adding
terminal- or application-specific behavior to Engine.

The next physical run disproved descriptor inheritance as the final solution.
`VT_ACTIVATE` returned `EPERM`: Linux authorizes that ioctl by controlling-TTY
ownership, which the deliberate `setsid` boundary removes, rather than by the
mere possession of an open descriptor. Sophia then exited with status 1, so
the greetd screen observed after Ctrl-Alt-F3 was greetd reclaiming tty7; the
existing tty3 login had never become active.

VT switching now belongs to a libseat controller. Switch rejection is
nonfatal. A successful switch produces an explicit release boundary that
stops input, drains and releases native scanout, and acknowledges suspension;
acquisition rebuilds both hardware domains and repaints the retained scene.
Kitty, X11 clients, focus, and Engine state remain above that hardware
lifecycle, and Engine gains no application-specific branch.

The first physical return from tty3 exposed an incomplete authority boundary.
libseat delivered disable and enable correctly, but Sophia reopened
`/dev/dri/card*` with ordinary filesystem access after enable. AMDGPU rejected
the initialization with `EACCES`, and the session exited cleanly to greetd.
Login-session ACLs are not the device authority once libseat owns the session.

The live backend now runs the non-`Send` libseat handle on a dedicated broker
thread. KMS card and udev-libinput opens request libseat device leases; backend
objects receive duplicated descriptors while the broker retains and closes
the lease token. Suspension drops input and KMS resources before acknowledging
disable, and acquisition obtains fresh leases before rebuilding them. Direct
device opens remain available only to standalone validation paths that do not
participate in the managed live-session lifecycle.

Physical validation of the installed Kitty baseline then completed the missing
proof. Ctrl-Alt-F3 released the graphical seat and exposed the already-active
text login; the Sophia session remained alive. Repeated Ctrl-Alt-F7 returns
reacquired KMS and input, repainted Kitty, and preserved interactive keyboard
and pointer operation. Switching away again continued to work. This promotes
the libseat-backed Kitty session to the known-good installed baseline while
leaving the full F1-through-F12 matrix and xmonad workflow as separate open
proofs.

The first xmonad VT attempt exposed an ordering race hidden by the quieter
Kitty workload. The xmonad profile had a primary-plane frame in flight when
Sophia called `libseat_switch_session`. Seat authority moved before the owner
drained that frame, so its page-flip callback could no longer arrive. Waiting
500 milliseconds after revocation then failed with `persistent native scanout
remained in flight during teardown` and incorrectly ended the whole session.

Operator-requested VT changes now use a prepare-before-release boundary shared
by every profile. The owner stops input, prevents further native submission,
retires and releases KMS work while the seat is still active, drops the old
leases, and only then requests the switch. A request rejection or missing
disable event rebuilds hardware and repaints instead of ending the session.
An unsolicited disable cannot be drained after authority is gone; that path
immediately detaches native state, reports any abandoned scanout, completes an
already-submitted Present as `Skip`, and preserves queued work for acquisition.

Physical xmonad switching then proved KMS survival but exposed a separate input
state boundary. Ctrl and Alt presses had already reached the focused X client
before the function-key press identified the sequence as a VT chord. Their
physical releases occurred on the text VT, outside Sophia's libinput ownership,
so XKB and the WM shortcut router retained both modifiers after acquisition.
The reopened libinput poller was healthy; application input was interpreted
with stale Ctrl-Alt state.

VT activation now emits synthetic releases for every pressed chord modifier,
clears the WM seat state, and waits up to 500 milliseconds for X Authority to
acknowledge those deliveries before KMS quiesce and `switch_session`. Failure
to flush rejects the switch without ending the graphical session. Suspension
still clears local keyboard state as a second boundary, but no longer relies
on that local reset to repair client-visible XKB state.

The next physical xmonad capture isolated a distinct multi-output retirement
failure. Output 2 submitted its third startup frame but never produced another
page-flip callback; output 1 continued submitting and retiring normally. Both
VT attempts therefore reached the prepare boundary with one permanently
in-flight output and timed out before `switch_session`. Emergency shutdown hit
the same strict drain and returned status 1.

Native suspension now has one data-oriented result for both authority states:
whether all callbacks drained, how many scanouts were abandoned, and which
submitted Present was settled as a Skip. While authority remains active the
owner still attempts exact retirement first. A bounded timeout transitions to
the same detached runtime representation used after unsolicited revocation,
then drops the native leases before requesting the VT switch. Final teardown
uses this operation as well. This keeps the missing kernel callback observable
without allowing it to wedge VT control or emergency recovery, and avoids
duplicating detach/Present-settlement policy across lifecycle callers.

The following physical run proved that boundary: an owner-requested VT switch
timed out with one abandoned scanout, detached before release, resumed both
hardware domains, preserved the action-launched Kitty, and completed final
logout with a fully drained scanout. The remaining status-1 exit was unrelated:
two RANDR `GetOutputProperty` requests used atom `None` and correctly received
`BadAtom`, but the session counted those optional client probes as unexpected.

Protocol reduction now recognizes only the complete probe tuple (`BadAtom`,
RANDR `GetOutputProperty`, atom `None`) as expected. The client-visible reply
does not change, and an unknown nonzero atom remains unexpected. The physical
verifier also now distinguishes Sophia's structured failures from harmless
Kitty/GLFW stderr and uses xmonad's actual action identity `768` for
Super-Enter; action `1` remains focus-next. Mutation fixtures preserve each
distinction so the acceptance gate cannot silently regress to broad error
whitelisting or the former action-ID alias.

The next physical start exposed a false-positive readiness gate rather than an
application-launch failure. Kitty mapped, xmonad committed layout and focus,
and output 1 repeatedly retired mixed Present transactions. Output 2 submitted
its startup frame but never delivered a callback. Despite that partial KMS
state, the owner reported `status=ready`: the CPU fallback examined only the
first output, while the DMA-BUF path treated transaction retirement as visual
proof without inspecting the composed pixels. Completion later reported zero
CPU detail, no output-1 nonzero export, and no output-2 retirement.

Startup readiness now reduces flat evidence for every owned output and requires
at least one callback from each. Mixed composition captures a bounded one-time
GPU readback, carries its nonzero RGB count with the exact submitted
transaction, and becomes stable only after that content retires. A missing
output callback after 750 milliseconds, or retired mixed frames without visible
pixels after 1500 milliseconds, triggers one shared native detach/reopen and
repaint through the existing libseat authority. The eight-second deadline
remains authoritative; a second failure exits through guarded cleanup instead
of accepting a cursor-only desktop. Engine remains protocol- and
application-neutral.

## 2026-07-25: Per-Output Pending Content Is Required For Native Submission

The four-Kitty proof exposed a false clean-shutdown result. Output 2 accepted a
CPU commit after the startup terminal exited but never produced its final
page-flip callback. Logout therefore timed out and abandoned one scanout, while
the focused verifier accepted the detached runtime because its final
`native_in_flight` value was false.

The deeper error preceded logout. A primary-owned mixed Present queued content
only for the primary output but still executed the native scanout tick for
every output, producing repeated secondary submissions with `content=None`.
CPU composition also requeued the unchanged secondary marker whenever only
primary content changed. Native submission now requires explicit pending
content, primary mixed Presents service only their owning output, and typed
CPU-content reduction suppresses a frame only when the same CPU content is
already pending, submitted, or displayed. A mixed-to-CPU correction is never
suppressed by an old checksum.

Native suspension now reports a typed drained, timeout-detach, or revoked-seat
outcome. Forced detach remains a bounded liveness mechanism, but it is not
clean evidence. The four-Kitty verifier requires an exact drain, zero
abandoned scanouts, balanced per-output callbacks and retirements, and no
empty-content submission. Engine remains application-neutral; the CLI only
orchestrates and reports the Engine-owned lifecycle.

## 2026-07-25: Pending Layout Commit Merges Concurrent Surface Admission

The first run after per-output hardening reached the four-window transition,
then exited with `new WM surface is missing from live layout`. Rapid
Super-Enter actions admitted another Kitty while an older resize proposal was
waiting for matching pixels. Authority intake inserted the new surface into
the live layout and unmanaged set, but committing the older proposal replaced
the layout with its pre-admission snapshot. The unmanaged ID survived without
its layer, so the next manage request correctly rejected the inconsistent
state.

Pending layout snapshots now merge every authority observation not owned by
that proposal's requested-size set. A concurrently admitted surface is
preserved for the next blind-WM manage request, and ordinary pixel updates for
unrequested surfaces advance with the pending snapshot. Resize-owned surfaces
remain quarantined until their matching pixels arrive. The merge is a pure
data reducer with integration coverage for insert, replace, and resize-owned
outcomes.

## 2026-07-25: Output Baselines And Launch Pressure Are Separate Lifecycles

The next rapid Super-Enter run did not fail in Engine, KMS submission, or X11
protocol handling. Initial modeset had already displayed the synthetic marker
on the secondary output, but modeset itself produced no page-flip callback.
The new unchanged-CPU-frame reduction saw the same displayed checksum and
suppressed the first event-bearing flip. Output 2 therefore remained at zero
callbacks, startup never reached its all-output proof, and the eight-second
deadline ended the session while several action-launched Kitty clients were
entering resize transactions.

An unchanged displayed frame is now suppressible only after that output has
observed a callback. Before then, the reducer emits a baseline-required outcome
and queues exactly one nonblocking flip; matching pending and submitted frames
remain deduplicated. Startup readiness is a monotonic passive record pinned to
the startup surface rather than whichever later surface owns focus.

Application launch pressure is also bounded independently from visual
authority. The CLI session supervisor retains a sixteen-entry FIFO across
active and queued action applications, waits for one opaque surface admission,
matching pixel retirement, and a settled layout pipeline before spawning the
next, and treats capacity, spawn, exit, or admission timeout as an application
outcome rather than a fatal session error. Global scanout quiescence is not an
admission condition: continuously presenting clients may supersede frames
without invalidating the fact that the new surface was displayed. Logout
cancels pending work. Engine and the blind WM remain application-agnostic.

## 2026-07-25: X11 Controls Leave The Presentation Owner's Wait Path

The delayed native submission during the four-Kitty workload was an ownership
bug rather than a renderer throughput limit. The live owner sent X11
configure, rollback, focus, clear-focus, and close requests and then waited as
long as 500 ms for each acknowledgement. During that wait it could not poll
DRM retirement or service input.

Those requests now enter a fixed-capacity session-control ledger. Configure
and close controls may be in flight concurrently; focus and clear-focus are
serialized globally. Acknowledgements correlate on client, command kind,
transaction, and surface. Channel pressure leaves work queued for a later
owner tick, while rejection, timeout, disconnect, duplicate identity, and
unexpected acknowledgement fail closed.

Engine focus remains authoritative. Client focus becomes applied only after
the matching X authority acknowledgement. While Engine and frontend focus
differ, physical routing retains cursor motion, emergency exit, VT switching,
and WM shortcuts but suppresses client keyboard, button, and axis delivery.
The owner services controls at both tick boundaries and limits its authority
wait to one millisecond while controls remain pending.

The CLI emits a separate `sophia_live_session_control` record with balanced
lifecycle counts, peak depth, queue dwell, and acknowledgement latency. The
four-Kitty verifier requires a drained failure-free ledger and bounds both
latencies to 100 ms. Synchronous initial-modeset evidence also moved from the
backend library to the CLI evidence boundary, removing direct library output.

## 2026-07-25: Focus Suppression Must Preserve Key Symmetry

The first physical run with asynchronous focus controls completed cleanly, and
all 28 controls were delivered without rejection or timeout. Keyboard hardware
also remained active. The remaining apparent keyboard loss was an input-state
ordering defect: a key press could reach the old X client immediately before a
focus handoff, while its physical release arrived during the deliberately
suppressed Engine/frontend focus mismatch. The WM observed that release, but
the X client did not, leaving keys such as Super logically pressed.

The live input boundary now retains a fixed-capacity data record of key presses
actually delivered to each surface, seat, and device. Before a focus,
clear-focus, VT, seat-release, or logout handoff, it sends a release for every
record owned by the old client and includes those releases in normal delivery
accounting. A later physical release without a matching delivered press is
suppressed instead of being sent to the new client. Surface removal clears any
remaining record and updates the local modifier reducer, preventing stale
state from crossing a client exit.

Completion evidence reports peak pressed-key depth, synthetic releases,
suppressed orphan releases, surface-removal cleanup, and final debt. The
four-Kitty verifier requires final pressed-key debt to be zero.

The close-window physical proof refined the ordering requirement. X authority
owns one XKB reducer per seat, not one per surface. Clearing two local records
when Meta-Shift-C removed its surface left Meta and Shift pressed in that
seat-wide reducer, so the replacement Kitty inherited modified input. Control
dispatch is now held behind the exact synthetic-release delivery IDs. A close
or focus request cannot reach its X control writer until every preceding
release has been acknowledged by X authority. Completion and the physical
verifier require both the pressed-key ledger and this release barrier to be
empty, with no keys abandoned during surface removal.

Client-initiated exits add a different boundary: a terminal may destroy its
surface in response to Return before the physical Return release arrives.
There is then no live target for an X event, but the seat-wide XKB reducer must
still observe the release. Routed input now distinguishes ordinary
Engine-selected delivery from a state-only seat update. Surface removal emits
state-only releases for its residual key records; X authority updates XKB
without resolving a surface or emitting an event to the newly focused client.
This preserves global keyboard state without inventing an application target.

## 2026-07-25: Composition Reuse Requires A Lease-Aware Pool

The successful keyboard proof still reported 201 composition-target and GL
pipeline creations for 201 mixed exports. CPU and direct DMA-BUF paths already
returned their persistent target to the renderer after exporting a locked
front buffer. The mixed-composition path instead destroyed its target on every
successful export even though the exported buffer retained an `Arc` lease on
the GBM/EGL surface until scanout retirement.

An attempted optimization returned the context, surface, and GL pipeline to
the per-output target slot after a successful export. The exported front
buffer independently retained the originating surface, and the verifier was
temporarily changed to require zero target recreations.

The first physical run of this lifetime change aborted on the third render
after AMDGPU rejected the command stream. Moving startup proof from a
post-swap CPU map to a pre-swap GL readback produced the same third-render
abort, disproving front-buffer mapping as the root cause.

The invalid lifetime is single-surface reuse while a front buffer from that
GBM surface remains leased to KMS. Mixed composition therefore returns to the
previous fail-safe rule: destroy its rendering target after each successful
export while the exported buffer's independent surface lease survives through
scanout retirement. Future reuse must use a bounded lease-aware pool and only
select a target whose surface has no exported buffer owners. Verbose tracing
still captures one representative composition frame instead of synchronously
reading every frame.

## 2026-07-25: Startup Evidence And Input Waits Follow Their Owners

The first clean run after restoring fail-safe composition lifetime survived
201 mixed frames and completed with balanced callbacks and cleanup. Its strict
verifier still found two evidence and latency defects.

Per-output synchronous-modeset records were printed before
`LiveProductionVisualRuntime` initialized native scanout, while every head
still carried an empty initial-modeset state. The later aggregate
`output_baseline_ready` record observed the correct state, but the detailed
records had already been skipped. Native heads now retain the exact initial
submission identity, and the CLI emits each detailed record at the same
readiness transition as the aggregate record.

The physical input worker dispatch gap remained one millisecond, but an event
could enter its queue immediately after the owner drained input. The owner then
waited as long as 25 ms for X authority work before a composition taking as
long as 75 ms, producing 120 ms of measured queue dwell. Physical-input
presence now selects the existing one-millisecond owner wait budget. Cursor
and control work retain that same budget, while sessions without those
latency-sensitive sources keep the 25 ms idle wait.

## 2026-07-25: Retained Contexts Across Fresh Surfaces Are Unsafe

The fail-safe mixed path bundled four lifetimes into one target: EGL context,
GL pipeline, EGL window surface, and GBM surface. Destroying that bundle after
every successful export protected the leased front buffer but also rebuilt the
context and shaders on every frame. This explained the repeated composition
creation count and left input and page-flip latency coupled to driver setup.

The attempted lifetime split created a distinct GBM/EGL surface for every
export while retaining the EGL context and GL pipeline. Physical startup
presented the first two mixed submissions successfully. The third render then
aborted with `amdgpu: The CS has been rejected ... (-2)`. This falsifies the
assumption that avoiding same-surface reuse alone is sufficient on this stack:
the context and pipeline cannot safely be rebound across independent leased
window surfaces either.

The fail-safe path again destroys the composition context and pipeline after
every successful export while the exported buffer retains its surface through
KMS retirement. Resource evidence still distinguishes complete-target and
frame-surface creation so this invariant is machine-checkable. The next
optimization is a bounded generational pool of complete targets, not a pool of
surfaces behind one context. A slot may become free only through explicit
page-flip retirement, never through reference-count inference.

## 2026-07-25: One-Shot Targets Restore Physical Stability

The first physical cycle after removing every retained GPU target completed
normally with 253 mixed exports. Target, pipeline, frame-surface, and successful
target-retirement counts were all 253. Generation and recovery replacement
were zero, no AMDGPU command stream was rejected, native submissions and
callbacks drained, and the lifecycle returned exit status zero.

The strict gate exposed a separate owner-scheduling defect. Of 254 callback
observations, 248 completed within 20 ms and three exceeded 100 ms. Two
213--214 ms stalls coincided with managed terminal exit and xmonad resize
epochs; a 171 ms stall covered the final blank frame after the startup terminal
exited. Target creation peaked at 4 ms and rendering at 47 ms, while input
queue dwell reached 191 ms. The next fix must prioritize native callback and
input draining across child-exit and layout-transition work. The latency budget
must not be relaxed to hide these isolated starvation events.

## 2026-07-25: Native Retirement Precedes Shortcut RPC

The physical owner previously routed input before servicing native retirement.
A global shortcut could therefore enter the external WM transport's synchronous
request path, whose configured response timeout is 500 ms, while an accepted
KMS callback waited in the native event queue. The same wait could accumulate
physical input in the acquisition queue.

Native service now precedes shortcut routing. Completion evidence separately
records maximum child-reap, input-routing, and WM-request durations, and the
four-Kitty verifier caps each at 100 ms. This is an ordering correction, not a
claim that synchronous WM transport is suitable long term. If the next
physical evidence attributes the remaining input dwell to WM request time, WM
actions move to a bounded typed worker with explicit response correlation and
stale-response rejection.

## 2026-07-25: WM Socket Wait Leaves The Physical Owner

The next two-output, four-Kitty hardware cycle completed without a crash,
AMDGPU rejection, resource replacement, or cleanup debt. It recorded a 180 ms
maximum external-WM request, a 100 ms physical-input phase, 246 ms input queue
dwell, and a 210 ms submit-to-page-flip observation. Child reaping peaked at
25 ms. The correlated peaks identify synchronous WM socket waiting as the owner
starvation source; renderer and target lifetimes remained stable.

WM transport now uses a capacity-one worker channel behind a sixteen-entry
owner request bound. Exactly one packet is in flight. Passive request and
completion records carry the transaction ID; the owner correlates them and
rejects a response when the current layout topology or geometry no longer
matches the request fingerprint. Surface removal remains serialized before its
relayout so the latter is planned from post-removal workspace state. The owner
alone validates and commits proposals and applies focus, workspace, launcher,
close, and logout effects. A neutral empty coordinator batch lets a WM-only
transaction reach Engine without waiting for unrelated X11 traffic; it is not
counted as an X authority batch.

Completion now reports owner timing separately from WM transport depth,
rejections, stale responses, queue dwell, and round-trip latency. The external
round trip retains its 500 ms fail-closed bound; it is no longer misclassified
as owner-thread execution time.

## 2026-07-25: Asynchronous WM Physical Gate Passes

The first two-output four-Kitty cycle after moving WM socket waits off the
owner thread completed normally. Maximum physical-input phase time fell from
100 ms to below the millisecond evidence resolution, input queue dwell fell
from 246 ms to 12 ms, and submit-to-page-flip observation fell from 210 ms to
23 ms. The WM transport reached a 101 ms round trip without holding the owner;
its peak depth was one and it drained with zero rejection, stale response, or
pending request.

The session produced 220 mixed exports with exactly 220 complete targets,
pipelines, and frame surfaces, zero recovery or generation replacement, clean
callback retirement, and clean input, control, protocol, and process teardown.
The fourth-window transition atomically held and committed four surfaces
because xmonad promoted the newly focused window to master. The verifier had
assumed the old master remained fixed and required exactly three changed
surfaces. It now correlates the held transaction with its matching commit and
accepts either three or four changed surfaces while retaining the exact
pixel-matched four-pane geometry checks.

## 2026-07-25: Cursor And Primary KMS Commits Must Be Serialized

The next physical cycle ran the full four-Kitty workload but failed promotion
after one early primary-plane atomic submission was rejected. The failure
followed physical pointer motion; every later submission recovered, 182 mixed
exports retained exact complete-target lifetime, native drain completed, and
the final failure count remained exactly one.

The owner ordering exposed a KMS transaction race. Native service could submit
a nonblocking primary flip, then the cursor path independently issued a
nonblocking cursor-plane commit on the same card. A successful cursor ioctl
only admitted that asynchronous commit; it did not prove the cursor update had
completed before the next owner tick submitted another primary request. The
earlier cursor-side `EBUSY` handling covered only the opposite ordering, where
a cursor update encountered an existing primary commit.

Backend-live now treats primary page-flip state as the admission boundary for
cursor work. A dirty cursor remains coalesced while any primary flip is in
flight. Once admitted, the cursor-only atomic commit is blocking, so it cannot
remain pending across the next primary submission. Completion evidence records
both primary-in-flight deferrals and maximum cursor update duration, and the
physical verifier caps the latter at 100 ms. This is the bounded daily-driver
repair. The long-term graphics path should build primary and cursor plane state
through one per-output atomic KMS transaction owner rather than preserve two
independent commit builders.

The required xmonad real-client preflight exposed an independent proof
regression before the next physical run. Its headless configuration has one
virtual output and intentionally disables native scanout, but startup readiness
waited for the native-only `OutputsPresented` event. The owner now satisfies
that output fact only when native scanout is absent; physical sessions still
require every real output's callback or synchronous modeset evidence. The
preflight verifier also correlates the injected resize transaction directly,
instead of assuming initial placement and the later 960x640 configure collapse
into one asynchronous WM response. The corrected preflight passes with matched
configure delivery, later pixels, exact synthetic input, and clean teardown.

## 2026-07-25: Workspace Policy Must Project Visibility Into Every Consumer

The post-cursor physical cycle validated the KMS repair with zero native submit
failures and a 12 ms maximum cursor update, then exposed an independent
workspace defect. Super-2 committed workspace policy and cleared focus, but the
owner continued presenting all four workspace-1 surfaces. The private xmonad
server also kept their synthetic windows mapped. Launching Kitty on workspace 2
therefore made xmonad tile five cross-workspace windows; all five resize
requests timed out, rollback rejected four queued Presents, and logout later
detached one unretired scanout. Shortcut IDs remained distinct: the log recorded
workspace actions 257 and 258, application action 768, close action 769, and
logout action 772.

Workspace focus is now stored by workspace and projected onto the outputs that
currently display it, so hiding a workspace clears client focus without
destroying the focus that should return with it. The live owner filters
presentation layers through Engine's workspace visibility before composition;
the same order bounds hit-testing and retained mixed layers. Relayout requests
contain only nodes assigned to the active workspace. The blind xmonad bridge
tracks its active workspace and mapped synthetic-window set, issuing explicit
unmap/map transitions so hidden windows cannot influence legacy layout policy.
CPU composition consumes the same ordered visible-surface projection, including
the empty-workspace case. A Present targeting a surface outside that projection
settles as a skip before any native submission; it cannot append itself back
into the mixed frame. These are protocol-neutral state projections; no terminal
or application identity enters Engine or rendering.

The first physical workspace run then proved that filtering composition is not
enough by itself. Super-2 committed action 258, cleared the focused client's
keys and focus, and stopped accepting its Presents, but submitted no replacement
KMS frame. The previously scanned-out workspace therefore remained visible
while keyboard routing was correctly disabled. The CPU-cycle preservation rule
had examined every committed DMA-BUF surface rather than the visible projection
and suppressed the empty-workspace repaint.

GPU preservation now reduces only the ordered visible transaction set. A
visibility-order change cannot be discarded by ordinary CPU coalescing. An
empty projection queues a black CPU frame; a returning projection queues its
retained mixed DMA-BUF frame when available, and otherwise paints the bounded
CPU background until the client supplies new pixels. Thus a workspace commit
always has a concrete scanout consequence instead of leaving the old workspace
on screen.

The follow-up physical cycle exercised workspace 1 seven times, workspace 2
nine times, and workspace 3 twice. Empty projections submitted the stable
blank CPU frame; populated projections restored their retained mixed frame and
focus. The run recorded no layout timeout or resize abort, zero native submit
and retirement failures, a clean native drain, and bounded completion after
122 submissions and 120 asynchronous retirements. The operator confirmed all
three workspaces visually.

Future captures no longer rely on that visual statement alone. Every committed
WM policy state now emits a reduced projection record containing only
transaction, output, workspace, visible-surface count, and whether focus is
present. The strict verifier requires workspace 2 and 3 to commit empty,
requires workspace 1 to return with visible surfaces and focus, and correlates
the focus-clear, suppressed-key, workspace-3, return, and focus-restore order.

The first capture with projection schema 2 populated Kitty independently on
workspaces 1, 2, and 3, committed 25 layouts without a timeout, and closed one
workspace-2 client without disturbing the others. It completed with zero
native submit, retirement, callback, control, or protocol failure. The control
ledger drained 22 enqueued and delivered commands with 17 ms maximum queue
dwell and 14 ms maximum acknowledgement latency; the input ledger drained
2,258 events with no pending key state.

This was a valid workspace and control-ledger proof, but not the complete
standard workflow. The startup Kitty remained open, and the capture contained
no click/drag button route, focus-next action, next-layout action,
hidden-workspace key suppression, or VT lifecycle. The strict verifier
correctly stopped at the missing startup exit instead of treating a clean
logout as evidence for steps that were not performed.

## 2026-07-25: Core Selection Ownership Must Be Queryable

The next physical cycle covered the normal interaction set in a different
order. Mouse selection worked, but Ctrl-Shift-C and Ctrl-Shift-V did not.
Kitty's GLFW layer repeatedly reported that it failed to become owner of the
clipboard selection. This distinguishes the defect from physical key routing:
Kitty received the copy chord and attempted the X11 ownership transition.

The frontend accepted `SetSelectionOwner` into its namespace-aware selection
table, but core `GetSelectionOwner` unconditionally replied with `None`.
GLFW performs the standard set-then-query ownership check and therefore
correctly treated every copy as failed. Core owner queries now return only the
owner visible in the caller's admitted namespace. Classic shared-X clients see
the shared owner, while a confined client cannot discover an owner in another
namespace. The wire regression covers the visible and confined cases.

Normal-session evidence now reports only owner-change and conversion counts;
clipboard content remains redacted. The strict physical gate requires both
operations, rejects GLFW ownership failures, and retains operator confirmation
that the selected text was pasted unchanged.

The first retest acquired clipboard ownership twice with no GLFW ownership
failure, and the operator confirmed that paste appeared to work. It produced
no `ConvertSelection`, however, because the copy and paste occurred inside one
Kitty process; Kitty can reuse its locally held selection without a protocol
round trip. The promotion sequence now pastes into an independently launched
Kitty before the owner exits, so the conversion witness measures the intended
same-namespace X11 path.

That retest also exposed an independent pointer-boundary defect. After paste,
the hardware cursor disappeared. Input remained healthy and the final record
showed 160 successful cursor-plane updates with zero failures, but only 1,202
of 8,158 observed pointer events routed to a surface. Session pointer placement
had applied the libinput accumulator plus its startup offset without confining
the result to any Engine output. The KMS cursor owner deliberately detaches the
cursor plane when given a point outside every output, so that reachable path
matches the reported disappearance without producing a hardware failure.

Engine now provides one output-union confinement system. The live session
projects its existing ordered output topology into that system before physical
input starts. Confinement chooses the nearest valid point across all output
rectangles, including unequal output heights, and corrects the raw-to-logical
offset at the edge so discarded overshoot does not create a sticky boundary.
Integration coverage drives positions past every side and into the dead area
beside a shorter output. Completion evidence now counts intentional hidden
updates separately from successful updates and failures; the strict gate
requires zero. Physical edge/reversal confirmation remains pending.

The first confinement repair still left its accumulated raw position, startup
offset, corrected edge offset, and current logical position in the CLI owner.
That was the right behavior in the wrong authority: the CLI could effectively
choose Engine cursor state, and the confinement helper alone could not prove
that a real input stream reversed immediately after overshoot.

`OutputUnionPointerState` now owns that complete state machine inside Engine.
The live owner supplies only immutable output rectangles, the optional initial
surface geometry, and each raw backend point. Engine returns a logical
placement plus reduced boundary contact/reversal facts; no backend handle,
device identity, client metadata, or pointer coordinate is logged. Deterministic
coverage proves all four edge directions and unequal-output projection.

The rebuilt two-output `xmonad-m7` guest then drove the real virtio-mouse path
hard against the right edge and sent one 96-unit reverse delta. Engine emitted
an output-edge contact followed by an immediate-reversal observation, after
which the complete click-drag focus, keyboard, workspace, bridge-restart,
launch/close/logout, and native-drain workflow passed with zero protocol,
cursor-plane, stale-WM-response, or cleanup failure. This is unattended
evidence for the state machine; the physical gate still requires every edge of
the actual output union and visible hardware-cursor confirmation.

## 2026-07-25: Held-Key Repeat Belongs To Engine Timing And Frontend Semantics

The next physical run retained ordinary keyboard routing but exposed that
holding Backspace or an arrow produced only one action. This is expected from
libinput: it reports physical press and release edges; the display stack must
schedule held-key repeat. Sophia had no such scheduler.

Engine now owns a fixed-capacity, allocation-free repeat clock with one active
repeatable key per seat. The live owner binds that record to the exact focused
surface, seat, device, and physical key. The configured XKB map determines
repeatability, so editing and cursor keys repeat while modifiers and Super do
not. Existing focus, workspace, surface-removal, and VT release barriers cancel
the bound record; a repeat can never migrate to a newly focused client.

The X frontend receives an explicit repeat delivery mode. It emits another
KeyPress with current XKB modifiers without replaying a physical state
transition or reactivating a passive grab. This keeps Engine timing
protocol-neutral, preserves X11 delivery authority, and prevents xmonad global
shortcuts from repeating. Missed timer intervals coalesce to one pulse rather
than bursting after a slow frame. Completion evidence requires the scheduled
and routed counts to match, capacity exhaustion to remain zero, and every seat
to drain.

The first physical capture routed and acknowledged 66 held-key pulses with
zero missed-interval coalescing and zero repeat-seat capacity exhaustion. The
operator confirmed the editing/navigation behavior, and logout left no active
repeat seat or pressed-key debt after all 1,289 expected input deliveries
flushed. The run retained clean input, cursor, protocol, renderer, KMS, and
frontend teardown. It did not complete the broader promotion sequence: the
startup Kitty remained open and the clipboard peer performed no
`ConvertSelection`, so those independent gates remain pending.

## 2026-07-25: Override-Redirect Is A Presentation Role, Not WM Metadata

Unmodified xmobar creates its bar with `CWOverrideRedirect`, requests top
geometry, publishes dock/strut properties, and maps the window. Sophia
previously discarded that attribute, reported it as false in replies and
events, and admitted every rendered surface to the blind xmonad policy stream.
That would tile a status bar as an ordinary application even if all of
xmobar's drawing requests succeeded.

The X frontend now owns the exact override-redirect bit in its passive window
record and returns it through core X11 semantics. Across the authority
boundary it reduces the bit to `SurfacePresentationRole::ClientPositioned`.
The live Engine layout retains client geometry, composition, hit testing, and
top presentation for that role, while withholding the surface from WM
management. No XID, class, title, atom, dock type, or application identity is
sent to xmonad.

The xmonad TTY launcher can now discover an installed xmobar or an executable
built from the operator-selected source checkout, then supervise it as a
secondary X client with a deterministic local config. Xmobar and xmonad remain
unmodified compatibility clients. This first slice intentionally overlays the
bar: interpreting `_NET_WM_STRUT_PARTIAL` as protocol-neutral output
reservations is the next work-area step and requires retained physical
evidence before promotion.

The first real-client trace exposed two generic drawing gaps rather than an
xmobar-specific condition. Xmobar's Cairo backend uses MIT-SHM `GetImage` and
`PutImage` against an offscreen pixmap, followed by core `CopyArea` into its
window. Sophia initially omitted `GetImage` and then acknowledged pixmap
`PutImage` while discarding its bytes, so the final window transaction was
blank. Pixmap dimensions and software pixels are now passive authority data;
upload, readback, and copy use the same drawable buffer path as other software
clients. The unmodified xmobar 0.51.1 smoke subsequently completed 163 requests
across 28 opcodes with two committed transactions, 8,967 nonzero pixel bytes,
and `first_error=none`. The source checkouts for xmobar and xmonad were not
modified.

The first physical session then proved that xmobar stayed supervised, retained
its client-positioned role, and continued publishing nonzero CPU buffers, but
the bar was not visible above Kitty. The defect was in the generic mixed
renderer boundary: all CPU surfaces were flattened into one output-sized
background before the current and retained DMA-BUF layers were appended.
Flattening discarded the per-surface ordering needed for a CPU overlay, so the
later Kitty layer covered valid bar pixels.

Mixed presentation now snapshots CPU surfaces as passive
surface/geometry/buffer records and reduces the Engine presentation order into
one interleaved CPU/DMA-BUF layer sequence. The scheduler owns that immutable
snapshot with each queued Present; the renderer remains unaware of xmobar,
Kitty, X atoms, or window roles. Reducer regressions cover both a CPU overlay
above GPU clients and an ordinary CPU client below the current GPU surface.
The xmobar smoke was also strengthened to require six committed transactions
and nonzero pixels in the newest redraw; the retained run completed 215
requests across 28 opcodes with 25,929 nonzero bytes and no protocol error.
Corrected physical visibility remains pending, and neither client source tree
was modified.

The next physical run rendered the status bar, confirming the corrected
surface order, but Kitty never became visible. Kitty did start, registered
three DMA-BUFs and fences, submitted 19 Presents, and Super-Enter committed a
`LaunchTerminal` action. All 15 attempted mixed native submissions failed
before KMS with `ScanoutExportFailed`, after which the startup focus-control
gate timed out.

The remaining defect was a stale renderer invariant. Its persistent CPU
texture was allocated at output size and `draw_cpu_layer` rejected every other
extent because the previous mixed seam supplied only a flattened full-output
background. A correctly represented status bar is instead a narrow CPU layer.
The native pipeline now tracks the texture's allocated extent, reallocates it
only when the next CPU layer differs, and uses the existing sub-image fast path
for same-sized redraws. This supports arbitrary application-agnostic CPU
layers without allocating a new texture for each bar update. A passive reducer
test locks the reallocate-versus-update decision; corrected physical
bar-plus-Kitty presentation remains pending.

## 2026-07-25: Work Areas Are Engine Policy Facts, Not Dock Metadata

The corrected mixed renderer made xmobar and Kitty visible together, then
exposed that Kitty still began at the root origin beneath the bar. Rendering
was healthy: the run completed 141 mixed exports with matching target,
pipeline, and frame-surface lifetimes and no native failure. The missing
mechanism was layout reservation, not another renderer special case.

The X frontend now decodes exact CARDINAL `_NET_WM_STRUT_PARTIAL` and legacy
`_NET_WM_STRUT` properties in client byte order. Valid partial data takes
precedence; legacy data is the fallback. The authority emits complete,
bounded `SurfaceOutputReservations` replacements keyed by Sophia `SurfaceId`.
Atoms, XIDs, dock types, titles, classes, and application identity stay inside
the frontend. Malformed values remain legal X properties but produce no
reservation.

Engine owns the lifecycle table and pure work-area reducer. Only mapped
`ClientPositioned` surfaces are active. Replacement, deletion, unmap, and
surface removal update the table; same-edge depths take the maximum, different
edges combine, and partial root spans clip independently against each output.
An empty aggregate is rejected so the session preserves its last valid work
area. The full output remains available for composition and pointer hit
testing.

The live WM owner now stores reduced bounds in `WmWorkspaceState` without
changing workspace or focus state. Manage and relayout requests use that
stored rectangle, and the existing WM compatibility bridge forwards it as its
synthetic root bounds. Consequently the implementation is generic across
native WMs and legacy profiles; neither Engine, renderer, live session, nor
bridge contains an xmobar, xmonad, Kitty, or toolkit condition. Physical
bar-plus-Kitty geometry and lifecycle evidence remain the promotion gate.

## 2026-07-26: External WM Focus Ownership Starts Before Reconciliation

The guarded TTY3 pointer-focus run physically confirmed both plain-click focus
and click-drag focus. Each selected the intended Kitty, cross-window copy/paste
worked during the drag workflow, and Engine-owned focused borders made the
handoff visible. Workspace transitions exposed a separate transient: the
status bar briefly received a focused-border outline before the empty
workspace settled.

The defect was in generic initial-focus reconciliation, not xmobar or xmonad.
After external policy cleared Engine and X11 focus, the owner selected and
focused the first committed surface before checking whether an external WM
owned focus policy. It then returned without applying matching client focus.
The result was a hidden Engine focus, a one-frame compositor border, and
`control_plane_only` input suppression because Engine and frontend focus no
longer agreed.

Initial-focus candidate selection now rejects external-WM sessions before any
Engine mutation. A deterministic regression supplies a committed hidden
surface and requires no candidate. The two-output `xmonad-m7` guest then
switched to an empty workspace, recorded `focus=none`, suppressed both primary
button edges with `reason=no_target`, emitted no pointer-focus request or
client delivery, returned with Super-1, preserved focus through one
compatibility-bridge restart, and completed the independent click and drag
proofs plus clean logout. Reduced policy-suppression evidence remains available
for diagnosing future Engine/frontend focus transitions; it carries only mode
and counts.

The follow-up physical session exercised 36 workspace projections. Ten empty
projections retained `focus=none`; 26 populated projections restored focus.
No focused-border composition occurred between an empty projection and its
next legitimate focus restoration, no pointer button was suppressed by the
focus-transition policy, and the session completed with clean protocol, WM,
input, native-scanout, frontend, and namespace state. This closes the transient
status-bar border regression without adding client-specific chrome policy.

## 2026-07-26: Private Legacy-WM Displays Are Leased One-Shot Endpoints

A physical clipboard run exited during startup after the compatibility bridge
reported `no private X display available` ten times. The frontend display was
healthy; the failure was isolated to the bridge's synthetic X facade. Earlier
forced and aborted sessions had left owner-only socket nodes for every display
in the bridge's original `:90..:199` allocation range. Unix socket names remain
occupied after an ungraceful process exit, and the allocator treated each stale
name as a live policy endpoint.

The bridge now separates display-number ownership from the one-shot socket
name. A process-scoped file lease serializes each bounded display number, the
allocation range extends through `:4095`, and the socket name is unlinked as
soon as the configured legacy WM connects. The established Unix connection
continues to carry the synthetic X protocol, while a later abort has no socket
path left to leak. The lease remains held until the WM child and bridge worker
are stopped, and the kernel releases it even if the bridge is killed.

An integration regression launches two isolated fake legacy-WM processes at
once, requires distinct leased display numbers, and verifies that neither
accepted endpoint remains in `/tmp/.X11-unix`. This lifecycle belongs entirely
to the optional legacy-WM adapter; Engine, the real X frontend, renderer, and
blind WM protocol remain unchanged.

The immediate physical rerun confirmed the lifecycle fix. Sophia reached a
focused presented Kitty in 1,012 ms, entered workspace 3, launched two
independent peers, and completed 14 WM requests without a bridge restart or
degraded interval. The same run observed two selection-owner changes and one
selection conversion with content redacted. The operator confirmed that the
exact token was copied from the workspace-3 Kitty and pasted into the
independent workspace-1 Kitty after the workspace transition. The run flushed
all 1,397 expected input deliveries, retired native presentation without a
failure or live fence, logged out normally, restored the TTY exactly, and
revoked the frontend namespace. This closes the physical same-namespace
cross-workspace clipboard gate.

## 2026-07-26: Configuration Is Two Ownership Domains, Not an Override Stack

Sophia now resolves one strict KDL 2 source for session/Engine mechanism and a
separate source for Sophia-native WM policy. The user defaults are
`${XDG_CONFIG_HOME:-$HOME/.config}/sophia/config.kdl` and `wm.kdl`;
`/etc/sophia` and compiled snapshots are ordered fallbacks. There is no
include graph, field merge, KDL 1 fallback, or WM override of Engine policy.
External WMs continue to use their native configuration.

The new `sophia-config` crate contains only bounded passive schema data,
discovery, parsing, snapshots, deltas, last-known-good state, and a
parent-directory inotify source. Atomic editor replacement is therefore
observable without watching a stale inode. Unsafe ownership/mode, files over
one MiB, unknown or duplicate fields, broken references, invalid paths, and
the emergency chord fail closed.

Core candidates apply as one transaction. Application registry, repeat,
fallback chrome, and diagnostic changes are live-safe; mechanism changes mark
the complete candidate pending restart and do not leak its live-safe subset.
The session owner waits for an idle key ledger before replacement. Renderer
entry points now consume the Engine border style stored by the visual runtime
instead of recreating a default at each composition call.

The WM API advances to version 5. Negotiation carries a nonzero policy
generation and bounded chrome preference, while Engine continues to own
geometry, damage, rendering, and scanout. Generation-ordered update/ack
packets and an idle-shortcut reducer establish the hot-update contract.

The supervised transport now completes that contract. The socket worker
forwards immutable unsolicited candidates to the Engine owner and returns the
owner's exact-generation acknowledgement; it never applies policy itself. The
WM suspends new-policy request service until acknowledgement. Both ends also
handle the race where a bounded Engine request is already in the socket: the
worker accepts the intervening policy frame, the WM holds the request, and the
response follows the applied acknowledgement. Socket integration coverage
exercises atomic file replacement, generation delivery, request deferral, and
acknowledgement ordering.
## 2026-07-26: fixed-extent Vulkan smoke exposed a stale-control teardown race

- Physical `vkcube --wsi xcb` evidence advanced far enough to create and frame
  the fixed-extent surface, then the session exited with
  `X11 route targets unknown client 3`.
- The final records show a pointer focus request for the new surface followed
  by the fatal route error. The client worker had disconnected between the
  Engine focus decision and the bounded control-broker delivery.
- A disappearing application is normal frontend lifecycle, not an X authority
  failure. The broker now returns a distinct `ClientGone` control acknowledgement
  for that race, and the live owner retires that stale target without ending
  the graphical session. Backpressure and registry corruption remain fatal.
- The physical run also created a blank frame without a vkcube presentation.
  The dedicated proof launcher now enables redacted X11 and Present tracing so
  the next run can distinguish client-side exit, rejected Present validation,
  and feedback delivery without adding application-specific engine policy.

## 2026-07-26: declared constraints must fence blind-WM proposals before configure

- The traced fixed-extent run completed cleanly and proved that all Present
  rejection paths emitted Complete/Skip, Idle, and actual xshmfence signals.
  Vkcube registered three DMA-BUF images and six fences but stopped submitting
  after Sophia initially configured its fixed 500x500 surface as a 1276x1422
  tile and later recovered it.
- `WM_NORMAL_HINTS` is advisory and xmonad's default tiled layout does not
  enforce it. Treating an external WM proposal as authoritative client size
  therefore lets WM policy violate application constraints.
- `LayoutEpochCoordinator` now reconciles content geometry and configure sizes
  against Engine-owned declared constraints before control delivery. Placement
  remains WM-selected, but constrained extents are clamped and kept inside the
  output work area. Impossible constraints are rejected explicitly.
- This is protocol-neutral Engine policy: it applies to every WM bridge and
  fixed/min/max constrained surface, with no Vulkan, vkcube, or xmonad identity
  in the decision.

## 2026-07-27: truthful X map state is required before deferred admission

- A guarded xmonad run started Kitty and xmobar, but no managed surface became
  focused. Super-Enter queued another launch immediately before the startup
  watchdog exited at `stage=not_focused`.
- The isolated real-Kitty authority probe reproduced the failure without DRM,
  a WM, or a display manager. Sophia observed Kitty's created
  `PolicyManaged` windows and Present buffers, but no map intent.
- `GetWindowAttributes` had always reported every known window as viewable.
  Kitty trusted that reply and omitted `MapWindow`, so the new deferred
  admission protocol had no lifecycle edge from which to emit its request.
- X authority now derives the reply from its stored `XMapState`: created and
  policy-pending windows report unmapped, and only admitted/mapped windows
  report viewable. The real-Kitty probe has a deferred mode that requires one
  map intent, one delivered `AdmitSurface`, continuing Present feedback,
  delivered focus, and consumed routed text.
- The corrected probe passed end to end, and the full offline all-feature
  suite passed. Physical xmonad/vkcube verification remains open.
- The contemporaneous elogind diagnostic was unrelated. Session 193 was the
  valid online greetd `_greeter` session on tty7, not a stale Sophia session.

## 2026-07-27: recovery epochs must preserve admission ownership

- The first physical run after exact queued-Present ownership reached healthy
  KMS output and committed Kitty's recovery layout, but retired zero Kitty
  Presents and exited through the eight-second `not_focused` startup watchdog.
  This was a bounded session failure, not a renderer or kernel crash.
- The initial layout epoch timed out after the frontend had acknowledged
  admission, leaving the surface correctly in `AwaitingPixels`. The retry
  epoch staged and committed the retained pixels, but classified only a fresh
  `PolicyPending` surface as admission-owned. It therefore left both the
  transaction and its Present submission permanently in pre-admission
  quarantine.
- Surface-control staging now treats `ControlPending` and `AwaitingPixels` as
  continuing phases of the same admission. A retry may deliver the necessary
  configure, but its atomic commit also marks the surface managed and releases
  the retained transaction and Present exactly once.
- An all-feature regression reproduces an acknowledged admission entering a
  recovery transaction and requires the retry's finalization set, managed
  transition, transaction release, and Present release.

## 2026-07-28: standalone Vulkan isolated unresolved software Present

- The first standalone `vkcube --wsi xcb` run passed natural-layout admission,
  configure, and focus, then timed out at `no_visual_detail`. Vulkan selected
  llvmpipe. Sophia selected a 500-by-500 `XPixmap` candidate while recording
  zero DMA-BUF registrations and zero Present submissions, so no renderable
  storage could reach composition or KMS.
- A raw X pixmap is not a renderer buffer. The X authority now materializes
  regular software pixmaps into immutable CPU snapshots at Present time. It
  also retains MIT-SHM pixmap bindings and snapshots client-owned shared pixels
  at the same transactional boundary; DRI3 remains the zero-copy path.
- Unresolved pixmaps now fail closed instead of becoming false
  `PresentedBuffer` evidence. Complete-presentation semantics travel as a
  passive, protocol-neutral surface observation independent of CPU or DMA-BUF
  storage, preserving the admission reducer's distinction between a submitted
  frame and an accumulated backing image.
- The whole-pixmap SHM copy is the bounded correctness fallback. Damage-scoped
  persistent mappings remain an explicit post-proof optimization.

## 2026-07-28: software Present feedback must follow composed KMS retirement

- The first materialized software-Present run made the llvmpipe cube visible,
  but it remained on its first frame. The log contained one authority visual
  transaction and a retired nonzero CPU scanout, while Present Complete, Idle,
  and idle-fence-trigger counts all remained zero. The client was correctly
  waiting for permission to reuse its software pixmap.
- Presentation lifetime is now independent of storage kind. A software
  Present carries only its transaction, surface, and optional acquire/idle
  fence handles across the passive authority and production records. It owns
  no fabricated DMA-BUF handle.
- The production runtime registers that lifetime beside the CPU transaction,
  marks it submitted only when the composed primary frame reaches native KMS,
  and emits Complete followed by Idle after the matching page-flip retirement.
  Headless composition settles at its deterministic submission boundary.
- Focused regressions require source-free presentation retirement, actual idle
  fence signaling, authority-to-production observation preservation, and
  Complete-before-Idle routing. The physical verifier now rejects a visible
  but static software frame by requiring at least three authority frames and
  positive Complete, Idle, and idle-fence evidence.

## 2026-07-28: admission release must replace every storage form exactly once

- The first physical run with software feedback failed immediately after
  committing the initial CPU snapshot with `DuplicatePresentation`. The same
  transaction reached production twice: admission projection removed the
  quarantined surface transaction and DMA-BUF Present from the original
  observation, but omitted the equivalent software-Present record. The
  same-iteration admission release then appended its retained copy.
- Projection now applies one quarantine predicate to CPU transactions,
  DMA-BUF Presents, and software Presents. The released admission group is the
  sole owner of the reprojected transaction and presentation lifetime.
- Admission and production validation also reject duplicate software Presents
  for one transaction/surface before renderer resource registration. Focused
  regressions reproduce same-iteration replacement and the defensive failure
  boundary.
- The corrected physical standalone run passed its exact verifier. The
  500-by-500 llvmpipe cube animated through 487 software-Present transactions
  in 17,755 ms; all 487 produced native retirements, Complete, Idle, and
  idle-fence triggers. Native submission and retirement failures remained
  zero, all presentation resources drained, X protocol errors remained zero,
  and the session completed normal cleanup.

## 2026-07-29: retained repaints must reuse renderer image generations

- The latest physical GLX run completed two mixed Presents and routed the
  predecessor Idle correctly. A focus-border repaint then recreated the current
  DMA-BUF's EGLImage and GL texture. The draw blocked for 10.4 seconds in the
  frame completion barrier before AMD reported a guilty-context hard recovery;
  Sophia aborted with status 134 and the GLX client lost its X connection.
- The static gears were therefore not a GLX bootstrap, input, KMS callback, or
  TTY-recovery failure. Only the first two frames retired before a generic
  compositor-only repaint repeated a live import.
- Mixed layers now carry opaque renderer-image generations. A bounded
  renderer-private slot table imports one EGLImage/texture per generation,
  validates the complete DMA-BUF identity on hits, and reuses the texture for
  focus, chrome, workspace, and retained-damage repaint.
- Replacement retirement evicts the predecessor while its native context is
  current before triggering the idle fence. Context recreation and normal
  shutdown clear residency before presentation leases are released.
- Mixed and CPU presentation now reports X Present `Copy`; `Flip` is reserved
  for future direct scanout. Reduced metrics and the GLX reporter require cache
  hits, reject mismatch/capacity debt, and preserve post-KMS cadence evidence.
