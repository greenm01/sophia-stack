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

The currently retained installed candidate provides:

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
the remaining promotion boundary. Immutable xterm attempt `0003` closes the
corrected CPU-scene VT-recovery boundary. The same archive now also closes the focused
xmobar/work-area gate through exact two-output reservations, repeated exact
bar repaints, primary retirement, clean logout, and packaged xmobar identity.
TrueColor attempt `0003` from `883666a2` closes the physical color boundary
through exact X11 color requests, final primary-output composition evidence,
Kitty DMA-BUF color, native retirement, normal logout, and exact recovery.
Independent emergency attempt `0004` and ten-cycle range `0044` through `0053`
also pass on `883666a2`. Preliminary soak attempt `0054` then retained a
workspace-isolation and first-admission failure in that candidate. The bridge
and admission fixes passed short successor run `0055`. The next source
successor adds the practical opaque-action profile, immutable Engine-owned
IR_Black chrome, and self-reporting soak evidence. It requires one short
installed proof before restarting the two-hour soak; the full-workday gate
remains after it. Active client projection onto output 2 is a separate
compatibility boundary and is not claimed by the TrueColor gate.

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

- Release `0.1.0-4c3121421f12` remains installed. Automatic Firefox attempt
  `0002` passes the dedicated immutable gate, including exact renderer-image VT
  capture/restore, the browser and floating-dialog workflow, clean normal
  logout, zero unexpected protocol errors, and no retained profile. The
  remaining promotion work is outside this focused browser boundary.
- The xmonad bridge has one flattened `active_workspace` policy view even
  though the session descriptor can express output/workspace mappings. True
  independent per-output workspaces require output-scoped active-workspace
  state throughout the bridge and Engine transaction path.
- The xmonad compatibility profile now exposes opaque focus-master,
  swap-master/up/down, shrink/expand, master-count, reset-layout,
  toggle-floating, and sink actions without expanding the WM wire format.
  Focus-output, move-to-output, output-scoped layout state, and supervised WM
  restart remain compatibility work.
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
  physical captured-pixel proof on the successor installed candidate. The
  proof command and fail-closed archive verifier are implemented but do not
  count as physical evidence until a new installed run passes.
- The daily-driver session still uses the `classic-shared` X namespace. The
  confined-group architecture and most portal executors are not yet promoted
  into the normal Firefox session.
- Tray/XEmbed, lock, screenshots, wallpaper, audio control, and general prompt
  UI are shell or portal work. `xcompmgr` must never run under Sophia because
  Sophia is the compositor.
- Full classical-desktop parity remains explicitly deferred and ownership
  split. A trusted shell/session broker must own arbitrary launch, lock,
  screenshots, wallpaper, audio/media/eject, and launch-placement provenance.
  Engine chrome must own metadata-free tabs, decorations, and fullscreen
  presentation. A redacted shell feed must own workspace/layout/focus labels.
  The X compatibility layer still needs tray/XEmbed, output focus/move,
  optional input aliases such as Super+Tab and button-2 swap-master, and
  evidence-backed per-WM profiles. None of these may introduce titles,
  classes, XIDs, PIDs, namespace identity, or executable commands into the
  blind WM boundary.
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

## Milestone 12: Frozen Classical-WM Compatibility Baseline

The previous ten-cycle gates remain valid historical evidence for commits
`958fb5e6` and `56dad4de`. Installed `1a7d67c3` retains the admission-recovery
failure, installed `7bd3e7db` retains the move-feedback failure, and installed
`a50dfb67` retains the committed-layout reseed failure. Installed `53a21365`
fixes that failure but retains the false core `GetImage` reply ceiling.
Installed `fb1c3804` fixes both and reaches zero unexpected protocol errors but
retains unbounded ephemeral Firefox profiles. Installed `ce494942` fixes that
resource lifecycle and run `0042` proves the cleanup. Installed `7a6be56c`
routes Firefox attempts correctly but exposes a renderer-worker settlement gap
during VT handoff. Installed `4c312142` closes that gap and passes automatic
Firefox run `0002`. All remaining promotion gates and soaks must use this exact
immutable build or a verified source-identical successor.

