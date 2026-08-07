# Sophia Active Roadmap

Sophia is a research prototype moving toward a usable native-X daily driver.
This file contains active work, ordering constraints, and promotion gates.
Completed milestones belong in `docs/roadmap-history.md`; detailed decisions,
diagnoses, and retained evidence belong in `docs/research-log.md`.

Roadmap rules:

- Keep exit criteria measurable and fail closed.
- Expand X11 behavior only from retained real-client evidence.
- Use QEMU for repeatable protocol, policy, transaction, and application
  semantics. Do not substitute it for physical DRM, input-device, VT,
  display-manager, or visible-pixel requirements.
- Keep Engine protocol-neutral and free of application-specific policy.
- Keep the WM blind to XIDs, namespace IDs, titles, classes, PIDs, and client
  payloads.
- Rebuild and re-prove the installed candidate whenever its executable,
  packaged policy, or supervised application set changes.
- Archive a milestone when its complete exit gate passes.

---

## Current Position

Sophia's product path is its native **Sophia X Server Frontend**. Engine owns
physical input, focus authority, scene state, rendering, presentation, and
scanout. X Authority owns X11 protocol semantics and private client resources.
One versioned, protocol-neutral WM API accepts native Sophia policy clients or
legacy-X11 policy translated through a private compatibility bridge. Xmonad is
the first mature bridge profile and current promotion vehicle; it is not
Sophia's architectural WM. XLibre and Wayland prototypes remain under
`research/` as architectural evidence.

The current installed candidate provides:

- guarded two-output startup and exact TTY restoration;
- automatic Kitty, supervised xmonad, and optional unmodified xmobar;
- physical keyboard, pointer, focus, workspace, resize, clipboard, Firefox,
  floating-dialog, and normal-logout workflows;
- Engine-owned KMS presentation, protocol-neutral cursor and input policy,
  native chrome, and retained-frame recovery across VT release; and
- commit-pinned normal, fallback, watchdog, emergency, native-chrome, and
  switch-away/switch-back evidence with exact runtime identity.

Milestones 9 through 11 are complete. The 30-minute unattended churn gate and
the automated ten-cycle installed lifecycle gate also pass. Milestone 12 owns
the remaining promotion boundary. The first immutable successor exposed a
missing core depth-1 pixmap format during its focused startup gate. The
corrected successor is installed; run its focused and automated lifecycle
gates, then pass the visible color proof, two-hour interactive soak, and one
full workday.

The current Void host has the required xmonad-configuration build and runtime
dependencies installed. Dependency installation is complete and is not an
active roadmap item.

## Daily-Driver Promotion Contract

Sophia becomes a first physical daily-driver candidate only when one installed
xmonad session proves all of the following:

1. Normal login, automatic Kitty startup, and normal logout through greetd.
2. Keyboard, pointer, focus, workspaces, shortcuts, resizing, and both outputs.
3. At least two Kitty windows plus Firefox remaining independently usable.
4. Small-text `CLIPBOARD` and `PRIMARY`, dialog handling, application close,
   and representative 24-bit color rendering.
5. Clean application, WM, frontend, renderer, KMS, input, and VT teardown.
6. Independent emergency recovery from a separate destructive-path run.
7. Repeated startup/logout cycles and an interactive soak with zero unexpected
   protocol errors, stuck input, rejected callbacks, or cleanup debt.
8. Installed release artifacts: no source build, mutable home-directory policy,
   manual service repair, or ad hoc process cleanup during ordinary login.

---

## Boundary And Capability Ledger

This ledger records the current product limits so later feature work goes to
the correct authority.

### Boundaries To Preserve

- **Engine** owns physical input, outputs, work areas, scene geometry, focus,
  chrome, transactions, rendering, presentation, and scanout. It must not learn
  X11 resource identities or application metadata.
- **X Authority** owns visuals, colormaps, X resources, ICCCM/EWMH reduction,
  X11 events, client drawing, and protocol feedback. It lowers pixels and
  opaque policy facts into Engine; it does not own physical layout or scanout.
- **Blind WM policy** consumes opaque surfaces, workspaces or views, geometry,
  constraints, and permitted role facts. A native Sophia WM speaks this API
  directly. A classical X11 WM speaks to a private synthetic X server whose
  bounded profile translates its policy into the same API. Neither path may
  receive XIDs, titles, classes, PIDs, namespace IDs, or payloads.
