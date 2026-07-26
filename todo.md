# Sophia Active Roadmap

Sophia is a research prototype moving toward a usable native-X daily driver.
This file contains only active work and the next promotion gates. Completed
milestones belong in `docs/roadmap-history.md`; detailed evidence and diagnosis
belong in `docs/research-log.md`.

Roadmap rules:

- Keep exit criteria measurable and fail closed.
- Expand X11 behavior only from retained real-client evidence.
- Do not substitute QEMU evidence for a physical DRM, input, VT, or
  display-manager requirement.
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

This is still development evidence: the captured lifecycle reports
`installed=false`, `build=true`, and `manual_service=true`. The immediate
blocker is completing the strict physical xmonad workflow. Physical evidence
showed that neither a reused GBM surface nor a reused EGL/GL context bound to
fresh surfaces is safe on the current AMDGPU stack: both fail on the third
mixed render. Mixed composition therefore keeps one complete target per export
as the correctness baseline. Retirement-driven target pooling remains a
post-soak optimization unless measured latency prevents promotion. The focused
xmobar promotion capture now establishes the protocol-neutral work-area path
on both outputs: the 14-pixel top reservation produced exact reduced managed
geometry, button and axis input reached the client-positioned bar without
stealing Kitty focus, workspace and repeated VT round-trips preserved the
scene, 50 mixed composition lifetimes balanced with zero replacement, and
teardown was clean. Held-repeat, clipboard, post-last-window pointer, and the
broader interaction sequence remain in the normal xmonad gate.

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

## Milestone 9: Physical xmonad Session Promotion

This is the active milestone. The Kitty-only physical baseline and the
underlying xmonad protocol/resize corrections are established and archived.
Promotion now follows the gates below in order.

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
- [ ] Retain focused rollback, resize, output-change, and recovery regressions
  proving resources retire exactly once.
- [x] Replace the transient aggregate async-service booleans with one
  Engine-owned, output-scoped frame-service reducer. Backend-live must execute
  only named native effects, reobserve after each effect, and remain bounded
  when callbacks or cleanup do not advance. The reducer now validates one
  stable primary and bounded unique output set, orders per-output retirement
  before presentation, reserves the primary queued-Present path without
  starving secondary pending frames, and issues each effect at most once per
  service pass.
- [ ] Re-run the xmobar, four-Kitty, and normal xmonad gates from the same
  frame-lifecycle commit with no latency, ordering, or resource regression.
  Xmobar and four-Kitty now pass; the complete normal xmonad gate remains.

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
- [ ] Capture the documented standard run from TTY3 with launcher, guard,
  recovery, WM, frontend, renderer, and lifecycle evidence.
- [ ] Require focused Kitty within eight seconds, `outputs_ready=2/2`, nonzero
  mixed composition, and correct retained content on both outputs.
- [ ] Prove typing, pointer motion, click-drag selection, focus changes,
  Super-Enter, tiling resize, workspace switching, and no input delivery to
  hidden or unfocused windows.
- [x] Add Engine-owned held-key repeat scheduling with XKB-derived per-key
  repeatability and explicit X-frontend repeat delivery. Repeats bypass global
  shortcut evaluation and cancel on physical release, focus handoff, surface
  removal, workspace hiding, and seat suspension.
- [x] Physically prove held editing/navigation keys repeat while Super and
  modifiers do not. The first capture routed and acknowledged 66 repeat pulses
  with zero coalescing or capacity exhaustion, drained all 1,289 input
  deliveries, and ended with no active repeat seat or pressed-key debt.
- [ ] Prove Kitty `CLIPBOARD` ownership and same-namespace UTF-8 copy/paste.
  Require copy/paste between independent Kitty clients before and after
  workspace switches, at least one owner change and conversion, no GLFW
  ownership or conversion failure, and visual confirmation that the text is
  unchanged. Deterministic same-namespace and portal regressions now cover
  subscriber-routed `PropertyNotify`, distinct target/property atoms, Xlib's
  complete-property request shape (`AnyPropertyType`, `delete=true`, maximum
  `long_length`), and deletion only after a complete read. Local namespace
  ownership is preferred; cross-namespace source capture and execution
  revalidate the exact namespace and generation through the portal boundary.
  Physical confirmation remains required.