### 12.1 Close The Intended Desktop Configuration

This section records the xmonad compatibility baseline. Its profile-specific
work remains behind the generic compatibility boundary; it must not make
xmonad concepts part of Engine or the universal WM API.

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
- [x] Adapt the safe practical core of the retained personal xmonad profile to
  opaque Sophia actions: focus/swap master, swap up/down, shrink/expand,
  master-count, layout reset, floating toggle, and sink. Keep physical chords
  Super-based and private compatibility chords inside the bridge.
- [x] Package the IR_Black-derived one-pixel focused/unfocused frame as an
  Engine core configuration. Keep xmonad borders disabled and xmobar static,
  redacted, and title-free.
- [x] Add one shared soak-evidence reducer, an installed
  `sophia-soak-progress --watch` command, and a checksummed redacted summary.
  Require every practical action once while retaining workload thresholds and
  exact zero-debt health gates.

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
- [x] Run the real-client physical proof with `sophia-truecolor-proof` and
  verify it with `sophia-verify-truecolor-runs 1`. Require the same palette to
  survive client rendering, Engine composition, native presentation, and capture
  without channel swaps or black/white collapse. Include a Kitty 24-bit
  ANSI-color sample, while treating its client-side rendering as an end-to-end
  pixel proof rather than a colormap-wire proof.
- [ ] Update the X11 compatibility matrix only after both the wire regression
  and visible physical proof pass.

### 12.3 Rebuild And Re-Prove The Candidate

- [x] Model and implement two-phase admission recovery: retire the fallback,
  clear only its temporary constraint, retain one bounded standing-target
  successor, queue one normal relayout, and commit the target only after exact
  native retirement. Fail session completion on any standing-target debt.
- [x] Deliver complete position-and-size geometry to X Authority for every
  changed surface while retaining resize-only pixel epochs. Require pure-move
  Present-before-core ConfigureNotify, no-op silence, complete timeout
  rollback, focus-only stability, terminal Engine/X convergence, and a
  committed-layout reassertion when stale pixels remain at another size.
  Cover the boundary in Rust, verifier, and TLA+ gates.
- [x] Build and install the new source successor as one repository-independent
  candidate containing the pinned Sophia, configured xmonad, and xmobar
  artifacts. Verify the greetd entry uses those exact paths and digests without
  a source checkout. Installed `4c312142` and Firefox run `0002` prove the
  immutable artifact, runtime identity, automatic recorder, and handoff fix.
- [x] Re-run the installed xterm startup that exposed the auxiliary-pixmap
  defect. Require exact two-output and work-area readiness, a presented
  correctly sized xterm, clean VT switch-away/resume, normal WM logout, zero
  protocol errors, exact TTY restoration, and no host process residue. The
  installed `sophia-xterm-proof` command now reserves and verifies this
  profile-specific archive automatically. Attempt `0001` exposed and now
  regresses the launcher's Kitty-only argument leak. Attempt `0002` then proved
  the corrected launcher and clean CPU-backed VT recovery, while exposing that
  the first verifier fixture had incorrectly modeled xterm as a Present client
  with an imported image. The corrected CPU-snapshot contract and a fresh
  immutable run remained. Installed attempt `0003` passes on `7e18ea3a`: exact
  inset CPU geometry, two-output scene rehydration, normal VT recovery and WM
  logout, zero protocol errors, exact TTY restoration, and no process residue.
- [x] Run the focused Kitty, Firefox, floating-dialog, xmonad-layout, VT-switch,
  and normal-logout gate on that exact candidate. Automatic Firefox run `0002`
  passes the immutable aggregate verifier.
- [x] Close the focused xmobar/work-area gate from the checksummed installed
  xterm attempt `0003`. `sophia-verify-xmobar-work-area` now requires exactly
  one 14-pixel reservation on both outputs, repeated exact 2560-by-14 primary
  repaints, native retirement, packaged xmobar identity, normal logout, clean
  lifecycle, and unmodified archive checksums.