- **Session shell and configuration** own trusted launch provenance, key-bound
  applications, status presentation, wallpaper, lock, screenshots, audio, and
  process supervision. These are not X Authority shortcuts.
- **Portals** own cross-namespace clipboard, drag-and-drop, file, URI, capture,
  and notification decisions. Only the small-text `CLIPBOARD` and `PRIMARY`
  execution path is complete.

### Current Limitations

- Release `0.1.0-56dad4de8b5f` is the installed candidate. It freezes the
  intended xmonad and xmobar policy and restores the conventional auxiliary
  pixmap depths rejected by its predecessor. Its focused live gates have not
  yet run.
- The xmonad bridge has one flattened `active_workspace` policy view even
  though the session descriptor can express output/workspace mappings. True
  independent per-output workspaces require output-scoped active-workspace
  state throughout the bridge and Engine transaction path.
- The opaque WM API lacks focus-master, swap-master/up/down, shrink/expand,
  master-count, reset-layout, focus-output, move-to-output, and supervised
  WM-restart actions.
- `ThreeColMid`, `Tall`, `Mirror Tall`, `Full`, and `Spiral` have exact
  configured-bridge geometry coverage. Xmonad's `Tabbed` layout depends on
  title-aware, WM-drawn decorations and therefore does not fit the blind-WM
  contract. If tabs are admitted later, Engine must draw metadata-free native
  tabs.
- Xmobar can render, reserve a work area, update, and retire cleanly, but it has
  no private workspace/layout/focus feed. Such a feed must be emitted by
  Engine or a trusted shell broker and contain only workspace number, approved
  layout name, and focus state—never window titles or client identity.
- Application placement cannot use xmonad class/title rules. Requested launch
  placement, such as Firefox on workspace 2, must come from trusted launch
  provenance or explicit user action.
- The X setup catalog, passive colormap ownership, RGB16 allocation,
  named-color lookup, color query, and error paths now agree on fixed 24-bit
  XRGB and 32-bit ARGB TrueColor semantics. The remaining color gate is a
  physical captured-pixel proof on the successor installed candidate.
- The daily-driver session still uses the `classic-shared` X namespace. The
  confined-group architecture and most portal executors are not yet promoted
  into the normal Firefox session.
- Tray/XEmbed, lock, screenshots, wallpaper, audio control, and general prompt
  UI are shell or portal work. `xcompmgr` must never run under Sophia because
  Sophia is the compositor.
- The compatibility bridge currently has a complete xmonad profile, not broad
  classical-WM compatibility. Other WMs such as i3, dwm, and qtile require
  separate evidence-backed profiles against the same synthetic-X and Sophia
  WM boundaries; no profile may grow into a proxy for the real X Authority.
- The small bundled native WM proves the direct API and native chrome path, but
  it is not the intended full desktop policy. Hagia is the planned first
  demanding Sophia-native WM and shell family: a blind spatial-policy process,
  an optional separately authorized shell, and ordinary Sophia session and
  portal services.

---

## Milestone 12: Immutable Desktop Candidate And Workday Soak

The previous ten-cycle gate remains valid evidence for commit `958fb5e6`, but
the closed xmonad/xmobar configuration and TrueColor semantics require a new
successor candidate. Do not begin the final soak on the older installed build.

### 12.1 Close The Intended Desktop Configuration

This section prepares the current xmonad-based promotion candidate. Its
profile-specific work must remain behind the generic compatibility boundary;
it must not make xmonad concepts part of Engine or the universal WM API.

- [x] Modernize the personal xmonad configuration for the packaged xmonad
  0.18 series without loading mutable `~/.config/xmonad` state at session
  runtime. Preserve the established Sophia key actions and use only supported
  blind geometry policy.
- [x] Admit `ThreeColMid`, `Tall`, `Mirror Tall`, `Full`, and `Spiral` with
  exact configured multi-surface geometry. Retain profile-level constraint,
  focus, workspace, floating-pointer, work-area, output-change, release, and
  bridge-restart regressions around those layout transitions.
