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

Milestones 9 through 12 are complete historical evidence for the xmonad
compatibility profile and are archived in `docs/roadmap-history.md`. Their
bounded lifecycle, recovery, color, work-area, and soak artifacts remain
reproducible regressions, but elapsed wall time is not a current promotion
criterion. Milestone 13 owns the active product path. Hagia is already the
ordinary remembered installed session: it passed bounded deterministic
preflight as attempt `0004`, records every real session automatically, and
leaves Kitty, xmonad, and the previous immutable release available for
recovery. What remains in Milestone 13 is the `sophia_wm_v1` freeze and the
retained Triad behavior port that gates it. Protocol freeze and legacy-policy
removal remain separate evidence-driven decisions, in that order: API v7 may
not be removed until the freeze conditions hold.

The current Void host has the required xmonad-configuration build and runtime
dependencies installed. Dependency installation is complete and is not an
active roadmap item.

## Installed Hagia Promotion Contract

Sophia/Hagia becomes the ordinary physical session when one packaged candidate
passes bounded deterministic preflight and preserves all recovery routes. Live
use then produces immutable evidence rather than serving a fixed-duration gate:

1. Normal login, automatic Kitty startup, and normal logout through greetd.
2. Exact Sophia and Hagia executable identities in every archived attempt.
3. Clean application, policy, frontend, renderer, KMS, input, and VT teardown.
4. Ctrl-Alt-Backspace returns safely and is classified as `recovered`, never
   as a clean session.
5. Unexpected termination and invalid final health remain failed evidence.
6. Installed release artifacts: no source build, mutable home-directory policy,
   manual service repair, or ad hoc process cleanup during ordinary login.
7. No minimum elapsed time, launch count, or action count. Scenario coverage is
   cumulative and informational; a named scenario becomes a gate only when its
   affected architectural change requires that bounded proof.

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
- [x] Add an explicitly selected Hagia live profile through the public
  transport and canonical reducer, with no silent API-v7 fallback.
- [x] Promote that profile to the installed native default while retaining
  Kitty, xmonad, and the previous immutable release as recovery routes.
- [ ] Remove v7 and Engine-owned workspace state after the complete Hagia
  restart and last-layout gates. Migrate xmonad later through the compatibility
  adapter.
- [x] Preserve registered physical actions and session operations as opaque,
  capability-gated tokens. Keep raw input, executable commands, client
  metadata, protocol objects, namespaces, pixels, and renderer handles out of
  policy IPC.
- [x] Add a two-stage canonical reducer: validate a complete proposal against a
  cloned successor, preserve last-good authority, and reject promotion if its
  connection, request, scene generation, or earlier commit was superseded.
- [x] Wire staged projections through production frontend configure and
  renderable-content settlement. Emit `committed` only when authoritative
  state matches; otherwise request a fresh snapshot without silently changing
  policy geometry.
- [x] Bind the owner-only endpoint before a supervised peer starts, authorize
  its exact UID/PID afterward, and prove that ownership order through the
  independent C and Hagia conformance host.
- [x] Host the production endpoint in the Sophia session, supervise exactly one
  admitted peer, preserve the committed scene across replacement, and keep
  policy checkpoints private to that peer.

### 13.4 Prove The Draft Boundary And Port Triad

Revision 1 remains experimental throughout this section. The fixed nine-view
scroller proves the public boundary and supports daily use, but it is not the
feature ceiling. Before any freeze, close Hagia's retained-behavior port ledger
across spatial policy, Hagia Shell, Sophia session/dedicated authorities, and
the required brokers/portals. River/Wayland and Niri compatibility machinery
is excluded; retained product behavior is not.

- [ ] Implement the minimum experimental display-list, target-resolved input,
  redacted broker, and shell-role transport needed to port retained Triad shell
  workflows before the WM freeze. Keep this endpoint distinct from
  `sophia_wm_v1`; this item does not itself stabilize `sophia_shell_v1` or pull
  general rendering-efficiency work forward from Milestone 14.
  **Start with the broker, not the transport.**
  `docs/sophia-shell-v1-direction.md` is explicit that the metadata broker is the
  larger prerequisite: the redacted presentation feed has no implementation, and
  without it the shell interface would be specified against a data source that does
  not exist. Specifying a transport first would produce a wire for nothing to send.
  The feed has exactly two sources with different trust properties, and conflating
  them is the failure this row is most likely to have. Policy-authored structure —
  workspace list, occupancy, focus — is blind-safe by construction and already
  answered by `docs/sophia-indicator-descriptor.md`, because workspace state
  originates in the policy process where no broker can see it. Only toplevel
  identity for taskbars and docks needs real sanitization, and keeping the two apart
  is what lets a status bar never request identity at all.
  The first buildable piece is therefore the toplevel descriptor and its reduction:
  a record a shell can render, constructible only by reducing client metadata, so
  that leaking a title, app ID, PID, or path is a compile error rather than a review
  finding. Activation, close, and minimize ride the existing opaque action tokens,
  which already exist and already carry issuer scoping — the broker mints no new
  authority for them. Decision 2 settled the neighbouring question for the WM side:
  classifications reach policy through a capability-gated extension chunk, so the
  broker's design is no longer constrained by what fits in `SnapshotSurface`.
  The crate question is settled: `sophia-broker`, its own crate and its own
  `PolicyRole::Broker` socket. Not `sophia-portal`, whose broker is a single-use
  transfer grant lifecycle with nothing in common beyond the word, and whose own
  ownership row forbids the client-global visibility a metadata broker needs.
  **Built so far**, all with the authority reducing and the broker never holding raw
  identity: the disclosure vocabulary in `sophia-protocol`; authority-side reduction
  in `sophia-x-authority`; the `sophia-broker` crate owning trust, icon tokens,
  disclosure rules, and descriptor emission; `PolicyRole::Broker` with its own socket
  and env var; and the metadata broker health smoke reporting the real broker.
  The chain is proven to compose end to end in `crates/sophia-cli/tests/metadata_chain.rs`
  — authority reduction, broker, `ChromeDescriptorTable` — including that a title
  never reaches Engine under a `ClassOnly` rule, and that the Engine ingress needed
  no widening to accept broker output.
  **Not yet hosted.** Neither `ChromeDescriptorTable` nor `MetadataBroker` has a
  production instance; both live in tests. Hosting them needs a session owner and,
  because they are separate authorities, the broker interface family's own wire —
  schema, codec, and revision line under clause 3, which is its own tranche rather
  than a loose end of this one. Until that lands the chain is proven and unwired,
  which is worth stating plainly: no running session produces a chrome descriptor
  today.

- [x] Create Hagia as a standalone Nim repository with no Triad history,
  River/Wayland dependency, inherited binary, or shared build scaffolding. Its
  independent envelope and record decoder passes Sophia's retained corpus.
- [x] Complete Hagia's first socket proof: strict snapshot assembly, exact
  affected-output request, projection encoding, committed outcome, and
  canonical Engine reduction without generated Sophia or Triad protocol types.
- [x] Keep Hagia tags, stable `ViewId` values, ordered per-output views, and
  reconnect affinity private. Project the fixed nine-view, fixed-point
  scrolling layout into one-output-per-surface Sophia geometry
  with no Hagia back door.
