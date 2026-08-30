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
- Hagia speaks the Sophia WM and shell roles directly; the xmonad bridge is a
  compatibility oracle and regression profile, not the product path.
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
  Continuous software-content presentation and the comparison/soak contracts
  are implemented; their fresh physical evidence remains.

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
| CPU continuity | sustained real-xterm regression: 541 runtime commits and CPU buffers, no X error |

Promotion does not imply default enablement. Damage-limited repaint is now the
default, with `SOPHIA_ENABLE_BUFFER_AGE_DAMAGE=0` as the opt-out; its
pixel-equivalence proof runs in `cargo xtask check`. Shared rendering and direct
scanout remain opt-in in ordinary sessions, each owing its own promotion
decision. The atomic cursor is preferred, with `--legacy-cursor` and the startup
probe preserving the ioctl fallback. Verify current code and packaged policy
before changing any product default.

## Critical Path

### CP-14.1 — Stable X backing and continuous presentation (`NOW`)

- [ ] Replace full immutable CPU presentation replacement for stable
  software-rendered X toplevels with lease-safe copy-on-write backing, and prove
  sustained post-readiness content reaches physical retirement.

Backing implementation, formal proof, and hardware mechanism evidence are
complete. `StableBackingLease.tla` carries six negative controls; the backing is
`Arc`-backed copy-on-write end to end; over-bound damage coalesces; and derived
density variants preserve patches. The retained terminal run reported 63 of 64
updates as patches, two COW splits, and one resident registry buffer.

That run also exposed a pre-existing frozen-output path. Successful core X
`CopyArea` omitted the required `NoExpose`, so xterm waited after its first
scroll. X Authority now emits `NoExpose` according to the GC's
`graphics_exposures` flag, and the sustained real-xterm regression reaches 541
runtime commits and CPU buffers without an X error. The live session now emits
bounded post-readiness intake, composition, exact primary-retirement, latest-
wins accounting, cadence, and latency evidence. The schema-4 terminal reporter
refuses startup-only progress, gaps over one second, fewer than three changed
retirements, unaccounted updates, or update-to-retirement latency beyond two
refresh periods.

Required exit:

- preserve child composition and exact admission extents;
- keep historical handles immutable until their last presentation lease
  retires;
- bound generations, backing storage, damage history, and fallback behavior;
- model the lifetime transition before changing it and retain every
  implementation-relevant counterexample as a deterministic regression;
- prove no warmed steady-state allocation growth on the retained workload;
- account every accepted post-readiness CPU update as presented, superseded, or
  pending, with no pending update at completion; and
- show sustained source and physical-retirement progress at refresh-relative
  latency on the physical terminal workload.

Two physical attempts on signed commit `32b555b6` ended before the visual
prompt on the host's known lost-vblank path: first head 1 after 307 retirements,
then head 2 after 41. Both had an empty, routed, clean poller while the peer head
continued. The runner now retains and retries only that attributed
below-process signature within a bounded budget. Outstanding: rerun the guarded
TTY3 terminal gate, confirm that xterm scrolls continuously, and retain its
passing schema-4 report. Do not close this row from the offline real-xterm
regression or either failed physical attempt.

### CP-14.2 — Same-hardware comparison (`NEXT`)

- [ ] Run identical Kitty, Firefox, resize, launch-burst, and soak workloads
  against Sophia, XLibre+xmonad, and a mature Wayland compositor on the same
  hardware.

Required exit:

- pin executable/configuration identities, topology, refresh, workload, sample
  windows, and raw evidence;
- report resource, frame-time, latency, allocation, and failure populations
  without converting reference results into Sophia correctness thresholds; and
- classify the comparison as diagnostic. Sophia's absolute correctness,
  authority, and refresh-relative latency gates remain authoritative.

The typed orchestration is complete under
`cargo xtask conformance desktop-comparison`: preparation requires a clean,
signed Sophia candidate and pins repository inputs, binaries, topology, kernel,
Mesa, and GPU; ingestion binds checksummed raw logs to a rotated schedule; and
verification requires all 39 samples. The matrix is twelve short samples per
stack plus one two-hour soak per stack. Relative results always report
`verdict=none`. Outstanding: run and retain the physical matrix on one machine.

### CP-14.3 — Close Milestone 14 (`NEXT`)

- [ ] Verify the milestone exit, archive the concise result in
  `docs/roadmap-history.md`, and update every affected current/target statement.

Milestone 14 exits only with bounded warmed resource counts, no steady-state
allocation growth, refresh-relative latency evidence, clean normal teardown,
and no change to Sophia's native-X authority model.

The current-soak verifier now requires a nonsaturated five-second resource
series, at least 120 contiguous samples, and flat settled peaks with zero
tolerance for accounted resources. Historical installed archives explicitly use
the archive policy and remain reproducible. A fresh two-hour current run remains
required; no fixture or historical archive closes this row.

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
  schema 1 against an emitter that writes 4. These are xmonad-era physical and
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

- Stamp geometry, interaction, pointer-handoff, and legacy-WM work queued across
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
  presented-extent raster demand, CPU GBM pooling, cursor images, concurrent
  producers, and equal-mode scanout cloning.

### Compatibility and diagnostics

- Keep xmonad as a regression oracle. Repair stable-relayout silence and add
  other classical-WM profiles only from named versions and retained workflows;
  no profile may proxy real X Authority metadata.
- Retain physical `glxgears`, pc105 keyboard, exhaustive xmobar reservation,
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
