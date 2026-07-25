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
blocker is native presentation lifetime and latency. Physical evidence showed
that neither a reused GBM surface nor a reused EGL/GL context bound to fresh
surfaces is safe on the current AMDGPU stack: both fail on the third mixed
render. Mixed composition therefore uses one complete target per export.
Installed-session promotion requires preserving that lifetime rule while
moving to an explicit retirement-driven pool of complete target slots.

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
- [ ] Pass three physical four-Kitty cycles with complete-target creation and
  retirement equal to the mixed-export count. One post-worker cycle passes.
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
- [ ] Introduce a three-slot per-output generational pool of complete render
  targets; recycle a slot only through explicit page-flip retirement and defer
  instead of exceeding the bound.
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
- [ ] Prove a stable physical workload has bounded context and pipeline
  creation, zero launch-admission timeouts, and bounded input-to-submit and
  presentation latency.
- [ ] Retain focused rollback, resize, output-change, and recovery regressions
  proving resources retire exactly once.

### 9.2 Complete physical xmonad workflow

- [ ] Capture the documented standard run from TTY3 with launcher, guard,
  recovery, WM, frontend, renderer, and lifecycle evidence.
- [ ] Require focused Kitty within eight seconds, `outputs_ready=2/2`, nonzero
  mixed composition, and correct retained content on both outputs.
- [ ] Prove typing, pointer motion, click-drag selection, focus changes,
  Super-Enter, tiling resize, workspace switching, and no input delivery to
  hidden or unfocused windows.
- [ ] Prove an unmodified primary-button press on an unfocused visible window
  commits WM-selected focus before client delivery; require the following
  keyboard input and ordered button release to reach that target, while hidden
  surfaces remain unselectable.
- [ ] Prove the four-Kitty Tall layout has one full-height pane and three
  pixel-matched stack panes with no staging geometry, flashing, or corruption.
- [ ] Pass that normal four-Kitty workflow for three consecutive clean cycles.
- [ ] Capture twenty rapid Super-Enter presses as a separate nonfatal
  capacity-overflow proof with bounded rejection and no session failure.
- [ ] Require the session-control ledger to drain with balanced
  enqueue/dispatch/delivery counts, zero rejection/timeout/unexpected
  acknowledgements, and queue/ack latency at or below 100 ms.
- [ ] Prove three tiled Kitty windows remain usable before and after a
  TTY2/TTY3 round-trip, with keyboard and pointer restored.
- [ ] Validate full pc105 US shifted punctuation and libseat-backed
  Ctrl-Alt-F1 through Ctrl-Alt-F12 suspend/resume while `K_OFF` is active.
- [ ] Close applications and request xmonad logout; require zero pending input,
  WM work, Presents, fences, scanouts, or cleanup debt, followed by correct TTY
  and greetd restoration.
- [ ] Run emergency recovery separately from the same commit and require
  bounded input flush, KMS cleanup, exact TTY restoration, and usable greetd.

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
