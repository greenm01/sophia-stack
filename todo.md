# Sophia Active Roadmap

This file is Sophia's compact execution queue. It contains current truth,
ordered gates, and admitted parallel debt. The detailed pre-cleanup roadmap is
preserved verbatim in
[`docs/roadmap-archive-2026-08-30.md`](docs/roadmap-archive-2026-08-30.md).

## AI Operating Contract

Use this file as a decision surface, not as a project diary.

1. Read the applicable Sophia `AGENTS.md` instructions, `README.md`, and
   `docs/README.md` before acting.
2. Normative architecture and role specifications own behavior. This file owns
   execution order and may not override those contracts.
3. Take the first unchecked item under **Critical Path** unless the user names
   another scope. A later row never justifies bypassing an earlier gate.
4. `Parallel` work may proceed only when it does not delay, weaken, or silently
   broaden the critical path. `Candidate` work must be promoted here before
   implementation. `Deferred` work is out of scope.
5. A checked historical row is evidence, not work. Detailed diagnoses, failed
   attempts, and measurements belong in `docs/research-log.md`; completed
   milestone summaries belong in `docs/roadmap-history.md`.
6. Keep this file short. When a gate closes, record its durable result and
   evidence elsewhere, remove its implementation narrative here, and expose
   the next measurable gate.

Status vocabulary:

| Status | Meaning |
| --- | --- |
| `NOW` | First incomplete critical-path gate |
| `NEXT` | Ordered behind `NOW` |
| `PARALLEL` | Admitted non-blocking production debt |
| `CANDIDATE` | Retained but unprioritized; do not start implicitly |
| `DEFERRED` | Explicitly outside the current product path |

## Product Invariants

- The native Sophia X Server Frontend is the application authority. XLibre and
  Wayland implementations are references, not runtime dependencies.
- Engine owns physical input, focus authority, scene state, chrome,
  transactions, rendering, presentation, and scanout.
- X Authority owns X11 semantics and private client resources; it lowers pixels
  and opaque policy facts into Engine.
- WMs remain blind to XIDs, namespace IDs, titles, classes, PIDs, executable
  paths, and portal payloads.
- `sophia_wm_v1`, `sophia_shell_v1`, and `sophia_output_v1` are separate
  authority endpoints in one language-neutral protocol family. Hagia is the
  first proof-of-concept WM and shell client, not a protocol.
- Public role protocols carry bounded semantic policy, never shader programs.
  Engine retains compositor authority and any private renderer-provider seam.
- QEMU may prove deterministic protocol and policy behavior. Only physical
  evidence may prove DRM, input-device, VT, display-manager, latency, or visible
  pixels.

## Current Product State

- Milestones 9 through 12 are archived compatibility evidence. Milestone 13's
  installed native policy path is complete.
- Hagia is the ordinary installed native session. The retained Triad behavior
  port is complete, `sophia_wm_v1` interface major 1 / wire revision 3 is
  frozen, and Engine-owned API v7 workspace policy is gone.
- Hagia and Narthex speak the Sophia WM and shell roles directly. Sophia ships
  no legacy-X11-WM bridge or compatibility policy profile; existing WMs must be
  ported to the language-neutral protocols.
- The common native protocol-family contract is ratified. The role-by-role
  lifecycle audit and one family-level conformance entry point are not complete.
- `sophia_shell_v1` revision 1 remains experimental. Its descriptor switcher,
  protected launch, capability path, reconnect behavior, C proof, and Hagia
  shell client exist; permanent compatibility, the complete independent
  lifecycle proof, reservation coordination, and signed installed evidence
  remain open.