- [ ] Add deterministic Hagia reducer messages for view/tag changes, focus,
  movement, grouping, layout adjustment, output focus/moves, presentation
  state, floating, scratchpads, and opaque session operations.
  The fixed nine-view scroller profile now covers ordered view moves, output
  focus and movement, column consume/expel, width/height adjustment, floating,
  fullscreen, maximize, minimize/restore, and operation-slot-bound opaque
  session requests. Hagia now retains nonempty multi-tag view and
  focused-window membership through eighteen additional opaque actions;
  dynamic workspace lifecycle, occupancy navigation, and scratchpads are now
  implemented in Hagia's action catalog; configured workspace naming remains
  partial because `setWorkspaceName` has no bound action. The transitions
  remain unbound until configuration migration preserves Triad's existing chord
  meanings. Revision 1 admits 256 binding registrations, so capacity is not the
  constraint: the bootstrap emits 39 key plus 2 pointer bindings from Triad's
  baseline 132 key plus 5 pointer bindings, and the other 96 classify into
  shell, broker, portal, and session authorities that do not exist yet. That
  authority split, not a slot count, is what keeps binding classification on
  the pre-freeze port path.
- [ ] Add Engine-owned pointer interactions, bounded focus history, private
  checkpoint/reconciliation, and crash/restart proof while applications and
  the last committed scene remain alive.
  One completed Engine-captured move/resize now crosses as a reduced final
  interaction, and Hagia validates its target, capability, output bounds, and
  exact floating geometry. Focus and minimized histories are bounded, and an
  owner-only, size-bounded checkpoint uses a same-directory fsynced atomic
  replacement and is revalidated before complete-snapshot reconciliation. The
  owner-side recovery matrix now kills the public
  path at proposal-staged, frontend-pending, prepared, and terminal-outcome
  boundaries through the normal supervisor; all four preserve layout, restart
  at a fresh epoch, and drain cleanly. Continuous updates, a physical
  checkpoint restore and operation-phase faults remain open. The separate
  client lifecycle gate now defines post-negotiation and complete-snapshot
  crash/replacement checks; it remains unclaimed until an authorized live run.
  Hagia now emits exactly one bounded `PolicyDirty` after a restored checkpoint
  first reconciles and commits; the independent socket test proves generation
  advance and a fresh complete cycle. Both installed Hagia gates require its
  diagnostic after restart, but no physical evidence has been claimed.
  `tools/hagia_policy_physical_gate.sh` now encodes the opt-in two-output
  restore/presentation/active-output procedure. Its first authorized run
  proved checkpoint save, supervised epoch-2 restart, checkpoint load and
  reconciliation, retained fullscreen geometry, and post-restart page flips,
  then failed the exact text boundary because seven keys arrived while Engine
  and frontend focus differed. Engine now retains only those client-bound keys
  in a bounded exact-target handoff while continuing to resolve Hagia's
  reserved chords. A replacement run then proved the exact text and clean
  bounded shutdown, but the operator entered the final phrase before the
  post-restart actions. That run also showed that checkpoint occurrence 4 was
  reached during startup, before the intended physical trigger. The gate now
  arms occurrence 6, after the two pre-restart actions, labels every missing
  phase explicitly, and uses the phrase only as the final exit signal. A third
  run delivered exactly the 34 press/release events for the 16-character final
  phrase plus Enter, with no preceding physical chord events, no ordered
  actions, and no restart. It was a clean negative attempt rather than routing
  evidence. The gate now keeps an event-driven procedure visible inside Kitty,
  advances only when each committed action appears in the evidence stream, and
  withholds the final phrase until the restart and complete action sequence are
  proven. Its first guided run exposed a harness-ordering bug: the leading
  Super transition reached the application path before the bound action key
  was consumed and incorrectly entered the exact text matcher. The text proof
  now excludes non-text modifier transitions while leaving their ordinary
  client delivery intact; an unmatched non-modifier key still fails exactly.
  The next guided run proved the complete pre-restart sequence, epoch-2
  checkpoint load/reconciliation and refresh, post-restart fullscreen, and two
  maximize transitions. It then minimized its own guide before displaying the
  restore instruction and timed out with no text events. Minimize and restore
  are now one visible paired instruction. A follow-up still stopped after
  minimize with exactly the twelve routed modifier transitions belonging to
  the six committed chords. The guide now leads with an explicit three-line
  warning to press and release `Super+R` while the screen is blank, and evidence
  distinguishes immediate physical action admission from later policy commit.
  A later attempt reached the second maximize prompt but exhausted the old
  deadline while the operator was reading. Timestamp review corrected that
  diagnosis: a separate 15-second physical-sequence timeout fired, while the
  global deadline had not. Physical proofs now retain that fail-fast default
  but may request an explicit bounded override; this human-guided gate uses ten
  minutes inside an eleven-minute global ceiling and still exits immediately
  on success. The following run proved that `Super+R` was admitted and
  committed but still left a blank scene: minimize had removed the surface
  from Sophia's supposedly complete policy snapshot, so Hagia correctly
  reconciled it as destroyed before applying restore. Sophia now retains
  authority-observed facts separately from visible layers. A follow-up run
  proved that the X `mapped` observation also cannot carry policy lifetime:
  Engine admission is not a second client `MapWindow`, so an admitted surface
  may retain its pre-admission `mapped=false` observation. The snapshot now
  follows explicit request/withdraw ownership until withdrawal or removal. The
  gate additionally requires a nonempty checkpoint after restore, rejecting
  the observed committed no-op. The first request/withdraw-lifetime run then
  failed closed at startup because authority presentation observations admitted
  three transient surfaces while one lacked an X11 client route. Snapshot
  admission now intersects retained policy ownership with a live authority
  route, which survives minimize but excludes unrouted hierarchy observations;
  the regression covers that race. The nonexclusive real-Hagia restart smoke
  passes this boundary with one routed surface, nonempty epoch-2 checkpoint
  reconciliation, and clean shutdown. The following physical run proved the
  complete action sequence and, critically, retained one nonempty surface
  through minimize and restore. It then failed after accepting the final text
  because Kitty closed while Hagia's last one-surface projection was in flight.
  Public response handling now advances current Engine scene facts before
  materializing placements and retires such a response as `RejectedStale`.
  The guide now remains alive after accepting the phrase until Sophia completes
  the proof, rather than voluntarily closing during evidence settlement.
  The next attempt admitted and committed both `Super+Right` chords but never
  restarted: legitimate extra settlement checkpoints had shifted the fixed
  occurrence used for fault injection. The physical gate now correlates its
  one restart to the committed fullscreen action, the first committed
  active-output action, and the next nonempty checkpoint. Its watcher runs
  beside an `exec`-replaced Hagia, preserving the supervisor-authorized PID,
  and the marker prevents a second epoch from being killed.
  The resulting run proved restart timing, checkpoint recovery, and the entire
  physical action sequence, then flushed all 52 action-plus-text key
  transitions. It failed only because the stock semantic-result writer was
  appended after Kitty's custom guide command and therefore became unused
  guide arguments. Sophia now passes the private result path explicitly and
  the guide records the exact line it reads; an isolated completed-log replay
  proves the witness.
  The final physical run passed: one causal restart preserved the nonempty
  checkpoint and fullscreen state; every required post-restart action
  committed; all 52 routed action-plus-text transitions flushed; exact text
  changed pixels and reached a kernel page flip in 24 ms; and health, topology,
  native ownership, namespace, Xauthority, and process cleanup were clean.
  Continuous pointer updates and operation-phase fault coverage remain open
  within this broader item.
- [x] Carry committed public-policy fullscreen, maximize, minimize, and restore
  state through the X frontend's protocol-visible state transition and verify
  exact configure/state feedback. Engine geometry, focus exclusion, semantic
  minimized placement, and render-layer omission are implemented. The offline
  path now installs protected `_NET_WM_STATE` and ICCCM `WM_STATE`, waits for a
  flushed frontend acknowledgement before policy promotion, and restores the
  previous state on rejection. Exact socket tests cover property values,
  notifications, and denied client overwrite/deletion. The installed Hagia
  physical gate now proves all four transitions with real Kitty across one
  supervised checkpoint restart and clean shutdown.
