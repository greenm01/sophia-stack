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
7. A usage failure produces a diagnosis, a focused regression where feasible,
   a correction, and revalidation of affected behavior. Insert a blocking repair
   into the active stage; do not restart a comparison campaign or reset unrelated
   completed work. Retain exact candidate identities and review evidence impact.

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
  authority endpoints in one language-neutral protocol family. Hagia is the WM
  reference client; Narthex is the independent shell reference client.
- Public role protocols carry bounded semantic policy, never shader programs.
  Engine retains compositor authority and any private renderer-provider seam.
- Shell IPC is independent of application frontends and UI toolkits. Quickshell
  is a downstream content reference; it adds no Qt dependency to Sophia.
  Narthex remains the independent descriptor reference.
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
- `sophia_shell_v1` revision 2 is experimental, with revision-1 clients still
  supported. Narthex implements switcher, bounded reservation, and persistent
  tab descriptors; Sophia owns geometry, rendering, input, and presentation.
  The family audit, stabilization, and current signed physical tab acceptance
  remain open. Hagia's native trees and tab protocol pass offline verification.
- Milestone 14 has retained physical evidence for frame slots, buffer-age damage,
  input latency, shared rendering, direct scanout, cursor motion, and continuous
  software-content presentation. Candidate `2823807e` passed the short Firefox
  canary with changing nonblack pixels, exact retirement, and clean logout.