- Milestone 14 has promoted generational three-slot presentation, bounded
  buffer-age damage, refresh-relative input latency, one shared renderer worker
  per DRM group, direct scanout, return to composition on overlay/effect
  activation, direct-versus-composed measurements, and the atomic cursor path.
  Continuous software-content presentation is closed by one signed physical
  machine-and-visual pass. The comparison capture/replay contract is now
  teardown-aware and separates stack from workload cost. Its latest owner-only
  run failed closed before row 1 when an output-policy commit dropped the
  selected atomic cursor plane from reconstructed KMS head state. The next
  zero-row run proved that repair physically: both heads presented and the
  cursor crossed between them. Withdrawal of the qualification window then
  exposed a stale focused-surface identity in the public policy snapshot. The
  producer, wire codec, and Engine validation now enforce one complete-snapshot
  focus invariant. The subsequent zero-row `cp14-schema4-d0b10a2c` attempt is
  retained as a partial launch diagnostic. Manual observation also exposed two
  presentation gaps: authority turns could compose outside a refresh deadline,
  and the cursor used a Sophia-only bitmap. The first corrections added
  deadline coalescing and one configurable CPU/KMS cursor asset, but follow-up
  observation still found mouse-dependent Kitty cadence. The remaining cause
  was protocol and KMS ordering: X Present reported transaction IDs as MSCs,
  and input could issue a blocking cursor-only atomic commit before ready
  Present feedback. Signed candidate `07effa0a` then completed a fresh physical
  60-second Sophia workload with continuous focused visibility and 3,600
  contiguous kernel frames, but the row failed after sampling when capture
  attempted to re-read now-missing live-session qualification evidence.
  Qualification is now admitted before timed capture and nested gate output
  is durable. The next run exposed the underlying shell bug without starting a
  workload: a `0/4` qualification timeout continued into capture because the
  caller's status guard disabled Bash's implicit `errexit` inside the helper.
  Each prerequisite now returns explicitly on failure. Run
  `cp14-schema4-401d2b68` then passed the four-target qualification and retained
  a healthy 60-second measurement, but failed closed before finalization when
  two accepted X clients outlived the workload launcher and session quiescence
  timed out. Private workload process groups removed one survivor on signed
  candidate `69520f50`, and valid ImageText8 eliminated all protocol errors, but
  one connection still outlived the consistently observed three-process Kitty
  tree. The conformance owner now retains exact PID/start identities from its
  existing sampler and drains those processes as well as their original group.
  A new signed physical run is required. No comparison evidence is promotable
  yet.

Latest retained Milestone 14 evidence:

| Slice | Retained result |
| --- | --- |
| Frame slots | signed native archive `0001` |
| Buffer-age damage | signed native archive `0002` |
| Input latency | physical run `20260828T231430Z`: p99 24 ms / 34 ms budget over 245 presses |
| Shared renderer | signed native archive `0003`: one worker, zero misroutes |
| Direct scanout | archives `0001`–`0003`: eligibility, effect fallback, and same-session cost |
| Cursor | archives `0004`–`0006` plus continuous shakedown: 57.97 fps, p95 16.687 ms |
| Stable X backing | physical terminal run: 63/64 patches, 2 COW splits, registry peak 1 buffer |
| CPU continuity | signed run `20260902T002500Z` on `b9f0735a`: 7,116 accepted updates accounted, 1,190 presented, 5,926 superseded, zero pending, 16.586 ms maximum source gap, 18.825 ms maximum display gap, and 31.737 ms maximum update-to-retirement latency |
| Comparison acquisition | legacy `cp14` remains paused after 15 biased rows. `cp14-schema4-tools` reached 2/36 but both rows are diagnostic only. Owner-only runs remain at 0/36 while preserving successive cursor-plane, stale-focus, cadence, qualification, and teardown diagnostics. `cp14-schema4-69520f50` passed 4/4 cursor targets and retained a complete 60-second row, but one X client survived. The successor `cp14-schema4-94bf507e` then safely refused before display takeover because the launcher resolved the repository-local `cargo xtask` alias from the TTY login directory. The launcher now accepts the gate's absolute xtask and has a manifest-rooted standalone fallback; a fresh signed run must prove both that correction and exact PID/start workload teardown. Zero comparison results are promotable |

Promotion does not imply default enablement. Damage-limited repaint is now the
default, with `SOPHIA_ENABLE_BUFFER_AGE_DAMAGE=0` as the opt-out; its
pixel-equivalence proof runs in `cargo xtask check`. Shared rendering and direct
scanout remain opt-in in ordinary sessions, each owing its own promotion
decision. The atomic cursor is preferred, with `--legacy-cursor` and the startup
probe preserving the ioctl fallback. Verify current code and packaged policy
before changing any product default.

## Critical Path

### CP-14.2 — Same-hardware comparison (`NOW`)

- [ ] Run identical Kitty, Firefox, resize, and launch-burst workloads against
  Sophia, XLibre+xmonad, and a mature Wayland compositor on the same hardware.
  Run the separate Sophia two-hour soak only when overnight durability evidence
  is useful; it is optional and non-blocking.

Required exit:

- pin executable/configuration identities, topology, refresh, workload, sample
  windows, and raw evidence;
- report resource, frame-time, latency, allocation, and failure populations
  without converting reference results into Sophia correctness thresholds; and