- [ ] Prove the retained scrolling layout plus actions, constraints,
  focus, hidden surfaces, multi-output moves, output loss/return, crash,
  restart, and hot-swap. Port Janet after candidate validation and fallback
  behavior have their own model and deterministic tests; both remain on the
  revision-1 freeze path.
  `PolicyOutputSettlement.tla` now proves the topology core for an atomic
  two-output candidate, output loss, and generation-advancing return. Dynamic
  output ingress now uses a capacity-one udev rescan hint, an owner-wide
  quiescence/rebuild barrier, one routed-input epoch advance, complete
  scanout/pointer/RandR/policy publication, and policy-plus-presentation fence
  before input resumes. `OutputTopologyLifecycle.tla`, Alloy, Z3, and focused
  Rust tests cover the offline boundary. Its guarded physical multi-output
  disconnect/reconnect harness remains unrun, so installed evidence is open. The public
  owner now admits only complete, valid output snapshots atomically, advances
  generations after disappearance, selects a surviving active output, and
  recognizes same-ID descriptor changes without partially mutating state.
  Unified desktop output admission now also constructs a pure stable-ID plan
  with exact candidate and rollback states after revalidating the complete
  reconciliation against the owned capability snapshot. Startup still issues
  no configuration KMS mutation; atomic test/apply execution and backend
  settlement remain the next dedicated output-authority tranche. A pure
  coordinator now models those typed test/apply/rollback effects and exact
  generation/digest completions, rejects stale or phase-invalid results,
  discards test-rejected candidates, and requires terminal rollback settlement
  after apply failure. That coordinator is no longer dead code: a typed effect
  executor trait and a bounded driver now carry one prepared candidate from test
  through apply or rollback to a terminal settlement, and startup drives the real
  phase machine. Deterministic
  tests cover activation, declined test, rollback after apply failure, and failed
  recovery, and prove the declined path never reaches apply.
  **Startup's test phase now reaches the kernel.** The candidate is resolved into
  topology heads and submitted as one `TEST_ONLY` request, so the kernel judges the
  complete desktop rather than nothing at all. Startup still issues no configuration
  KMS mutation, and not because a flag says so:
  `NativeOutputTopologyValidationExecutor` has no apply to gate. Its heads carry no
  plane state, so there is nothing to scan out and applying them would activate
  CRTCs showing nothing; mutation needs `NativeOutputCommitExecutor` and real
  framebuffers.
  A validated topology still settles as rejected, because apply then refuses, and it
  refuses with the same `WouldBlock` a busy device reports. The settlement alone
  therefore cannot distinguish an accepted desktop from a busy card, so the executor
  retains what the kernel said and startup logs it separately as `validation=`.
  A topology spanning two DRM devices is declined rather than validated: one atomic
  request reaches one device, and validating a fragment would answer a question
  nobody asked.
  `tools/run_native_output_gate_tty4.sh` passed on tty4 at commit `eab7922e`,
  which is the first identity-pinned evidence for this row:
  `result=passed`, probe accepted with and without plane state, and
  `validation=accepted outputs=2 heads=2`. Its transcript carries the commit, the
  binary's checksum, the sysfs connector facts, and a digest over the body, so the
  claim can be rechecked rather than believed — the checksum matches what that
  commit builds and the digest matches the body.
  `tools/native_topology_validate.sh` runs that chain read-only against real
  hardware — capabilities, projection, reconciliation, plan, heads, phase machine —
  differing from startup only in opening the cards directly rather than through a
  seat controller. **Its first run passed on the AMD two-output reference:**
  `validation=accepted settlement=not_applied outputs=2 heads=2`. The kernel accepts
  the configured two-output desktop as one atomic request.
  What remains unproven offline is the executor adapter itself. Covering its three
  arms needs a fake atomic-commit device, which means the `drm` crate in
  `sophia-cli`, and `docs/live-backend-dependency-policy.md` keeps device-facing
  types in `sophia-backend-live`. The submission beneath it and the resolution above
  it are both covered; the seam is covered by that hardware run.
  Apply now exists behind two gates and remains **unrun**. `native-topology-apply`
  refuses without `SOPHIA_NATIVE_OUTPUT_APPLY=1`, and
  `tools/native_topology_apply.sh` refuses again, validates first, and refuses to
  apply a topology `TEST_ONLY` would not accept. What apply can reach is bounded by
  construction rather than by care: apply heads reuse the framebuffer each CRTC
  already scans out, so the only topology expressible is one whose scanout size
  matches what is displayed, and anything else declines as `NeedsFramebuffer` before
  a commit is submitted. A mode change needs a buffer allocated at the new size,
  which is renderer work and is not in this tranche.
  **That bound turns out to exclude the reference host, which was not the
  expectation.** The plan was that re-applying the topology already on screen would
  be the smallest real mutation available. It is not available: the first authorized
  run refused, and the refusal names why. `DP-2` is a 1920x1080 panel scanning out a
  2560x1440 framebuffer, because the console gives both CRTCs one buffer sized for
  the larger monitor, while the candidate asks each output for its own preferred
  mode. There is no correctly sized buffer to reuse, so no apply is expressible here
  without allocating one. Notably the shared buffer is the mirror-group shape, and
  the two outputs disagree on size, so reusing it for both heads would fail
  `MismatchedMirrorSize` as well — the same constraint reached from the other side.
  Apply therefore stays unrun, and its hardware evidence waits on the renderer
  rather than on anything in the apply path. Nothing was submitted and no output
  state changed; the resolver failed closed before the first commit, which is the
  behavior the gate exists for.
  **Apply is frame-fed, not scratch-fed.** The obvious unblock — allocate a buffer
  at the new mode's size so apply can run whenever it likes — is foreclosed by
  `docs/renderer-import-boundary.md`: native KMS initialization waits for the first
  committed-state frame rather than requiring a speculative or blank visual
  bootstrap. A scratch buffer is that bootstrap, and it would put a frame on screen
  that no committed state produced. The sequence is therefore resize the frame
  target to the new mode, compose one frame at that size from committed state, then
  apply the topology naming that frame. `LiveGbmEglFrameTargetRecord` and its
  created/retained/resized/invalidated/retired lifecycle already model the resize
  step; what was missing was the precondition tying it to activation.
  `native_output_apply_admission` is that precondition. It answers whether every
  enabled output has a valid frame target at its requested mode, and names the
  output and both sizes when one does not, so a mode change reports where the
  session is in the transition rather than reporting a hardware defect. Disabled
  targets owe no frame, for the same reason they contribute no head. Apply consults
  it before composing anything, which turns a missing framebuffer from a discovery
  mid-resolution into a refusal before submission: the reference host now reports
  `native output 2 has a 2560x1440 frame but the requested mode is 1920x1080`.
  Both the precondition and head composition read the currently scanned-out
  framebuffer through one reader, `read_native_current_framebuffer`, so they cannot
  disagree about what "currently displayed" means.
  What remains for a real apply is the renderer half: resizing the frame target on a
  configured mode change and recomposing before activation runs. The three-slot
  recycling pool in Milestone 14 stays gated on its own measurement and is not a
  prerequisite here.
  Rollback heads are resolved beside apply heads, before anything is submitted,
  from the topology still on screen. Sourcing them afterwards would source them from
  a desktop that is already wrong. An output that cannot be restored fails the whole
  plan closed, so an apply never begins without a way back.
  Applying blocks and carries no page-flip event: a modeset must complete before a
  caller may believe it did, and there is no flip to wait on.
  The DRM primitives that executor needs already exist:
  `LibdrmNativeAtomicCommitRequest` exposes `modeset`, `allow_modeset`, and
  `test_only`, and property discovery already finds connector `CRTC_ID` and CRTC
  `MODE_ID`/`ACTIVE`. Composing one request across N heads now exists too:
  `build_native_multi_head_atomic_request` folds every head into one
  `AtomicModeReq` so the kernel accepts or rejects the complete topology and a
  partially applied desktop is never observable. It validates before adding any
  property, so a rejected build never yields a half-populated request, and it
  rejects an empty head set, a shared connector, CRTC, or plane, an invalid size,
  and a missing mode blob on a modeset. Heads sharing one framebuffer are a mirror
  group and must agree on scanout size, which is where the same-mode rule is
  enforced. The returned request carries the previously dead `test_only` path, so
  a caller can validate a topology without touching hardware.
  A planned timing now resolves to a mode too: `resolve_native_output_mode_index`
  matches a requested timing against a connector's reported modes and returns the
  first match, which is the same choice the capability reader makes when it dedupes
  reduced timings, so advertisement and commit cannot disagree about which mode a
  timing names. `create_mode_blob` takes that resolved mode, and
  `create_mode_blob_for_selection` delegates to it.
  `submit_native_multi_head_topology` submits one topology as one request, setting
  `TEST_ONLY` for a validation intent and reporting an unbuildable head set apart
  from a kernel refusal, since one is a mistake in what was asked for and the other
  is hardware declining something well-formed. `NativeOutputCommitExecutor` adapts
  that to the activation reducer and gates apply, so a caller can validate against
  real hardware and then decline.
  What remains is the piece that cannot be settled offline: resolving a candidate
  into heads. That means naming connectors, CRTCs, and planes for outputs that may
  not be active yet, sourcing correctly sized framebuffers for a mode that is not
  running, and sourcing the previous topology's heads for rollback.
  `native-topology-probe` exists to answer the one question that decides how much
  of that is needed: whether a `TEST_ONLY` modeset requires plane state and a valid
  `FB_ID`, which is driver-dependent. It is read-only — every commit carries
  `TEST_ONLY` and the only framebuffer it names is one the CRTC already scans out,
  so nothing is allocated and no output state changes. It submits the same modeset
  twice, once with connector and CRTC state only and once with plane state added,
  and reports the two outcomes separately. `tools/native_topology_probe.sh` runs it
  as one step: it refuses to start while a display server holds the card, because a
  `MasterUnavailable` run proves nothing, then builds, captures the report, and
  states the conclusion.
  Atomic commits need DRM master even to validate, so the probe reports
  `MasterUnavailable` rather than a rejection when a compositor holds the card, and
  refuses to draw a conclusion.
  **The framebuffer question is answered.** On the AMD two-output reference, from a
  bare TTY holding DRM master, both probes are accepted: 2 connected connectors, 36
  modes, `without_plane_state=accepted`, `with_current_framebuffer=accepted`. A
  `TEST_ONLY` modeset validates with connector `CRTC_ID` and CRTC `MODE_ID`/`ACTIVE`
  alone, so resolving a candidate into heads does **not** need a framebuffer
  allocated at the new mode's size before anything can be checked. Validation can
  precede allocation.
  Getting there required fixing a defect the probe existed to expose. Its first run
  reported both probes rejected with `EINVAL` at matching mode and framebuffer sizes,
  which was not the hardware refusing anything: `LibdrmNativeAtomicCommitRequest`
  defaulted `page_flip_event` true and `test_only()` did not clear it, and the kernel
  rejects `TEST_ONLY` together with `PAGE_FLIP_EVENT` with `EINVAL` before inspecting
  a single property. Every validation-only commit in the tree returned `EINVAL`
  unconditionally, including `submit_native_multi_head_topology`'s `Validate` intent,
  so `NativeOutputCommitExecutor::validating` would have declined every topology and
  looked like hardware saying no. The flag is now derived rather than stored, so the
  combination is unrepresentable, and a deterministic test holds it.
  That is also the reason the report now carries errno and both sizes: the first run
  recorded `Rejected` with nothing to diagnose it by, and a rejection that cannot say
  why is indistinguishable from a bug in the asker.
  A separate power authority now exists: `crates/sophia-engine/src/output_power.rs`
  holds per-output levels for the outputs the desktop currently has. Power is kept
  apart from enablement because blanking a screen and removing a monitor are
  different facts — a dark output keeps its bounds, work area, and surfaces, and
  policy must not see the transition, while a disabled output leaves the complete
  snapshot and forces a relayout. The distinction is easy to lose at the KMS layer,
  where atomic modesetting powers a head down by clearing the same CRTC `ACTIVE`
  that disables one; that is a property of the commit, not a licence to merge the
  two above it. A topology change preserves the level of every surviving output and
  keeps none for a departed one, so a mode change cannot relight what was powered
  down and a reconnected monitor cannot inherit a stale level. Power transitions do
  not travel through topology activation: they alter no geometry, invalidate no
  candidate, and need no rollback beyond the previous level. The KMS write waits on
  the same framebuffer allocation apply does.
  Reservations turned out to be largely present rather than missing. Work areas are
  already re-projected from the new output rects as part of topology publication
  (`owner_loop/topology_phase.rs`), inside the same publication that swaps the
  outputs, so geometry and work area do commit together on the hotplug path. What
  was missing was any test pinning it; a mode change that shrinks an output under a
  live reservation is now covered.
  One fail-open edge is now documented and pinned rather than fixed. Reservations
  are root-relative, so a shrink can leave one outside the new root, and such a
  reservation is filtered *before* reduction — reduction then succeeds and reports
  the full output as available, silently releasing the reservation. An out-of-root
  reservation that arrives malformed should indeed be ignored, and the pure reducer
  cannot tell that case from one that was valid until the mode changed, because it
  holds no previous state. The fail-closed path exists next door: a reduction that
  returns `None` makes callers preserve the previous work area. Closing the gap
  means deciding at the layer that does hold previous state
  (`SurfaceOutputReservationState`), and it changes behavior for every bar, so it is
  called out here rather than settled quietly.
  That decision is now taken, and it rules out both shortcuts. Failing closed by
  preserving the previous work area is wrong after a shrink: the preserved rectangle
  belongs to the larger output and would put policy beyond the screen. Releasing the
  reservation is suboptimal; preserving a stale one is incoherent. Clamping the span
  inside the pure reducer is also wrong, because the reducer cannot tell a span
  clamped by a shrink from one that arrived malformed, and admitting the latter to
  fix the former trades a fail-open edge for weaker rejection. The fix belongs in
  `SurfaceOutputReservationState`: a reservation already admitted against a larger
  root is re-projected onto the smaller one, while a reservation arriving for the
  first time is validated against the current root and rejected if it does not fit.
  Same geometry, different provenance, different answer. Implementation is open.
  Evidence follows that.
  `PolicyRefreshLifecycle.tla` additionally proves that newer dirty
  generations survive an older in-flight refresh and that active output
  settles atomically with the frontend layout. Alloy and Z3 retain operation
  binding and presentation-geometry attacks alongside their protected checks.