- Comparison run `cp14-schema4-251d9acd` sealed nine Kitty rows; Firefox row 10
  measured successfully but failed final validation after VT resume reset native
  counters. Suspension also delays the runtime deadline. These lifecycle gaps
  are the first task in CP-14.3; the comparison remains deferred at 9/36. See the
  [diagnosis](docs/research-log.md#2026-09-04-cp14-row-10-exposes-scanout-evidence-lifetime-and-suspended-deadline-gaps).
- The current target is a recoverable Hagia development session, evaluated by
  real workflows and targeted checks. Milestone 14 is not complete; neither a
  new comparison matrix nor a clean-day timer is a prerequisite for this work.

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
| Comparison acquisition | `cp14-schema4-251d9acd` sealed 9/36 Kitty rows; Firefox row 10 measured 60 focused/visible samples and 3,600 kernel frames, then failed native completion after VT resume reset counters. The partial blocks its own run; CP-14.2 remains deferred and incomplete. |

Promotion does not imply default enablement. Damage-limited repaint is now the
default, with `SOPHIA_ENABLE_BUFFER_AGE_DAMAGE=0` as the opt-out; its
pixel-equivalence proof runs in `cargo xtask check`. Shared rendering and direct
scanout remain opt-in in ordinary sessions, each owing its own promotion
decision. The atomic cursor is preferred, with `--legacy-cursor` and the startup
probe preserving the ioctl fallback. Verify current code and packaged policy
before changing any product default.

## Critical Path

### CP-14.3 — Development-session readiness and Milestone 14 closure (`NOW`)

Milestone 14 now targets a reliable personal development session: terminal,
Firefox, clipboard, layouts/tabs, two monitors, dependable input, recovery, and
logout. Readiness is demonstrated through working tasks and resolved failures,
with no consecutive-clean-day requirement. CP-14.2 is explicitly deferred and
incomplete; neither its 36-row matrix nor the optional soak gates this milestone.
The [decision and former queue](docs/research-log.md#2026-09-04-milestone-14-retargeted-to-development-session-readiness)
retain the change in exit criteria. Work through these stages in order.

- [ ] Accept [desktop composition](docs/desktop-composition.md) in one normal
  session: select the desktop from `sophia/desktop.kdl`, retain the current
  Hagia/Narthex/Quickshell arrangement, and confirm WM reload does not replay
  login applications. Native shortcut help was reported working in the
  installed session on 2026-09-06; see the research log.

- [x] Implement the native application launcher through the generic revision-4
  shell protocol: session-owned catalog and execution policy, Engine-owned
  input/GPU presentation, and independent Narthex search. Hagia exposes only
  session operation slot 7. See [application launcher](docs/application-launcher.md).
- [ ] Accept the installed Sophia/Hagia/Narthex launcher changes with
  Super+Space through normal use: search, keyboard/click activation, Escape,
  terminal entries and opening a third window. The desktop-composition and
  native-launcher configuration is installed. The first login failed argument
  validation because the selected `brave-origin` browser was unregistered; its
  explicit core registration now passes installed-session validation. The next
  login exposed duplicate nominal DRM modes in the output projection; the
  projection and full-modeline selection fixes are installed. That release
  presented both heads, then rejected frontend publication because the authority
  snapshot lacks backend timing metadata. Release `3fc0ab14` completed installed
  login on 2026-09-06: both outputs committed, startup ready, zero recovery
  attempts, and revision-4 shell connected. Super+Space dispatched Ghostty;
  Ghostty then failed during its MIT-SHM mask upload. The upload repair passes
  an isolated Ghostty startup probe; installed interaction acceptance remains
  pending. No live reload was performed.
- [ ] Accept the client-failure repairs in normal use: Ghostty opens from the
  launcher, and a Brave typing test leaves the desktop usable. Packed MIT-SHM
  uploads are repaired. Normal sessions contain rejected/failed input deliveries
  while proof sessions remain strict. The original Brave `RouteRejected` reason
  is unresolved; new diagnostics must guide any follow-up. See the
  [diagnosis](docs/research-log.md#2026-09-06--ghostty-mask-uploads-and-browser-input-failure-containment).

#### 1. Recover reliably

- [x] Preserve session-wide native evidence across scanout replacement,
  retaining earlier failures and each owner's drain obligations.
- [x] Honor runtime deadlines while the seat is suspended, without requiring
  reacquisition to begin shutdown.
- [x] Add regressions for retirement before replacement, repeated resume,
  immediate shutdown after resume, rejected VT switches, topology replacement,
  suspension across the deadline, and failure retention. Lifecycle model and
  negative controls pass; `cargo xtask check` passes (2,348 test executions).
- [x] Complete the physical suspended-deadline canary. The fixed candidate's
  retry retained 3,425 native retirements, shut down while suspended at its
  90-second deadline, drained in 539 ms with zero pending work, and restored
  fallback input with exit status 0. The operator reports it seems to work.
- [x] Complete the [physical VT-return canary](docs/native-recovery-canary.md).
  Two returns opened epochs 2 and 3 with 229 and 19 subsequent retirements;
  session totals retained all 275 retirements across three settled owners.
  Browser launch and logout were accepted after resume. Normal logout drained
  in 46 ms with zero pending work, native failures, or emergency recovery;
  fallback input was restored and exit status was 0. The operator reports done.
  The Firefox deduplicated-GPU routing regression is fixed; the isolated full
  gate passes 2,350 test executions. Both recovery canaries now pass.

Stage 1 is complete. Next: establish the normal live session in stage 2.

Exit: recovery and bounded shutdown pass without contradictory counters,
abandoned accepted work, or loss of the fallback desktop. The
[row-10 diagnosis](docs/research-log.md#2026-09-04-cp14-row-10-exposes-scanout-evidence-lifetime-and-suspended-deadline-gaps)
is the starting evidence; no new comparison run is required.

#### 2. Establish the live session

- [x] Allow local session installation from signed local Hagia commits without
  requiring a push or `origin/master` equality. Retain source signature,
  committed-profile, manifest identity and artifact verification checks.
- [x] Add the [opt-in Quickshell X11 panel](docs/quickshell-x11-panel.md) and
  `cargo xtask panel` launcher with explicit GPU/software selection and retained
  binary identity. Isolated normal-session CPU content, popup update/withdrawal,
  work-area release/reacquisition and clean normal-exit teardown pass.
  Runtime profile activation and stale resize/work-area startup races repaired.
  `cargo xtask check` passes (2,430 Rust test executions).
- [x] Honor the desktop startup list in the ordinary Hagia launcher, retaining
  explicit CLI overrides and a terminal fallback. Register the opt-in panel in
  the trusted core application registry; keep process launch out of Hagia.
- [ ] Revalidate the third-terminal crash repair: `4eb1136a` physically showed
  both panels, kept terminal bounds correct and admitted new terminals, then
  failed when a repaint had no staged Present image. The lowered-frame guard
  now skips invisible candidates while preserving the old-area repaint. Accept
  terminal insertion and scrolling out of view/back on the rebuilt session;
  [diagnosis and retained evidence](docs/research-log.md#2026-09-06-third-terminal-crash-after-successful-panel-presentation).
- [ ] Confirm shell-owned panel startup: the automatic Tier-0 bar, fixed top
  reservation and hit targets are removed. Quickshell is the selected panel;
  Narthex's switcher and explicit tab descriptors remain enabled. Check one bar
  per output and full work-area restoration when the panel stops.
- [ ] Accept this panel in normal GPU use on both outputs: pointer hit targets,
  popout anchoring, focus, stop/relaunch and coexistence with Narthex. Native
  shell content and DMS remain deferred; this is an X11 compatibility client.
- [x] Confirm the panel popout visibility repair in normal GPU use. On installed
  Sophia `05ef0eb8` / Hagia `12f7493`, the operator confirms the incrementer
  works; the live renderer samples the 240x112 popup on output 1. Evidence:
  `/tmp/sophia-panel-camera-confirmed-05ef0eb8`. Both-output and lifecycle
  acceptance remain in the broader panel row above.
- [ ] Diagnose the forced-deadline control drain race captured in
  `/tmp/sophia-panel-probe-v5` (12 dispatched, 11 delivered, one pending).
  Normal-exit probe success does not waive this separate shutdown finding.

- [ ] Accept the scrolling repair in normal use: three Kitty windows, reversal,
  insertion/close, vertical scrolling and both outputs. The implementation adds
  committed Hagia camera anchors and Engine GPU translation; see the
  [contract and focused check](docs/window-transitions.md). Broader physical
  acceptance remains pending; new-window camera following is confirmed below.
- [x] Confirm that opening new windows in the scrolling layout moves the camera
  to them. The operator confirms this on Sophia `05ef0eb8` / Hagia `12f7493`;
  logs show three added terminals receiving focus with committed layout moves.
  Second-output placement, vertical scrolling and close behavior retain their
  separate acceptance scope.
- [ ] Revalidate installed startup after the output-ownership repair. Release
  `0.1.0-86b5fe1d20bc` exited on the first Kitty Present after losing its output
  assignment. The operator entered installed fix `84c109c6`; logs confirm two
  outputs settled, continuing GPU Presents, and no startup fatal error.
  Visible both-output behavior and normal logout still need acceptance. See the
  [diagnosis](docs/research-log.md#2026-09-05-installed-startup-loses-the-first-windows-output-ownership).
- [x] Delegate WM policy-setting validation to Hagia. Sophia preserves bounded
  ordered policy fragments; Hagia validates values before activation. The
  default and personal Hagia profiles pass paired offline checks without edits.
  TTY startup checks both owners before display-manager takeover. Cross-repository
  regression coverage includes repeated workspace records and rejection before
  acknowledgement; runtime/installed acceptance still follows below.
- [ ] Reuse the existing launcher and installed session path; identify the exact
  Sophia/Hagia/Narthex binaries and profiles, retain a known working fallback,
  and document start, normal logout, emergency escape, and rollback.
  Release `0.1.0-417e97d2e25b` is packaged with Hagia `38ea8da` and checked
  executable/profile digests. The personal profile's two legacy application IDs
  were corrected with operator approval; paired parser checks and complete
  session-argument preflight pass. That release was installed and started,
  which surfaced two session-ending defects now fixed in `e18beede`: a pointer
  preference a device could not hold failed the whole seat, and the requested
  output topology was validated and then never applied. Release
  `0.1.0-e18beede1831` packages those with Hagia `5662d43`, whose profile
  validation now refuses a trigger Sophia cannot bind to a keycode — the second
  defect that ended a login, and one `hagia config check` had been calling
  valid. Installation of that release awaits local sudo authentication.
- [ ] Pass a short physical acceptance check: start normally, launch terminal
  and Firefox, type and change focus, resize, use both outputs, return from a
  VT, and log out cleanly. Include a basic tab-layout interaction.

Exit: the operator can enter and leave a normal development session without a
comparison controller or benchmark workload. This stage establishes pilot use,
not milestone completion; fuller workflow acceptance follows in stage 4.

#### 3. Make failures diagnosable

- [ ] Reuse existing telemetry for identifiable per-session logs and bounded
  resource observations; preserve diagnostics after abnormal exit.
- [ ] Provide a simple incident-time marker and document how to find the
  matching build, configuration, session, and surrounding events. Keep expensive
  tracing and pixel inspection opt-in; retain metadata-disclosure boundaries.

Exit: a reported problem can be investigated without reproducing it merely to
recover an overwritten log. Extend existing session/tooling owners rather than
building a separate monitoring platform.

#### 4. Exercise real development workflows

- [ ] Use terminal editing/building and Firefox for real work; verify clipboard
  transfer in both directions, keyboard shortcuts, focus, resize/move, workspace
  navigation, and both monitors.
- [ ] Complete the [physical tab acceptance](docs/tabbed-layouts.md#verification-and-operator-acceptance)
  for frame-tree/Notion and i3/split-tree: empty/nested groups, hidden-member
  activation, shell recovery, title changes, fullscreen, and floating occlusion.
  Implementation and offline verification are already complete.
- [ ] Observe idle, VT resume, and normal logout during actual use. Turn any
  blocking failure into the next concrete repair task within this stage; retain
  its evidence, add a focused regression where feasible, and recheck the affected
  workflow after correction.

Exit: every listed workflow has retained observations, with no unresolved
failures preventing those tasks or safe recovery. Unrecoverable sessions, lost
input, application-blocking failures, visible corruption, undrained work, and
unbounded resource growth are blockers. Longer sessions are useful evidence,
not mandatory hour/day counters that restart after every fix.

#### 5. Close Milestone 14

- [ ] Review workflow evidence, recovery, clean teardown, bounded warmed
  resources/no steady-state allocation growth, and relevant refresh-relative
  latency evidence. Record the accepted candidate, limitations, and why earlier
  evidence still applies or which affected checks were rerun.
- [ ] Archive the concise result in `docs/roadmap-history.md` and update current
  product statements only after the development-session target is demonstrated.

Exit: the declared workflow is usable and recoverable with no unresolved
blockers, and Sophia's native-X authority model is preserved. Comparison and
optional soak evidence remain separately labelled incomplete and non-blocking.
Lock, audio controls, screenshots, notifications, conferencing, broader confined
workflows, and effects are outside this first target; promote them when actual
use establishes the need. Targeted protocol repairs remain part of these stages;
the full family audit follows below and is not a prerequisite for pilot use.

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
- shell stabilization specifically retains the independent C proof and Narthex's
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

- [x] Shell reference preparation: generic boundary documented, Quickshell fork
  and `sophia` branch established, Void baseline built, and
  [panel/popout requirements and results retained](docs/shell-reference-client-audit.md).
  Eight of nine test suites passed; the existing popup-movement failure is
  downstream baseline debt. This completes preparation, not content support.
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

- [ ] Implement `RENDER`. Both Quickshell and Brave ask for it and are refused:
  `sophia_x11_authority_extension schema=1 status=absent name="RENDER"`. It is
  the antialiased-text and image-compositing path every toolkit reaches for
  first, so a client refused it falls back to core drawing and looks wrong or
  feels slow rather than failing outright. The largest of these by a wide
  margin, and the one worth doing properly.
- [ ] Implement `SHAPE`. Quickshell asks for it in the same trace. Small: a
  handful of requests for non-rectangular window regions, and Sophia already
  carries region machinery for XFIXES.
- [x] Implement `XC-MISC`, before something needs it. It is how Xlib recycles
  XIDs once a client exhausts its range, and the client that hits it is a
  browser left open for days. Nothing has asked yet, which is exactly why this
  is cheap now and an incident later: the failure mode is the client dying
  rather than degrading. Done: grants draw from the same counter that connection
  setup draws from, so a grant cannot collide with a range a client already
  holds, and both refuse rather than overlap once the pool is spent. See the
  `XC-MISC identifier ranges` matrix row.
- [ ] Decide, rather than implement, `Composite`, `DAMAGE`, `XTEST` and `DPMS`.
  Each is a domain Sophia owns -- compositing, input, power -- and a client
  reaching through one of them is asking to step around that authority. They
  belong in the matrix as deliberate exclusions or as admitted surface, not as
  gaps that stayed open because the list looked incomplete.

Every row above came from measurement rather than a survey of what a server
usually has. `QueryExtension` now records what it refuses, so the next live
session extends this list by observation; the four decisions above should be
revisited against a week of real logs rather than against this paragraph.

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

- Before admitting a content prototype, characterize Quickshell's retained
  popup-movement failure with an isolated display backend. After the CP-15
  coherence gates, separately admit one panel/popout workflow and require an
  independent C content client against the same public shell contract. The
  [audit](docs/shell-reference-client-audit.md) owns requirements; this does not
  add a frontend, toolkit dependency, content wire, or live-session change.
- [x] Implement the bounded session-owned [control v1](docs/sophia-control-v1.md)
  endpoint and `sophia msg`: startup-only `session.control "host-admin"`, disabled
  by default, socket-derived pidfd and user/mount/PID namespace admission,
  bounded worker, exact catalog/action correlation, Engine-settled policy
  actions, and asynchronous WM restart confirmed by replacement commit.
  Generic WM/shell authority boundaries and their existing wires are preserved.
- [ ] Optional short installed control smoke: discover, invoke a safe registered
  action, restart WM, and observe continued input/rendering. Automated endpoint
  and supervised-owner evidence is separate; do not restart the 36-row gate.
- [ ] Repair transactional profile reload before advertising `reload-profile`.
  Keep shell commands, delegated grants, parameters, queries, and subscriptions
  behind separately specified contracts. Linux admission does not attest
  arbitrary third-party sandboxes sharing the host namespaces.
- Harden the implemented issuer-scoped action checks and reservation/work-area
  coordination only against a named remaining lifecycle gap; preserve existing
  offline conformance and distinguish it from signed physical acceptance.
- Stabilize the minimum `sophia_shell_v1` lifecycle only after CP-15.1 and
  CP-15.2; require signed installed Narthex evidence and preserve metadata
  separation from the blind WM.
- Add bounded target-resolved move, resize, drag, and scrolling interactions.
- Extend launch-placement or output-scoped workspace policy only for a named
  unmet workflow; opaque launch provenance and active-output selection already
  exist. Native tab implementation is complete; acceptance belongs to CP-14.3.
- Expose per-head scanout framebuffer handles from `sophia-backend-live` so a
  requested output topology can be applied inside a live session.
  `compose_from_current_framebuffer` in `desktop_output_heads.rs` reads the
  CRTC's current framebuffer, which is correct for a standalone command that
  composes nothing and empty inside a session, whose atomic commits leave the
  legacy field unset. A head's own submissions are `pub(crate)`, so this is new
  public surface. It is followed by an unanswered question: whether an atomic
  modeset is safe while the session owns and is flipping those planes.
  `apply_requested_native_output_topology` already runs after startup presents
  and declines cleanly, so the gap reads in every session log as
  `sophia_live_native_topology_apply status=declined reason=heads`.
  What it costs on this seat: DP-1 is a DELL S3222DGM whose EDID offers
  2560x1440 at 120Hz, the profile requests exactly that, and the session runs
  it at 60. DP-2 is a DELL P2319H that tops out at 1920x1080 at 60, so it is
  already at its best and only DP-1 is waiting on this.
- Define a bounded redacted workspace/layout/focus status feed. The native
  launcher action is implemented above; add lock, screenshot, wallpaper and
  audio through their owning shell/session capabilities. The wire is not what blocks these:
  `SnapshotSessionOperation` (record kind 4, max 256) already advertises
  every operation with its slot each snapshot, and a policy client resolves
  slot to operation and sends `SessionOperationRequest`, so no revision is
  needed. What is closed is this repository's own vocabulary — the
  variants of `DesktopSessionShortcut` in `crates/sophia-config/src/
  shortcut_candidate.rs` and `WmActionBehavior` in `.../types.rs` — plus each
  behavior's implementation, lock's being a security transition rather than a
  launch. A policy client's side is one profile-whitelist string and one
  appended action per capability, so each operation defined here unblocks
  Hagia in a few lines.

### Developer outreach

- Publish `docs/building-on-sophia.md` as a public web page. Low priority until
  the project is ready to advertise; the checked-in document is authoritative
  and the page is a rendering of it, not a fork.

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

### CP-14.2 — Same-hardware comparison (`DEFERRED`)

- [ ] Complete and verify the 36-row Kitty/Firefox/resize/launch-burst matrix
  against Sophia, XLibre+xmonad, and niri only when the user selects a stable
  candidate or a named performance investigation. No automatic restart follows
  a development-session fix. The separate two-hour soak remains optional.

Run `cp14-schema4-251d9acd` retains nine sealed Kitty rows and a failed Firefox
row-10 partial. These remain exact-candidate historical evidence, not a complete
or passed comparison. A changed comparison candidate requires a fresh pinned
run when this work is resumed; old rows cannot be relabelled for newer binaries.

The [comparison contract](docs/validation.md#deferred-same-hardware-comparison)
and all existing tooling remain intact: clean signed preparation, executable
and configuration hashes, raw visibility/resources/kernel-frame evidence,
post-teardown sealing, exact complete matrix verification, and diagnostic
`verdict=none`. The partial still blocks only its own run. Deferral does not
waive validation or promote the failed attempt, and does not block CP-14.3.

### Other deferred work

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
   for promotion claims;
6. update this file, `docs/roadmap-history.md`, affected current/target sections,
   and `docs/research-log.md` so they agree;
7. run relevant independent client checks for protocol changes, and the short
   physical session acceptance check for startup/rendering/input/lifecycle
   changes, plus checks targeted to the defect. Documentation-only changes need
   inspection and whitespace/link checks, not another operator session; and
8. retain prior evidence under its original identity. Record a change-impact
   review before relying on it for a newer candidate, and rerun affected
   workflows rather than treating unrelated changes as a reset of all progress.

Normal usage complements deterministic tests. Existing architecture, ownership
models, compatibility gates, and physical proof requirements remain binding.
See [development-session validation](docs/validation.md#development-session-readiness)
for the evidence boundary.

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