- classify the comparison as diagnostic. Sophia's absolute correctness,
  authority, and refresh-relative latency gates remain authoritative.

The acquisition contract is implemented under
`cargo xtask conformance desktop-comparison`. A clean signed preparation
detects host identity and hashes the descriptors, isolated profiles, Firefox
fixture/profile, tracefs adapter, all stack-launch adapters, and the six stack,
policy, and shell executables. `gate` is the single TTY3 row owner: it
revalidates the clean prepared checkout and release
build before takeover, chooses the typed next stack, launches no operator
application, keeps the controller outside the measured supervisor tree,
resolves DP-1's active CRTC, and owns capture plus teardown. `attest`,
`preflight`, `qualify`, `capture`, `finalize`, `replay`, `verify`, and `report`
remain separately callable diagnostics.

Each attempt retains exact raw visibility, split resource, deduplicated
kernel-frame, workload, native-timing, and post-teardown attempt records plus a
derived schema-4 result and internal ledger. The first Sophia row also requires
an excluded four-target physical cursor qualification. Replay requires an empty
application baseline; a capture-owned, focused, visible DP-1 toplevel with zero
foreign application toplevels at settlement and every sample; uniform
60-second short windows; 120 resize observations; contiguous/monotonic
populations; zero crash/loss; and clean teardown. The optional soak lane
independently requires a full two-hour sample.
Correlation consumes PID/start identity only inside trusted conformance code
and persists no application identity. Partial attempts block progress only
within their own run. Regression coverage includes ready-but-hidden and
foreign-window rejection, legacy-run refusal, raw replay/archive integrity,
matrix/order mutation, kernel normalization, owner-only modes, tracefs probe
records, isolated Kitty configuration, and bounded runtime socket paths.
Reports retain resource, allocation, latency, and frame distributions with
`verdict=none`.

The first physical attempt failed closed before row 1 and is documented in
`docs/research-log.md`. A later run on signed candidate `00deb788` sealed the
first 15 rows, but the operator did not see Firefox during Sophia row 15.
Investigation found that the Sophia launcher keeps the Kitty used to invoke
capture inside the measured supervisor tree, while Hagia preserves focus on
that terminal and can leave the owned workload off-screen. Reference sessions
have no equivalent client. Readiness and DRM-vblank evidence do not prove a
visible workload, so Sophia rows 1, 6, 8, 10, and 15 are biased and the complete
prefix is non-promotable. Acquisition is paused before row 16.

Implementation and recovery hardening are complete; remaining critical-path
work:

- [x] replace Sophia's operator-terminal acquisition with a terminal-free
  session and a capture controller outside the measured supervisor tree;
- [x] fail closed unless trusted passive observation binds the capture-owned
  workload to focused, visible DP-1 placement without disclosing application
  identity to the blind WM, with hidden/foreign negative regressions;
- [x] commit and sign the corrected candidate, then prepare a fresh run using
  the already provisioned pinned XLibre prefix and isolated reference profiles;
- [x] accept Sophia's connector-neutral RandR names and harden teardown so a
  greeter is activated only after both the origin and manager TTY input states
  are restored and verified, with a text-TTY fallback and persistent handoff
  record;
- [x] reproduce the schema-4 failure with the isolated real `xrandr` client,
  implement the advertised read-only `GetPanning`, `GetCrtcTransform`, and
  `GetCrtcGamma` requests, and make X protocol errors terminate topology
  admission immediately with preserved diagnostics;
- [x] make greetd recovery attributable and layered: verify exact captured state
  before restart, fall back to a verified safe text-console baseline if exact
  kernel round-tripping diverges, then require stable text display, a
  non-disabled keyboard mode, readable termios, and a live tuigreet on the
  configured VT before activation;
- [x] sign the protocol and recovery corrections as `d5a1f7da` and prepare
  `cp14-schema4-randr` against that exact candidate;
- [x] stop after its first Sophia-row attempt and inspect the zero-row result:
  topology and attestation passed, recovery safely established greetd on tty7,
  and capture aborted before creating an attempt because Firefox had upgraded
  from the pinned 154 to 155;
- [x] update the Firefox comparison pin and move exact Kitty, Firefox, and niri
  version admission into both preparation and the pre-takeover gate, retaining
  capture-time revalidation against upgrades during a run;
- [x] sign the version-admission correction, prepare `cp14-schema4-tools`, and
  stop after the first Sophia and XLibre rows for inspection;