- [ ] Implement native output mirroring as one logical output backed by N
  connectors, and prove it on two same-mode heads. This closes the port ledger's
  mirroring requirement with evidence rather than an exclusion. The shape is fixed:
  policy sees exactly one `SnapshotOutput` and no connector identity, so mirroring
  carries no `sophia_wm_v1` wire risk. The rejected shape is two logical outputs
  sharing surfaces, which violates one-output-per-surface and raises
  `DuplicateSurface`; do not attempt it.
  Ordering is fixed by the standing rule that the bounded visual-retirement model
  is extended before multi-output or buffer-lifetime semantics change. Joint
  multi-head retirement is exactly that, so Milestone 14's first item moved here.
  Both prerequisites are now closed. `validation/tla/VisualRetirement.tla` carries
  a head layer beneath output retirement: a logical output retires only when its
  last head flips, the framebuffer stays leased until then, and one output in the
  checked configuration is a two-head mirror group, so a single run exercises
  joint retirement within a group and independent retirement between groups.
  Head loss drops its lease without counting as a flip and fails the candidate
  closed. 112,252 distinct states at depth 19; all 23 models pass. The ratified
  output-scoped presentation invariant in `docs/engine-architecture.md` is
  narrowed to match rather than dropped.
  The configuration half is done. `mirror` is now an arm of the closed named-output
  KDL match, carrying the connectors a primary drives, and
  `DesktopNamedOutputCandidate` holds them. The group is named from its primary
  because policy sees one `SnapshotOutput` and no connector identity, so the group
  needs a single owner and the configured output is it.
  Validation is split by what each layer can answer. Parsing rejects a group that
  names itself, repeats a connector, names none, or exceeds the output bound —
  mistakes that hold whatever hardware is attached. The whole candidate rejects one
  connector claimed by two logical outputs, the single arrangement that would make
  "one logical output backed by N connectors" untrue. The topology rejects an
  unknown or disconnected member, and a member that cannot present the primary's
  mode, resolved through the same `resolve_mode` the primary's own reconciliation
  uses so the two cannot disagree. Same-mode is enforced here rather than later
  because no plane scaling exists anywhere on this path, and the alternative to
  refusing is silently letterboxing a screen the operator asked to match.
  A group that would work is then refused as `MirrorUnsupported` until the scanout
  half exists. Refused, not dropped: a mirror directive that is accepted and ignored
  leaves an operator staring at an unmirrored screen with no error to search for.
  The impossible cases are reported before the unsupported one, because "this asks
  for something impossible" and "Sophia cannot do this yet" send an operator to
  different places.
  What remains lifts singular per-output connector selection to a set with per-head
  page-flip intake, shares the rendered buffer lease that is exclusive today, allows
  N heads per logical rect in the topology, exempts mirror-group members from the
  overlap rejection they would otherwise trip, projects the group into
  `DesktopOutputState`, and handles mirror-group member loss in topology settlement. Mirroring is same-mode
  only because no plane scaling exists anywhere; mismatched modes must fail closed
  at reconcile time rather than silently letterbox. Hagia's output migrator gains a
  `mirror` arm, which closes a Triad config-migration gap.
  The scanout foundation is now in place. `outputs()` returns logical outputs
  rather than one per head, and the first-match lookups that silently addressed
  head 0 -- the callback router, presentation feedback, stable present, and head
  composition -- resolve a head explicitly. Head composition is keyed by connector
  name, not by `OutputId`: two activation targets in a group share an `OutputId`,
  so keying by output composed the same head twice and left the group's other
  connector dark. `NativeMirrorGrouping` is keyed by connector name for the same
  reason it has to be -- configuration speaks names, and the name-to-id map lives
  on a capability that is only readable after the sessions the grouping is needed
  to build.
  The two models that could not express head loss now can.
  `OutputTopologyLifecycle.tla` carries a live head count and the head set recorded
  at publication, so losing one connector of a group republishes the full epoch
  exactly as losing an output does; `PublishedHeadsAreCurrent` fails when it does
  not. `PolicyOutputSettlement.tla` gives each output a head set that feeds the
  canonical scene, so a head-set change makes an in-flight candidate stale and
  `CommittedTopologyWasCurrent` keeps its meaning under mirroring. Both new
  invariants were confirmed non-vacuous by negative controls that let a head loss
  leave the scene alone. `VisualRetirement.tla` needed no change; it was already
  ahead of the code.
  Most of the render path is now in place. Page-flip callbacks carry the connector,
  which the decode already had as a slot and threw away; the router matches on it
  instead of taking the first head with the right output, and the monotonic
  frame-serial guard is per connector so a sibling's flip is no longer rejected as
  a stale repeat of the first. A group retires when its last head flips rather than
  its first, which is what keeps a framebuffer from being destroyed while a sibling
  connector still scans it out; whether a head has flipped is derived from the
  intake's per-head serials against the submission baseline rather than tallied a
  second time. The callback channel is per logical output, because a group's heads
  feed one runtime. The topology owner carries a head count beside the output list,
  so losing one connector of a group is a new candidate rather than no change at
  all -- the code answer to `PublishedHeadsAreCurrent`.
  One fault found on the way was wider than mirroring. Nine call sites passed a
  position in the logical-output list into a parameter that indexes the head table.
  The two agree exactly while every output has one head, so it never failed; a
  group ends the coincidence and every output after it would be driven through the
  wrong connector. Every per-head entry point now takes an `OutputId` and resolves
  the head itself, and the lookup is renamed `primary_head_index` because
  `output_index` is the name that invited the mistake.
  The hardware has now answered the question buffer sharing rests on. On card0,
  DP-1 and DP-2 share modes up to 1920x1080, and a validation-only two-head modeset
  naming one framebuffer for both primary planes is accepted -- as is one mode blob
  serving both CRTCs. The control ran beside it, two heads with a framebuffer each,
  and was also accepted, so the acceptance is about sharing rather than about the
  driver being indifferent to two-CRTC commits. A mirror group can therefore own a
  single buffer, which is what the exporter-per-group and single-`ADD_FB2` design
  assumes. `tools/native_mirror_probe.sh` is the probe; it allocates dumb buffers
  and destroys them, and every commit carries `TEST_ONLY`, so it changes nothing.
  The probe's second phase found something better than it went looking for. Both
  CRTCs on this machine already scan out **the same framebuffer handle** -- the
  kernel console drives two connectors from one buffer -- which is a stronger
  demonstration that sharing works on this hardware than any validation-only commit
  can give. It is not a mirror group, because the two run different modes
  (2560x1440 and 1920x1080), and `MismatchedMirrorSize` refused the request exactly
  as it should: one buffer cannot satisfy two scanout sizes without plane scaling.
  So one hardware question stays open, and joint retirement already depends on the
  answer: does a two-CRTC page-flip commit deliver one event per CRTC, or one per
  commit? One per commit and a group waiting for every head waits forever. It
  cannot be asked with `TEST_ONLY`, which the kernel rejects together with
  `PAGE_FLIP_EVENT`, and it cannot be asked of the current desktop, which is not a
  group. It belongs in the tty4 output gate, where Sophia owns the modeset and both
  heads are same-mode by construction -- which is the condition the question is
  about. Until it is answered, treat one-event-per-CRTC as an assumption the
  retirement rule rests on rather than a fact.
  Three implementations were read as references, and the result narrows the design.
  **niri** and **river** do not implement mirroring at all: niri keys surfaces by
  `crtc::Handle` and river creates one `Output` per `wlr_output`, so both are one
  logical output per connector with no group concept. niri's only mention is a
  comment allowing for the possibility. Neither offers a model to copy.
  **X.Org's modesetting driver does**, and has for a long time
  (`hw/xfree86/drivers/video/modesetting/pageflip.c`). What it does differs from the
  plan on one point that matters. Even with `atomic_modeset` enabled,
  `drmmode_crtc_flip` builds a request holding **one CRTC's** plane properties and
  commits it alone with its own cookie; `ms_do_pageflip` loops over every enabled
  CRTC issuing one such commit each. X deliberately does not name several CRTCs in a
  single atomic commit. It joins them afterwards with `flipdata->flip_count`, a
  refcount incremented per queued flip, and `ms_pageflip_handler` notifies the
  extension and calls `drmModeRmFB` on the old framebuffer only when the last
  completion arrives -- which is joint retirement, arrived at independently and
  already the shape `group_awaiting_flip` has.
  So item 8 changes: submit **per head** and join by count, rather than building a
  multi-head page-flip commit. That also closes the open hardware question, because
  one commit per CRTC unambiguously yields one event per CRTC; the ambiguity only
  ever existed for the multi-CRTC commit nobody ships. Three further details are
  worth taking: X holds a local reference across the submit loop so a first-flip
  failure cannot free the shared state still in use; on partial failure it does not
  try to cancel flips already queued, and removes the new framebuffer only if none
  were; and it offers `async_flip_secondaries` because a group otherwise throttles
  to its slowest head, judder that X names as specifically a clone/mirror problem.
  The architectures differ where Sophia's contract requires it. X has no logical
  output at all: the screen is one canvas, CRTCs are viewports at an `(x, y)` offset
  into a single shared framebuffer, and mirroring is two CRTCs given the same
  offset -- which RandR clients see as two CRTCs. Sophia's one-logical-output-backed-
  by-N-heads exists so policy sees one screen and no connector identity, so the
  grouping cannot simply be borrowed even though the buffer sharing and the
  retirement count can.
  The exporter now belongs to the logical output rather than the head, built from
  the modifiers every head of the group can scan out -- a modifier only one plane
  advertises would make a buffer its sibling cannot display, so the intersection is
  the only safe set. Several counts moved with it, from per head to per exporter,
  because a group's single exporter was being counted once per connector.
  The render path is complete. Submission is per head joined by a completion count,
  on X's model: the export and the framebuffer happen once, and only objects,
  request, and commit loop. The hazard there was ownership -- every failure path
  destroyed the resource bundle, which is right for one head and catastrophic for a
  group, because once any head's commit lands a connector is scanning that buffer.
  Ownership now transfers to the submission on the first successful commit and
  later failures keep it, with `PartiallySubmitted` carrying that through three
  layers so the candidate fails closed without the buffer being freed.
  Head loss fails closed rather than hanging. X leaves a flip queued on a display
  that went away waiting forever and calls it a configuration error;
  `VisualRetirement.tla` settles such a generation as removed, so the lost head
  leaves the awaited set, never counts as a flip, and a survivor's flip cannot
  retire the frame as displayed.
  The config gate is open. `MirrorUnsupported` is gone, `mirror_of` names each
  group's primary, members take the primary's whole visual state rather than merely
  being allowed to overlap it, and `reject_overlaps` skips same-group pairs because
  members share a position by definition. The activation plan admits capabilities
  sharing an `OutputId`, which is what a group is.
  **Not reachable from any live path, which an earlier note in this entry got
  wrong by calling it complete.** Three disconnections, found by tracing rather
  than assumed. The config side and the KMS side are not wired together:
  `LiveProductionNativeScanout` builds sessions through `into_page_flip_sessions`,
  which hardcodes `NativeMirrorGrouping::none()`
  (`hardware_validation/atomic_scanout_card/session.rs:591`); the mirroring-aware
  constructor beside it is called only from tests. So a configured group parses,
  validates, and then reaches a KMS layer that is always told there are no groups.
  Frame targets never resize: `observe_gbm_egl_frame_target_size_for` and
  `observe_output_size_for` (`runtime/frame_target.rs:39`, `:57`) have no
  production callers at all, so an output's frame is fixed at the size it had when
  the runtime was built, and the only way a size changes is tearing the runtime set
  down and rebuilding it on hotplug.
  And no session can apply a modeset. The live session uses
  `NativeOutputTopologyValidationExecutor`, whose `apply` is a hard refusal --
  "Apply is not gated here, it is absent" (`desktop_output_commit.rs:35`). The one
  apply-capable executor is built only by the standalone `native-topology-apply`
  command, which opens its own DRM master and so cannot run against a live session.
  Nothing triggers an activation after startup either: no reload, no signal, no
  control-socket message.
  The tty4 gate cannot express any of this. Both `native-topology-validate` and
  `native-topology-apply` pin the profile source to `None`
  (`backend.rs:363`, `:434`), which means the compiled default
  `output { inherit-sophia #true; }` rather than the operator's config, so the gate
  can never request a group.
  The shortest path to visible mirroring avoids runtime mode changes entirely:
  wire config groups into `NativeMirrorGrouping` at session construction, and let
  the session's *initial* modeset bring each head up at its own mode, composing the
  scene into each head's buffer at a fitted rect. That needs no apply executor, no
  runtime trigger, and no frame-target resize, because nothing changes mode after
  startup. The framebuffer is created inside each head's submit, so two
  heads sharing a buffer object would each `ADD_FB2` and get distinct handles, and
  `RM_FB` would then run twice against one handle -- the second failing and latching
  cleanup permanently. Both have to be resolved together with the lease, which is a
  move-only affine token whose `Drop` is the release.
  Then the config tranche: `mirror_of` on `DesktopOutputState`, the same-group
  overlap exemption, and deleting `MirrorUnsupported`. That tranche is what makes
  an end-to-end mirror test reachable at all; until it lands, reconciliation
  refuses both the shared position and the `mirror` directive, so the
  head-composition test asserts the projection shape those changes must preserve.
