# Sophia Active Roadmap

Sophia is a research prototype moving toward a usable native-X daily driver.
This file contains only active work and the next promotion gates. Completed
milestones belong in `docs/roadmap-history.md`; detailed evidence and diagnosis
belong in `docs/research-log.md`.

Roadmap rules:

- Keep exit criteria measurable and fail closed.
- Expand X11 behavior only from retained real-client evidence.
- Use QEMU for repeatable policy, protocol, transaction, and application
  semantics. Do not substitute it for physical DRM, input-device, VT,
  display-manager, or visible-pixel requirements.
- Keep Engine protocol-neutral and free of application-specific policy.
- Archive a milestone when its complete exit gate passes.

---

## Current Position

Sophia's product path is its native **Sophia X Server Frontend**. Engine owns
physical input, focus authority, scene state, rendering, presentation, and
scanout. X11 is the sole supported application protocol; the retired Wayland
and XLibre prototypes remain under `research/` as architectural evidence.

The development TTY profile now establishes:

- Guarded two-output startup with automatic Kitty, keyboard, pointer, focus,
  atomic xmonad resize, presentation, and clean teardown.
- Bounded Super-Enter admission and four concurrently presented Kitty clients
  without flashing in the latest focused physical captures.
- Engine-owned KMS presentation, protocol-neutral cursor and input policy, a
  blind WM API, and a supervised xmonad bridge.
- Native-X Kitty and Firefox protocol coverage plus unattended two-output QEMU
  mix and soak evidence.

This remains development evidence: the captured lifecycle reports
`installed=false`, `build=true`, and `manual_service=true`. Milestone 9's
commit-pinned promotion ledger passed all five gates on commit `727c716d`:
unattended QEMU semantics, native chrome, four-Kitty hardware smoke, xmobar
hardware smoke, and independent emergency recovery. The current renderer uses
one persistent composition target and frame surface with measured reuse,
balanced import-cache teardown, and a drained renderer-worker ledger. The next
product milestone is the combined physical Firefox workload; installed-login,
repeat-cycle, and workday-soak requirements remain later gates.

## Daily-Driver Promotion Contract

Sophia becomes a first physical daily-driver candidate only when one installed
xmonad session proves all of the following:

1. Normal login, automatic Kitty startup, and normal logout through greetd.
2. Keyboard, pointer, focus, workspaces, shortcuts, resizing, and both outputs.
3. At least two Kitty windows plus Firefox remaining independently usable.
4. Small text `CLIPBOARD` and `PRIMARY`, dialog handling, and application close.
5. Clean application, WM, frontend, renderer, KMS, input, and VT teardown.
6. Independent emergency recovery from a separate destructive-path run.
7. Repeated startup/logout cycles and an interactive soak with zero unexpected
   protocol errors, stuck input, rejected callbacks, or cleanup debt.
8. Installed release artifacts: no source build, manual service repair, or
   ad-hoc process cleanup during ordinary login.

---

## Post-Milestone 9 retained session follow-ups

Milestone 9 completed on commit `727c716d` on 2026-08-01. Its five immutable
promotion records all pass. The unchecked items retained below are explicitly
post-promotion compatibility, optimization, and stress follow-ups; they do not
reopen the completed Milestone 9 exit gate.

### 9.1 Native presentation lifetime and latency

- [x] Decouple exported scanout-buffer ownership from the persistent EGL
  context, GL pipeline, and GBM target lifetime.
- [x] Falsify retained-context composition on the physical AMDGPU path: two
  exports presented, then the third command stream was rejected even though
  every export used a distinct GBM/EGL surface.
- [x] Restore one complete context/pipeline/surface target per successful mixed
  export as the fail-safe baseline.
- [x] Confirm the restored lifetime in one physical cycle: 253 mixed exports,
  matching target/pipeline/surface creation and retirement, zero recovery,
  zero AMDGPU rejection, and a normal exit.
- [x] Confirm that layer-sized CPU overlays retain the fail-safe lifetime:
  the first bar-plus-three-Kitty run completed 141 mixed exports with matching
  target, pipeline, and frame-surface creation, zero native failures, and clean
  teardown.
- [x] Pass three physical four-Kitty cycles with complete-target creation and
  retirement equal to the mixed-export count. The third clean cycle exercised
  four-window layouts on two workspaces and completed 524 balanced mixed
  target, pipeline, and frame-surface lifetimes after the output-scoped
  frame-service change.
- [x] Prevent child-exit and resize-epoch work from starving input and native
  callback service. The first post-worker cycle reduced input dwell to 12 ms
  and submit-to-page-flip observation to 23 ms.
- [x] Poll native retirement before routing shortcuts that can synchronously
  enter the external WM transport.
- [x] Emit bounded owner-phase evidence for child reaping, physical-input
  routing, and external-WM request latency.
- [x] Attribute the remaining stall: the physical cycle recorded 180 ms in the
  synchronous WM request, 246 ms input dwell, and 210 ms submit-to-page-flip
  observation while child reaping remained at 25 ms.
- [x] Move the blocking WM socket round trip to one bounded typed worker while
  retaining validation, stale-response rejection, policy state, and atomic
  commit effects on the physical owner.
- [x] Re-run the two-output four-Kitty gate and require owner input below
  100 ms, a drained WM transport ledger, and clean page-flip/resource teardown.
- [x] Serialize cursor-plane updates against primary page flips: defer while a
  primary commit is in flight, complete admitted cursor-only commits before
  returning to the owner, and report cursor update latency and deferral.
- [x] Re-run the four-Kitty gate after cursor/primary serialization. The
  workspace-stress cycle recorded zero native submit failures and a 12 ms
  maximum cursor update; it then exposed an independent workspace-visibility
  defect.
- [x] Emit reduced recreation-reason and lifetime evidence without native
  handles or application metadata.
- [x] Remove blocking X11 configure/focus/close acknowledgements from the
  render/input owner; correlate them through a bounded typed control ledger
  while preserving global shortcuts and cursor motion during focus handoff.
- [x] Track client-delivered key presses and require acknowledged synthetic
  releases before focus, close, VT, seat-release, and logout handoffs so
  suppressed physical releases cannot leave the seat XKB state stuck.
- [x] Apply state-only XKB releases when a client destroys its surface before
  the physical key release, without delivering an orphan event to the next
  focused application.
- [x] Emit immutable per-output synchronous-modeset evidence at the aggregate
  startup-readiness transition rather than before native initialization.
- [x] Use the low-latency owner wait budget whenever physical input is active,
  so an idle X channel cannot add 25 ms to queued input before composition.
- [x] Prove a stable physical workload has balanced context, pipeline, and
  frame-surface creation/retirement with zero live-resource growth, zero
  launch-admission timeouts, and bounded input-to-submit and presentation
  latency. The 77-second two-workspace run balanced 524 complete composition
  lifetimes, drained every native and admission ledger, held input dwell to
  11 ms and submit-to-page-flip observation to 47 ms, and exited cleanly.
- [x] Retain focused rollback, resize, output-change, and recovery regressions
  proving resources retire exactly once. The named native-presentation
  lifecycle gate covers compensating resize rollback and late-pixel fencing,
  output/target size replacement and stale-allocation removal, stale prepared
  page-flip settlement, displayed-buffer replacement, cleanup retry, and
  duplicate-retirement rejection. Each case remains an isolated reducer or
  fake-device test; no native handles or application policy enter Engine.