- [ ] Prove pointer confinement across the complete output union: hard edge
  motion must keep the hardware cursor visible, and reversing direction must
  move it immediately without first consuming discarded overshoot.
- [ ] Prove an unmodified primary-button press on an unfocused visible window
  commits WM-selected focus before client delivery; require the following
  keyboard input and ordered button release to reach that target, while hidden
  surfaces remain unselectable. WM API v4, the blind xmonad focus bridge, and
  the bounded Engine press/motion/release handoff now have deterministic
  coverage; physical confirmation remains.
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
- [ ] Pass that normal four-Kitty workflow for three consecutive clean cycles.
  One work-area-aware frame-service capture passes.
- [ ] Capture twenty rapid Super-Enter presses as a separate nonfatal
  capacity-overflow proof with bounded rejection and no session failure.
- [x] Require the session-control ledger to drain with balanced
  enqueue/dispatch/delivery counts, zero rejection/timeout/unexpected
  acknowledgements, and queue/ack latency at or below 100 ms. The three-
  workspace cycle drained 22/22 controls with 17 ms queue dwell and 14 ms
  acknowledgement latency.
- [ ] Prove three tiled Kitty windows remain usable before and after a
  TTY2/TTY3 round-trip, with keyboard and pointer restored.
- [ ] Validate full pc105 US shifted punctuation and libseat-backed
  Ctrl-Alt-F1 through Ctrl-Alt-F12 suspend/resume while `K_OFF` is active.
- [ ] After physical geometry stability, add a minimal Engine-owned focused
  surface border through the renderer-neutral compositor display list. Derive
  its bounds and damage from the same committed surface/focus snapshot, keep
  the blind WM and X frontend unaware of chrome, and verify it across focus,
  resize, workspace, VT, and mixed CPU/DMA-BUF presentation.
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
- [ ] Physically prove that xmobar renders above managed Kitty windows,
  continues updating, accepts pointer events, and does not steal keyboard
  focus across launch, resize, workspace, and VT round-trips.
  The first physical run exposed a mixed-scene ordering bug: the flattened CPU
  frame sat below Kitty's DMA-BUF. The next run rendered the bar and exposed a
  second generic boundary: the native GL path rejected CPU layers smaller than
  the output, so all Kitty Presents failed export. Sophia now interleaves
  CPU/GPU surfaces by Engine order and supports persistent, layer-sized CPU
  textures. The next run rendered the bar with Kitty, completed 141 mixed
  exports with zero native failures, and exposed the remaining overlap:
  managed Kitty geometry still begins at `y=0` beneath the bar.
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

Milestone 9 exits only when the automated resize regression and both physical
normal/emergency captures pass from the same committed release.

---

## Milestone 10: Physical Firefox Workload

QEMU already proves the bounded Firefox protocol workflow. This milestone
tests the combined physical AMD/KMS, xmonad, Kitty, and Firefox session rather
than adding speculative browser compatibility.

- [ ] Run the deterministic local Firefox workload beside two independently
  usable Kitty windows.
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

---

## Milestone 11: Installed Daily-Driver Candidate

- [ ] Promote one immutable, versioned release and greetd entry that uses no
  source build, repository-relative binary, temporary path, manual `sudo`,
  process kill, or service repair during ordinary login.
- [ ] Preserve the independent emergency path, a known-good fallback session,
  and a documented rollback procedure.
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
- [ ] Add a renderer-private, generation-keyed import cache whose capacity
  derives from the live-registration bound and whose entries evict only with
  zero frame and scanout leases.
- [ ] Keep one latest pending frame and one KMS submission in flight per
  output; prove input remains within half a refresh period at p99.
- [ ] Move GL execution to one renderer worker per GPU only if pooled physical
  evidence still exceeds that input budget. Use immutable bounded commands and
  explicit retirement tokens.
- [ ] Add atomic-test-gated direct scanout for one compatible opaque DMA-BUF
  layer, followed by a hardware cursor plane; retain mixed composition as the
  fail-closed fallback.
- [ ] Compare identical Kitty, Firefox, resize, launch-burst, and soak
  workloads against separate XLibre+xmonad and mature Wayland-compositor
  sessions on the same hardware. Comparative results are diagnostic; Sophia's
  absolute correctness and latency gates remain authoritative.

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