- [ ] Apply a desktop output configuration change to a *running* session, so an
  operator can change a mode, position, scale, or transform without restarting.
  None of this exists today, and the pieces that look like it are startup-only.
  Four gaps, in dependency order, all found by tracing rather than assumed:
  Frame targets never resize. `observe_gbm_egl_frame_target_size_for` and
  `observe_output_size_for` (`runtime/frame_target.rs:39`, `:57`) have no
  production callers at all, so an output's frame is fixed at the size it had when
  the runtime was built. The only way a size changes is tearing the whole runtime
  set down and rebuilding it, which is what hotplug rescan does.
  No session can apply a modeset. The live session drives activation with
  `NativeOutputTopologyValidationExecutor`, whose apply is a hard refusal --
  "Apply is not gated here, it is absent" (`desktop_output_commit.rs:35`). The one
  apply-capable executor, `NativeOutputCommitExecutor::activating`, is built only
  by the standalone `native-topology-apply` command, which opens its own DRM
  master and therefore cannot run against a live session at all.
  Nothing triggers an activation after startup. The activation block runs once in
  `run_persistent_xterm_session` before the event loop; there is no reload, no
  signal handler, and no control-socket message that reaches it. The output
  profile is read once at construction and never re-read.
  And nothing reconciles after a successful apply: the engine topology, the
  session's `initial_outputs`, and the policy scene would all still describe the
  old modes.
  Ordering note: this is **not** a prerequisite for mirroring. A group's heads can
  come up at their own modes during the session's initial modeset, which needs
  none of the above. The two were conflated once already, and the frame-target
  gap was misdiagnosed as mirroring's blocker when the real one was that the
  standalone apply command composes nothing and can only re-apply what is already
  on screen.