- [x] Move non-cooperative resize/admission recovery into a protocol-neutral
  Engine layout-epoch coordinator. Preserve safe pixels, fence abandoned
  extents, publish temporary exact-size constraints for one bounded blind-WM
  replan, and translate those constraints through generic ICCCM size hints in
  the legacy bridge without application identity.
- [x] Reconcile blind-WM proposals against Engine-owned declared content
  constraints before client configure delivery. Preserve WM placement, clamp
  min/max extents inside the work area, and reject impossible constraints so a
  WM that treats ICCCM hints as advisory cannot destabilize an application
  swapchain before recovery begins.
- [x] Add protocol-neutral pre-pixel presentation admission. Keep a
  policy-managed X window unmapped after `MapWindow`, let Engine and the blind
  WM plan from passive geometry/constraint facts, then atomically configure and
  map it through a transaction-keyed `AdmitSurface` control. Require matching
  concrete pixels before the surface becomes committed visual state. Preserve
  the X parent tree so `MapSubwindows` maps only direct client-positioned
  descendants and cannot consume a later root-child admission; quarantine
  pre-map drawing outside renderer intake, reduce descendant software drawing
  into the toplevel presentation surface, and release it with any bounded
  queued Present submissions exactly once at the accepted geometry.
- [x] Report real X window lifecycle through `GetWindowAttributes`: created
  and policy-pending windows are unmapped, admitted windows are viewable.
  Retain a real-Kitty deferred-map probe that requires a presentation intent,
  delivered admission, continued Present feedback, focus, and routed text.
- [x] Replace the session-wide layout/Present barrier with per-submission
  dispositions. Stage only submissions owned by the pending epoch, reject known
  wrong-size buffers without contaminating visible layers, and keep unrelated
  surfaces eligible. Retain bounded queue storage with one shared immutable
  authority batch.
- [x] Separate queued Present ownership from persistent scene projection.
  Each queued submission now retains its exact surface transaction; Engine
  preparation overlays only that candidate on committed visual state, while
  unrelated CPU/GPU surfaces preserve their generations and transaction
  identity. Input projection is rebuilt from committed state after synchronous
  commit or page-flip retirement. The mixed-scene regression reproduces xmobar
  transaction 198 beside Kitty Present 403 and rejects malformed candidate
  pairings without terminating the session.
- [x] Preserve transaction identity across pre-pixel admission release.
  Production intake now carries ordered atomic groups inside one bounded
  resource envelope; quarantine retains complete homogeneous groups and
  releases them once without relabelling them as the current frontend batch.
  The retained regression reproduces ordinary transaction 367 beside released
  vkcube transaction 858 and commits both independently. A routed
  `x-authority-vkcube-admission-smoke` now requires deferred map intent,
  delivered generic admission, continued DRI3 imports, and two exact
  Complete/Idle Present round trips.
- [x] Gate DMA-BUF admission on exact page-flip retirement rather than X's
  mutable mapped snapshot. Require one-to-one surface/transaction/buffer
  pairing between the quarantined transaction and Present, defer focus until
  that candidate retires, keep referenced DMA-BUF/fence releases behind
  renderer ownership, and prevent retained CPU projection from overtaking a
  Present-bearing visual candidate. Retain reduced `armed`/`presented`
  diagnostics and an exact physical-log verifier.
- [x] Close the fixed-extent recovery escape around visual admission. Include
  every policy/control/pixel-pending layer in admission finalization even when
  the blind WM did not change its size, and require an exact newly staged
  transaction rather than accepting retained committed-size history. Keep
  bufferless geometry, chrome, and focus outside visible projection; bounded
  timeout withdraws a client that never supplies matching pixels.
- [x] Replace scalar safe-size overwrite and reverse admission lookup with an
  evidence-ranked visual candidate. Retain source transaction, extent,
  evidence class, and Engine observation sequence; prefer complete Present
  buffers over accumulated backing snapshots during admission in either
  arrival order. Select only that exact transaction, settle older covered
  groups, fence newer groups until retirement, and require candidate evidence
  in the physical verifier. The regression reproduces a 500-by-500 Present
  followed by the 1276-by-1422 blank backing snapshot from the failed run.
- [x] Add a visible single-client Vulkan isolation profile before changing the
  combined desktop again. Launch default `vkcube --wsi xcb` directly, omit
  Kitty/xmonad/xmobar and the compatibility bridge, and use a generic
  metadata-blind natural-size reference policy while retaining deferred
  admission, Present, rendering, and native KMS. Require an exact offline
  verifier for PresentedBuffer selection, retirement, nonzero pixels, and
  resource-clean normal teardown.
- [x] Re-run `tools/start_sophia_vkcube_standalone_tty3.sh` physically and
  retain `tools/verify_sophia_standalone_vkcube.sh` evidence. The first run
  isolated a blank frame below the WM bridge; after pixel materialization, the
  second run exposed a visible but static first frame because software Present
  never received Complete/Idle feedback. The first feedback-enabled rerun then
  exposed duplicate same-iteration admission projection; the original software
  observation and retained admission release both reached production. X
  authority now snapshots regular and MIT-SHM pixmaps at Present time, emits no
  unresolved pixmap as visual success, projects every storage form through one
  exact admission owner, and carries source-free Present lifetime through
  composed KMS retirement.
  The verifier requires at least three software frames, nonzero CPU composition
  and native export, Complete-before-Idle delivery, idle-fence triggers, and
  clean teardown.
  The retained physical result animated 487 software frames in 17,755 ms with
  487 native retirements, Complete/Idle pairs, and idle-fence triggers; it had
  zero native failures, protocol errors, or live presentation resources at
  clean teardown.
- [ ] Re-run the short physical xmonad/xmobar/Kitty startup proof. Require two
  committed runtime surfaces, a focused and interactive Kitty, successful
  Present retirement, zero mismatched-transaction warnings, and clean teardown
  before returning to the `vkcube` recovery proof.
- [ ] Physically launch default `vkcube --wsi xcb` from Kitty. Require the
  existing desktop to remain responsive, the cube to become visible through
  pre-pixel admission (using bounded fixed-extent recovery only if the client
  misses the accepted resize), and normal shutdown with no stale Present,
  control, input, or native-resource debt. Use
  `tools/start_sophia_xmonad_vkcube_recovery_tty3.sh` for the retained run.
- [ ] After the visible vkcube gate passes, model arbitrary post-admission X11
  Present, SHM, clear, and core-drawing operations as one bounded ordered
  logical-window content stream. Follow Xserver's copy/clip semantics without
  GPU readback, keep river-style configured/rendering state separate, and
  preserve one hot-path representation for future protocol authorities.
- [x] Optimize the proven software-Present fallback after correctness is
  retained. X authority now owns bounded reusable read-only SysV mappings,
  resolves XFixes valid/update regions with source clipping and fixed capacity,
  copies only requested SHM rows, preserves the presentation handle across
  same-size generations, and publishes one atomic immutable patch batch rather
  than cloning a whole pixmap per Present. Renderer registries validate every
  patch before mutation. CPU output storage is reference-counted and reused
  when its previous lease has retired; exact pixel scans are limited to three
  startup proofs, later frames use generation/damage evidence; same-stride GBM
  writes borrow the composed pixels; and mixed native composition retains its
  EGL/GBM target across frames. X resources and SysV identity remain authority
  private, and neither Engine nor WM policy gained X-specific state.