- [x] Exclude xmonad `Tabbed`, title/class manage hooks, dzen property control,
  and xmonad-owned decorations from this candidate. Record any later native-tab
  design as Engine chrome, not an expansion of fake X drawing or WM metadata.
- [x] Package the exact configured xmonad executable and exact xmobar
  executable as immutable release inputs. Record source revision, build
  configuration, binary digest, and runtime path; reject home-source discovery
  and digest mismatch.
- [x] Keep xmobar's present static/redacted content for this milestone unless
  the soak demonstrates that a dynamic status feed is required. Dynamic
  workspace/layout/focus status belongs to the post-promotion broker slice.

### 12.2 Complete TrueColor Semantics

- [x] Make the X Authority's advertised TrueColor contract internally exact:
  validate the 24-bit XRGB and 32-bit ARGB visual/depth combinations, convert
  `AllocColor` RGB16 components through the advertised masks, and make
  `QueryColors` recover the corresponding channel intensities instead of
  reducing every nonzero pixel to white.
- [x] Replace the current “black or white” `AllocNamedColor` behavior with a
  bounded, deterministic color-name table required by retained clients.
  Unknown names must return the correct X error rather than silently becoming
  white. Do not add mutable server-wide colormap allocation to TrueColor.
- [x] Keep visual IDs, colormap IDs, channel masks, and X color names inside X
  Authority. Engine receives only bounded XRGB8888/ARGB8888 pixel content and
  protocol-neutral opacity facts.
- [x] Verify setup, create-window/pixmap/colormap validation, `AllocColor`,
  `AllocNamedColor`, `QueryColors`, both byte orders, XRGB upload, ARGB
  allocation facts, disconnect cleanup, and invalid-resource/error paths
  against X11 wire rules and the retained XLibre/Yserver references.
- [x] Advertise and retain the conventional 1/4/8/15/16 auxiliary pixmap
  formats separately from 24-bit XRGB and 32-bit ARGB visuals. Prove creation
  and geometry for every retained depth in both byte orders while keeping others
  fail-closed.
- [x] Add a deterministic non-gray XRGB upload fixture with distinct red,
  green, blue, mixed, and grayscale pixels and exact byte preservation.
- [ ] Run the real-client physical proof. Require the same palette to survive
  client rendering, Engine composition, native presentation, and capture
  without channel swaps or black/white collapse. Include a Kitty 24-bit
  ANSI-color sample, while treating its client-side rendering as an end-to-end
  pixel proof rather than a colormap-wire proof.
- [ ] Update the X11 compatibility matrix only after both the wire regression
  and visible physical proof pass.

### 12.3 Rebuild And Re-Prove The Candidate

- [x] Build and install one repository-independent candidate containing the
  pinned Sophia, configured xmonad, and xmobar artifacts. Verify the greetd
  entry uses those exact paths and digests without a source checkout.
- [ ] Run the focused xmonad-layout, xmobar/work-area, TrueColor, Kitty,
  Firefox, floating-dialog, VT switch, normal-logout, and emergency-recovery
  gates on that exact candidate.
- [ ] Repeat the one-shot ten-cycle installed lifecycle gate. It must stop at
  the first failure and return to greetd after aggregate verification, with no
  manual repair, stale graphical process, or emergency recovery in an ordinary
  cycle.

### 12.4 Interactive Soak And Promotion

- [ ] Pass a two-hour interactive soak with repeated Kitty and Firefox
  launch/close, focus, workspace, layout, floating, resize, clipboard,
  TrueColor, and multi-output actions. Every named workload must appear in
  reduced session evidence without recording typed content or metadata.
- [ ] Pass one full workday using the same committed build, packaged policy,
  application digests, and installed session entry.
- [ ] Require zero unexpected protocol errors, allocator diagnostics, rejected
  page-flip callbacks, stuck keys/buttons, presentation starvation, in-flight
  ownership, cleanup debt, or failed TTY restoration.
- [ ] Record bounded latency and health summaries without logging typed
  content, clipboard payloads, window titles, classes, PIDs, or application
  metadata.
- [ ] Rotate retained logs and preserve the exact Sophia commit and executable
  digest, configured xmonad and xmobar digests, kernel, Mesa, Kitty, Firefox,
  output, and input-seat identities.