- [x] Run the automatic physical TrueColor gate on the source successor.
  Installed attempt `0003` passes directly with `reverified=0` on `883666a2`.
- [x] Run the independent emergency-recovery gate on that same candidate.
  Installed attempt `0004` passes on `883666a2` with both observers, drained
  keys and native presentation, status-130 handoff, and exact TTY restoration.
- [x] Repeat the one-shot ten-cycle installed lifecycle gate. It must stop at
  the first failure and return to greetd after aggregate verification, with no
  manual repair, stale graphical process, or emergency recovery in an ordinary
  cycle. Installed runs `0044` through `0053` pass on `883666a2`; startup
  readiness remained between 288 and 324 milliseconds across both outputs.

### 12.4 Interactive Soak And Promotion

- [x] Retain preliminary soak attempt `0054` as a failed immutable artifact.
  It completed clean logout and TTY recovery after 413,133 milliseconds but
  recorded four layout timeouts and WM restarts. Hidden synthetic-window
  geometry now stops at the compatibility bridge, and first admission uses
  the selected safe pixel extent before driving the WM's standing target.
- [x] Audit workspace/admission recovery with the optional commit-pinned
  Specula development tool, then retain only project-sized formal models and
  deterministic regressions. Exact snapshot replacement and direct assignment
  now keep unique cached membership; hard-deadline or other bridge failure
  requires process replacement; and a pixel-silent first admission preserves
  its owner, standing target, and one bounded retry. The pinned TLA+ suite
  covers projection, response-boundary, and pixel-silent behavior without
  adding a runtime or build dependency.
- [x] Install the successor and pass one short workspace/admission proof with
  Kitty, Firefox, glxgears, and vkcube before repeating a long soak. Installed
  run `0055` on `a2fdf4f6` retained clean admission and workspace projection,
  two independently advancing animated surfaces, zero layout timeout, resize
  abort, hidden-surface configure/render command, or WM restart, and clean
  normal-session teardown.
- [x] Re-run the retained practical xmonad acceptance in isolated QEMU after
  the physical launch incident. It completed focus, layout, workspace,
  pointer, launch, bridge restart/reseed, and clean logout; the prelude's
  one-versus-two visible-surface predicate was corrected and locked by the
  verifier regression. QEMU does not satisfy the physical short gate below.
The remaining practical short gate and long xmonad soaks are deferred to the
Classical X11 WM Compatibility roadmap. Resume them after native Hagia is
solid; xmonad is regression evidence, not the promotion vehicle.

---

## Milestone 13: Public Policy Protocol And Hagia

This is the active promotion milestone. Production follows the public native
policy path directly; the frozen xmonad baseline remains a regression and
future compatibility target, not a prerequisite.

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
- [x] Audit retained Triad capabilities against Sophia, Hagia, River, and Niri.
  Keep spatial policy in Hagia; keep input, client settlement, rendering, and
  scanout in Engine; reserve separate session, shell, broker, and portal roles.
- [x] Extend the policy models for ordered action causes, policy-initiated
  reprojection, configuration generations, frontend settlement, reduced
  pointer interactions, and opaque session-operation outcomes before adding
  those transitions to the draft wire.

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
- [x] Complete draft revision 1 before stability: add output work rectangles;
  reduced surface kind, presentation request/current state, and exact-size
  constraints; projection presentation decisions; request causes; policy
  configuration; Engine chrome; session-operation tokens; reduced
  interactions; and a bounded policy-dirty request.
- [x] Preserve non-idempotent activation order with the existing bounded
  sixteen-request owner queue. Coalesce only replaceable scene refreshes and
  continuous interaction geometry; saturation consumes the shortcut, fails
  closed, and emits a bounded diagnostic.
- [x] Regenerate and re-run the Rust/C golden and malformed corpora, then update
  Hagia's independent Nim codec without adding a Sophia build dependency.