- [x] Make optimized admission progress and CPU-buffer residency independent
  of frontend timing. Service ready layouts after authority, policy, or control
  transitions; defer rather than overwrite an occupied WM-update slot. Retain
  renderer-private CPU buffers through a bounded staged-handle snapshot until
  the exact transaction commits or is withdrawn, without making pre-admission
  pixels scene-visible.
- [x] Run the bounded 900-frame physical performance proof with
  `tools/benchmark_sophia_vkcube_tty3.sh`. Retain the schema-2 report with
  positive patch traffic, bounded replacement pressure, Present FPS and p95
  cadence, CPU compose/upload maxima, clean retirement, and no visual
  regression. Capture a same-machine Xorg/XLibre reference with
  `tools/benchmark_xserver_graphics.sh`; require at least 90% of its rate and
  no more than 1/0.90 of its p95 interval before declaring the software
  fallback daily-driver performant.
  The paired tooling is complete: `tools/benchmark_xserver_graphics.sh`
  observes real X Present completions for the identical fixed workload and
  `tools/compare_sophia_xserver_rendering.sh` rejects provider, geometry,
  frame-count, mode, and output mismatches. Its optional `glxgears` phase is a
  separately labeled GLX compatibility/cadence probe and cannot satisfy or
  fail the Vulkan parity threshold. Physical 900-frame Sophia and Xserver
  captures passed on the same llvmpipe provider and 2560-by-1440 output:
  Sophia measured 59.953 FPS and 17.155 ms p95; composited Xorg measured
  59.950 FPS and 16.686 ms p95. The resulting rate ratio was 1.0001 and the
  inverse-p95 ratio was 0.9727, both above the 0.90 gate. Sophia retired 898
  observed frames with 6 ms maximum CPU composition, 3 ms maximum upload, and
  no native submission failure.
- [ ] After the cadence gate, obtain an unredirected Xorg/XLibre `Flip`
  reference if end-to-end presentation-latency parity is needed. A composited
  Xserver `Copy` result is valid client-cadence evidence but may complete
  before compositor scanout; retain the path label and do not present it as a
  scanout-latency comparison.
- [ ] Retain the paired bounded `glxgears` physical proof under Sophia. The
  generic standalone workload slot, bounded 500-by-500 swap-interval-one
  runner, and fail-closed schema-2 reporter are implemented in
  `tools/benchmark_sophia_glxgears_tty3.sh`. Require visible animation, direct
  GLX bootstrap, positive DRI3/mixed-composition and Present idle-fence
  progress, advancing post-KMS Flip cadence, positive DMA-BUF imports, zero
  descriptor mismatch/capacity rejection, clean retirement, and the same
  renderer provider under the reference Xserver. Cache hits are reported but
  are not required when every application frame carries a new buffer
  generation. Record client and
  presentation cadence separately and keep this a compatibility diagnostic,
  not a substitute for the fixed Vulkan acceptance workload.
- [x] Promote renderer-image residency from Milestone 13 into the GLX
  correctness path. Mixed DMA-BUF layers now carry opaque image generations;
  each native output context imports a generation once, validates descriptor
  identity on every hit, reuses it for compositor-only repaints, and evicts the
  predecessor before routing Idle. Capacity derives from the 256-presentation
  bound, context reset clears residency, shutdown clears imports before
  presentation teardown, and reduced metrics expose imports, hits, evictions,
  mismatches, capacity rejection, and live debt.
- [x] Correct retained DMA-BUF presentation semantics and isolate production
  GL execution from the session/input owner. Protocol-neutral feedback now
  distinguishes `Retained`, `Copied`, and `Skipped`; X maps a retained
  compositor lease to Present `Flip` and sends Idle only after its exact KMS
  retirement. The native hot path no longer calls `glFinish`. After the
  initial modeset, a bounded renderer worker owns EGL/GL/GBM, imported images,
  and locked front buffers while the owner retains input, cursor, VT, X
  polling, and KMS authority. Require the physical GLX rerun to show advancing
  animation, zero hard stalls, zero release-enqueue failures, and a clean
  worker request/completion drain. The first worker-enabled rerun proved the
  owner remained responsive but exposed stale CPU replacement of an in-flight
  mixed frame and ambiguous scanout transport. CPU fallback is now deferred
  across renderer/KMS ownership. The next run reached and retired a correctly
  labelled mixed KMS frame but exposed loss of the Present record during
  asynchronous worker deferral. The scheduler now owns one immutable
  `Rendering`/`Submitted` record from worker acceptance through page-flip
  retirement. The following
  run retired two advancing Flips, then exposed the same missing rendering
  slot in output content/damage plus an unsafe PRIME round trip inside the
  shared DRM file. Both output records now follow
  `Pending -> Rendering -> Submitted -> Presented`; shared-file scanout uses
  direct GEM descriptors, independent-file scanout requires PRIME FDs, and a
  500 ms page-flip watchdog terminates a lost-event session. The bounded
  six-second physical rerun visibly animated at 52.956 client FPS, completed
  242/242 renderer-worker requests without worker stalls or failures, drained
  scanout, and reported zero X protocol errors. It exposed a final proof-only
  ordering bug: the first stable nonzero KMS frame preceded X focus
  acknowledgement and its evidence was discarded. Startup now retains
  monotonic presentation evidence keyed by exact surface until focus pins the
  application. Two follow-up runs proved 240/240 and 242/242 worker completion,
  zero live import debt, visible animation up to 53.542 client FPS, and exposed
  the remaining false base-committed-surface membership gate. The consumer now
  treats exact stable GPU evidence independently of that optional CPU/base
  record, with a regression for the absent-record state. A fourth run animated
  at 52.981 client FPS and exposed duplicate native-retirement service paths:
  the authority-wait copy logged transaction 46 but omitted startup evidence.
  Both scheduling sites now use one shared retirement reducer. The clean
  lifecycle rerun reached readiness in 178 ms without recovery, animated at
  59.088 client FPS, completed 352/352 renderer requests and 351 exact
  Flip/Idle pairs, drained every import, and exited cleanly. Its reporter
  exposed a measurement-only dependency on disabled per-frame tracing.
  Cadence is now accumulated from routed UST values in bounded owner state and
  emitted once at completion. The final physical run passed schema 3: 161 ms
  startup, 59.197 client FPS, 352 retained-buffer samples, 59.953 presentation
  FPS, 17.324 ms p95, 353/353 worker completion, zero protocol errors, and
  zero live imports. The external watchdog process-group regression passes
  locally. Verbose per-stage tracing remains opt-in so benchmark cadence is
  not distorted by diagnostic I/O.
- [ ] If the measured software fallback remains outside that parity gate,
  replace per-frame direct CPU GBM allocation with an output-scoped,
  retirement-fed three-slot scanout pool. Slot state must be plain indexed data
  (`Available`, `Rendering`, `Queued`, `Displayed`), recycle only after KMS
  retirement, and retain bounded recovery. If upload bandwidth still dominates,
  route eligible DRI3 buffers directly through the existing DMA-BUF path rather
  than adding another application-specific fast path.