Milestone 12 exits only when the installed path—not a repository launcher—passes
the complete Daily-Driver Promotion Contract on one immutable candidate.
Failures create the next smallest evidence-driven compatibility or lifecycle
slice; they do not justify broad X11 conformance work.

---

## Milestone 13: Public Policy Protocol And Hagia

Architecture and formal-model work may land before Milestone 12 closes because
it does not change the installed runtime. Production protocol changes begin
only after the immutable Milestone 12 candidate passes promotion.

### 13.1 Ratify And Model The Boundary

- [x] Reconcile the architecture, WM API, Hagia design, specification draft,
  and research log around one language-neutral policy protocol. Mark the
  current workspace-oriented Rust API v7 experimental and reserve
  `sophia_wm_v1` for the first stable public projection interface.
- [x] Add bounded `PolicyConnection` and `PolicyProjection` TLA+ models before
  changing production IPC or Engine policy state. Check negotiation,
  capabilities, transfer assembly, connection epochs, stale proposals,
  multi-output atomicity, focus, timeout, disconnect, restart, and
  last-committed projection preservation.
- [x] Map every model action to its owning Rust boundary. Preserve each
  implementation-relevant TLC counterexample as a deterministic Rust
  regression before correcting the implementation or model.

### 13.2 Publish A Dependency-Free Wire Contract

- [x] Keep the bounded 24-byte little-endian Sophia envelope and owner-only
  Unix transport. Make the session host role-specific sockets beneath its
  private runtime directory and admit exactly the expected supervised peer.
- [x] Define stable layouts in a narrow checked-in KDL schema. Generate and
  retain dependency-free Rust and C99 codecs, normative byte tables, and
  golden vectors; normal builds and third-party clients must not run or link
  the generator.
- [x] Add strict begin/chunk/end transfers for complete snapshots and
  projections above the 64-KiB frame limit. Bound the first WM interface to 16
  outputs, 1,024 manageable surfaces, and 256 registered bindings.
- [x] Compile an independent C client and run it against the same golden and
  malformed-frame corpus as the Rust codec. Reject unknown, excessive,
  partial, duplicate, reordered, stale, and trailing data without mutation.

### 13.3 Replace Workspaces With Output Projections

- [x] Introduce one canonical Engine reducer for complete scene snapshots and
  complete affected-output projection proposals. Validate generations,
  capability, constraints, geometry, uniqueness, one-output-per-surface, and
  visible focus before one logical commit.
- [x] Keep full snapshots and complete projections as stable semantics. Permit
  only model-equivalent chunking, coalescing, caching, or later delta encoding;
  no transport optimization may expose partial policy state.
- [x] Add the API v7-to-projection adapter and prove the dormant Rust reference
  WM and generic X11 WM bridge against the canonical reducer.
- [ ] After Milestone 12 promotion, route the installed xmonad profile through
  that adapter, migrate production to the public transport, then remove v7 and
  Engine-owned workspace state before declaring the interface stable.
- [ ] Preserve registered physical actions and session operations as opaque,
  capability-gated tokens. Keep raw input, executable commands, client
  metadata, protocol objects, namespaces, pixels, and renderer handles out of
  policy IPC.

### 13.4 Prove Hagia And Freeze `sophia_wm_v1`

- [x] Create Hagia as a standalone Nim repository with no Triad history,
  River/Wayland dependency, inherited binary, or shared build scaffolding. Its
  independent envelope and record decoder passes Sophia's retained corpus.
- [x] Complete Hagia's first socket proof: strict snapshot assembly, exact
  affected-output request, projection encoding, committed outcome, and
  canonical Engine reduction without generated Sophia or Triad protocol types.
- [ ] Keep Hagia tags, stable `ViewId` values, ordered per-output views, focus
  history, reconnect affinity, and session-local checkpoint private. Project
  them into one-output-per-surface Sophia geometry with no Hagia back door.
- [ ] Prove one tiling, one scrolling, and one bounded Janet layout plus
  actions, constraints, focus, hidden surfaces, multi-output moves, output
  loss/return, crash, restart, and hot-swap.
- [ ] Run one black-box conformance corpus against the Rust reference WM,
  Hagia, the X11 bridge, and the independent C client. Publish
  `sophia_wm_v1` only after all paths pass, then retain an archived v1 client
  as a permanent compatibility gate.