- [x] Add the indicator descriptor to revision 1 before the 13.4 freeze.
  `capability "indicators" bit=8`, record kinds `ProjectionIndicator` (max 256)
  and `ProjectionOutputStatus` (max 16), and `indicator_count`/`status_count`
  fields in `ProjectionBegin` are in the schema with generated Rust and C99
  codecs and golden vectors. The generator gained a fixed-octet field type so
  records could carry bounded labels while staying fixed width. Wire bounds are
  permanent: 256 indicators, 16 status records, 32-byte UTF-8 labels and layout
  names. The 32-per-output limit is Engine validation, not a wire constant.
  See `docs/sophia-indicator-descriptor.md`.
- [x] Model the descriptor before changing the schema. Revise
  `validation/tla/ShellObservation.tla` so the descriptor rides the proposal and
  its invariants hold with no explicit publish or invalidate step, and add
  `validation/tla/IndicatorTransfer.tla` for declared-count, ordinal, and
  bounds integrity across begin/chunk/end.
- [x] Regenerate the Rust and C99 codecs, wire tables, and golden corpora for
  the new records. `tools/check_policy_protocol.sh` passes end to end. Closing
  it also repaired a pre-existing gap: the C conformance harness had never been
  taught `snapshot_session_operation` or the five policy/session messages, so
  its valid-frame and record gates had been failing before this work began.
- [x] Update Hagia's independent Nim codec for the new records so the
  cross-repository conformance gate stays green, without adding a Sophia build
  dependency. Hagia decodes both records, rejects an over-long label length and
  non-zero padding, and declares zero indicators until it advertises the
  capability. `SOPHIA_STACK_ROOT=… nimble test` passes.
- [x] Defer the tier-1 texture question rather than blocking on it. Whether the
  shared transport can carry shell texture traffic under the 64-KiB frame limit,
  single in-flight transfer, and bytes-only wire binds `sophia_shell_v1` only.
  Tier-0 Engine chrome renders the descriptor with no client interface, which
  removes that question from the freeze path; see
  `docs/sophia-shell-v1-direction.md` open question 2.

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
- [ ] Route the installed native profile through the public transport and
  canonical reducer, then remove v7 and Engine-owned workspace state after the
  Hagia recovery gate. Migrate xmonad later through the compatibility adapter.
- [ ] Preserve registered physical actions and session operations as opaque,
  capability-gated tokens. Keep raw input, executable commands, client
  metadata, protocol objects, namespaces, pixels, and renderer handles out of
  policy IPC.
- [x] Add a two-stage canonical reducer: validate a complete proposal against a
  cloned successor, preserve last-good authority, and reject promotion if its
  connection, request, scene generation, or earlier commit was superseded.
- [ ] Wire staged projections through production frontend configure and
  renderable-content settlement. Emit `committed` only when authoritative
  state matches; otherwise request a fresh snapshot without silently changing
  policy geometry.
- [x] Bind the owner-only endpoint before a supervised peer starts, authorize
  its exact UID/PID afterward, and prove that ownership order through the
  independent C and Hagia conformance host.
- [ ] Host the production endpoint in the Sophia session, supervise exactly one
  admitted peer, preserve the committed scene across replacement, and keep
  policy checkpoints private to that peer.

### 13.4 Prove Hagia And Freeze `sophia_wm_v1`

- [x] Create Hagia as a standalone Nim repository with no Triad history,
  River/Wayland dependency, inherited binary, or shared build scaffolding. Its
  independent envelope and record decoder passes Sophia's retained corpus.
- [x] Complete Hagia's first socket proof: strict snapshot assembly, exact
  affected-output request, projection encoding, committed outcome, and
  canonical Engine reduction without generated Sophia or Triad protocol types.
- [x] Keep Hagia tags, stable `ViewId` values, ordered per-output views, and
  reconnect affinity private. Project the implemented equal-column and
  fixed-point scrolling layouts into one-output-per-surface Sophia geometry
  with no Hagia back door.