- [x] Replace the transient aggregate async-service booleans with one
  Engine-owned, output-scoped frame-service reducer. Backend-live must execute
  only named native effects, reobserve after each effect, and remain bounded
  when callbacks or cleanup do not advance. The reducer now validates one
  stable primary and bounded unique output set, orders per-output retirement
  before presentation, reserves the primary queued-Present path without
  starving secondary pending frames, and issues each effect at most once per
  service pass.
- [ ] Pass the same-commit unattended QEMU semantic gate, four-Kitty hardware
  smoke, and xmobar hardware smoke with no latency, ordering, or resource
  regression. Historical captures remain diagnostic evidence; the
  commit-pinned ledger is authoritative for the candidate.

### 9.2 Complete physical xmonad workflow

- [x] Make workspace visibility authoritative at every downstream boundary:
  preserve focus per workspace, restrict xmonad synthetic mappings and
  relayout nodes to the active workspace, and project only visible surfaces
  into composition and hit-testing.
- [x] Re-run the workspace stress sequence through Super-1, Super-2, and
  Super-3. The physical cycle committed seven workspace-1, nine workspace-2,
  and two workspace-3 actions, restored each workspace's focus, submitted a
  blank CPU frame for an empty projection, recorded no resize timeout, and
  drained native scanout cleanly.
- [ ] Capture the short hardware smoke from TTY3 with launcher, guard,
  recovery, WM, frontend, renderer, and lifecycle evidence.
- [ ] Require focused Kitty within eight seconds, `outputs_ready=2/2`, nonzero
  mixed composition, and correct retained content on both outputs.
- [ ] In the candidate ledger, prove physical typing, pointer focus,
  Super-Enter, one VT round-trip, four-window geometry, and normal logout.
  Prove layout switching, workspaces, clipboard/PRIMARY, hidden-surface input
  suppression, application close, and bridge recovery unattended in QEMU.
- [x] Physically prove plain-click focus and click-drag focus independently.
  The guarded TTY3 run routed both gestures to the selected Kitty, preserved
  cross-window copy/paste during the drag workflow, and rendered the
  Engine-owned focused border. The retained QEMU gate independently correlates
  request, Engine commit, X11 application, handoff release, and a following key
  for each gesture.
- [x] Keep external-WM empty workspaces focusless. Initial-focus reconciliation
  now exits before mutating Engine focus whenever an external WM owns policy.
  The QEMU gate switches to an empty workspace, observes both click edges
  suppressed with `reason=no_target`, and rejects a hidden focus request or
  client route before returning with Super-1.
- [x] Add Engine-owned held-key repeat scheduling with XKB-derived per-key
  repeatability and explicit X-frontend repeat delivery. Repeats bypass global
  shortcut evaluation and cancel on physical release, focus handoff, surface
  removal, workspace hiding, and seat suspension.
- [x] Physically prove held editing/navigation keys repeat while Super and
  modifiers do not. The first capture routed and acknowledged 66 repeat pulses
  with zero coalescing or capacity exhaustion, drained all 1,289 input
  deliveries, and ended with no active repeat seat or pressed-key debt.
- [x] Prove Kitty `CLIPBOARD` ownership and same-namespace UTF-8 copy/paste.
  Require copy/paste between independent Kitty clients before and after
  workspace switches, at least one owner change and conversion, no GLFW
  ownership or conversion failure, and visual confirmation that the text is
  unchanged. Deterministic same-namespace and portal regressions now cover
  subscriber-routed `PropertyNotify`, distinct target/property atoms, Xlib's
  complete-property request shape (`AnyPropertyType`, `delete=true`, maximum
  `long_length`), and deletion only after a complete read. Local namespace
  ownership is preferred; cross-namespace source capture and execution
  revalidate the exact namespace and generation through the portal boundary.
  The dedicated physical capture copied the exact token from a workspace-3
  Kitty and pasted it into the independent workspace-1 Kitty after switching
  workspaces. It recorded two owner changes and one conversion and completed
  with zero unexpected protocol, input, WM, native-presentation, or cleanup
  failure.
- [x] Prove pointer confinement across the complete output union: hard edge
  motion must keep the hardware cursor visible, and reversing direction must
  move it immediately without first consuming discarded overshoot. The full
  raw-position, startup-offset, edge-correction, and logical-position state
  now belongs to Engine rather than the CLI. The two-output xmonad QEMU gate
  drives the virtio mouse past the right edge and proves that the first reverse
  delta moves immediately through ordered, reduced Engine observations.
  A dedicated guarded TTY3 wrapper now asks for free crossing at the internal
  seam, one clamp/reversal at each far horizontal edge, and one at the top and
  bottom of each physical output. Reduced schema-8 evidence records boundary
  entry once, attributes it to a protocol-neutral output slot, distinguishes a
  free internal transition from projected gap crossing, and avoids logging
  every repeated edge contact. Its fail-closed verifier requires six ordered
  Engine clamp/reversal pairs, both free seam directions, visible
  hardware-cursor health, clean native/WM state, normal logout, and exact TTY
  restoration. The physical two-output run proved all six ordered edge
  clamp/reversal pairs and both free seam directions. It completed with 2,245
  hardware-cursor updates, zero hidden updates or hardware failures, zero WM
  restarts, clean Engine health, normal logout, and exact TTY restoration. A
  follow-up regression keeps each edge axis latched during perpendicular
  motion so one physical contact produces one reduced entry rather than a
  stream of duplicate observations.
- [x] Prove an unmodified primary-button press on an unfocused visible window
  commits WM-selected focus before client delivery; require the following
  keyboard input and ordered button release to reach that target, while hidden
  surfaces remain unselectable. WM API v4, the blind xmonad focus bridge, and
  the bounded Engine press/motion/release handoff now have deterministic
  coverage. A fail-closed physical evidence verifier requires request, Engine
  commit, frontend acknowledgment, retained input release, and a following key
  routed to the selected surface in order. A dedicated TTY3 wrapper now guides
  plain click and click-drag as separate focus transitions, returns after
  normal logout, and automatically verifies both sequences; physical
  confirmation remains. The unattended two-output xmonad gate now
  independently performs a plain click/key sequence and a click-drag/key
  sequence through virtio input. Each requires its own focus request, Engine
  commit, frontend acknowledgment, ordered handoff release, focused
  border/damage retirement, and following key on the selected surface. The
  same run also completes startup, ordered WM response rebasing, bridge
  restart, action-window launch/close, empty-workspace logout, and native drain
  with zero stale responses or protocol errors.
- [x] Prove the four-Kitty Tall layout has one full-height pane and three
  pixel-matched stack panes with no staging geometry, flashing, or corruption.
  With the active 14-pixel top reservation, the physical capture produced one
  `1280x1426` pane and exact `1280x475`, `1280x475`, and `1280x476` stack panes
  spanning `y=14..1440`.
- [x] Prove every ManageSurface resize commit and its first matching Present
  consume the same Engine layout snapshot. No new surface may present at the
  `(80,60)` admission staging offset after `layout_committed`, and the next
  focus-only transaction must report `moved_surfaces=0`. Deterministic Present
  admission, pending pixel/geometry authority, and sequential three-window
  real-xmonad regressions cover the boundary. The physical confirmation
  correlated seven action-launched surfaces across workspaces 1 and 2: every
  first post-commit Present used the exact work-area master geometry, every
  following transaction moved zero surfaces, and no staging target appeared.