Milestone 13 exits only when the public wire is independently implementable,
the formal and deterministic gates pass, Hagia and xmonad use the same Engine
projection path, and a policy crash or replacement preserves the last coherent
desktop.

---

## Milestone 14: Native Graphics Efficiency

This milestone starts after the installed workday soak. It optimizes the same
native-X product; XLibre, Xorg, niri, river, and other mature compositors are
references rather than Sophia runtime components.

- [ ] Extend the bounded visual-retirement model before changing frame-slot,
  coalescing, multi-output, shared-worker, direct-scanout, or buffer-lifetime
  semantics. Check out-of-order output retirement, supersession, fallback, and
  release safety. Retain a deterministic Rust regression for every
  implementation-relevant counterexample.
- [ ] Recycle three generational frame-surface slots per output through
  explicit page-flip retirement, with bounded deferral when all slots are
  leased.
- [ ] Carry bounded buffer-age damage history per slot and repaint only
  accumulated damage. Fall back to a full repaint whenever history is
  incomplete.
- [ ] Keep one latest pending frame and one KMS submission in flight per
  output; prove physical input remains within half a refresh period at p99.
- [ ] Coalesce all outputs in the same DRM/render-device group onto one shared
  renderer worker. Preserve one latest pending request per output, bounded
  response demultiplexing, explicit per-output retirement tokens, and bounded
  inter-output service skew under concurrent producers.
- [ ] Add atomic-test-gated direct scanout for one compatible opaque DMA-BUF
  layer, followed by a hardware cursor plane. Retain mixed composition as the
  fail-closed fallback.
- [ ] Replace the bounded legacy cursor baseline only after one per-output KMS
  transaction owner can combine primary and cursor-plane state in the same
  atomic request. Retain bounded cursor-only idle work and the pointer-motion
  GLX cadence gate.
- [ ] Replace full immutable CPU presentation replacement for stable
  software-rendered X toplevels with lease-safe damage generations or
  copy-on-write backing. Preserve child composition, bounded storage,
  historical-handle immutability, and exact admission extents.
- [ ] Compare identical Kitty, Firefox, resize, launch-burst, and soak
  workloads against separate XLibre+xmonad and mature Wayland-compositor
  sessions on the same hardware. Comparative results are diagnostic; Sophia's
  absolute correctness and latency gates remain authoritative.

Milestone 14 exits with bounded warmed resource counts, no steady-state
allocation growth, refresh-relative latency evidence, and no change to
Sophia's native-X authority model.

---

## Post-Promotion Capability Roadmap

These are ordered product capabilities, not Milestone 12 blockers unless a
named soak failure promotes one.

### Blind WM And Multi-Output Policy

- [ ] Add opaque actions for focus master, swap master/up/down, shrink/expand,
  master count, reset layout, focus output, move surface to output, and
  supervised WM restart.
- [ ] Replace the bridge's singular active-workspace view with output-scoped
  active workspace and focus state. Prove independent workspace changes,
  surface moves, output removal, output return, and bridge restart without
  exposing application identity.
- [ ] Add trusted launch-placement provenance for configured applications.
  Keep class/title matching out of the WM and Engine.
- [ ] If tabs are justified, design metadata-free Engine-owned native tab
  chrome and opaque tab actions. Do not emulate title-aware xmonad decorations.

### Classical X11 WM Compatibility

- [ ] Separate profile-independent synthetic-X lifecycle, layout translation,
  validation, supervision, and recovery from xmonad-specific bindings and
  request patterns. Keep one shared conformance suite for every compatibility
  profile.
- [ ] Define profile admission criteria: a named upstream WM and version,
  frozen configuration, minimal captured synthetic-X request surface,
  complete opaque-action map, deterministic layout/focus/workspace/restart
  tests, and one real installed-session proof.
- [ ] Add classical WMs incrementally from retained user workflows. Likely
  candidates include i3, dwm, and qtile, but ordering follows user demand and
  evidence rather than nominal X11 compatibility.
- [ ] Reject profiles that require real client metadata, global X server
  ownership, drawing through the fake server, raw input, arbitrary command
  execution, or protocol-specific authority below Engine. Supply missing
  metadata, shell, and session behavior through their proper bounded brokers.

### Native Sophia Follow-Ups