- [ ] Add deterministic Hagia reducer messages for view/tag changes, focus,
  movement, grouping, layout adjustment, output focus/moves, presentation
  state, floating, scratchpads, and opaque session operations.
- [ ] Add Engine-owned pointer interactions, bounded focus history, private
  checkpoint/reconciliation, and crash/restart proof while applications and
  the last committed scene remain alive.
- [ ] Prove the retained tiling and scrolling layouts plus actions, constraints,
  focus, hidden surfaces, multi-output moves, output loss/return, crash,
  restart, and hot-swap. Defer Janet until candidate validation and fallback
  behavior have their own model and deterministic tests.
- [ ] Run one black-box conformance corpus against the Rust reference WM,
  Hagia, the X11 bridge, and the independent C client. Publish
  `sophia_wm_v1` only after all paths pass, then retain an archived v1 client
  as a permanent compatibility gate.

### 13.5 Migrate And Promote The Native Policy Path

- [ ] Install a bounded Hagia profile through the session-hosted public
  transport and canonical reducer, using only the retained column and
  scrolling layouts. Prove Kitty, Firefox, floating dialogs, work areas,
  ordered repeated actions, pointer move/resize, multi-output views,
  `glxgears`, `vkcube`, policy restart, and clean logout.
- [ ] Remove API v7 and Engine-owned workspace policy after Hagia passes the
  restart and last-layout gates. Keep the adapter and its deterministic xmonad
  regressions until classical-WM migration resumes.
- [ ] Freeze `sophia_wm_v1` and retain an archived revision-1 client only after
  the Rust reference, Hagia, X11 bridge, and C client pass the complete
  black-box corpus.

Milestone 13 exits only when the public wire is independently implementable,
the formal and deterministic gates pass, installed Hagia uses the Engine
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

- [ ] After native promotion, reinstall the practical xmonad profile and pass
  its physical short gate, two-hour soak, and workday soak on one immutable
  candidate. Require exact action and pointer commits, correct Kitty, Firefox,
  xmobar, chrome, and TrueColor behavior, zero lifecycle debt, redacted health
  summaries, and checksummed artifacts.
- [ ] Migrate that profile through the public projection transport without
  changing retained behavior; it must use the same Engine reducer as Hagia but
  may keep its profile translation behind the compatibility adapter.
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
- [ ] Consider a conventional GTK3 desktop profile such as Xfce as the driver
  for X11 compatibility completeness: EWMH coverage, `_NET_WM_STRUT_PARTIAL`
  work-area reservation, and tray/XEmbed admission. Such a profile draws its
  own pixels and can never exercise a display-list interface, so it is
  compatibility evidence only and must not be cited as `sophia_shell_v1`
  evidence; see `docs/sophia-shell-v1-direction.md`.
- [ ] Reject profiles that require real client metadata, global X server
  ownership, drawing through the fake server, raw input, arbitrary command
  execution, or protocol-specific authority below Engine. Supply missing
  metadata, shell, and session behavior through their proper bounded brokers.

### Native Sophia Follow-Ups

- [x] Ratify target-resolved input as a pre-schema prerequisite. The contract
  resolves against per-output presented snapshots, admits targets only inside
  their owner's visible allocation, gives non-recyclable authority/session
  identity and device/contact-bound per-seat capture, and paces normalized
  continuous values. Exceptional coordinates require an independently issued,
  revocable region/precision/rate capability. The target, pacing, and
  cross-authority arbitration TLA+ models precede any `sophia_shell_v1` schema
  or runtime work; see `docs/target-resolved-input.md`.
- [x] Admit the first complementary architecture-model gate. Alloy checks
  bounded role/namespace/portal and presented-target topology; Z3 checks target
  geometry/disclosure arithmetic and schema-generated `sophia_wm_v1` wire
  bounds. Every promoted rule retains a satisfiable negative control. Keep
  temporal ownership in TLA+ and keep Spin, dependency policy, and fuzzing
  deferred until they have concrete retained artifacts; see
  `docs/architectural-alignment.md`.