- [ ] Pass the short physical four-Kitty hardware smoke once from the candidate
  commit. Keep multi-cycle repetition as an unattended soak and release-burn-in
  requirement rather than an operator promotion ritual.
- [ ] Add the rapid Super-Enter capacity-overflow workload to an unattended
  QEMU scenario. The passive queue-capacity tests remain authoritative until
  that scenario exists; the physical hardware smoke performs three ordered
  launches.
- [x] Require the session-control ledger to drain with balanced
  enqueue/dispatch/delivery counts, zero rejection/timeout/unexpected
  acknowledgements, and queue/ack latency at or below 100 ms. The three-
  workspace cycle drained 22/22 controls with 17 ms queue dwell and 14 ms
  acknowledgement latency.
- [ ] Prove four tiled Kitty windows remain usable before and after one
  TTY2/TTY3 round-trip, with keyboard, pointer, pixels, and bar restored.
- [ ] Retain the full pc105 US shifted-punctuation and Ctrl-Alt-F1 through
  Ctrl-Alt-F12 runner as a focused hardware diagnostic. Candidate promotion
  requires one real VT suspend/resume plus the complete deterministic XKB
  suite; repeat the exhaustive physical matrix only after input/seat changes
  or for release burn-in.
- [x] After physical geometry stability, add a minimal Engine-owned focused
  surface border through the renderer-neutral compositor display list. Derive
  its bounds and damage from the same committed surface/focus snapshot, keep
  the blind WM and X frontend unaware of chrome, and verify it across focus,
  resize, workspace, VT, and mixed CPU/DMA-BUF presentation.
  The bounded Engine display list, stable border-node identities, old/new
  damage, CPU reference lowering, and native solid-rectangle lowering are now
  implemented. The production CPU and mixed CPU/DMA-BUF paths interleave the
  four border primitives immediately after the focused surface and use the
  exact committed or prepared-candidate geometry associated with that frame.
  Those frames retain their immutable display-list identity through native
  queueing. A bounded per-output Engine ledger now compares pending chrome with
  the submitted or presented predecessor as appropriate, rejects failed and
  superseded state transitions, and advances the baseline only after the
  matching page-flip callback. The QEMU gate requires both output baselines and
  nonzero retired focus damage. Every CPU and mixed frame now carries one
  immutable Engine snapshot combining ordered client generations, geometry,
  buffer identity, compositor nodes, and optional software-cursor bounds.
  Initial or output-shape changes force full damage; the hardware cursor keeps
  its independent plane lifecycle. Engine clips combined damage to the output,
  coalesces exact rectangular unions, and selects bounded `skip`, `partial`, or
  fail-safe `full` repaint plans under generic rectangle-count and coverage
  policy. The QEMU focus proof correlates compositor damage, combined output
  damage, and the matching safe repaint decision from one retired frame.
  A reduced chrome-set observation now counts eligible surfaces, focused and
  unfocused frames, rings, primitives, and clearance without exposing client
  identity. Destination-buffer age/history, partial redraw, and KMS damage-clip
  consumption remain later performance work.
  The two-output QEMU xmonad gate proves that a click-drag focus commit is
  followed by a rendered four-primitive border on the same opaque surface
  before the next key reaches it, and observes borders on two distinct focus
  targets. Physical focus, resize, workspace, VT, and mixed-presentation
  confirmation remains required before closing this item. The fail-closed
  physical verifier is ready: it requires border/focus correlation on two
  surfaces, a focused geometry-generation change, empty-workspace hide/restore,
  a post-VT-resume recompose, nonzero mixed exports, and clean teardown.
- [ ] Close applications and request xmonad logout; require zero pending input,
  WM work, Presents, fences, scanouts, or cleanup debt, followed by correct TTY
  and greetd restoration.
- [ ] Run emergency recovery separately from the same commit and require
  bounded input flush, KMS cleanup, exact TTY restoration, and usable greetd.

### 9.3 Unmodified status-bar compatibility

- [x] Preserve core `CWOverrideRedirect` state through create/change,
  `GetWindowAttributes`, and lifecycle events.
- [x] Reduce override-redirect windows to the protocol-neutral
  `ClientPositioned` presentation role. Keep their client geometry in Engine
  composition and exclude them from the blind WM's managed-node stream.
- [x] Add optional unmodified xmobar discovery and supervised startup to the
  xmonad TTY launcher, with a deterministic Sophia-owned local config.
- [ ] Re-prove that xmobar renders above managed Kitty windows,
  continues updating, accepts pointer events, and does not steal keyboard
  focus during the short candidate hardware smoke. Workspace and lifecycle
  semantics remain in the unattended QEMU gate; the exhaustive focused
  xmobar runner remains available after work-area or seat changes.
  The first physical run exposed a mixed-scene ordering bug: the flattened CPU
  frame sat below Kitty's DMA-BUF. The next run rendered the bar and exposed a
  second generic boundary: the native GL path rejected CPU layers smaller than
  the output, so all Kitty Presents failed export. Sophia now interleaves
  CPU/GPU surfaces by Engine order and supports persistent, layer-sized CPU
  textures. The next run rendered the bar with Kitty, completed 141 mixed
  exports with zero native failures, and exposed the remaining overlap:
  managed Kitty geometry still begins at `y=0` beneath the bar.
  A later click/drag focus run exposed a brief focused-border outline around
  the bar during workspace changes. The cause was generic external-WM focus
  reconciliation selecting the first committed hidden surface after xmonad
  cleared focus; the owner now rejects that candidate before Engine mutation.
  The follow-up physical run committed 10 empty focusless projections and 26
  focused restorations with zero focused-border compositions during any empty
  interval, zero pointer-policy suppressions, and clean teardown.
  The refreshed schema-2 verifier additionally requires exactly one decorated
  managed Kitty and proves the client-positioned bar is excluded from both
  frame and focus-ring composition.
- [x] Decode bounded `_NET_WM_STRUT_PARTIAL` and legacy `_NET_WM_STRUT` values
  entirely inside the X
  frontend and reduce them to protocol-neutral edge/span reservations tied to
  the presenting surface. Reject malformed type, format, length, range, and
  overflow without leaking atoms, XIDs, dock metadata, or client identity.
- [x] Derive an output work area from active mapped, client-positioned
  reservations and send
  only that rectangle to the blind WM. Keep client-positioned bar geometry in
  the full output scene, while managed Kitty geometry begins exactly at the
  reserved top edge and ends exactly at the output boundary.
- [x] Make reservation replacement, deletion, unmap, surface destruction,
  configured-output projection, workspace projection, and frontend teardown
  trigger one bounded work-area update while preserving workspace and focus
  state. Invalid aggregate reductions retain the last valid work area.
- [x] Cover no reservation, valid top reservation, partial-span reservation,
  malformed values, conflicting edges, two-output clipping, replacement, and
  removal with passive reducer, lifecycle, policy-state, and X11 socket tests.
- [ ] Physically prove those lifecycle paths have no stale gap, overlap,
  resize timeout, or focus change; dynamic output-topology change remains part
  of the later multi-output hotplug gate.