- [ ] Run one black-box conformance corpus against the Rust reference WM,
  Hagia, the X11 bridge, and the independent C client. This is draft boundary
  evidence while the Triad port is incomplete; it does not publish or freeze
  `sophia_wm_v1`.
  The authenticated behavior host now runs the Rust reference, independent C,
  and Hagia clients through the same sequential eleven-scenario corpus:
  constrained single output, two-output partition, output loss/migration, and
  generational return, followed by an ordered focus action, timeout discard,
  and successful post-timeout recovery. Hagia retains its private adapter
  across the sequence. Stale-scene and invalid-candidate outcomes are also
  discarded before later successful cycles. The X11 bridge's explicit API-v7
  corpus adapter consumes the same canonical scenes and causes, combines each
  affected output, and passes the reducer without claiming public-wire
  negotiation. Shared reconnect/restart remains open, as does the archived
  revision-1 client.

### 13.5 Migrate And Promote The Native Policy Path

- [ ] Install a bounded Hagia profile through the session-hosted public
  transport and canonical reducer, using only the retained column and
  scrolling layouts. Prove Kitty, Firefox, floating dialogs, work areas,
  ordered repeated actions, pointer move/resize, multi-output views,
  `glxgears`, `vkcube`, policy restart, and clean logout.
  Packaging and installation accept an explicitly supplied Hagia binary and
  publish `Sophia Hagia (Native Policy)` beside Kitty and xmonad recovery
  entries. Promotion means selecting that entry as the remembered ordinary
  session after deterministic preflight; it has no workday-duration
  prerequisite. Every exit must enter the checksummed Hagia ledger as passed,
  recovered, failed, or pending, with cumulative scenario coverage.
  Installed attempt `0001` reached both physical outputs and admitted Kitty,
  then failed closed at `layout_pending`: Hagia proposed `2560x1440` while the
  Engine retained a coherent `1323x1424` recovery extent. The immutable failed
  attempt and exact TTY recovery are preserved. Public proposals now undergo
  Engine constraint reconciliation before reducer staging, the bounded live
  restart regression passes, and a replacement physical attempt remains the
  acceptance proof.
  Installed attempt `0002` then exposed the public admission-ownership half of
  the same boundary: the corrected Manage committed, but remained eligible for
  replanning until visual retirement, producing 3,121 reconciliations before
  startup failed closed. A committed public Manage now consumes only its exact
  planning ownership while retaining the independent visual-retirement fence;
  deterministic tests cap the resulting policy and checkpoint traffic.
  Installed attempt `0003` then completed the physical session but exposed an
  evidence-only manifest collision: the release and attempt recorder emitted
  the same Hagia digest field. The immutable attempt remains failed closed.
  Signed successor `66a279286bddd0354b6022102c4dac5254e34481`
  canonicalizes that field without weakening duplicate rejection. Installed
  attempt `0004` passed exact Sophia/Hagia identity, two-output startup,
  physical actions, presentation, clean normal logout, lifecycle, coverage,
  and archive verification. Hagia is now the ordinary remembered session; the
  item remains open for the named restart, state-transition, application, and
  output-topology scenarios above.
