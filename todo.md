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
blocker is native presentation lifetime and latency. A stable scene currently
recreates GBM/EGL targets and pipelines far too often, with observed page-flip
and input dwell spikes. Installed-session promotion cannot begin until stable
output epochs reuse rendering resources and the complete physical workflow is
repeatable.

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
- [ ] Retain per-output rendering resources across a stable
  size/format/modifier epoch; recreate them only for topology change,
  incompatible target change, or explicit bounded recovery.
- [x] Emit reduced recreation-reason and lifetime evidence without native
  handles or application metadata.
- [x] Remove blocking X11 configure/focus/close acknowledgements from the
  render/input owner; correlate them through a bounded typed control ledger
  while preserving global shortcuts and cursor motion during focus handoff.
- [ ] Prove a stable physical workload has zero per-frame target or pipeline
  recreation, zero launch-admission timeouts, and bounded input-to-submit and
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