- [x] Retain a bounded real-xmobar request trace proving override-redirect
  lifecycle, MIT-SHM pixmap upload/readback, copy-to-window pixels, and no X
  protocol errors.
- [x] Retain a guarded physical xmobar regression from the same commit as the
  status-bar work-area implementation. Require an updating bar, Kitty at
  `y=bar_bottom` with `height=output_height-bar_bottom`, no visible seam or
  occluded Kitty pixels, working bar pointer interaction, unchanged Kitty
  keyboard focus, one workspace round-trip, one VT round-trip, and clean
  teardown.
  The guarded capture on frame-service commit `9b14ea9` applied `y=14` on both
  outputs, routed button and axis input to the generic client-positioned role,
  preserved Kitty focus across the interaction, committed workspace 2/1,
  completed three clean VT suspend/resume cycles, balanced 50 mixed
  composition target/pipeline/frame-surface lifetimes with zero replacement,
  and exited normally with clean TTY recovery.

### 9.4 Rendering workload coverage

The `vkcube` and `glxgears` proofs occupy the same workload cell: one
fixed-size, always-animating window emitting a fresh full-frame DMA-BUF every
vsync, measured for throughput/cadence. They prove the GPU DRI3 hot path twice
and leave the workloads a daily driver actually lives in unmeasured. These
items add the missing rendering-optimization proofs. Each keeps the existing
launcher/probe/reporter/fixture/check pattern, a fail-closed schema report, and
a same-hardware Xserver reference where an apples-to-apples one is definable.
They are rendering-coverage evidence; only a named daily-driver failure
promotes one to a hard M9 exit gate.

- [x] Terminal CPU-path throughput. Drive the SHM/software-Present
  path with a bounded standalone `xterm` scrollback workload rather than a GPU
  client. Require positive CPU patch traffic (`cpu_patch_updates>0`,
  `cpu_payload_bytes>0`) proving the immutable patch-batch path rather than
  whole-pixmap replacement, at least one `partial` `sophia_live_output_repaint`
  proving damage-driven repaint rather than a full frame every present, bounded
  `cpu_max_compose_msec`, zero unexpected X11 protocol errors, and clean
  teardown. Follow-up: define an apples-to-apples Xserver xterm-redraw cadence
  reference (Copy-based redraw has no clean per-frame flip) before claiming
  CPU-path parity.
  Status: the first physical run hard-locked the machine. Root cause was a
  probe bug, not the CPU path: `run_bounded_xterm.sh` passed the pixel intent
  into xterm's `-geometry` (character cells), so `500x500` requested a
  4004x5004 px window that overran the 64 MiB software-buffer cap, was rejected
  `BadWindow`, and aborted startup before any layout committed; the hard lock
  was the downstream greetd/RDNA3 KMS re-take after that abnormal early exit.
  Reproduced deterministically offline via `x-authority-xterm-input-smoke`
  (no KMS). Fixed by converting px→cells against a pinned `6x13` font and
  clamping under the cap. The schema-3 performance reporter now fail-closes
  above the established 25 ms CPU-composition budget and reports the applied
  budget beside the observed maximum. A commit-pinned TTY3 gate now refuses a
  dirty tree or inactive persistent logging and archives the report, session,
  guard, recovery, launcher, and exact kernel-log delta. Native path audited
  and cleared as a lock cause.
  The first post-geometry-fix run then failed closed at transaction 650 because
  the probe emitted unpaced 200-line bursts into the intentional 256-batch
  authority queue. The X frontend stopped rather than dropping visual facts;
  xterm exited 84, and TTY3/greetd restoration completed normally. The probe
  now declares a fixed eight-line/16 ms cadence in schema-3 evidence, and the
  runner preserves structured interrupted evidence. See
  `docs/research-log.md` 2026-07-30. Two paced physical attempts then proved
  clean rendering, input, native drain, kernel delta, and TTY handback, but
  produced no client completion: one was manually logged out before 20 seconds;
  the other exposed a probe-controller bug where xterm backpressure held
  `seq(1)` past the shell's wall-clock test and the outer safety timeout arrived
  before the final-only count write. The producer now has an independent timer,
  records completed bursts incrementally, and has a stalled-pty regression.
  Two controller-fixed physical runs on `4cb4f5f` then passed the schema-3
  automated gate: 6,648 lines / 831 iterations, positive patch traffic,
  partial repaint, zero authority drops or unexpected protocol/native
  failures, 7 ms maximum CPU composition against the 25 ms budget, clean
  kernel deltas, and clean TTY3/greetd handback. The retained runner records
  remain `visual-confirmed=false` because the local prompt did not capture
  `yes`; the operator separately observed the expected scrolling-number
  surface and responsive pointer in the paced session. Do not rewrite the
  immutable archive. The Sophia CPU-path evidence is complete; the Xserver
  comparison remains a follow-up before any parity claim.
- [x] Input-to-photon latency. Inject input through the physical libinput
  dwell/budget path and measure ingress to the exact presented frame that
  reflects it. Kernel `PageFlipEvent::duration` now survives the private native
  callback path into production retirement, with completion counters exposing
  kernel timestamps, synthetic fallbacks, and pending correlations. The raw
  ingress gap remains: `--inject-text` bypasses the libinput queue-dwell stage.
  Add a bounded uinput-backed injector and per-sample correlation from the
  libinput event timestamp through the exact submitted/presented frame. Require
  positive kernel timestamp coverage and zero fallbacks/pending correlations.
  For synchronized scanout, require full-chain latency below two refresh
  periods at p95 while independently requiring maximum queue dwell at or below
  1 ms, dwell-to-submit at or below one refresh, and submit-to-page-flip at
  or below one refresh. This keeps random vblank phase out of the controllable
  pre-submit budget without accepting an extra queued frame.
  Implemented: the uinput keyboard helper, per-event libinput timing sidecar,
  post-ingress submission correlation, kernel-UST retirement, stage reporting,
  and a commit-pinned 20-sample TTY3 runner. The unattended QEMU regression now
  drives QMP input through virtio evdev/libinput, correlates software-composed
  scanout to its kernel UST, and rejects fallback/pending clocks; it caught and
  fixed a no-WM visual-admission startup deadlock. Remaining: run the physical
  gate and retain a passing p95 archive before checking this item complete.
  The first damage-reuse physical rerun completed 16 clean samples with a
  2 ms maximum native upload, then stopped before injection on a transient
  cursor-plane `EACCES`; the runner now retries only that exact pre-input
  startup failure and still fails closed after input readiness or injection.
  The next complete archive retained the 2 ms upload maximum but measured
  22 ms p95 because the paced seven-character injector created intermediate
  input frames and made the final key wait behind an earlier page flip. The
  gate now emits the same exact text as one zero-spacing uinput burst so one
  sample measures one isolated libinput-to-presented-frame transaction. That
  isolated run proved zero queue dwell but exposed 11–14 ms of pre-submit work:
  every primary update was rebuilding and scanning an unchanged 1920x1080
  secondary-output marker. Secondary frames are now cached by output slot and
  descriptor, reused across primary recomposition, and invalidated when the
  output changes. The dual-output QEMU GBM/KMS path passes with 2 ms maximum
  composition; a same-commit physical p95 archive remains required.
  The next 20-sample physical archive on `df1f385b` retained exact input,
  kernel timestamps, clean teardown, and a 2 ms maximum native upload, but
  measured 25 ms p95 and 28 ms maximum. Queue dwell remained at 0–1 ms while
  dwell-to-submit remained 9–13 ms: caching the secondary output removed only
  part of the owner-path work because the primary CPU scene still cleared and
  recomposed the complete 3840x960 frame for each bounded xterm change. Primary
  display-list composition now derives a normalized repaint plan from the
  retained output snapshot, clears and replays only intersecting layers,
  borders, and cursor pixels in stacking order, and falls back to full
  composition whenever history or storage is incompatible. Focused pixel
  regressions and the complete offline gate pass. Commit-pinned QEMU on
  `a2c91f51` passed with exact input, kernel page-flip timestamps, clean
  teardown, 3 ms maximum CPU composition, and an 18 ms full chain split into
  0 ms queue dwell, 14 ms dwell-to-submit, and 4 ms submit-to-page-flip; QEMU
  remains correctness evidence rather than the physical performance gate. The
  authoritative physical rerun on `b6e94d5d` retained 20 exact samples with
  kernel timestamps, zero fallback/pending correlations, clean health and
  teardown, 26 ms p95, 27 ms maximum, 0 ms maximum queue dwell, 10 ms maximum
  dwell-to-submit, and 16 ms maximum submit-to-page-flip. The physical contract
  separates the synchronized two-refresh end-to-end bound from one-refresh
  processing and scanout-stage maxima. The immutable archive's report records
  the superseded half-refresh processing gate as failed; schema-2 re-evaluation
  under the corrected contract passes all four gates without modifying that
  evidence. Precise glyph-level damage propagation remains a Milestone 13
  efficiency improvement because safely reusing older frames requires
  accumulated buffer-age history across coalesced generations.