- [x] Ratify the WM/shell hardening prerequisites. Blind WM policy cannot share
  a protection domain with metadata-bearing shell, broker/portal, or frontend
  roles; opaque actions bind issuer/recipient epochs, operation class, and
  target generation; and a tier-1 shell reservation, derived work area, and WM
  projection promote as one exact presented bundle. Alloy and
  `ShellWorkAreaCoordination` retain focused negative controls. This is target
  architecture, not a shipped shell or sandbox.
- [ ] Enforce protection domains in session supervision before admitting a
  metadata-bearing shell: close ambient descriptors, prohibit conflicting role
  composition and unsupervised cross-domain IPC/shared writable state, and add
  executable isolation tests. Exact UID/PID socket admission alone is not this
  gate.
- [ ] Implement issuer-scoped action-capability validation and the atomic
  shell-reservation/work-area/WM coordinator with the eventual shell schema.
  Preserve the prior complete presented bundle on ordinary failure and keep
  lock/session security takeover independent of shell or WM acknowledgement.
- [ ] Repair native application input before shell coexistence.
  - [x] In the installed primary-output pointer domain, derive hit-test layers
    from the immutable output-frame snapshot only after accepted page-flip
    retirement. Committed/submitted moves, removals, and stacking changes no
    longer become selectable before their pixels.
  - [ ] Introduce output-local pointer coordinate domains and per-output
    presented interaction epochs rather than merging independently retired
    heads into one global projection.
  - [x] Advance Sophia `SurfaceId` generations when a client reuses an XID and
    retire the exact frontend route on successful surface removal. Frozen or
    deferred generation-N input cannot resolve to generation N+1.
  - [ ] Turn frontend grabs into Engine-visible profile-scoped leases with
    ordered release acknowledgement; isolate a non-reading client's queue
    failure; and revalidate deferred focus-handoff routes against current
    membership and authority epoch. Lock, session, and other security
    transitions must revoke locally without waiting for frontend cleanup.
- [ ] Add bounded policy interactions for move, resize, drag, and scrolling.
  Engine owns hit-testing, grabs, raw physical input, cursor state, and
  animation; Hagia receives only opaque targets and reduced geometry updates.
- [ ] Model and publish `sophia_shell_v1` through the same formal, schema, C
  client, and permanent-compatibility process. Keep its endpoint and
  capabilities separate from `sophia_wm_v1`. Start only after 13.4 freezes
  `sophia_wm_v1`; the two interfaces are sequential, not parallel. Derive the
  vocabulary from a driving client with a retained scene graph rather than from
  first principles; see `docs/sophia-shell-v1-direction.md`.
- [ ] Settle the remaining display-list vocabulary before schema work. Admit
  generic target regions and a desktop-background surface class, evaluate analytic
  screen-corner and indeterminate-progress primitives, and refuse per-widget
  visual novelty in favor of client-rasterized textures. Record the damage,
  bandwidth, and power cost of that texture path before relying on it.
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
  Workspace number, layout name, and focus state are settled by the indicator
  descriptor and arrive on the layout commit, not through a broker: policy owns
  them and no broker has an upstream source. Output and supervised-component
  health remain session-owned and still need a path. See
  `docs/sophia-indicator-descriptor.md`.
- [ ] Render tier-0 indicator chrome in Engine from the committed descriptor,
  reusing the existing `capability "chrome"` path and the renderer-neutral
  display list. Admit no new primitive. Add a verifier in the shape of
  `tools/verify_sophia_native_chrome.sh` plus one physical TTY3 proof. This
  replaces xmobar's role without any client interface.
- [ ] Emit indicators from Hagia's private tags, keeping tags private and
  crossing only labels, state bits, and action tokens. Extend the
  cross-repository conformance gate to cover them.
- [ ] Register a new bounded opaque launcher action and decide whether the
  compatibility UI is dmenu or native Engine/shell chrome. Do not reuse the
  established xmonad layout-action IDs.
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