- [x] diagnose the two-row discrepancy: the atomic cursor accepted pending
  positions without a guaranteed post-retirement commit, XMonad self-replaced
  through a missing isolated cache executable, duplicated DRM deliveries
  inflated X timing, and capture claimed clean teardown before teardown ran;
- [x] implement a topology-wide latest-wins atomic cursor owner with idle
  cursor-only progress, combined-commit retry, hard-rejection legacy fallback,
  bounded counters, and truthful queued-versus-visible reporting;
- [x] make comparison capture stage before teardown, finalize only after the
  exact supervisor exits, keep required component identities live throughout
  sampling, split stack/workload/aggregate resources, deduplicate kernel
  sequences, preserve nested gate diagnostics, and add the excluded cursor
  qualification;
- [x] land the cursor and evidence corrections in a clean signed candidate;
- [x] make the prepared comparison root and its identity/checksum records
  owner-only independent of umask, and reject later ownership or mode drift;
- [x] stop after the first owner-only Sophia attempt and diagnose its bounded
  runtime failure: candidate topology reconstruction retained the fixed
  connector, CRTC, and primary plane but discarded the discovered cursor
  plane, invalidating the already-selected atomic cursor path after commit;
- [x] preserve the cursor plane across output-policy candidate and rollback
  selections and cover that KMS-route invariant through the public topology
  planner;
- [x] stop after the next zero-row attempt and diagnose its bounded failure:
  cursor qualification proved two-head atomic motion, then withdrawing its
  final window left committed focus naming a surface omitted from the next
  complete public snapshot;
- [x] sanitize snapshot focus from the same live surface set and reject stale,
  cross-output, non-focusable, or minimized focus at both the protocol codec
  and Engine authority boundaries;
- [x] decouple primary content cadence from input turns with one
  refresh-relative latest-wins deadline and cover still-versus-moving input
  schedules deterministically;
- [x] replace the renderer-private cursor bitmap with one bounded immutable
  Engine asset, configurable standard Xcursor lookup, validated hotspot and
  static-frame handling, and the canonical X11 core `left_ptr` fallback;
- [x] pin the comparison's Sophia core profile and canonical cursor digest,
  materialize the same pixels as an owner-only Xcursor theme for niri, and
  select XLibre's matching core cursor without reading personal configuration;
- [x] diagnose the remaining pointer/cadence coupling: DMA-BUF Present used
  global request transaction IDs as its MSC, while cursor-only atomic commits
  could block ahead of ready Present feedback;
- [x] route physical KMS `(ust, msc)` through GPU and software Present
  completion, make transaction IDs correlation-only, give primary submission
  and feedback priority over cursor-only DRM service, preserve a superseding
  cursor cell, and keep hardware-cursor pixels out of native CPU repaints;
- [x] inspect the resulting signed physical Sophia attempt: the workload stayed
  focused and visible for 60/60 samples with 3,600 contiguous 60 Hz kernel
  frames, but late re-reading of volatile cursor qualification prevented
  `measurement.kdl` and retained the row as a partial diagnostic;
- [x] admit and snapshot live-session qualification before creating a partial
  or starting the timed workload, and preserve each nested conformance result
  in the durable TTY gate log;
- [x] reproduce the missing-qualification path without consuming a capture:
  the window mapped and routed pointer motion, but timed out at 0/4 targets;
  make the shell helper return explicitly after failed attestation or
  qualification even when its caller condition disables implicit `errexit`;
- [ ] prepare a fresh interactive run and inspect its physical cursor
  qualification plus the first Sophia, XLibre, and niri rows before
  continuing. Candidate `69520f50` passed qualification, captured a healthy
  first Sophia measurement, and eliminated qualification protocol errors, but
  did not seal it because one accepted X client survived process-group cleanup.
  Exact sampled PID/start identity retention and bounded group-plus-process
  teardown now need a fresh signed candidate and prepared run;
- [ ] run the unified one-row TTY3 gate for all 36 required rows on this
  machine; and
- [ ] retain and verify the complete interactive matrix. A separate one-row
  Sophia two-hour soak remains optional overnight evidence and does not block
  this gate or CP-14.3.

### CP-14.3 — Close Milestone 14 (`NEXT`)

- [ ] Verify the milestone exit, archive the concise result in
  `docs/roadmap-history.md`, and update every affected current/target statement.

Milestone 14 exits only with bounded warmed resource counts, no steady-state
allocation growth, refresh-relative latency evidence, clean normal teardown,
and no change to Sophia's native-X authority model.

