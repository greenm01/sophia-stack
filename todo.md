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

The following foundations are established:

- Namespace admission, bounded portals, and text `CLIPBOARD` plus `PRIMARY`.
- Concurrent native-X clients, XKB and XI2 input, focus/grabs, RandR,
  software rendering, MIT-SHM, GLX, DRI3, Present, and mixed composition.
- Engine-owned multi-output KMS presentation and a protocol-neutral default
  hardware cursor.
- Blind WM API, supervised xmonad bridge, workspaces, named application
  actions, logout, and bridge recovery.
- Unattended xmonad/Firefox mixed and soak evidence in two-output QEMU.
- Guarded physical Kitty-only TTY3 startup, keyboard, pointer, presentation,
  clean exit, and TTY/display-manager recovery.

The missing product proof is one integrated physical xmonad desktop. Broader
protocol compatibility, interactive-QEMU polish, and speculative compositor
features do not precede that proof.

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

This is the active milestone. The Kitty-only physical gate is complete; xmonad
remains disabled until its real-Kitty resize and physical-session gates pass.

### 9.1 Real-Kitty xmonad resize regression

- [ ] Add a bounded automated session using the real Kitty binary, the generic
  WM bridge, and real xmonad.
- [ ] Require WM hello/binding handshake, automatic Kitty map, Engine focus,
  initial DRI3/Present frame, and zero client-visible protocol errors.
- [ ] Drive an xmonad layout resize and require the matching ConfigureNotify,
  configure acknowledgement, updated Engine surface bounds, and a later Kitty
  Present at the new size.
- [ ] Route exact shell input after the resize and require both the shell result
  and a later presented frame.
- [ ] Close Kitty and xmonad normally and require zero live Present sources,
  fences, transactions, pending WM work, pending input, or native cleanup.
- [ ] Retain a focused regression for every protocol correction; do not add
  Kitty or xmonad branches to Engine.

Gate command and verifier must be documented in `docs/validation.md`; the
physical xmonad result must receive its own compatibility/session evidence
rather than borrowing the completed Kitty-only promotion.

### 9.2 Guarded physical xmonad run

- [ ] Start from TTY3 with the independent recovery guard armed and capture the
  launcher, guard, recovery, WM, frontend, and native-session logs.
- [ ] Automatically present a focused Kitty on the primary output within eight
  seconds.
- [ ] Prove typing, pointer motion, click-drag selection, and Super-Enter
  launching a second independently interactive Kitty.
- [ ] Prove keyboard and pointer focus changes, tiling resize, workspace
  switching, and no input delivery to hidden or unfocused windows.
- [ ] Prove correct retained content, cursor behavior, and independent
  page-flip retirement on both physical outputs.
- [ ] Close both terminals and request xmonad logout; require normal return to
  the originating TTY and successful greetd restoration.
- [ ] Run emergency recovery separately and require bounded input flush, KMS
  cleanup, exact TTY restoration, and a usable greetd session.

Milestone 9 exits only when the automated resize regression and both physical
normal/emergency captures pass from the same committed release.

---

## Milestone 10: Physical Firefox Workload

QEMU already proves the bounded Firefox protocol workflow. This milestone
tests the combined physical AMD/KMS, xmonad, Kitty, and Firefox session rather
than adding speculative browser compatibility.

- [ ] Register an explicit Firefox application action in the guarded physical
  session and launch a local/offline deterministic page.
- [ ] Require visible page rendering plus keyboard, pointer, scroll, resize,
  refocus, dialog open/close, and status-zero browser exit.
- [ ] Prove bounded UTF-8 text transfers through `CLIPBOARD` and `PRIMARY`
  between Firefox and Kitty.
- [ ] Close and restart Firefox while both Kitty windows remain interactive and
  retain their content, focus, and workspaces.
- [ ] Verify browser failure or forced close cannot terminate the X frontend,
  WM, or unrelated applications.
- [ ] Audit only the desktop services observed by this run—such as DBus,
  PipeWire, or portal helpers—and add session integration only for evidenced
  failures.
- [ ] Retain zero unexpected X11 errors, pending actions/input, native resource
  debt, and protocol-specific state below Engine.

Milestone 10 exits with one strict physical workflow verifier and three
consecutive passing runs.

---

## Milestone 11: Installed Daily-Driver Candidate

- [ ] Install versioned release binaries and a real greetd session entry; do not
  compile from source during login.
- [ ] Replace development takeover behavior with explicit seat/VT lifecycle
  ownership and bounded display-manager handoff.
- [ ] Remove ordinary-login dependence on manual `sudo`, manual process kills,
  temporary paths, and repository-relative binaries.
- [ ] Preserve the independent emergency path, a known-good fallback session,
  and a documented rollback procedure.
- [ ] Add startup diagnostics that identify the installed version and exact
  failing phase without exposing application content.
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

## Architecture Conformance Debt

The source-layout audit now prevents new unreviewed large modules, inline
production tests, and direct library printing. Existing exceptions remain an
explicit migration ledger in `docs/source-layout-exceptions.txt`.

- [x] Replace application-specific WM launch variants with opaque
  `SessionApplicationId` values owned by session configuration.
- [x] Remove per-event allocation and sorting from Engine input hit testing.
- [x] Cache the visual runtime's input-layer projection instead of rebuilding a
  vector for every input batch.
- [x] Centralize authority-transaction layer projection under Engine and
  preserve `NamespaceId` through backend templates.
- [ ] Split X authority state, connection service, routing, input, clipboard,
  wire families, dispatch families, and client output behind the existing
  public facades.
- [ ] Split live-session configuration/policy, admission, WM/layout, input,
  presentation, process supervision, and the owner loop.
- [ ] Split native scanout and the legacy WM bridge by ownership.
- [x] Extract visual diagnostics and asynchronous output servicing from the
  production visual-runtime facade.
- [ ] Move every remaining inline test to crate integration tests and split
  oversized test programs around shared `tests/support` fixtures.
- [x] Replace remaining library `println!`/`eprintln!` diagnostics with
  structured, redacted `tracing` fields.
- [x] Replace callback-owned retirement mutation with explicit Engine commit,
  protocol-feedback, and output-projection steps.
- [x] Replace callback-owned Present delivery with a bounded, owned queue
  drained explicitly by the session owner.
- [x] Remove synthetic committed-surface seeding; initial authority
  transactions pass normal Engine validation and commit.
- [ ] Remove client-local X identifiers from Engine/session routing and replace
  free-form X trace strings with bounded, typed, redacted observations.

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