- [ ] Resize-under-render storm. Continuously relayout a rendering client using
  the existing `--inject-surface-resize` / `--inject-output-size` hooks. Require
  no admission staging offset after `layout_committed`, no wrong-size buffer
  reaching scanout, no resize timeout, and bounded recovery with resources
  retired exactly once.
- [ ] Multi-producer concurrent present. Present N DMA-BUF clients beside one
  CPU-composited bar and measure per-output frame-service fairness, import-cache
  pressure and eviction, and single renderer-worker request latency under
  contention.
- [ ] Idle / partial-damage efficiency. Hold a mostly-static desktop and require
  near-zero recomposition, a high import-cache hit rate, and no full-frame CPU
  upload when nothing changed. This is the inverse of the always-animating
  benchmarks and the only proof that exercises the import-cache hit path, since
  every animation frame is a cache miss by construction.
- [ ] Producer-overload / frame-drop discipline. Drive an unthrottled producer
  above the refresh rate and prove one latest pending frame plus one KMS
  submission in flight per output, bounded queue storage, no unbounded memory
  growth, and no tearing.

Tier 3 rendering coverage — atomic-test-gated direct-scanout bypass and
multi-output cadence parity — is not new work here; it extends the existing
Milestone 13 direct-scanout and multi-output-worker items.

Milestone 9 exits only when the commit-pinned unattended semantic, native
chrome, four-Kitty hardware, xmobar hardware, and emergency gates pass for one
candidate commit.

---

## Milestone 10: Physical Firefox Workload

QEMU already proves the bounded Firefox protocol workflow. This milestone
tests the combined physical AMD/KMS, xmonad, Kitty, and Firefox session rather
than adding speculative browser compatibility.

- [x] Complete generic Firefox wheel compatibility in the X frontend. Preserve
  protocol-neutral Engine axis routing, translate it through the appropriate
  X11 input semantics, and require a real routed axis event to produce the
  deterministic local page's DOM `wheel` stage. The clean QEMU gate negotiates
  Firefox XI2 2.1, advertises horizontal and vertical scroll valuators, routes
  cumulative smooth-axis positions through the selected ancestor window, and
  completes the DOM stage with `source=wheel`, `axis_route=true`, and
  `keyboard_fallback=false`.
- [ ] Run the deterministic local Firefox workload beside two independently
  usable Kitty windows. The first isolated-profile physical run proved a real
  Firefox DRI3 frame plus the deterministic loaded and keyboard stages, then
  exposed clipped-frame rejection and descendant-click focus ambiguity after
  xmonad resize. Pixel-aligned clipped Presents, same-client managed-toplevel
  focus handoff, reparent role withdrawal, and client-scoped observation
  overload handling allowed the next run to retire the browser frames, but the
  trace then proved that Firefox's render child was still projected at global
  origin and overlapped Kitty's input region. Descendant DMA-BUF Presents now
  reduce to their managed toplevel with an accumulated X-hierarchy offset and
  target-bounded clipping. The following physical run proved correct unit-scale
  browser placement plus keyboard and clipboard interaction, then exposed an
  edge-target instruction and an unsynchronized two-Kitty prompt race in the
  proof harness. Those prompts now coordinate exactly one restart and direct
  pointer work away from compositor edges; repeat the physical workflow before
  closing this item.
- [ ] Require visible rendering plus keyboard, pointer, scroll, resize,
  workspace hide/show, refocus, dialog open/close, and status-zero exit.
- [ ] Prove bounded UTF-8 text transfers through `CLIPBOARD` and `PRIMARY`
  between Firefox and Kitty.
- [ ] Close, restart, and force-close Firefox while both Kitty windows retain
  their content, focus, workspaces, and interactivity.
- [ ] Audit only the desktop services observed by this run—such as DBus,
  PipeWire, or portal helpers—and add session integration only for evidenced
  failures.
- [ ] Retain zero unexpected X11 errors, pending actions/input, native cleanup
  debt, unrelated-client failure, or protocol-specific state below Engine.
- [ ] Pass the complete workflow three consecutive times from one commit.

Milestone 10 exits with one strict physical workflow verifier and three
consecutive passing runs.

Compatibility follow-up outside the Firefox exit gate:

- [ ] Run the same deterministic wheel/keyboard/pointer fixture in Chromium as
  an independent native-X consumer after the Chromium package is installed.

---

## Milestone 11: Installed Daily-Driver Candidate

- [x] Establish two strict KDL 2 configuration domains: XDG
  `sophia/config.kdl` for session/Engine mechanism and `sophia/wm.kdl` only
  for a Sophia-native WM. External WMs retain their native configuration.
- [x] Add single-source discovery, safe file admission, compiled defaults,
  immutable SHA-256 snapshots, strict schema/cross-reference validation,
  public check/print commands, parent-directory inotify, last-known-good
  retention, and whole-file pending-restart semantics.
- [x] Apply the initial live-safe core slice at owner-loop boundaries:
  future application launches, idle key-repeat replacement, fallback chrome,
  and diagnostics. Engine-owned border rendering now consumes runtime style
  instead of a backend hard-code.
- [x] Split compositor chrome into explicit focus-ring and frame roles. Reserve
  stable Engine-owned clearance inside each WM allocation, derive client
  content once through a checked data-oriented geometry system, keep
  client-positioned surfaces ineligible, and lower semantic borders through
  one shared fixed-band path so wide chrome cannot cover application pixels.
- [x] Make native chrome ownership an explicit WM API capability. Keep the X11
  compatibility bridge chrome-blind, route external WMs through the core
  fallback, and hold candidate width changes behind the same committed
  relayout boundary used by native policy.