The current-soak verifier remains available for optional overnight durability
evidence. It requires a nonsaturated five-second resource series, at least 120
contiguous samples, and flat settled peaks with zero tolerance for accounted
resources; the native sampler holds 1,560 samples, covering two hours plus ten
minutes without saturation. Historical installed archives explicitly use the
archive policy and remain reproducible. A fresh two-hour run is useful but does
not block Milestone 14 closure.

### CP-15.1 — Native protocol-family lifecycle audit (`NEXT`)

- [ ] Audit `sophia_wm_v1`, `sophia_shell_v1`, and `sophia_output_v1` against
  `docs/sophia-policy-ipc.md`.

Required exit:

- align hello/welcome negotiation, effective bounds, capabilities, epochs,
  transaction identity, complete transfers, outcomes, recovery, and extension
  handling;
- document every intentional role-specific difference in its role contract;
  and
- remove or explicitly version accidental transport forks without weakening
  the frozen WM revision.

### CP-15.2 — One family-level conformance surface (`NEXT`)

- [ ] Add one canonical conformance entry point that invokes every role's
  retained valid, malformed, codec, and lifecycle corpus.

Required exit:

- a contributor can validate the family without discovering role-specific
  scripts or treating generators and Rust crates as the specification;
- every stable role retains an immutable old-client compatibility gate;
- every stable role has a complete non-Rust lifecycle client implemented from
  normative prose and checked-in schemas; and
- shell stabilization specifically retains the independent C proof and Hagia's
  independent Nim proof without linking Sophia crates or generated bindings.

### Post-CP-15.2 planning checkpoint

Do not silently choose a broad shell, effects, portals, or compatibility tranche
before the two protocol-coherence gates close. Promote one bounded product slice
from the candidate queue below, give it measurable exit criteria, and keep
authority-specific services separate. The likely product sequence is a minimal
stable shell lifecycle, security takeover and trusted services, confined
application/portal workflows, then optional visual-effect vocabulary; retained
evidence may change that order.

## Parallel Production Readiness

These rows do not reorder the critical path.

- [ ] Repair the evidence readers still pinned below their emitter. Ten accept
  `sophia_live_session status=bounded_complete` at schema 15 or lower against an
  emitter that writes 16, and nine accept `sophia_live_wm status=ready` at
  schema 1 against an emitter that writes 4. These are retired-policy physical and
  QEMU gates; they fail loudly rather than silently, so each needs a per-gate
  decision about whether it still earns its keep. Add each repaired record to
  `tools/check_live_record_schema_readers.sh` once its emitters are confirmed to
  agree.
- [ ] Decide whether `run_frame_fed_output_gate_tty4.sh` and
  `run_current_critical_path_tty4.sh` keep requiring HEAD to equal the locally
  known origin/master. The direct-scanout and Hagia native runners no longer do;
  `package_live_session.sh` keeps it deliberately, because packaging is the
  publishing question the rule was wrong about being.
- [ ] Move remaining session-private test modules out of production `src` as
  visibility boundaries permit, and split the oversized cohesive units named in
  `docs/source-layout-debt.txt`. Do not weaken privacy or add test-only
  production APIs.
- [ ] Reduce `tools/start_sophia_tty3.sh` to the minimum TTY/display-manager
  adapter around `sophia session run`. Typed parsing, verification, archive
  handling, and gate orchestration stay in Rust.
- [ ] Repair the load-sensitive `sophia-x-authority` `x11_wire` flake. Rewrite
  the affected tests together with `read_x_reply`: it currently treats Present
  event type 35 as a reply and interprets bytes 4..8 as a body length. Raising
  the ten-second timeout is not a fix. Preserve the 178-test baseline while
  making record-kind parsing explicit.

Completed infrastructure baseline: `sophia-session` owns production lifecycle,
`sophia-conformance` owns development-only evidence logic, `cargo xtask` is the
canonical developer/CI surface, `just` is optional human shorthand, canonical
installed commands live under `sophia session`, and source-layout debt is an
exact identity ledger.

## Candidate Queue

These are retained product debts, not an invitation to work out of order. Before
starting one, move it into the critical path or an explicitly admitted parallel
tranche with a named driver and exit gate.

### Authority and lifecycle hardening

- Stamp geometry, interaction, pointer-handoff, and WM-policy work queued across
  an output-topology transition; revalidate it against typed authority epochs.
- Bound the page-flip callback read without losing retirement, then close the
  broker swallow points, unbounded input delivery, route-lease send, and
  config-bound publication silent drops. Evidence gates must assert zero
  discards.