- [ ] Add bounded policy interactions for move, resize, drag, and scrolling.
  Engine owns hit-testing, grabs, raw physical input, cursor state, and
  animation; Hagia receives only opaque targets and reduced geometry updates.
- [ ] Model and publish `sophia_shell_v1` through the same formal, schema, C
  client, and permanent-compatibility process. Keep its endpoint and
  capabilities separate from `sophia_wm_v1`.
- [ ] Build `hagia-shell` as one ordinary separately authorized shell client
  for tabs, overview, switchers, previews, and other visible furniture. Shell
  metadata must never leak into Hagia's blind spatial-policy projection.
- [ ] Add trusted classification, launch, lock, capture, output, and transfer
  services through brokers, session capabilities, and portals. Hagia may
  request opaque actions but may not receive executable paths, client
  metadata, portal payloads, or compositor authority.

### Status, Launcher, And Shell Integration

- [ ] Define a bounded redacted status feed for workspace number, approved
  layout name, focus state, output health, and supervised-component health.
  Feed xmobar through a trusted shell broker without exposing client metadata.
- [ ] Register a bounded launcher as physical action 3 and decide whether the
  compatibility UI is dmenu or native Engine/shell chrome.
- [ ] Implement lock, screenshot, wallpaper, and audio actions through their
  owning shell or portal boundaries.
- [ ] Admit tray/XEmbed only from a retained application workflow and keep it
  outside blind WM policy.

### Portals And Namespace Promotion

- [ ] Promote a confined daily-driver application group only after Firefox and
  Kitty pass the same workflow under explicit grants.
- [ ] Implement large X11 `INCR` clipboard transfers from retained evidence.
- [ ] Implement Xdnd and URI/file launching through portal grants.
- [ ] Implement prompt UI, notification actions, and capture/FD handoff through
  the existing reducers and bounded executors.

### Rendering And Compatibility Follow-Ups

- [ ] Retain the bounded physical `glxgears` proof with visible animation,
  advancing Present/KMS cadence, matching reference provider, clean retirement,
  and zero protocol or renderer debt.
- [ ] Obtain an unredirected Xorg/XLibre `Flip` reference only if end-to-end
  presentation-latency parity is needed. Keep composited `Copy` results labeled
  as client-cadence evidence.
- [ ] Complete the two-output concurrent-producer workload after the shared
  renderer-worker prerequisite in Milestone 13. Require bounded inter-output
  service skew and no producer starvation.
- [ ] Replace per-frame CPU GBM allocation with an output-scoped,
  retirement-fed three-slot pool only if measured software fallback remains
  outside its parity gate.
- [ ] Run the deterministic Firefox pointer/keyboard/wheel fixture in Chromium
  as an independent native-X consumer after Chromium is installed.
- [ ] Add client-selected classic X11 cursor images or further toolkit,
  extension, font, color, and WM behavior only when a retained workflow exposes
  the missing protocol fact.

### Hardware Diagnostics And Hotplug

- [ ] Retain the exhaustive pc105 US shifted-punctuation and Ctrl-Alt-F1
  through Ctrl-Alt-F12 physical runner as a focused diagnostic. Repeat it after
  input/seat changes or for release burn-in; ordinary candidate promotion
  requires one real VT round-trip plus the deterministic XKB suite.
- [ ] After work-area, output, or seat changes, re-run the exhaustive xmobar
  reservation lifecycle and require no stale gap, overlap, resize timeout, or
  focus change. Pair dynamic output-topology behavior with the later physical
  multi-output hotplug gate.

---

## Secondary Development Tooling

Interactive QEMU is useful for reproduction but is not a physical daily-driver
blocker. Work on it only when it shortens an active milestone.

- [ ] Complete one human-visible `xmonad-interactive` capture proving pointer
  movement, terminal launch, typed text, focus change, application close, and
  clean manual shutdown. The fail-closed verifier, mutations, and RFB capture
  already pass.

## Deferred

- XLibre provider integration until measured native-X gaps justify its
  authority and maintenance cost.
- Any new application protocol or compatibility frontend without a
  specification amendment backed by named product evidence.
- VRR activation until physical hardware reports `vrr_capable=1`.
- General X11 conformance work not required by a retained daily-driver client.