- [ ] Complete the physical schema-2 proof for ring-only, frame-only, and
  combined chrome. Require an atomic 2→6 width transition with no content
  occlusion, partial geometry, focus resize, bar decoration, or input loss.
  Reduced aggregate evidence and fail-closed fixtures now cover each policy,
  resize, and composition boundary; physical capture remains.
- [x] Version the blind WM API to v6 with negotiated focus-ring/frame chrome,
  generation-ordered policy-update/ack packets, stale rejection, and an
  Engine idle-shortcut application reducer.
- [x] Carry native-WM policy updates and acknowledgements through the
  supervised live transport without requiring an existing binding to trigger
  a request; update bindings, action policy, workspace policy, and active
  chrome as one idle-boundary transaction.
- [x] Add guarded physical hot-reload proof tooling: valid live edit, invalid
  edit
  retaining last-known-good state, restart-required edit with no partial
  apply, atomic replacement, deletion/recreation, and external-WM native
  config isolation. The native-WM runner is
  `tools/start_sophia_native_hot_reload_tty3.sh`; the external-xmonad core
  runner is `tools/start_sophia_xmonad_config_reload_tty3.sh`. Both have
  ordered sequence logs and negative-fixture verifiers. Their commit-pinned
  physical captures remain.
- [ ] Promote one immutable, versioned release and greetd entry that uses no
  source build, repository-relative binary, temporary path, manual `sudo`,
  process kill, or service repair during ordinary login.
- [ ] Preserve the independent emergency path, a known-good fallback session,
  and a documented rollback procedure. The repository launcher now supports
  an opt-in process-external wall-clock deadline, and the bounded GLX proof
  enables it automatically; the installed session still needs an
  installation-owned equivalent and physical recovery proof.
- [ ] Add startup diagnostics that identify the installed version and exact
  preflight, guard, takeover, session, or handoff failure without exposing
  application content.
- [ ] Validate three consecutive installed logins, normal logouts, one
  emergency recovery, and successful fallback-session login.
- [ ] Document supported hardware, required services, known limitations, and
  the operator commands for status, logs, stop, recovery, and rollback.

Milestone 11 exits when three normal logins and the independent recovery path
work without a repository checkout or development takeover script.

---

## Milestone 12: Stability And Workday Soak

- [ ] Pass ten consecutive installed login/startup/logout cycles without
  emergency recovery, stale graphical processes, or display-manager repair.
- [ ] Pass a two-hour interactive soak with repeated Kitty and Firefox
  launch/close, focus, workspace, resize, clipboard, and multi-output actions.
- [ ] Pass one full workday using the same committed build and installed
  session entry.
- [ ] Require zero unexpected protocol errors, allocator diagnostics, rejected
  page-flip callbacks, stuck keys/buttons, presentation starvation, in-flight
  ownership, cleanup debt, or failed TTY restoration.
- [ ] Record bounded latency and health summaries without logging typed content,
  clipboard payloads, window titles, or application metadata.
- [ ] Rotate retained logs and preserve the exact Sophia commit, binary digest,
  kernel, Mesa, Kitty, Firefox, xmonad, output, and input-seat identities.

Failures create the next smallest evidence-driven compatibility or lifecycle
slice. They do not justify broad X11 conformance work. Milestone 12 exits when
the installed path—not a repository launcher—passes the complete Daily-Driver
Promotion Contract.

---

## Milestone 13: Native Graphics Efficiency

This milestone starts only after the installed workday soak. It optimizes the
same native-X product; XLibre and Wayland remain external performance
references rather than Sophia runtime components.

- [ ] Recycle three generational frame-surface slots per output through
  explicit page-flip retirement, with bounded deferral when all slots are
  leased.
- [ ] Carry bounded buffer-age damage history per slot and repaint only
  accumulated damage; fall back to a full repaint whenever history is
  incomplete.
- [x] Add a renderer-private, generation-keyed import cache whose capacity
  derives from the live-registration bound and whose entries evict only with
  zero frame and scanout leases. The correctness subset was promoted into
  Milestone 9 for retained compositor repaints; direct-scanout lease accounting
  remains part of the later direct-scanout item.
- [ ] Keep one latest pending frame and one KMS submission in flight per
  output; prove input remains within half a refresh period at p99.
- [x] Move production GL execution off the session owner using immutable
  bounded commands and explicit retirement tokens. The initial physical
  implementation has one worker per native output and therefore one worker on
  the current single-output GPU.
- [ ] Coalesce all outputs in the same DRM/render-device group onto one shared
  renderer worker before multi-output promotion. Preserve one latest pending
  request per output, bounded response demultiplexing, and explicit per-output
  retirement tokens. Its multi-output cadence-parity proof is the 9.4 Tier 3
  workload.
- [ ] Add atomic-test-gated direct scanout for one compatible opaque DMA-BUF
  layer, followed by a hardware cursor plane; retain mixed composition as the
  fail-closed fallback. Its direct-scanout-bypass proof is the 9.4 Tier 3
  workload.
- [ ] Compare identical Kitty, Firefox, resize, launch-burst, and soak
  workloads against separate XLibre+xmonad and mature Wayland-compositor
  sessions on the same hardware. Comparative results are diagnostic; Sophia's
  absolute correctness and latency gates remain authoritative.
- [ ] Replace full immutable CPU presentation replacement for stable
  software-rendered X toplevels with lease-safe damage generations or
  copy-on-write backing. Preserve child-to-toplevel composition, bounded
  storage, historical-handle immutability, and exact admission extents while
  measuring xterm redraw latency and copied bytes.

Milestone 13 exits with bounded warmed resource counts, no steady-state
allocation growth, refresh-relative latency evidence, and no change to
Sophia's native-X authority model.

---

## Secondary Development Tooling

Interactive QEMU is useful for reproduction but is not a physical daily-driver
blocker. Work on it only when it shortens one of the active milestones.

- [ ] Replace the acceptance-oriented `xmonad-m8-soak` guest with a dedicated
  `xmonad-interactive` scenario: manual shutdown, no scheduled bridge restart,
  and no proof watchdog.
- [ ] Retain host and guest diagnostics that distinguish host grab delivery,
  guest device discovery, key/button/motion intake, Engine routing, and
  focus/display targeting without recording input content.
- [ ] Gate the supported interactive backend with visible pointer movement,
  terminal launch, typed text, focus change, application close, and clean
  manual shutdown.

## Evidence-Driven Compatibility Follow-ups

These are admitted only when a named daily-driver workflow fails:

- Large X11 `INCR` clipboard transfers.
- Full Xdnd and URI/file launching.
- Prompt UI, notification actions, and capture/FD handoff.
- Client-selected classic X11 cursor images.
- Additional toolkit, extension, font, color, or window-manager behavior.

Each follow-up begins with the first missing request, reply, event, state
transition, or lifecycle fact and ends with a focused wire regression plus the
real-client gate that exposed it.

## Deferred

- XLibre provider integration until measured native-X gaps justify its
  authority and maintenance cost.
- Any new application protocol or compatibility frontend without a
  specification amendment backed by named product evidence.
- VRR activation until physical hardware reports `vrr_capable=1`.
- General X11 conformance work not required by a retained daily-driver client.