- [ ] Remove API v7 and Engine-owned workspace policy only after both freeze
  conditions below hold. Keep the adapter and its deterministic xmonad
  regressions until classical-WM migration resumes.
  Extraction was not gated and came first. The shortcut half is done:
  `WmShortcutRegistry` and `WmShortcutRouter` now live in
  `crates/sophia-engine/src/shortcut.rs`, which mentions no API version, hello, or
  wire frame. `WmShortcutRegistry::new` takes the bindings, capabilities,
  generation, and chrome a caller already resolved, so the public path builds its
  registry from configuration instead of fabricating a `WmHello` stamped
  `WM_API_VERSION`. No such fabrication remains outside tests. The v7 adapters —
  `from_hello`, which adds only the API-version check, and `apply_policy_update`,
  which speaks `WmPolicyUpdate` and `WmPolicyAck` — stay in `wm.rs` as `impl`
  blocks on the extracted types, so deleting that module deletes the adapters and
  not the behavior.
  `WmSocketTransport` was deliberately **not** lifted, against the ledger pointer's
  wording. It exists to encode and decode v7 frames, eleven call sites of them, and
  is reached only by the legacy bridge; the public path uses `PolicyTransport` and
  never touches it. Moving it would relocate v7 code rather than free anything, and
  it should be deleted with v7 once the xmonad profile migrates to the public
  projection transport.
  The extraction item is therefore complete. Engine-owned `WmWorkspaceState` is
  *not* a second extraction: it lives in `wm_policy.rs`, already a separate module
  from the v7 one, and the public path's use of it is removed by the gated item in
  13.3 — "after the complete Hagia restart and last-layout gates" — not by lifting
  anything. `wm_policy.rs` does import `WM_API_VERSION` and v7 packet types, so it
  still has to be untangled before v7 is deleted, but that untangling is part of the
  gated removal and is deliberately not ahead of its gate.
- [ ] Freeze `sophia_wm_v1` and retain an archived revision-1 client only after
  the retained Triad behavior port is complete and the Rust reference, Hagia,
  X11 bridge, and C client pass the complete black-box reconnect/restart
  corpus. Do not remove API v7, declare stability, or create the permanent
  archived compatibility client before both conditions hold.
  The first condition is defined by Hagia's `docs/triad-port-ledger.md` at Triad
  baseline `fb8fb27e`; `docs/triad-port-ledger-pointer.md` locates it and
  summarizes its 27 retained rows. The shell and broker/portal tables are
  entirely open and are inside the gate, so the freeze is not near.
  Before it lands, settle the wire decisions enumerated in
  `docs/wm-v1-freeze-surface.md`. Twenty-three of the 27 rows need no wire change;
  the residue was workspace-name projection, broker classification shape, the
  continuous-pointer payload, and the output logical-space contract. Two of the four
  are now settled and normative in `docs/sophia-policy-ipc.md`. The output
  logical-space contract landed ahead of the output-authority tranche that would
  otherwise be the first thing tempted to widen `SnapshotOutput`.
  Broker classification shape is also settled, and it removed a pre-freeze
  obligation rather than satisfying one. Reading Triad's `WindowRule` at baseline
  `fb8fb27e` showed the rules are mostly parametric — a default workspace, a column
  proportion, a named scratchpad, a floating position — which no bitfield or enum
  carries, so the "small closed set in spare `capability_bits`" shape was never
  going to fit. Classifications instead ride a capability-gated extension chunk in
  the reserved `0xFF00`–`0xFFFF` range, which is uncounted and therefore costs no
  `*Begin` layout change. Nothing needs reserving in `SnapshotSurface`, and the
  classification vocabulary is no longer frozen with the revision. This option only
  became available when outbound gating landed and made clause 2 sound; the original
  analysis predates it.
  The generator now rejects any ordinary record declaring a kind in the reserved
  range, so what was a review-time rule about a number is checked.
  The last two are now settled too, both as recommended, so all four wire decisions
  are closed and none of them cost a layout change.
  Workspace names project as `ProjectionIndicator` labels: no field, no record, a
  hard 32-byte UTF-8 ceiling truncated on a character boundary, and a name is never
  an identity — activation stays on the action token so a label cannot become a
  namespace. This closes the naming half of its row; complete command parity is what
  keeps the row partial.
  The continuous-pointer payload fixes its vocabulary now and its behavior later.
  `PolicyInteractionKind` gains `Drag` and `Scroll`, and four values is the whole
  vocabulary; the payload rides the existing `interaction_*` fields with the axis
  discriminant in the former `reserved_cause` slot, now named `interaction_axis`,
  scroll using the coordinate pair as its delta and leaving the size fields zero.
  `PolicyInteractionPhase` needs nothing — all
  four phases are already wire-reachable and only `End` is ever constructed. The
  coalescing rule and `Cancel`'s revocation semantics are behavior rather than
  layout, and both stay gated on the lock and security-authority epoch barrier: a
  guessed revocation contract would be worse than a late one.
  What remains pre-freeze from this analysis is implementation, not decisions. The binding
  constraint is server-to-client enum vocabularies, not record kinds: an uncounted
  extension chunk in reserved kinds `0xFF00`–`0xFFFF` stays available after the
  freeze, but enum values sit at fixed offsets inside fixed-width records where no
  side channel reaches them.
- [x] Decide the `sophia_wm_v1` forward-compatibility rule. The three-clause form
  is recorded normatively in `docs/sophia-policy-ipc.md` under Versioning: the
  frozen revision is final for record layouts and enum vocabularies; new WM-side
  facts arrive as capability-gated extension chunks in the reserved kind range;
  new authorities take new interface families. Receivers keep rejecting unknown
  kinds because gating guarantees they are never sent one they did not negotiate.