- Decide whether blind spatial/output roles require Bubblewrap protection by
  default and define fail-closed behavior on hosts without `bwrap`.
- Bind lock and future security-authority takeover to the existing input epoch
  barrier.

### Native WM and shell product

- Finish issuer-scoped action-capability validation and the atomic
  shell-reservation/work-area/WM coordinator.
- Stabilize the minimum `sophia_shell_v1` lifecycle only after CP-15.1 and
  CP-15.2; require signed installed `hagia-shell` evidence and preserve metadata
  separation from the blind WM.
- Add bounded target-resolved move, resize, drag, and scrolling interactions.
- Add trusted launch-placement provenance, output-scoped active workspaces, and
  only evidence-backed metadata-free native tabs.
- Define a bounded redacted workspace/layout/focus status feed and opaque
  launcher action; add lock, screenshot, wallpaper, and audio through their
  owning shell/session capabilities.

### Portals and confined applications

- Promote a confined daily-driver group only after Kitty and Firefox pass their
  grant and recovery gates.
- Add evidence-driven X11 `INCR`, Xdnd, URI/file launch, prompts,
  notifications, and capture/FD handoff through portals.

### Compositor graphics and effects

- Before broadening the shell schema with effects, model capability admission,
  bounded parameters, supersession, Engine-clock cancellation, provider
  absence/failure, deterministic fallback, and atomic multi-head presentation.
- Implement one protocol-neutral Engine effect registry and private
  build-linked provider seam. Prove one scene-sampling effect and one
  Engine-clocked transition, including damage/pixel gates and direct-scanout
  fallback. Do not expose shader programs or reopen frozen WM revision 3.
- Settle remaining display-list vocabulary from a driving client: generic
  target regions, desktop background, and only measured additions beyond
  client-rasterized textures.
- Retain only measured rendering follow-ups: cross-drawable `CopyArea`, bounded
  raster storage, upscale filtering, linear-light blend/opacity, mirror remode,
  presented-extent raster demand, CPU GBM pooling, configurable semantic cursor
  themes (theme, nominal size, named shapes, hotspots, and deterministic
  fallback), concurrent producers, and equal-mode scanout cloning. Comparison
  profiles must pin one cursor theme and size across Sophia and references.

### Compatibility and diagnostics

- Keep XLibre+xmonad only as a direct external desktop-comparison reference;
  it never runs as Sophia policy and never connects to a Sophia endpoint.
- Retain physical `glxgears`, pc105 keyboard, exhaustive X11 reservation,
  Chromium fixture, and human-visible QEMU/RFB proofs only when they cover a
  promoted product change.

## Deferred

- XLibre provider integration until a measured native-X gap justifies its
  authority and maintenance cost.
- Any new application protocol or compatibility frontend without a
  specification amendment backed by named product evidence.
- VRR until physical hardware reports `vrr_capable=1`.
- General X11 conformance not required by a retained daily-driver client.
- Runtime effect plug-ins or a sandboxed effect host until the private
  build-linked provider proves a need and a safe lifecycle.

## Definition of Done

For any promoted row:

1. preserve the authority, metadata-disclosure, passive-data, and protocol
   boundaries above;
2. model temporal or ownership changes before implementation and keep meaningful
   negative controls;
3. add deterministic regressions outside production `src` where visibility
   permits;
4. run `cargo xtask check` for code changes and the named physical gate for
   hardware claims; docs-only edits require inspection and `git diff --check`;
5. retain exact source, binary, configuration, topology, and evidence identity
   for promotion claims; and
6. update this file, `docs/roadmap-history.md`, affected current/target sections,
   and `docs/research-log.md` so they agree.

Canonical tooling and command ownership are defined in
[`docs/development-tooling.md`](docs/development-tooling.md). Validation gates
are indexed in [`docs/validation.md`](docs/validation.md).

## Archive Map

- [`docs/roadmap-archive-2026-08-30.md`](docs/roadmap-archive-2026-08-30.md):
  verbatim 2,736-line roadmap before this cleanup, including completed rows,
  investigation narratives, exact failed attempts, and superseded details.
- [`docs/roadmap-history.md`](docs/roadmap-history.md): completed milestone
  summaries.
- [`docs/research-log.md`](docs/research-log.md): active decisions, diagnoses,
  and retained evidence.
- [`docs/research-log-archive.md`](docs/research-log-archive.md): superseded
  research material.