- [x] Build outbound capability gating before the freeze. It is a prerequisite of
  the rule above, not an optimization: without it a frozen client can be sent
  content it must reject. `encode_wm_v1_policy_snapshot` now takes the selected
  capability set and omits governed record kinds along with their declared counts,
  so the transfer stays self-consistent; scene outputs and surfaces stay ungated as
  core semantics. The production caller passes what negotiation actually selected.
  Two tests hold the line, both in `crates/sophia-protocol/tests/policy_semantics.rs`:
  a producer pin asserting that enabling a capability leaves ungated chunk ordinals,
  kinds, item counts, and bytes byte-identical, and one binding the corpora to the
  default set plus each capability in isolation. Clause 2 of the forward-compatibility
  rule is sound as a result: an extension chunk in the reserved kind range reaches
  only a client that negotiated it. Enum widening remains now-or-never, because no
  gate reaches a value at a fixed offset inside a record already being sent.

Milestone 13 exits only when the public wire is independently implementable,
the retained Triad behavior port is complete across the correct authorities,
the formal and deterministic gates pass, installed Hagia uses the Engine
projection path, and a policy crash or replacement preserves the last coherent
desktop.

---

## Milestone 14: Native Graphics Efficiency

This milestone starts after installed Hagia is the usable ordinary session and
the revision-1 compatibility gate passes. It does not wait on elapsed dogfood
time. It optimizes the same native-X product; XLibre, Xorg, niri, river, and
other mature compositors are references rather than Sophia runtime components.

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

These are ordered product capabilities. They do not block Milestone 13's freeze
unless the retained Triad port ledger names the same behavior as a retained row,
or a named failure promotes one. Check the ledger before assuming an item here
is post-freeze work: several rows under Native Sophia Follow-Ups and Status,
Launcher, And Shell Integration are pre-freeze port requirements.

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
  its bounded physical scenario corpus on one immutable candidate. Require
  exact action and pointer commits, correct Kitty, Firefox,
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
  - [x] Introduce output-local pointer coordinate domains and per-output
    presented interaction epochs rather than merging independently retired
    heads into one global projection.
  - [x] Advance Sophia `SurfaceId` generations when a client reuses an XID and
    retire the exact frontend route on successful surface removal. Frozen or
    deferred generation-N input cannot resolve to generation N+1.
  - [x] Isolate a non-reading X client's private input queue. Saturation now
    removes that endpoint's sender set, rejects tracked delivery, and leaves
    the shared frontend broker available to healthy clients.
  - [x] Revalidate deferred pointer-focus handoffs before release. Every exact
    generational target must remain in the last-presented input projection and
    frontend route table; otherwise the complete buffered sequence is dropped.
  - [x] Preserve ordered client keyboard input across asynchronous focus
    acknowledgement without retargeting it. Reserved controls resolve first;
    the remaining keys retain their exact seat, generational surface, order,
    and libinput timing in a capacity- and timeout-bounded handoff. Focus,
    target, topology, or security invalidation drops the complete sequence.
  - [x] Turn ordinary and passive-grab pointer sequences into exact
    Engine-visible profile-scoped leases with ordered release acknowledgement.
    VT and seat transitions advance a shared epoch, clear frontend active
    ownership, and reject queued or frozen old-epoch input without waiting.
  - [ ] Reduce client-initiated explicit `GrabPointer` and admitted XI grab
    requests into the same Engine lease handshake, then bind lock and future
    security-authority takeover to the established epoch barrier.
- [ ] Add bounded policy interactions for move, resize, drag, and scrolling.
  Engine owns hit-testing, grabs, raw physical input, cursor state, and
  animation; Hagia receives only opaque targets and reduced geometry updates.
  Revision 3 now permanently fixes `Drag = 3`, `Scroll = 4`, and the in-place
  `interaction_axis` values (`None = 0`, `Horizontal = 1`, `Vertical = 2`) in
  Rust, generated C, and Hagia's independent Nim decoder. The codecs reject
  ambiguous geometry/axis combinations; live Begin/Update coalescing, End, and
  security-epoch Cancel behavior remain open, so this row is intentionally not
  complete.
- [ ] Model and publish `sophia_shell_v1` through the same formal, schema, C
  client, and permanent-compatibility process. Keep its endpoint and
  capabilities separate from `sophia_wm_v1`. Begin experimental modeling and
  the Hagia Shell port before the WM freeze so retained workflows can falsify
  both contracts. Derive the vocabulary from a driving client with a retained
  scene graph rather than from first principles; see
  `docs/sophia-shell-v1-direction.md`.
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
  multi-output hotplug gate. `tools/output_topology_physical_gate.sh` now arms
  that exact two-output loss/return procedure and requires two input-epoch
  barriers, generation-advancing complete publications, policy settlement,
  later page flips, client survival, and clean final topology health. It has
  not been run or promoted as evidence.

---

## Secondary Development Tooling

Interactive QEMU is useful for reproduction but is not a physical daily-driver
blocker. Work on it only when it shortens an active milestone.

- [ ] Fix the load-sensitive flake in `sophia-x-authority`'s `x11_wire` suite.
  Diagnosed, not fixed. It is **not** a timeout: under a parallel build,
  `routed_service_confines_input_and_control_to_two_workers_and_drains` fails at
  `socket_observation.rs:710` with `BadWindow` (3) where `BadAccess` (10) is
  expected, and `routed_lifecycle_events_follow_structure_and_substructure_masks`
  and `configured_present_child_receives_xlibre_ordered_geometry_notification`
  fail the same way intermittently. The shape is a cross-client race: the first
  client writes four requests and never reads, so nothing establishes that the
  server processed its `CreateWindow` before a second client refers to that window.
  The obvious fix is wrong. Adding a round-trip barrier on the first client — a
  request against an absent resource, whose error reply proves everything earlier
  was processed — makes the test fail **deterministically** rather than fixing it.
  So per-connection request ordering is not the whole mechanism, and the routed
  two-worker path or the confined-namespace boundary between the two clients is
  involved. That is where the next attempt should start, and it is worth more than
  the failed patch, which was reverted.
  A second mechanism is now recorded, and it rules out the tempting fix.
  `configured_present_child_receives_xlibre_ordered_geometry_notification` fails
  under full-workspace load inside `read_x_reply`: the reply's 32-byte prefix
  arrives and the body never does, until the **10-second** `SOCKET_IO_TIMEOUT`
  expires. Raising timeouts is therefore not the answer — ten seconds is already
  generous, and a reply that is half-sent for ten seconds is a server withholding a
  body, not a machine that was briefly busy. Both mechanisms point at the same
  place: what the routed workers do when more than one client is live.
  Note also that a failure here truncates the workspace run, because cargo stops
  before the remaining test binaries. A full-suite total that drops by roughly
  thirty-six tests is this flake, not a missing suite.
  A third attempt narrowed the mechanism to `read_x_reply`
  (`tests/x11_wire/support_extensions.rs`) and then failed too, which is the most
  useful thing recorded here. That helper reads 32 bytes and derives a body length
  from bytes 4..8 whatever the record is. Only a reply has a body: an error's bytes
  4..8 are its offending resource id, and an event has none at all. So a non-reply
  record yields a nonsense length and a read that blocks for the full timeout.
  What makes it stubborn is that the two failing tests **depend on that mis-parse**.
  Instrumenting the helper to reject non-replies showed both reading Sophia Present
  **event type 35** through `read_x_reply` on *every* run, not only under load — one
  call site even names the result `present`. The mis-parse is load-bearing: those
  events carry zero in bytes 4..8, so the bogus length is zero and the helper
  happens to return the event intact. Returning non-reply records whole, which is
  what the wire actually says, also broke both tests deterministically, so they rely
  on more than the zero-length coincidence.
  Two conclusions. The fix is **not local to the helper** — those two tests must be
  rewritten alongside it, which needs someone to work out what they intend to assert
  about Present events versus replies. And raising timeouts remains wrong: the
  records arrive promptly, they are simply parsed as the wrong kind.
  Both attempted fixes were reverted. Baseline is 178 passing.
  A suite that fails for non-reasons erodes every other claim in this file, so this
  is worth closing even though it is not on the critical path.
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
