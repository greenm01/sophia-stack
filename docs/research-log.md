# Active Research Log

This file records decisions and unresolved questions for the active milestone.
Completed evidence is archived in `research-log-archive.md`.

## 2026-08-16: unjournaled Present invalidates density replay

- The first run after head-affine image realization completed candidate and
  rollback preparation, applied all three heads atomically, presented both
  logical outputs, and committed topology epoch 2. It then terminated X
  Authority while reconciling the new physical-density demand with `failed to
  satisfy surface raster requirements: InvalidResource`.
- Standard pixmap and DRI3 Present replace surface pixels without an X drawing
  command that the semantic raster journal can replay. The old path left any
  earlier journal live. After a presentation resize, density satisfaction saw
  the old journal extent and elevated that private mismatch to a process-fatal
  resource error; at an unchanged extent it could instead replay stale commands
  and falsely label the result as an authority raster.
- Every unjournaled Present now invalidates the surface's replay state and
  derived variants at the actual presentation extent. Later density demand
  returns the existing bounded `unsupported_command` sampled fallback, while a
  stale journal extent returns `logical_extent_mismatch`; neither kills the X
  server. A later full opaque semantic baseline may recover replay. This does
  not weaken the extended-output proof: a client-rendered DMA-BUF at that
  head's exact native extent remains exact without derived raster replay.

## 2026-08-16: retained renderer images are head-affine topology resources

- The next three-head run crossed policy admission, output-owner
  authorization, quiescence, and candidate-frame lowering, then failed before
  KMS with head 3's worker reporting `InvalidTarget`. The candidate frame had a
  valid 1920x1080 native target; its Kitty layers named renderer-image IDs that
  existed only in head 1's persistent renderer store. A retained image ID is a
  semantic identity, not proof that every physical renderer already owns its
  pixels. Head 3 had previously displayed an empty output and therefore could
  not resolve those IDs when it joined head 1's mirror group.
- Candidate and rollback frame admission now derives a unique retained-image
  requirement set per opaque physical head. While frame ownership is quiescent
  and before any candidate worker submission, native topology preparation
  accepts an already-promoted local copy, promotes a staged local copy, or
  exports a compositor-owned DMA-BUF snapshot from another live head and
  restores it into the target head's renderer store. Missing donors and failed
  restoration reject preparation with zero KMS submits. The scene remains an
  Engine plan; this is renderer-resource realization, not policy geometry or a
  cross-head scanout-buffer lease.
- The failed worker took several owner turns to drain after abort and the owner
  logged the unchanged `Aborting` report on every turn, producing a 1.2 MiB
  incident log. Preparation progress telemetry is now emitted only when phase
  or prepared counts change. Worker polling and affine cleanup remain
  nonblocking and unchanged.

## 2026-08-16: mixed topology preparation belongs to the session owner

- The first three-head mixed-output run reached two admitted Kitty surfaces but
  failed before any modeset. Two ordinary scene advances made the bundled Rust
  reference receive `RejectedStale`; it treated that expected generation race
  as a process error and was restarted twice. Unlike Hagia and the xmonad
  bridge, this reference is a pure snapshot-to-projection function and mutates
  no private model. It now returns to fresh snapshot intake on the same
  connection only for `RejectedStale`; all other noncommitted outcomes remain
  fatal, and stateful peers retain the restart rule recorded on August 5.
- The fatal third attempt exposed a separate Engine/backend contract mismatch.
  The canonical display list may name a policy-admitted surface before it has a
  committed pixel state, and Engine's output snapshot deliberately filters that
  command. Retained native composition instead required every such command to
  be committed and reported `retained head plan lost a displayed surface`.
  Retained source membership is now the intersection of policy presentation
  order and Engine's committed surface table. A committed displayed surface
  without an authority-owned CPU or renderer source still fails closed.
- Output preparation had also been delegated backward across the authority
  boundary. The IPC client submitted up to sixteen distinct transactions while
  the backend reported that ordinary frames still owned scanout resources.
  Only the session owner can observe and advance those resources. It now accepts
  one effect, quarantines new ordinary authority and policy intake at existing
  bounded queues, services native retirement, and begins renderer preparation
  only after a backend-owned quiescence predicate becomes true. The wait is
  bounded at two seconds; cancellation takes precedence, and timeout rejects
  without KMS mutation. The output client's single proposal has an eight-second
  socket deadline.
- The old evidence condition required active DP-2 content in the first topology
  frame. That ordering was impossible: blind policy sees three logical outputs
  before publication and can partition the two proof surfaces only after the
  mirror plus singleton becomes two logical outputs. The first DP-2 frame may
  therefore be empty, but must already use exact mapping with no sampling or
  fallback. Visual acceptance now requires a later exact active DP-2 plan after
  the `placement=1,1` marker, joined causally through frame queue, KMS submit,
  callback, and retirement.

## 2026-08-16: the mixed-topology proof has one dual-role reference client

The remaining physical cutover could not be driven by the static desktop
profile: that would establish the requested topology before the public output
role connected and therefore would not prove `sophia_output_v1`. The dormant
Rust reference policy now has a bounded live proof mode that owns both permitted
connections in one supervised process. Its policy side accepts the exact staged
profile, installs a valid empty configuration, and continues serving complete
snapshot/projection cycles. Its output side independently negotiates the
exclusive owner socket and submits the live candidate.

The output reducer accepts exactly three connected descriptors named by the
gate: two become one mirror group and the third remains an extended group to its
right. All three keep their advertised current modes and generations. A fourth
connected descriptor rejects the proof rather than being silently disabled.
Existing logical identities and mappings are preserved where meaningful, and
the candidate passes the same protocol validator before it reaches the socket.
After publication, the policy side partitions two proof surfaces by logical
output geometry alone. The connector labels used by the output role do not
inform that choice and never enter the blind policy snapshot.

The recovery-TTY runner records the signed source identity and both executable
hashes, requires exactly the three configured connectors, and waits for a
committed two-group output outcome plus clean session/topology health before it
admits a manual visible-pixel statement. It is deliberately not promotion
evidence yet. This host currently has only two connected physical heads, and
the retained archive verifier, head-loss/rollback phase, and physical
native-sharp observation still require a real three-head run.

## 2026-08-16: native startup now crosses a semantic prepare-all barrier

The initial empty desktop, first CPU authority cycle, explicit repaint, and
resume/recovery paths no longer modeset a projected flat raster and then replace
it with per-head work. Each validates and queues one complete
`HeadCompositionPlan` result per physical head, establishes every renderer
worker before export, prepares every framebuffer and modeset property owner
without KMS mutation, and admits the blocking card commit only when the worker
and prepared-owner sets exactly cover the required opaque heads. Mirror groups
remain card-local, so the first visible mutation is one atomic card-scoped
request rather than a sequential head prefix. Accepted owners are all adopted
before any bookkeeping error may return; rejection cancels prepared owners into
the retryable cleanup ledger. A failure before KMS also drains or discards every
queued renderer-worker command before clearing its passive content and damage
state, so startup cannot leave an affine renderer lease detached from rollback.

The physical mirror contract now rejects `direct_cpu` startup evidence. It
requires matching plan, queue, worker-composed, and synchronous-modeset records
for one shared semantic scene, plus the later plan/queue/KMS chain for the final
interactive frame. A pure barrier regression proves that missing workers,
missing prepared owners, prepared-before-worker state, and foreign heads cannot
reach KMS. Whole-output admission additionally rejects duplicate or incomplete
logical-output batches before any output runtime advances. Resume and recovery
restore retained renderer images before deriving the same per-head startup
transaction; there is no compatibility flat-baseline modeset left in the native
startup lifecycle.

## 2026-08-15: per-head frame identity now survives planning through KMS

The per-head lowerer produced the right native geometry, but its passive backend
envelope retained only the opaque head and logical checksum. A queued worker
result could therefore no longer prove which Engine scene generation, committed
target generation, or `Fit`/`Cover`/`Exact` mapping had produced its pixels.
That omission weakened stale-plan rejection precisely where an IPC topology
change may replace a target while older renderer work is still completing.

`LiveProductionHeadCompositionFrame` now carries the scene generation, target
generation, and protocol-neutral mapping in addition to head and logical
checksum. Admission validates the complete unique head set against the current
`HeadRenderTarget` set before mutating any exporter, and topology candidate and
rollback pools validate the same identity against their respective plan side.
Stable plan and queue evidence records preserve the chain without exposing DRM
identity. The mirror physical verifier accepts a generation only when each head
has one native-size plan, one matching queued frame, the expected sampling
density, and strict plan-before-queue-before-KMS ordering. Deterministic negative
fixtures cover missing heads, stale target generations, mapping/extent mismatch,
and late queue evidence.

## 2026-08-15: committed head mappings now drive every projection lane

The output IPC already committed `Fit`, `Cover`, or `Exact` independently on
each opaque head, but ordinary native composition still reconstructed every
`HeadRenderTarget` from one session-global `mirror_fit`. Cursor, flat fallback,
bootstrap, mixed, retained, and damage projection used the same hidden global.
An accepted runtime mapping change could therefore update authority state while
the displayed pixels continued using the startup policy.

The duplicate backend policy is removed. Initial desktop configuration is
normalized directly to protocol-neutral `OutputHeadMapping` when the native
owner is constructed; VT and hotplug reconstruction retain that initial policy.
After an output-authority commit, each live head's stored mapping is the single
source consumed by composition, damage, cursor, and fallback projection.
`HeadRenderTarget` projection also carries that head's actual target generation,
scale, refresh, and transform rather than hard-coded generation/transform facts.
Connector-neutral authority snapshots are patched transactionally from the
backend's opaque-head mapping table before publication, so missing coverage
cannot mutate a valid prefix.

Deterministic regressions prove distinct mappings and target generations on two
heads, rejection without partial snapshot mutation, and omission of disabled
heads from the render-target set. The 223-test libdrm feature suite and the
187-test CLI binary suite pass. Remaining architecture work is the native scene
bootstrap, authority-owned native-density variants for server-rendered content,
and signed physical mixed mirror-plus-extended evidence.

## 2026-08-15: ordinary presentation no longer has a primary-output composition lane

The live output runtime now retains the exact root-space logical viewport from
the output authority across both candidate installation and rollback. Ordinary
CPU, DMA-BUF, retained renderer-image, compositor-chrome, software Present, and
hardware-cursor work consumes that placement instead of reconstructing a
horizontal layout or treating every output as origin zero.

Engine owns two new pure facts at the cross-output boundary. The applicable
retirement set is the union of logical outputs intersecting the old and new
root-space surface geometry, so a move cannot update its destination while
leaving stale source pixels. `TransactionPresentationCohort` then joins those
independently submitted and retired logical outputs at the latest output UST;
the transaction generation is its logical sequence, rather than a fabricated
combination of CRTC-local MSCs. Output-local mirror cohorts continue to join
their own physical heads below this reducer.

Production DMA-BUF Present now builds all applicable output snapshots from one
prepared Engine candidate, resolves CPU/DMA-BUF/retained sources once, lowers
every head at native size, and retains one frame identity per logical output.
The client source becomes submitted only after every applicable output submits,
and feedback, source release, committed input geometry, and displayed-image
promotion wait for the final output retirement. Software Present uses the same
retained source-set lowerer and output join; the former synthetic secondary
marker frame is no longer a presentation path. Focus, order, chrome, outline,
resume, and cursor updates likewise cover the complete runtime output set.

Deterministic evidence includes the Engine cross-output move/retirement reducer,
asymmetric output submission and retirement in the Present scheduler, exact
root-space runtime replacement, opaque-head DMA-BUF duplication, retained-image
native geometry, and a cursor projected through negative/raised viewports. The
full backend, 187-test CLI binary, Engine, and renderer-live suites pass. The
remaining acceptance boundary is signed physical evidence for a mixed
mirror-plus-extended topology and native-density authority variants for client
content that can actually provide them.

## 2026-08-15: topology resources now join card-scoped apply requests

`HeadlessOutput` has no origin, so retaining only its extent during candidate
resolution erased the distinction between a real extended layout and several
overlapping outputs. Resolved candidates now retain ordered root-space logical
viewports. A read-only visual-runtime seam captures each viewport from one
committed surface slice, selects per-head content variants, and lowers native-
size frames; a deterministic spanning-surface test proves both halves of an
extended desktop are produced from the same scene generation.

Renderer topology preparation now uses blocking modeset policy and retains the
renderer owner, framebuffer/imports, and mode blob without submitting. Prepared
heads expose copied atomic contributions while keeping their resources affine.
The new card request builder combines independently buffered enabled heads and
explicit disabled heads, rejecting object overlap before adding properties. A
successful combined commit can adopt prepared owners into ordinary scanout
retirement; failed/unsubmitted work remains cancellable. The remaining live
transaction must prepare a rollback pool, submit cards, roll back prior cards on
failure, rebuild runtime targets, and wait for first presentation before publish.

## 2026-08-15: live output effects now resolve complete native head plans

Tracing the first post-cohort apply boundary found that an omitted connected head
was absent from `OutputTopologyTransaction`: the protocol correctly defines
omission as disable, but the reducer would have allowed publication without ever
preparing, applying, or rolling back that physical effect. Resolved candidates
now carry generation-stamped disabled heads alongside enabled targets, and the
transaction requires both sets.

Apply proposals now leave the WM-owned protocol service as immutable effect
contracts and enter the visual/session owner. The owner projects every opaque
head onto its existing card-qualified selection, resolves independently requested
modes through the live DRM master, preserves mirror/extended output membership,
and produces a passive plan without changing runtime or KMS state. Deterministic
coverage proves split and disable plans, incomplete coverage rejection, stale
generation rejection, and zero mutation of the published binding. The next
physical boundary is replacement target/resource preparation followed by
card-scoped apply and rollback.

## 2026-08-15: content crosses the authority boundary as a bounded variant set

- `SurfaceTransaction` no longer carries a bare `target_buffer` and
  `target_content_size`: it carries a `SurfaceContentSet` — a bounded,
  density-deduplicated list of `SurfaceContentVariant` records with its own
  named capacity (`MAX_SURFACE_CONTENT_VARIANTS`). Every current producer
  normalizes into a one-variant set at identity density; the committed state
  clones the whole set so no later stage can silently squash
  authority-asserted variants to the canonical raster.
- Set invariants are enforced at construction with private fields, so an
  empty, over-capacity, or duplicate-identity set is unrepresentable rather
  than validated downstream. Envelope identity is structural too: the set
  lives inside its transaction or committed state, so a variant cannot name a
  different surface or generation than its envelope. This replaces the
  reviewed plan's duplicated identity fields plus intake validation.
- Deliberately deferred until their consumers exist, per the
  production-reachability rule: per-variant damage and readiness (their
  consumer is the per-head damage ledger), a source-transform class (no
  producer can emit one), and a wire encoding for multi-variant sets — the
  authority socket still encodes the canonical extent and source, so the wire
  format is byte-identical and the frontend needs no version bump.
- Content generation is not a second clock: a new set is a new authority
  transaction, so `CommittedSurfaceState.committed_generation` is the set's
  generation identity.
- DMA-BUF Present pairing now ranges over every variant of a set: a DMA-BUF
  variant anywhere requires its exact Present pair even when the canonical
  variant is a CPU raster.

## 2026-08-15: the unequal-mirror sharp-text bar is sharp-not-blocky

- The first tty4 run on the head-identity commit proved the rekeyed path end
  to end (head-keyed readiness, joined generations, active sharp downscale,
  zero fallbacks) and failed only at the operator's visual confirmation: the
  1080p head's terminal text is soft. That softness is the mathematical floor
  of fractionally downscaling 1px bitmap glyphs, not a filter defect.
- Decision: the gate's `scaled_text=sharp_not_blocky` confirmation means what
  it says — no blocky resampling artifacts — and is the honest bar while
  client content is a single raster. Native-sharp client text on a 0.75x head
  is out of reach for sampling, for per-head composition (client rasters stay
  single-variant), and for integer-scale authority variants alike; it would
  require glyph rasterization at fractional density, which is deliberately
  not on the current roadmap. Operators who want pixel-exact text on the
  smaller head can configure a same-mode 1920x1080 mirror instead.

## 2026-08-15: RenderHeadId replaces raw connector identity above the backend

- Engine records no longer carry connector or CRTC integers. The DRM registry
  is replaced by `EngineHeadRegistry`: bounded `HeadRenderTarget` records
  grouped by logical output, keyed by an opaque session-scoped `RenderHeadId`
  the backend mints when it builds page-flip sessions. The backend retains the
  card/connector/CRTC/name mapping privately in
  `LiveProductionNativeHeadTable`; sysfs discovery moved with that identity
  into `sophia-backend-live`.
- Head targets are generation-stamped: readmitting a head with a different
  shape must advance `target_generation` or the registry rejects it as stale,
  which is the record-level footing for stale-plan rejection when per-head
  composition plans land.
- The logical view of an output is the shape all of its heads agree on, fail
  closed; refresh is deliberately excluded from that agreement because a
  mirror group's heads legitimately run near-but-not-equal rates. Logical
  pacing reduces to the slowest head's rate, matching the joined-retirement
  contract.
- Mirror lifecycles, page-flip routes, callbacks, kernel timestamps, and the
  per-head evidence lines are keyed by `RenderHeadId`. The one place a head id
  is printed beside its connector name is the readiness mapping line
  (`sophia_live_native_head schema=2 status=ready`), which is how physical
  verifiers correlate; every later per-head record carries only `head=`.
- A callback whose CRTC resolves to no admitted head fails closed with a named
  unknown-head error instead of being dropped or matched by output.

## 2026-08-15: per-head composition review fixes the prerequisite contracts

- Review of the multi-monitor architecture found two load-bearing divergences
  in `VisualRetirement.tla`: one global committed generation accidentally made
  unrelated outputs supersede each other, and no guard prevented one physical
  head from being in flight for two generations. The model now commits per
  output, prepares and submits per head, reserves overlapping output cohorts,
  and joins transaction feedback only after every required output retires.
- A deliberate premature-completion control initially passed: it settled the
  generation as superseded instead of emitting false feedback, exposing an
  invariant gap rather than a production defect. The final model separately
  requires every terminal candidate to settle every required output, so both
  false success and premature failure are rejected.
- Mirror completion evidence now distinguishes shared logical identity from
  optional physical pixels. A head reports scene generation and logical-content
  checksum; a native head-pixel checksum, when available, may differ. Unrelated
  outputs may legitimately show identical content, so checksum inequality is no
  longer treated as output identity.
- The reviewed architecture now states the current-to-target migrations rather
  than assuming them complete: raw connector/CRTC identities leave Engine,
  configured mirror fit is normalized into an Engine transform, startup obeys
  prepare-all, and final target/lease ownership remains permanently head-local.
  The appended review was folded into the normative sections instead of retained
  as a competing non-normative appendix.

## 2026-08-15: multi-monitor composition fans out before rasterization

- The mismatched-mode mirror proof established native scanout ownership and
  joined retirement but also made the remaining visual shortcut obvious. The
  production CPU path composes a 2560x1440 logical frame first and reduces that
  flat image for the 1920x1080 head. A sharper filter can preserve more contrast,
  but it cannot turn an output-level raster into native per-head composition.
- The ratified target is one immutable logical Engine scene with a distinct
  native composition for every physical head. Mirrored heads retain the same
  scene generation, surface set, geometry, stacking, focus, and logical
  retirement identity; their native targets, damage ledgers, selected client
  variants, framebuffers, KMS owners, and pixel results are intentionally
  distinct. Extended outputs use the same path and no longer inherit visual
  work from a distinguished primary output.
- The client-content limit is explicit. Engine-owned primitives can be
  rasterized at each head's density, but an arbitrary client buffer cannot be
  semantically recreated at another resolution. Protocol authorities may emit
  a bounded immutable set of raster variants for one surface generation.
  Engine selects the best ready variant per head; a singleton client buffer
  remains admissible through an observable resampling fallback. Engine never
  acquires permission to replay X11 drawing requests.
- Ownership and retirement do not change. The WM still sees one `OutputId` for
  a mirror group and no connector identity. Rendering prepares every required
  mirror head before the first per-head KMS submit, callbacks join on the
  logical scene generation, and head loss fails the candidate closed. The
  existing `VisualRetirement.tla` head layer was deliberately written over
  heads rather than shared buffers; it remains the base and must gain the
  prepare-all invariant before scheduler implementation.
- [Multi-Monitor Per-Head Composition](multi-monitor-composition.md) is the
  normative target. The existing flat-frame projection is retained as current
  implementation evidence, not described as compliant per-head rasterization.

## 2026-08-14: failed mirror proofs are diagnostics, not promotion evidence

- The physical mirror runner now preserves a strict evidence boundary. Only a
  successful session plus explicit pixel confirmation reaches the promotion
  archive; runtime and visual-confirmation failures go to a separate diagnostic
  archive and retain their original exit semantics.
- A failed record pins the same source, binary, and profile identity as the
  promotion proof, then adds the failing stage, exit, derived signal, and a
  bounded kernel-log delta. Kernel access is opportunistic and non-interactive:
  inability to read it is an explicit fact rather than a reason to prompt or to
  discard the rest of the failure evidence.
- Kernel deltas retain the newest 256 lines by default and record whether the
  snapshots were continuous, reset, or truncated. The archive verifier binds
  that metadata to the retained delta and rejects promotion markers, so a useful
  crash artifact cannot accidentally become proof that mirroring worked.

## 2026-08-14: mirror presentation is a joined physical-head generation

- The failed physical run disproved the primary-head shortcut. A mirror output now
  owns one scanout lane per connector: exporter, displayed and submitted buffers,
  cleanup, callback serial, and timing. The Engine advances once, physical heads
  submit independently at their native modes, and logical presentation becomes
  visible only after the connector set joins on one frame identity.
- The group is bounded to one active generation plus the exporters' newest pending
  frame. A partially submitted generation is poisoned: accepted KMS owners drain,
  no replacement generation is admitted, and no logical presentation is emitted.
  Sequential per-head commits remain deliberate; they follow the X-style decision
  recorded in `todo.md` and avoid unproven multi-CRTC event semantics.
- CPU mirror layers now share immutable source bytes. Retained CPU, DMA-BUF,
  renderer-image, solid, and cursor geometry is projected per head. Renderer-image
  suspend handoff also covers each physical exporter instead of only the primary.
- Mirror bootstrap uses head-sized CPU buffers before renderer workers start. This
  removes the mirror-specific inline-EGL-to-worker transition from the failed run.
  It is a code-side crash control, not proof that the AMDGPU rejection is fixed;
  that conclusion still requires a diagnostic-capable physical rerun.

## 2026-08-14: continuous geometry is latest-value, cancellation is ordered

- Engine's floating pointer capture now produces Begin, Update, and End for the
  public policy path while retaining the one-shot completion for API-v7 peers.
  Raw motion and client delivery remain outside the policy process.
- A queued Update replaces only the latest queued Update with the same opaque
  target and interaction kind. Begin, End, Cancel, actions, and unrelated
  targets retain order; the sixteen-request owner bound does not grow.
- Output-topology, VT, and seat security transitions clear Engine capture,
  remove its stale queued values, and prioritize a Cancel. Hagia accepts each
  continuous geometry phase and treats Cancel as a no-op, so revocation cannot
  accidentally commit the last sampled geometry.
- Policy restart increments a locally observed epoch before the next physical
  input drain; an active capture is cleared and a Cancel is prioritized on the
  fresh connection, so it cannot observe an orphan Update. This still does not
  claim the entire interaction row because drag and scroll have no live Engine
  producers.

## 2026-08-14: revision 3 spends its last pointer slot before the freeze

- The freeze analysis had already fixed four interaction kinds and four phases,
  but the code still exposed only move/resize and still rejected the proposed
  axis slot as reserved. Leaving the decision in prose would have made the frozen
  enum permanently smaller than the retained Triad input vocabulary.
- `PolicyInteractionKind` now fixes `Drag = 3` and `Scroll = 4`. The existing
  phases remain `Begin`, `Update`, `End`, and `Cancel`; a fifth kind or phase
  requires a new interface family after the freeze.
- `ProjectionRequest.reserved_cause` is consumed in place as
  `interaction_axis`: zero for move/resize/drag, one horizontal, two vertical.
  Scroll reuses X/Y as signed deltas and requires zero width/height. This changes
  no offset or payload size. The semantic packet carries the axis explicitly so
  Rust, C, and Nim clients agree without retaining an unnamed wire value.
- This entry closes only the irreversible vocabulary decision. The separate live
  move/resize coalescing and security-cancellation tranche is recorded above.

## 2026-08-12: The authority reduces; the broker never holds raw identity

- Four documents disagreed about who sanitizes client metadata, and the
  disagreement decided which process holds every client's title, class, and PID.
  `style-guide.md` said authorities emit sanitized candidates.
  `architecture.md`, `sophia-x-authority.md`, and `compositor-graphics.md` gave
  sanitization to the metadata broker, making raw identity a broker *input*.
- I resolved that inconsistency in the wrong direction first. Seeing three docs
  against one, I edited the style guide to match the majority and recorded it as
  putting the boundary "in one place". Counting documents is not the same as
  reading the design, and the majority was wrong.
- **The code held the better answer.** `XMetadataPropertyCandidate`
  (`sophia-x-authority/src/property.rs:74`) carries the property's name, type, and
  `byte_len` — never its `bytes` — and a test named
  `x11_property_records_emit_metadata_candidates_without_raw_payloads` pins that.
  Someone had already decided raw payloads do not leave the authority, and none of
  the prose said so.
- **Decision: the authority reduces.** The broker publishes a
  `MetadataDisclosureRule` per surface; each authority applies it to text it
  already legitimately holds and emits a bounded label. Raw identity exists in
  exactly one process. The broker keeps what only it can decide — disclosure
  policy, trust assignment, icon tokens, and cross-authority aggregation — because
  those are shared facts and an authority deciding them alone would let two
  authorities disagree about what a user is looking at.
- The argument that lost is worth recording because it is the attractive one:
  centralizing sanitization gives one sanitizer, one policy, one place to audit.
  It also gives one component holding every client's identity across every
  authority, and one more copy of each secret crossing a process boundary to get
  there. Least privilege wins because the thing being centralized — policy — can be
  centralized *without* centralizing the data it governs.
- Distributed reduction costs N implementations of truncate-and-validate. That is
  mechanical, shares one bound and one validator, and is a far smaller drift risk
  than N implementations of policy would have been.
- Lesson worth keeping: when documents disagree, the code may already encode the
  decision, and it is cheaper to look than to take a vote. Four documents now say
  the same thing, and they say it because one test did.

## 2026-08-12: Mirror groups are configurable before they are drivable

- The configuration half of mirroring landed ahead of the scanout half, which
  raises the question of what a `mirror` directive should do while nothing can
  drive it. It is validated fully and then refused. Accepting and ignoring it would
  leave an operator staring at an unmirrored screen with no error to search for,
  which is the worst of the three options; refusing without validating would hide
  configuration mistakes until the day the feature arrives.
- Validation is split by what each layer can answer, which kept each rule where its
  inputs are. Parsing rejects self-reference, repeats, emptiness, and the bound —
  true whatever hardware is attached. The candidate as a whole rejects one connector
  claimed by two logical outputs, the only rule needing more than one output in
  view, and the only arrangement that would make "one logical output backed by N
  connectors" untrue. The topology rejects unknown, disconnected, or mode-mismatched
  members.
- Same-mode is enforced at reconcile because no plane scaling exists on this path.
  The alternative to refusing is letterboxing a screen the operator asked to match,
  and silently changing what a display shows is not a fallback.
- Ordering the errors mattered more than expected. An impossible request and an
  unimplemented one send an operator to different places, so the topology checks run
  before the unsupported refusal. A single "not supported" for both would have
  buried real configuration errors until the feature shipped.
- The mode is resolved through the primary's own `resolve_mode` rather than a second
  copy. Two answers to "which mode did the operator ask for" is one too many, and
  the copy would have drifted the first time preferred-mode handling changed.

## 2026-08-12: Topology apply is fed by a committed frame, not a scratch buffer

- The apply path has been blocked on "output-scoped framebuffer allocation" since
  the reference host refused it. Reading the architecture before building the
  unblock showed the obvious version of it is forbidden.
  `docs/renderer-import-boundary.md` states that native KMS initialization waits for
  the first committed-state frame rather than requiring a speculative or blank
  visual bootstrap. Allocating a scratch buffer at the new mode's size so apply can
  run on demand is exactly that bootstrap, and it would put a frame on screen that
  no committed state produced.
- The correct sequence is renderer-led: resize the frame target to the new mode,
  compose one frame at that size from committed state, then apply the topology
  naming that frame. `LiveGbmEglFrameTargetRecord` and its
  created/retained/resized/invalidated/retired lifecycle already model the resize;
  nothing tied it to activation.
- `native_output_apply_admission` is that tie. It is a pure reducer over the plan
  and a slice of reduced frame targets, returning ready or one precise cause with
  the output and both sizes. Apply consults it before composing heads, so a mode
  change now refuses with "output 2 has a 2560x1440 frame but the requested mode is
  1920x1080" instead of discovering a missing framebuffer mid-resolution and
  reporting it as a property of the hardware.
- One fidelity mistake caught on the way. The first wiring derived the frame size
  from the output's configured size rather than from what the CRTC is actually
  scanning out. Those are equal except during a mode change, which is the only time
  the question is asked, so the check would have passed exactly when it mattered
  least. Both the precondition and head composition now read the live framebuffer
  through one function, `read_native_current_framebuffer`, because two readers of
  "currently displayed" is one more than the number of answers that can be right.
- The roadmap's three-slot recycling pool is not a prerequisite. It is gated on
  measured software-fallback parity in Milestone 14, and treating it as the unblock
  would have built a pool to answer a question nobody had measured.

## 2026-08-12: The reservation shrink edge is fixed at the state layer, not the reducer

- Decision on the fail-open edge recorded yesterday: a mode change that leaves a
  root-relative reservation outside the new root currently releases it, so policy
  briefly sees the full output while a bar still occupies pixels. Two obvious
  answers were considered and both are wrong.
- **Fail closed by preserving is actively wrong.** A reduction returning `None`
  makes the caller keep the previous work area
  (`live_session/wm/work_area.rs`, `status=preserved reason=invalid_reduction`).
  That is safe when a malformed reservation arrives against unchanged geometry, and
  unsafe after a shrink: the preserved rectangle belongs to the *larger* output, so
  policy would lay out beyond the screen. Releasing the reservation is merely
  suboptimal; preserving a stale work area is incoherent. The existing fail-closed
  path is therefore not the fix, and reaching for it because the codebase is
  fail-closed elsewhere would have made things worse.
- **Clamping inside the pure reducer is also wrong**, though it looked right. A top
  bar of depth 40 is still a top bar after the output shrinks, and only its span was
  expressed against the old root, so clamping the span preserves the meaning. But
  the reducer cannot tell a span clamped by a shrink from a span that arrived
  malformed: `malformed_or_out_of_root_reservations_do_not_change_work_area` pins
  that a span starting at `-1` is rejected, and clamping would make that
  off-by-one publication take effect. Fixing a fail-open edge by weakening malformed
  rejection is a bad trade.
- **Decision: the fix belongs in `SurfaceOutputReservationState`**, which is the only
  layer holding what the reducer lacks — the reservations it previously admitted,
  and the root they were valid against. A reservation already admitted against a
  larger root is re-projected onto the smaller one when the root shrinks; a
  reservation arriving for the first time is validated against the current root and
  rejected if it does not fit. Same geometry, different provenance, different
  answer — which is exactly the distinction the pure function cannot make and this
  one can.
- Not implemented in this pass. It changes behavior for every bar, and the analysis
  that rules out the two shortcuts is worth more than a rushed version of the third.
  Current behavior stays pinned by a test that names it fail-open.

## 2026-08-12: Output power is a separate authority; reservations mostly were not missing

- The output-authority ledger row names four things beyond the topology machine:
  test, apply, rollback, reservations, and a separate power authority. Working the
  last two showed one was genuinely absent and one was mostly already built.
- **Power is now its own authority.** `OutputPowerState` holds per-output levels for
  the outputs the desktop currently has. The reason it is separate from enablement
  is that blanking a screen and removing a monitor are different facts: a dark
  output keeps its bounds, work area, and surfaces, and policy must not see the
  transition at all, while a disabled output leaves the complete snapshot and forces
  a relayout. The temptation to merge them is not hypothetical — atomic modesetting
  powers a head down by clearing the CRTC's `ACTIVE`, the very property that
  disables one. That is a fact about the commit, and letting it leak upward would
  make "blank the screen" and "unplug the monitor" the same operation to every layer
  above.
- Two rules fall out of admission. A topology change keeps the level of every
  surviving output, so a mode change on one monitor cannot relight another that an
  idle policy powered down; and an output that leaves keeps no level, so a
  reconnected monitor cannot inherit a stale one and come back dark for no traceable
  reason. Both are the same reconciliation, and both are tested.
- **Reservations were largely present.** Work areas are already re-projected from
  the new output rects inside the topology publication that swaps the outputs, so
  geometry and work area commit together on the hotplug path. What was missing was
  any test pinning it. The plan assumed a gap that the code had already closed;
  reading the runtime path before writing the fix is what turned a rewrite into a
  regression test.
- **One fail-open edge, documented rather than fixed.** Reservations are
  root-relative, so shrinking an output can leave one outside the new root. Such a
  reservation is filtered *before* reduction, so reduction succeeds and reports the
  full output as available — silently releasing it. Between the mode change and the
  reserving client republishing, policy may lay out under a bar that still occupies
  pixels. The fail-closed path exists immediately next door: a reduction returning
  `None` makes callers preserve the previous work area. The pure reducer cannot
  distinguish a malformed reservation from one invalidated by a geometry change,
  because it holds no previous state, so closing this belongs at
  `SurfaceOutputReservationState`. It changes behavior for every bar, which is why
  it is recorded as a decision to take rather than taken at the end of a long
  session.

## 2026-08-12: Broker classifications are extension-chunk content

- The open question was how broker-issued classifications reach Hagia, and it was
  the last wire decision that could force a `*Begin` layout change. Two shapes were
  on the table: a small closed set of policy classes riding
  `SnapshotSurface.kind` or spare `capability_bits`, or an expiring per-surface
  grant that fits neither and forces a layout change. The answer is neither.
- **The closed set does not fit the rules.** Triad's `WindowRule` at baseline
  `fb8fb27e` (`src/types/runtime_values.nim:211-283`) carries about thirty-five
  outcomes. Stripping what never crosses (match expressions), what already crosses
  (`min_size`/`max_size`, `request_state_bits`, reduced parent role), and what
  belongs to Engine chrome or the session and security authorities
  (`border`, `focusRing`, `clipToGeometry`, `keyboardShortcutsInhibit`,
  `idleInhibitMode`, `presentationMode`, `openOverlay`, `openUnmanagedGlobal`)
  still leaves around nine WM-bound booleans and eleven WM-bound *parameters*:
  default workspace, output, column width, scroller proportions, default window
  size, named scratchpad, floating position, maximize policy, forced layout.
  Eleven free `capability_bits` would not survive the booleans, and a bitfield
  cannot carry a workspace number or a scratchpad name at all. The phrase "a small
  closed set of policy classes" described a classification vocabulary accurately and
  these rules inaccurately.
- **Decision: a capability-gated extension chunk**, carrying
  `(surface, classification)` records under a reserved `0xFF00`–`0xFFFF` kind. It is
  uncounted, so it costs no layout change; chunk data is self-delimiting, so it
  carries parameters as easily as flags; and it reaches only a client that
  negotiated the capability.
- **This removed a pre-freeze obligation instead of satisfying one.** The decision
  was believed to require reserving vocabulary in `SnapshotSurface` before the
  freeze. It requires reserving nothing there, and the classification vocabulary is
  no longer frozen with the revision — a rule family recognized later is an added
  chunk record rather than an impossibility.
- The option existed only because outbound capability gating landed earlier this
  session. The original analysis was written while clause 2 was unsound, when every
  server-to-client addition really was now-or-never. Its framing survived the change
  that invalidated it, which is the general hazard: a decision inherits the
  constraints of the day it was framed, and those constraints are exactly what the
  intervening work is meant to remove. Re-derive a pending decision after landing
  anything that changes its premises.
- One review-time rule is now checked. The generator rejected nothing in the
  reserved range because keeping it clear was documented rather than enforced; it
  now refuses any ordinary record declaring a kind at `0xFF00` or above. Verified by
  temporarily moving `SnapshotAction` to `0xFF01` and confirming the refusal.

## 2026-08-11: Reusing the console's framebuffer cannot apply this desktop

- The apply path was bounded to reuse the framebuffer each CRTC already scans out,
  on the reasoning that re-applying the topology already on screen would be the
  smallest real mutation available and therefore the right first one. The first
  authorized run refused, and the reasoning was wrong for a reason worth recording.
- `DP-2` is a 1920x1080 panel scanning out a **2560x1440** framebuffer. The console
  gives both CRTCs one buffer sized for the larger monitor, while Sophia's candidate
  asks each output for its own preferred mode. So "the topology already on screen"
  and "the configured topology" are not the same topology on this host, and the
  buffer that exists fits neither output's requested mode.
- The same constraint appears from the other direction. Two heads sharing one
  framebuffer are a mirror group, and a mirror group must be same-mode because no
  plane scaling exists anywhere; these two outputs disagree on size, so reusing the
  shared buffer for both heads would fail `MismatchedMirrorSize`. Two independent
  rules, one answer, which is a good sign the model is coherent.
- Nothing was submitted. The resolver failed closed before the first commit, which
  is what the bound was for: the failure mode it was chosen to prevent is a
  half-applied desktop, and a refusal at resolution time cannot produce one.
- The first refusal said only "has no framebuffer at the requested mode's size",
  which could not separate an output displaying nothing from one displaying the
  wrong size — the same defect as a rejection without an errno, made twice in one
  day. The cause now carries what the CRTC actually scans out. A diagnosis that
  requires a second tool is not a diagnosis.
- Consequence for the tranche: apply is complete as code and blocked as evidence,
  on output-scoped framebuffer allocation rather than on anything in the apply path.
  That is renderer work, and it now has a concrete reason to exist rather than a
  speculative one.

## 2026-08-11: The configured two-output desktop is valid to the kernel

- First hardware answer about a Sophia-composed topology rather than a hand-built
  one. `tools/native_topology_validate.sh` runs startup's own chain — capabilities,
  topology projection, candidate reconciliation, plan preparation, head resolution,
  activation phase machine — and submits the result as one `TEST_ONLY` request:
  `validation=accepted settlement=not_applied outputs=2 heads=2`.
- What that does and does not establish. It establishes that the configured
  candidate on this host resolves into KMS objects the kernel accepts as one atomic
  desktop, which is the question head resolution existed to raise. It does not
  establish that applying it works: no framebuffer is named, nothing was mutated,
  and apply remains absent rather than gated.
- The proof runs read-only by construction, not by care. `NativeOutputTopologyValidationExecutor`
  has no apply path at all, and bringing the live scanout up performs no modeset. A
  validation that could accidentally mutate would need a reviewer to check; one that
  cannot express mutation needs only its type read.
- An accepted validation still settles as `Rejected`, and that is not a defect. The
  reducer proceeds from a passing test into apply, apply refuses, and the settlement
  records the refusal. The trap is that apply refuses with `WouldBlock`, which is
  also what a busy device reports, so the settlement cannot distinguish "the kernel
  accepted your desktop" from "the card was busy". The executor therefore retains
  the kernel's own answer and it is logged separately. A reduced log line that
  cannot separate the interesting outcome from the boring one is not evidence.
- Multi-device topologies are declined rather than validated. One atomic request
  reaches one DRM device, so a desktop spanning two cards cannot be judged as a
  unit; validating one card's fragment and reporting it as the answer would be worse
  than reporting nothing.

## 2026-08-11: A validation-only modeset needs no framebuffer

- The open question was whether a `TEST_ONLY` modeset requires plane state and a
  valid `FB_ID`. It decided the shape of head resolution: if validation needs a
  framebuffer, then checking a new mode means first allocating one at that mode's
  size, and allocation stops being a consequence of a validated topology and
  becomes a precondition for validating one. It is driver-dependent and cannot be
  settled offline, which is why `native-topology-probe` exists.
- **Answer: no.** On the AMD two-output reference, from a bare TTY holding DRM
  master, a modeset naming only the connector's `CRTC_ID` and the CRTC's `MODE_ID`
  and `ACTIVE` validates. Adding primary-plane state and reusing the framebuffer
  already being scanned out also validates. Validation precedes allocation, so head
  resolution may size and allocate framebuffers after the kernel has agreed to the
  topology.
- The first run said the opposite, and the difference was ours. Both probes came
  back `EINVAL` with mode and framebuffer sizes in agreement, which ruled out the
  plane-sizing explanation and pointed at the request. The kernel rejects
  `TEST_ONLY` together with `PAGE_FLIP_EVENT` outright, before inspecting any
  property, and `LibdrmNativeAtomicCommitRequest` defaulted `page_flip_event` true
  while `test_only()` left it alone. Every validation-only commit in the tree was
  returning `EINVAL` regardless of content.
- The blast radius was larger than the probe. `submit_native_multi_head_topology`'s
  `Validate` intent carried the same flags, so `NativeOutputCommitExecutor::validating`
  would have declined every topology it was ever handed, and the declined-test path
  at startup would have read as hardware refusing a valid desktop. The bug would
  have been discovered as a mysterious hardware incompatibility rather than as a
  flag error.
- The fix derives the flag instead of storing it: a request that is `test_only`
  reports and sends no page-flip event, so the illegal combination cannot be
  constructed in any call order. A validation-only commit never reaches scanout and
  so has no completion to signal; the kernel's rule and the semantics agree.
- Two general lessons, both cheap to state and expensive to relearn. A probe that
  records only `Accepted`/`Rejected` cannot distinguish a hardware answer from a
  malformed question, so a refusal must carry its errno and the operands that
  explain it. And a negative hardware result should be suspected of being a defect
  in the asker until its errno says otherwise — the first run's conclusion was
  written into `todo.md` as two plausible hardware hypotheses, and neither was true.

## 2026-08-11: Mirroring is admitted as one logical output with many heads

- Native output mirroring was an open product call the port ledger refused to
  leave implicit. The decision is to implement it before the freeze rather than
  reject the port obligation. Triad's own contribution here is only a repudiated
  screencopy prototype — an ordinary fullscreen client whose documentation
  forbids citing it as output-cloning evidence — so nothing is being ported; this
  is new capability.
- The shape is forced, not chosen. Modeled as **one logical output backed by N
  connectors**, mirroring is invisible to policy: `SnapshotOutput` carries no
  connector identity, mode, scale, or enabled flag, so there is no
  `sophia_wm_v1` wire risk and no competition for the pre-freeze window. The
  alternative shape, two logical outputs sharing surfaces, is inexpressible: it
  violates one-output-per-surface and raises `DuplicateSurface`. Recording this
  matters because the inexpressible shape is the more intuitive one.
- Joint multi-head retirement changes multi-output and buffer-lifetime
  semantics, so the standing rule to extend the bounded visual-retirement model
  first applies. That pulls Milestone 14's first roadmap item into Milestone 13.
  `VisualRetirement.tla` now carries a head layer beneath output retirement: a
  logical output retires only when its last head flips, the framebuffer stays
  leased until then, and one output in the checked configuration is a two-head
  mirror group so a single run exercises joint retirement within a group and
  independent retirement between groups. 112,252 distinct states, depth 19.
- The ratified presentation invariant is narrowed rather than dropped.
  Retirement is joint within a mirror group and independent between groups; the
  unit of retirement is still one logical output, distinct outputs still retire
  on their own page-flip timelines, and Sophia still claims no globally
  simultaneous multi-output retirement instant.
- Head loss fails the candidate closed. A connector that disappears mid-flight
  drops its lease without counting as a flip, and a surviving-head topology is a
  new candidate rather than a salvaged one. Head loss sits outside the fairness
  assumption, because nothing guarantees a connector disappears.
- Two negative controls were required to get the invariant set right, and the
  second reversed a wrong conclusion. Treating any flipped head as retiring the
  whole output violates `CommittedAfterExactRetirement`, which initially looked
  like proof that a head-level commit invariant would be redundant. It is not:
  making loss silently shrink a group corrupts the `RetiredOutputs` definition
  itself, so the invariant expressed through that definition goes blind while
  `MirrorGroupCommitsOnlyWithEveryHead`, which names the heads directly, still
  fails. An invariant stated through a derived operator cannot check that
  operator. A separate release-safety invariant was dropped as genuinely
  redundant, since `ReleasedResourcesAreTerminal` already states the condition
  directly on head-scoped `inFlight` with no definition in between.
- Mirroring is same-mode only. No plane scaling exists anywhere in the tree, so
  mismatched modes must fail closed at reconcile time rather than silently
  letterbox. Implementation work remains: lifting singular per-output connector
  selection to a set, sharing the currently exclusive buffer lease, allowing N
  heads per logical rect, exempting mirror-group members from overlap rejection,
  and a physical gate on two same-mode heads.

## 2026-08-11: The freeze surface is bounded, and reserved space is asymmetric

- The roadmap had been treating "additive wire records are cheap" as a standing
  property. It is a pre-freeze-only property, and nothing recorded that. This
  pass enumerated the risk and wrote `docs/wm-v1-freeze-surface.md`.
- Two rejection behaviors set the cost. Unknown record kinds fail closed in both
  transfer decoders, and unknown enum discriminants fail closed everywhere, so a
  pre-freeze client cannot tolerate a post-freeze value. Both `*Begin` messages
  are fixed-layout with a strict terminal length check, so a new record kind
  needs a sixth count field, which is an existing-layout change and therefore a
  new interface family. The indicator descriptor is the worked precedent: it
  landed pre-freeze exactly so `indicator_count` and `status_count` could be
  added.
- A previous reading of this boundary was wrong in a way worth recording.
  Grepping the generated record module for `reserved` returns nothing, which
  looks like proof that no reserved space exists. The generated Rust omits
  reserved fields while the wire still carries and validates them:
  `WmV1ProjectionOutputRecord` has four fields, `PROJECTION_OUTPUT_RECORD_SIZE`
  is 24 bytes, and the codec pushes a zero it later rejects if non-zero. Struct
  shape is not wire shape.
- Reserved space is real but unevenly distributed. Fourteen reserved fields
  exist: twelve messages carry a `u16`, and `ProjectionOutput` and
  `ProjectionOutputStatus` carry a `u32` each. Six of the eight record kinds have
  none, so any new per-surface, per-placement, per-indicator, or per-output fact
  is either a bitfield extension or a layout change. `capabilities` is a `u64`
  with bits 0 through 9 assigned.
- Enumerating all 27 retained port-ledger rows found 23 that need no
  `sophia_wm_v1` change. The residue is four decisions, three of which must be
  settled before the freeze forecloses the cheaper option:
  configured workspace and view names should project as indicator labels, since
  a policy-authored label is blind-safe by construction and the record already
  carries a bounded label; broker-issued classification should commit to a
  closed set of policy classes that fits `SnapshotSurface.kind` or spare
  `capability_bits`, because expiring per-surface grants are the only thing in
  the ledger that forces a layout change; the continuous-pointer payload fits
  the four existing `interaction_*` integers with `reserved_cause` spent on an
  axis discriminant, so drag and scroll need enum values but no layout change;
  and the output logical-space contract needs writing down so the output
  authority does not widen `SnapshotOutput`, which has no reserved space.
- Two coupling facts constrain when, not what. `Cancel` is a lease-revocation
  contract and must be specified alongside the lock and security epoch barrier
  or it will be specified twice. Per-motion `Update` requests are currently
  dropped because the queue deduplicates by pointer-gesture source, so
  continuous updates need a replaceable latest-value coalescing rule rather
  than more queue capacity.
- Two items remain open product calls: the forward-compatibility rule
  (skip-unknown behind an extension chunk, or declare revision 3 final for
  WM-side records) and whether native output mirroring is implemented with
  evidence or explicitly rejected. The ledger forbids leaving the second
  implicit, since "not yet implemented" is not an exclusion.
- Corrected in passing: the claim that the bootstrap profile used 39 of 64
  binding slots. Every constant in both repositories is 256. The bootstrap emits
  39 key plus 2 pointer bindings from Triad's 132 key plus 5 pointer baseline,
  and the remaining bindings are classified into authorities that do not exist
  yet. The open question is the authority split, not capacity.
- Milestone 12 is archived to `docs/roadmap-history.md`. Its last open item was
  the X11 compatibility matrix update, whose evidence had already landed with the
  passing TrueColor proof; only the checkbox was stale.

## 2026-08-10: The trusted profile barrier now shares Hagia's proven semantics

- Sophia already validated and staged exact fragments for all seven desktop
  authorities, but the executable prepare/activate/rollback reducer existed
  only in Hagia's model-checked reference implementation.
- `sophia-config` now ports that transition shape over Sophia's canonical
  `DesktopAuthority`, `ConfigGeneration`, and `ConfigDigest` types. Effects are
  emitted in deterministic authority order; all preparations are required
  before activation; any prepare or activation failure rolls every authority
  back; and the active identity changes only after all matching activations.
- Rejected generations remain consumed, stale completions are inert, and
  generation exhaustion cannot wrap. Mirrored Rust tests cover success,
  rejection, partial activation, stale completion, failed rollback, and the
  rejected-generation reuse defect previously found through Hagia's formal
  verification. Authority protocols, production effect handlers, and watched
  reload remain deliberately disconnected.
- The CLI now has the corresponding injected startup executor seam, porting
  Hagia's typed-completion pattern without importing its runtime. Prepare,
  activate, and rollback dispatch to separate handlers and return the exact
  authority/generation/digest completion message. Only fakes populate those
  handlers today; production authority transports and watched reload remain
  deliberately disconnected.
- A synchronous startup driver now drains those effects through both barriers.
  It stops unused prepare or activation work after the first failure, attempts
  rollback for all seven authorities even if one rollback fails, preserves the
  previous active profile, and returns the exact pending recovery model.
  Deterministic tests enumerate every authority as each failure point, prove
  initial activation from empty state, and prove that rejected generations
  cannot be retried. The driver adds no asynchronous interleaving beyond the
  lifecycle already exhaustively checked in Hagia's TLA+ model.
- A reusable authority-local participant reducer now gives every authority the
  same transition discipline without copying handlers: strict monotonic
  prepare, exact-key idempotent prepare/activate/rollback, previous-active
  restoration, and inert strictly older cleanup. It retains the last admitted
  generation and digest after cleanup, which closes the ambiguity between an
  exact rollback retry and a same-generation digest mismatch. Tests apply the
  initial activation and rollback invariants to all seven authorities and
  cover retries, mismatch rejection, prior-active restoration, generation
  consumption, and exhaustion.
- Participant activation is not yet externally visible. Transactional startup
  may eventually use it behind the existing graphical launch gate, where no
  client observes partial activation. Watched reload needs a distinct global
  visibility/recovery protocol and remains out of scope for this milestone.
- Refinement testing found that the synchronous driver stops the remaining
  prepare batch after one rejection but deliberately rolls all authorities
  back. A participant skipped by that batch had treated the future rollback as
  an identity mismatch, which would turn every early preparation rejection
  into a false recovery failure and leave the skipped generation locally
  reusable. Idle or previously activated participants now consume that exact
  unseen rollback as a no-state tombstone while retaining the active identity;
  conflicting prepared work and same-generation digest mismatches still fail
  closed.
- The test-only executor now drives the coordinator's real effect sequence
  through seven independent authority-local candidate slots rather than
  identity-only participant models. Across every authority as the prepare,
  activation, and rollback failure, it proves successful identity and payload
  agreement, complete last-known-good payload restoration, rejected-generation
  consumption, exact single-authority divergence, and deterministic recovery.
  A staged integration case then loads the exact owner-safe fragments for all
  seven authorities through the shared admission function and proves semantic
  payload promotion under the coordinator's exact activation key.
- Startup tracing found one avoidable split in candidate ownership: the shared
  preparation result was partially moved into session fields, while public
  policy setup parsed the shortcut candidate again from the raw profile. The
  preparation boundary now checks all seven candidate identities against the
  profile generation/digest, session configuration retains one immutable typed
  bundle, and policy setup clones its shortcut value. This gives future
  authority handlers one canonical input without labeling preparation as
  activation or changing launch order.
  A later ownership pass partitions that bundle once into session, input, and
  output owner records plus the shortcut transfer payload; the aggregate is no
  longer retained as a second coordinator-owned copy.
- Public policy directory creation and fragment staging previously lived inside
  WM process construction. A linear `PreparedPublicPolicyLaunch` context now
  performs owner-only directory creation, stages and re-admits every authority
  fragment against the exact key, and prepares shortcut state immediately after
  trusted configuration parsing. Tests prove mode, complete fragment presence,
  exact admission, `Prepared` phase, and cleanup without launching a process.
- The synchronous startup driver previously fused its prepare and activate
  phases even though the graphical launch gate needs a stable prepared pause.
  `run_desktop_profile_startup_preparation` now returns a typed
  `Prepared`/`Rejected` report after settling exactly the prepare batch and any
  rollback, with no activation calls. The complete driver reuses it. Tests
  cover success plus every authority as the prepare rejection point.
- Public Hagia startup now invokes that prepare-only driver immediately after
  staging and exact fragment admission. A fixed-field dispatcher borrows the
  separate raw policy/shell/broker owners, the public shortcut owner, and the
  typed session/input/output owners; it owns no slot collection. Success leaves
  all seven participants and the coordinator at the same `Prepared` key before
  display setup. Injected failure at every authority proves generation-wide
  rollback, empty candidates, unchanged active identity, cleanup, and no
  process launch.
- The retained activation driver is now the only public-policy launch path.
  After the complete prepare barrier, six Sophia-local owner slots activate in
  canonical order and Hagia is started with the exact staged Policy identity;
  the coordinator promotes only after Hagia's matching completion. A launch
  timeout, disconnect, rejection, or participant failure drains generation-wide
  rollback before graphical construction. Removing the prepared-only branch is
  deliberately limited to synchronous startup visibility: watched reload still
  needs a separate global visibility and durable-recovery protocol. The former
  `--wm-profile-activation` proof switch remains accepted as a compatibility
  no-op, so installed launch profiles do not break while the unsafe choice is
  gone.
- `load_desktop_profile` had already run the shared preparation boundary before
  returning, so live-session startup's immediate second preparation was pure
  duplication. `load_prepared_desktop_profile` now returns a typed aggregate
  containing the validated provenance-bearing profile and its derived bundle;
  its activation key is projected directly from that same generation/digest.
  The raw loader delegates to this path and discards only the bundle when a raw
  caller explicitly requests that view.
- The next handler prerequisite reuses rather than extends the staged format.
  `load_desktop_authority_fragment` applies Sophia's existing absolute,
  regular, bounded, owner-safe file rules, rejects symlinks, parses exactly one
  assigned authority section, validates its settings, and requires the file's
  generation/digest to equal the coordinator key. Tests round-trip every
  fragment emitted by `stage_desktop_profile` and reject authority crossing,
  generation drift, digest drift, unsafe mode, and symlink substitution. Hagia
  retains its independent Nim reader as cross-implementation evidence.
- A generic authority-local candidate slot now joins admitted payloads to the
  participant reducer without duplicating its identity state machine. It owns
  active, previous-active, and candidate payloads and updates them only after a
  successful pure participant transition. Prepared typed candidates advertise
  their fixed authority and key; raw candidates advertise their assigned
  authority. Exact raw retries compare semantic settings rather than provenance
  paths, allowing in-memory and staged reconstruction of the same payload while
  rejecting changed content at one identity. Focused tests cover prepare,
  activate, restoration, retries, conflicts, invalid identity, unseen rollback
  tombstones, and fragment rejection with unchanged slot state.
- Session startup previously parsed and mutated CLI application overrides in
  the middle of configuration assembly. A typed immutable overlay now owns the
  bounded application additions, argument extensions, startup order, and action
  selectors. One pure preparation function applies the canonical session
  profile candidate and CLI-superior overlay to a cloned trusted registry.
  Existing profile/CLI parity tests still pass; new evidence covers identical
  preparation and rejection with the previously accepted state unchanged.
- The trusted session configuration now retains its canonical typed profile
  payload in the generic authority-local slot. Effective application
  preparation reads the slot's candidate rather than bypassing participant
  state. Tests prove the slot and canonical bundle carry the same payload and
  exact key in `Prepared` phase; startup assembly performs no activation.
- A shared `with_candidate` constructor now centralizes empty-slot creation and
  the initial prepare transition. The public shortcut owner uses it to retain
  the canonical typed shortcut candidate and resolves registrations from the
  slot payload. This keeps session and shortcut participant state with their
  respective owners and avoids a coordinator-owned slot collection.

## 2026-08-10: Desktop output activation has an immutable pre-I/O plan

- Native startup already projected the atomic owner's capabilities and
  reconciled the unified output candidate before launching clients, but the
  reconciled connector states had no explicit handoff shape for a future
  backend executor or rollback path.
- `sophia-config` now centrally validates a complete output reconciliation
  against its admitted topology, including exact connector coverage, modes,
  scale, transform, VRR, focus, overlap, and at least one enabled output. The
  reconciler calls this same validator, so downstream boundaries do not copy
  those invariants.
- The CLI coordinator now prepares one immutable plan in stable Engine output
  order. Each target carries its `OutputId`, exact requested state, and exact
  rollback state plus the shared generation/digest; no connector, CRTC,
  property, framebuffer, or file-descriptor handle crosses the backend
  boundary. A second capability snapshot with changed mode or VRR facts is
  rejected as drift. Startup logs `prepared_not_applied` and performs no KMS
  test or mutation. Atomic request construction and effect execution remain
  the next tranche.
- A pure coordinator now supplies that typed completion contract without
  connecting hardware I/O. Test, apply, and rollback effects carry the same
  immutable plan and exact generation/digest key. Only a completion for the
  expected phase may advance the attempt; late, duplicate, and out-of-order
  completions are inert. Test failure discards the candidate immediately,
  apply failure requires rollback, and rollback failure is a terminal outcome
  retaining both failure causes. The executor and native request construction
  remain deliberately deferred.

## 2026-08-09: Revision 1 remains experimental through the Triad port

The fixed nine-view Hagia scroller and its physical checkpoint/restart proof
establish that the public WM boundary is usable; they do not establish its
final vocabulary. Freezing `sophia_wm_v1`, removing API v7, or archiving a
permanent revision-1 client before the retained Triad behavior is ported would
turn missing product workflows into revision-2 changes.

Port completion is now behavior-based and authority-aware. Spatial policy
belongs in Hagia; overview, tabs, switchers, and visible feedback belong in a
separate Hagia Shell; input/output/launch/lock operations belong to Sophia
services; and metadata or payload-bearing workflows belong to brokers and
portals. River/Wayland and Niri compatibility machinery is excluded, but a
feature is not excluded merely because it requires a separate process. The
canonical inventory is Hagia's `docs/triad-port-ledger.md` at Triad baseline
`fb8fb27ec294e0fe2361375de0b2fa8c08be0ca9`.

This supersedes the earlier sequential claim that WM revision 1 must freeze
before shell modeling begins. The interfaces remain separate, but retained
policy and shell workflows may drive both experimental contracts before either
stability decision. Shared reconnect/restart conformance and the archived
client become final freeze checks after port completion, not substitutes for
it.

The first post-decision slice adds nonempty multi-tag view and focused-window
membership to Hagia through eighteen opaque reducer actions and deterministic
tests. No raw input or Sophia identity enters its policy model. Triad does not
expose these transitions as default multi-tag commands, and its
`Super+Ctrl+1..9` chords already move windows, so Hagia does not install novel
bindings for them. The bootstrap profile emits 39 of the draft's 256 admitted
binding registrations, while the recorded Triad default has 132 key bindings
and 137 total physical bindings. Those bindings span WM, shell, Engine,
session, and portal owners, so the raw count does not justify blindly raising
the WM limit. It does require the port to classify the retained set and measure
each authority's capacity before revision 1 freezes.

## 2026-08-09: The physical Hagia gate carries its procedure into Kitty

The third authorized Hagia restart attempt ended cleanly but was not a policy
exercise. Its total physical ingress was exactly 34 events: press and release
for each of the 16 letters in `hagiapolicyproof`, plus Enter. It contained no
preceding chord events, no ordered policy actions, and no supervised restart.
That makes it a useful negative observation, not promotion evidence.

The proof launcher had printed the action sequence before transferring the
operator and device ownership to TTY4. The gate now starts Kitty with a bounded
POSIX-shell guide that watches the evidence file, displays one chord at a time,
and advances only after Sophia commits that action. It reveals the final phrase
only after the supervised restart and complete post-restart sequence. This
changes only the human proof harness; action admission, fault injection, and
evidence validation remain the same. A replacement authorized run is still
required.

The first guided attempt immediately exposed a separate verifier-ordering
error. Its initial Super press reached physical ingress and mapped to X keycode
133 before the bound `Y` press could be consumed as an Engine-owned action; the
exact application-text matcher treated that non-text modifier as the first
letter and aborted. Physical text verification now excludes Control, Shift,
Alt, and Super transitions from its text-producing sequence. Their application
delivery semantics are unchanged, and any unmatched non-modifier key still
produces an exact mismatch.

The next guided run crossed the intended failure boundary for the first time:
fullscreen and active-output actions committed before the injected fault;
epoch 2 loaded and reconciled the nonempty checkpoint and requested the
generation-2 refresh; and post-restart fullscreen plus both maximize actions
committed. The following minimize action correctly hid Kitty, but that also hid
the guide's later restore instruction. The run timed out without application
text rather than guessing past the missing prompt. The guide now presents
minimize and its immediately required restore chord together before waiting for
either outcome.

A follow-up again stopped at the committed minimize boundary. The twelve
client-routed modifier transitions account exactly for the six committed
chords, but terminal evidence could not distinguish an absent restore chord
from a recognized restore awaiting policy settlement. The owner now records
physical action admission immediately, before its ordered policy request
settles. The guide's minimize screen also leads with a three-line instruction
to release `Super+N` and then press and release `Super+R` while Kitty is hidden,
without waiting for another prompt.

The next attempt initially appeared to exhaust the 120-second global ceiling,
but timestamp review proved the operator's objection correct: only 15.5 seconds
elapsed. A separate hard-coded physical-sequence timeout had fired after the
second maximize action. That 15-second fail-fast default remains appropriate
for existing short physical proofs, so it is now an explicit bounded session
option rather than being weakened globally. The Hagia guide requests ten
minutes inside an eleven-minute global ceiling. This adds no soak requirement:
the run exits immediately after its exact proof and only bounds an abandoned
exclusive DRM/input session at a practical human timescale.

The next run reached `Super+N` and `Super+R` without a timeout. Both actions
were admitted and committed, proving the blank screen was not operator input
loss: the minimize projection contained zero render layers, and the following
restore projections also contained zero. Reduction found that Sophia built its
nominally complete public-policy snapshot from visible and planning layers
only. Once minimize removed the visible layer, Hagia's complete-snapshot
reconciliation correctly interpreted the absent surface as destroyed and
discarded its private minimized history. Sophia now retains authority-observed
facts independently from render visibility and includes every policy-owned
surface in the snapshot until an explicit withdrawal or removal. The follow-up
run showed why the frontend's `mapped` bit cannot define that lifetime: an
Engine-admitted X surface retains the client's pre-admission `mapped=false`
observation because admission is not a second client `MapWindow` request.
Sophia now uses the retained request/withdraw ownership record instead. A
regression covers the admitted, hidden, `mapped=false` record and its later
explicit withdrawal.
The physical verifier and in-Kitty guide now additionally require the first
post-restore checkpoint to remain nonempty, so a committed no-op restore cannot
be promoted. A replacement physical run is still required.

The first run with request/withdraw ownership failed closed during Kitty
startup, before physical interaction. Its diagnostic snapshot contained three
unassigned surfaces, and Hagia's resulting proposal reached a surface for
which Sophia had no X11 client route. Authority presentation observations can
describe transient hierarchy members without carrying a frontend client
identity, so retained presentation facts alone are broader than the public
policy capability set. Complete snapshots now admit only the intersection of
retained policy ownership and a live authority client route. That route is
created by the client request, survives minimize/restore, and is removed with
the actual surface. The regression includes an unrouted authority observation
and proves it cannot enter the snapshot. The real-Hagia nonexclusive restart
smoke then completed cleanly: its startup snapshot admitted exactly one routed
surface, epoch 2 loaded and reconciled the nonempty checkpoint, the refresh
cycle committed, and all session/layout/cleanup health checks passed.

The replacement physical run then crossed the architecture boundary that had
blocked every earlier attempt. The minimize request retained one snapshot
surface; the next request reported that same surface as minimized; restore
returned it to non-minimized state; and every checkpoint remained nonempty.
Both output actions, fullscreen, two maximize transitions, and the exact final
text were physically delivered. A final close race prevented promotion: after
the guide accepted the phrase and exited, Hagia returned a projection based on
the just-retired one-surface snapshot. The canonical reducer still held that
base generation, so Sophia attempted to materialize the dead placement before
advancing to the already-queued surface-removal cycle. Public response handling
now snapshots current Engine facts before reading any response placement. If
the scene advanced, it retires the outstanding request as `RejectedStale` and
only then permits a fresh complete cycle. Thus close-during-response cannot
name a missing planning surface or terminate the session.
The guide also remains alive after accepting the final phrase until Sophia's
exact proof ends the session, removing the harness-created close race while
retaining the Engine-side stale-response defense for real client exits.

The next replacement attempt showed both requested `Super+Right` actions as
admitted and committed, but the guide remained at its restart wait because no
restart occurred. The injected fault was still armed on the sixth global
checkpoint. Extra valid startup and stale-response settlement cycles changed
that count, so checkpoint ordinals were not causally tied to the physical
procedure. The gate now arms an evidence watcher for the ordered fullscreen
and first active-output commits, then kills Hagia only after the next nonempty
checkpoint. The wrapper starts the watcher before `exec`-replacing itself with
Hagia, preserving the exact supervised PID authorized by the private policy
endpoint. A deterministic process test proved that an earlier checkpoint does
not trigger the kill, the first following checkpoint does, and an epoch-2
invocation with the marker present runs without a second injection. Another
physical run remains required.

That replacement run proved the complete policy path: the restart occurred
immediately after the checkpoint following the first active-output action;
epoch 2 loaded and reconciled its nonempty candidate and issued the
generation-2 refresh; fullscreen, both maximize transitions, minimize/restore,
and both output actions committed; and the restored checkpoint remained
nonempty. The operator then entered the exact phrase, all 52 action-plus-text
key transitions flushed to the X frontend, and native presentation remained
active. The run did not promote because Sophia timed out waiting for the
semantic result file. Command construction had appended the stock proof-result
writer after Kitty's custom guide program, making it unused guide arguments
rather than an executed command. Sophia now passes the owner-only result path
explicitly in the guide's environment, and the guide writes the exact bounded
line only after reading it. An isolated replay over the completed evidence
proves that witness path. One final physical run is required for the combined
semantic, pixel, presentation, health, and cleanup verdict.

The final authorized run passed that combined verdict. The causal injector
restarted Hagia immediately after the nonempty checkpoint following the first
active-output commit. Epoch 2 loaded and reconciled the candidate, requested
its generation-2 two-output refresh, and preserved fullscreen state. The
post-restart sequence committed fullscreen, maximize twice, minimize, restore,
left output, and right output, with a nonempty checkpoint after restore. The
guide recorded the exact 16-byte line it read; Sophia matched all 34 text
press/release events, flushed all 52 action-plus-text X11 transitions, observed
changed terminal pixels, and correlated the final libinput ingress to a kernel
page flip in 24 ms. The bounded session ended after 24.994 seconds with one WM
restart, 13 committed WM cycles, no degraded state, no pending WM/action/input
work, no unexpected protocol errors, drained native ownership, clean topology,
and clean process/namespace/Xauthority teardown. This is the first passing
installed Hagia physical policy/restart evidence and closes that promotion
boundary; it is not a universal cadence or latency guarantee.

The foundation-era revalidation at Sophia commit `9b3750e4` and Hagia commit
`cbb629a` passed the same installed gate as immutable promotion run `0002`.
That record independently verifies the unified compiled profile path,
Chronicles checkpoint evidence, epoch-two restoration, all required physical
actions, exact semantic input, checksums, and clean bounded teardown.

## 2026-08-09: Revision-1 behavior corpus crosses three independent clients

One authenticated host now drives the Rust reference client, the independent
C99 client, and Hagia through the same eleven policy cycles on a single
connection. The sequence covers constrained single-output layout, admission of
a second output, loss with complete surface migration, and return of the same
raw output at a new generation. It then delivers one ordered focus action,
times out that cycle's successor, rejects one candidate after a newer scene
supersedes it, rejects one deliberately invalidated candidate, and requires a
later complete cycle to commit after every outcome. Committed proposals must
pass the canonical reducer, retain every assigned surface exactly once, and
preserve the snapshot's active output. Hagia keeps its private adapter alive
across the sequence, so both the generation-advancing return and every
noncommitted-candidate discard exercise real client state rather than
standalone decoder fixtures.

The X11 bridge now consumes the same eleven canonical scenes and causes through
an explicit API-v7 corpus adapter. It translates synthetic-X configure/focus
decisions for every affected output, combines them into one candidate, and
passes the canonical reducer without being mislabeled as a direct
`sophia_wm_v1` peer.
This still does not freeze revision 1: shared reconnect/restart scenarios and
an archived client remain open.

Dynamic KMS topology ingress now has its offline implementation and formal
boundary. A capacity-one udev monitor coalesces `HOTPLUG=1` changes into a
monotonic rescan hint; only the session owner rebuilds authoritative connector
state. The owner advances routed-input authority once, quiesces or detaches the
old native generation, retains application processes and CPU buffers, and
publishes the same complete replacement generation to scanout, pointer, RandR,
and policy consumers. Cross-process settlement is sequential while input stays
quarantined; it is not claimed to be simultaneous IPC visibility. Empty or
failed rescans remain shortcut-only and retry without consuming a second input
epoch. Changed topology cannot restore application input until its policy
projection commits and a later page flip retires. VT resume and startup
recovery enter the same transition rather than aborting on a changed output
vector.

`OutputTopologyLifecycle.tla` checks that ordering across a replaceable notice,
security epoch, retirement, rebuild, publication, policy settlement, and first
presentation. TLC explored 1,484,394 generated and 230,797 distinct states to
depth 32. Temporary controls exposed old RandR publication, old callback
acceptance, early input enablement, and output-identity reuse. Alloy separately
rejects torn publication, while Z3 checks complete horizontal topology
geometry against both the pinned 4.16.0 solver and the local 5.x differential.
The physical disconnect/reconnect gate is defined but remains unrun; no live
hotplug evidence is claimed here.

## 2026-08-09: Restored Hagia state now drives a bounded private refresh

Hagia now exercises the retained policy-to-session refresh path instead of
merely decoding it. After a private checkpoint is restored, reconciled against
a complete snapshot, and committed once, the client advances the last admitted
private generation and emits one geometry-free `PolicyDirty` scoped to the
complete live output set. Ordinary actions do not create redundant refreshes,
and a pending session operation settles before the refresh is sent.

The independent socket test observes projection transaction 1, dirty
transaction 2, and the generation-2 projection at transaction 3. Hagia's full
verification task and Sophia's Rust/C/Nim/X11 client matrix pass. The installed
smoke and two-output physical gates now require the post-restart refresh
diagnostic; they remain evidence definitions until an authorized live run.

## 2026-08-09: Public presentation state reaches the X client boundary

The public-policy path now carries each changed fullscreen, maximized,
minimized, or ordinary state through a dedicated bounded frontend control.
X Authority atomically installs `_NET_WM_STATE` and ICCCM `WM_STATE`, routes
selected `PropertyNotify` events, flushes the socket records, and acknowledges
only afterward. The owner keeps the policy candidate staged until that exact
acknowledgement. Timeout or invalidation queues the last committed state as a
separate ordered restoration control.

Those properties become Engine-owned after the first state delivery. Client
replacement, deletion, and delete-on-read are rejected or suppressed, so a
client cannot make later same-state policy commits skip necessary correction.
The protocol-neutral state remains in Sophia/Hagia; X atoms and EWMH rules stay
inside the frontend.

Focused tests cover atomic little-endian property values, invalid combination
rejection without partial mutation, the layout acknowledgement barrier,
rollback, and a routed Unix-socket client observing exact property events and
values while its overwrite and deletion attempts fail with `BadAccess`.
`PolicyLifecycle.tla` already abstracts this as frontend settlement; its
correspondence comment now names presentation acknowledgement. The installed
physical Hagia gate remains required before the roadmap item closes.

## 2026-08-08: Native Hagia policy closes the pre-physical reducer slice

The critical-path profile is now one fixed nine-view scroller rather than an
attempt to reproduce Triad's full runtime before the public boundary settles.
Hagia retains stable logical window/output identities, bounded focus and
minimize histories, output focus, consume/expel and size actions, floating and
fullscreen/maximize/minimize state, and completed Engine-reduced pointer
geometry. General tag mutation, scratchpads, continuous pointer phases, and
additional layouts remain outside this promotion slice.

Revision-1 now carries the missing lifecycle facts directly. Snapshots and
proposals name one explicit active output; requests carry the strictly admitted
private policy generation; and committed bindings explicitly reference an
optional advertised session-operation slot. Numeric action ranges no longer
confer session authority. Canonical validation requires an active-output switch
to replace both old and new outputs, fullscreen geometry to equal complete
output bounds, nonfullscreen geometry to remain in the work area, and a
minimized surface not to hold focus. Minimized placements remain semantic but
do not enter the render-layer candidate.

Idle `PolicyDirty` admission is generation-fenced and output-bounded. Pending
scopes coalesce, while a newer generation arriving during an in-flight refresh
remains pending for a later complete cycle. Reducer and layout successors still
promote only after frontend settlement. Hagia's independent Nim codec and
reducer implement the same contract, and its private checkpoint is bounded,
validated, atomically replaced, and reconciled against the next complete
snapshot. Sophia supplies that checkpoint inside the owner-only endpoint
directory so it survives supervised child replacement but not session teardown.

`PolicyRefreshLifecycle.tla` passed independently and in the pinned complete
TLA+ gate. Its temporary non-atomic active-output control produced the expected
counterexample. `PolicyOperationBinding.als` and
`PolicyPresentationGeometry.smt2` passed the official Alloy 6.2.0 and pinned Z3
4.16.0 gate, including satisfiable weakened attacks; the local Z3 5.x
differential matched. The remaining promotion evidence is an opt-in installed
physical run proving checkpoint restore, presentation transitions, active
output, and refresh behavior without losing the standing application scene.

## 2026-08-08: Public-policy recovery is phase-addressable

The previous Hagia restart smoke killed the client after it submitted a
projection. That proved supervision and eventual reseeding, but timing could
not establish whether the owner had only staged the reducer successor, had
installed a frontend layout, or had already queued the terminal outcome.

The live owner now admits one explicit bounded proof control with four named
boundaries: `proposal_staged`, `frontend_pending`, `prepared`, and
`terminal_outcome_queued`. The control requests the ordinary supervised
transport restart and is consumed once; it does not mutate reducer or layout
state directly. `tools/hagia_owner_settlement_fault_smoke.sh` ran every point
against real Hagia and Kitty. Both settlement-bearing cases recorded
`settlement_aborting`, exact layout timeout/abort, epoch-2 restart, later
startup readiness, and clean session/layout health. The staged and terminal
cases also restarted once and drained cleanly.

The complementary `PolicyOutputSettlement` model covers the remaining
topology mechanism before dynamic output ingress exists. An output loss or
identity return advances the canonical scene, a stale prepared candidate
cannot replace either half of the last-good reducer/layout pair, and return
increments the output generation. TLC explores 86 generated and 64 distinct
states to depth 13. Removing the final topology recheck produced the expected
seven-state stale-commit counterexample; suppressing generation advancement
produced an output-ABA counterexample in three states.

## 2026-08-08: Native policy replaces xmonad as the promotion path

The retained xmonad/QEMU/physical evidence remains valuable, but continuing to
make its practical soak block the public policy protocol would let one
classical X11 WM profile shape Sophia's universal boundary. Milestone 13 is now
the active promotion path: Sophia hosts and authenticates `sophia_wm_v1`, and
standalone Hagia is its first installed policy client. xmonad returns after
native recovery is solid, through the generic compatibility adapter.

The first revision-1 implementation slice adds typed configuration, policy
dirty, session-operation, interaction-kind, and interaction-cancel semantics.
The endpoint can be bound before a supervised process starts and then narrowed
to that exact UID/PID. Configuration rejects invalid chrome, ambiguous actions
or chords, unsupported modifiers, and the emergency chord before routing.
The canonical projection reducer now validates against a cloned successor and
promotes that staged state only if the connection, request, scene generation,
and prior commit remain current. This is the reusable frontend-settlement seam;
the installed owner loop still needs to drive it from actual configure and
renderable-content completion before the native session can be promoted.

The expanded `PolicyLifecycle` model covers configuration activation, ordered
actions, replaceable interaction geometry, cancellation, opaque operations,
frontend settlement, disconnect, and queue saturation. With a one-entry queue,
TLC reaches the saturated transition and proves that the later shortcut is
consumed as a bounded rejection rather than silently reordered or admitted.
The complete formal suite and Rust workspace pass.

Hagia's checked-in native profile creates nine output-local views over nine
shared tag slots. Sharing slots is required: allocating nine unique tag bits
per output would exhaust a 64-bit mask before the protocol's sixteenth output.
The private model now proves all sixteen outputs, direct view activation,
move-to-view, and ordered repeated actions without exposing tag machinery on
the Sophia wire.

## 2026-08-08: Desktop status rides the layout commit

Engine cannot publish desktop status. Snapshots carry no workspace, tag, or view,
and 13.3 replaces workspaces with output projections outright. Only the policy
process knows. Left there, every window manager grows a shell-facing socket and
every shell grows a backend per window manager. Noctalia carries nine such
backends behind one interface, 12,435 lines, and that is the cost of the
alternative.

The decision is to attach an indicator descriptor to the layout proposal. Engine
commits it with the geometry and republishes it verbatim, never interpreting it.
No policy process serves a socket. That rule is now recorded in the load-bearing
ownership rules in `docs/architecture.md`.

The descriptor is deliberately not a workspace record. A scrolling policy has
columns and a kiosk has nothing, and forcing either into a workspace schema
produces a lie or a side channel. An indicator is an ordered labelled slot with
state flags and an optional action token. A shell renders slots and submits
tokens without learning what a workspace is. Noctalia's independently derived
`{id, name, coordinates, index, active, urgent, occupied}` fits inside that
without becoming the schema.

Two properties that an earlier design had to enforce now fall out of the
mechanism. A rejected proposal discards its indicators with its geometry, so no
observer reads a tag the screen never showed. Engine holds the descriptor, so
Engine clears it when the connection epoch changes and a replacement policy
cannot inherit its predecessor's published state. `ShellObservation.tla` refuted
the previous design in five steps when either explicit rule was removed; the new
design needs neither rule.

One consequence is scheduling, not design. `ProjectionBegin` must declare every
category count, so indicators require an `indicator_count` field there. Adding a
record kind is additive; adding a field to an existing message layout is not.
After 13.4 freezes `sophia_wm_v1`, this becomes a new interface family. It must
land in revision 1.

Rendering splits into tiers, which also resolves a standing contradiction between
`docs/architecture.md` and `docs/sophia-policy-ipc.md` over whether a shell is
compositor chrome or an external client. Both, at different tiers. Engine chrome
draws indicators at tier 0 and covers a status bar's whole job with no client
interface; `sophia_shell_v1` remains tier 1 for shells that need more. Tier 0
also removes the unresolved 64-KiB texture question from the critical path, since
that constraint binds tier 1 alone.

Contract and permanent bounds are in `docs/sophia-indicator-descriptor.md`.

## 2026-08-08: A driving client will supply the shell vocabulary

`sophia_shell_v1` waits on retained shell workflows to establish its smallest
useful display-list, hit-target, presentation-data, and action vocabulary.
Nothing supplies that today. xmobar is the only shell-like client with retained
evidence, and it is static text with no hit targets, popups, or animation.
Specifying against it would produce an interface too narrow to carry a desktop.

The decision is to derive the vocabulary from a complete external shell rather
than from first principles, and to select that shell by one criterion: it must
already keep a retained tree of typed drawing primitives, because that is the
artifact a display-list protocol standardizes. Noctalia qualifies. Its
`src/render/scene/` holds rectangle, text, glyph, image, effect, and
hit-area nodes, and its bindings enumerate 25 protocols a real shell needs.
A first comparison against `docs/compositor-graphics.md` shows the proposed
vocabulary already covers eight of its node kinds and omits nine, of which four
are per-widget visual novelty that should stay client-rasterized.

Xfce was considered and assigned elsewhere. It draws through GTK3 and Cairo, so
it can never emit a display list and cannot falsify any decision in this
interface. It is strong evidence for X11 compatibility completeness instead —
EWMH, work-area reservation, and tray/XEmbed — and belongs in the classical
compatibility profiles beside i3, dwm, and qtile.

Ordering is sequential: `sophia_wm_v1` freezes at 13.4 before shell modeling
starts. Hagia has already proved the specification pipeline end to end in a
third language, and repeating that machinery concurrently on a second interface
would reopen shared framing decisions while they need to settle.

One question this raised is sequencing-critical and remains open. A shell that
rasterizes its own novelty must upload textures, but a frame is capped at
64 KiB with one transfer in flight over a bytes-only wire. A 1920x40 ARGB bar
is roughly 307 KiB, and continuously animated content is not expressible at
all. Content-addressed cached textures may be sufficient; a shell-role
descriptor channel may not be avoidable. The envelope is role-neutral and each
role negotiates its own family, so this is implementation coupling in
`sophia-runtime` rather than wire-format lock-in — but the answer belongs
before the freeze, not after. Recorded as an analysis item in 13.2.

Full reasoning, capability tables, and the vocabulary delta are in
`docs/sophia-shell-v1-direction.md`.

## 2026-08-08: The workspace/admission successor passes physically

Installed normal-session run `0055` binds its checksummed archive to release
`0.1.0-a2fdf4f69dfb` and commit `a2fdf4f6`. The automatic login, runtime-
identity, and lifecycle verifiers all pass: Sophia reached both outputs in 305
milliseconds, returned through normal logout after 350,850 milliseconds, and
left no application group, frontend worker, namespace, Xauthority file,
in-flight presentation, or pending WM/input work.

The operator confirmed that both `glxgears` and `vkcube` animated correctly.
Their two animated workload surfaces independently retired 5,367 and 5,384
frames, while the bounded cadence summary observed 8,192 advancing intervals
and no nonadvancing interval before its sample counter filled. Kitty and
Firefox action launches reached PresentedBuffer admission, and the session
retained seven workspace-away projections followed by visible workspace
returns. Forty-two WM projections committed with zero transport rejection,
stale response, or pending request. The reduced log contains no hidden-surface
configure/render command, layout timeout, resize abort, or WM restart; final
session and layout health are clean. This closes the short successor gate and
makes the two-hour interactive soak the next promotion gate.

## 2026-08-07: Ten automatic installed lifecycles pass consecutively

The one-shot installed runner completed and independently reverified runs
`0044` through `0053` on `883666a2`. Every cycle used the runner's bounded
uinput keyboard only to arm recovery and send the normal logout chord, reached
exact two-output readiness in 288 to 324 milliseconds, recorded both connected
outputs in runtime identity, exited without emergency recovery, drained owned
resources, and restored the local VT. The aggregate verifier binds all ten
checksummed archives to one commit and reports `through=0053`. This closes the
repeated ownership lifecycle gate without equating it with repeated PAM login.

## 2026-08-07: Independent emergency recovery passes the color candidate

Installed emergency attempt `0004` passes on the same `883666a2` runtime as
the canonical TrueColor proof. The independent input guard and the live owner
both observed the single `Ctrl+Alt+Backspace` chord. Sophia released and
drained routed keys, drained native presentation without abandoned scanouts,
completed the graceful status-130 emergency handoff, removed owned frontend
and application state, and restored the original KD mode and termios exactly.
Runtime identity records both connected outputs, and the automatic schema-4
archive passes without modification. The earlier `0003` failure remains an
immutable diagnostic from superseded commit `a50dfb67` and does not describe
the promoted candidate.

## 2026-08-07: Immutable color evidence survives verifier correction

Automatic TrueColor attempt `0001` exited normally and proved the real Kitty
DMA-BUF path and the X Authority's exact core color round trip, but correctly
failed promotion. The proof client had placed its palette at global x=2800 on
output 2. The present classical-WM compatibility path builds its retained
client scene for output 1; output 2 owns an independent startup baseline but
does not yet receive active client projection. The renderer therefore rejected
the global palette rectangle as outside its output-local composition target,
and no later output-2 frame could retire.

The corrected gate keeps both real clients inside output 1, where their final
regions can be correlated with actual native submissions and retirements, and
continues to require output 2's nonzero startup baseline. This proves TrueColor
through the implemented boundary without pretending to close active
cross-output projection. The attempt also exposed a diagnostic-label defect:
the session banner still prints its legacy `terminal=xterm` constant even when
Kitty is the configured application. The verifier now identifies Kitty through
its selected PresentedBuffer/DMA-BUF evidence and immutable runtime identity;
changing that banner belongs to a separate schema correction.

Corrected attempt `0002` from commit `c62eabd6` then recorded the exact palette
populations, chromatic Kitty DMA-BUF region, causally next output-1 submissions
and retirements, both-output startup, normal logout, clean ownership drain, and
exact TTY restoration. Its automatic verifier nevertheless rejected the run
because one regular expression assumed `outputs_ready` preceded `presented` in
a structured startup record. Both fields were present with the required values
in the opposite order.

The verifier now parses those fields by name. The run-set gate may also
re-adjudicate an immutable exit-zero `reason=session_verification` record under
the current verifier. It does not rewrite the archive and does not admit a
session-exit failure, another failure reason, a checksum change, or evidence
that fails any current semantic check. Attempt `0002` consequently closes the
physical TrueColor gate without asking the operator to replay an already valid
physical sequence.

The operator subsequently ran the corrected installed gate once more. Attempt
`0003` from commit `883666a2` passed at capture time and under the run-set gate
with `reverified=0`. It independently reproduces the exact palette, Kitty
DMA-BUF, two-output startup, native retirement, logout, ownership drain, and
TTY-recovery evidence. Attempt `0003` is therefore the canonical promotion
record; attempts `0001` and `0002` remain immutable diagnostics of the
cross-output placement and verifier-ordering defects.

## 2026-08-07: Color promotion measures a real X11 region before scanout

The physical TrueColor gate cannot rely on visual inspection or a screenshot
whose capture path is unrelated to the frame sent to KMS. The installed proof
therefore starts two ordinary clients. A small X11 client in the packaged
Sophia executable validates fixed-colormap allocation and query behavior,
draws an asymmetric RGB/CMY/gray palette through core `PutImage`, and requires
an exact `GetImage` round trip. A normal packaged Kitty independently renders
a 24-bit ANSI sample through its DRI3 DMA-BUF path.

The native renderer's opt-in composition trace now reads both the complete
framebuffer and the exact rectangle just drawn. Generic channel-population
metrics distinguish red, green, blue, yellow, cyan, magenta, gray, and other
pixels without learning X11 identities or application metadata. Unequal bar
widths make every expected population unique, so a channel swap, collapse, or
contamination fails deterministically. The palette and Kitty stay inside the
implemented primary-output projection, and each final rectangle must precede
a matching output-1 submission and KMS retirement. Output 2 independently
retains its nonzero startup baseline. The proof repeats only final-region
readback; it does not enable the older full-frame-after-every-layer diagnostic.
Ordinary sessions keep the previous cost and privacy boundary.

The same work closes the focused xmobar gate without repeating a physical
sequence. Checksummed xterm attempt `0003` already contains one exact 14-pixel
reservation on each output, ten exact 2560-by-14 primary repaints, fourteen
primary retirements, packaged xmobar identity, normal logout, and clean
recovery. A new archive verifier binds those facts to the existing immutable
record, while mutation fixtures reject a wrong repaint extent or unreduced work
area.

## 2026-08-07: The installed xterm successor gate passes

Installed release `0.1.0-7e18ea3a01e6` completed automatic xterm attempt
`xterm-runs/0003` with a passing immutable result and matching release,
executable, runtime, and two-output identities. Xterm committed a
2556-by-1422 CPU backing snapshot at `2,16`, exactly inset inside the primary
2560-by-1426 work area. The native owner drained before the VT switch and
reacquired both outputs without abandoning scanout.

The recovery evidence follows the corrected ownership contract: the imported
renderer-image count remained exactly zero, while the Engine rehydrated two
nonzero output frames from its retained CPU scene. A new primary frame retired
after seat reacquisition. Super-Shift-Q then committed normal WM logout with no
unexpected protocol errors, no degraded state, exact KD and termios recovery,
clean namespace and X Authority teardown, and no Sophia, xmonad, xmobar, or
xterm process residue. This closes the installed xterm/work-area successor
gate; failed attempts `0001` and `0002` remain retained as launcher and verifier
regressions.

## 2026-08-07: The xterm gate must verify CPU-scene recovery

Installed xterm attempt `xterm-runs/0002` launched correctly, committed a
2556-by-1422 backing snapshot inside the primary work area, switched VTs with a
drained native owner, reacquired both outputs, retired new primary frames, and
logged out cleanly. The automatic result nevertheless failed because the new
fixture had modeled xterm as a Present client with one imported renderer image.
The live contract is deliberately different: xterm's ordinary drawing commits
an X Authority CPU backing snapshot, while the Engine scene retains those
pixels across renderer replacement. No imported image exists to capture.

The successor gate now records the atomic source and target geometry when a CPU
backing snapshot is admitted. It requires that commit to reach native retirement
before startup readiness, requires an exact zero-image renderer handoff, and
records the nonzero Engine scene rehydrated on both outputs before post-resume
retirement. This proves recovery without depending on a static client to issue
a new Present or repaint after VT reacquisition. The imported-image handoff
contract remains covered by Firefox and Vulkan rather than being imposed on
the CPU-only terminal path.

## 2026-08-07: Terminal roles do not imply Kitty command-line syntax

Installed xterm attempt `xterm-runs/0001` proved the new profile-specific
ledger and exact executable identity, then failed before X11 admission. The
session registry launched `/usr/sbin/xterm` with Kitty's `--config NONE` and
`--override` arguments. Xterm rejected the first option, and Sophia correctly
failed startup when no primary frame arrived. Renderer, X Authority, xmonad,
and work-area code were never reached.

The shell launcher had treated the protocol-neutral `terminal` application
role as if it also selected Kitty's command-line grammar. Terminal adaptation
is now explicit. A small passive helper resolves only the supported `kitty`
and `xterm` kinds, appends their disjoint base and title arguments, and rejects
an unknown kind before takeover. The installed xterm wrapper pins its kind;
Firefox proof profiles continue to require Kitty because their checkpoint
scripts use Kitty's executable-tail convention.

The regression requires xterm's `-cm`, `-dc`, and `-title` spelling, rejects
every Kitty-only option in the xterm vector, and asks a real xterm binary to
parse that vector when available. Packaging carries the adapter beside the
session launcher. This is installed-launch policy, not Engine or X Authority
state, so it does not change the transition model. A new immutable install and
physical attempt remain required.

## 2026-08-07: The xterm successor gate has its own installed contract

The earlier xterm pass on `56dad4de` proves the intended physical shape but
predates the installed `4c312142` renderer-handoff successor. Recording the
next xterm launch as an ordinary login would preserve its logs while allowing
the generic cycle verifier to ignore terminal identity, work-area geometry,
and retained-image restoration. That is not a sufficient regression gate for
the auxiliary-pixmap failure that originally blocked startup.

The installed artifact now exposes `sophia-xterm-proof` as a text-VT command,
without adding a seventh greetd choice. It selects xterm and reserves a
schema-4 `record_kind=xterm` attempt before graphics takeover. Runtime identity
records xterm's historical `-version` output and executable digest; the common
identity verifier remains backward-compatible with archives created before
that field existed, while the xterm run-set verifier requires it.

The dedicated session verifier derives both work areas from runtime geometry,
requires one bounded top reservation on each output, and proves that xterm's
source pixels match a symmetrically inset target inside the primary work area.
It also requires ordered renderer capture, drained VT quiescence, equal-count
restore, seat reacquisition, and new xterm pixels plus a primary retirement
after resume. Normal logout, zero unexpected protocol errors, drained native
and application ownership, an untriggered guard, and exact KD/termios recovery
remain mandatory. Fixture mutations reject missing work-area, presentation,
handoff, resume, logout, cleanup, and identity evidence. A fresh installed
physical run remains the acceptance boundary.

## 2026-08-07: Installed Firefox and VT-handoff gate passes

Installed release `0.1.0-4c3121421f12` completed automatic Firefox attempt
`firefox-runs/0002` with a schema-4 `record_kind=firefox` pass and exact runtime
identity for both connected outputs. The physical VT cycle captured one
retained renderer image, drained scanout without abandonment, restored that
image after seat reacquisition, and retired a new primary-output frame. The
former `WorkerPending` handoff failure did not recur.

The same run retained two independently interactive Kitty processes across
exactly two Firefox launches. Firefox completed the loaded, keyboard, scroll,
layout, refocus, and dialog stages once each. Its first process exited normally
through Ctrl+Q; its second exited through the WM close action. One initial
Firefox admission timed out, performed the permitted single WM restart,
reseeded committed layout before replaying pending admission, and converged
without standing-target or geometry debt. Normal logout then completed with
`protocol_errors=0`, `unexpected=0`, no pending input, actions, or WM work,
clean renderer and frontend ownership, one-percent runtime-tmpfs use, and no
retained proof profile. The installed aggregate verifier passed the immutable
archive and release identity.

The physical Firefox verifier now makes the VT evidence normative for this
gate. It requires ordered queue, prepare, renderer capture, drained quiescence,
seat release/acquire, equal-count renderer restore, active resume, and a
post-resume primary retirement; it rejects forced detach. Fixture mutations
remove capture and falsify the restore count, complementing the worker-level
race regression. A future browser pass therefore cannot hide a broken VT
renderer handoff.

## 2026-08-07: VT handoff must settle detached renderer work

Installed Firefox attempt `firefox-runs/0001` proves that release
`0.1.0-7a6be56c6b29` selects the dedicated schema-4 Firefox ledger. The attempt
then failed four seconds after startup, before Firefox launched, when the
physical Ctrl+Alt+F2 path queued and prepared VT target 2. KMS startup and
retirement were healthy, the runtime tmpfs remained at one-percent use, and no
proof profile survived teardown. The terminal error was `WorkerPending`.

VT suspension drained native scanout and detached the skipped Present. It then
exported retained renderer images for switch-back recovery while the renderer
worker still held the detached frame as its in-flight result. The result was
already irrelevant to presentation, but image export rejected any in-flight
work rather than collecting it. The earlier maintenance correction covered
image clearing during final teardown, not retained-image export during VT
handoff.

Renderer-image maintenance now has one settlement path shared by export and
clear. It waits within the existing bounded maintenance deadline, discards an
exported lease only after the Present has been detached, clears the associated
worker-frame classification, and then reads the older promoted image set.
Worker failure or stall remains fatal. A deterministic worker regression
submits one frame and immediately enters image export; it requires the real
backend failure and rejects the former `WorkerPending` result. This refines one
backend worker step without changing handoff admission or cross-authority
ordering, so the existing transition reducers remain the applicable model and
no TLA+ state expansion is needed.

## 2026-08-07: Installed proof profiles require profile-specific ledgers

Installed release `0.1.0-ce494942fb32` reclaimed the stale Firefox profiles on
the next launch and removed its current profile during teardown. Physical run
`0042` left `/run/user/1000` at one-percent use with no `firefox-m10.*`
directories. It also retained `protocol_errors=0`, `unexpected=0`, clean layout
and renderer health, normal logout, and complete frontend and resource drain.
This closes the `GetImage` and proof-profile resource regressions.

The installed result still reported `session_verification`, but the archive
was under the ordinary XMonad run ledger and had been judged by the generic
desktop verifier. The `Sophia Firefox Proof` wrapper passed the proof argument
without selecting a Firefox attempt mode, so every physical Firefox run was
misclassified regardless of its contents. Applying the dedicated verifier to
run `0042` exposed the actual workflow failure: six action-launched Firefox
processes instead of exactly two, with incomplete Kitty retention checkpoints.

The installed Firefox entry now selects a Firefox attempt mode before invoking
the common session wrapper. That mode reserves and finalizes a schema-4
`record_kind=firefox` archive under `promotion/firefox-runs`, applies the
dedicated browser verifier, identity check, and normal-lifecycle verifier, and
emits `sophia_installed_firefox` as its result. The manual Firefox recorder
remains available for compatibility, and the aggregate verifier accepts both
legacy archives and the stricter automatic schema. A fake installed-release
regression uses evidence that deliberately fails the generic desktop verifier,
proving that a passing Firefox attempt cannot silently take that route. The
exact two-launch contract is unchanged.

## 2026-08-07: Firefox proof profiles are session-lifecycle resources

Installed release `0.1.0-fb1c38046d37` cleared the core `GetImage` failure.
Physical run `0041` completed with `protocol_errors=0`, `unexpected=0`, clean
layout health, normal logout, and clean frontend/resource teardown. Its
installed-run verifier still failed because the startup Kitty, selection, and
held-repeat proof stages did not complete. Firefox reported
`NS_ERROR_FILE_NO_DEVICE_SPACE` while writing its isolated profile.

The user runtime tmpfs was full: 44 prior `firefox-m10.*` proof profiles used
6.2 GiB under the Sophia session directory. The launcher created one isolated
profile for every proof run but never removed it on normal, failed, or
emergency teardown. The profiles are mutable test inputs, not retained
evidence; the reduced session log and installed-run archive already carry the
proof result.

Profile ownership now follows the session process lifecycle. After proving
that no prior wrapper or graphical session is active, the launcher removes
only stale `firefox-m10.*` directories beneath its exact private runtime
directory. It creates the current profile only after installing the cleanup
trap, and removes that exact directory after terminating supervised children.
A launcher regression locks in trap ordering, stale-profile reclamation, and
current-profile cleanup. The next installed Firefox run will reclaim the
existing backlog automatically; no manual cleanup sequence is required.

## 2026-08-07: GetImage replies are not bounded like image-upload requests

Installed release `0.1.0-53a213655a41` corrected committed-layout reseeding.
Physical run `0040` reasserted the stale Kitty geometry, admitted Firefox,
advanced the automated workflow through the dialog stage, retained correct
mixed presentation, and reached normal logout. The strict verifier rejected
the otherwise complete run because it observed eight X protocol errors. The
first was `BadValue` at sequence 414 for core major opcode 73 (`GetImage`).
Firefox also reported that its background page-thumbnail request failed.

The wire decoder was the root cause. Core `GetImage` is a fixed 20-byte request,
but Sophia computed the potential reply as `width * height * 4` while decoding
it and rejected anything above the 256 KiB `PutImage` request-data limit. A
normal Firefox readback around `1290x1050` is roughly 5.4 MiB, so a valid
request was rejected before drawable validation or readback. The request and
reply bounds are different protocol concerns.

XLibre's `DoGetImage` validates `XYPixmap` or `ZPixmap`, drawable access,
viewability, and bounds, then streams the computed reply through a bounded
intermediate buffer. Yserver retains the same request validation and computes
the format-specific reply length without applying a request-body ceiling.
Sophia now follows that division. The decoder validates only the fixed request
shape and legal format. X Authority derives a checked ZPixmap or XYPixmap
layout from the advertised depth, scanline pad, plane mask, and client byte
order; rejects invalid drawables, matches, formats, or allocations with the
corresponding X error; and caps authority-owned CPU image memory at 64 MiB.
Core X11 and MIT-SHM use the same validation, passive software-buffer readback,
and pixel packer. Missing CPU backing remains deterministic zero-filled data;
this change does not add a GPU screenshot path or move X11 semantics into
Engine.

Regressions cover both byte orders, a Firefox-sized decode, ZPixmap and
XYPixmap layout, empty replies, pixel preservation, drawable/access/bounds
errors, and allocation refusal. A real Unix-socket test reads a 320,000-byte
reply, above the retired ceiling, and the compiled Xlib smoke now performs
`XPutImage` followed by `XGetImage` and verifies the returned pixel. This is a
request/reply implementation boundary rather than concurrent authority state,
so it does not require a new TLA+ model. Physical promotion remains pending a
new installed Firefox run with zero protocol errors.

## 2026-08-07: Recovery reseed must reassert stale client pixels

Installed release `0.1.0-a50dfb672794` received Super+F and launched Firefox,
but did not complete admission. The first three-surface layout configured both
Kitty surfaces from `1276x1422` to `636x1422` and targeted Firefox at
`1276x1422`. It narrowly timed out before the new Kitty pixels arrived, then
correctly restored the committed rectangles. The aborted `636x1422` Kitty
frames retired after that rollback. The committed-layout reseed therefore held
two `1276x1422` pixel obligations while Engine already retained those exact
rectangles.

The full-geometry correction derived X controls only from an Engine rectangle
change or an admission-owned candidate. It did not include an ordinary resize
obligation whose target rectangle already matched Engine. Consequently every
reseed waited for `1276x1422` pixels but emitted no ConfigureSurface for either
Kitty, timed out, restarted xmonad, and repeated. The live run recorded ten
restarts, a dropped focus handoff, and emergency exit. Super+F routing and
process launch were not the failing boundary.

Geometry-control derivation now also includes every retained resize
obligation. That preserves the separation between `moved_surfaces` and pixel
readiness while ensuring a rollback/reseed can reassert the target even when
logical Engine geometry is unchanged. A deterministic regression reproduces
the exact `Engine=1276`, `pixels=636`, `requested=1276` state and requires one
full-rectangle control with `moved_surfaces=0`. The physical recovery canary
requires every committed-layout reseed control to receive its correlated X
Authority acknowledgement. The installed release remains failure evidence; a
new immutable successor must repeat Super+F once before broader gates resume.

## 2026-08-07: Move feedback is a full-geometry X Authority operation

Installed release `0.1.0-7bd3e7db0a90` proved the two-phase admission
successor, then exposed the next independent boundary defect. Super+Space
committed xmonad transaction 9 with three moved surfaces, but the session
reported only two configure deliveries and two matched resize candidates.
Those two Kitty surfaces changed size. Firefox retained its `1266x1412`
content while moving from the right column to the left, so the old size-only
control path sent it nothing. Engine rendered Firefox at `1266x1412_7_21`
while X Authority retained the old root-relative position. The proof page's
`screenX` therefore stayed stale and the layout stage did not advance. Later
pointer-focus transactions moved no surfaces; they merely made the split
Engine/X geometry visible as apparent Firefox jumping.

This is not xmonad policy or an Engine rendering defect. XLibre's
`ConfigureWindow` emits `ConfigureNotify` for a real pure move and invokes the
Present configure hook before core delivery. Yserver retains the same
position-and-size operation and has a pure-move ordering regression. Sophia's
X Authority now follows that ownership: `ConfigureSurface` carries the whole
logical rectangle, updates X position and size together, emits Present
ConfigureNotify before core ConfigureNotify for any real change, and remains
silent for an identical rectangle.

The session coordinator separately derives geometry controls and pixel
obligations. Every changed surface receives one full-rectangle control, while
only resized surfaces await new pixels; a move-only surface keeps its committed
pixels. Timeout recovery queues the complete last-committed rectangle, so a
late target control cannot leave X Authority at a stale position. Focus-only
proposals emit no geometry control. Deterministic Rust tests cover pure move,
no-op silence, move-only layout, focus-only layout, and full-rectangle
rollback. The `GeometryFeedback` TLA+ model explores delivery on either side
of logical commit plus late-target/FIFO rollback and requires terminal
Engine/X convergence. The physical Firefox verifier now requires the
correlated geometry acknowledgement and stable Present target through the
focus cycle. Installed `a50dfb67` proves this boundary reaches the live
session, but exposes the recovery-reseed omission recorded above.

## 2026-08-07: Admission recovery now has a deterministic successor phase

The installed `1a7d67c3` session reproduced the short, black Firefox window on
Super+Space. Its fallback Present retired at `1280x1040`, but admission kept
that recovery extent because the standing `1276x1422` target was still unmet.
The same temporary extent then constrained every ordinary xmonad relayout back
to the fallback. A target Present could not become the bounded successor while
the fallback candidate was still awaiting retirement, so the only previously
working route was an unarmed-retirement timing race.

Recovery is now explicitly two phase. Exact fallback retirement makes the
surface managed, removes the temporary constraint while retaining its pixels,
preserves the standing target, and queues one normal relayout. The visual
tracker permits one fallback and one distinct logical-target successor per
surface, rejects repeated successors, and requires exact native-retirement
identity before the target changes committed layout state. Session completion
also fails closed on a remaining standing target, not only a recovery extent.
The `AdmissionRecovery` TLA+ model explores target observation before and after
fallback retirement and proves one relayout, constraint release before target
commit, exact target retirement, and eventual convergence.

The packaged xmonad order is restored to the user's established
`ThreeColMid`, `Tall`, `Mirror Tall`, `Full`, `Spiral` sequence. The promotion
page no longer assumes the focused Firefox surface must resize on the first
cycle: it accepts an outer position or size change, while the full M8 proof
retains its resize-specific checkpoint. The strict verifier correlates Firefox
from its launch transaction, requires one moved layout, every affected exact
retirement, three visible managed surfaces, a post-action Firefox Present, and
clean recovery. Deterministic Rust, proof-page, canary, verifier, configured
xmonad, and TLC gates cover the recovery change. Installed `7bd3e7db` proves
that recovery phase but exposes the separate move-feedback defect recorded
above; its focused Firefox run is not promotion evidence.

The local source audit also exposed the already-generated `sophia_wm_v1` Rust
wire table above the review threshold. It remains one generator-owned protocol
table, so the exact path now has a temporary cohesion-ledger entry; splitting
the schema generator is separate from this runtime recovery correction. The
new recovery tests were instead split at their actual admission/recovery seam.

## 2026-08-07: The inset-geometry successor is installed

Release `0.1.0-1a7d67c30615` packages signed Sophia commit `1a7d67c3`, the
configured xmonad policy whose first cycle is `ThreeColMid` to `Mirror Tall`,
and xmobar commit `f3d7fb5461c1`. `/opt/sophia/current` resolves to that
repository-independent release. Its manifest, complete `SHA256SUMS`, and
packaged-policy verifier pass against the installed directory.

This closes only the successor's build/install criterion. The previous
`56dad4de` xterm and ten-cycle results remain historical evidence; the new
surface-transaction contract and packaged layout order require fresh focused
physical, lifecycle, color, and soak gates on `1a7d67c3`.

## 2026-08-07: Inset Present content is distinct from outer layout geometry

The live Firefox proof received every Super+Space action, but its standing
resize target never retired. Firefox presented through a child inset by five
pixels on every side: the managed outer surface was `1276x1422`, while the
exact DMA-BUF was `1266x1412`. Sophia had already accumulated the child offset
for rendering, but the layout coordinator treated the raw buffer extent as the
outer extent. It therefore retained the `1280x1040` launch recovery constraint
and reconciled every later xmonad proposal around the stale fallback.

XLibre keeps parent and child window geometry separate through
`ConfigureWindow`, and yserver likewise retains window-tree geometry and
Present offsets as independent facts. Sophia now makes the same distinction at
its protocol-neutral boundary: `SurfaceTransaction::target_geometry` remains
the logical surface rectangle, while `target_content_size` records the exact
buffer extent. X Authority derives the latter before projecting a descendant
Present to its toplevel. The live reducer accepts the logical extent only when
the imported buffer exactly matches that content extent, and the visual tracker
retains both sizes so native retirement cannot be forged by either one. An
inset regression covers exact acceptance, stale-buffer rejection, standing
target retirement, and recovery release.

The checked-in xmonad order also placed `Tall` immediately after
`ThreeColMid`. For the focused master those layouts share the same outer extent,
so one valid NextLayout action resized only the two Kitty surfaces while the
Firefox proof waited for a Firefox resize. The order is now `ThreeColMid`,
`Mirror Tall`, `Tall`, `Full`, `Spiral`; the first action changes the focused
master from vertical to horizontal geometry. The configured real-xmonad smoke
proves the exact sequence. This is policy behavior, not an Engine shortcut or
an application-specific proof exception.

The full Rust all-features suite and configured real-xmonad smoke pass. Because
the executable protocol contract and packaged policy changed, the installed
`56dad4de` xterm and ten-cycle results remain historical evidence; a new
immutable successor must repeat installation and physical gates.

## 2026-08-07: The corrected candidate passes ten installed cycles

The one-shot `Sophia Cycle Gate (Automated)` entry produced ten consecutive
immutable passing attempts, `0026` through `0035`, for installed commit
`56dad4de8b5f76ba0e3999be60a7865053e0c532`. The packaged verifier accepts the
exact endpoint with `sophia-verify-cycles 10 0035`: every archive checksum,
unique launch identity, application digest, two-output startup, input-guard
interlock, normal cycle-runner handoff, TTY recovery, and runtime identity
matches. Startup readiness remains between 297 and 324 ms across the set.

The runner stopped after the tenth success and returned once to greetd. No
manual repair or emergency path was used, and a host-level check finds no
Sophia, xmonad, xmobar, xterm, or monitoring-process residue. This closes the
corrected candidate's automated lifecycle gate; focused application and layout
proofs, visible TrueColor, the interactive soak, and the workday remain open.

## 2026-08-07: The corrected installed xterm startup passes

The installed `0.1.0-56dad4de8b5f` xmonad profile starts xterm without the
predecessor's `CreatePixmap(depth=1)` failure. Both native outputs present,
xmobar reduces their work areas, and xmonad commits the xterm at
`2556x1422_2_16` inside the `2560x1426_0_14` primary work area, including its
two-pixel Engine frame. Stable mixed scanout makes the session ready in 314 ms
with zero X11 protocol errors.

The same run quiesces cleanly for a switch to tty2, restores retained content
on resume, and commits `Super+Shift+Q` through the blind WM path. Presentation
drains with no abandoned scanout, session health and layout health are clean,
all native and X11 ownership reaches zero, the packaged normal-lifecycle
verifier passes, and KD mode plus termios are restored exactly. A host-level
check finds no Sophia, xmonad, xmobar, or xterm residue. This closes the narrow
startup regression only; the remaining focused application, layout, color,
automated-cycle, and soak gates stay open.

## 2026-08-07: The corrected successor candidate is installed

Release `0.1.0-56dad4de8b5f` packages signed Sophia commit `56dad4de`, the
configured xmonad 0.18.1 executable with xmonad-contrib 0.18.2, and xmobar
0.51.1 from clean source commit `f3d7fb5461c1`. Its complete `SHA256SUMS` set
and packaged-policy verifier pass from
`/opt/sophia/releases/0.1.0-56dad4de8b5f`. `/opt/sophia/current` and all six
greetd entries resolve through that repository-independent release; the failed
`0.1.0-199fa11d6876` release remains available as `/opt/sophia/previous` for
comparison.

This closes the corrected build/install criterion only. Runtime identity still
describes the most recently completed session until this release launches. Its
focused physical gates, visible TrueColor capture, repeated lifecycle proof,
and soak evidence remain open.

## 2026-08-07: Auxiliary pixmap depths are separate from TrueColor visuals

The first `199fa11d` installed-session start reached both outputs, xmonad, and
xmobar, then xterm exited on `CreatePixmap(depth=1)` with `BadValue`. The
TrueColor closure had incorrectly reduced all core pixmaps to the two color
visual depths. This is a setup/authority defect, not an Engine rendering or WM
policy failure.

XLibre's `ProcCreatePixmap` admits depth 1 as the core bitmap case independently
of the screen's ordinary visual depths and derives other nonvisual depths from
its pixmap-format table. Yserver advertises the conventional
1/4/8/15/16/24/32 storage formats, retains pixmap depth, and exercises depth-1
masks and stipples. A real xterm smoke confirmed that depths 4 and 8 are probed
before its depth-1 mask. Sophia now uses that shared bounded format family for
setup and request validation, while only depths 24 and 32 have TrueColor
visuals. Pixmap records retain depth and return it through `GetGeometry`;
auxiliary pixmaps never become XRGB Engine surfaces. Both byte orders prove the
exact setup catalog, creation and geometry for every retained depth, and
rejection of depths outside the catalog.

The failed installed release remains useful evidence but is not a promotion
candidate. The corrected successor above supersedes it for all remaining live
gates.

## 2026-08-07: The immutable successor candidate is installed

Release `0.1.0-199fa11d6876` packages signed Sophia commit `199fa11d`, xmonad
0.18.1 with xmonad-contrib 0.18.2, and xmobar 0.51.1 from clean source commit
`f3d7fb5461c1`. The package verifier and complete `SHA256SUMS` set pass from
`/opt/sophia/releases/0.1.0-199fa11d6876`; `/opt/sophia/current`, every
operator-command symlink, and all six greetd entries resolve through that
repository-independent release. The preceding `958fb5e6` installation remains
available as `/opt/sophia/previous`.

This closes only the build/install criterion. Runtime identity still describes
the most recently completed older session until the successor launches. Its
focused physical gates, visible TrueColor capture, repeated lifecycle proof,
and soak evidence remain open.

## 2026-08-07: The promotion policy and TrueColor contract are closed inputs

The Milestone 12 desktop policy no longer depends on mutable home
configuration or executable discovery. Sophia checks in one minimal xmonad
0.18.1 configuration using xmonad-contrib 0.18.2 and only the blind
`ThreeColMid`, `Tall`, `Mirror Tall`, `Full`, and `Spiral` geometry policies.
The configured real-xmonad smoke requires exact three-surface results across
all five layouts, wrap, focus, constraints, release, and restart. The existing
profile suite retains workspace, floating-pointer, work-area, and output-change
coverage. Tabbed decorations, metadata manage hooks, spawn/kill policy, and
dzen control remain excluded.

Release packaging now builds that checked-in configuration and the exact clean
`~/src/xmobar` revision, copies both executables and configurations, and records
their source identities and SHA-256 digests. Installed resolution accepts only
the packaged absolute paths. The package verifier rejects wrong source
versions, missing files, executable or configuration digest mismatch, and an
unrecorded xmobar revision; runtime identity and soak verification cover both
artifacts. Development builders remain available, but they are not installed
fallbacks.

X Authority now owns the complete fixed-TrueColor contract. XLibre's
`miResolveColor`, `AllocColor`, and `QueryColors` establish the retained
semantics: RGB16 allocation takes each high byte, returns that value expanded
by `0x0101`, packs the advertised masks, and supplies an opaque alpha mask for
the 32-bit visual; query rejects bits outside the visual mask and expands each
channel by `0x0101`. XLibre also establishes `BadColor`, `BadMatch`, `BadName`,
`BadValue`, and `BadIDChoice` behavior. Yserver confirms the value of passive
colormap records and a bounded normalized name table, while its permissive
query validation is not copied.

Sophia therefore stores only colormap ownership plus visual identity, never a
mutable TrueColor palette. Window depth/visual/colormap triples must agree,
client colormaps are released on disconnect, allocation and query replies carry
actual channel values, and unknown names fail instead of becoming white. Both
wire orders, XRGB and ARGB allocation, duplicate/invalid resources, advertised
masks, pixmap depth, and a non-gray XRGB upload palette are deterministic
regressions. The remaining color boundary is the visible physical capture on
the successor installed candidate.

## 2026-08-07: The public policy protocol is the extension point

Hagia is Sophia's first planned native WM, not a privileged Engine component or
the definition of Sophia policy. Sophia will publish independently
implementable, role-specific local IPC so other WMs and shells may be written
in any language. The first native proof, the Rust X11 WM bridge, and an
independently compiled C client must use the same wire and semantic conformance
suite.

River supplies the decisive architectural precedent: its compositor and WM are
separate processes joined by a stable protocol, which permits replacement and
hot-swap without moving rendering into policy. Sophia does not adopt River's
Wayland runtime. Unlike River, Sophia has retired its production Wayland
frontend and already has opaque generational IDs, bounded binary framing, and
Engine transactions. Importing a Wayland server solely for WM IPC would add a
second object/runtime model without improving Sophia's narrower blind-policy
boundary.

The wire remains the dependency floor. Clients need no Sophia library,
generator, Rust crate, Wayland stack, CBOR codec, or schema runtime. A narrow
checked-in KDL description will generate retained Rust and C99 codecs,
normative tables, and golden vectors; normal builds do not run the generator.
CBOR remains inappropriate for this authority path because its flexible maps,
duplicate keys, nesting, tags, and multiple equivalent encodings would require
a Sophia-specific restricted profile while buying little for fixed bounded
projection records.

The session, not the policy process, will host owner-only role sockets. This
aligns endpoint admission and hot-swap with session authority. Interface
versions are independent of the common frame version. The current Rust WM API
v7 is experimental; the first stable public family will be
`sophia_wm_v1` after Hagia and the bridge pass the same projection and recovery
suite. Once published, a stable revision remains accepted unless an explicit
security amendment retires it. Old revisions normalize at the IPC edge so
Engine retains one current internal projection model.

Complete scene snapshots and complete affected-output projections are the
semantic baseline. Strict begin/chunk/end transfer permits those records to
cross the existing 64-KiB frame boundary without exposing partial state.
Engine validates and atomically commits the affected logical outputs, preserves
the last projection on every failure, and permits a surface on at most one
output. Hagia privately owns nonempty tag sets, stable `ViewId` values, ordered
per-output views, focus history, reconnect affinity, and a session-local
checkpoint. Engine stores none of them. Mirroring remains a later separate
capability.

Two bounded TLA+ models precede production changes. `PolicyConnection` checks
negotiation, capabilities, transfer assembly, connection epochs, timeout,
disconnect, and replacement. `PolicyProjection` checks snapshots, stale
proposals, validation, multi-output atomicity, focus, removal, and
last-committed recovery. Wire offsets remain codec/golden-vector work rather
than TLA+ state.

The initial model checks exposed two protocol-level ambiguities before Rust
implementation. A transfer keyed only by client and connection epoch collided
with a later transaction on the same connection, so admitted work is identified
by client, epoch, and transaction, and a transaction cannot be reused within an
epoch. Separately, accepting any proposal whose declared base generation
equaled the current scene let a client guess a future generation and become
accidentally valid after a scene change. The session must issue the request for
the current generation, and a proposal must answer that exact outstanding
request. With those requirements, `PolicyConnection` passes 2,177 distinct
states to depth 23 and `PolicyProjection` passes 524,396 distinct states to
depth 18, including safety and liveness checks.

The first retained wire slice now derives ten draft handshake and transfer
message layouts from `protocol/sophia-wm-v1.kdl`. The generator emits the Rust
codec, an allocation-free C99 codec, normative byte tables, and shared valid
and malformed frame corpora. Generation is an explicit developer operation;
the ordinary build consumes only retained outputs. One check rejects generated
drift, round-trips every golden frame through both language implementations,
and requires the same fail-closed result for truncation, bad magic or version,
unknown kind, excessive payload, reserved data, trailing data, and invalid
transaction identity. Transfer ordering and semantic record validation remain
owned by the next bounded-assembler slice rather than the scalar codec.

The next draft slice implements that boundary without changing the installed
API v7 session. A Linux session-owned endpoint creates a new mode-0700 role
directory and mode-0600 WM socket, authenticates the exact supervised UID and
PID through peer credentials, and admits one exclusive client. Its connection
reducer negotiates one epoch, prohibits transaction reuse, accepts one bounded
transfer at a time, verifies ordinals and declared category totals, caps total
assembly memory, and discards complete queued work if its epoch disconnects
before Engine intake. Snapshot and projection assemblers share generated
fixed-width semantic records; all scalar and record bytes still round-trip
through the same retained Rust and allocation-free C99 artifacts.

Engine now has a dormant canonical projection reducer. It validates complete
scene snapshots, exact server-issued request identity, live surface
generations, constraints, geometry, output membership, global surface
uniqueness, and visible focus before replacing all affected outputs in one
mutation. Rejection, timeout, and disconnect preserve committed layout; scene
removal prunes only dead surfaces and invalid focus. A focused adapter converts
an API v7 workspace plan into this canonical shape, but production v7 remains
the installed owner until Milestone 12 promotion permits migration.
Deterministic tests preserve both formal counterexamples and a real Unix-socket
handshake-to-semantic-projection path.

The first ordinary client conversion exposed two missing facts in the draft
wire. A projection request named only its affected-output count, so an
independent policy could not know which complete outputs it had to replace. A
snapshot surface named current geometry but not its committed output, and no
snapshot record named current focus. A new policy therefore could not
distinguish hidden surfaces or reconstruct the active output state. The schema
now carries the bounded affected-output ID vector, an optional current output
(`0` means hidden), and optional per-output focus; regenerated Rust, C,
documentation, and golden artifacts agree on those fields.

Three non-production clients now exercise the corrected boundary. The dormant
Rust reference WM completes an authenticated snapshot/request/projection
cycle. The generic X11 bridge translates a real synthetic-X layout response
through API v7 and then the canonical reducer. A standalone C99 client assembles
the strict snapshot, tiles two opaque surfaces, and has its proposal accepted
by that reducer while linking only the retained C codec and libc. The protocol
gate retains the scalar malformed corpus and adds this live cross-language
cycle.

An initial uncommitted Triad clone was discarded when the project boundary was
clarified. Hagia starts with independent history, a Nim-only manifest, and no
River/Wayland dependency, binary, configuration, or build scaffolding. Its
long-term purpose remains a standalone Sophia port of Triad's useful policy and
experience, but that port is deliberately deferred. Hagia is currently a thin
independent protocol challenger: its decoder passes Sophia's valid and
malformed corpus, and its proof client completes strict snapshot assembly,
projection encoding, and committed outcome through the authenticated socket
and canonical reducer. Its private tag/view model remains incomplete, and none
of these draft paths changes the installed Milestone 12 candidate.

## 2026-08-07: One WM API serves legacy profiles and native Sophia policy

Xmonad is Sophia's first mature classical-X11 compatibility profile and the
current daily-driver promotion vehicle. It is not Sophia's architectural
window manager. Engine exposes one blind, versioned WM policy API. A native
Sophia policy process speaks that API directly; a classical X11 WM speaks only
to the compatibility bridge's private synthetic X server, whose bounded
profile translates policy into the same API. Future i3, dwm, qtile, or other
profiles must each retain their own request and action evidence without seeing
real X Authority clients, metadata, pixels, or physical input.

The immediate xmonad configuration work therefore remains profile-local. The
installed session will compile and package a fixed xmonad executable rather
than loading mutable home configuration, package the exact xmobar executable,
and verify both digests. Geometry-only layouts may enter after deterministic
bridge coverage. Title-aware tab decorations and metadata rules may not widen
the fake X server or leak into Engine. The current Void host already has the
required configuration build and runtime dependencies; dependency installation
is closed.

Hagia remains the intended first demanding Sophia-native policy and shell
family. Its spatial-policy process will own private tags, layout structures,
focus policy, scrolling, and Janet layouts while remaining blind. An optional
`hagia-shell` will own authorized visible furniture through a separate shell
projection. Engine retains hit-testing, input, animation, rendering, and
scanout; session services, portals, and trusted classification brokers retain
launch, lock, capture, transfer, and metadata authority. The direct and legacy
paths must share semantic conformance tests so compatibility work strengthens
rather than forks the WM boundary.

The same roadmap review exposed an incomplete X Authority TrueColor contract.
Sophia correctly advertises 24-bit XRGB and 32-bit ARGB TrueColor visuals and
maps arbitrary `AllocColor` components into the advertised masks. However,
`QueryColors` currently reports every nonzero pixel as white, and
`AllocNamedColor` treats every name except black as white. Completing mask
round-trips, bounded retained-client color names, validation and error paths,
and a non-gray physical pixel proof belongs in X Authority. Engine must continue
to receive only normalized XRGB8888/ARGB8888 pixels and opacity facts.

## 2026-08-07: The active roadmap contains decisions, not evidence transcripts

`todo.md` had grown to 1,474 lines, including 94 checked items and long causal
transcripts from completed Milestones 9 and 10. Several unchecked lines were
stale duplicates of gates already closed by the Milestone 9 promotion record.
That shape obscured the actual promotion boundary and made old diagnostic work
look active.

The active roadmap now retains only open work, current constraints, ordering,
and measurable exit gates. Completed Milestones 9 through 11 and the completed
Milestone 12 precursor gates live in `docs/roadmap-history.md`; causal evidence
stays in this log. Conditional rendering investigations remain explicitly
post-promotion or Milestone 13 work so they cannot silently reopen completed
daily-driver gates.

## 2026-08-06: Lifecycle repetition is a runner, not an operator ritual

The first Milestone 12 instruction asked the operator to select the ordinary
greetd entry and press the logout chord ten times. That contradicted the
existing decision that installed cycles identify distinct launches rather than
repeat broader operator choreography. It also spent human attention replaying
the same shortcut already exercised by physical and QEMU input gates.

The installed artifact now carries one automated cycle entry. After one greetd
selection, its external runner creates a fresh bounded uinput keyboard and a
fresh installed Sophia launch for each cycle. It waits for an exact schema-2,
two-output startup-ready record from the new log inode, then sends
`Super+Shift+Q` through libinput, Engine input authority, and the blind WM. A
per-cycle deadline remains outside Engine. The runner requires a new immutable
attempt, verifies it immediately, stops on the first failure, and runs the
contiguous aggregate verifier before returning to greetd.

Nested lifecycles explicitly record `handoff=cycle_runner`; ordinary,
emergency, and watchdog sessions retain `handoff=display_manager`. This avoids
claiming ten PAM or display-manager round trips when the invariant is repeated
Sophia acquisition and cleanup on one authenticated local VT. The gate still
uses physical DRM, KMS, libseat, and VT ownership. Uinput removes repetitive
human key presses without adding an Engine test mode or replacing the retained
physical keyboard evidence.

The first installed cycle entry failed before graphics takeover. The runner
itself owned TTY7, but Bash redirected the stdin of its asynchronous child to
`/dev/null`; the installed lifecycle therefore reported `vt=other` and
correctly rejected preflight. The runner now opens its controlling VT once and
passes that exact descriptor to every asynchronous installed session. Its
self-test launches a child through the production helper and requires a value
read from the preserved descriptor. Failed runner work directories now move to
a durable, private diagnostic archive instead of disappearing during cleanup.

The first descriptor-preserving rerun exposed a narrower identity error. The
runner reopened its controlling terminal through `/dev/tty`, so the child saw
that generic alias instead of `/dev/tty7`; installed preflight correctly
requires a concrete local VT. The runner now duplicates its original stdin
descriptor. This preserves both the controlling terminal and its kernel device
identity across the asynchronous launch.

The next rerun reached installed preflight on the concrete VT and exposed an
ordering omission in the automation. The production input guard still required
its independent recovery-arm chord, while the runner's virtual keyboard could
emit only the later logout chord. Sophia therefore failed closed before
graphics takeover, and the runner correctly withheld logout injection. A cycle
now uses one bounded virtual keyboard for two ordered phases: after exact guard
readiness it injects Ctrl-Alt-Backspace and requires the new guard's armed
record; after exact two-output readiness it injects Super-Shift-Q. Fresh log
identity checks prevent either phase from accepting evidence from a preceding
cycle. This exercises the production interlock instead of bypassing it.

The armed rerun then reached graphics takeover but never presented its startup
surface. Host process and file-descriptor evidence found four orphaned legacy
WM bridges from earlier releases. Three retained xmonad children, and the
bridges retained Sophia-owned DRM and physical-input descriptors. The seat
broker had duplicated libseat descriptors without close-on-exec; separately,
the bridge socket server retained its runtime and returned to `accept` after
its sole Sophia client disconnected. Those orphans kept kernel ownership alive
after the Engine process disappeared and poisoned the next startup.

Seat-device duplicates are now close-on-exec, and the cycle launcher closes
its extra retained VT descriptor before executing the installed wrapper. The
legacy bridge server owns exactly one Sophia control-client lifetime, removes
its socket on exit, and bounds the preconnection wait. Client disconnect now
drops the bridge runtime and its xmonad child instead of returning to an
unowned listener. A Rust socket-lifecycle regression requires that teardown,
and every installed cycle now rejects preexisting helpers and requires the WM
process set to drain before accepting its immutable attempt.

The first clean-host rerun exposed a separate arm-boundary race. The recovery
guard published `status=armed` when the three keys were pressed, while the
uinput producer wrote its completion receipt only after releasing them. The
runner sampled that receipt once and could reject the cycle during the roughly
30-millisecond release interval, terminating an otherwise healthy startup.
`EmergencyChordAction::Armed` now means the complete first chord has been
released. The runner also waits independently for the producer receipt, so
neither guard observation nor producer completion stands in for the other.

Signed commit `958fb5e6` then passed the complete physical gate. Installed
attempts `0014` through `0023` each reached two-output readiness in 291--336 ms,
accepted the injected normal logout through the production input and WM path,
and exited with status zero. Every recovery record preserved KD mode 0 to 0
and restored termios; no Sophia, bridge, or xmonad process survived. The
installed aggregate verifier accepted all ten contiguous immutable attempts
through `0023`, closing the repetition gate without manual repair or emergency
recovery.

## 2026-08-06: The daily-driver churn gate waits for settled admissions

Signed commit `5fbfc849` passed the strict unattended `xmonad-m8-soak` gate
after the launch barrier began waiting for session-owned admission rather than
an action's incidental layout or focus record. The two-output QEMU session ran
for 1,901,036 ms and completed 25 terminal, Firefox, and launcher cycles, 75
close actions, and 11 scheduled WM-bridge recoveries. Every recovery preserved
layout. The session routed 663 expected input events, recorded 50 expected and
zero unexpected protocol errors, retired 338,220 native page flips without a
rejected callback, and drained with no pending WM, action, input, frontend,
namespace, Xauthority, or native-cleanup ownership.

This is the bounded unattended precursor to Milestone 12, not a substitute for
its physical gates. The strict verifier requires at least 30 minutes, 20 cycles,
60 close actions, two layout-preserving bridge recoveries, the complete Firefox
workflow, clean health and cleanup summaries, and normal guest completion. The
exact revision is packaged as immutable release `0.1.0-5fbfc849fb63` and is now
the `/opt/sophia/current` target. Installation remains separate from the
evidence run, so the next acceptance boundary is physical use of that unchanged
release.

## 2026-08-06: Action launches settle on exact admission

The repaired QEMU soak completed seven full application cycles and four
scheduled bridge recoveries. Cycle eight launched Firefox while a prior
terminal removal and bridge reseed were settling. The harness accepted the
launch action's own no-op layout and focus records as readiness, then sent the
Firefox close chord before Firefox's surface entered policy. Sophia correctly
closed the still-focused startup terminal; Firefox was admitted afterward and
the harness eventually reported a close timeout.

The action-launch barrier now waits for the session-owned schema-2 admission
record. That record is emitted only after the new surface is observed, policy
and visual admission settle, and focus is stable. Generic layout or focus
records cannot satisfy it. The same barrier covers terminal, Firefox, and
launcher churn, allowing future layout/reseed optimizations without reopening
this race. Its regression executes the same basic-grep numeric pattern as the
harness, preventing a regex-dialect mismatch from turning a valid admission
into a false timeout.

## 2026-08-06: Retained X11 remaps do not owe redundant geometry

The unattended M12 QEMU lifecycle reached Firefox focus isolation, then the
xmonad bridge disconnected after waiting three seconds for one synthetic
`ConfigureWindow`. Engine had switched to an unfocused workspace containing
one previously admitted Firefox surface. Its action snapshot already carried
the exact committed node and geometry; xmonad legally left that unchanged
during the remap.

The private facade compounded the wait by deleting its window and stacking
record on `UnmapNotify`. XLibre and Yserver both retain the window, change only
its map state, and leave destruction to `DestroyWindow`. The facade now follows
that lifecycle. A sole-node focus cycle still sends the bounded profile chord
so xmonad's stack converges, but does not require a geometry response that has
no remaining policy choice. The regression queries the retained unmapped child,
remaps it without a configure response, and requires the opaque `FocusSurface`
result. New-window admission and every non-deterministic layout fence remain
fail closed.

The first rerun passed all eight Firefox stages and the repaired refocus cycle,
then exposed the adjacent teardown invariant. Destroying focused Firefox left
the private core-focus record stale, and xmonad's next focus-stack update named
an unmapped workspace child. Engine correctly rejected that proposal as
`HiddenFocus`. The facade now reverts focus on unmap or destroy, returns X11
`BadMatch` for a later hidden `SetInputFocus`, and translates focus only for a
mapped synthetic target. Regressions retain the unmapped child while proving
focus reversion, local rejection, and suppression at the blind-WM boundary.

## 2026-08-06: The soak gate uses generic session evidence

The installed soak verifier required a Firefox M8 proof-completion record even
though the ordinary installed Sophia entry does not enable that proof mode. A
documented daily-driver run could therefore never satisfy its own archive gate.
The same verifier also accepted action launches without matching clean exits or
close actions and did not require workspace, resize, held-input, cursor, or
kernel page-flip-clock evidence.

The gate now consumes the generic redacted summaries already owned by the live
session. It requires clean Kitty and Firefox exits, complete close coverage,
repeated focus and workspace transitions, visually committed resizes,
bidirectional selection activity, distinct output identities, drained input
and key state, clean cursor and page-flip clocks, and zero allocator or bounded
ownership failures. This keeps the ordinary installed entry authoritative and
does not add application metadata or payload logging. Focused mutation fixtures
remove each evidence class independently.

## 2026-08-06: Present coalescing is surface-local

Native-chrome attempt `0003` applied and rendered the combined policy, but its
sequence driver correctly rejected the resize boundary. The preceding
two-surface epochs armed exact Present candidates for both Kitty surfaces while
only one candidate per epoch reached native retirement. The other surface kept
an older Engine committed extent. When combined chrome restored that extent,
the layout coordinator suppressed its Configure as already committed and
produced a one-surface epoch with clipped pixels.

The production Present scheduler was coalescing all runnable queued work into
one newest transaction. This crossed surface identity: releasing two staged
Presents, or receiving the second surface after the epoch had committed, could
reject the first surface even though both carried independent visual debt. It
also contradicted the architecture rule that unrelated surfaces remain
independently runnable.

Runnable coalescing now keeps the newest transaction per surface. Same-surface
overload remains bounded, while distinct surfaces retain FIFO order and exact
retirement ownership. Regressions cover both two-surface release in one epoch
and the observed ordering where one staged Present becomes runnable before the
second surface arrives. The native-chrome verifier now requires both distinct
armed candidates to retire at their exact extents before the next policy
generation may advance.

Installed commit `6a5bc833` passed native-chrome archive `0004`. Ring-wide,
frame-only, and combined policy each delivered two Configures, armed two exact
surface candidates, and retired both before advancing. The archive records 14
routed physical keys, two connected outputs, normal logout, clean native drain,
and no protocol, submission, retirement, or cleanup debt.

## 2026-08-06: Interactive QEMU is separate from acceptance choreography

The retained `xmonad-m8-soak` guest is an unattended acceptance workload. Its
thirty-minute clock, scheduled compatibility-bridge restarts, and host-driven
input make it a poor environment for investigating one live interaction.
`xmonad-interactive` now packages the same isolated terminal, Vulkan, Firefox,
launcher, xmonad, and two-output native-X stack without a runtime deadline,
fault injection, or automated action sequence. The guest powers off only after
the ordinary logout action.

The supported interactive display is an unnetworked Unix-domain VNC socket.
QEMU traces only the relevant VNC and input-core boundaries into a FIFO; a
stream reducer retains the first display, keyboard, pointer, motion, and button
boundary crossings plus bounded keyboard-count checkpoints without persisting
raw keycodes, coordinates, or button values. The guest's existing reduced
records then distinguish virtio-device discovery, Engine intake and routing,
focused-client targeting, output projection, and cleanup. A fail-closed
verifier covers that entire chain plus
manual terminal launch, later typed input, focus, close, and logout order. The
Q35 guest explicitly disables `vmport`; otherwise QEMU activates its legacy
absolute `vmmouse` ahead of the declared relative virtio mouse, and viewer
motion never reaches the guest. The RFB client honors QEMU's pointer-type
pseudo-encoding and keeps relative button events at zero delta.

The tooling regressions and a complete RFB-to-Engine QEMU capture pass. One
human-visible viewer capture remains before the supported backend gate closes.

## 2026-08-06: VT resume transfers renderer-owned snapshots

Native-chrome attempt `0001` isolated the resume failure. The seat and KMS
state quiesced and reacquired correctly, but `LiveProductionVisualRuntime`
retained renderer-image IDs after the old native owner—and therefore its image
table—was destroyed. The replacement worker received a retained mixed frame
whose IDs belonged to the discarded generation. It correctly rejected that
frame as `InvalidTarget`, and every later Present remained mixed with the same
unresolvable scene state.

Keeping the old worker alive is not currently safe because its GBM device is a
clone of the seat-leased primary DRM node. Sophia instead exports each promoted
compositor snapshot as an opaque, bounded DMA-BUF lease after native work has
drained. The old KMS, EGL, and GBM owners are then released. After seat
reacquisition, the replacement renderer copies and promotes the exact same
image-ID set before `resume_native_scanout` may queue retained content. Missing,
duplicate, invalid, or unexpected identities fail the lifecycle transition.
An unsolicited revoke cannot guarantee a handoff, so it explicitly clears
stale runtime identities rather than entering a permanent invalid-target loop.

This keeps migration inside renderer/backend ownership and leaves Engine
protocol-neutral. It also preserves the future optimization suggested by
niri's split render/KMS model: Sophia can later move composition onto a
persistent same-GPU render-node owner and make the same typed handoff a no-copy
generation transfer. The current implementation does not retain revoked KMS
authority to obtain that optimization prematurely.

Installed native-chrome attempt `0005` exposed an ordering defect in that
handoff. VT release drained native work and captured both retained images, but
resume tried to import them before native output initialization had created the
replacement renderer worker. The new exporter therefore had neither an inline
context nor a worker image table and rejected the first snapshot. Resume now
initializes every replacement output owner, restores the exact image set, then
publishes the output runtime and queues retained content. A transition reducer
rejects restore-before-owner and duplicate lifecycle observations; an exporter
regression proves that the image owner does not exist before initialization.

This matches the mature reference sequence: niri reactivates DRM devices and
connectors before it schedules redraw, while yserver re-establishes modesets
before its full-damage repaint. Sophia additionally preserves its explicit
renderer-generation handoff because its retained scene stores renderer-owned
image identities across native-owner replacement.

The installed `d29e2f2c` rerun passed as native-chrome archive `0006`. The VT
transition drained with no abandoned scanout, captured two images, restored
both after tty7 reacquisition, and retired the first nonzero retained mixed
frame before later Present work. The session routed 28 physical keys, completed
all chrome generations, and logged out normally with zero native submit,
retirement, callback, renderer-worker, protocol, or cleanup failures. This
closes the focused installed switch-away/switch-back gate.

## 2026-08-06: The final chrome capture is an installed one-shot proof

The schema-2 native chrome verifier was strict, but its physical runner still
depended on a checkout, an on-login release build, and a separately retained
sequence file. The installed release now packages the native WM and guarded
driver as `Sophia Native Chrome Proof`. One menu selection reserves an attempt,
advances ring-only, frame-only, and combined modes, and finalizes a checksummed
archive after normal logout.

The shared installed-attempt ledger accepts explicit bounded extra evidence and
verifier inputs, keeping reservation, launch identity, lifecycle, and checksum
semantics common rather than cloning them for chrome. The archive verifier
binds the sequence commit to the release and fails closed on incomplete
transitions, lost physical input, output/native debt, emergency recovery,
modified evidence, or identity drift.

Installed commit `e07afa0f` passed native-chrome archive `0002` on two physical
outputs. The checksummed evidence contains all six ordered ring/frame phases,
48 routed physical keys, normal logout, clean native drain, an untriggered
guard, and exact VT restoration. This closes the remaining physical schema-2
chrome capture.

The preceding archive `0001` remains a useful failed attempt. An operator VT
switch quiesced native work before releasing tty7 and reacquired the seat on
return, but the resumed renderer repeatedly returned `InvalidTarget` instead
of rebuilding a usable target. Emergency recovery then retained a failed
status-130 archive. The chrome proof is complete, but the installed candidate
must restore and re-prove VT-resume target recreation before stability work can
advance.

## 2026-08-06: Consecutive-cycle evidence has a stable endpoint

Installed commit `4cc84913` passed normal archives `0003` through `0005`, then
passed fallback `0005`, watchdog `0003`, and emergency `0002`. The intentional
emergency correctly added a failed normal attempt. Because the cycle command
only selected the latest runs, that later evidence made the earlier passing
three-cycle gate impossible to reproduce even though its immutable inputs
remained intact.

`sophia-verify-cycles COUNT THROUGH_RUN` now selects the named direct ledger
child and its immediately preceding attempts. It applies the unchanged
checksum, result, identity, lifecycle, commit, and launch-uniqueness checks and
never skips an intervening failed or pending attempt. Fixtures retain an
earlier pass across later failures, reject a failed endpoint, reject an
endpoint outside the ledger, and keep latest-run behavior unchanged.

## 2026-08-06: Installed login proof accepts the production trace envelope

The first normal installed run on `02505e81` retired nine asynchronous kernel
page flips, reported 16 ms maximum submit-to-flip latency, and drained cleanly.
Its recorder nevertheless marked the attempt failed because the focused login
verifier required the page-flip schema at byte zero. Production emits that
record through `tracing`, after its timestamp, level, target, and ANSI state.

The login verifier now uses the same whitespace-delimited structured-payload
boundary as the fallback verifier. It still requires a genuine retirement and
rejects a log with that payload removed. A fixture wraps the passing record in
the production trace envelope so formatting metadata cannot invalidate later
installed evidence. Archive `0002` remains an immutable failed attempt; a new
run must supersede it.

## 2026-08-06: Recovery evidence belongs to one launch

The live session rotated its launch, runtime identity, lifecycle, input-guard,
and session logs, but appended every TTY handoff to one `recovery.log`. That
file could grow without bound, and a later immutable attempt could inherit
recovery records from unrelated launches.

One shared lifecycle helper now rotates all active reduced logs to a current
file and one `.previous` generation. The runner creates an empty, private
recovery log before preflight, so even an early failure cannot reuse older
handoff evidence. The installed wrapper uses the same boundary for launch and
runtime identity. Regressions cover replacement semantics, private modes,
preflight isolation, and installed-wrapper rotation; promotion archives remain
immutable and checksummed rather than being silently pruned.

## 2026-08-06: The installed soak gate owns its archive

The packaged `sophia-verify-soak` command previously accepted a mutable
session-log path. It could prove duration, application exercise, health, and
resource drain, but it did not prove which installed attempt, commit, binary,
launch, or lifecycle produced that log. It also required the operator to find
and paste a path after a long run.

The command now selects the latest normal-run ledger entry without arguments
and verifies its checksums, passed result, schema-4 kind, normal login and
lifecycle, launch digest and timestamp, release commit, and exact Sophia,
Kitty, Firefox, and xmonad identities before applying the focused soak budgets.
Numeric arguments adjust the duration and action thresholds without restoring
a log-path choreography; an explicit archive remains available for historical
checks. Fail-closed fixtures cover a failed latest attempt, unavailable Firefox
identity, a checksummed false Sophia digest, and post-checksum log mutation.

## 2026-08-06: Installed archives retain the Sophia binary identity

The installed recorder verified `/opt/sophia/current/SHA256SUMS` while a
session started, but an attempt archive retained only the release manifest and
runtime facts for Kitty, Firefox, and xmonad. Installing a later candidate
could therefore remove the only local file that named the exact Sophia binary
digest behind an older run. The commit remained known, but the durable
Milestone 12 identity contract was incomplete.

Runtime identity schema 2 now records the packaged Sophia version and
executable SHA-256 digest. The shared ledger verifies that value against the
installed file before takeover and finalization, then copies it into the
checksummed schema-4 attempt manifest. Normal-cycle, fallback, watchdog, and
emergency archive verifiers compare the manifest value with the retained
runtime identity without consulting the current installation. Fixtures cover
real capture, a missing or unavailable identity, a false expected digest, and
a self-consistently checksummed archive whose manifest lies about the binary.
No application content enters the record.

## 2026-08-06: Producer overload retains one newest Present

The production scheduler could already retain one queued Present, but a
same-surface owner in `SurfaceContentStream` admitted the first generation and
deferred every successor before the scheduler could see overload. Retirement
released the entire FIFO, then the first released generation became active and
hid the rest again. The queue therefore preserved stale work instead of the
newest drawable frame.

Engine admission now gives a pure, immediate, one-surface DMA-BUF Present one
replaceable deferred slot. Replacement never crosses a layout fence,
multi-surface group, CPU update, removal, software Present, or later work for
the same surface. Backend policy still owns payload classification and routes
every superseded transaction through the ordinary rejected-Present lifecycle.
XLibre's Present completion path confirms `Skip` for work discarded before
display; yserver confirms independent, exact Complete/Idle ownership; niri's
output loop confirms that a second frame does not overtake one awaiting vblank.

The diskless two-output virgl gate drives a three-buffer DRI3 client at 5 ms
intervals beside a static CPU Xterm. Its client selects and drains Complete and
Idle events. Two consecutive runs sustained two five-second overload phases,
kept both the replaceable Engine slot and scheduler queue at depth one, allowed
one KMS submission in flight, and retained at most three sources and two
Present records. The latest run completed 357 displayed frames, skipped 925,
and routed all 1,282 Complete and 1,282 Idle events exactly once. It recorded
906 supersessions, 361 balanced renderer requests/completions, a 40 ms maximum
worker request, no worker stall, no route failure, and clean resource teardown.

The subscribed client exposed two transport bugs during promotion. Protocol
events and WM controls used separate writers contending on one unfair socket
mutex, so feedback could starve focus acknowledgement. A control-output
priority barrier now prevents ordinary replies, input, or protocol events from
overtaking a pending control write. The live broker also inherited its
64-record protocol queue from the unrelated key-input bound. Route capacities
are now independent; the 512-record protocol bound derives from the 256-work
authority queue and Present's two feedback phases. Deterministic regressions
cover both the socket-lock race and lifecycle capacity. The production verifier
also mutation-tests queue depth, KMS overlap, supersession accounting,
client-visible feedback, worker debt and latency, resource high-water marks,
output confinement, and cleanup. Cumulative 250 ms progress samples replace
per-event serial diagnostics so evidence collection does not create a renderer
stall.

## 2026-08-06: Retained focus damage keeps the native visual owner

The first long idle-efficiency attempt could stop after an arbitrary successful
focus transition. X11 focus and the external WM transaction both committed,
but no retained native submission followed. Reduced diagnostic runs reproduced
the stop earlier and showed that the production runtime had no native owner for
the cycle that observed the focus change.

The CLI had treated a coalesced authority batch as a reason to withhold native
scanout from the entire Engine cycle. That decision was valid only for building
a redundant CPU frame. Retained DMA-BUF projection and Engine-owned chrome use
the same native owner without requiring a new CPU frame. Because the runtime
had already committed the new focused surface, the next batch observed no
change and could not recover the omitted repaint.

CPU-frame deferral is now advisory only for CPU composition. Every
native-enabled production cycle retains access to the native visual owner, and
the backend initializes native frame state only when that cycle actually
contains a new CPU frame set. A focused regression fixes the ownership policy;
diagnostic markers preserve the queue boundary for future renderer work.

The final diskless virgl proof freezes one real `glxgears` DMA-BUF surface next
to a static CPU/SHM Xterm. Two consecutive production runs each committed 256
Super-J actions and delivered 256 partial, page-flip-retired `RetainedMixed`
submissions without another client Present or CPU submission. Both then spent
two seconds idle with zero repaint, page flip, or client Present. The latest
run recorded 73 imports, 257 cache hits, 73 evictions, 334 balanced worker
requests/completions, a 34 ms maximum worker request, one active output, one
baseline-only output, and clean teardown. The two runs legitimately differed
between two and three startup uploads because the static initial frame may
coalesce; the causal retained window rejects uploads regardless of that startup
choice. Mutations cover lost transitions, full damage, CPU submission, idle
work, weak cache reuse, worker debt, output leakage, and cleanup debt.

## 2026-08-05: Real DMA-BUF contention has a bounded single-output gate

Lavapipe could exercise the Vulkan client path but could not populate Sophia's
native import cache. The new rendering image instead connects QEMU's virgl GPU
to an explicit host render node and runs three unmodified `glxgears` clients.
The image includes Mesa's dynamically loaded GLX vendor library and Xlib's
locale database; the latter is required for unmodified xmobar to create its
font set and publish the 14-pixel work-area reservation.

Starting all producers simultaneously made the setup measure admission races
rather than steady-state rendering. The final profile uses Sophia's existing
bounded application-admission FIFO: the first producer and xmobar establish a
stable desktop, then two session actions introduce one producer at a time. No
sleep declares admission complete. Each action must reach exact presented
pixels, settled layout, and the matching application-admission record before
the next action is sent. Initial client dimensions match the deterministic
work-area allocations, so the retained run committed all six WM transactions
without timeout, recovery, or restart.

Two consecutive production runs passed the contract. The latest bounded window
retired 97 frames: 32, 33, and 32 from the three distinct DMA-BUF surfaces.
Completion reported 816 imports, 1,069 cache hits, 816 final evictions, zero
live cache entries, 818 balanced renderer requests/completions, no worker
failure or stall, and a 47 ms maximum request. Xmobar produced 56 CPU patches
beside those clients. All nine frontend controls were delivered with a 1 ms
maximum acknowledgement, Present cadence advanced for every retained sample,
and native, layout, protocol, application, and frontend ownership drained
cleanly.

The verifier derives per-surface counts only between causal window markers and
checks the marker totals against the raw retirements. Mutations starve one
producer, falsify those totals, remove cache reuse, leave worker debt, exceed
the 100 ms request budget, inject layout recovery, or silence the CPU bar; each
must fail. The two-output topology is retained, but output 2 carried only its
startup baseline. This closes the single-active-output cell, not the roadmap's
inter-output fairness requirement. That remaining proof needs active work on
both outputs in one render-device group after the shared-worker prerequisite.

## 2026-08-06: Resize storms advance only after exact pixels commit

The former `--inject-surface-resize` proof could exercise one transition but
could not distinguish a robust resize pipeline from one that happened to
recover once. Its bounded sequence extension retains one active proof at a
time and advances only after the matching transaction has delivered a client
configure, committed the resize epoch, and installed pixels at the exact
target size. This preserves the production visual-admission contract instead
of using sleeps or overlapping speculative transactions to manufacture load.

The new diskless `xmonad-resize-storm` profile continuously redraws an Xterm
through the CPU/SHM patch path while cycling 12 policy sizes across two virtual
outputs. Two consecutive production runs committed every
request→layout→resize-epoch→pixel chain without timeout, rollback, authority
drop, stale WM response, or restart. Both observed partial repaint and another
retired frame after the final resize, then shut down with balanced renderer
requests and completions, zero live snapshot/import-cache entries, and clean
application, native, input, and WM ownership.

The verifier is causal rather than count-only. Mutations remove an exact-pixel
commit, alter its dimensions, inject a layout timeout, remove post-storm frame
progress, or leave worker ownership outstanding; each must fail. This closes
the software-present resize workload. It deliberately does not stand in for
the remaining multi-producer DMA-BUF contention proof.

## 2026-08-05: X map deferral requires a live policy owner

The second commit-pinned Kitty fallback attempt retired one valid black Present
and then reached the visual-readiness deadline without another submission.
Native scanout, page-flip retirement, focus delivery, and cleanup were healthy.
The real-Kitty frontend probe also passed with production's idle-before-complete
Copy feedback, excluding Present routing as the stalled boundary.

The live frontend had unconditionally enabled policy-deferred mapping, while
the no-WM layout deliberately bypassed policy admission. Those two decisions
left no owner capable of admitting the deferred toplevel. Kitty's pre-map
bootstrap Present could retire, but the window could never cross the X11 map
transition or receive the MapNotify, VisibilityNotify, and Expose events that
start its visible rendering.

XLibre maps immediately unless a SubstructureRedirect owner intercepts the
request. yserver independently implements the same rule and emits map,
visibility, and exposure only after the window becomes viewable. Sophia now
derives frontend deferral and Engine admission from one policy-map mode:
external-WM sessions defer, while no-WM sessions fulfill MapWindow directly.
The reducer regression makes the two states mutually exclusive, and the real
Kitty probe now uses production's Copy feedback order and cadence.

## 2026-08-05: black client content cannot authorize native recovery

The first commit-pinned Kitty fallback attempt exited at its eight-second
readiness deadline. KMS ownership, synchronous modesets, page-flip callback,
Present retirement, focus delivery, and VT restoration had all succeeded.
Kitty's first mixed frame was valid but entirely black, and another primary
Present was already queued. The owner nevertheless treated the absence of
nonzero pixels after 1.5 seconds as a native transport failure.

That transition was invalid. The recovery drained and replaced the active KMS
and renderer owner while the runtime still retained the first Present's
renderer-image identity. The replacement worker could not resolve an image
owned by the discarded worker, reported `InvalidTarget`, and left the queued
client work unable to satisfy startup. A valid black frame is application
readiness evidence; it is not evidence that page-flip transport has stalled.

Startup native recovery is now admitted only by an objectively missing output
callback after the bounded 750 ms transport threshold. Valid black content
remains owned by the normal eight-second readiness deadline, allowing queued
client Presents to advance without destroying renderer state. A reducer
regression proves that elapsed time alone cannot authorize recovery and that
the missing-callback threshold retains its exact boundary.

## 2026-08-05: an emergency outcome archives before handoff

An ordinary installed session already reserved a normal attempt before
graphics takeover, but a successful Ctrl-Alt-Backspace proof still depended on
a later manual recorder. The status-130 handoff therefore retained failure in
the normal ledger while leaving the positive independent-recovery evidence in
rotating active logs.

The installed wrapper now recognizes only the exact status-130 outcome and
runs the strict emergency recorder before finalizing the interrupted normal
attempt or returning to greetd. The emergency recorder uses the shared
schema-3 ledger with an expected status of 130 and the emergency lifecycle
mode. If archival fails, it leaves a failed emergency directory; if the wrapper
is interrupted first, the original normal attempt remains pending. The normal
attempt always remains failed, so recovery cannot satisfy a clean login cycle.

The aggregate verifier rechecks archive digests, both guard and owner chord
observations, key and native drain, graceful recovery, installed lifecycle and
runtime identities, record kind, release commit, start time, and launch digest.
Automatic-session fixtures preserve status 130, require the separate passed
archive plus failed normal attempt, reject duplicate recording, and reject a
modified archive. Packaging, status, installation, and both operator guides
now expose `sophia-verify-emergency` without adding another greetd entry.

## 2026-08-05: the watchdog proof records itself before takeover

The dedicated recovery entry previously produced valid live logs but required
a later `sophia-record-watchdog-run` invocation. That post-session copy had no
pending state: a wrapper interruption or later graphical launch could erase
the only active proof before it entered the immutable archive.

`Sophia Recovery Proof` now selects the watchdog attempt profile explicitly.
The installed wrapper reserves a separate numbered directory before graphics
takeover, preserves a crash as pending, and finalizes only after the expected
status-124 display-manager handoff. The shared ledger now parameterizes the
expected session status, lifecycle mode, and whether the focused verifier also
consumes lifecycle evidence; normal and fallback semantics remain unchanged.

The repository-independent aggregate verifier checks archive digests, the
watchdog result and focused recovery contract, runtime identity, schema-3 kind,
release commit, start time, and launch-identity digest. Automatic-session
fixtures reject a wrong exit, failed latest attempt, and modified archive.
Packaging, installation, status, validation guidance, and the operator runbook
expose `sophia-verify-watchdog`; the old no-argument recorder remains only as a
compatibility importer for an unrecorded proof.

## 2026-08-05: installed fallback attempts are automatic and fail closed

The Kitty baseline previously rotated live logs but had no immutable attempt
boundary. A successful-looking later run could overwrite the evidence for a
failed fallback, and the operator had no repository-independent command that
bound the reduced session to its installed release identity.

Normal and fallback logins now share one profile-parameterized attempt ledger.
Each installed Kitty launch reserves a numbered directory before graphics
takeover and finalizes it after display-manager handoff. A crash remains
pending; a nonzero or unverifiable run remains failed. The archive contains
checksummed session, guard, recovery, lifecycle, launch-identity, runtime-
identity, and release records, while normal and fallback attempts remain in
separate ledgers and cannot be relabeled across profiles.

The fallback verifier admits only the bounded one-Kitty, WM-disabled profile.
It requires two-output startup and visible retirement within eight seconds,
positive routed physical keys, clean protocol, presentation, application, and
lifecycle shutdown, an armed but untriggered guard, and exact Kitty-profile KD
and termios restoration. Mutation fixtures reject missing Kitty exit,
one-output or slow startup, missing retirement, absent physical input, external
WM policy, emergency recovery, a wrong recovery profile, a failed latest
attempt, and archive modification. Packaging, installation, status, and the
operator runbook expose the same contract without a source checkout.

## 2026-08-05: installed operations have one packaged source of truth

The immutable release now carries its own operator runbook instead of relying
on a checkout after installation. The guide records the single retained AMD
two-output support boundary, required greetd, runtime-directory, libseat,
DRM/input, runtime-library, and application contracts, plus the exact status,
log, stop, emergency, fallback, evidence, and atomic rollback procedures. It
also labels native X11 scope, desktop-service isolation, physical coverage,
VRR, direct-scanout, cursor-plane, and rollback-retention limitations.

`sophia-status` reports the packaged guide and latest automatic cycle attempt.
The installed stop command discovers `/run/user/$UID` when a control-TTY shell
does not export `XDG_RUNTIME_DIR`, preserving the documented independent-stop
path. Installer fixtures verify that the runbook is checksummed, survives
install and rollback, and remains discoverable without the source tree.

## 2026-08-05: installed cycles identify launches, not operator rituals

The Milestone 11 cycle recorder reused the complete xmonad promotion verifier.
That made each ordinary login repeat workspace, VT, pointer-edge, clipboard,
close, and relaunch choreography already retained by earlier physical gates.
It was simultaneously too weak about the requirement it claimed to prove:
three archives copied from one launch had distinct directory numbers and could
pass as three cycles.

Installed cycles now have a focused boundary. A passing cycle requires
automatic Kitty startup, two-output readiness within eight seconds, native
retirement, normal logout, clean protocol and session health, drained native
and application state, an armed but untriggered guard, exact TTY restoration,
the installed lifecycle, and the release identity. The full xmonad and Firefox
interaction proofs remain separate and are not multiplied to count logins.

Each recorded cycle carries the launch timestamp and SHA-256 digest of its
installed launch-identity record. The recorder rejects an identity already in
the ledger, and the aggregate verifier recomputes every digest and rejects
duplicates or mixed commits. Mutation fixtures cover missing logout, one-output
startup, slow readiness, native debt, emergency recovery, repeat recording,
and a copied archive. The staged immutable installer carries the new verifier;
three distinct physical installed cycles remain the acceptance boundary.

The installed wrapper now reserves a ledger entry before graphics takeover and
finalizes it only after the display-manager handoff. A process failure leaves a
pending entry, while a nonzero or unverifiable session leaves a checksummed
failed entry. The latest-N verifier includes both states rather than selecting
only passes. Reservation failure stops takeover, and an atomic directory claim
prevents simultaneous launch attempts from sharing a sequence number. Fixtures
prove automatic success, preserved session exit status, failed-attempt
interruption, fail-closed reservation, and three subsequent clean launches.

## 2026-08-05: mixed-session control handoffs have explicit boundaries

The complete M8 workload exposed four races after the earlier presentation and
focus repairs. An X request could observe no pending control, lose the runtime
mutex, and then overtake the newly queued control. A control's acknowledgement
deadline began while it was still behind a prerequisite or key release. The
session's 500 ms WM transport deadline contradicted the xmonad bridge's bounded
three-second reply collection. Finally, fixed host sleeps and per-frame serial
tracing let fault injection and input run against stale guest state.

Request dispatch now rechecks control priority while holding the runtime lock.
Control admission records when an item first becomes dispatchable and gives a
dispatched item an independent acknowledgement deadline. The outer WM response
budget is four seconds, still below the ten-second transaction and twelve-second
admission budgets. The QEMU proof anchors its fault after startup clients exist,
uses action, projection, layout, focus, and clipboard-owner barriers, and pins
the final modal click to a deterministic DOM anchor. Reduced M8 logging keeps
the serial channel causal without changing verifier evidence.

Modern GTK also required a real session bus before it would connect to X. The
guest image now packages the D-Bus runner, daemon, and session configuration,
and Sophia plus every child run inside one session-scoped bus. The minimal GTK
scenario explicitly disables its out-of-scope accessibility-bus lookup. The M8
Zenity launcher now opens, is admitted, closes normally, and no longer strands
the postlude.

Deterministic regressions cover request/control lock arbitration, dispatch
eligibility, independent queue and acknowledgement deadlines, deadline
ordering, and every new verifier barrier. The rebuilt full M8 gate passed all
eight Firefox stages, launcher admission and exit, one expected bridge restart,
zero timed-out or unexpected controls, and clean health and teardown. The
older isolated GTK `--entry` scenario still times out before X connection with
the host's GTK 4 Zenity, while the M8 `--info` launcher passes; that separate
harness compatibility issue is not evidence for the mixed-session milestone.

## 2026-08-05: presented admission has one positive-focus writer

The first mixed-application QEMU run after the CPU admission repair reached
vkcube's exact presented candidate, then terminated on a duplicate X-authority
focus control. The layout state machine had correctly deferred positive focus
until that candidate retired. Independently, the WM workspace-projection
adapter queued the same transaction and surface immediately. Retirement could
therefore reproduce a control that was still awaiting its frontend
acknowledgement.

Positive focus now has one owner: `PersistentLiveLayout` queues it immediately
for committed backing snapshots or after the exact retirement for presented
admissions. The projection adapter only clears an old focus when policy leaves
no visible target. Focus evidence remains ordered after workspace projection
for immediate transitions and is emitted at retirement for deferred ones. A
pure projection regression rejects positive-focus synthesis, while the
presented-admission regression proves that no focus is available before the
matching retirement. The mutation-tested xmonad verifier, source audit, and
complete all-features suite pass; the commit-pinned QEMU gate remains the
acceptance boundary.

## 2026-08-05: CPU admission releases the replacement before its patches

The first QEMU run past the repaired semantic preflight admitted a third xterm
with CPU handle 172, then failed composition because the committed scene lacked
that handle. X authority had correctly emitted one replacement followed by
several same-handle patches while the surface was quarantined. Admission
released only the selected final patch; the renderer rejected its missing base
and the following Engine commit exposed the absent handle.

Backing-snapshot admission now releases the complete ordered group prefix
through the selected transaction and rebases each generation at the accepted
geometry. PresentedBuffer admission retains its stricter behavior: passive CPU
history cannot overtake or impersonate the selected Present. A deterministic
regression reproduces replacement transaction 380 and selected patch
transaction 381, requiring replacement-before-patch order and generations zero
then one. The source audit and complete all-features suite pass; the unattended
QEMU gate remains the acceptance boundary.

## 2026-08-05: the semantic preflight restores source ownership boundaries

The commit-pinned semantic gate stopped before QEMU because seven Rust files
exceeded the reviewed 1,000-line source boundary. Six had direct ownership
seams: WM admission tests, layout commit reduction, native one-shot rendering,
X input-event writing, routed X delivery, and X routing tests. Each domain now
lives behind its existing facade without changing its public API or runtime
order.

The remaining file is the single ordered X11 connection lifecycle, from setup
through request dispatch and disconnect cleanup. Splitting its control flow
would scatter one tightly coupled algorithm, so it has one explicit temporary
cohesion exception. The source-layout audit, formatting checks, offline
metadata, and complete all-features suite pass after the extraction. The
commit-pinned semantic gate remains the next acceptance step.

The next preflight exposed an independent fixture-ownership error. The generic
TTY verifier still consumed the integrated Firefox fixture even though the
shortened Firefox workflow had intentionally removed the last-window exit and
desktop relaunch sequence. A dedicated generic TTY fixture now owns that
sequence, two-output retirement, and clipboard evidence. Firefox fixture
changes can no longer invalidate the unrelated TTY verifier.

## 2026-08-05: composited Present owns a renderer snapshot

The retained DMA-BUF path had given X Present `Flip` semantics to an ordinary
composited frame. Sophia kept duplicated client DMA-BUF descriptors in scene
state after page-flip completion, so the client could legally reuse its pixmap
while later focus, layout, or damage repaints still sampled it. XLibre's copy
path idles the source after copying, and yserver independently sends Idle
before Complete for Copy; Flip is reserved for retaining the exact source.

The native renderer now captures each current DMA-BUF into a bounded,
same-format compositor-owned GBM image. The image is staged during rendering,
promoted only by the exact mixed-frame page flip, and rolled back on terminal
failure. Retained scene state contains image identity and geometry but no
client file descriptors. Output-target recreation may discard EGL imports
without discarding the renderer image. Replacement evicts the import before
dropping its backing store.

Page-flip retirement now reports Copy and releases the client source with Idle
before Complete. X-authority tracks the two phases independently, accepting
both Copy and future Flip ordering exactly once. Reduced snapshot metrics and
`PresentCopyOwnership.tla` cover capture, promotion, rollback, eviction, live
debt, and the rule that displayed composited content is compositor-owned.

The paired physical acceptance runs on `39f87687` passed. The bounded GLX run
remained visibly animated under continuous pointer motion, sustained 59.950
presentation FPS with a 16.685 ms p95 interval, and balanced 1,193 snapshot
captures and promotions with matching Copy, Idle, and idle-fence completion.
The four-Kitty mixed-scene run balanced 146 captures and promotions, reused
retained imports 356 times, and completed 146 Copy feedback cycles. Both runs
reported zero rollback, live snapshot or import debt, unexpected protocol
errors, and cleanup failure. This closes the snapshot correctness gate; the
conditional three-slot software scanout pool is not justified by these
results.

## 2026-08-04: delayed Presents reconcile resolved layout epochs

The first installed run after grouped CPU content selected vkcube's 500-by-500
software Present, but no vkcube window became visible. The live trace showed
that application creation and pixel capture had succeeded. Its admission epoch
timed out while an existing Kitty DMA-BUF Present was staged; rollback rejected
the scheduler-owned Kitty Present, but later Kitty groups remained behind that
Present in `SurfaceContentStream`.

Those later groups retained `StageLayout { epoch: 2 }`. They reached the
Present scheduler only after epoch 2 had aborted, so the one-shot abort could
not see them. The scheduler treated the dead epoch as pending again. No future
commit or abort could release it, Kitty stopped supplying resize evidence, and
vkcube's valid admission transaction remained outside the visible workspace.
The new content stream exposed this latent time-of-classification error by
making the deferred ownership exact.

The Present scheduler now retains bounded outcomes for resolved layout epochs
and reconciles a delayed submission when it actually enters the queue. Work
from an aborted epoch receives ordinary controlled Skip/Idle settlement; work
from a committed epoch runs when its surface is already visible or waits only
for visibility. An outcome older than the bounded exact history fails closed
instead of recreating an epoch that cannot progress. Crate-boundary regressions
cover both delayed abort and delayed commit. A fresh installed vkcube run
remained the physical acceptance boundary.

Installed commit `663934ca` passed that boundary. The run reproduced the
important recovery shape: vkcube's first 500-by-500 software Present arrived
during epoch 2, the blind-WM resize timed out, and one staged Kitty Present was
aborted. Kitty then resumed native retirement, epoch 4 committed both visible
surfaces, and vkcube's exact transaction 574 retired on native frame 20. The
cube continued for 665 clocked software retirements with increasing kernel
MSC values. Normal logout reported 691 Complete events and 691 Idle/fence
signals, 132 native retirements, no protocol or native failure, no live
presentation resource, clean layout health, and clean frontend teardown.

## 2026-08-04: software Present feedback owns an exact native frame

The live mixed-session trace disproved FIFO association between software
Present and native page flips. Software transaction 599 remained pending until
an unrelated Kitty DMA frame 704 retired; transaction 715 behaved the same way
behind frame 742. The runtime marked and settled the oldest software submission
on whichever native callback arrived next. CPU output work was also suppressed
while a GPU projection existed, so vkcube could advance a few frames only when
unrelated desktop damage happened to drive KMS.

Native frames now carry monotonic typed identities from queueing through submit
and page-flip retirement. A software Present first owns an immutable CPU or
retained-mixed frame, then only callbacks naming that frame may mark its
resources submitted or route clocked Copy/Idle feedback. Mixed owner batches
serialize an unrelated DMA frame and the software follow-up frame; the latter
uses the DMA transaction's prepared candidate so the new Vulkan surface cannot
disappear between flips. Same-owner coalescing excludes software Present work.

The deterministic reducer regression injects unrelated submission and
retirement observations and requires them to be no-ops. The physical verifier
joins every software retirement to its nonzero native frame and submission,
and `PresentFrameOwnership.tla` checks both the safety relation and eventual
retirement under weak fairness. The offline gates establish the lifecycle; a
fresh installed xmonad/vkcube run remains the physical acceptance boundary.

The first installed run exposed a legal callback/submission overlap in the new
guard. Retained software frame 30 retired while the same backend tick had
already submitted the next DMA Present as frame 31. The retirement finalizer
compared frame 30 with the scheduler's newer current frame and terminated the
session before settling transaction 699. Frame identity was correct; the
reducer had confused captured retirement ownership with current submission
state.

Retirement reduction now treats an exact CPU or retained frame as independent
of a newer submitted DMA frame, settles only bindings owned by the captured
frame, and leaves the successor scheduler entry intact. A `MixedPresent`
retirement without the matching scheduler frame still fails closed. The Rust
regression reproduces frames 30 and 31, and `PresentFrameOwnership.tla` now
allows successor submission between native retirement observation and feedback
settlement.

The installed `ad84d88a` rerun visibly animated vkcube and shut down cleanly
after 411 Copy completions, 21 Flip completions, and 435 Idle/fence signals,
with zero live Present resources, native failures, or protocol errors. The
first verifier result was a false failure: it enrolled startup Kitty DMA
transaction 410 and demanded software feedback records from it. The verifier
now enrolls only an armed admission with an exact schema-4 software retirement.
Its pass fixture includes the unrelated DMA admission and the legal sequence in
which frame 30 retires, DMA successor 31 submits, and frame 30 then settles. It
also rejects an unrelated-only log, a successor frame stolen for software
feedback, short animation, and insufficient diagnostic or aggregate feedback.
The corrected verifier passes the retained physical session.

## 2026-08-04: Present retirement is independent of storage

The fresh installed xmonad/vkcube run disproved the remaining DMA-only
assumption. Engine selected exact transaction 626, surface 6291456, CPU buffer
28 as `PresentedBuffer`, but admission committed it as `cpu_snapshot` without
native retirement. The vkcube main and WSI queue threads then remained blocked
on their FIFO Present wait. In the owner batch that released transaction 626,
an unrelated Kitty DMA-BUF Present selected the GPU production path. That path
scheduled the DMA-BUF group but did not register the separate software-Present
group, so no Complete or Idle event could unblock vkcube.

XLibre's copy path copies the pixmap and sends Idle plus clocked Complete after
the target MSC. Yserver independently preserves the same Idle/Complete
lifecycle when its scheduler chooses copy instead of flip. Storage selects the
composition mechanism; it does not erase Present identity or retirement.

Sophia now carries the exact transaction/surface/target-buffer key and source
extent on software Present submissions. Any Engine-selected `PresentedBuffer`
enters `AwaitingRetirement`, including a CPU materialization. A mixed owner
batch registers its separate software groups before the DMA-BUF group drives
the shared native frame; submission and page-flip settlement then route
clocked Copy/Idle feedback and expose the exact software retirement to layout
and admission. Only a non-Present `BackingSnapshot` may use immediate CPU
admission. Same-group storage ambiguity fails closed.

The Rust regressions cover exact CPU admission fencing, software feedback
retirement, source extent, and intake cardinality. `AdmissionRecovery.tla`
explores both DMA and CPU storage for the same Present lifecycle, and the
physical verifier accepts either exact storage identity while rejecting any
retirement bypass. Offline gates pass; a fresh installed physical run remains
the milestone gate.

## 2026-08-04: admission evidence requires exact target-buffer identity

The live TTY7 vkcube trace selected transaction 683 as a `PresentedBuffer` for
surface 6291456, then committed that same transaction from `cpu_snapshot`.
The authority batch contained both the DMA-BUF Present and a CPU backing update
for the same surface and extent. Sophia retained only transaction and extent in
its safe observation, while a surface-level Presented flag upgraded both
sources. The backing snapshot could therefore impersonate the Present, leaving
the Vulkan client waiting behind a frame that never entered native retirement.

XLibre and yserver use extent to select flip versus copy/clip, not to establish
Present identity. Niri and river likewise keep requested, configured, and
rendering state attached to their owning serial or transaction. Sophia now
carries one exact transaction/surface/target-buffer key through candidate
selection, admission, scheduler ownership, native retirement, and feedback.
A valid buffer that does not match a pending resize renders against committed
geometry without promoting that resize; only malformed or superseded work is
rejected. Layout commit and abort are explicit epoch transitions, so timeout
cannot release staged work through a size-based recovery heuristic.

The regression places a DMA-BUF Present and CPU backing snapshot in the same
transaction and requires only the DMA-BUF key to receive Presented evidence.
The production scheduler regression proves that aborting one epoch does not
disturb another. `AdmissionRecovery.tla` checks exact selection through timeout,
recovery, native retirement, and Complete/Idle feedback, and the physical log
verifier now requires three increasing, clocked Present retirements. Offline
Rust, verifier, and TLC gates pass; one fresh installed TTY run remains the
physical milestone gate.

## 2026-08-02: admission release must precede current authority work

The first focused selection run reproduced the short black Firefox window.
Live evidence showed that Firefox's 1280-by-1040 fallback retired while an exact
1276-by-1422 standing-target Present was already quarantined. On admission
release, production batch assembly placed the current Firefox group before the
older retained groups. Engine therefore saw generation 50 before generations
3 through 49 and correctly rejected the entire chain as stale against visible
generation 1. The standing-target Present never reached native retirement, so
the temporary recovery extent remained active and the lower tile stayed black.

This is an owner-side authority ordering defect, not an X11 configure or client
geometry defect. Released admission groups now precede the current observed
batch, preserving FIFO generation order for the same surface. The regression
uses one surface with an older released DMA-BUF Present followed by a newer
current CPU update and requires both Engine commits plus final generation 2.
The geometry/admission gates remain fail-closed; a fresh focused physical run
is required.

## 2026-08-02: focused slices precede the Firefox promotion gate

The combined physical Firefox workflow had become a poor debugging loop. A
late selection or lifecycle failure discarded several minutes of unrelated
manual interaction, and repeating the full sequence made operator timing part
of diagnosis. The long workflow remains the Milestone 10 promotion contract,
but it is no longer the first test for a localized change.

Two source-tree diagnostic slices now reuse the production session and X
authority while narrowing the manual surface. The selection slice launches one
Kitty and one Firefox, stops after four browser stages, and requires four
ordered cross-client owner-change/conversion intervals. Direction-specific
tokens plus a trusted full-field selection arm make stale CLIPBOARD or PRIMARY
state fail at the step that introduced it. The lifecycle slice launches two
Kitty and two Firefox processes, skips the content choreography, and proves a
normal close followed by a WM-forced close with both peers retained. Each slice
has its own completion record, fail-closed verifier, and negative fixture.

The validation ladder is therefore: offline reducer/coordinator/verifier
regressions, the affected focused physical slice, then one complete physical
promotion run. Repetition belongs to the unattended installed-session soak,
not repeated manual choreography. This separation enables future automation
and timing optimization without weakening the release contract.

## 2026-08-02: selection conversion requires an independent requestor

The first complete post-floating physical workflow reached all eight Firefox
stages, both normal and forced browser closes, and all six original Kitty
retention checkpoints. It nevertheless ended with one selection-owner change
and zero conversions. The browser page had advanced because its paste handler
prevented the default operation and wrote the expected token itself. Even a
real same-process Firefox copy and paste may reuse locally retained selection
content without sending core `ConvertSelection`, so neither signal proved the
cross-client X11 path required by Milestone 10.

XLibre's `ProcConvertSelection` and yserver's independent Rust implementation
agree that conversion begins only when a requestor sends core opcode 24. The
server then routes `SelectionRequest` to the current owner or returns
`SelectionNotify(property=None)` when no owner exists. The server must not
fabricate a conversion from an application-level paste event. Sophia's X
authority behavior is therefore unchanged; the defect was in the proof
workload and its completion contract.

The physical M10 page now opts into a peer-selection mode that leaves default
browser paste intact and advances only when an input contains the exact bounded
token. Kitty B validates Firefox-owned `CLIPBOARD` and `PRIMARY`, publishes two
new redacted title checkpoints, then returns ownership so Firefox consumes both
selections from an independent X client. The verifier requires an ordered
owner-change and conversion in each of the four directional intervals and at
least four of each operation overall. QEMU M8 retains its existing same-process
fixture path. Offline coordinator, reducer, and fail-closed verifier
regressions pass; a fresh physical workflow remains required.

## 2026-08-02: floating is WM placement, not an X-authority bypass

Repeated Firefox popup work exposed a boundary error in the earlier transient
and EWMH reductions recorded below. Sophia treated transient, dialog, utility,
menu, splash, and popup-like hints as `ClientPositioned`. That removed ordinary
non-override-redirect X toplevels from WM redirection instead of telling the WM
how they should be placed. It also coupled protocol classification to visual
authority, so a dialog could either become another tile or bypass the blind WM
entirely; both outcomes violated the intended architecture.

XLibre's root-window redirect path and yserver's native Rust request reduction
agree on the decisive rule: a non-override-redirect root child remains subject
to the WM's map/configure policy regardless of `WM_TRANSIENT_FOR` or
`_NET_WM_WINDOW_TYPE`. Override-redirect is the protocol bypass. Explicit
desktop/dock ownership is also client-positioned in Sophia because those
surfaces reserve output space rather than participate in application layout.
Transient ownership, functional type, floating preference, and stack order are
separate facts. This section supersedes the earlier log entries that describe
transient or dialog-like types as client-positioned.

The implemented boundary is protocol-neutral. Authority surface and
presentation packets now carry `LayoutNodeKind`, `SurfacePlacementPreference`,
an optional opaque presentation owner, and an explicit bottom-to-top stack
rank. X Authority decodes ordered EWMH types, keeps dialogs/utilities/popups
policy-managed with floating preference, preserves real override-redirect in
map delivery, and applies sibling/stack-mode ConfigureWindow requests through
one ranked stacking table. The wire regressions cover a genuine
`WM_TRANSIENT_FOR` dialog, EWMH dialog classification, override-redirect, and
raise/lower ordering.

Engine and the blind WM own the resulting desktop state. WM API v7 adds
persisted floating state, transactional `SetFloating`, and one completed
pointer-gesture packet containing only opaque surface/output/workspace IDs and
integer start/end positions. The xmonad adapter exposes standard
`WM_NORMAL_HINTS`, `WM_TRANSIENT_FOR`, and `_NET_WM_WINDOW_TYPE` properties to
its private synthetic server. `Super+Shift+Space` toggles floating;
`Super+left-drag` moves and `Super+right-drag` resizes through xmonad's stock
mouse policy. The private server preserves query/grab/warp ordering and real
bottom-to-top QueryTree order, so the adapter receives the final configure
only after the completed gesture.

Pointer capture is owned by Engine. While a drag is active, matching physical
events do not reach the client. Each motion renders a topmost compositor-owned
outline using retained committed pixels; the client geometry and buffer remain
unchanged. The outline and final bridge result clamp the entire frame to the
output containing the gesture start. Release retires the outline and sends one
WM request, so configure, pixels, placement, focus, and floating state still
cross the established atomic commit boundary. Reducer, multi-output,
compositor-border, codec, policy-persistence, and process-external xmonad
regressions lock these seams for later gesture coalescing and rendering
optimization.

The deterministic Firefox milestone no longer pretends that an HTML action is
an X11 dialog conformance test. Its final step uses an in-document `<dialog>`
with ordered ready/confirm checkpoints and no surface-count transition. The
genuine hinted X11 dialog remains a separate authority/bridge regression. This
keeps browser interaction evidence independent from transient-toplevel
protocol evidence and removes the popup admission loop that repeatedly blanked
the owner. A fresh physical Firefox workflow is still required before closing
Milestone 10.

## 2026-08-02: CreateWindow is not a configure transition

Three freshly built physical runs produced the same blank Firefox owner after
`Open proof dialog`, despite the transient and EWMH role reductions. The GDK
thaw diagnostic landed immediately after the real Engine admission configure.
The classification work was correct but could not repair an earlier lifecycle
violation: Sophia had emitted an unconditional core `ConfigureNotify` from
`CreateWindow`, even when no client selected a lifecycle mask. That false
configure unbalanced GTK's toplevel update bookkeeping, so the later real
configure exposed the thaw underflow and blank frame.

XLibre `dix/window.c` emits only parent-selected `CreateNotify` during
creation, returns silently from a no-op configure, and delivers map events
through structure/substructure selection. Its realized/viewable split also
keeps a mapped descendant off-screen below an unrealized ancestor. yserver's
independent Rust tables encode the same protocol states as `Unmapped`,
`Unviewable`, and `Viewable`, promote mapped descendants when an ancestor
becomes viewable, and gate visibility/exposure on that final state.

Sophia now follows those boundaries. Deferred policy admission is a separate
flag rather than a fake X map state. Ancestor map, unmap, and reparent changes
propagate viewability through mapped descendants. Create, map, configure,
visibility, and exposure events are filtered by each client's masks and
structure events are also delivered to parent substructure selectors. A
managed configure denial remains an explicitly synthetic response, while an
unchanged client-controlled configure stays silent. The Firefox-shaped
regression requires an early-mapped render child to remain Unviewable until
top-level admission, then observes ordered configure/map, subtree visibility,
and subtree exposure. A separate two-client wire regression locks down parent
and owner delivery and makes any queued create-time configure fail the next
geometry assertion.

The physical gate now rejects GDK thaw warnings, popup-era layout timeout or
WM restart, and a popup layout that precedes exact matching visual retirement.
The offline X-authority and fail-closed verifier suites pass. A fresh physical
workflow remains the acceptance boundary.

## 2026-08-02: Firefox disproved the transient-only popup diagnosis

The physical run from `d38a217c` used a freshly built binary and still admitted
Firefox surface `8388650` as a fourth policy-managed tile. That is decisive:
the live popup did not produce a valid `WM_TRANSIENT_FOR` reduction, so the
root-transient fix was correct protocol handling but was not the observed
Firefox fix. The ensuing four-surface resize epoch timed out with the popup at
zero committed size, restarted xmonad, retried, and then entered a repeating
single-Firefox resize loop. The GDK thaw warning occurred immediately after
the admission control, consistent with the blanked browser frame.

The missing authority input is EWMH `_NET_WM_WINDOW_TYPE`. EWMH requires this
pre-map functional hint to influence WM behavior, but Sophia stored the
property without reducing it and the blind WM cannot inspect application X11
properties. X Authority now decodes the ordered ATOM list, skips unknown
extension types, keeps `NORMAL` policy-managed, and reduces dialog/menu/
utility/splash/popup-like types to `ClientPositioned`. Replacement and deletion
publish role snapshots, and a redacted trace records the live reduction. A
wire regression sets an unknown preferred type followed by `DIALOG` before
map, proves immediate client-positioned mapping, and proves deletion restores
normal policy.

There is also a recovery-loop bug independent of the hint. A timed-out
admission can safely publish retained pixels at its initial extent, but the
retirement path immediately cleared that recovery extent while its original
WM target was still outstanding. The automatic relayout therefore drove the
same failed resize again. Recovery extents now remain pinned until the standing
target actually commits; the retained surface can stay visible and a future
explicit optimization may retry convergence without blanking the committed
layout. A fresh physical run remains the acceptance boundary.

## 2026-08-02: A root-owned transient is still client-positioned

The next physical Firefox run proved control-time refocus and reached the real
popup. Clicking `Open proof dialog` then blanked the owner. The trace showed
the popup entering ordinary WM admission: xmonad tiled it with the three
existing application surfaces, Firefox did not produce any of the four exact
requested extents, and each preserved-layout timeout restarted and reseeded
the bridge. The restart fix prevented the earlier `UnknownSurface` exit, but
could not correct the popup's wrong presentation role.

`WM_TRANSIENT_FOR` has two independent facts: the property marks a transient,
and its window value may resolve to an Engine surface owner. Sophia had
conflated them by classifying a window as client-positioned only when owner
resolution succeeded. ICCCM group transients legitimately point at the root,
which has no application surface, so the hint was reduced to no owner and the
popup was incorrectly promoted into blind WM admission. Window state now
retains transient-hint presence separately from the optional reduced owner.
Root-owned and otherwise unresolved transients remain client-positioned while
publishing no false owner edge; deleting the property restores normal policy
management. A mapped root-transient wire regression covers both transitions.
A fresh physical popup run remains required.

## 2026-08-02: Rejected WM proposals require a bridge reseed

The first physical run with control-time XI2 focus passed the repeated focus
handoffs and reached the second Firefox launch. Sophia then exited cleanly with
status 1 and `UnknownSurface`; there was no panic or memory fault. The earlier
Firefox popup admission had timed out, so Engine correctly preserved its prior
workspace state, but the xmonad bridge had already added the popup to its
private synthetic X11 model. When the second Firefox surface arrived, xmonad
returned tiling commands for that stale popup and strict workspace validation
terminated the owner loop.

An external WM necessarily applies a request before Sophia can prove the
resulting resize. A rejected or timed-out WM proposal therefore invalidates
the peer's speculative model even while Engine state remains correct. Timeout
results now retain their WM proposal source. The owner uses that evidence to
request the existing bounded transport restart, discards queued and in-flight
requests, and reseeds the restarted bridge from the last committed layout.
Non-WM resize timeouts remain local and do not restart the bridge. A regression
locks down source retention and the reseed decision; a fresh physical workflow
must confirm that popup timeout recovery and the second Firefox launch remain
live.

## 2026-08-02: XI2 focus must be emitted by the authority transition

The latest physical Firefox run reached resize stage 5 and then committed every
repeated `Super+J` action, xmonad layout response, Engine focus reconciliation,
and X control acknowledgement. It still ended at 6/8 because the page never
observed a DOM blur/refocus pair. This disproves the WM and Engine paths as the
remaining cause.

The cross-client broker fix delivered core `FocusOut` to the old client and
core `FocusIn` to the new client, but XI2 focus remained synthesized lazily by
the input writer on a later key packet. A compositor-owned Super chord is not
delivered to Firefox, so no packet existed to trigger that synthesis. The one
earlier physical refocus success was therefore nondeterministic later-input
behavior, not a locked focus transition.

XLibre confirms that `SetInputFocus` calls `DoFocusEvents`, which emits core and
device focus events together. Yserver independently reduces each focus crossing
into mask-filtered core plus XI2 events at mutation time, including the
ancestor-derived detail and current pointer coordinates. Sophia now follows
that boundary for Engine-originated surface focus: the passive broker packet
carries one monotonic timestamp, each client writer snapshots protocol-local
pointer/modifier/button state, builds selected core then XI2 records before
taking the socket lock, and writes them without waiting for input. Keyboard
delivery no longer owns focus synthesis; its public transition mask is narrowed
to pointer crossings.

The two-client wire regression selects and deselects XI2 focus while exercising
repeated A-to-B-to-A transitions with no key injection. The Firefox page and
QEMU harness no longer contain the diagnostic `r` bypass, and the physical
verifier requires ordered focus-away, selected XI2 out/in, focus return, and
only then the DOM checkpoint. A fresh physical workflow remains the acceptance
boundary.

## 2026-08-02: Firefox resize requires Present notification on its render child

The physical three-window trace separated the remaining short black Firefox
frame from Engine resize admission. Xmonad repeatedly requested a
1276-by-1422 left pane, but Firefox submitted only 1280-by-1040 PresentedBuffer
candidates. Engine correctly preserved the retired 1280-by-1040 recovery frame
and kept the standing resize target pending; the committed launch frame carried
no visible browser pixels, so the safe result was a short black pane rather
than a falsely stretched or partially committed browser.

The missing transition was in the X frontend. Firefox presents from a
descendant render window. Sophia accepted the descendant's client-controlled
`ConfigureWindow`, updated its geometry, and sent core `ConfigureNotify`, but
did not send the Present `ConfigureNotify` selected on that exact child.
XLibre confirms that Present wraps the screen `ConfigNotify` hook and therefore
notifies every matching subscriber before core event delivery. Yserver provides
an independent native-Rust confirmation: its Firefox investigation found Mesa
retaining the old swap-buffer size and rendering blank until the same Present
notification was emitted on real window reconfiguration.

Present configure delivery therefore remains X-authority policy. The frontend
now notifies every Configure-mask subscriber for the exact reconfigured window,
uses each receiving client's sequence, preserves Present-before-core ordering,
and suppresses the Present event for failed or no-op geometry requests. The
Engine's protocol-neutral standing-target, visual-evidence, recovery, and
retained-frame rules remain unchanged. Focused socket regressions cover a
Firefox-shaped child resize, cross-client subscription routing, no-op and mask
filtering, and Engine-originated configure ordering. A new physical run remains
the acceptance boundary.

## 2026-08-01: Firefox failures converged on XI state, popup lifecycle, and recovery constraints

The latest physical observations separated three failures that the old
eight-stage fixture could accidentally conflate. Wheel packets could reach a
page without moving the document; Firefox popup/toplevel lifecycle changes did
not always publish a compositing snapshot; and a temporary exact-size recovery
extent could survive successful visual admission, pinning Firefox at
1280-by-1040 while xmonad resized only the two Kitty surfaces. The old resize
verifier compounded this by looking for focus action 1 even though
Super+Space is xmonad action 3.

The X frontend now reports current valuator values from `XIQueryDevice`, full
hierarchy-relative `XIQueryPointer` coordinates/child/button/modifier state,
and immediate-child plus button-mask state in XI2 device events. Root-child
`WM_TRANSIENT_FOR` is reduced to a protocol-neutral presentation-owner edge.
Attached client-positioned surfaces publish map/unmap snapshots, follow the
owner's workspace visibility, never enter blind-WM admission, and stay hidden
after owner removal until the client publishes a new ownership snapshot.

Successful CPU admission or exact DMA-BUF retirement clears the Engine-owned
temporary recovery extent and requests one coalesced relayout. Clean shutdown
now fails if any such extent or relayout obligation remains. The offline page
requires a real local navigation, a post-baseline DOM wheel event, and nonzero
document displacement while the verifier independently requires both physical
axis routes. The strict physical verifier requires a three-surface
resize epoch/layout, action 3, a three-visible-surface projection, and zero
recovery constraints. Focused Engine, X wire, transient lifecycle, query reply,
and verifier mutation tests pass locally. A fresh physical run remains the
Milestone 10 acceptance boundary; these changes do not promote historical
evidence.

The first strengthened QEMU run then proved the navigation click and both
axis routes, but timed out at scroll. The fixture incremented its wheel counter
only for nonzero DOM deltas, even though GTK can consume the first XI2 absolute
value as a zero-delta baseline. After correcting that counter, a second run
exposed the independent harness race: both notches were routed within 160 ms of
the click, before the replacement document had an observable ready point. The
navigated fixture now publishes an out-of-band title-length checkpoint, the
session reports a redacted `navigation_ready` marker, and QEMU waits for it
before injecting exactly two notches. Because XI2's first absolute value does
not produce a DOM wheel event, the page requires the second notch's DOM event
plus `scrollY > 0`, while the verifier independently requires both routed axis
packets between navigation readiness and the scroll checkpoint. Thus packet
delivery without real document displacement still cannot pass.

That corrected run passed scroll, resize, and refocus, then exposed an existing
fixture gap: Firefox rendered JavaScript `alert()` as a tab-modal overlay, so it
could not prove the X11 transient-toplevel lifecycle at all. The dialog step now
opens a real click-gated Firefox popup with an autofocus confirmation button.
The harness waits for its attached four-surface layout snapshot before sending
Return, then waits for the popup's X focus acknowledgement because layout
publication precedes focus application. The popup finalizes its blank document
before installing its confirmation handlers. The Enter handler publishes the
final redacted title-length checkpoint on the popup itself before closing,
avoiding cross-process messages and throttled opener timers during teardown.
Close is delayed by one second so Firefox can publish `_NET_WM_NAME`; the
harness then requires the return from four to three surfaces and publishes
dedicated redacted `dialog_open` and `dialog_closed` checkpoints. This directly
exercises the popup-lifecycle snapshot that the strengthened session path is
intended to guarantee.

The first real-popup QEMU run found one more ordering boundary: the attached
surface and X focus acknowledgement both preceded Firefox installing the
popup's DOM key handler, so an immediate Return was lost. The later retry then
opened a second popup and made an already completed stage look like a close
timeout. The popup now publishes a distinct redacted `dialog_ready` title only
after its confirmation handler is installed. QEMU waits for that readiness,
uses a pre-interaction stage baseline instead of accepting stale completion,
and fails the confirmation attempt instead of reopening an ambiguous popup.
The ready-popup run also proved that X toplevel focus does not guarantee
Firefox's internal keyboard focus proxy will deliver an immediate synthetic
Return. Because this stage is a pointer and popup-lifecycle proof (keyboard is
already proven separately), QEMU and the operator now click the popup's
full-window confirmation button. The title checkpoint wait is forty seconds:
under llvmpipe load the redacted metadata batch can trail the visible popup
close by more than twenty seconds.

The physical verifier now carries the same boundaries into the promotion
contract. It requires replacement-document readiness before counting its two
wheel routes, then orders popup document readiness, a four-surface layout,
dialog confirmation, and the return to three surfaces before Firefox's normal
exit. Mutation fixtures independently remove each readiness and layout record,
so the physical gate cannot regress to accepting the former overlay-only
dialog or pre-navigation wheel delivery.

The first run against that contract reached real document scroll but exposed
an admission-time timeout rather than an input failure. The initial diagnosis
widened manage-surface resize fences from two to eight seconds because Firefox
had published a 1280-by-1040 buffer but had not satisfied the three-window
1276-by-1422 epoch. That avoided an early rollback but did not make Firefox
honor the size. The physical verifier also counts xmobar's retained
non-workspace surface explicitly: four surfaces at the normal Firefox
baseline, five while the real popup is attached, then four again after close.

The next physical launch exposed two coupled owner-loop bounds. Application
admission still used the five-second proof-completion timeout even though a
manage-surface resize may now wait eight seconds (and the session accepts at
most ten). It declared Firefox timed out while that layout fence was valid.
At nearly the same point, a pointer focus handoff accumulated a full bounded
batch and propagated its capacity result as a fatal session error. Application
admission now has a distinct twelve-second bound, strictly beyond the maximum
WM transaction. Adjacent held pointer motions coalesce; an exceptional full
handoff is discarded atomically and reported without terminating the desktop.
Focused regressions cover both deadline ordering and the bounded input path.

The immediate rerun proved the longer fence was masking the actual authority
violation. Sophia admitted Firefox at the WM's 1276-by-1422 geometry, then
accepted Firefox's own mapped-toplevel `ConfigureWindow` and let it overwrite
that Engine-owned geometry with 1280-by-1040. The epoch could therefore never
match and the browser stayed hidden for the entire eight-second fence. Mapped
policy-managed toplevel geometry is now immutable from the client path;
children, override-redirect windows, and pre-admission windows retain their X11
geometry authority, while a denied toplevel request receives the current
Engine geometry in `ConfigureNotify`. The xmonad admission fence returns to two
seconds as a bounded fallback.

The same trace exposed a recovery ordering defect: selected admission pixels
were drained while the surface still remained in the unassigned WM set. The
later policy projection made Firefox visible only after that one-shot frame was
gone. Released admission groups now remain quarantined until policy assignment,
then enter production exactly once. Pointer focus handoff remains separately
bounded at four seconds so the two-second layout fallback cannot win a timeout
race with a click made during launch.

## 2026-08-01: First physical Firefox rendering/input diagnosis

The isolated-profile physical M10 run advanced the deterministic Firefox page
through its loaded and keyboard stages and produced a real 1280-by-1040 DRI3
Present frame. After xmonad tiled the Firefox toplevel to 1276-by-1422, Sophia
rejected every subsequent frame because the final unit-scale predicate compared
the DMA-BUF dimensions with the clipped surface extent. The renderer placement
was already pixel-aligned: it retained a 1280-by-1040 target and clipped that
target to the client-positioned X child. Unit scale is now proved against the
unclipped target, so size transitions clip without stretching while Firefox is
between swapchain extents.

The same run showed that a physical click could hit Firefox's client-positioned
render child while Engine focus remained on Kitty; the later `Ctrl+Q` was then
correctly delivered to Kitty. Pointer delivery still targets the exact child,
but focus handoff now resolves that child to the highest containing
policy-managed surface owned by the same frontend client. X reparent operations
also publish their resulting presentation role, policy-to-client transitions
withdraw stale WM ownership, and WM requests reject client-positioned nodes.
This keeps X hierarchy semantics in the X/session frontend rather than the
protocol-neutral Engine.

Finally, the VT suspend/exit path found the Firefox request stream filling the
bounded observation channel. Observation overload now remains fail-closed and
bounded by disconnecting only the overloaded X client; it no longer turns a
client worker error into failure of the persistent authority service. Focused
regressions cover clipped unit-scale frames, descendant focus resolution,
policy withdrawal, and reparent role publication. A new physical run is still
required before any M10 workflow item is closed.

## 2026-08-01: Milestone 9 commit-pinned promotion passed

Commit `727c716d2f762bbed47e1132d7770dc8b92f5015` passed the complete
Milestone 9 promotion ledger. The unattended gate retained the M7 xmonad, M8
Firefox/Vulkan mix, and dual-output libinput-to-kernel-page-flip QEMU evidence.
The physical gates retained native chrome and hot reload, four visible and
interactive Kitty surfaces with pointer focus and VT suspend/resume, xmobar
work-area and pointer behavior without keyboard-focus theft, and graceful
Ctrl-Alt-Backspace emergency recovery with exact TTY restoration.

The final four-Kitty run also resolved a verifier-version mismatch rather than
a runtime failure. Current schema-4 evidence reports one persistent composition
target and frame surface, 34 target reuses across 35 mixed exports, balanced
import-cache imports and evictions, zero replacements or recreation, and 35 of
35 renderer-worker completions. The verifier now checks those persistent
resource invariants and its mutation suite rejects reuse gaps, cache debt,
worker failure or incompletion, and excessive worker latency. Native chrome
and hardware evidence adopted from the immediately preceding runtime-identical
commit retain explicit source provenance; xmobar and emergency evidence were
captured directly on the promoted commit.

This is the development-session promotion point, not installed daily-driver
promotion. The recorded lifecycle still uses source builds and manual service
ownership. Physical Firefox, installed-session cycles, and workday soak remain
Milestones 10 through 12.

## 2026-07-31: Synchronized input latency uses an end-to-end and stage contract

The former physical gate required full-chain p95 below one 17 ms refresh. A
randomly phased input event can spend nearly that entire interval waiting for
the next synchronized page flip after useful work has completed, leaving an
unrealistic sub-millisecond p95 allowance for input delivery, client response,
composition, and submission. The aggregate bound therefore encouraged tearing,
VRR, or workload-specific bypasses before the normal synchronized path was
otherwise ready.

The physical contract now requires full-chain p95 below two configured refresh
periods and independently fails when maximum queue dwell exceeds 1 ms,
dwell-to-submit exceeds one refresh, or submit-to-page-flip exceeds one
refresh. The first draft used half a refresh for dwell-to-submit, but physical
evidence showed that this interval also includes the external client's response
rather than only Sophia-owned work. One refresh is the meaningful correctness
boundary: it rejects an additional processing frame, while the aggregate bound
still rejects two complete stages at their limits. The two-refresh bound is
exclusive; stage bounds are inclusive. The reporter emits the refresh, derived
aggregate budget, every stage budget, and named failed gates under schema 2.
Existing immutable schema-1 evidence remains valid historical evidence and is
not rewritten.

## 2026-07-30: CPU patch residency is validated after transaction reduction

The first commit-pinned Milestone 9 semantic rerun exposed a frontend-timing
race during two-xterm startup. Renderer residency was derived from committed,
staged, and current transaction buffers. A replacement or patch carried in an
update-only intake was not itself a residency root, so the replacement could
be installed and reclaimed before a later patch arrived. A superseded patch
whose base had already been reclaimed then terminated frame composition with
`MissingPatchBase`.

Current replacements and patches refresh a 16-handle renderer-private recent
update set. Production joins that bounded working set with committed, staged,
and incoming transaction roots, bridging update-to-transaction queue gaps
without granting scene visibility or retaining an unbounded X resource cache.
Production discards a late patch only when its base is absent; the strict
renderer registry still rejects missing bases for direct callers. After Engine
transaction reduction and residency reconciliation, production counts
committed CPU surfaces without buffers and fails the cycle before composition
if the count is nonzero. Thus superseded traffic cannot kill the session,
while a relevant missing buffer still fails closed instead of producing absent
or mismatched pixels.

Regressions cover consecutive replacement/patch intakes, the bounded recent
update set, late unrooted patch disposal, and the post-reduction
missing-committed-buffer check. The exact M7 two-xterm QEMU acceptance then
completed startup, both
resize epochs, pointer click/drag focus, output-edge reversal, workspace
projection, WM restart, launch/close, and clean logout.

The first commit-pinned M9 rerun subsequently exposed nondeterminism in the
QEMU pointer-focus harness: it reset only the horizontal coordinate, so an
inherited `y=0` placed the scripted click on compositor chrome and correctly
produced `button_suppressed reason=no_target`. Focus click and drag setup now
reset both axes, move 32 units horizontally, and use eight separate 16-unit
vertical steps before sending a separate gesture command. This avoids relying
on one acceleration-sensitive relative movement to leave the top edge.
The same rerun reached Firefox's refocus proof and showed that back-to-back
focus chords could race the X11 handoff, while a fixed two-chord pair could
start and end on the terminal. The proof now cycles one surface at a time,
waits for `focus_applied`, and sends an `r` probe accepted only by the page's
refocus stage. This proves an acknowledged focus change plus delivery to the
returned browser without depending solely on Firefox surfacing a DOM focus
event under headless QEMU.

The first physical input-latency sample then completed its exact key and pixel
proof but raced renderer teardown: KMS was drained while one asynchronous
renderer frame was still finishing, so image cleanup returned `WorkerPending`.
Renderer maintenance now settles and discards an unsubmitted worker result
within the existing one-second maintenance boundary before clearing cached
images. A failed or stalled worker remains a hard teardown error.

The following commit-pinned M8 rerun exposed a second restart ordering window:
the compatibility bridge could exit after policy-event polling but before a
request submit or completion poll. Those request-channel disconnects now enter
the same supervised, layout-preserving restart path as policy-channel
disconnects instead of terminating the live session.

## 2026-07-30: Unattended QEMU input-latency regression

- **Promotion coverage.** The commit-pinned Milestone 9 semantic gate now
  archives and verifies this isolated regression after its M7 and M8 scenarios.
  A candidate cannot pass gate zero with only application semantics while
  omitting the current libinput/page-flip correlation contract.
- **Local gate hygiene.** The promotion rerun exposed a stale generic-QEMU
  pass fixture and two unreviewed source-layout overages before virtualization
  began. The fixture now carries the latency and kernel-clock records, session
  configuration tests have their own domain file, and layout support is split
  from the persistent layout owner. The complete local gate passes again.
- **Host uinput is no longer required for development validation.** The QEMU
  session injects QMP keys through the guest virtio keyboard, evdev, and the
  normal threaded libinput poller. `tools/run_sophia_input_latency_qemu.sh`
  rebuilds the guest by default and retains commit-pinned evidence under the
  user's state directory.
- **No-WM admission deadlock fixed.** Presentation-intent quarantine is an
  external-WM ownership boundary. Proof sessions without a WM now commit
  policy-managed X11 pixels directly instead of waiting forever for an absent
  policy process to admit them.
- **Software scanout is correlated directly.** CPU-composed xterm frames do
  not create a GPU Present retirement record. The native head now retains the
  accepted kernel page-flip UST as well as its submission UST, allowing the
  input proof to select the changed post-ingress software frame exactly.
- **Retained result.** The isolated two-output guest reached startup readiness
  in 74 ms, routed and flushed all 14 keyboard events, matched `sophia`, proved
  the pointer path, and reported a 6 ms full chain with 8 kernel timestamps,
  zero fallbacks, and zero pending correlations. QEMU validates the clock and
  correlation plumbing; it does not replace the physical 20-sample p95 gate.

## 2026-07-30: Input-to-photon clock provenance

- **Kernel presentation timestamp preserved.** The native DRM event adapter now
  retains `PageFlipEvent::duration` as microseconds on its private callback,
  carries it beside the public callback through bounded polling, and correlates
  it by output/frame serial before presentation retirement. Production
  retirement therefore uses the DRM kernel's monotonic page-flip UST instead of
  `presentation_started.elapsed()`.
- **Fallback is observable, not silent.** Synthetic presentation timestamps
  remain available for fake and non-kernel callback sources, but production
  completion now emits `sophia_live_page_flip_clock` with kernel timestamp,
  fallback, and pending counts. The physical latency gate must require positive
  kernel timestamps with zero fallbacks and zero pending correlations.
- **Raw ingress is now separately injectable.**
  `tools/probes/uinput_text_injector.py` creates a bounded Linux virtual
  keyboard and publishes its event node so the session opens it through the
  same threaded libinput path as hardware. The poller retains a private
  per-event timing sidecar (serial, kernel event time, queue dwell); protocol
  packets remain passive and unchanged. `--inject-text` remains synthetic and
  is never counted as this proof.
- **Exact-frame correlation and gate.** The physical proof anchors on the last
  routed key press, waits for X delivery, requires a changed output frame
  submitted after that raw ingress, and computes its retirement from the
  matching kernel page-flip UST. GPU Present surfaces retain the stable-surface
  proof, while software-composed frames correlate on the native output
  submission/page-flip pair. Completion reports queue dwell,
  dwell-to-submit, submit-to-page-flip, and full-chain latency.
  `tools/run_sophia_input_latency_tty3.sh` collects 20 independent
  commit-pinned uinput/libinput samples, rejects any fallback/pending page-flip
  timestamps, and requires full-chain p95 below the configured refresh period.
  `tools/setup_sophia_uinput.sh` installs the persistent Void/udev module,
  device-node, and `input`-group policy required by that unprivileged runner.
  The code path and injector ABI self-test pass offline; physical p95 evidence
  is still required before closing the todo item.

## 2026-07-30: Terminal CPU benchmark hard-locked the machine — xterm geometry char/pixel confusion

The first physical run of the 9.4 terminal CPU-path benchmark
(`tools/benchmark_sophia_terminal_tty3.sh`) hard-locked the machine and
required a power reset. Recovered evidence from
`~/.local/state/sophia/standalone-session/` isolated two distinct problems.

- **Functional root cause (fixed).** `tools/probes/run_bounded_xterm.sh` passed
  the intended *pixel* size (`SOPHIA_XTERM_WIDTH/HEIGHT=500`) straight into
  xterm's `-geometry`, but xterm reads `-geometry` in *character cells*. With
  the default font that requested a 4004x5004 px window. `apply_text_draw`
  backs each CPU window with one immutable software buffer bounded by
  `X_AUTHORITY_SOFTWARE_BUFFER_MAX_BYTES` (64 MiB, `sophia-x-authority`
  `software.rs`); 4004x5004x4 ≈ 80 MB overran it, so `draw_text` returned
  `None`, the `ImageText8` was rejected `BadWindow`, xterm aborted (exit 83),
  and the session ended with `"live session ended without a committed external
  WM layout"` (exit 1). The X authority failed *closed* here — it refused the
  buffer rather than allocating it.
- **Deterministic offline reproduction.** Driving the committed
  `x-authority-xterm-input-smoke` at `500x500` reproduced the crash
  bit-identically (opcode 76 `X_ImageText8`, resource `0x200014`, serial `362`)
  with no KMS involved. `40x8`/`100x50`/`200x150` pass. The authority runs the
  same dispatch path offline, so this class of bug is debuggable without a
  physical takeover. Note the `sophia_terminal_performance_pass.log` fixture was
  aspirational: the benchmark had never had a real green run.
- **Fix.** The probe now converts the pixel intent to a character geometry
  against a pinned fixed-metric core font (`-fn 6x13 -b 2`) and clamps the pixel
  intent well under the cap. Default 500 → `82x38` cells → 496x498 px → 988 KB;
  the worst-case clamp (2048 px) stays at 16.7 MB. `SOPHIA_XTERM_WIDTH/HEIGHT`
  remain the reported pixel intent on the `sophia_terminal_benchmark` line.
- **Fail-closed compose budget.** The terminal performance report rejects
  `cpu_max_compose_msec` above 25 ms, matching the established
  CPU-composition gate used by the retained two-xterm and QEMU evidence. It
  records `cpu_compose_budget_msec` beside the observed maximum; malformed or
  zero overrides fail before evidence is accepted.
- **Commit-pinned physical runner.**
  `tools/run_sophia_terminal_gate_tty3.sh` refuses a dirty worktree, requires
  both persistent logging services and a nonempty kernel log before takeover,
  and archives the source commit, benchmark/report results, session/guard/TTY
  recovery, launcher handback, and the exact appended kernel-log bytes. A
  rotated log or new AMDGPU rejection/reset/timeout fails closed.
- **First post-geometry-fix physical result: bounded transport overload.** The
  run on commit `d7fbcff` passed offline preflight, armed the input guard,
  acquired both outputs, completed the synchronous output baseline, and then
  stopped the X frontend at transaction 650:
  `X authority observed transaction channel is full`. The probe's tight loop
  wrote 200 lines per iteration with no interval; a few bursts filled the
  intentional 256-batch frontend-to-owner queue before ordered visual facts
  could drain. X authority correctly refused to allocate or drop facts, xterm
  exited 84, native suspend drained, and TTY3/greetd handback completed. This
  is neither the prior geometry failure nor evidence for enlarging an
  eventually finite queue.
- **Paced workload decision.** The terminal probe now defaults to eight lines
  every 16 ms and carries both values through schema-2 benchmark/client records
  into the schema-3 performance report. The reporter rejects mismatched cadence
  or inconsistent line totals. Zero and over-one-second intervals are invalid,
  so an override cannot silently restore the unbounded producer. The gate
  runner also leaves a structured `interrupted` result and copies available
  session artifacts from its exit trap.
- **First paced physical results: rendering passed, controller completion
  fixed.** On commit `839d21a`, the first run was manually logged out at 14.3
  seconds after visually confirming the scrolling xterm and responsive pointer;
  it drained native scanout and restored TTY3/greetd cleanly but correctly had
  no 20-second client record. The next run was left for the full window:
  authority batches dropped `0`, unexpected protocol errors were `0`,
  `cpu_max_compose_msec=6`, native failures were `0`, the kernel delta was
  complete and clean, and handback was clean. It still failed the report
  because xterm backpressure held the producer inside `seq(1)` past its
  wall-clock loop test; the outer 25-second safety timeout ended xterm before
  the producer's final-only count write. The probe controller now runs the
  producer under an independent bounded timer and records each completed burst
  incrementally. A stalled-pty offline regression exercises that path. A real
  software-only Sophia session then emitted the client completion and cleaned
  up normally even though xterm lingered until its process safety timeout.
  The standalone launcher now explicitly tells bounded-xterm operators to let
  it exit automatically instead of showing the generic logout hint.
- **Native path is not the lock cause.** Audit: CPU-layer GL textures are
  reallocated to the incoming layer size (`sophia-renderer-native-egl` `gl.rs`),
  but that layer *is* the ≤64 MiB software buffer (≤ ~4096², inside RDNA3's
  16384 GL limit); DMA-BUF layers are client-bounded; scanout framebuffers are
  output-sized. In the crashed run the oversized CPU buffer was refused, so
  nothing oversized ever reached the GPU — only one output-sized blank frame
  presented.
- **The hard lock is downstream of the abnormal early exit at KMS handback.**
  There is no DRM/VT code in Rust; the launcher stops greetd/lightdm, Sophia
  becomes DRM master implicitly, and on exit drops master by closing the fd
  while the launcher restarts the display manager. Teardown
  (`detach_native_scanout`, `production_visual_runtime/native.rs`) drains
  in-flight scanouts, rejects pending presents, and rebuilds the output set
  without scanout — but it does **not** disable the CRTC or restore the prior
  mode. Sophia leaves its last framebuffer active on the CRTC and drops master;
  greetd/Xorg then re-modesets the RX 7900 GRE from that state. This teardown
  path is identical for normal exits, which hand back cleanly, so the trigger
  was the *early* abnormal exit (session aborted right after the first blank
  frame, before the steady-state page-flip loop) hitting the RDNA3 re-take in a
  fragile transient state.
- **Decision.** The probe fix removes the trigger; the benchmark should now
  reach steady state and hand back like every other clean run. Optional
  hardening for the handback (deferred to a validated physical change): issue an
  explicit CRTC-disable / mode-restore atomic commit before dropping master.
- **Environment (not in-repo).** Host is Void Linux (runit, no journald), so
  the prior-boot kernel dmesg was lost on reset. Persistent logging is now
  enabled via `socklog-void` (`/var/log/socklog/kernel/current`); setup helper
  is `~/sophia-amdgpu-logging-setup.sh`. GPU: Radeon RX 7900 GRE (Navi 31,
  RDNA3, `03:00.0`) plus a Raphael iGPU (`16:00.0`); `amdgpu` `gpu_recovery`,
  `lockup_timeout`, `reset_method`, `runpm` are all auto (`-1`).
- **Controller-fixed physical gate passed.** Two commit-pinned runs on
  `4cb4f5f` completed the 20-second producer and emitted passing schema-3
  reports. Both recorded 6,648 lines / 831 completed iterations, positive
  immutable CPU patch traffic, damage-driven partial repaint, zero authority
  drops, zero unexpected protocol/native failures, clean kernel deltas, and
  clean TTY3/greetd handback. Maximum CPU composition was 7 ms against the
  25 ms budget. The runner archives retained `visual-confirmed=false` because
  the local prompt did not record `yes`; the operator had separately observed
  the expected scrolling-number surface and responsive pointer in the paced
  session. That prompt metadata is not rewritten. The named automated
  acceptance criteria are complete; no Xserver parity claim is made.

## 2026-07-30: GLX Animation Passed; Startup Evidence Lost a Valid Early Frame

The bounded six-second physical rerun visibly animated. Radeonsi reported
52.956 client FPS on the RX 7900 GRE. Sophia completed 242 renderer requests
with 242 completions, no worker failure, soft stall, hard stall, or
release-queue failure; native teardown drained without an abandoned scanout,
and X protocol errors remained zero. Neither the 500 ms page-flip watchdog nor
the independent 11-second session deadline fired.

The run still exited status 1 because the completion gate claimed startup
readiness was never reached. Transaction 46 had in fact produced 70,988
nonzero RGB pixels, retired through KMS, committed visual admission, and was
logged as a stable mixed scanout. That happened immediately before the X focus
control acknowledgement pinned the startup surface. The startup reducer
correctly rejected pre-pin evidence to prevent another client from satisfying
the gate, but the owner retained only the latest transaction per surface.
Continuous animation made every later retirement overlap a newer pending
frame, so the already-proved stable frame could not be reconstructed.

Startup presentation evidence is now retained monotonically in a
surface-keyed map until readiness or native recovery. It records only a frame
that was stable at its actual KMS retirement and preserves the maximum observed
nonzero count. Once focus pins the startup surface, only evidence with that
exact surface identity can supply visual-detail and stable-presentation
events. A status bar or another client therefore cannot satisfy the startup
application, while asynchronous focus and presentation ordering no longer
creates a false failure.

The next physical rerun showed that the map alone was insufficient. It
retained transaction 46 correctly, but the consumer redundantly required a
current base committed-surface record. Removing the earlier `BufferSource`
subtype check had left this surrounding membership gate in place. DRI3 Present
content is a presentation-layer lease; after admission, the base committed
surface may legitimately be absent. A third bounded run still animated at
53.542 client FPS, completed 242/242 worker requests with no stalls or failures,
and drained all imports, while this gate alone produced the false startup
failure. The stable nonzero KMS retirement already proves both GPU content and
surface identity, so startup visual-detail reduction now uses that evidence
even when no base committed record exists. A regression covers that exact
state instead of approximating it with a DMA-BUF-backed base surface.

The following run again animated normally at 52.981 client FPS and drained
241/241 renderer requests, but exposed the underlying control-flow split.
Transaction 46 retired while the owner was waiting for the next authority
batch. That authority-wait path logged the same stable scanout as the ordinary
lifecycle path but did not update startup presentation evidence. The two
nearly identical retirement blocks had drifted semantically. Retirement is now
recorded by one shared function used by both service sites; it owns admission
retirement, surface-keyed startup evidence, reducer input, and the structured
retirement/scanout records. Phase-local input-proof bookkeeping remains at the
call sites.

The corrected physical run reached GPU content readiness and full startup
readiness in 178 ms without native recovery. It visibly animated at 59.088
client FPS, accepted 352/352 renderer requests, completed 351 Present Flips and
Idle notifications, reported zero protocol errors, drained every imported
image, and exited status 0 with clean session and TTY recovery.

That successful run exposed a final benchmark-only mismatch: verbose tracing
was intentionally disabled, but the cadence reporter still parsed per-frame
Present diagnostic lines. Presentation cadence is now accumulated in bounded
owner state from routed retained-buffer UST values and emitted once at
completion. The report keeps exact sample/interval counts, nonadvancing and
overflow flags, mean FPS, and p95 frame time without adding per-frame logging
overhead. Reporter regressions reject insufficient, nonadvancing, or overflowed
summaries.

The final six-second run validated the aggregate path end to end. Startup
reached readiness in 161 ms without recovery. The client reported 59.197 FPS;
352 routed retained-buffer samples produced 351 advancing UST intervals, zero
nonadvancing observations, 59.953 presentation FPS, and 17.324 ms p95 frame
time. Sophia completed 353/353 renderer requests, 352 exact Flip/Idle pairs,
zero protocol errors, zero live imports, and clean exit/TTY recovery. The
schema-3 performance reporter returned `status=pass`.

Teardown also exposed stale worker metrics: `ClearImages` was queued without an
acknowledgement, then the owner immediately sampled the previous cache state.
The maintenance command now returns its eviction result and updated persistent
statistics over a one-slot bounded channel with a one-second deadline.
Changing GLX buffers legitimately produced 242 imports and zero cache hits:
Idle releases each generation before the client may mutate and reuse it. The
report therefore requires positive imports and clean zero-debt teardown, while
retaining cache hits as an informative counter rather than inventing reuse.
The benchmark no longer forces verbose per-stage tracing. Normal structured
presentation and resource summaries remain enabled, while diagnostic tracing
can be opted into explicitly, so the performance probe does not measure its
own high-volume logging.

## 2026-07-29: GLX Freeze Was a Retained-Buffer Race on the Owner Thread

The failed physical `glxgears` run completed and retired two mixed frames. On
the next repaint Sophia emitted X Present `Copy` completion for a DMA-BUF it
continued to retain and sample, then entered a production `glFinish`. That
finish blocked the session owner for 10.097 seconds. Radeonsi reported a
cancelled command stream, lost context, and guilty hard recovery; during the
same interval the owner could not service the hardware cursor, physical input,
VT control, or X traffic. The static gears, frozen pointer, and session abort
were one failure, not separate input and animation bugs.

Xserver's Present implementation confirms that `Copy` means the server has
copied the pixmap and may idle it. Sophia had used that protocol result for a
zero-copy retained compositor lease. The protocol-neutral backend contract now
uses `Retained`, `Copied`, and `Skipped`. The X authority maps `Retained` to
Present `Flip`; software snapshots alone use `Copy`. Idle remains behind exact
KMS retirement.

Production rendering no longer executes `glFinish` in composition, cache
eviction, cache clear, or full-screen DMA-BUF drawing. EGL swap and KMS
retirement carry the normal same-GPU ordering; diagnostic readback is bounded
to the initial nonzero proof.

The stronger containment boundary moves production EGL/GL/GBM work to a
bounded renderer worker after the initial modeset. The worker receives an
owned immutable frame and retains the native BO under a lease ID. The session
owner receives only duplicated DMA-BUF FDs and a descriptor, so a driver stall
cannot stop input or cursor service. A request becomes a deferred scanout while
pending, reports a soft stall at 100 ms, and is quarantined after 1 second
without fake Present feedback. Completion metrics now include worker requests,
completions, failures, stalls, release-queue failures, and maximum request age.
The remaining multi-output optimization is to share one worker among outputs
that use the same render device.

The first worker-enabled physical rerun remained static but did not reproduce
the 10-second owner-thread stall. It exposed two asynchronous-boundary defects.
While the mixed Present was rendering, a CPU repaint replaced the output's
pending content label; the resulting KMS retirement was therefore recorded as
CPU and could not satisfy the exact visual-admission transaction. A second CPU
fallback then produced a valid GEM-backed scanout buffer whose DMA-BUF FD
export was unavailable. The old synchronous submit path correctly used the GEM
descriptor in that case, but the worker had incorrectly made FD export
mandatory and rejected the frame.

The first repair preserved mixed content ownership against CPU replacement
while the GPU frame was in flight. It also treated PRIME export as optional
and retained a descriptor fallback. That transport choice was provisional;
the later lockup evidence below showed that PRIME and shared-file descriptors
must be mutually exclusive topology modes, not preferred/fallback paths.

The next physical run proved those repairs: the seat and DRM device became
active, mixed transaction 46 reached KMS, and its page flip retired. The
concurrent elogind `failed to add session ... to hash map: File exists`
message was therefore nonfatal session-manager noise, not the presentation
failure. Admission still remained `not_committed` because the Present
scheduler had only queued and submitted states. A deferred renderer-worker
export was popped from the queue as if it had failed; when KMS later accepted
and retired that exact frame, no scheduler record remained to connect the
retirement to its prepared Engine commit.

The scheduler now retains one mutually exclusive in-flight record with
`Rendering` and `Submitted` variants. The full immutable Present record moves
`queued -> rendering` when the worker accepts it, `rendering -> submitted`
only when KMS accepts the returned buffer, and leaves the scheduler only on
retirement or controlled failure. A newer client frame may remain queued, but
cannot replace the prepared transaction, displayed layer, or resource
ownership of the frame already crossing the asynchronous boundary.

The following physical run proved admission and protocol feedback through two
advancing mixed Flip retirements, then froze the graphical session. Its log
identified two deeper ownership defects. Output content and damage still had
only pending/submitted slots, so newer compositor work relabelled transaction
51's worker result as `RetainedMixed`. More seriously, the worker and KMS use
duplicated descriptors for the same DRM file, but submission preferred a PRIME
round trip. Importing that exported DMA-BUF back into the same DRM file may
return GBM's existing GEM handle; KMS resource cleanup then closes a handle
still owned by the renderer. The observed sequence was framebuffer resource
creation failure, `DmaBufImageCreateFailed`, repeated EGL target replacement,
then a submitted page flip that did not retire before emergency recovery.

Output damage and native content now carry their own `Rendering` slot beside
`Pending`, `Submitted`, and `Presented`. A worker request moves the exact
snapshot and content into that slot; worker completion promotes that same
record even if newer work is queued. Scanout transport now declares its DRM
file topology explicitly. The current shared-file production path submits GEM
descriptors directly and never PRIME-imports its own buffers. A future
independent render-node path must provide PRIME FDs and fails closed if they
are unavailable; the two modes are not runtime fallbacks for one another.
A 500 ms page-flip watchdog terminates the session so a lost DRM event cannot
leave the graphical seat waiting indefinitely.

The operator reported the resulting graphical freeze as a complete system
lock. The process-local page-flip watchdog is necessary but cannot be the only
recovery boundary because it still depends on the session owner being
scheduled. The development TTY launcher therefore accepts an independent
wall-clock deadline. Its separate shell process records the deadline, sends
TERM and then KILL to the complete Sophia session process group, and lets the
existing parent cleanup restore keyboard, console graphics mode, keyd, and the
display manager. The bounded `glxgears` proof enables that deadline at workload
duration plus five seconds. This is containment, not evidence that the
rendering defect is fixed, and it cannot recover a kernel-wide scheduler
failure.

## 2026-07-28: GLX Compatibility Uses the Generic Standalone Workload Slot

Sophia's bounded GLX proof now launches `glxgears` through the same standalone
application lifecycle used by the fixed Vulkan proof. Workload selection and
bounded-process policy remain operator tooling concerns: Engine receives an
ordinary X11 client and retains no `glxgears`, Kitty, xmonad, or benchmark
identity. This preserves the frontend-neutral scene and presentation model.

The probe runs at a fixed 500-by-500 geometry with swap interval one and exits
after 20 seconds. Its report fails closed unless the log identifies the OpenGL
renderer, shows client animation samples, advancing routed post-KMS Flip
timestamps, positive native and mixed exports, Present idle-fence progress,
and a clean resource drain. Client-reported FPS and Sophia's actual
presentation cadence are separate fields because they describe different
boundaries.

This proof diagnoses direct GLX bootstrap and the DRI3/Present compatibility
path. It cannot replace the deterministic Vulkan parity workload, and no
GLX-specific fast path has been added to Engine or the renderer. Physical
metrics remain pending until the dedicated-TTY run is retained.

The first physical attempt failed before creating a window:
`glxgears` could not select an RGB double-buffered visual. Kitty had exercised
the modern FBConfig/context path, while the classic catalog advertised zero
depth bits and the authority did not decode legacy visual-based context
creation. The repair makes both GLX entry paths explicit data variants,
advertises a 24-bit depth buffer in matching visuals and FBConfigs, normalizes
legacy visuals to the bounded FBConfig runtime identity, and validates direct
MakeCurrent context/drawable pairs. A bounded external-client preflight must
now reach visual discovery, direct context creation, DRI3 import, and Present
before the script takes over the TTY. The real-Mesa preflight passes those
stages without an X protocol error. Mesa keeps MakeCurrent local for this
direct-rendering workload, so that supported request is not a required wire
observation.

The first run that reached native output exposed two coupled generic
presentation-lifetime faults. The initial gears frame was imported, composed,
and retired successfully, but an unrelated software cycle had queued a stale
CPU frame while that mixed Present was in flight. It replaced the visible
gears with a blank output. Sophia then retained the completed client DMA-BUF
until the successor's KMS retirement; Mesa needed that idle fence while the
successor was being imported, and radeonsi eventually reported a guilty
context hard recovery.

Production frame reduction now preserves an already-submitted GPU Present
instead of queuing CPU fallback behind it. When an acquired mixed Present
replaces the same surface, the runtime retires the prior composited source and
signals its idle fence before importing the successor. Present Complete remains
tied to the prior KMS page flip, so protocol timing and client-buffer reuse are
separate facts. Focused regressions require both the no-superseding-frame
policy and actual xshmfence progress while the successor remains ready.

## 2026-07-28: Xserver Parity Uses Present Evidence, Not Demo Wall Time

The software-Present optimization reached an observed 59.947 FPS with a
17.564 ms p95 interval on a 459-sample physical run, but the tree retained no
same-machine Xorg or XLibre baseline. Treating an ideal 60 Hz interval as that
baseline would hide provider, output, mode, and server-side presentation
differences.

The paired reference runner now launches the same fixed 500-by-500,
900-frame, FIFO `vkcube` workload under an Xserver. A bounded XCB probe attaches
only to newly created matching top-levels and records actual Present Complete
UST/MSC values. The shared cadence reducer computes FPS and p95 for both
systems. The comparison fails closed when workload, requested frames, Vulkan
provider, Vulkan/X Present mode, or output pixel count differ, then applies the
existing 90% rate and inverse-p95 gate.

`glxgears` is retained only as an optional Xserver GLX/OpenGL reference and
gross cadence probe. Its workload and driver behavior are not representative
enough to become Sophia's rendering acceptance metric, so its record is kept
separate and cannot affect the Vulkan parity decision. A bounded Sophia-side
direct-GLX/DRI3/Present pair is tracked explicitly rather than treating the
Xserver-only number as Sophia evidence.

The first retained Xserver capture used XFCE's composited Xorg path. It
completed 898 of 898 observed frames with monotonically advancing UST and MSC,
but every completion mode was `Copy`; Sophia's corresponding completion is
post-KMS `Flip`. Rejecting all non-Flip samples incorrectly classified healthy
FIFO client cadence as missing evidence. The reference reporter now admits
advancing Pixmap `Flip`, `Copy`, and `SuboptimalCopy` completions while
recording the path. A mismatch is explicitly `cadence_only`: it can gate
throughput and cadence, but cannot establish final scanout latency. An
unredirected Xserver Flip capture remains the stronger follow-up if that claim
is required.

The paired physical Vulkan gate passes. Both fixed workloads produced 898
observed completions with the same llvmpipe provider, 500-by-500 surface,
FIFO Vulkan mode, and 2560-by-1440 target output. Sophia measured 59.953 FPS
and 17.155 ms p95; composited Xorg measured 59.950 FPS and 16.686 ms p95. The
rate ratio is 1.0001 and the inverse-p95 ratio is 0.9727, above the required
0.90 in both dimensions. Sophia's maximum CPU composition was 6 ms, maximum
native upload was 3 ms, and native submission failures remained zero. This
closes the software-Present cadence gate while preserving the separate
unredirected-Flip latency follow-up.

## 2026-07-28: Software Present Optimization Keeps Ownership Boundaries Intact

The first correct standalone software-vkcube result retired 487 frames in
17,755 ms, about 27.4 FPS. Its retained maxima identified two independent
costs: CPU composition reached 17 ms and native upload reached 10 ms. The hot
path also cloned the source pixmap, copied a full immutable snapshot across the
authority boundary, allocated and checksummed a 2560-by-1440 output, cloned
that output for reporting, copied it again before GBM write, and recreated
mixed EGL/GBM render targets. Correct Present feedback had exposed throughput
rather than another policy or lifecycle fault.

The optimized path keeps the architectural boundaries unchanged. X authority
retains a bounded read-only SysV mapping, resolves XFixes regions, copies only
clipped update rows into its logical-window backing, and emits a fixed-capacity
immutable patch batch with a stable handle and monotonic generation. Renderer
intake prevalidates the complete batch before applying it, so malformed suffix
data cannot expose a partially updated frame. Engine sees only ordinary buffer
generations and damage; the WM remains blind to storage and protocol details.

Output composition now owns reference-counted bytes, reclaims the allocation
when no downstream lease remains, performs three exact startup pixel proofs,
and then derives bounded evidence from display-list generations and geometry.
The same-stride native CPU upload borrows those bytes instead of making another
full-frame vector. Mixed CPU/DMA-BUF composition retains its native target and
frame surface across same-size frames. Metrics expose replacement versus patch
traffic, payload bytes, evidence mode, and target reuse so a physical result
can prove that the intended path actually ran.

`tools/benchmark_sophia_vkcube_tty3.sh` supplies a fixed 900-frame workload.
`tools/report_sophia_rendering_performance.sh` computes FPS and p95 cadence from
Present UST values and can enforce a same-provider Xorg parity gate. Physical
results are pending. A retirement-fed three-slot CPU scanout pool is deliberately
conditional: per-frame GBM allocation should be replaced only if the new
measurements show it remains material, because recycling a scanout BO before
KMS retirement would violate the existing ownership proof.

The first bounded benchmark exposed a scheduling regression before it could
collect performance data. Sophia selected the 500-by-500 PresentedBuffer and
admitted its frontend surface, but submitted an unchanged empty CPU frame
between output-baseline readiness and visual-admission commit. That page flip
retired successfully; the visual transaction itself never committed, two
layout epochs timed out, and the eight-second `no_surface` startup guard
performed a clean shutdown. There was no panic, protocol error, native submit
failure, or emergency recovery.

The cause was an overloaded checksum contract. The first three compositions
used an exact output-pixel checksum, while later compositions used bounded
generation/damage evidence. Switching metric modes therefore changed the value
for identical pixels, and native scheduling interpreted the proof-algorithm
change as new content. Scheduling identity is now always derived from immutable
buffer generations, geometry, compositor primitives, and cursor state; the
metric mode changes only how nonzero output is counted. Regressions prove
identical display lists retain one identity across the warm-up boundary and
that an immutable generation change still advances it. A new physical
benchmark result remains pending.

The first retest removed that false page flip but still reached `no_surface`.
It exposed the underlying admission-order dependency more clearly. The
PresentedBuffer candidate was selected before the blind WM staged its pending
layout; the layout correctly retained that selected transaction, then
`AdmitSurface` completed. Control completion advanced Engine from
`ControlPending` to `AwaitingPixels`, but production resolved pending layout
only while processing a later authority batch. The software client was already
blocked waiting for Present feedback, so no later batch existed and the two
sides deadlocked until the startup guard fired. In the historical successful
run, the candidate happened to arrive after control completion and authority
processing called the reducer immediately.

The owner loop now runs one shared layout-progress service after every event
class that can unlock a pending epoch. Admission-control acknowledgement
advances only Engine admission state; authority observation, WM staging, and
control completion all invoke the same idempotent reconciliation. A ready
layout remains pending when the WM-update slot is occupied and commits after
that slot drains rather than failing or overwriting the older update. Thus
candidate-before-control and control-before-candidate converge without a
synthetic authority wakeup or hidden commit side effect.

That change advanced the next physical run through visual admission, layout,
focus, and eight CPU compositions, then exposed a second ownership defect as
`no_visual_detail`. Renderer CPU intake installed the quarantined snapshot and
immediately reclaimed every buffer absent from Engine's committed surface
snapshot. Absence was intentional before admission, so the later released
transaction referenced a buffer the renderer had already discarded. The
unchanged empty output correctly produced no new native submission.

CPU update application and residency reclamation are now separate operations.
The live layout emits a sorted, bounded handle snapshot for CPU buffers
referenced by pre-admission or release-pending transaction groups. Backend-live
joins those handles with committed surfaces and the current production batch;
renderer-live retains exactly that complete root set. Staged pixels may reside
in renderer-private storage but cannot become visible until their exact Engine
transaction commits. Removal, withdrawal, supersession, or timeout drops the
root and reclaims the buffer on the next cycle. This avoids another pixel copy
and keeps X resources, application identity, and layout policy out of Engine
and renderer state.

The console's contemporaneous elogind message was not causal. Session 214 was a
valid `_greeter` session on tty7 created after Sophia's clean tty recovery; the
Sophia run had already acquired input devices, modeset both outputs, and
executed for eight seconds. Its leader and `.ref` FIFO were live during
diagnosis, and the runtime record later disappeared normally. The warning
belongs to greetd/elogind handoff diagnostics, not rendering or admission.

## 2026-07-28: Visible Vulkan Diagnosis Starts With One Natural-Size Client

The first physical xmonad run after evidence-ranked admission produced no
visible change: default vkcube still opened a blank bordered surface. Further
changes made only inside the combined Kitty/xmonad path would not distinguish
the X11 Present/rendering fault from compatibility-bridge layout behavior.

Sophia now has a dedicated single-client production profile. It launches
`vkcube --wsi xcb` directly, omits Kitty, xmonad, xmobar, and the X11 WM
compatibility bridge, and uses the external reference WM's new generic
`natural` layout policy. That policy sees only an opaque layout node, preserves
its natural allocation, centers it within output bounds, and emits no policy
resize. It is usable for any single-purpose session; neither its reducer nor
Engine branches on vkcube identity.

The profile deliberately retains policy-managed deferred mapping, the X
authority's DRI3/Present stream, exact visual-candidate admission, renderer
composition, and KMS page-flip retirement. It therefore draws a useful fault
boundary: a blank standalone window localizes the defect below the xmonad
bridge, while a visible cube localizes the remaining defect to full-desktop
policy/configure integration. The strict verifier requires one
PresentedBuffer candidate, exact armed/presented/retired identity, nonzero
scanout pixels, normal logout, and zero live presentation resources. Physical
evidence is pending.

## 2026-07-28: Admission Recovery Requires Evidence, Not Latest Extent

The first physical run after staged-pixel recovery still produced the same
blank bordered window. The retained trace disproved another readiness-only
fix. The new policy-managed surface reached frontend admission and recovery
transaction 3 committed its geometry and focus ring, but no
`visual_admission status=armed` or native Present retirement belonged to that
surface. Every retired GPU frame still belonged to the existing terminal.

The causal defect was the scalar `safe_sizes` table plus reverse transaction
scan. Every accepted authority transaction overwrote the surface's safe extent.
A full-size software backing clear created after the blind WM's tiled configure
therefore replaced the earlier natural client-frame extent. Recovery published
that policy-sized blank extent as an exact constraint, selected its CPU
transaction, and treated admission as synchronous. The real Present group
remained quarantined, so the application could not advance its frame loop.
This single ordering error explained both the missing cube and failure to
produce a fixed-size floating placement.

River's separate scheduled/configured/rendering states are the closest policy
and timeout model for Sophia. Xserver supplies the X11 content rule: Present,
clears, and core drawing belong to one ordered logical window stream. Niri
supplies only the policy precedent that exact client constraints may open
floating. Sophia combines those lessons without adopting another project's
protocol or shell architecture.

Engine now retains a passive `SafeSurfaceObservation` containing source
transaction, extent, evidence class, and Engine observation sequence. During
admission, a complete Present/XPixmap buffer outranks an accumulated software
backing snapshot regardless of arrival order; equal evidence remains
newest-first. The admission stage must select the exact transaction named by
that record. A selected complete Present supersedes older covered groups,
newer groups remain fenced until retirement, and only the matching page flip
publishes managed state and focus. The X authority still owns X semantics, the
blind WM still owns floating placement, and no application identity crosses
either boundary.

Offline reducers reproduce the physical 500-by-500 Present followed by a
1276-by-1422 blank backing snapshot and prove recovery retains the Present
transaction and extent. The physical verifier now requires presented-buffer
candidate evidence before every armed admission. A fresh hardware run remains
required before this becomes retained session evidence.

## 2026-07-27: Recovery Cannot Substitute Extent History for Pixels

The first physical vkcube run after retirement-gated DMA-BUF admission still
showed a small tiled border with no cube. The retained log proved that this was
not an XMonad floating decision: vkcube surface 6291456 reached generic
frontend admission, the first two-surface resize timed out, and recovery
transaction 3 published a three-layer layout and focus ring. It never emitted
`sophia_live_visual_admission status=armed`, and no Present retirement belonged
to that surface. All observed GPU retirements remained Kitty surface 2097168.

The escape was the recovery readiness reducer. It allowed
`layout_epochs.committed_size == requested_size` to satisfy a pending
admission, even when the proposal owned no staged transaction for that
surface. A WM proposal could also carry a bufferless planning node without a
size change, leaving the surface outside `admission_surfaces`. The layout then
published geometry, chrome, and focus from extent history alone.

Every `PolicyPending`, `ControlPending`, or `AwaitingPixels` layer now becomes
an explicit admission target whether or not the WM changed its size. Retained
size state remains valid recovery guidance only; admission readiness requires
the proposal's exact staged concrete transaction. The regression recreates an
acknowledged recovery surface with matching retained extent but no pixels and
proves that the proposal stays held, committed layers remain empty, and focus
is not published. A client that never supplies matching pixels is withdrawn by
the existing bounded timeout path instead of receiving an empty compositor
frame.

## 2026-07-27: DMA-BUF Admission Completes at Exact Retirement

The follow-up physical trace removed the earlier mixed-transaction rejection
but exposed the remaining visual-lifecycle defect: vkcube surface 6291456 was
admitted, laid out, focused, and framed without any retired Present for that
surface. All 51 retired Presents belonged to the existing Kitty surface. The
admission commit had treated a released DMA-BUF transaction with no causally
paired Present as a synchronous visual commit, while the X mapped snapshot
could independently disable quarantine.

DMA-BUF admission now enters `AwaitingRetirement` with the exact selected
visual transaction. It becomes managed and eligible for deferred focus only
when that surface and transaction retire from KMS. Admission quarantine no
longer consults the mutable X mapped bit. Both quarantine and production intake
require one-to-one surface/buffer pairing between DMA-BUF transactions and
Present submissions. Resource release is held while a quarantined group
references its DMA-BUF or fences, and backend intake registers and begins
presentation ownership before applying release. A Present-bearing GPU cycle
also cannot queue a retained CPU frame ahead of its candidate.

Offline Engine, CLI, and backend regressions cover exact retirement matching,
mapped-bit independence, malformed-group rejection, deferred resource release,
and deferred focus. The retained session log predates its source commit and is
diagnostic rather than proof; the new physical verifier requires matching
`armed`, `presented`, and native `retired` records plus clean teardown.

## 2026-07-27: Admission Release Preserves Atomic Transaction Groups

The latest physical vkcube trace exposed a second identity defect after
per-Present scene ownership was corrected. The admission quarantine retained
vkcube transaction 858, then appended it to the next ordinary frontend batch
whose envelope transaction was 367. Engine correctly rejected the manufactured
two-surface batch with `expected_transaction=367 actual_transaction=858`.
Transport, Kitty, and KMS remained alive, but the admitted vkcube surface never
crossed into committed visual state.

The envelope and the atomic unit are now explicit separate data shapes.
`LiveProductionAuthorityBatch` owns envelope-scoped DMA-BUF and fence lifetime
facts plus ordered `LiveProductionAuthorityGroup` records. Each group owns one
transaction ID and validates every surface transaction and Present submission
against it before scheduler or Engine intake. The pre-admission path retains
the same complete groups in a fixed 256-entry FIFO, reprojects and rebases them
only at accepted admission, and releases them beside—never inside—the current
frontend group. Mixed identity and capacity exhaustion fail the session closed.

Offline regressions reproduce transactions 367 and 858, validate both groups,
and commit them independently through the production coordinator. A routed
deferred-map vkcube smoke additionally requires real map intent, generic
`AdmitSurface` delivery, DRI3 import, and two exact Present Complete/Idle
round trips. The restricted sandbox has no `/dev/dri`; an unrestricted local
attempt reached intent and admission, but this environment's vkcube selected
llvmpipe and emitted no DRI3/Present handoff. The command therefore remains a
hardware-Vulkan preflight rather than retained proof. Visible vkcube pixels and
native KMS retirement remain the short physical roadmap gate.

## 2026-07-27: Present Requests Are Not Persistent Scene State

The first physical run after truthful deferred admission reached
`frontend_admitted`, but every Kitty Present was rejected before rendering.
The live visual runtime had stored xmobar and Kitty as historical
`SurfaceTransaction` values, cloned the entire table for each Present, and
asked Engine to prepare it under Kitty's newest transaction ID. Engine
correctly rejected mixed batches such as expected transaction 403 with actual
transaction 198. Kitty never entered committed state or focus, and the startup
watchdog exited at `stage=not_focused`; KMS and protocol transport remained
healthy.

Xserver's Present implementation keeps each queued request's window, pixmap,
serial, fences, and timing separate from persistent window state. Niri likewise
uses client transactions as readiness blockers and builds output render
elements from current compositor state. Sophia retains its stronger
`PreparedSurfaceCommit` contract but applies the same ownership lesson: a
queued Present owns exactly one matching surface transaction, while unrelated
surfaces come from Engine's committed baseline.

The production coordinator now prepares one Present candidate and rebases only
its causal Engine generation. Backend input and compositor projections derive
from committed state rather than pending transactions, and page-flip retirement
promotes the candidate before successful feedback. The Present hot path no
longer clones and validates every historical scene transaction. Mixed
xmobar/Kitty identity, malformed-candidate rejection, generation preservation,
and exact retirement are covered by offline regressions; physical startup
reproof remains open.

## 2026-07-26: X Hierarchy Defines the WM Admission Boundary

The first physical pre-pixel-admission candidate still exited with
`stage=not_focused`: native scanout and page flips remained healthy, but the WM
never received a manage request. A retained real-xterm authority trace exposed
the exact ordering. xterm issues `MapSubwindows` for descendants of its
toplevel, then issues `MapWindow` for the toplevel itself. Sophia had discarded
the requested parent at `CreateWindow` and implemented `MapSubwindows` as
"map every window in this namespace." That prematurely moved the toplevel out
of its deferred state, so the later real map request could not emit the
presentation intent required by Engine and the blind WM.

The authority window table now owns parent links as passive X protocol state.
`QueryTree` projects that state, reparenting validates cycles, and
`MapSubwindows` affects only direct children. A non-override-redirect root
child is a policy-managed toplevel; descendants and override-redirect windows
are client-positioned. Deferred map policy therefore applies only at the X
root boundary. The retained wire regression reproduces xterm's real opcode
sequence and proves that mapping a child does not admit its parent, while the
following toplevel `MapWindow` emits exactly one managed presentation request.

Core software drawing follows the same tree reduction. Descendant buffers stay
X-authority resources, while their translated damage is accumulated into one
immutable toplevel presentation buffer and one Sophia surface generation.
The concrete presentation extent grows only with observed descendant coverage
and is capped by the toplevel geometry; a configure alone therefore cannot
manufacture a full-size buffer that would satisfy admission. This gives xterm's
shell/content window split one visual identity without leaking X children into
the WM.

The audit also closed a visual-boundary gap. Drawing to an unmapped managed
window is valid X11 traffic, but it is not permission to enter Sophia's
committed scene. The live layout now keeps at most one latest pre-admission
transaction per surface in a bounded data table. It records the safe extent
for planning and recovery, excludes the transaction from renderer intake, and
releases it exactly once—rebased to the first Engine visual generation and the
accepted WM geometry—after frontend admission is acknowledged and matching
pixels are ready. Withdrawal, removal, and terminal timeout erase the retained
record. Early Present submissions follow the same boundary in a fixed
256-record queue; overflow fails the session closed instead of leaking a GPU
submission or growing memory without bound. No client identity or X resource
enters Engine or WM policy.

## 2026-07-26: Managed X11 Mapping Requires Pre-Pixel Admission

The repeated blank `vkcube` frame was an ordering defect, not evidence that the
application required a hard-coded floating or fixed-size rule. The live X
frontend fulfilled `MapWindow` immediately and emitted `MapNotify`/`Expose`
before the blind WM had an opaque node to manage. Sophia could create that node
only after observing a pixel-backed transaction, while the application was
already reconfiguring its swapchain from the initial 500-by-500 window to the
tiled allocation. The latest trace registered buffers and fences but retired no
cube Present at the accepted layout.

X server map-redirection semantics supplied the useful reference model: keep a
policy-managed window unmapped while policy decides its geometry, then configure
and map it as one admitted transition. Sophia implements that invariant through
its own protocol-neutral boundaries. The frontend emits
`SurfacePresentationIntent`; Engine retains passive admission facts; the WM
plans an opaque bufferless node; and `AdmitSurface` configures and maps only
after proposal validation. Matching authority pixels, not the control
acknowledgement, establish committed visual truth. No X server code or
application-specific fact enters Sophia.

The same audit found an overly broad Present barrier. Any pending layout had
blocked every queued Present, including unrelated stable surfaces. Each
submission now carries an immutable disposition: immediate, staged for one
layout epoch, or rejected for a known size mismatch. The bounded scheduler can
continue unrelated work, shares one immutable CPU-layer batch, and releases
staged submissions when the epoch resolves. Wrong-size pending pixels update
only the safe-observation record and cannot leak into the visible layer table.

Reducer, wire, admission, WM pre-pixel planning, and mixed Present scheduling
tests cover the new ordering. Default `vkcube --wsi xcb` remains the physical
AMDGPU proof; until that retained run succeeds, the roadmap item stays open.

## 2026-07-26: Resize Timeout Recovery Is An Engine Replan

Launching a non-cooperative Vulkan top-level exposed an architectural failure,
not an application quirk. This recovery model remains valid, but the later
pre-map admission audit above supersedes it as the root-cause diagnosis for the
blank initial `vkcube` window. Xmonad proposed a tiled epoch, existing clients
and the new surface did not all publish the exact requested extents before the
deadline, and the CLI compensated back to old sizes. Preselecting startup
dimensions would still have encoded application policy.

Resize/admission recovery now belongs to a protocol-neutral Engine coordinator.
It retains safe authority content extents, fences pixels from abandoned sizes,
and stores declared constraints separately from temporary exact recovery
constraints. A timed-out first admission is retried once with
`min_size == max_size == safe_extent` and `resizable = false`; the blind WM
still chooses placement. The unsafe slow-client option that synthesized visual
truth from timed-out pending pixels was replaced by a replan-at-committed-extent
decision.

The legacy compatibility bridge generically exposes effective constraints as
synthetic ICCCM `WM_NORMAL_HINTS`. When manage-time constraints change it
remanages the private synthetic window so stock xmonad reevaluates fixed-size
policy. No Vulkan, Kitty, xmonad-client, XID, namespace, title, class, or PID
fact enters Engine policy. Default `vkcube --wsi xcb` is the physical
compatibility proof, not an implementation branch.

The real unmodified-xmonad bridge smoke now follows its sequential three-window
tiling proof with a manage-time transition of one opaque node to an exact
500-by-500 constraint. Xmonad returned that floating placement after the bridge
remanaged the private synthetic window, proving the ICCCM path without client
metadata. Physical authority/presentation admission remains the open roadmap
gate.

The first physical run then found one remaining unit-boundary error. Recovery
retained a 500-by-500 client buffer but sent 500-by-500 as the WM's outer
constraint. The active two-pixel clearance correctly inset that allocation to
a 496-by-496 client configure, which the application did not satisfy. Engine
now owns the inverse conversion as well: committed geometry and content
constraints become 504-by-504 outer facts before the WM boundary, and the
existing inset returns exactly 500-by-500 to the authority. Focus ring/frame
width is therefore handled generically rather than encoded in recovery policy.

## 2026-07-27: Delivery Acknowledgement Retires Both Input Ledgers

The second emergency capture proved that both synthetic modifier releases
reached the frontend: all 547 expected input deliveries flushed, the pressed
key ledger reached zero, repeat was inactive, and Engine/native teardown was
clean. Completion still failed because the two release IDs remained in a
separate control-ordering barrier. Normal loops eventually pruned that set
during another control-service pass; emergency completion exits immediately
after the input-delivery reducer settles and exposed the split ownership.

Input-delivery acknowledgement now atomically retires an ID from both the
general pending set and the client-key release barrier. A focused reducer test
locks that invariant. The pre-emergency physical gates on the parent candidate
remain valid for this isolated bookkeeping correction. After the new commit
runs its unattended semantic gate, a closed-path adoption command reverifies
the parent native, hardware-smoke, and xmobar evidence and records provenance.
It cannot adopt emergency evidence or accept broader runtime changes.

## 2026-07-26: Emergency Shutdown Owns Routed-Key Drain

The first commit-pinned emergency gate triggered both the live owner and the
independent guard, drained native presentation, restored the TTY, and returned
control, but the inner session failed its final key-ledger invariant. Ctrl and
Alt had already been routed to the focused client before Backspace completed
the emergency chord. The loop waited for existing authority deliveries but
did not synthesize releases for those two routed presses, leaving the client
ledger nonempty at completion.

Emergency shutdown now snapshots the complete bounded pressed-key ledger,
cancels active repeat, routes releases through the normal X authority path,
and adds those delivery IDs to the existing acknowledgement barrier. This is
session-wide input ownership, not Kitty, xmonad, or chord-specific client
policy. Surface-scoped focus and logout flushes share the same release reducer.

The run also exposed an outer-control-plane mismatch. The independent guard
intentionally exits its launcher with status 130 after recovery, while the
promotion driver previously rejected every nonzero launcher status before
running the emergency verifier. Gate policy now admits 130 only for emergency
evidence. The verifier remains authoritative: it requires the inner session
to exit zero with drained key, control, native, and Present state plus exact
TTY restoration.

## 2026-07-26: Physical Evidence Exposed Verifier Assumptions

The first short hardware-smoke promotion run was healthy but its verifiers
rejected four assumptions that were stricter than the runtime contract. A
physical click can queue only its press before focus applies; its release may
then route normally, so one or more queued handoff events is valid. Managed
Present geometry is the WM allocation inset once by negotiated Engine chrome,
not the outer allocation. A damage-driven compositor may idle stable windows,
so mixed-export ledgers require positive balanced work rather than an
arbitrary minimum frame count. Finally, a physically connected output with no
moving client retains its synchronous baseline buffer without producing an
asynchronous page-flip retirement.

The corrected proof checks data-oriented end-state invariants: ordered focus
application and queued input delivery, negotiated clearance and exact inset
geometry, positive mixed exports with balanced target/pipeline/frame-surface
accounting, and per-output completion with one retained displayed buffer.
The retained run satisfies those invariants on both outputs, completed one
drained VT round-trip and normal logout, and reported clean protocol, control,
native, namespace, and TTY teardown.

Promotion can explicitly adopt this one parent run after a verifier-only
correction. The adoption path accepts only a verifier-rejected result, uses a
closed changed-path allowlist, and reruns both hardware verifiers before
writing source-commit provenance. Runtime changes or launcher failures still
require fresh physical evidence.

## 2026-07-26: Promotion Separates Semantic Automation From Hardware Proof

The accumulated Milestone 9 operator sequence had become both tedious and
weakly reproducible. It asked one person to remember clipboard, workspace,
focus, repeat, launch, close, VT, bar, and teardown gestures across many
physical sessions even though most of those state machines already have
deterministic two-output QEMU or reducer coverage. A missed gesture could fail
the ledger without identifying a product defect.

Promotion now begins with one commit-pinned unattended semantic gate. It runs
the canonical offline local regression suite and the retained M7 xmonad and M8
mixed-application QEMU scenarios, then verifies their exact evidence. Policy, protocol,
application, focus, workspace, clipboard, damage, and teardown semantics are
therefore machine-driven and replayable. The promotion driver can run this
first gate outside TTY3.

Physical evidence is reduced to facts virtualization cannot establish:
native chrome/hot reload, real KMS pixels and lifetime, actual keyboard and
pointer routing, one libseat VT round-trip, client-positioned bar geometry and
pointer behavior, normal TTY recovery, and independent emergency recovery.
The four-Kitty and bar proofs each have a short dedicated verifier. Exhaustive
keyboard/VT, pointer-edge, launch-burst, clipboard, and long xmobar workflows
remain focused diagnostics after their owning subsystem changes; they are no
longer memorization-heavy per-candidate rituals.

This is an evidence-boundary change, not a runtime exception. QEMU never
substitutes for AMDGPU, libinput, monitor, greetd, or emergency-recovery proof.
The Engine, X frontend, WM bridge, and renderer remain unaware of promotion
policy.

## 2026-07-26: Real M7/M8 QEMU Runs Found Cross-Layer Regressions

Running the retained scenarios as real guests, rather than relying only on
fixture verifiers, exposed five independent defects. The M7 click probe landed
on newly reserved compositor clearance at the output edge; the harness now
resets to the edge and moves 32 pixels into client content. The M8 Vulkan
fixture assumed its fixed startup size would survive xmonad relayout; it now
uses a deterministic software-rendered size on a separate workspace so the
test measures mixed presentation rather than toolkit resize timing.

Firefox then reached an unimplemented XSync counter request after Sophia
advertised XSync 3.1. The frontend now implements bounded generic counter
create, set, change, query, destroy, list, resource ownership, and namespace
cleanup. No browser policy entered Engine. Firefox also creates more than one
top-level surface and its software-rendered content does not retire through the
Present ledger. Launch admission therefore tracks a bounded fixed set of
observed surfaces and settles on any committed stable visual surface, whether
stability is proven by CPU visual detail or retired Present.

The final GTK launcher close exposed a teardown ordering defect. Shortcut
prefix modifiers had entered the per-client pressed-key ledger, but the
launcher did not select keyboard events. Sophia waited for synthetic release
acknowledgements before sending close, so the close timed out without being
dispatched. Closing surfaces now clear those keys through the state-only
authority path and dispatch close immediately. Focus handoff and VT suspension
still deliver ordered releases because those clients remain alive. The real
M8 run completed all eight browser stages, delivered all 48 controls with zero
timeouts, reaped the launcher, drained input and WM state, and revoked the
namespace and X authority cleanly.

The guest also established a precise compatibility boundary. Physical wheel
axis input is observed, hit-tested, and routed to Firefox, but the current
reduced core-button translation does not produce Firefox DOM `wheel` events.
The fixture advances its scroll stage with a focused Space key only after a new
axis route is observed. This proves generic Engine axis routing and continued
browser interaction; it does not claim native Firefox wheel compatibility.
That work remains in the X frontend compatibility milestone.

## 2026-07-26: Client-Positioned Clicks Must Bypass WM Focus

The first external-config promotion run completed every ordered reload phase,
then terminated when the operator clicked the status bar. Hit testing correctly
selected the `ClientPositioned` surface, but the generic primary-button path
started a managed focus handoff because the bar differed from keyboard focus.
The blind WM correctly had no workspace registration for that excluded
surface; treating its rejection as fatal ended the session.

Physical input now consults the existing protocol-neutral presentation-role
table before starting a focus handoff. A `ClientPositioned` target receives
button input directly while the current keyboard focus is retained.
`PolicyManaged` targets keep the ordered WM/Engine/frontend focus handoff.
This is role policy rather than a status-bar or xmobar exception.

## 2026-07-26: Physical Promotion Is A Commit-Pinned Ledger

Milestone 9 previously depended on several correct but independently retained
physical captures. That could not prove that normal, stress, chrome, status-bar,
VT, and emergency behavior all belonged to one candidate build. The promotion
driver now keys an append-only evidence directory by the full Git commit,
refuses a dirty tree, selects one ordered gate per invocation, archives the
source logs before verification, and advances only when the gate-specific
fail-closed verifier succeeds. Failed evidence is retained but never counted.

The same evidence boundary stays data-oriented and privacy preserving. Engine
reduces a compositor display list to aggregate frame/ring/primitive counts and
clearance rather than exposing surface identities. Physical keyboard coverage
reduces to 21 shifted printable positions and 12 VT targets without logging
typed characters. Protocol-specific authorities remain unaware of compositor
chrome, and the xmonad compatibility bridge explicitly reports that it did not
negotiate native chrome policy.

## 2026-07-26: Chrome Uses Stable Allocation Clearance

The first focused-border prototype painted four inside-edge solids over the
committed client rectangle. Increasing its width therefore hid client pixels.
The durable model now distinguishes a focused ring from an optional
focused/unfocused frame. Engine treats WM geometry as an outer allocation and
derives client content by one checked inset equal to the maximum enabled chrome
width. Focus changes only repaint; width changes prepare matching client
buffers before the new visual style becomes active.

The renderer-neutral display list carries one semantic border node per
surface/role. Damage and backend lowering share one fixed four-band expansion,
eliminating per-edge policy records and temporary edge vectors. Client-
positioned surfaces are excluded before display-list construction. KDL schema
2 and WM API v6 name focus-ring and frame policy explicitly; schema 1 is
rejected with migration guidance. This follows Niri's useful separation of
focus indicators and frames without importing its Wayland authority model or
making Sophia's Engine dependent on a particular WM, frontend, or renderer.

Chrome ownership is capability-gated rather than inferred from the presence of
a WM process. A native WM may negotiate the policy; the X11 compatibility
bridge deliberately does not, so xmonad and other external WMs use the
Engine/compositor fallback. Both sources reduce into the same candidate versus
visually committed state, keeping width changes behind one relayout boundary.

## 2026-07-26: Click And Drag Now Have Independent Focus Proofs

The first unattended pointer-focus proof used only a click-drag gesture. That
exercised the generic ordered handoff but left plain click-to-focus as an
inference. The M7 harness now creates two independent focus transitions against
an unfocused visible surface. The first sends primary press/release and a
following key. It then moves focus away through the WM and sends primary
press/motion/release plus a different following key.

Both real QEMU sequences passed through virtio input, Engine hit testing, the
blind-WM focus request, X-frontend acknowledgment, ordered deferred-input
release, focused-border composition, combined output-damage retirement, and
keyboard delivery to the selected opaque surface. The click released two
records and the drag released three. Completion retained two output baselines,
clean bridge recovery and logout, zero stale WM responses, zero protocol
errors, and no native cleanup debt.

The verifier treats each gesture as its own bounded state machine. It rejects a
missing or overlapping gesture, an incomplete handoff, insufficient click or
drag records, a missing key-probe boundary, a key routed to another surface,
missing target border/damage/repaint evidence, or a missing completion marker.
Physical libinput and visual confirmation remain required because QEMU cannot
prove the actual mouse, display, or TTY path.

The physical operator gate now has the same two-sequence shape through
`tools/start_sophia_xmonad_pointer_focus_tty3.sh`. The wrapper guides a plain
click and key, moves focus away, guides a click-drag and different key, then
automatically checks both ordered handoffs after normal logout. Its verifier
requires two independent requests and at least press/release for the click
plus press/motion/release for the drag. It does not infer visual success: the
operator still confirms pointer selection, border movement, and text delivery
on the physical outputs.

## 2026-07-26: Native Lifetime Regressions Form One Named Gate

The remaining Milestone 9.1 regression item did not require a new lifecycle
owner. Its constituent failures already reduce through focused deterministic
tests at the boundaries that own them:

- CLI resize coordination tests compensating rollback, abandoned-pixel
  fencing, disconnect cleanup, and stale prepared-frame settlement.
- Backend startup tests output/target size mismatch, replacement, stale target
  allocation removal, and reduced target retirement.
- Backend presentation and scanout tests accepted replacement, stale callback
  rejection, cleanup retry, final displayed-owner cleanup, and repeated
  retirement as a no-op.
- Renderer lifetime tests stale CPU retirement and reusable DMA-BUF retirement
  without exposing renderer-native handles.

Validation now gives these tests one named command set. This preserves DRY
ownership: rollback remains transaction policy, target replacement remains
backend readiness, and native resource destruction remains KMS/backend state.
The gate closes the deterministic Milestone 9.1 item, but it does not replace
the same-commit physical xmonad run, whose balanced complete-target,
frame-surface, page-flip, and teardown counts remain authoritative.

## 2026-07-26: One Snapshot Now Accounts For All Primary-Frame Pixels

Compositor damage could not safely become output scheduling authority while
client and software-cursor changes remained separate. Sophia now builds one
bounded immutable output-damage snapshot for every CPU and mixed CPU/DMA-BUF
frame. It contains only pixel-relevant Engine facts: output shape and scale,
ordered opaque surface IDs, committed generations, geometry, buffer identity,
the compositor display list, and optional software-cursor bounds. It contains
no XIDs, application metadata, WM facts, protocol objects, or renderer-native
resources.

The reducer damages old and new client extents for generation, geometry, or
buffer changes; damages all involved client extents for stacking, creation, or
removal; includes old/new compositor nodes; and includes old/new software
cursor bounds. Initial presentation and output shape/scale changes force full
output. Hardware cursors keep their independent plane lifecycle. Snapshots are
bounded to 1,024 client nodes and revalidated before reduction so public record
mutation cannot bypass ID, ordering, capacity, or output invariants.

CPU and mixed frames retain this snapshot through cloning, latest-frame-wins
queueing, native export, accepted KMS submission, and page-flip retirement.
QEMU established full initial plans on both outputs and emitted separate
compositor, combined-output, and repaint records for every retired primary
frame. During pointer focus, the matching combined plan correctly became full
when the same transaction also advanced client generations; requiring a
chrome-only partial plan would have hidden client damage. The verifier now
requires the compositor record, combined record, and safe partial-or-full
decision from the same retired frame before the following key.

The planner still does not authorize partial drawing. Native destination
buffers are not yet proven preserved or reconstructed for the reported region.
The next optimization step is explicit destination-buffer age/history and a
full fallback whenever that proof is absent.

## 2026-07-26: Repaint Planning Fails Safe Before Native Optimization

The retirement-safe display-list region is not, by itself, authority to skip a
frame: client-buffer, software-cursor, output, and future animation damage must
also participate. Sophia therefore adds the next optimization boundary without
prematurely changing pixels. Engine now reduces compositor damage to an
output-local `skip`, `partial`, or `full` repaint plan. It clips every rectangle
to output bounds, coalesces only unions that remain exact rectangles, and
computes a bounded pixel count. More than 2,048 raw rectangles, more than 32
partial rectangles, or at least 60 percent output coverage fails safe to a
full-output plan. Invalid output dimensions and policies are rejected.

Deterministic tests cover clipping, exact versus L-shaped coalescing, pixel
accounting, coverage fallback, fragmentation fallback, raw-capacity fallback,
invalid inputs, and attachment to the in-flight display-list lifecycle. The
two-output QEMU xmonad run then observed empty `skip` baselines, four-rectangle
partial border creation, partial old/new focus damage, partial border removal,
and zero compositor repaint work for stable client-only frames. The M7 verifier
now rejects a focus proof without a nonempty retired partial plan.

This is planning evidence, not a claim that rendering is partial. The next
implementation stage must combine client, compositor, cursor, and output damage
into one frame plan, preserve or reconstruct the destination buffer correctly,
and use full rendering whenever buffer age or native capability is uncertain.

## 2026-07-26: Compositor Damage Follows KMS Retirement

Display-list damage previously described the difference between two lists but
had no temporal identity in the production scanout path. Advancing a single
“last list” at composition time would be incorrect: latest-frame-wins queueing
can supersede pixels, native export can fail, and a submitted buffer remains
in flight until its page-flip callback.

Engine now owns a bounded per-output display-list presentation reducer with
pending, submitted, and presented slots. CPU and mixed CPU/DMA-BUF frames carry
the immutable list that generated their pixels. A queued list compares against
the submitted list when a flip is in flight and otherwise against the
presented list. Superseding or rejecting pending work cannot change the
presented baseline. Only an accepted KMS submit advances pending to submitted;
only the corresponding accepted callback advances submitted to presented.
Legacy frames without display-list identity explicitly clear pending chrome
state rather than inheriting an unrelated list.

The two-output QEMU xmonad gate passed this lifecycle. Both outputs established
empty initial baselines. Focus creation retired four border rectangles, focus
changes retired eight old/new rectangles, border removal retired four, and
stable client-only frames retired zero compositor rectangles. The gate now
rejects missing secondary-output initialization or missing nonzero retired
damage during the click-drag focus proof. Partial redraw, frame suppression,
and KMS damage-clip submission remain later scheduling optimizations; the new
ledger supplies the retirement-safe region those optimizations must consume.

## 2026-07-26: Focus Chrome Uses The Ordinary Engine Display List

Sophia now has the first production compositor-owned visual: a minimal focused
surface border. Engine builds one bounded immutable display list from opaque
surface order, committed focus, and committed surface state. Four stable
inside-edge nodes follow the focused surface in that list. Node identity,
generation, geometry, and color drive deterministic old/new damage; unchanged
nodes produce no compositor damage. The external WM and X frontend receive no
chrome records, graphics parameters, renderer handles, or application facts.

The CPU reference path consumes the ordered surface and solid commands
directly, including clipping and exact XRGB byte output. The native mixed path
uses the same list to interleave CPU buffers, DMA-BUF buffers, and compositor
solids. EGL lowers each solid to a scissored opaque clear, avoiding texture
allocation in the frame hot path. A prepared GPU Present uses the candidate
surface state associated with those pixels; retained and CPU composition use
committed state, so a border cannot move ahead of matching client geometry.

The two-output QEMU xmonad gate passed with the new path. It observed four
border primitives on both focus targets. During the click-drag proof, WM focus,
Engine focus, and X frontend focus acknowledgment completed first; the retained
pointer records were then released, the new border frame was composed for that
same opaque target, and the following key reached it. The verifier rejects a
missing border, the wrong target, fewer than four primitives, or border
evidence that arrives after the following key. Physical DRM confirmation across
focus, resize, workspace, VT, and mixed CPU/DMA-BUF presentation remains open.

Border generation hashes only facts that change compositor pixels: geometry,
thickness, and color. Client buffer commits therefore do not create false
compositor damage or repeated evidence. Hiding focus clears retained
observation state so restoring the workspace proves a new border composition;
VT and native-recovery repaints explicitly re-emit the reduced border fact even
when geometry is unchanged. A dedicated physical verifier now requires those
workspace and VT sequences, two focus targets, a focused geometry-generation
change, nonzero mixed exports, and clean shutdown. Its pass and mutation
fixtures fail closed without claiming physical completion.

## 2026-07-26: Click Focus Requires A Cross-Authority Input Barrier

Physical pointer events were already hit-tested against Engine scene truth and
routed to opaque surfaces, but a primary press never asked spatial policy to
change focus. Consequently clicking or click-dragging another xmonad tile
could move the cursor and deliver pointer input without changing Engine, WM,
or X11 focus.

WM API v4 adds `FocusRequested`, containing only surface, output, and
workspace. The reference WM returns `FocusSurface`; the metadata-blind legacy
bridge translates the target into a private synthetic primary-button gesture
so xmonad updates its own focus stack before returning the same opaque Sophia
surface. No XID, namespace, application metadata, or raw input payload crosses
the WM boundary.

Engine now owns a bounded pointer-focus handoff. The initial press and following
motion/release records remain ordered against the selected surface, including
drag coordinates outside its geometry, until the X frontend acknowledges that
same focus. A 256-record capacity and two-second timeout fail closed. Protocol,
reference-policy, bridge, route, ordering, and timeout regressions pass. The
real unmodified-xmonad smoke also focused a requested opaque surface. Xmonad's
focus refresh re-emitted unchanged configure requests, so the compatibility
runtime discards those only for a focus-only request; the resulting Sophia
transaction contains `FocusSurface` and no placement. Physical TTY confirmation
remains the promotion evidence.

The local QEMU xmonad gate then exposed a harness-only seat regression before
WM negotiation: its minimal initramfs provides neither logind nor a seatd
daemon, so automatic libseat discovery returned `Function not implemented`.
The guest now explicitly selects libseat's direct `noop` backend. Production
sessions keep ordinary backend discovery and remain libseat-owned.

After cursor recovery, the M7 harness exposed a stale restart assertion. Its
input sequence intentionally left workspace 2 active and empty, but the host
required a new focused-surface record after restarting the compatibility
bridge. Preserving that workspace correctly produces `hidden_focus_cleared`.
The harness now accepts exactly the two valid reduced recovery states: a new
focus reconciliation for a focused projection or a new clear-focus record for
an empty projection.

The next M7 pass reached post-restart launch/close and exposed a separate
supervision classification error. Proof mode treated every secondary child
exit as fatal, including an approved action-launched terminal carrying a
launch transaction. Fixed proof witnesses still must remain alive, but
transaction-correlated action children may now exit normally in both proof and
normal sessions.

Repeated two-window startup exposed a stateful WM queue ordering defect.
Sophia treated geometry changes and earlier committed workspace state as
reasons to discard a later xmonad response. Xmonad had already processed that
request, so the rejection desynchronized the two state machines and a
following action failed with `UnknownSurface`. Response lifetime checks now
track only the opaque surfaces in the request. Each ordered response is reduced
against the latest committed workspace state, with a `ManageSurface` target
added to that current planning projection. Engine transaction validation
continues to own geometry correctness.

The same QEMU gate found that startup readiness was reduced only when an
optional timeout was configured, although bounded completion always required
the readiness result. Readiness reduction is now unconditional; the option
only supplies a failure deadline. Its post-detail frame barrier requires a new
submission only on outputs intersecting the focused surface while retaining
the initial presentation baseline for every owned output. The final M7 run
completed two-head startup in 187 milliseconds, processed 14 WM requests with
13 commits and zero stale responses, recovered one compatibility-bridge
restart, launched and cleanly closed the action terminal, logged out with zero
protocol errors or pending work, and drained both outputs.

The M7 verifier previously required at least two CPU layers in the final frame.
That contradicted the scripted empty workspace at logout. Its three-surface
peak remains independently required by committed-layout evidence; bounded
completion now proves clean external-WM lifecycle without requiring closed or
hidden windows to remain in the shutdown frame.

The unattended M7 workflow now exercises the pointer-focus barrier itself
through the existing virtio mouse. After two xterm surfaces settle, a keyboard
focus action selects the non-master tile, relative motion clamps the pointer to
the unfocused master tile, and QMP emits primary press, drag motion, and
release. The run recorded `FocusRequested` for the other opaque surface,
committed and acknowledged that focus, released all three retained pointer
records only afterward, and routed a following ordinary key to the same
surface. The final ledger contained two routed buttons, four routed pointer
events, zero stale WM responses, zero protocol errors, and no pending input.

The shared physical verifier now requires that following keyboard record as
well as request, Engine commit, X frontend acknowledgment, and retained
press/motion/release order. QEMU proves the complete software and virtual-input
path reproducibly; the real libinput device, physical DRM, and operator
interaction remain the final promotion evidence.

## 2026-07-26: Pending Pixels Must Not Replace WM Geometry

The latest normal xmonad capture exposed a temporal geometry defect that the
four-Kitty end-state verifier did not cover. After ManageSurface transaction 3
committed the two-window layout, the new surface continued to present at its
admission staging position, `1280x1426_80_60`. The existing surface presented
at its correct right-hand tile. A later Super-J focus transaction moved the new
surface to `1280x1426_0_14` without a configure, after which the geometry
remained stable. The three-window sequence reproduced the same pattern.

A strengthened real-xmonad smoke now manages three opaque layout nodes
sequentially with the physical `2560x1426_0_14` work area. It requires the
exact full-height master and two `1280x713` stack panes. The smoke passes,
proving that xmonad and the compatibility bridge return every committed
placement.

The first correction made Present admission consume the same immutable Engine
presentation layout used for composition. This prevents a newly queued buffer
from missing reprojection merely because it did not exist at the beginning of
the cycle. A following physical run proved that correction for the second
window, then reproduced the defect with the third window on workspace 2.

That topology exposed the underlying resize-quarantine race. The third window
already had the requested full-height pixel size, while the two existing
windows needed resize redraws. An ordinary third-window Present arriving while
those redraws were pending was therefore an unrequested layout observation.
The pending-layout reducer replaced the whole proposed layer with that
observation, including its `(80,60)` staging geometry and old stack rank. It
preserved new pixels by silently discarding the WM placement.

Unrequested policy-managed observations now merge only authority-owned visual
content into an existing pending layer: source, damage, generation, identity,
opacity, crop, transform, and resize capability. WM-owned geometry and stack
rank remain from the proposal. Client-positioned surfaces explicitly retain
authority-owned geometry, so an xmobar update during a resize epoch is not
lost. Deterministic regressions cover both authority assignments.

The following 77-second physical run confirmed the authority split across
seven action-launched Kitty surfaces on workspaces 1 and 2. Each ManageSurface
commit moved exactly the projected surface count, and the new surface's first
retired Present used the work-area master geometry. Three-window layouts
presented one `1280x1426` master and two exact `1280x713` stack panes;
four-window layouts presented the master plus `1280x475`, `1280x475`, and
`1280x476` stack panes. No target used `(80,60)`, and every following layout
transaction reported `moved_surfaces=0`. The run balanced 524 mixed target,
pipeline, and frame-surface lifetimes, held input dwell to 11 ms and
submit-to-page-flip observation to 47 ms, recorded no unexpected protocol
error or WM degradation, and completed cleanly.

The work-area-aware four-Kitty verifier now checks this temporal boundary for
every action-launched surface it observes. It correlates launch observation,
ManageSurface commit, active-workspace projection, first retired Present, and
the following stability transaction. Mutation fixtures reject staging
geometry, mismatched pixel dimensions, and a second geometry change.

Focused borders remain a separate compositor-chrome concern. They must derive
from committed focus and committed surface geometry, enter the ordinary Engine
frame and damage lifecycle, and never compensate for or conceal a placement
error. The renderer-neutral chrome model in `docs/compositor-graphics.md`
already defines that boundary; implementation follows physical geometry
stability.

## 2026-07-26: Clipboard Routing Is Workspace-Blind And Namespace-Explicit

The next physical workflow reported that Ctrl-Shift-C did not work on
workspace 3. The retained session did not show a swallowed chord or lost
focus: workspace 3 kept a focused surface, six selection-owner changes and 21
conversions reached the X authority, and the session exited cleanly. Kitty did,
however, report four failed selection conversions around workspace
transitions. The normal-session verifier previously rejected ownership failure
but could still accept this conversion failure.

The socket audit found a protocol-routing defect independent of Kitty and
xmonad. A client writing a property on another client's selection requestor
received the resulting `PropertyNotify` itself. X11 requires delivery to each
client that selected `PropertyChangeMask` on that window. The routed frontend
now retains bounded per-client core event subscriptions, routes property
changes to those subscribers, removes subscriptions with their client or
window, and preserves synchronous reply/event order when the requester is also
the subscriber. This state remains inside the X frontend; Engine, workspaces,
the WM, and the compositor never receive selection objects or payloads.

The same audit tightened namespace resolution. A request first resolves an
owner in its own admitted namespace and uses ordinary X11 transfer semantics.
Only the absence of a local owner permits a cross-namespace portal request.
Owner generations are globally monotonic, and portal source capture plus
target execution revalidate the exact source namespace and generation instead
of whichever namespace changed most recently. Explicit owner clear and
disconnect cleanup also retain that namespace instead of clearing an
arbitrarily ordered owner. Policy still sees only bounded facts; the runtime
executor alone handles correlated clipboard bytes.

The same-namespace socket regression now follows Kitty's material request
shape: distinct target and property atoms, requester-side property
subscription, `AnyPropertyType`, deletion, and a maximum `long_length`. The
cross-namespace regression uses the same distinct target/property and
requester subscription while proving broker-mediated payload capture and
handoff. The strict physical verifier now rejects Kitty's conversion-failure
diagnostic as well as ownership failure and requires at least two ownership
changes plus two conversions. The operator sequence now copies workspace 1 to
workspace 3 and back before continuing the normal promotion workflow. A new
physical pass remains required before promotion.

## 2026-07-26: GetProperty Length Is A Ceiling, Not A Payload

A physical same-namespace Kitty copy/paste attempt terminated the receiving
Kitty with three opcode-20 X errors. The session used the `classic_shared`
namespace profile, emitted no `CloseFocused` action, and never entered the
portal path. Kitty 0.48.0's X11 selection receiver calls `XGetWindowProperty`
with `long_length=LONG_MAX` and `delete=True` to read the complete property.
Sophia incorrectly multiplied that request ceiling by four, compared it with
the 256 KiB retained-property limit, and returned `BadValue`. Xlib's default
error handling then terminated the client.

Core X11 semantics instead define the returned length as the minimum of the
stored remainder and four times the requested length. The property reducer now
saturates the request conversion and clamps it to already bounded retained
bytes; it never allocates from the request ceiling. The 256 KiB per-value and
4 MiB table limits remain unchanged. An offset beyond the actual property
still fails with `BadValue`.

`GetProperty(delete=True)` is now one explicit authority transition. A
complete type-matching read returns the reply, removes the property, and emits
the existing deletion notification; partial reads, type mismatches, missing
properties, and failed reads preserve state. Reducer, core-dispatch,
same-namespace multi-client, and cross-namespace portal regressions all use the
maximum wire length and prove exact bytes, no protocol error, and post-reply
deletion. No Kitty, xmonad, compositor, namespace-policy, or portal special
case was added. Physical copy/paste and the strict normal-session verifier
remain the promotion gate.

## 2026-07-26: Four-Kitty Geometry Follows The Engine Work Area

The first four-Kitty run after the output-scoped frame-service change rendered
and exited cleanly, but its verifier rejected the missing pre-bar target
`1280x1440_0_0`. The retained layout proved that the expectation was stale:
the active status-bar reservation correctly changed the primary work area to
`2560x1426_0_14`. Xmonad and Engine produced one `1280x1426` master pane and
three `1280x475`, `1280x475`, and `1280x476` stack panes that covered the work
area exactly through `y=1440`.

The verifier now correlates the four-surface atomic resize transaction with its
workspace projection, obtains that output's applied Engine work area, and
derives the expected Tall geometry from the bounded rectangle. It no longer
assumes a particular output mode, origin, or absence of reservations.
Mutation fixtures use the 14-pixel reservation and reject an incomplete
projection or a one-pixel work-area/tile mismatch.

The corrected strict gate passes the retained physical run: 259 mixed exports
matched 259 target, pipeline, frame-surface creation and retirement lifetimes;
generation and recovery replacement remained zero; maximum input dwell was
20 ms, render time 20 ms, and submit-to-page-flip observation 32 ms. Both
outputs retained their valid startup lifecycle, the WM and session-control
ledgers drained, and cleanup recorded zero unexpected protocol errors. The
complete normal xmonad gate and one more consecutive four-Kitty cycle remain
before the frame-service lifecycle slice is promoted.

## 2026-07-26: Output-Scoped Frame Service Passes The Xmobar Gate

The first physical capture from frame-service commit `9b14ea9` passed the
focused unmodified-xmobar verifier. One 14-pixel top reservation reduced both
output work areas exactly; managed Kitty pixels began at `y=14`, and the bar
remained in the full-output client-positioned scene. Both button and axis
packets routed to that generic role without changing Kitty keyboard focus.
Workspace 2/1 and three VT suspend/resume cycles preserved the bar, managed
pixels, pointer, and keyboard.

The run created and retired 50 mixed composition targets, pipelines, and frame
surfaces with zero generation or recovery replacement. Both startup outputs
retained synchronous modeset proof, the WM worker drained with no pending or
rejected requests, and native suspension, session health, protocol accounting,
cleanup, input guard, and TTY restoration were clean. This closes the focused
status-bar gate and provides the first physical confirmation of the
output-scoped reducer. The four-Kitty and complete normal xmonad captures
remain required from the same commit before the lifecycle change is promoted.

## 2026-07-25: Frame Service Is An Output-Scoped Engine Reduction

The status-bar work-area run completed 162 mixed exports with balanced target,
pipeline, and frame-surface counts, but the native service still selected work
from aggregate booleans such as “some output has retirement” and “some output
has a pending frame.” That representation discarded output identity and made
fairness, primary reservation, callback stalls, and exact effect ordering
implicit in backend control flow.

The local Niri source reinforces two reusable boundaries without changing
Sophia's product architecture: frame state belongs to each output, and
scheduling policy should be testable independently from renderer and KMS
execution. Sophia does not adopt Niri's Wayland state graph, compositor
authority, protocol objects, or damage implementation. Damage history and
leased buffer pools remain deferred until the post-soak efficiency milestone.

Engine now reduces a bounded immutable observation for every output into named
effects: poll one output's retirement, submit the queued primary presentation,
or submit one output's pending frame. It validates a unique stable output set
and exactly one stable primary, orders retirement before new submission,
reserves a queued primary presentation without starving ready secondary
outputs, reobserves backend state after every effect, never reissues an effect
that failed to advance within the pass, and fails closed at a derived effect
budget. Backend-live maps those effects to mechanism and contains no runtime
selection policy.

Deterministic Engine tests cover idle, mixed retirement/presentation/pending
ordering, secondary fairness, primary reservation, stalled observation,
invalid identity sets, mutation during a pass, and budget exhaustion. The
physical promotion boundary remains unchanged: the focused xmobar, four-Kitty,
and normal xmonad gates must pass from this same lifecycle commit before the
architecture is promoted.

## 2026-07-25: Synchronous Initial Modeset Is Startup Presentation Proof

The first crash-free four-Kitty capture still failed verification because
startup forced-detached one secondary scanout after 750 ms. The trace showed
that both initial modesets had completed successfully, but startup immediately
queued an identical event-bearing framebuffer on each output and required an
asynchronous callback. The primary produced that callback; the secondary
driver did not emit an event for its redundant commit. Recovery then abandoned
a healthy buffer, recreated the DRM session, and repeated the same baseline.

Initial modeset is a synchronous KMS operation. A successful return already
proves that output's first framebuffer was committed and is not an in-flight
page flip. Native output state now records that fact explicitly. Startup
readiness accepts either this synchronous proof or an accepted asynchronous
page-flip callback, and unchanged-frame suppression no longer schedules a
redundant baseline solely to manufacture an event. Reduced per-output evidence
names `proof=synchronous_modeset`; the physical verifier requires exactly one
record for each of the two outputs. Callback requirements remain unchanged for
subsequent asynchronous commits.

The first physical run confirmed this correction: both outputs emitted
synchronous proof, startup performed no recovery or forced detach, and maximum
submit-to-page-flip latency fell from 119 ms to 36 ms. It also exposed four
previously opaque native submit failures later in the workflow. Export counts
showed three CPU attempts and 159 successful mixed exports, but only 158 total
submissions including the two initial modesets. Native submission now emits a
reduced failure record containing the output, submit stage, and generic content
class so the next capture can distinguish export, framebuffer, atomic request,
and commit failure without native handles.

The next capture recorded zero native submit failures and clean native drain,
but final completion still applied the old callback-only independence check to
the unchanged secondary output. The completion invariant now accepts exactly
two balanced forms: one successful synchronous submission with no callback or
retirement, or an asynchronous stream with equal callbacks and retirements and
one retained displayed submission. Both forms still require a nonzero export;
mixed, incomplete, or callback-imbalanced lifecycles fail closed.

The following capture completed that lifecycle cleanly. Startup reported both
outputs ready with no recovery, input queue dwell fell within budget at 91 ms,
and every native submission, callback, retirement, and cleanup balanced. The
remaining verifier failure was one 202 ms submit-to-callback interval while a
four-to-three-window close synchronously waited for two X Authority configure
delivery acknowledgements. Adjacent page flips completed in roughly 25 ms.
The kernel path is therefore not the demonstrated source of this outlier; the
owner thread stopped polling DRM events while WM control delivery blocked.
The next latency correction must make configure acknowledgement intake
incremental or otherwise keep native event servicing live during that wait.

## 2026-07-25: Persistent Native Targets Are Isolated By Render Class

The latest physical four-Kitty evidence showed that a stable output epoch
created a GBM/EGL target and GL pipeline for every mixed frame. That avoided an
older AMDGPU command-stream rejection by destroying mixed DMA-BUF state before
the next CPU upload, but replaced the hazard with frame-rate resource churn,
185 ms page-flip latency, and 291 ms input queue dwell.

Native scanout now keeps separate persistent CPU, DMA-BUF, and composition
targets for each output context. Render classes cannot leak EGL/GL state into
one another, while each class reuses its target across a stable
size/format/modifier epoch. An exported scanout buffer retains a reference to
the persistent native surface, so buffer release still precedes surface
destruction without forcing the context or pipeline to be rebuilt per frame.
An epoch change retires the affected target; a bounded retry may replace a
target after an explicitly classified EGL, GL, upload, or composition failure.

Reduced completion evidence reports total and per-class target creation plus
epoch- and recovery-driven replacement counts. The four-Kitty verifier now
requires sustained mixed composition, class-consistent creation counts, zero
replacement in the stable workload, zero launch-admission timeout, and bounded
input, upload, and page-flip latency. Local compilation and verifier mutation
coverage establish the ownership model; the physical workload and recovery
retirement proof remain open.

The first physical run of that implementation crashed on its third mixed frame
with an AMDGPU command-stream rejection. A bounded two-target experiment then
crashed at the same point: target A rendered frame one, target B rendered frame
two, and the driver rejected target A's first reuse after its exported KMS
lease had retired. This disproves both cross-class contamination and an
in-flight front-buffer lease as the complete cause. The retained Mesa/AMDGPU
path cannot safely reuse a composition EGL context after the current DMA-BUF
import sequence.

Mixed composition has therefore returned to the previously proven fail-safe:
destroy its context, pipeline, and target after every exported frame. CPU and
direct DMA-BUF targets remain class-isolated and persistent. Reduced evidence
requires composition creation and retirement counts to match mixed exports,
while epoch and recovery replacement of the other classes remains zero. This
restores startup correctness without falsely closing the lifetime milestone.
The next optimization must change the import/synchronization architecture, not
extend an unsafe context pool.

The same milestone now names click-to-focus separately from pointer motion and
client click-drag delivery. A primary click on an unfocused visible surface
must first select that surface through the blind WM interface, then deliver
ordered client input to the newly focused target. This is pending work; no
xmonad- or application-specific policy belongs in Engine.

## 2026-07-25: Native Service Must Preserve The Resize Epoch Barrier

The first physical four-Kitty run isolated the remaining corruption. Xmonad
requested three 1280-by-480 stack buffers, and Kitty produced them, but the
asynchronous native service scheduled the queued Presents while the WM layout
was still pending. The per-batch defer flag did not survive that service
boundary. Those buffers were compared with the old 1280-by-720 or
1280-by-1440 geometry and rejected; the 300 ms policy deadline then expired,
and the fourth surface remained visible at its 80-by-60 staging offset.

Presentation scheduling is now persistently blocked for the lifetime of the
pending layout. KMS retirement continues, but queued Presents cannot enter
scanout until the layout commits. A timeout rejects the complete quarantined
queue, preserves the prior displayed surfaces, does not focus the uncommitted
surface, and leaves admission eligible for one retry after rollback pixels
arrive. The xmonad bridge uses the existing bounded two-second maximum so a
multi-client resize is not failed merely because three clients repaint
serially. This state is surface- and transaction-based; no application-specific
branch was added.

The first validation run exposed a necessary refinement: the startup buffer
may precede the first WM configure. Holding that wrong-size Present withheld
the feedback Kitty needed before allocating the configured buffer, producing a
startup deadlock. A Present whose pixels conflict with the pending requested
size is therefore completed as a controlled skip immediately. Only matching
pixels, or pixels for a moved-only surface with no size request, enter the
quarantine.

The successful four-window transition also invalidated cloning the primary
mixed frame onto every output. The primary composition was 2560 by 1440 while
the secondary scanout target was 1920 by 1080, producing one controlled export
failure per primary Present and a final teardown error. Mixed X11 composition
now stays on its owning primary output; other outputs retain their own
output-sized frames until Engine has an explicit per-output scene projection.

## 2026-07-25: Present Configure And Pixel Size Define Resize Readiness

The physical three- and four-Kitty trace disproved the earlier placement-only
resize fix. Xmonad requested 1280-by-720 and 1280-by-480 tiles, but every
retired Kitty source remained 1280-by-1440. The frontend emitted core
`ConfigureNotify` only, although Mesa's DRI3 loader selects Present
`ConfigureNotify` and uses it to update drawable dimensions. The session then
mistook the already-updated window geometry for matching pixels and committed
the layout before the client had allocated a resized DMA-BUF.

Engine-driven X11 resize now emits the standard mask-selected Present
configuration event in addition to core configure and expose delivery. Live
resize readiness resolves the actual DMA-BUF or CPU-buffer dimensions by
handle; target window geometry is no longer pixel evidence. Present submissions
for a partial multi-surface resize remain queued while the layout is pending,
and a mismatched source is rejected rather than clipped and reported as a
successful resize. Configure evidence is named delivery rather than
acknowledgement because X11 clients do not acknowledge core configure events.

The same trace exposed an independent output-service defect: the primary output
continued retiring mixed frames while output 2 retained only its startup
submission. Mixed and retained-resume frames are now queued on every active
output so each KMS head participates in the presentation lifecycle.

## 2026-07-25: Retained Frames Follow One Atomic Layout Snapshot

A physical three-Kitty xmonad run kept submitting and retiring KMS frames but
showed a blinking third tile, then only two visible tiles after a TTY
round-trip. The trace proved that client DMA-BUF sources remained valid while
individual Present records crossed a two-to-three-window layout transition with
different geometry. Per-surface page-flip health was therefore not sufficient
full-state evidence.

The production runtime keeps retained frame ownership separate from
placement. Every cycle projects queued and displayed Presents through one
stack-ordered WM layout snapshot before composition. DMA-BUF pixels remain
unscaled; clipping is not accepted as proof that a client completed a resize.
Present source offsets remain frontend facts carried to the generic backend. The first displayed
buffer stays busy until replacement instead of receiving an immediate Idle.
Native resume also queues a complete retained mixed frame, so quiet clients do
not have to repaint merely to recover visible contents after a VT switch.

This is protocol-neutral visual state. The Engine and renderer contain no
Kitty or xmonad policy. Physical promotion remains open until three windows
stay visible before and after a TTY2/TTY3 round-trip.

## 2026-07-24: Empty Desktop Input Remains Engine-Owned

The first physical xmonad run after controlled stale-Present retirement proved
that Kitty exited normally and both outputs received correction frames, but the
remaining cursor appeared frozen. The live-session policy selected
`ShortcutsOnly` when the startup child had exited and both focus and proof
surface were absent. That mode intentionally discarded pointer events even
though no client surface remained.

An empty desktop now uses the ordinary full physical-input path. Engine still
consumes global shortcuts first, keyboard focus rejects application keys, and
scene hit-testing produces no client pointer target; only the session-owned
hardware cursor moves. This is protocol- and application-neutral and adds no
empty-desktop object or Kitty branch.

The same run exposed four `RANDR:GetOutputProperty` `BadValue` errors that also
occurred in the Kitty-only profile. Conventional RandR property atoms are now
created with the X server's shared atom table. Valid outputs return an empty
property for unavailable hardware identity and `CARDINAL(0)` for
`non-desktop`; Sophia does not invent EDID or connector data. Invalid outputs
and atoms remain protocol errors. A two-output real-Kitty smoke retains this
compatibility evidence without weakening normal-session error policy.

## 2026-07-24: Stale Present Retirement Is A Controlled Settlement

An xmonad physical run exposed a normal client-exit race: Kitty exited after
Present transaction 778 entered KMS but before its page-flip callback. The
surface-removal batch advanced Engine state, so retirement correctly rejected
the prepared candidate as `RejectedStaleSurface`; the live backend incorrectly
promoted that controlled result into a fatal session error.

Prepared retirement now remains ordered through the production coordinator:
Engine revalidates first, then the backend maps a committed result to
Present `Flip` and a rejected result to `Skip`, followed by `Idle` and exact
resource release. A rejected retirement never becomes a stable or focusable
surface and the current Engine snapshot is projected unchanged. Missing or
duplicate presentation resources remain fatal because they indicate broken
ownership rather than an ordinary asynchronous race.

CPU-frame preservation now follows the post-batch active transaction set
instead of a removed pre-batch DMA-BUF surface. Removing the last GPU surface
therefore queues the current CPU snapshot behind the in-flight frame, allowing
the asynchronous service to replace exited-client pixels after retirement.
Focused regressions reproduce the removal-before-retirement ordering without
physical hardware and require `Skip`, `Idle`, unchanged Engine state, and
exactly-once resource cleanup. This supersedes the earlier wording that stale
prepared retirement invokes no feedback callback: it invokes no successful
`Flip`, but it must still settle backend and protocol lifetimes.

## 2026-07-24: Architecture Conformance First Slice

- Audited production and test source layout against `docs/style-guide.md` and
  `docs/dod.md`; added an executable exact-path exception ledger so existing
  debt is visible and new unreviewed violations fail validation.
- Advanced the blind WM contract to API v3. Launch requests now carry only an
  opaque nonzero `SessionApplicationId`; executable names and application roles
  remain session-owned. Existing CLI evidence labels remain stable.
- Removed the application-protocol name from the renderer's built-in default
  cursor without changing its dimensions, hotspot, or pixels.
- Removed allocation and sorting from Engine input hit testing and cached the
  backend visual runtime's input-layer projection at authority-update
  boundaries.
- Split the production visual runtime into a 795-line transaction/presentation
  facade plus focused native-scanout and asynchronous-service modules. Present
  feedback now crosses the backend/session boundary through a bounded owned
  queue, and KMS retirement performs explicit Engine commit, protocol feedback,
  and output projection steps instead of callback-owned mutation.
- Removed visual-state seeding from backend and CLI startup. The runtime begins
  empty, accepts initial generation zero only through normal Engine authority
  commit, and rejects a forged nonzero initial generation.
- Centralized authority-transaction layer templates under Engine, preserving
  namespace identity and stack order for both production and deterministic
  backend paths. Moved protocol cursor coverage to the crate boundary and
  removed implementation-only legacy-WM builder and atomic-helper inline tests.
- Replaced direct native-scanout library printing with `tracing` while
  preserving the stable evidence message bodies emitted through CLI-installed
  subscribers.
- Converted native EGL/GBM pixel and lifecycle diagnostics to `tracing` and
  removed the process ID from DMA-BUF lifecycle output. Pixel evidence message
  bodies remain compatible with the existing verifier.
- Converted X authority dispatch, socket-write, close, and input diagnostics to
  `tracing`. Request-byte prefixes, file descriptors, raw XIDs, and key details
  are now redacted; diagnostics retain only opaque client IDs, protocol
  opcodes/counts, routing decisions, and bounded timing.
- Converted the private legacy-WM opcode trace to `tracing`; the source-layout
  ledger now has no direct-library-printing exceptions.
- All workspace targets compile with all features. Focused protocol, Engine,
  renderer, backend, WM, and bridge tests pass. Strict workspace Clippy remains
  a tracked migration gate because pre-existing native renderer argument
  bundles and style warnings are not yet clean.

## 2026-07-19: X-Centric Product Direction And Wayland Retirement

Sophia is an X-centric product built on a protocol-neutral architecture. X11
is the sole supported application protocol and the native X Server Frontend is
the product vehicle. Engine transactions, routed input, namespaces, portals,
rendering, and presentation remain independent of X11 object identity so a
future translator can be evaluated without moving authority.

The Smithay-backed Wayland frontend is retired from the workspace, CLI,
launcher, dependencies, documentation contracts, and validation gates. Its
source, tools, fixtures, last Kitty SHM evidence, and controlled linear
DMA-BUF evidence are frozen under `research/wayland`. Those results proved that
the Engine boundary was not X-shaped; they do not create an ongoing Wayland
compatibility promise.

Future application protocols are not deferred backlog. A translator or native
Sophia interface requires named product evidence, an explicit specification
amendment, existing authority boundaries, and bounded maintenance cost. Sophia
will not import another protocol ecosystem's shell, workspace, input,
presentation, or compositor-extension architecture.

## 2026-07-19: Milestone 8 Close, Sequence, And Soak Results

Firefox's intermittent XCB abort was an output-order race: asynchronous writers
could snapshot sequence N, wait behind a reply for N+1, then emit an event
carrying N. Request-sequence publication and every protocol, control, and input
event snapshot are now serialized by the output socket lock.

Close routing selects an exact `WM_DELETE_WINDOW` target, its nearest
protocol-advertising ancestor, or the only unambiguous protocol window. A
client with no cooperative target follows the bounded terminate path. The soak
also established bounded launch/focus settlement, close retries during busy
layout proposals, stale-surface WM resynchronization, and explicit discard of
the terminal logout chord's undeliverable release batch.

The final unattended QEMU run lasted 1,891,936 ms and completed 22 terminal,
Firefox, and GTK launcher cycles with 66 closes, 11 recovered bridge restarts,
all six Firefox semantic stages, zero unexpected protocol errors, no pending
WM/action/input state, and clean application, frontend, namespace, Xauthority,
and native presentation teardown. Three consecutive mixed-application runs
also passed before the soak.

## 2026-07-18: Semantic Firefox Gate And Xmonad Focus Reconciliation

The M8 page proof no longer treats changing pixels or fixed host sleeps as
evidence for browser behavior. The X frontend reduces title changes to property
name and byte length only; XIDs, namespaces, and title bytes remain inside the
authority. Six monotonically sized witnesses prove load, keyboard, clipboard,
primary selection, resize, and dialog stages. Separate reduced counters require
at least two selection-owner changes and two conversions before the session can
publish a complete Firefox record.

The first evidence-driven close attempt exposed a bridge synchronization bug.
Xmonad could focus a newly managed synthetic window after producing its layout,
while the bridge returned without reconciling the private X server's current
focus. Engine therefore retained the previous opaque surface and a subsequent
close-focused action targeted the wrong client. The bridge now queries its own
synthetic focus after each quiet layout cycle and translates that opaque window
back to `FocusSurface`; no XID or client metadata crosses the WM boundary.

Application launch waits for both process start and a committed layout. Close
waits for the WM action acknowledgement and a zero-status managed-process exit.
The soak repeats those evidence-driven terminal, Firefox, and launcher restarts
instead of fixed-delay chords, and its verifier requires twenty restarts of
each application, sixty committed closes, the semantic Firefox proof, zero
unexpected protocol errors, and clean final ownership. Offline verifier
regressions and the complete all-features test suite pass. Real mix and soak
promotion remain pending their unattended QEMU runs.

## 2026-07-18: Milestone 8 Evidence And Offline Application Shape

Normal sessions now identify every startup and action-launched application by
its approved registry ID, terminate the complete process group when its leader
exits, and treat every reduced X protocol error as fatal. Separate health and
post-teardown records preserve schema-14 while exposing pending WM/action/input
state and confirming frontend, namespace, Xauthority, and application cleanup.

The retained Firefox probe forces native X11, uses an isolated profile and
bounded process-group termination, and requires nonzero pixels with
`first_error=none`. A real host run now passes after adding bounded
`MIT-SHM CreatePixmap`, core `GetImage` and `ReparentWindow`, XKB `GetControls`,
larger bounded image/property payloads, and semantic classification of the
window-zero probes Firefox deliberately tolerates. The offline
page advances visible content and monotonically sized title witnesses after
keyboard, clipboard, primary-selection, resize, and dialog stages without
putting title bytes in evidence. Explicit `xmonad-m8-mix` and
`xmonad-m8-soak` scenarios use an optional Firefox/vkcube/Lavapipe image profile
and have strict positive plus negative fixture verifiers. The self-contained
image builder now resolves an installed xmonad and recursively includes and
validates Firefox's ELF dependency closure. The host Firefox proof completed
396 requests across 45 opcodes, two committed runtime surfaces, 5,971,968 CPU
buffer bytes, nonzero pixels, and no unexpected protocol error.

The first real mixed QEMU run reached xterm and Lavapipe but exposed an
application-size/recovery seam: `vkcube` retained its 500x500 default while the
post-bar xmonad layout requested 640x720. The guest now starts it at the stable
tiled size, and the host harness waits for per-application start/exit evidence
instead of fixed sleeps. The mix and 30-minute soak remain open until those
revised real gates pass; fixture verifiers and the complete offline test suite
are green.

## 2026-07-14: Explicit Final Scanout Retirement

The post-completion X11 allocator failure exposed a teardown ownership gap.
Persistent presentation deliberately retained the last displayed submission,
but the bounded session drained only the in-flight submission. Returning from
the loop therefore dropped the last GBM owner implicitly without first
retiring its framebuffer, mode blob, and imported GEM handles through the live
DRM device.

Persistent runtime shutdown now explicitly retires that displayed submission,
retries any reduced cleanup while the DRM device and renderer context are
still alive, and refuses clean completion if either in-flight or cleanup state
remains. Lifecycle diagnostics bracket the terminal retirement without logging
native handles. On X13, the focused backend regression and native-feature CLI
build pass, followed by ten of ten uninstrumented exact-text native stability
runs with clean evidence and no allocator diagnostic. The three operator-typed
runs remain the physical acceptance gate.

## 2026-07-14: Portal Requests And Grants Are Separate State

Portal policy decisions no longer need to double as execution authority. A
generic I/O-free lifecycle now retains deadline-bound request facts separately
from single-use grants. Allowed requests create active grants bound to source
generation and broker generation; completion, executor failure, expiry,
namespace disconnect, owner change, and broker restart have explicit terminal
transitions. A caller supplies monotonic time, the active set is capped at 64,
and no payload or operating-system handle enters this state. The first broker
IPC slice will use this reducer for every portal kind while clipboard remains
the first concrete executor.

## 2026-07-13: Core Keyboard Map Offsets And Semantic Input Evidence

The X13 stability workload exposed a false-positive input proof: xterm visibly
echoed repeated `^@` control notation, never printed its `received:` result, but
the session passed because fourteen events flushed and later pixels changed.
The `GetKeyboardMapping` decoder read the request padding byte as
`first_keycode` and the real first-keycode byte as `count`. Xterm therefore
cached keysyms for keycodes 0 through 7 while Sophia delivered normal core
keycodes such as 39 for `s`; Xlib translated every delivered key to NUL.

The decoder now reads the protocol body fields at bytes 4 and 5. Both wire byte
orders have regression coverage, and the real-xterm input smoke requires its
shell to receive exactly `sophia`. The live proof likewise uses an owner-only
result channel and emits schema 11 only after exact terminal bytes, flushed
delivery tokens, changed focused-surface pixels, and presentation all agree.
Pixel change alone is no longer input evidence. Kitty remains a separate
Wayland client proof; its X11 mode needs modern extension coverage beyond this
core-keyboard regression.

On X13, the standalone real-xterm smoke and all ten native repetitions reported
exact six-byte `sophia` receipt with no `^@` substitution. Nine native runs
exited cleanly. The tenth emitted complete schema-11 presentation and cleanup
records, then glibc reported `corrupted size vs. prev_size` during process
teardown. That preserves the allocator lifetime issue as a separate unresolved
bug; it does not weaken the now-semantic keyboard result.

## 2026-07-13: X11 Input Target Race And GBM Owner Drop Order

Two fresh dedicated-X13 milestone attempts stopped at different seams. The first
routed and flushed all fourteen physical key events but received no later xterm
pixels. The second reached terminal content and Engine-applied focus, then
aborted with `free(): invalid pointer` before input readiness. Both runs restored
`keyd`, released DRM ownership, and left no Sophia process or core file.

Inspection found that Engine `FocusSurface` control and client-selected keyboard
delivery shared one atomic X window. A late focus command could replace xterm's
VT child with its top-level surface window after the child selected key events.
Those states are now separate: focus control updates only the surface window,
while key delivery retains the latest client window selecting key events and
uses the focused surface only as a fallback.

The native CPU-upload path keeps each locked GBM front buffer and its originating
surface alive through KMS retirement. That owner's destruction order is now
explicit: release the front-buffer lock first, then release the surface. A
shared persistent GBM/EGL surface was tested and rejected after it reproduced a
pre-input guest crash; the proven per-frame EGL surface path remains in place.

`tools/run_x11_live_session_stability.sh` adds bounded normal, lifecycle-trace,
GDB, and core-capture modes. Timeout evidence now distinguishes no client
update, stale buffer generation, unchanged composition, and missing native
presentation. An X13 QEMU rerun now reaches changed pixels for all fourteen
physical key events and exits without the allocator abort. Its independent
pointer-selection proof still times out with observed-but-unrouted events, so
repeated local TTY runs and the full physical milestone gate remain required.

## 2026-07-13: Explicit Portal Taxonomy

The portal milestone began by removing two ambiguous protocol encodings.
`Screenshot` had represented both still capture and recording, while URI-open
requests were labeled as notifications and distinguished only by a type hint.
`PortalTransferKind` now has explicit clipboard, drag-and-drop, file-handoff,
screen-capture, screen-recording, URI-open, and notification values. Each maps
directly to its namespace capability. Reducer and codec regressions cover every
kind; established codec numbers for the five existing values remain stable,
with recording and URI-open using new tags. Request/grant lifecycle separation
is the next portal slice.

## 2026-07-13: Explicit X Session Profiles And Map Isolation

The live X launcher now selects `classic` or `confined` explicitly. Classic
remains the default shared-X group. A confined run receives a fresh registry
namespace with explicit zero portal capabilities, and those immutable facts
flow through every connection admission. The session status record exposes the
selected profile and directional capability bitsets without exposing namespace
identity.

The first simultaneous confined socket proof assigned two clients distinct
namespaces and exposed a real leak: `MapWindow` changed lifecycle state without
checking the runtime resource table. The runtime now performs namespace-aware
window lookup before mapping, so the second client receives native `BadAccess`;
classic same-namespace mapping remains valid. The following socket expansion
closes properties, selections, metadata, event selection, and routed input.

The next socket expansion found the same missing-boundary pattern in property
and selection paths. `ChangeProperty` previously keyed a foreign XID under the
requester's namespace and could emit a metadata candidate without checking the
window owner. Selection ownership and conversion likewise trusted the owner or
requestor XID instead of the admitted namespace. Runtime/dispatch now validate
all three before mutation or portal construction. The wire proof requires
`BadAccess` for foreign property and owner changes, normal
`SelectionNotify(property=None)` for foreign conversion, and zero metadata
candidates.

The final confinement expansion found that the socket bridge updated its
authority-local keyboard target from `CWEventMask` before dispatch authorization.
A rejected foreign event subscription could therefore redirect later input in
the requester's private worker to another namespace's XID. Event-target changes
now occur only after namespace validation. The drawable validator also
classifies a resource once so a foreign window's `CrossNamespaceDenied` is not
overwritten by a failed pixmap fallback. A routed simultaneous-client proof
requires native `BadAccess`, sends a broker-addressed key to the requester, and
verifies that its event target remains the local root; the broker's separate
queue regressions prove delivery stays client-specific. This completes the
bounded Milestone 1 confinement matrix; full XKB, XI2, focus, and grab semantics
remain Milestone 3 work.

The final admission-lifecycle gap was targeted supervisor revocation. Concurrent
workers now report only their session-issued `ClientAdmissionId` to frontend
supervision and retain a cloned socket solely as a disconnect handle. A
`RevokeAdmission` service command shuts down that one socket; the worker still
owns writer shutdown, private-route removal, connection-ledger cleanup, surface
removal observation, and admission-lease revocation in that order. A
pre-admission command is retained until the matching worker attaches, closing
the allocation/worker-registration race. A simultaneous classic-client
regression revokes admission 1, observes its surface removal and inaccessible
old window, then creates another window through the uninterrupted peer. This
completes the namespace/admission foundation and makes the portal broker plus
X11 clipboard the active milestone.

## 2026-07-13: Live Xauthority Ownership

The live X session no longer relies on an unauthenticated owner-only socket. Its
supervisor obtains a fresh 128-bit cookie from the kernel for every run, writes
a standard `FamilyLocal` `MIT-MAGIC-COOKIE-1` record with mode `0600`, syncs the
complete record before exposing its path, passes `XAUTHORITY` to both launched
terminals, and removes the file through explicit and drop cleanup. A private,
owner-only `XDG_RUNTIME_DIR` is preferred; the random, create-new owner-only
file remains safe when the system temporary directory is the fallback.

The frontend validates that cookie before invoking session admission. Policy
sees only `MitMagicCookie1` provenance and kernel peer credentials, never the
secret. A regression proves bad cookies do not invoke policy, while the accepted
connection is admitted once and revoked once. Fresh per-session generation is
the rotation boundary; confined launch credentials remain future policy work.

## 2026-07-13: Per-Connection X Admission Boundary

The native X frontend now calls a protocol-neutral session policy after setup
authentication and before allocating X client or resource-range identity. The
policy receives only the bounded authentication method and kernel Unix peer
credentials; it never receives raw cookie bytes. A successful decision returns
an immutable `ClientAdmissionContext` retained in an admission lease. Native
X11 setup failure represents denial, and teardown or any early worker error
revokes the lease after route and resource cleanup.

The live classic session backs that policy with its session-owned
`NamespaceRegistry`: it requires a peer UID matching the effective session UID,
allocates a distinct admission per connection, and intentionally assigns those
admissions the same classic-shared namespace. This removes the listener-wide
identity shortcut without weakening classic X semantics. Confined launch and
targeted supervisor revocation are now implemented as described above.

## 2026-07-13: X11-First Namespace And Portal Critical Path

Sophia's next architecture work is the native X Server Frontend, not broader
Wayland protocol or DMA-BUF coverage. The two-xterm frontend already proves
bounded concurrent workers, client-attributed transactions, targeted input,
Engine composition, and KMS presentation. Its next risk is no longer basic
visibility; it is admitting clients into the correct trust domain before more
X11 semantics depend on a hardcoded listener namespace.

The chosen dependency order is session-owned namespace admission, then a portal
broker with X11 `CLIPBOARD`/`PRIMARY` as its first complete adapter, then XKB,
grabs, Engine-derived output/resize, and standard presentation semantics.
Classic shared-X intentionally retains same-namespace resource visibility.
Confined sessions use distinct namespaces and explicit capabilities; XID ranges
remain creation/cleanup ledgers rather than access-control lists.

At this stage Wayland/Smithay stayed supported under maintenance gates. The
2026-07-19 retirement decision above supersedes that status. XLibre remained
frozen historical evidence and a possible future provider only if measured
native-X gaps later justified its authority and maintenance cost.

## 2026-07-13: Kitty DMA-BUF Direct-Scanout Boundary

Enabling the experimental DMA-BUF global for guarded Kitty failed before a
usable native presentation and surfaced the misleading scheduler invariant
`native frame was neither submitted nor retained for a later submit`,
disconnecting Kitty. Sophia's current DMA-BUF route is direct KMS scanout,
whose exporter requires the client buffer to match the physical output exactly.
An arbitrary Kitty toplevel therefore cannot be a valid client for that route,
regardless of the exact buffer that reached the failed run.

This is an architecture boundary, not evidence that Kitty can use the current
direct path. The controlled full-output XRGB producer remains the direct
DMA-BUF lifetime proof. The interactive Kitty harness now deliberately does
not advertise DMA-BUF and continues to prove native SHM composition, input,
recovery, and latency. The next DMA-BUF milestone is GPU composition: import
an arbitrary window-sized client DMA-BUF, scale/blend it into a Sophia-owned
output-sized render target, retain it through the target page-flip retirement,
and only then release the client buffer. Only that route can support Kitty
without requiring fullscreen, output-sized buffers.

## 2026-07-12: Controlled DMA-BUF First-Frame Heap Corruption

The first real controlled DMA-BUF run reached Sophia's full-size 1920x1200
client frame and recorded `sophia_wayland_frame` with `buffer=dmabuf`. The
native process then aborted with `corrupted size vs. prev_size`, disconnecting
the producer before presentation retirement or buffer release could be proven.
This is a renderer/resource-ownership safety failure, not DMA-BUF evidence.

The producer itself now follows the compositor's initial xdg configure rather
than assuming 640x480, and uses a driver-supported explicit linear GBM
allocation. Those corrections moved the test past allocation and target-size
rejection; they did not make the native import/presentation path safe. The
300-frame lifecycle and three-Kitty promotion gates remain blocked pending
allocator/lifetime diagnosis.

The next controlled rerun uses a GDB-backed diagnostic mode with explicit
DMA-BUF stages. The importer now detaches the EGLImage from its GL texture and
finishes that detach before destroying the EGLImage and dropping the imported
client FD. This makes input-image teardown independent from the retained GBM
front-buffer owner.

The GDB-backed three-frame rerun passed: each 1920x1200 frame completed EGL
image creation, rendering, texture detach, image destruction, KMS submission,
page-flip observation, scanout retirement, and client buffer release. The
session exited normally with three imports, three retirements, three callbacks,
no cleanup debt, and a 14 ms maximum submit-to-page-flip interval. The
GDB-backed 300-frame lifetime proof then completed with 300 imports,
submissions, page flips, and retirements, no allocator diagnostic or cleanup
debt, and the same 14 ms maximum submit-to-page-flip interval. A subsequent
normal release 300-frame run nevertheless aborted with `corrupted size vs.
prev_size` after frame 8 (an earlier normal run reached frame 13). This makes
the fault timing-sensitive: the GDB result is diagnostic evidence, not a
completed lifecycle gate. A release-timing trace then completed all 300 frames
with ordered ownership stages and an 18 ms maximum submit-to-page-flip interval.
One uninstrumented rerun and then three separately retained uninstrumented
300-frame runs all completed normally: each reported 300 imports, 300 callbacks
and retirements, no cleanup debt, no surviving process, and a 14 ms maximum
submit-to-page-flip interval. The next full promotion preflight nevertheless
aborted on its first uninstrumented DMA-BUF frame with `free(): invalid pointer`,
before Kitty started. A later post-repair normal run also aborted after frame 2.
The persistent CPU-upload texture was therefore isolated from imported images:
each import now gets a transient per-frame texture, which is deleted after
`glFinish` before EGLImage destruction. The repaired three-frame proof passed
with three imports, three retirements, and a 16 ms maximum interval. A normal
core-capture 300-frame run and three separate uninstrumented normal 300-frame
runs then all completed: every run had 300 imports, callbacks, and retirements,
no cleanup debt, no surviving process, and 14–16 ms maximum latency. This meets
the bounded controlled gate, while retaining the normal-stability wrapper as a
regression guard for the earlier intermittent abort. The next required evidence
is three guarded native-SHM Kitty runs; a later GPU-composition milestone must
precede any real-Kitty DMA-BUF runs.

## 2026-07-12: DMA-BUF Performance Gate and Renderer Safety Boundary

The current native Wayland/Kitty presentation route is SHM-backed and stable
enough to serve as the production fallback, but the latest hardware result was
about 110 ms input-to-presentation and therefore missed the 100 ms budget.
DMA-BUF descriptors are admitted only as a bounded single-plane linear subset;
their native import and presentation path remains explicitly experimental.
There is no passing real-hardware DMA-BUF result at this point.

A controlled external Wayland producer now allocates linear XRGB8888 GBM
buffers, alternates them only after `wl_buffer.release`, and waits for each
frame callback. The first hardware gate uses three frames; the second uses 300
frames to exercise import, presentation, feedback, and retirement lifetime.
Only after both pass may the three independent guarded real-Kitty runs begin.
Those acceptance runs remain on SHM until GPU composition exists. DMA-BUF stays
non-default until a real-Kitty GPU-composition log proves input, recovery,
presentation, and the 100 ms budget.

The current CPU composition copy and 2 ms native idle cadence are a safety
boundary, not merely a tuning choice. Removing the copy or tightening that loop
has reproduced native renderer/exporter heap corruption on hardware. Further
latency work must isolate that ownership fault before changing either setting.

## 2026-07-12: Native Wayland Replaces The Kitty Compatibility Runtime

Sophia's production Kitty path now terminates a private Wayland socket through
the Smithay-backed Sophia Wayland Authority. Engine input routes and layer
records are protocol-neutral; keyboard focus and pointer hit-testing remain in
Engine, while the authority translates accepted routes into `wl_keyboard` and
`wl_pointer` delivery. A real Kitty 0.47.4 process completes the headless smoke
with `DISPLAY` removed, changing nonzero SHM frames, and no X server process.

The installed launcher now uses the native Wayland/KMS session and retains the
independent Ctrl-Alt-Backspace recovery interlock. XLibre is excluded from the
production dependency graph and launcher; its frozen crate, CLI, patches,
scripts, fixtures, and notes live under `research/xlibre`.

The native-scanout session advertises a bounded single-plane linear/implicit
XRGB8888/ARGB8888 DMA-BUF subset. Accepted buffers cross the renderer boundary
as owned descriptors. Their experimental native import/presentation route is
now gated by the controlled first-frame/lifetime proof; arbitrary Kitty buffers
need GPU composition before they can enter this route. It is not yet recorded
as passing hardware evidence. Wayland
presentation and buffer-release feedback must remain withheld until the
matching KMS submission is observed as presented. The next evidence gate is the
controlled proof, followed by text, navigation, pointer, resize, sub-100 ms
presentation, clean exit, and TTY recovery in real Kitty.

## 2026-07-12: Installed-Session Input Recovery Interlock

The first installed Kitty operator run exposed a control-plane failure: scanout
reached a visible terminal, but normal keyboard delivery failed and the session
had no reliable local escape. The wrapper had stopped `keyd`, placed the TTY in
graphics/raw mode, and disabled XLibre VT switching while relying on the same
live input path for `exit`. A reboot then removed the runtime-directory logs.

The installed launcher now refuses graphics takeover until a separate libinput
guard observes one complete Ctrl-Alt-Backspace chord. A second chord requests a
graceful in-session exit without depending on focus, then lets the independent
guard force bounded wrapper cleanup if the live loop is wedged. Session groups
and XLibre receive TERM followed by a bounded KILL fallback; KD mode, termios,
and the previous `keyd` state are restored afterward. Full Ctrl-Alt-Fn switching
remains deferred until Sophia owns a correct VT/DRM suspend-and-resume cycle.

The input path now emits privacy-safe one-shot stages for poller readiness,
committed focus, observed keys, authority routing, and XTEST injection. Bounded
synthetic text also traverses Engine focus and evdev mapping instead of entering
the authority channel directly. Logs retain the latest and previous run in the
user state directory so a reboot does not erase the failing seam. The dedicated
TTY physical typing and deliberately wedged-session recovery run remain the
hardware acceptance gate.

Guard bring-up also exposed that libinput's path context can reject stable
`/dev/input/by-path` aliases before invoking Sophia's open callback. Native
input admission now canonicalizes configured absolute paths and honors the
requested read/write access mode without logging either path. The rebuilt QEMU
guest passes the 300-tick dual-output gate with all 14 keyboard events routed,
five pointer events routed, changed pixels, 112 native submissions, clean
retirement, and no callback rejection or cleanup debt.

The isolated recovery scenario now exercises the guard contract directly with
a virtio keyboard: one complete Ctrl-Alt-Backspace chord arms the independent
guard, a second chord triggers both the guard and the live loop, and both
virtual outputs drain with no in-flight scanout or cleanup debt before poweroff.
This is deliberately not recorded as host VT restoration evidence. A separate
same-build rerun of the default physical-text QEMU scenario routed all 14 key
events but timed out waiting for the expected xterm pixel change, so that
content regression remains open independently of the recovery gate.

The first guarded physical-TTY rerun then proved the interlock on hardware: the
first chord armed before takeover and the second returned to the text TTY
without rebooting. Persistent logs showed physical poller readiness, committed
focus, observed and routed keys, and XTEST injection, but Kitty received no
usable input. The installed XTEST protocol definition identifies the stopped
seam: FakeInput's `time` field is a delivery delay in milliseconds, while the
adapter had supplied libinput's monotonic event timestamp. The compatibility
injector now requests zero-delay delivery and synchronously checks each XTEST
request instead of treating a queued unchecked request as successful.

That hardware rerun also exposed the compatibility renderer's full-frame cost:
45 frames in 25.9 seconds, 8.74 MB read and hashed per frame, a 397 ms libinput
lag warning, and a maximum 798 ms submit-to-page-flip interval. The live bridge
now maintains X Damage trackers and a CPU-buffer base, emits one clipped packed
patch per damaged surface, and falls back to replacement only for initial,
resized, missing-base, or at-least-half-surface updates. XTEST uses an
independent connection and worker so capture cannot block channel draining.
The optimized dummy-XLibre proof presented routed xterm input in 17 ms with
three steady patches, 1.26 MB total readback, and a 9 ms maximum capture.

## 2026-07-11: Isolated Virtio-GPU Session Evidence

Sophia now has a direct-kernel QEMU initramfs builder and a headless session
harness. The guest has no storage or network device, uses serial control and an
unconnected Unix-domain VNC display sink, and owns emulated virtio-gpu and
virtio-keyboard devices. It starts udev, mounts devpts, launches real xterm,
opens the virtual input nodes through libinput, and runs persistent native
scanout for an exact `--max-ticks=300` budget without host DRM or VT access.

The passing run completed 300 session ticks, 42 native submissions, 41 steady
retirements, 41 accepted page-flip callbacks, two nonzero terminal exports,
injected terminal pixel change, and zero submit failures, retire failures,
rejected callbacks, saturated callback queues, in-flight frames, or cleanup
debt. The strict verifier accepted `/tmp/sophia-qemu-session.log`.

Guest bring-up exposed two real cross-driver defects. AddFB2 fallback passed a
linear modifier while clearing `DRM_MODE_FB_MODIFIERS`, which violated the DRM
crate's flag/value invariant; the implicit fallback now wraps the same planes
with `modifier=None`. Virtio-gpu also reports repeated zero page-flip sequence
values. Native CRTC routes now normalize driver values into strictly increasing
Sophia-local serials, preserving stale-event rejection across repeated values
and 32-bit sequence wrap. Focused regressions cover both fixes.

The guest virtual keyboard is present and opens through libinput, but the
current proof uses Sophia's bounded X key injection for the pixel-change check.
QMP-driven virtual-key input remains the next isolated input proof.

## 2026-07-10: Roadmap And Documentation Review

The xterm compatibility stream currently reaches `ImageText8`, emits four
ready `SurfaceTransaction` values, commits them through runtime, and passes the
deterministic composition/scanout lifecycle proof. Core drawing now updates
bounded XRGB8888 software buffers, renderer-live composes those bytes, and the
native EGL adapter can upload the composed frame into a GBM front buffer.
The TTY3 content proof now exports an exact composed xterm checksum through the
native GL/GBM path, submits that buffer to KMS, observes accepted page-flip
retirement, and drains cleanup. Requested and exported checksum evidence match;
the remaining presentation work is persistent-session ownership, not pixel
upload correctness.

The active milestone is therefore persistent session ownership, hardware
terminal-content presentation, and physical keyboard delivery. Injected core
key events already produce changed pixels in a real xterm. Pixel bytes remain
outside Sophia Engine and the blind WM protocol.

The persistent launcher now owns an explicit local display, one xterm, the X
Authority server, one live backend runtime, and the latest composed CPU scene.
A bounded real-xterm run passes repeated authority/runtime ticks and injected
pixel change. Building it exposed and fixed static drawing generations: X
Authority now advances a window generation after each emitted visual
transaction, so long-running Engine commits remain contiguous. Native scanout
now joins this owner behind `--native-scanout`: the same loop queues composed
CPU frames for GL/GBM export, polls native page flips, retires tracked KMS
submissions, and drains cleanup. Reduced schema 6 evidence records successful
submits, deferrals and failures, submit-to-page-flip latency, maximum in-flight
age, callback pressure, nonzero exports, authority drops, and cleanup debt.
The non-native repeated-xterm regression and strict verifier fixtures pass.

The strict persistent hardware proof now passes. Corrected counters first
exposed River ownership and then two real lifetime defects: the runtime retired
the newly displayed framebuffer instead of the previously displayed one, and
the shutdown loop retired a frame while immediately submitting another. The
persistent mode now performs a blocking initial modeset without waiting for an
event, retains the displayed owner until a later accepted page flip replaces
it, and has a retire-only idle/shutdown path. A 30-second TTY3 run completed 46
submissions with 45 steady retirements, six nonzero exports, zero dropped
authority batches, zero rejected callbacks, zero transition failures, and no
in-flight or cleanup debt. A subsequent bounded run also reports nonzero
submit-to-page-flip latency after fixing timestamp association.

Host iteration remains unnecessarily disruptive because River must release the
only DRM card. The next stability harness should boot Sophia in a headless QEMU
guest with `virtio-gpu`, serial control, virtual keyboard input, and no guest
compositor. Use that guest for the 300-tick and repeated-session proofs; retain
the AMD TTY run for final driver and modifier evidence.

Physical keyboard plumbing now enters the persistent owner through explicit
libinput event nodes. `InputFocusState` in Sophia Engine validates a seat's
focused surface against committed visual state before X Authority maps evdev
keycodes and modifiers to core events. The TTY keyboard node opens in a bounded
run, but the noninteractive validation process observed zero physical events;
an operator typing run is still the evidence gate.

Wayland was gated behind the operator-grade X session. Before considering it,
Sophia would connect the live session to the documented generic X11 WM bridge. The
bridge is an embedded minimal X server with synthetic windows; a configured
legacy WM is layout policy only and receives no physical input, raw metadata,
namespaces, or real client XIDs. Sophia Engine speaks only its generic WM IPC
and does not know which legacy WM the bridge supervises.

The xmonad source is cloned at `~/src/xmonad` commit `a9a8b5c` as the first
compatibility reference. It is not vendored and is not a Sophia runtime
dependency. The embedded server and real two-window xmonad process proof pass.
The remaining gate is to feed opaque live-session surface snapshots through the
same generic bridge socket and apply validated proposals to presented surfaces.

## Active Questions

- What is the smallest immutable admission record that lets the supervisor bind
  listener policy, peer credentials, cookie validation, session generation,
  namespace profile, and capabilities without exporting secrets?
- What bounded broker IPC keeps portal decisions and grant lifecycle pure while
  allowing runtime executors to retain X selection context, payload bytes, and
  later OS handles?
- Which XKB, grab, RandR, resize, and presentation gaps must close before the
  proven two-xterm hardware path can honestly become `session` evidence?

These questions remain probe-driven: implement the first observed missing path,
then rerun the relevant real-client smoke.

## 2026-07-11: Readable And Resizable Xmonad Session Path

The first physical xmonad attempt exposed that the earlier pixel-change proof
was not a visual proof: unimplemented core drawing painted damage bounds white,
and the partial fixed glyph table rendered ordinary punctuation as question
marks. The operator stopped the session without treating that run as milestone
evidence.

X Authority now preserves GC raster values, executes the xterm-used text,
fill, line, clear, copy, and image paths against bounded XRGB buffers, and
covers printable ASCII in its deterministic fixed raster. The real-xterm smoke
scans materialized pixels for the expected `Sophia` glyph sequence and reports
`ascii_marker_match=true`; nonzero bytes alone no longer pass it.

CPU drawing publication now distinguishes full replacements from tightly
packed damage patches. Resize keeps a buffer handle's generation monotonic,
publishes one correctly sized replacement, and applies later patches in order.
With fixed size constraints removed, the real-xmonad headless session completes
one configure acknowledgement, commits the resized layout and focus, and
observes changed pixels after injected input without authority backpressure.
The dedicated-TTY visual rerun remains the final gate.

## 2026-07-11: Real Xterm Through Generic Xmonad Policy

`sophia-live-session` now supervises an arbitrary Sophia WM socket process via
generic executable/argument flags. With `sophia-x11-wm-bridge` selected as that
process, the live session sends one opaque xterm surface to real xmonad, Engine
validates the response, and the committed placement drives composition,
hit-testing, backend visual state, and scanout. A headless integrated run proves
one moved surface, committed Engine focus, and a later injected terminal pixel
change. No xmonad identity enters Engine or the live-session policy path.

The Engine-to-X-Authority control seam now supports bounded configure/focus
commands and reduced acknowledgements keyed by `SurfaceId`. Probing arbitrary
full-output xterm resizing exposed a repaint loop in the core-drawing path, so
the first real one-client gate pins min/max size to the established live buffer
and uses xmonad for placement, stacking, and focus. Removing that constraint is
tracked explicitly rather than overstating resize compatibility.

## 2026-07-11: QMP Keyboard Proof And Presentation Boundaries

The isolated session no longer uses Sophia's internal core-X key injector for
its input claim. The guest announces readiness only after xterm pixels and
Engine-owned focus are stable. The host then sends `sophia` and Return through
QMP `input-send-event`, virtio-keyboard, the kernel input path, libinput, Engine
focus validation, and X Authority. The passing run observed and routed all 14
press/release events, changed later xterm pixels, completed exactly 300 session
ticks, submitted 46 native frames, retired 45 steady page flips, and drained
without rejected callbacks, failed transitions, or cleanup debt. Tick counting
pauses for a bounded five-second physical-input window so readiness at the last
scheduled tick cannot race QMP delivery.

The guest also exposes virtio-mouse and libinput maps pointer events to a
separate Engine device ID. The completed pointer slice performs QMP word
selection in the typed xterm. Five motion/button events pass through libinput,
Engine surface-only hit-testing/focus, and core X MotionNotify/Button events;
all five route and a second terminal pixel change is observed. The first drag
attempt exposed that targeting the last mapped X window was insufficient even
though all input reached Engine. Pointer events now carry only the routed
Sophia surface, and X Authority resolves that surface through its internal
surface/window table. This preserves the authority boundary: Engine never
receives or interprets the client XID.

Native presentation now has independent per-output scanout ownership, damage,
frame clocks, in-flight state, and retirement, proved with two QEMU heads. The
physical multi-connector AMD gate remains. Fixed-refresh evidence requires each
output to follow its own page-flip timeline without overlapping submission. VRR
remains a hardware proof gate: the property contract and Engine eligibility
policy exist, default off, but activation and fixed-refresh fallback still need
capable hardware evidence.

## 2026-07-11: Bounded Per-Output Timelines And Two-Connector QEMU Topology

Engine output discovery is now bounded to 16 descriptors. Backend assembly no
longer advances one global deterministic clock: it seeds an independent clock
for each discovered output using that output's fixed refresh rate. A separate
presentation registry tracks pending damage, one in-flight frame, exact
retirement, and the last retired serial per output. Two-output regressions prove
that 60 Hz and 120 Hz timelines advance independently, one output cannot submit
over an unretired frame, and a mismatched retirement cannot clear ownership.
These are scheduling invariants; the clock is not yet driven by DRM vblank.

A single virtio-gpu device configured with two scanouts exposed two connector
objects but only one connected connector, so it was rejected as multi-monitor
evidence. The accepted harness uses two isolated virtio GPU devices with one
scanout each. The guest reports two connectors and both connected; Engine
discovers two and creates two presentation timelines. That topology was the
prerequisite for native multi-output ownership, which is recorded below.

## 2026-07-11: Dual-Output Native Presentation And Fixed-Refresh Vsync

The persistent runtime now owns a bounded table of output-scoped frame targets,
callback intake, scanout submissions, displayed buffers, cleanup debt, and
retirement state. Native selection deterministically assigns disjoint
connector/CRTC/primary-plane chains, groups page-flip routes by DRM card, and
supports explicit selections so one card cannot silently resubmit its first
connector for every output.

The isolated QEMU session owns both virtio GPU outputs. Output 1 presents the
terminal while output 2 presents a deterministic Engine proof marker in the
extended desktop region; their checksums must differ. The 300-tick gate requires
nonzero per-output exports, submissions, callbacks, and retirements, plus zero
callback rejection, cleanup debt, overlapping submission, or non-monotonic
page-flip phase. Keyboard and pointer proofs remain mandatory in the same run.

VRR property discovery recognizes connector `VRR_CAPABLE` and CRTC
`VRR_ENABLED`. The Engine decision defaults off and permits enable only for one
opaque, unoccluded fullscreen surface without overlays or required composition.
Atomic page-flip request construction fails closed if VRR is requested without
the enable property. Activation and fallback remain an AMD hardware gate;
virtio-gpu is not accepted as VRR evidence.

The physical VRR gate now has a dedicated two-phase runner and strict reduced
evidence verifier. During implementation, the proof exposed that the native
page-flip builder carried `VRR_ENABLED`, but the modeset branch ignored the
same policy request. Modeset request construction now supports the property and
fails closed when its handle is absent. `tools/vrr_hardware_proof.sh` derives an
Enabled decision for one opaque, unoccluded fullscreen surface and commits
`VRR_ENABLED=true`, then derives an Ineligible decision for an overlay-present
scene and commits the fixed-refresh `false` fallback. It requires presented and
retired callbacks for both phases. The destructive AMD run is still pending
because it must be performed from the dedicated TTY, not the active graphical
session.

`tools/operator_keyboard_hardware_proof.sh` similarly packages the remaining
operator gate without guessing an input node. The operator supplies a stable
`...-event-kbd` path, waits for the physical-input readiness marker, and types
the expected lowercase proof text. Existing persistent-session evidence rejects
the run unless physical keys route through Engine focus and later xterm pixels
change.

## 2026-07-11: Exact Operator Input Evidence

The first AMD operator attempts exposed two proof-harness defects rather than a
new authority boundary. `keyd` exclusively owned the AT keyboard, so opening its
physical event node succeeded while libinput observed zero events. With `keyd`
stopped, Engine routed 27 events, but the original five-second deadline still
expired before a later xterm transaction changed the composed checksum.

The combined helper now detects an active `keyd`, stops it through an explicit
interactive `sudo sv down keyd`, and installs an EXIT trap that restores the
service. It has no separate Enter-to-begin prompt, preventing that confirmation
key from becoming the first exact-proof event.
The physical proof requires exact press/release pairs for the configured
lowercase text plus Return after Engine focus routing and evdev-to-core-X
translation. It gives the operator 15 seconds to complete that bounded sequence,
then starts a separate five-second pixel-settle deadline. The scanned-out xterm
shows the expected input, and readiness is withheld until those nonzero prompt
pixels are page-flip-confirmed on the primary output. Keyboard delivery freezes
after the exact Return release so operator retries cannot weaken the evidence.
Schema 2 input evidence records
expected events, matched events, and the later pixel change. The AMD acceptance
run now passes: all 14 events matched, xterm pixels changed, and the one-output
native session completed 62 submissions, 61 callbacks/retirements, 22 nonzero
exports, and zero overlap, phase, callback, transition, or cleanup failures.

Requiring a nonzero prompt baseline then exposed that the earlier QMP pixel
claim was a false positive: late prompt drawing changed the initial blank frame,
while routed key events still targeted the last mapped top-level X window.
Xterm selects core key events on its VT child through `CWEventMask`. Sophia X
Authority now parses that bounded value from `CreateWindow` and
`ChangeWindowAttributes`, retains the selected X window inside the authority,
and uses the last mapped window only as a fallback. Input readiness also
requires 500 milliseconds of quiescence after nonzero prompt pixels so event
selection cannot race the external sender. A rebuilt strict QEMU run
then matched all 14 events and changed pixels after the fully drawn prompt before
continuing through pointer and dual-output evidence. Raw X window identity never
crosses into Engine or WM state.

The first AMD VRR attempt then exposed a property-name mismatch. The connector
advertises the kernel-standard lowercase `vrr_capable`, while the selected CRTC
does expose uppercase `VRR_ENABLED`. Discovery had searched for uppercase
`VRR_CAPABLE` and therefore rejected capable hardware before building an atomic
request. The lookup now uses `vrr_capable` with the old uppercase spelling kept
only as a compatibility fallback for deterministic fixtures.

Non-destructive inspection after that correction reports connector 100, CRTC
86, `VRR_ENABLED` present, and `vrr_capable=0`. The current eDP panel is not VRR
capable, so activation/fallback evidence cannot be produced on this hardware.
The gate remains open for a connector reporting capability `1`; Sophia does not
override the value or treat a property contract without capability as proof.

## 2026-07-12: Temporary XLibre Compatibility Provider For Kitty

Kitty's installed X11 backend requires XKB and a working OpenGL context, while
Sophia X Authority deliberately does not yet advertise XKB, GLX, DRI3, or
Present. Pretending that Kitty was another core-drawing probe would therefore
produce a launcher that connected but could never render.

The first usable compatibility checkpoint instead reactivates the historical
XLibre bridge as an explicitly temporary protocol authority. XLibre runs on the
dummy video driver with software GL, no physical input devices, no TCP listener,
and a private MIT cookie. A persistent XComposite adapter owns the XIDs and
named pixmaps, converts readbacks into opaque `XLibrePrototype` surface
transactions, and never exposes client identity to Engine or the WM. Engine
continues to own physical input, focus routing, composition, frame scheduling,
and KMS. Core key events return through a bridge-private XTEST adapter until the
Sophia-owned X Authority has native GPU-buffer coverage.

The first real headless run used Kitty 0.47.4 against XLibre 1.25.1.8. It
materialized one 925 KB nonzero Kitty surface. Capture checksum deduplication
reduced a four-second run from 29 repeated batches to six actual pixel changes;
injected `sophia` plus Return then changed the composed checksum and completed
in 2.6 seconds. Native TTY presentation remains the operator gate.

The first installed-session input proof then showed that capture correctness
alone was insufficient: Kitty echoed typed characters several seconds late.
The launcher had used a debug build, the session cloned and repeatedly scanned
each 1280x720 frame, physical input was polled only after rendering, and native
export recreated its EGL/GL setup for every frame. The launcher now runs the
release binary; XLibre sessions acquire libinput on a bounded worker; the main
loop drains input before waiting for X transactions and again before composing;
CPU composition borrows source storage, row-copies clipped spans, and computes
its checksum/nonzero count in one pass; and the native renderer reuses one EGL
context and GL pipeline per output. KMS still receives a fully completed GL
frame because the atomic path does not yet provide an explicit native fence.

Schema 9 records the maximum composition, input-dispatch gap, queue depth and
dwell, upload, and persistent-resource counts. The final Kitty dummy rerun
presents input in 40 milliseconds with 8-millisecond CPU composition and
11-millisecond MIT-SHM capture. The stricter QEMU final-key-to-primary-output
measurement is 37 milliseconds. The dual-output QEMU proof
creates exactly two native targets and pipelines with zero recreations, drains
155 page flips without cleanup debt, and confirms that PRIME GEM cleanup treats
the driver's already-closed `EINVAL` result as idempotent success. Degraded
XGetImage remains operational but is rejected for interactive evidence.

The next operator run exposed a keymap mismatch hidden by ordinary typing.
Sophia correctly translated Linux input codes with the evdev `+8` convention,
but device-less dummy XLibre had selected its legacy `xfree86` keycode table.
Letter positions overlap between those tables; navigation positions do not, so
evdev keycode 111 (`Up`) arrived as `Print`. The private server now loads the
evdev XKB rules before launching a client and fails startup unless Up, Left,
Right, and Down resolve at keycodes 111, 113, 114, and 116. Sophia X Authority's
minimal core map now advertises the same navigation keysyms for direct clients.

## 2026-07-14: Engine Topology, Authority XKB, And Resize Quarantine

Milestone 3 now has three explicit boundaries. First, live Engine output records
become a validated, generation-bearing, at-most-16-output snapshot; X setup and
populated RandR CRTC/output/mode replies derive from it without exposing KMS
object identity. Dynamic RandR subscriptions and events remain separate work.

Second, Engine sends physical input as a `RoutedInputRequest` containing its
selected Sophia surface and global/local coordinates. The X frontend resolves
the owning worker, then a dedicated authority thread owns per-seat xkbcommon
state using a bounded explicit RMLVO configuration. `XKEYBOARD` remains
unadvertised until its map/name/state request surface is implemented.

Third, an X resize transaction whose pixels match a pending requested size is
quarantined with its CPU update. Neither can mutate the committed scene while
the old geometry is active. When every requested surface is ready, the staged
geometry and pixels replay together; timeout discards them and retains the last
committed scene. This closes the path that could display a large white drawing
update at the old top-left geometry, but hardware resize promotion still needs
an operator proof and rollback evidence.

## 2026-07-14: Probe-Backed GTK Startup And Real SHM Pixels

The native frontend now advertises the measured XKB and XI2 startup subset,
including XI version/device discovery, client pointer/focus queries, event
selection, and optional device-property reads. Zenity consequently advances
through normal window creation and software drawing with no X protocol error.

That probe exposed the blank-block cause: `ShmPutImage` validated the segment
but discarded its offset and payload, then materialized a zero-filled damage
buffer. A narrow SysV SHM adapter now validates segment size with `IPC_STAT`,
attaches read-only, copies only a bounded image range, and detaches immediately.
The generic pixel-proof policy now records one 310-by-233 committed surface,
288,920 nonzero bytes, and `first_error=none`. This is software-pixel evidence,
not completion of interactive GTK input, the full XI2/grab contract, or
Milestone 3 hardware promotion.

## 2026-07-14: One Session XKB Description And Stable RandR Identity

The native frontend now compiles one immutable xkbcommon snapshot from the
session RMLVO. Core `GetKeyboardMapping`, XKB `GetMap`, and per-seat event
translation consume that configuration instead of combining a handwritten US
wire map with an independently compiled state machine. The live command accepts
bounded `--xkb-rules`, `--xkb-model`, `--xkb-layout`, `--xkb-variant`, and
`--xkb-options` overrides; a German-layout regression proves that core and XKB
views change together.

RandR CRTC and output identities now derive from Engine `OutputId`, while mode
identity derives from the mode tuple. Reordering a topology snapshot therefore
does not renumber an unchanged output. Focus state is also namespace-local and
window destruction resets only its namespace. Dynamic RandR event diffs,
complete XKB state/name notifications, grabs, and XI2 event delivery remain
Milestone 3 work.

The follow-up dynamic path now acknowledges newer Engine snapshots, populates
`GetMonitors`, and sends mask-selected RandR screen, CRTC, output, and resource
notifications through each client's bounded protocol queue. A deterministic
`--inject-output-size=WIDTHxHEIGHT` live-session hook applies a validated
generation update after client startup, so update behavior can be retained as
evidence without requiring a physical connector hotplug.

The live resize rollback fence is now an exported coordinator rather than
private layout bookkeeping. It owns committed sizes, monotonic compensating
transaction IDs, abandoned-size filtering, and disconnect cleanup. Integration
tests cover successful advancement, timeout rollback construction, rejection of
late abandoned pixels until the old size is confirmed, and cleanup while a
rollback is pending. The live layout uses this coordinator for its existing
geometry-plus-pixels quarantine and compensating configure path.

Core input grabs now have connection identity and namespace-scoped authority
state instead of validation-only request handling. Active pointer/keyboard,
passive key/button with Any detail/modifier conflict checks, implicit button,
owner-events routing, synchronous freeze with bounded deferred input and
`AllowEvents`, ungrabs, and namespace-local `GrabServer` ownership all clean up
on disconnect. Engine still chooses the ordinary target surface and local
coordinates; the authority redirects only when X grab semantics require it.
XI2 generic-event delivery remains the next input-compatibility boundary.

That XI2 boundary now advertises XGE 1.0 and XI 2.0, reports master pointer
button/valuator classes plus the master keyboard key class, retains bounded
per-client selection masks, and emits selected Key, Button, Motion,
Enter/Leave, and Focus generic events. Device events preserve Engine-provided
root/local coordinates as FP16.16 values and follow core grab redirection. One
input delivery acknowledgement is returned only after the writer flushes the
core event and every selected XI2 record generated from it. Raw, touch, and
gesture events remain deliberately outside Milestone 3.

## 2026-07-14: XKB State, Names, And Subscriptions

The X authority now implements the generic XKEYBOARD 1.0 state/name path rather
than a toolkit-specific startup exception. `GetState` reports the last
authority-translated effective modifier state, `GetNames` publishes interned
component atoms derived from the configured session RMLVO, and bounded
`SelectEvents` parsing persists each client's StateNotify detail mask. Modifier
transitions emit the standard 32-byte StateNotify record only when the selected
state detail changed. Focus/hierarchy policy and retained classic/confined
session evidence remain separate open gates.

Window input routing no longer treats event-mask update order as focus. The
connection records CreateWindow parent links, mapped state, and ConfigureWindow
sibling/stack modes. Engine-selected target surfaces begin core propagation at
their owning window, ancestor selection is bounded against malformed cycles,
and root focus resolves through the current mapped stacking order. Scene-level
restack acknowledgement remains an Engine integration/evidence gate.

Retained live-session completion is now schema 12. It binds each completion to
its `classic_shared` or `confined` namespace profile and records whether the
deterministic Engine topology update was applied. The paired Milestone 3 runner
executes the same guarded two-xterm proof once per profile; its verifier requires
the confined startup record to have zero request and publish capabilities, both
runs to include an applied output update, and both to satisfy the existing
startup, composition, input-flush, presentation, resize, and cleanup checks.
The output-update acknowledgement now also carries the number of RandR records
queued to live subscribers. Schema 12 retains that count, and promotion rejects
an accepted topology update that reached no X11 client.

The paired runner now also requests a deterministic one-shot X11 surface
resize after both terminal surfaces have published. The live layout sends the
client-targeted ConfigureSurface command, validates the matching control
acknowledgement, and keeps the new geometry quarantined until a transaction
with matching resized pixels arrives. Schema 12 reports `surface_resize` only
after that commit; the promotion verifier requires the configure acknowledgement
and pixels marker in both namespace profiles.

The topology path now opens a dedicated authenticated RandR witness before the
Engine update, uses a reply-producing core request as a subscription barrier,
and reads back the resized ScreenChangeNotify record. This replaced the earlier
timing-dependent assumption that xterm itself would subscribe. The witness is
closed before frontend drain; a two-xterm headless live smoke then completed
with four queued RandR records, a matching wire event, committed resized
pixels, and clean process teardown.

Milestone 3 promotion no longer accepts the synthetic-input default. The paired
runner requires readable physical keyboard and pointer event nodes, exact
physical `sophia` plus Return input, flushed delivery, presented text pixels,
and a pointer-driven pixel change in both profiles. Schema 13 separates
automated terminal-content readiness from total operator interaction time, so
the two-second startup budget measures startup rather than typing speed.

## 2026-07-14: Retained Paired Milestone 3 Session

Fresh X13 runs under classic shared-X and a newly allocated zero-capability
confined namespace passed `tools/verify_live_session_milestone3_evidence.sh`.
Both schema-13 completions retained two live CPU layers, exact physical
`sophia` plus Return delivery, pointer-routed pixel changes, matching accepted
authority/runtime transaction counts, four authenticated RandR notifications,
committed configure-plus-pixels resize, native presentation, and no in-flight
or cleanup-pending KMS state.

Classic completed with 94 ms startup readiness, 13 ms maximum composition,
22/22 routed deliveries flushed, and 0 ms measured input-to-presentation.
Confined completed with 90 ms startup readiness, 13 ms maximum composition,
38/38 routed deliveries flushed, and 0 ms measured input-to-presentation. The
operator-bounded elapsed times include deliberate physical interaction and do
not replace the schema-13 startup metric. The ignored retained logs live at
`.evidence/remote-target/tmp/sophia-milestone3-{classic,confined}.log`.

## 2026-07-14: Milestone 4 Buffer Lifetime Foundation

Milestone 4 now has an explicit reduced buffer boundary. Protocol-visible
DMA-BUF descriptors use opaque buffer and fence identities, admit at most four
planes, accept only bounded XRGB8888/ARGB8888 dimensions and byte ranges, and
contain no native renderer objects or file descriptors. Native plane and acquire
fence FDs enter a renderer-private registry. That registry refuses duplicate or
malformed registrations, blocks submission behind an unsignaled acquire fence,
and releases ownership only after page-flip retirement, rejection, or disconnect.

X software drawing now publishes a new immutable handle for every accepted
generation rather than patching a buffer already visible to Engine. External
tests retain the earlier generation and prove its bytes do not change. A matching
CPU lifetime reducer keeps the last committed handle through stale retirement and
rejection, releases the previous handle only after the replacement page flip,
and drains disconnect ownership exactly once.

MIT-SHM now advertises a real extension event base and encodes the standard
Completion event layout when PutImage requested notification and the request was
accepted. The completion is not emitted for rejected window updates. The
workspace all-feature test gate passes offline on X13. Standard DRI3/Present
SCM_RIGHTS transport, client-visible Present feedback, mixed GPU/CPU composition,
and retained Vulkan hardware evidence remain open; the private SOPHIA-PRESENT
prototype is not promoted by this checkpoint.

## 2026-07-14: Standard DRI3/Present Transport Checkpoint

The X authority now negotiates the standard `DRI3` and `Present` extension
names at version 1.2 without routing them through the private
`SOPHIA-PRESENT` opcode. The Unix request reader uses `recvmsg` for the fixed
X11 header, captures up to four SCM_RIGHTS descriptors with close-on-exec set,
then completes the ordinary request payload read. Descriptor ownership is
RAII-bound and unexpected FD arity terminates the malformed connection instead
of leaking or guessing ownership.

The first admitted standard DMA-BUF request is DRI3 `PixmapFromBuffer`. Its
wire decoder preserves the pixmap/drawable and bounded storage metadata, marks
the request as requiring exactly one FD, accepts only 32-bpp XRGB8888/ARGB8888
shapes whose stride and declared storage cover the image, and records only the
authority-owned pixmap identity. The native FD remains borrowed through the
socket trace seam for renderer-side duplication and never enters authority
runtime state. DRI3 fences, PresentPixmap/events, and the live renderer handoff
remain the next transport checkpoint.

## 2026-07-15: DRI3 1.2 Vulkan Transport Proof

The X11 socket output boundary now sends a bounded byte record plus up to four
SCM_RIGHTS descriptors. Standard DRI3 `Open` obtains a duplicated render-device
FD only from the live backend provider and returns it in a one-FD reply; neither
the authority runtime nor Engine stores a device path or native handle.

Mesa's DRI3 1.2 startup required `GetSupportedModifiers`, modifier-bearing
`PixmapFromBuffers`, and the small XFIXES region lifecycle used by Present. The
portable modifier reply advertises linear plus the implicit-modifier sentinel,
and the multi-buffer decoder retains bounded plane strides, offsets, and the
wire modifier in the reduced DMA-BUF descriptor.

The first Vulkan failures were caused by Unix-stream FD association rather than
an AMD modifier. A single `sendmsg` can attach descriptors to bytes preceding
the X11 request that consumes them. The server now queues ancillary FDs in
stream order, leaves them pending across no-FD requests, and drains exactly the
declared arity for each later FD-bearing request. A deterministic regression
sends two descriptors alongside an earlier no-FD XFIXES request and proves that
the following DRI3 pixmap and fence requests consume one each.

On the Void Linux X13 with Mesa RADV, the bounded DRI3 1.2 `vkcube` run remained
healthy for its eight-second proof window: 68 requests, three imported pixmaps
and fences, one accepted standard Present transaction, one committed runtime
surface, and `first_error=none`. This proves Vulkan transport into the Engine
transaction seam; it does not yet claim native KMS presentation of the Vulkan
pixels.

## 2026-07-15: Reusable Renderer-Private DMA-BUF Sources

The renderer lifetime boundary now distinguishes a persistent DRI3 pixmap
source from one in-flight presentation. Plane FDs remain renderer-private and
reusable across Presents, while every presentation receives duplicated plane
and acquire-fence ownership in the existing bounded registry. Page-flip
retirement removes only the in-flight ownership; explicit source removal or
disconnect releases each persistent source once.

External tests use a real xshmfence to prove that an unsignaled acquire fence
holds submission, a trigger makes the presentation ready, page-flip retirement
allows the same source to be presented again, an in-use source cannot be
removed, and disconnect cleanup is idempotent. The complete offline all-feature
workspace suite passes with this reusable lifetime model. Live-session import,
mixed CPU/GPU composition, and page-flip-driven Present feedback remain open.
The X frontend also exposes a cloneable protocol-only feedback router that can
emit Present Complete and Idle after the broker moves into its service thread.
It is intentionally not attached to the current CPU fallback submission: doing
so would acknowledge a page flip that did not contain the imported Vulkan
pixels.

## 2026-07-15: Milestone 4 Live-Presentation Handoff

Commit `11f93ee` leaves Milestone 4 at the boundary between proven protocol
transport and unimplemented native GPU presentation. The frontend publishes
DMA-BUF registrations, fence registrations, and Present submissions through
`XAuthorityObservedTransactionBatch`. `LiveDmaBufPresentationRegistry` owns the
reusable source and per-Present FD model, and
`XServerFrontendProtocolRouter` owns protocol-only completion delivery. No
persistent-session consumer currently connects those pieces, so the bounded
`vkcube` result remains Engine-transaction evidence rather than proof that its
Vulkan pixels reached KMS.

The current live-session assembly is also an explicit architecture debt.
`PersistentNativeScanout` and `PersistentCpuScene` remain in the CLI command;
the latter retains a CPU-only `SurfaceId` projection outside the normative
Engine scene owner. Moving the entire session loop before proving GPU
presentation would broaden the active milestone, while wiring more durable
scene and renderer authority directly into the CLI would deepen the debt.

The chosen continuation is a narrow hybrid extraction. Establish an
Engine/backend-owned live-presentation seam, then move only DMA-BUF import,
acquire-fence polling, mixed CPU/GPU composition, KMS submission correlation,
and page-flip retirement through it. Source and fence FDs transfer immediately
into renderer-private ownership. Engine preserves the last committed
geometry-plus-pixels state while a presentation is pending or rejected. Only a
real page flip containing the imported pixels may route Present Complete, then
Idle, trigger the idle fence, and retire the presentation exactly once. Broader
CLI session-loop extraction and Milestone 5 compatibility work remain deferred
until the software-plus-`vkcube` native KMS matrix passes.

## 2026-07-15: Milestone 4 Mixed-Presentation Implementation

The narrow handoff is now implemented without moving protocol or native object
ownership into Engine. The X frontend assigns typed buffer/fence handles and
routes feedback by exact `TransactionId`. `LivePresentationResourceSession`
immediately duplicates frontend registrations into renderer-private ownership,
polls xshmfences, builds mixed CPU/DMA-BUF frames, and retains reusable DRI3
sources separately from individual Present lifetimes. The native EGL path
supports one-to-four-plane EGLImages, clipped placement, alpha blending, and a
single persistent output composition pass.

Engine now exposes a prepared surface commit for asynchronous presentation.
Preparation does not mutate committed state. Page-flip application revalidates
only surfaces touched by the prepared transaction, which prevents stale GPU
callbacks from overwriting a newer version of the same surface while allowing
unrelated CPU surfaces to continue committing. Rejection and disconnect drop
the candidate. Successful native feedback applies the candidate, routes Present
Complete with Flip mode, retires the renderer presentation and idle fence, then
routes Idle. Teardown converts remaining queued work to Skip/Idle and asserts
that no source, fence, presentation, or cleanup debt remains.

The offline all-feature workspace suite passes, including prepared-commit
merge/stale regressions, real xshmfence wait/trigger tests, repeated-pixmap and
deferred-release tests, mixed-frame backend ownership, multi-plane renderer
validation, and exact transaction routing. The schema-14 session evidence adds
mixed-export, acquire-wait, completion, idle-fence, and live-resource counters.
`tools/live_session_milestone4_hardware_proof.sh` pairs the established software
resize proof with a `vkcube`/CPU mixed session, controlled first acquire delay,
one rejected Present, required later Flip recovery, and strict teardown checks.
Its verifier passes positive and missing-mixed-export fixtures. The exclusive
TTY X13 run is deliberately still unclaimed and is the remaining Milestone 4
exit action.

## 2026-07-15: Milestone 4 Hardware Checkpoint And AMDGPU Mixed-Draw Blocker

The paired X13 gate now proves the software half after a real renderer defect
was isolated. Reusing mixed-composition GL state for the legacy full-screen CPU
upload eventually lost the AMD context. The persistent upload path now restores
its fixed full-screen quad and completes independently; the retained software
run committed the 800x600 configure-plus-pixels resize, flushed all 14 semantic
input events, reported exact text and changed pixels, submitted 36 native
frames, retired 35, and drained with no failure or cleanup debt.

The Vulkan attempt also exposed a transaction-domain mismatch. Present request
generations continue across controlled Skip, while Engine visual generations
advance only on accepted commits. Full-state Present snapshots are now rebased
to the current Engine committed generation immediately before preparation;
external regressions cover the empty baseline and a later post-Skip baseline.
This removed the stale-candidate flood and allowed the real imported image to
reach the renderer.

The remaining failure is specifically the required two-layer native EGL draw.
With the CPU background removed only for diagnosis, the same real `vkcube`
session completed 86 mixed exports and Flip completions, one controlled Skip,
87 matching Idle events and idle-fence triggers, 121 native submissions, 120
retirements, and zero live sources, fences, transactions, or KMS cleanup debt.
Restoring the CPU layer aborts Sophia inside Radeon `glFinish` with
`amdgpu: The CS has been rejected, see dmesg for more information (-2)` before
the first mixed KMS submission.

The failure survived `RADV_DEBUG=nodcc`, explicit CPU/import completion
boundaries, frame-local CPU textures, frame-local vertex buffers, and a
diagnostic layer-order reversal. Those experiments were removed; the retained
implementation keeps only the proven full-screen upload isolation, Present
generation rebase, and EGLImage sampling lifetime order. The next session
should capture the privileged kernel validator message immediately after one
failure, then reduce CPU-texture-plus-imported-image composition to a focused
native-EGL hardware regression before changing more session code. Retained
ignored evidence is under
`.evidence/remote-target/tmp/sophia-milestone4/` and
`.evidence/remote-target/tmp/sophia-milestone4-dmabuf-only/`; neither GPU log
is promotion evidence until the normal paired verifier passes.

## 2026-07-15: Milestone 4 Native-EGL Reduction

The remaining mixed-draw failure now has a bounded reproduction below KMS.
`native-egl-vkcube-mixed-smoke` launches the real native-X `vkcube` transport,
transfers its DRI3 planes through `LivePresentationResourceSession`, combines
them with the full-output CPU background, and invokes the persistent mixed
exporter directly. A watchdog parent reports child exit or timeout, while the
successful child emits schema-1 evidence only after disconnect drains all live
sources, fences, and presentations. Fixture-backed verification rejects a
missing CPU layer or cleanup debt.

Native composition now reports CPU upload, EGLImage create/bind, draw, finish,
and destroy failures separately. The mixed CPU background uses dedicated,
fixed-size texture storage and sub-image updates rather than reallocating the
fullscreen upload texture in the command stream that samples the imported
EGLImage. This is locally covered and the full offline all-feature suite passes,
but it is not hardware promotion evidence until the focused X13 smoke and the
normal schema-14 paired proof both pass. The paired wrapper now retains
privileged before/after kernel logs and driver environment identity so another
AMDGPU rejection cannot lose its validator record.

## 2026-07-15: Milestone 4 Mixed-Presentation Hardware Exit

The isolated mixed exporter passed while the persistent session aborted in the
Radeon command-submission thread. Lifecycle tracing and a GDB backtrace showed
that one CPU-plus-DMA-BUF frame drew, swapped, submitted, page-flipped, and
retired successfully; the asynchronous rejection surfaced in the following
ordinary CPU upload. Reusing the GL context across that imported-image boundary
was the distinguishing lifetime.

The native renderer now destroys the mixed frame's GL context after
`glFinish`, swap, and front-buffer lock. The returned GBM owner retains the
scanout surface independently through KMS submission and page-flip retirement;
the following CPU frame receives a fresh context. DRI3 `Open` also returns an
independently opened same-GPU render node instead of the compositor's primary
KMS node, preserving the protocol/backend ownership boundary.

The retained X13 schema-14 proof passed its strict verifier with 76 mixed Flip
completions, one controlled Skip, 77 matching Idle events and idle-fence
triggers, nine acquire-gate waits, zero submit/retire failures, and zero live
sources, fences, or transactions. The established software xterm/resize half
also passes, completing the Milestone 4 hardware exit.

## 2026-07-16: GTK3 Application Promotion Contract

Milestone 5 uses a direct, bounded Sophia-X client launcher instead of wrapping
applications in xterm. Application evidence correlates the existing live
session record with `sophia_x_application_session schema=1`: exact bounded
stdout, normal exit, reduced zero-error protocol observations, physical text
and pointer-button delivery, resize/redraw, native presentation, and clean
teardown are all mandatory. Error-only X dispatches cross the frontend boundary
as reduced code/opcode/sequence facts but never create empty Engine commits.
Zenity entry dialogs run under fresh classic shared-X and confined sessions;
the operator types without Return and completes the action with a physical OK
click.

## 2026-07-16: Zenity Probe-Driven RandR And XFixes Gaps

The current GTK3 Zenity engine probe exposed two bounded requests after its
package became available locally: RandR `GetOutputProperty` for EDID and XFixes
`SelectSelectionInput`. Sophia now returns a valid empty output-property reply
when no EDID payload is retained and validates the selection window, atom, and
three-bit event mask. The same probe showed that advertising DRI3 without a
render-device provider creates an avoidable `BadImplementation`; socket
advertisement now withholds DRI3 in that configuration so GTK selects MIT-SHM.
The repeated probe commits one surface with 288,920 nonzero software bytes and
`first_error=none`; no broader RandR property store or XFixes event expansion
was inferred.

## 2026-07-16: Sophia X TTY Recovery Is An Acceptance Gate

The first GTK hardware attempt could leave the active text VT black until a
power cycle because the X proof called the raw persistent KMS runner without
the guarded TTY lifecycle already used by native Wayland. The GTK runner now
builds and preflights before takeover, requires an independent
Ctrl-Alt-Backspace guard to arm, saves KD and termios state, runs each Sophia
session in a bounded process group, restores keyd and the console on every exit
path, and records a strict durable recovery line.

The isolated QEMU emergency gate then exposed five modifier deliveries queued
before the final Backspace trigger. Emergency completion now waits for those
deliveries to flush before frontend teardown. The repeated gate proves guard
arm/trigger, exact five-of-five settlement, clean two-head KMS retirement, zero
native cleanup debt, and clean guest shutdown.

## 2026-07-16: Milestone 5 Native Zenity Blocker Retained

Two guarded X13 classic shared-X attempts reached one Engine-owned native KMS
output and then showed a blank screen because Zenity aborted before presenting
a dialog. Both retained logs report GTK thaw-update assertions followed by
`BadRequest` at serial 304, request code 139 (`XFIXES`), minor code 0
(`QueryVersion`). The confined profile never started. The second emergency
chord restored KD mode 0, the exact termios state, keyd, and all Sophia
processes; the recovery record is complete. The earlier false keyd failure was
a service-start race, so the runner now waits boundedly for keyd after `sv up`.

The retained diagnosis was incomplete because wire-parse errors discarded the extension minor opcode and always encoded minor zero. A raw-minor trace on the X13 render-provider path reproduced XFixes request 11 (`SetRegion`) immediately after `CreateRegion`; `QueryVersion` had already succeeded. Sophia now retains extension minor codes, owns namespace-scoped XFixes region lifecycle, validates Present region references, and reclaims regions with the client resource range.

The first corrected run exposed a separate sentinel bug: raw region zero was converted with generation one and compared structurally to the generation-zero `NONE` value. Validity-based optional-resource checks fixed that rejection. The exact X13 sequence now accepts CreateRegion, SetRegion, DRI3 pixmap and fence resources, and Present with `first_error=none`. The non-KMS render-provider smoke reaches an Engine transaction but has no scanout consumer, so its remaining pixel-proof failure is expected and is not session evidence. Fresh guarded classic and confined hardware captures remain required before GTK promotion.

## 2026-07-17: GTK Input Stall Split From Scanout Throughput

The latest guarded X13 classic run presented the Zenity entry dialog but
accepted only five physical key presses before input stopped. The retained
15-second interval contained 984 X requests, including 252 outputless requests,
62 MIT-SHM PutImage requests, 31 CPU compositions, and 30 native submissions.
That showed both avoidable redraw work and socket-output lock contention, but
not a KMS deadlock: presentation continued while keyboard progress stopped.

Physical libinput collection now runs on a bounded worker instead of the
authority loop. Outputless X requests skip the shared output-stream lock,
software-only authority batches may coalesce their CPU composition while every
Engine transaction is still applied in order, and cursor-only movement produces
a composed native frame. During the pointer acceptance phase, physical Return
press and release are suppressed and reported instead of aborting the session.
Raw X request tracing and native lifecycle tracing are no longer enabled by the
normal GTK hardware runner; `SOPHIA_M5_GTK_DIAGNOSTIC=1` opts into both.

A bounded local Zenity entry proof then routed and flushed all fourteen
synthetic press/release events for `sophia` plus Return. GTK continued issuing
geometry, property, and SHM redraw requests but never exited or produced the
expected stdout before the semantic timeout. The throughput and lock fixes are
therefore retained, while GTK entry submission remains an explicit Milestone 5 compatibility gap.

## 2026-07-17: Unattended GTK Input Acceptance In QEMU

A direct-kernel, diskless, networkless QEMU guest now runs the real Zenity
entry dialog under both `classic_shared` and `confined` namespace profiles.
The host harness uses QMP only to drive virtio keyboard and mouse devices; the
guest receives those events through the normal physical-input poller. Both
profiles type exact `sophia`, observe changed pixels, route a physical OK-button
click, match Zenity stdout, exit normally, and cleanly retire both virtio-gpu
outputs with `protocol_errors=0`.

The trace-driven compatibility slices added core ChangeGC and CreateCursor,
XIChangeCursor, bounded opaque non-input SendEvent delivery, XIUngrabDevice,
and a protocol-shaped XIQueryPointer reply. It also exposed a proof-loop bug:
Return suppression was scoped to the entire pointer-proof run rather than only
the pre-selection phase. Suppression now ends when pointer selection becomes
ready, and an application proof cannot complete before its primary child exits.
The QEMU result closes the deterministic semantic gap; guarded target-hardware
classic/confined captures with resize remain the promotion gate.


## 2026-07-17: Presented Cursor Gate And Production-Loop Review

The final GTK QEMU regression corrected the earlier pointer-proof conclusion.
Pointer readiness now follows a centered cursor composition and matching native
presentation. Return remains suppressed until a physical pointer button routes,
not merely until pointer readiness. If the application proof surface disappears
before selection, the session exits through bounded cleanup. Both classic and
confined Zenity guests pass the click-then-submit sequence with normal exit and
clean two-output retirement. Fresh paired X13 evidence remains required.

A concurrent architecture review found that the authority boundaries are
implemented, but production orchestration remains duplicated. The 6,500-line
live-session command retains `PersistentCpuScene`, `PersistentBackendRuntime`,
and `PersistentNativeScanout` while Engine, runtime, and backend crates each
carry partial loop abstractions. The next architecture milestone after GTK
promotion is one protocol-neutral coordinator in
`sophia-engine::runtime_driver`: bounded authority intake, Engine
commit/preparation, composition from committed state, backend-private KMS
submission/retirement, then exact protocol feedback. CLI proof logic remains
an observer and supervisor rather than a visual-state owner.


## 2026-07-17: GTK Submit Deadlock Removed

A fresh local QEMU run reproduced the apparent post-submit blank screen after
Zenity had accepted exact physical text and pointer input. The client process
exited, but the CLI session loop synchronously called `read_to_end` on its
piped stdout. An inherited writer could therefore block the visual coordinator
forever, bypassing the 30-second session deadline and preventing native
retirement and console recovery. Application stdout now targets a private
mode-0700 capture directory and mode-0600 file. Once the child exits, the loop
reads at most 4,097 bytes from the regular file without waiting for every
inherited descriptor to close; a regression keeps a writer open while proving
the bounded read completes.

The rebuilt X13-hosted QEMU image passed both GTK profiles. Classic completed
in 4,617 ms and confined in 4,633 ms; both matched `sophia\n`, routed a
physical pointer button, reported `first_error=none`, retired both virtio-gpu
outputs, and ended with zero native cleanup debt. The initramfs builder also
requires xterm explicitly now: it can no longer silently produce a nominally
successful image whose default session scenario fails at boot-time readiness.
The guarded physical X13 resize captures remain the Milestone 5 promotion gate.


## 2026-07-17: Input Delivery Settlement Restores Bounded Sessions

The default xterm QEMU gate exposed a second post-input stall after keyboard and
pointer evidence had already succeeded. Phase tracing proved cursor composition,
KMS submission, and page-flip retirement all returned. The loop instead kept
`input_delivery_wait_started_at` populated after the exact key deliveries
settled. Because ordinary proof sessions advance `--max-ticks` only outside an
active delivery wait, a successful input proof made the session immortal; GTK
was unaffected only because its application-specific proof exits immediately.

Delivery settlement now consumes the wait timestamp exactly once. Later pointer
or emergency batches start their own bounded delivery wait and clear it after
settlement, while the initial key-flush record remains tied to the complete
14-event sequence. A regression covers the consume-once transition. The QEMU
verifier now recognizes current schema 14 and validates either native CPU export
mode: zero GL resources for the preferred direct linear GBM write, or exactly
one reusable GL target/pipeline per output for the fallback. Mixed counters,
recreation, missing uploads, latency violations, and cleanup debt still fail.

The rebuilt X13-hosted image passed every unattended profile. The strict
two-xterm run completed 300 ticks in 6,971 ms with two CPU layers, 8 ms input
presentation, 11 ms maximum composition, 40 submissions, 38 retirements, and
zero cleanup debt. Classic and confined GTK completed normally with exact
stdout, `first_error=none`, pointer selection, and clean two-output retirement.
The emergency profile armed and triggered Ctrl-Alt-Backspace, flushed all five
routed deliveries, and shut down cleanly in 187 ms.


## 2026-07-17: Backend Snapshot Ownership Moves Into Production Coordinator

`HeadlessCompositorBackendAssembly` no longer stores an independent
`Vec<CommittedSurfaceState>`. It owns a `ProductionSessionCoordinator`, and the
existing deterministic and live runtime adapters now receive and return the
coordinator-owned snapshot through one split Engine/state borrow. Public
`with_committed_surfaces`, replacement, input routing, rendering, and runtime
reports retain their behavior, but the live backend has one fewer visual-state
owner before the remaining CLI scene and native sequencing migration.

The focused Engine, all-feature live-backend, and live CLI suites pass. A rebuilt
X13 QEMU image also passed the strict two-xterm gate with two CPU layers, exact
keyboard and pointer routing, 8 ms input presentation, 40 submissions, 38
retirements, and zero cleanup debt. The confined GTK gate passed normal Zenity
exit, exact stdout, `first_error=none`, clean two-output retirement, and zero
native debt. This is an ownership migration, not the Milestone 6 exit: the
legacy runtime adapter still sequences commits and the CLI still owns
`PersistentCpuScene` and `PersistentNativeScanout`.


## 2026-07-17: CPU Pixel Storage Leaves The CLI

Renderer-live now owns a protocol-neutral `LiveCpuBufferRegistry`. It accepts
immutable replacements and packed damage patches, rejects stale generations,
missing bases, metadata changes, invalid bounds, and malformed byte lengths,
and retires unreferenced handles. The X frontend remains responsible for
read-only MIT-SHM admission and emits its existing immutable updates; the CLI
only converts those packets at the renderer boundary. `PersistentCpuScene` no
longer contains a CPU buffer map or applies pixel patches itself.

Four focused registry regressions cover replacement/patch ordering, stale
generation rejection, fail-closed malformed replacement and patch behavior,
and resource retention. The live CLI suite passes. On the rebuilt X13-hosted
image, strict two-xterm QEMU completed 300 ticks with two CPU layers, 7 ms input
presentation, 40 submissions, 38 retirements, and zero cleanup debt. Confined
GTK passed its high-volume SHM redraw path, exact text/pointer proof, normal
exit, `first_error=none`, and clean two-output retirement. The remaining
Milestone 6 scene gap is narrower but explicit: CLI still projects a
`SurfaceId` to geometry/handle table because commit and composition have not yet
been split into coordinator phases.


## 2026-07-17: Authority Commits Once Before Per-Output Projection

The persistent live backend no longer creates one authority inbox per output or
replays the same X transaction through every output Engine. The production
coordinator exposes a bounded commit phase; the primary runtime commits each
batch once, then every output consumes the same immutable committed snapshot and
the same precomputed commit observations. The late-client generation bridge now
runs once at this boundary, before the single commit, rather than priming every
output ahead of replay. A two-output regression proves both output assemblies
end on generation 6 while the runtime records one committed transaction.

The full offline all-feature suite passes. The rebuilt X13-hosted QEMU image
passed the strict two-xterm profile in 7,104 ms with 114 of 114 authority
transactions applied, two CPU layers, 7 ms input presentation, 4 ms maximum
composition, 40 submissions, 38 retirements, and zero cleanup debt. An initial
run without the centralized late-discovery bridge correctly rejected the second
xterm generations as stale; retaining that fail-closed evidence drove the fix.
Confined GTK then passed 54 SHM transactions, exact text and pointer evidence,
normal exit, `first_error=none`, 108 submissions, 106 retirements, and clean
resource shutdown. Classic GTK passed the same application contract, and the
emergency profile flushed all five routed chord deliveries before clean shutdown
in 178 ms. Composition still uses the CLI scene table and runs before
this commit phase; the next slice must compose from the coordinator snapshot.


## 2026-07-18: CPU Composition Consumes Engine Committed State

The persistent CPU path now splits authority preparation from per-output runtime
ticks. Each batch commits once, renderer pixel updates are reconciled against the
resulting immutable `CommittedSurfaceState` slice, and composition resolves
geometry, buffer handles, stacking, readiness, and proof generations from that
slice before any KMS submission. `PersistentCpuScene` retains only the renderer
buffer registry and composition evidence; its independent `SurfaceId` table and
raised-surface state are deleted. Native runtime construction also no longer
requires a pre-commit frame or a blank modeset: KMS initializes with the first
frame composed from committed state.

The full offline all-feature suite passes. A rebuilt X13-hosted QEMU image passed
the strict two-xterm 300-tick profile in 6,824 ms with 123 of 123 authority
transactions applied, two CPU layers, 8 ms input presentation, 2 ms maximum
composition, 40 submissions, 38 retirements, and zero cleanup debt. Confined GTK
passed 56 committed SHM transactions, exact text and pointer evidence, normal
exit, `first_error=none`, 107 submissions, 105 retirements, and clean shutdown.
Classic GTK also passed the final image with exact application evidence and clean
retirement. The emergency profile flushed all five routed chord deliveries and
shut down without native debt in 151 ms. The duplicate
scene milestone item is complete, while CLI ownership of runtime/scanout and
feedback sequencing remains open.


## 2026-07-18: Production Feedback Waits For Asynchronous Retirement

The initial production adapter incorrectly modeled KMS submission and retirement
as one synchronous callback. That shape could not own the real live path without
either blocking a cycle or treating submission as page-flip completion. The
contract now has separate `submit_frame` and `poll_retirements` phases. A
`ProductionRetirement` carries its originating cycle, and protocol feedback is
routed only for records returned by the retirement poll. Submission evidence and
zero or more feedback records remain distinct in each cycle report.

Engine regressions cover ordered immediate retirement, retirement-poll failure
with no feedback, feedback failure after retirement, and a frame held across one
cycle then retired on a later poll. The live closure adapter exposes the same four
callbacks. The full offline all-feature suite passes. This establishes the
correct state-machine seam for moving `PersistentNativeScanout` and Present
Complete/Idle timing out of the CLI; the live path is not yet wired through it,
so the Milestone 6 coordinator and sequencing items remain open.


## 2026-07-18: Backend Owns Page-Flip Retirement Correlation

`PersistentNativeScanout` no longer owns an Engine presentation registry, a
per-head scheduled frame slot, or a reduced UST/MSC feedback queue. The
protocol-neutral `LiveProductionPageFlipTracker` in backend-live now schedules
each submitted output against a production cycle, rejects overlap, validates
monotonic page-flip sequence and timestamp evidence, retires the exact scheduled
frame, and emits a `ProductionRetirement<LiveProductionPageFlipRetirement>` only
after all those gates succeed. Per-output take and discard operations preserve
the existing CPU-frame versus Present-frame separation without exposing backend
state to the frontend.

Regressions prove no retirement exists at submit time, a matching accepted flip
retains the originating cycle and reduced UST/MSC, overlap fails closed, and
non-monotonic callbacks produce no retirement. The full offline all-feature
suite passes. On the rebuilt X13-hosted QEMU image, strict two-xterm completed
300 ticks in 6,970 ms with 120 of 120 transactions applied, 8 ms input
presentation, 40 submissions, 38 retirements, and zero phase or cleanup debt.
Confined GTK passed 57 SHM transactions, exact text and pointer evidence, normal
exit, `first_error=none`, 113 submissions, 111 retirements, and clean shutdown.
Present protocol routing still resides in the CLI and is the next ownership seam.


## 2026-07-18: Present Complete And Idle Follow Backend Resource Retirement

`LiveProductionPresentFeedbackCoordinator` now owns the presentation resource
session and produces paired, protocol-neutral Complete/Idle outcomes only after
the matching page flip or controlled rejection retires the live presentation.
Missing or already-retired transactions return an explicit error and emit no
feedback. The live CLI translates the reduced Flip/Skip mode to X wire events
and observes counters, but no longer orders resource retirement, Complete, Idle,
or idle-fence accounting independently. Diagnostic abort still tears resources
down without falsely producing client feedback.

Tests prove a client-released DMA-BUF source and presentation retire before the
Flip/Idle outcome, a second completion fails closed, and an unknown Skip emits
nothing. The full offline all-feature suite passes. The guarded X13 native EGL
vkcube diagnostic passed with one CPU layer, one DMA-BUF layer, and zero live
sources, fences, or transactions. A rebuilt strict two-xterm QEMU image completed
300 ticks in 6,919 ms with 123 of 123 transactions applied, 7 ms input
presentation, 40 submissions, 38 retirements, and zero phase or cleanup debt.
The full real-KMS Milestone 4 proof could not run unattended because X13 sudo
requested a password before any modeset; it remains a later interactive gate.
Prepared Engine commit application and runtime/scanout invocation are still in
`live_session.rs` and remain the next production ownership migration.


## 2026-07-18: GPU Present Uses One Engine Snapshot Across Outputs

The live Present path no longer prepares and applies the same GPU transaction once per
output assembly. It prepares against the primary production coordinator snapshot, applies
that prepared commit exactly once after the matching page flip, and projects the resulting
immutable committed snapshot to the remaining outputs. This removes a multi-output visual
authority fork while preserving per-output scanout state.

A focused coordinator regression proves that applying a prepared Present mutates the
coordinator-owned snapshot, and the full offline all-feature suite passes. The rebuilt X13
QEMU image passed strict two-xterm in 6,907 ms with 120 of 120 authority transactions,
5 ms input presentation, 38 submissions, 36 retirements, and zero phase or cleanup debt.
Classic and confined GTK passed exact physical text and pointer selection, normal Zenity
exit, `first_error=none`, and clean two-output retirement. Emergency recovery flushed all
five routed chord deliveries and shut down cleanly in 187 ms. Runtime/scanout
invocation and the retirement-to-commit trigger still reside in `live_session.rs`; moving
that sequencing behind the production adapter remains the next Milestone 6 boundary.


## 2026-07-18: Coordinator Completes Retired Present Atomically

A matched GPU page flip now enters one `ProductionSessionCoordinator` operation that
applies the prepared Engine commit, captures the resulting immutable snapshot, retires
the backend Present resources, and produces the reduced Complete/Idle outcome. The CLI
requests that operation and translates its outcome, but no longer orders Engine commit
and backend feedback retirement itself. If the prepared baseline is stale, the coordinator
preserves the current snapshot and never invokes the feedback retirement callback.

Regressions prove commit-before-feedback on success and zero feedback calls for a stale
baseline. The full offline all-feature suite passes, the X13 release build succeeds, and
the guarded native EGL/vkcube diagnostic exports one CPU plus one DMA-BUF layer with zero
live sources, fences, or transactions afterward. The retained two-xterm, GTK classic, GTK
confined, and emergency QEMU gates already passed the immediately preceding snapshot; the
remaining production-loop gap is ownership of live runtime/scanout invocation itself.


## 2026-07-18: One Session Coordinator Owns Visual State

`PersistentBackendRuntime` now owns one session-level `ProductionSessionCoordinator`.
Authority commits, Present preparation, retired Present completion, and public committed
state all use that owner. Per-output backend assemblies receive immutable snapshot
projections for rendering and scanout; they are no longer selected as a primary authority.
A regression deliberately changes the first output projection to generation 99, then
proves the session coordinator independently commits generation 5 to 6 exactly once and
overwrites both output projections with its result.

The full offline all-feature suite passes. On the rebuilt X13 QEMU image, strict two-xterm
completed 300 ticks in 7,013 ms with 117 of 117 authority transactions, 7 ms input
presentation, 42 submissions, 40 retirements, and zero phase or cleanup debt. Confined GTK
committed 58 SHM transactions, accepted exact physical text and pointer selection, exited
normally with `first_error=none`, and retired both outputs cleanly. Live runtime and native
scanout method invocation still need to move behind the production live adapter before the
Milestone 6 coordinator item can close.


## 2026-07-18: Production Output Fanout Owns Runtime And Scanout Order

Engine now defines a protocol-neutral `ProductionOutputRuntimeAdapter`, and backend-live
provides its bounded callback implementation. The session coordinator projects its single
committed snapshot and enumerates outputs. Steady CPU ticks, committed-snapshot ticks, GPU
Present submission, native idle submission, page-flip retirement and cleanup, and displayed
buffer teardown all enter through that fanout. During the audit, retired Present completion
was also corrected to use the session coordinator directly and to project its result to
every output; it no longer mutates the former primary output coordinator.

Engine and backend regressions prove one snapshot reaches every output and projection plus
runtime invocation remain one adapter callback. The full offline all-feature suite passes.
The rebuilt X13 QEMU image passed strict two-xterm in 6,897 ms with 120 of 120 transactions,
7 ms input presentation, 40 submissions, 38 retirements, and zero phase or cleanup debt.
Classic and confined GTK accepted exact physical text and pointer selection, exited normally
with `first_error=none`, and cleanly retired both outputs. Emergency recovery flushed all
five routed chord deliveries and shut down cleanly in 189 ms. The concrete closures still
live beside `PersistentNativeScanout` in `live_session.rs`; extracting that implementation
into backend-live is the remaining runtime/scanout ownership step.


## 2026-07-18: Native Scanout Ownership Leaves The CLI

The concrete native output owner is now backend-live's `LiveProductionNativeScanout`.
It owns real atomic card/session groups, per-head GBM exporters, native callback queues,
page-flip correlation, submission and retirement counters, mixed-frame export, and the
production composed-frame record. The implementation is feature-gated in its own backend
domain module. `live_session.rs` no longer defines or directly names real atomic sessions,
GBM exporter types, or the page-flip tracker, and shrank by roughly 570 lines. The existing
opt-in native lifecycle diagnostic is preserved verbatim at the backend boundary.

Default and all-feature backend builds pass, as does the full offline all-feature suite.
The guarded X13 native EGL/vkcube diagnostic exported one CPU and one DMA-BUF layer with
zero live resources afterward. The rebuilt QEMU image passed strict two-xterm in 6,973 ms
with 120 of 120 transactions, 3 ms input presentation, 38 submissions, 36 retirements, and
zero phase or cleanup debt. Classic and confined GTK passed exact physical text/pointer
selection, normal exit, `first_error=none`, and clean two-output retirement. Emergency
recovery flushed all five routed chord deliveries and shut down cleanly in 164 ms. The
remaining Milestone 6 ownership gap is `PersistentBackendRuntime` plus the CPU composition
callbacks still implemented in the CLI.


## 2026-07-18: CPU Composition State Moves Into Renderer-Live

Renderer-live now owns `LiveProductionCpuScene`: the CPU buffer registry, immutable update
admission, committed-surface handle retention, focus-aware stacking, cursor composition,
focused-surface visual-detail inspection, composition evidence, and per-output composed
frame creation. The X boundary only converts authority SHM records into neutral
`LiveCpuBufferUpdate` values and observes reduced reports. Backend-live also owns composed
frame records and the high-level page-flip cleanup/retry and displayed-output release
operations, so CLI callbacks no longer invoke low-level native cleanup APIs or lifecycle
logging. The protocol-neutral authority, renderer, scanout, and feedback adapter roadmap
item is complete.

The full offline all-feature suite passes. On the rebuilt X13 QEMU image, strict two-xterm
completed in 6,995 ms with 117 of 117 transactions, 7 ms input presentation, 40 submissions,
38 retirements, and zero phase or cleanup debt. The guarded native mixed diagnostic exported
one CPU and one DMA-BUF layer with zero live resources. Classic and confined GTK accepted
exact physical text and pointer selection, exited normally with `first_error=none`, and
retired both outputs cleanly. Emergency recovery flushed all five routed chord deliveries
and shut down cleanly in 162 ms. `PersistentBackendRuntime` remains the last large CLI
visual-control wrapper; its X routing and proof observations must be separated from the
protocol-neutral production state machine before the Milestone 6 ownership item can close.


## 2026-07-18: CPU And Present Batches Enter Production Runtime Cycles

CPU authority batches now enter `ProductionSessionCoordinator::run_cycle`, which owns the
commit, immutable-snapshot composition, output submission, retirement poll, and feedback
order. The X session loop supplies translated authority updates and observes only the reduced
submission report. Present batches now likewise cross a single runtime-owned GPU production
entry point: CPU-background composition, per-output frame creation, native initialization,
and Present scheduling no longer occur in the outer CLI loop. Present remains asynchronous;
its prepared Engine state is committed only after the matching page flip through the existing
coordinator retirement gate.

The full offline all-feature suite passes. The rebuilt X13 target passed the guarded mixed
CPU-plus-DMA-BUF diagnostic with zero live sources, fences, or transactions. Its QEMU image
passed strict two-xterm in 6,966 ms with 117 of 117 authority transactions, 8 ms input
presentation, 40 submissions, 38 retirements, and zero cleanup debt. Classic and confined
GTK accepted exact physical text and pointer selection, exited normally with
`first_error=none`, and retired both outputs cleanly. Emergency recovery flushed all five
routed chord deliveries and shut down cleanly in 161 ms. The remaining Milestone 6 boundary
is structural: extract the runtime-owned visual control object from the X router, proof
counters, and process supervision, then delete the legacy committed-snapshot entry points.


## 2026-07-18: X Present Routing Leaves The Visual Runtime

The production visual runtime no longer stores an `XServerFrontendProtocolRouter` or any X
Present completion, idle, fence, or disconnect proof counters. It emits reduced
`LivePresentFeedbackOutcome` values through an injected protocol-neutral sink. The separate
`XPresentSessionObserver` translates those records to X wire events and owns all session-proof
accounting; shutdown consumes the renderer disconnect report outside visual control. A direct
regression proves the runtime sink receives the paired reduced outcome unchanged.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm
in 6,988 ms with 117 of 117 authority transactions, 8 ms input presentation, 40 submissions,
38 retirements, and zero cleanup debt. Classic GTK accepted exact physical text and pointer
selection, exited normally with `first_error=none`, and retired both outputs cleanly. The
remaining structural extraction is X authority-batch and resource translation plus session
supervision around the protocol-neutral runtime, followed by deletion of legacy committed-
snapshot entry points then shared with the Wayland maintenance path.


## 2026-07-18: X Authority Batches Stop At The Production Boundary

The X session loop now translates each projected authority batch once into a protocol-neutral
production record containing Engine transactions and surface removals, renderer DMA-BUF and
fence registrations, Present submissions, and release handles. X resource IDs, client IDs,
protocol errors, and authority-specific CPU update records do not cross that boundary.
`PersistentBackendRuntime` no longer accepts `XAuthorityObservedTransactionBatch`; CPU and GPU
production entry points consume only the reduced production batch plus renderer updates.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm
in 7,008 ms with 117 of 117 authority transactions, 7 ms input presentation, 42 submissions,
40 retirements, and zero cleanup debt. The guarded native mixed diagnostic translated and
exported one CPU plus one DMA-BUF layer with zero live sources, fences, or transactions. The
remaining Milestone 6 ownership work is moving the now-neutral visual control implementation
out of the CLI module and retiring its legacy committed-snapshot APIs.


## 2026-07-18: Backend Owns Production Intake Records

Backend-live now defines and exports the neutral authority batch, DMA-BUF registration, fence
registration, and Present submission records consumed by production visual control. The CLI
retains only the X-to-production translation function; it no longer defines the records or
their file-descriptor ownership shape. This makes the next runtime extraction a movement of
behavior behind an already backend-owned input contract rather than another protocol rewrite.
The full offline all-feature suite passes; runtime behavior is unchanged from the immediately
preceding strict QEMU and guarded native mixed evidence.


## 2026-07-18: Present Rebase Policy Moves Into Engine

The full-state Present generation rebase now lives beside `ProductionSessionCoordinator` in
`sophia-engine::runtime_driver`. The visual runtime no longer reaches back into a CLI library
module to reconcile skipped authority generations with the last visible Engine generation.
The former CLI module is only a compatibility re-export for its retained tests. The full
offline all-feature suite passes; this is a dependency-boundary change with no runtime
behavior change.


## 2026-07-18: Backend Owns The CPU Production Adapter

Backend-live now owns `LiveProductionCpuCycleAdapter`. It applies renderer updates after the
Engine commit, composes or coalesces from the immutable committed snapshot, creates native
frames for every output, invokes one narrow output-runtime callback, and returns reduced
composition timing and evidence. The CLI no longer implements `ProductionPresentationAdapter`
or defines the CPU production frame record; its remaining callback projects the snapshot and
invokes backend runtime/scanout objects pending their final owner extraction.

The full offline all-feature suite passes. On the rebuilt X13 QEMU image, strict two-xterm
completed in 6,971 ms with 117 of 117 authority transactions, 7 ms input presentation, 40
submissions, 38 retirements, and zero cleanup debt. Classic and confined GTK accepted exact
physical text and pointer selection, exited normally with `first_error=none`, and retired both
outputs cleanly after 54-56 CPU compositions. The next extraction is GPU scheduling and the
concrete per-output runtime owner; legacy committed-snapshot entry points remain only for the
then-active Wayland maintenance path and tests.


## 2026-07-18: Backend Owns Present Resource Admission

`LiveProductionPresentFeedbackCoordinator` now consumes the backend-owned production batch
directly to register DMA-BUF sources and fences and to process source/fence releases. The CLI
visual wrapper no longer clones file descriptors or sequences presentation-resource lifetime
admission. The full offline all-feature suite passes; behavior is covered by the immediately
preceding guarded native mixed and strict QEMU evidence.


## 2026-07-18: Backend Owns Present Scheduling State

Backend-live now owns `LiveProductionPresentScheduler`: queued and submitted Present state,
first-frame acquire delay, fence polling, bounded timeout rejection, controlled-rejection
proof policy, diagnostic triggering, and acquire/rejection counters. The CLI visual wrapper
asks for a reduced gate decision and supplies native scanout availability; it no longer owns
the scheduling tables or timing state. A backend regression proves delayed acquire admission
and one-shot controlled rejection with a registered DMA-BUF presentation.

The full offline all-feature suite passes. The rebuilt X13 guarded native mixed diagnostic
crossed the new scheduler, exported one CPU plus one DMA-BUF layer, and ended with zero live
sources, fences, or transactions. The remaining central Milestone 6 extraction is the
concrete per-output runtime owner and the legacy committed-snapshot APIs shared with Wayland.


## 2026-07-18: GTK QEMU Gate Now Proves Resize Redraw

The retained classic and confined GTK QEMU profiles previously passed input and native
presentation while reporting `surface_resize=disabled`, even though Milestone 5 requires a
CPU\/SHM redraw after an Engine-owned resize. Both guest profiles now request 640x360, and
the host harness rejects evidence unless the application record carries the complete semantic
tail: zero protocol errors, exact physical text, routed pointer selection, committed resize,
CPU\/SHM buffer path, native presentation, and clean teardown.

On the rebuilt X13 QEMU image, classic and confined Zenity each committed the resize with a
configure acknowledgement and changed pixels, accepted exact `sophia` input plus pointer
selection, exited normally with `first_error=none`, and retired both virtio-gpu outputs with
zero cleanup debt. Strict two-xterm also passed in 6,989 ms with 117 of 117 authority
transactions, 40 submissions, 38 retirements, and zero phase or cleanup debt. The remaining
Milestone 5 promotion gate is the deliberately operator-driven paired physical X13 capture.


## 2026-07-18: Production X Cursor Repaint Stops Replacing Engine State

The physical-pointer cursor repaint no longer composes frames in the outer X session loop or
calls the legacy committed-snapshot replacement entry point. A visual-runtime repaint method
reads the production coordinator snapshot, asks renderer-live to compose the cursor, creates
per-output frames, and submits them through the backend-owned output runtime set. The remaining
snapshot replacement API was named and called only by the Wayland maintenance adapter and its
regression; production X had no caller. The 2026-07-19 retirement removed that adapter.

The full CLI all-feature suite passes. On the rebuilt X13 QEMU image, strict two-xterm completed
in 6,941 ms with 120 of 120 transactions, exact keyboard and pointer proofs, 42 submissions,
40 retirements, and zero cleanup debt. Resize-enabled classic and confined GTK accepted exact
text and pointer selection, committed 640x360 CPU\/SHM redraws, exited normally with
`first_error=none`, and cleanly retired both outputs. The guarded native mixed diagnostic
exported one CPU and one DMA-BUF layer with zero live sources, fences, or transactions.


## 2026-07-18: Mixed Diagnostic Contract Moves Behind Backend Boundary

The native mixed-export completion record and its reduced evidence schema now live in
backend-live beside the native scanout diagnostic that produces them. The CLI only downcasts
the backend error, prints the reduced record, and applies command-level pass criteria; it no
longer defines a renderer\/scanout result type inside session supervision. A backend regression
freezes the exact schema. The rebuilt guarded X13 diagnostic still exported one CPU and one
DMA-BUF layer and retired all sources, fences, and transactions. This removes one CLI-specific
dependency that pinned the remaining neutral visual-control implementation to
`live_session.rs`.


## 2026-07-18: Visual Runtime Intermediate Records Move To Backend

Backend-live now owns the prepared-authority record and reduced CPU production submission
record used between visual-control phases. The CLI no longer defines internal records carrying
Engine commits, active transactions, backend ticks, renderer composition evidence, or compose
timing. Together with the backend-owned mixed diagnostic contract, this leaves the visual
control implementation dependent only on types already owned by engine, renderer-live, and
backend-live, preparing the concrete wrapper movement without changing runtime behavior.


## 2026-07-18: Concrete Visual Control Leaves The CLI

Backend-live now owns `LiveProductionVisualRuntime`, including the production coordinator,
per-output runtime set, renderer transaction projection, CPU and GPU cycle entry points, Present
resource admission and scheduling, native submission and retirement, cleanup, and reduced
feedback routing. The `PersistentBackendRuntime` type and roughly 950 lines of implementation
are gone from `live_session.rs`. CLI code constructs the runtime, translates X batches, supervises
clients, records proof evidence, and requests high-level service; it no longer defines visual
state-machine behavior. A reduced diagnostics snapshot replaces direct CLI access to Present
resource and scheduler internals.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm in
6,916 ms with 120 of 120 transactions, 40 submissions, 38 retirements, exact keyboard and pointer
proofs, and zero cleanup debt. Resize-enabled classic and confined GTK passed exact text, pointer
selection, committed 640x360 CPU\/SHM redraw, normal exit, `first_error=none`, native presentation,
and clean teardown. Emergency recovery flushed all five routed chord events and completed in 167
ms. The guarded native mixed diagnostic exported one CPU and one DMA-BUF layer with zero live
resources. Milestone 6 ownership, fail-closed, and retained-gate migration items are complete.
The remaining exit item is moving asynchronous GPU-service and retirement trigger timing from
the CLI event loop into `sophia-engine::runtime_driver`.


## 2026-07-18: Production X Uses One Native Service Poll

`LiveProductionVisualRuntime::service_native` now owns the asynchronous native service order:
page-flip retirement and cleanup first, eligible queued Present work second, and pending native
frames last. It returns one reduced report with the optional backend tick and phase observations.
The production X event loop no longer inspects pending exporter frames or separately invokes
retirement, GPU scheduling, and native idle submission. Wayland retains its specialized
maintenance service because it correlates client buffer release to its own submission counters.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm in
6,986 ms with 117 of 117 transactions, 40 submissions, 38 retirements, and zero cleanup debt.
Resize-enabled classic and confined GTK passed exact input, pointer selection, committed resize
redraw, normal exit, `first_error=none`, native presentation, and clean teardown. The remaining
Milestone 6 exit gap is exact rather than structural: GPU Present prepare\/retire sequencing still
lives in backend visual control and must enter `sophia-engine::runtime_driver` before that module
is the only production visual coordinator.


## 2026-07-18: Full-State Present Preparation Enters Runtime Driver

`ProductionSessionCoordinator::prepare_full_state_present` now owns authority-generation rebasing
and Engine preparation against its committed snapshot. Backend visual control no longer reaches
through `coordinator.engine()` or independently selects the preparation baseline. The same
coordinator already owns matching-retirement application and suppresses feedback when that
baseline is stale, so both sides of the asynchronous prepared-commit gate now remain in
`runtime_driver`. The external regression deliberately supplies generation 99 and proves the
coordinator rebases and commits it against the visible generation.

The full offline all-feature suite passes. The rebuilt guarded X13 mixed path crossed the new
coordinator entry point, exported one CPU plus one DMA-BUF layer, and retired all sources, fences,
and transactions. The remaining Milestone 6 coordinator gap is asynchronous KMS service adapter
shape, not Engine Present preparation or retirement ownership.


## 2026-07-18: Runtime Driver Owns Asynchronous KMS Phase Order

`ProductionAsyncServiceCoordinator` consumes reduced in-flight, cleanup, queued-Present, and
pending-frame observations and requests at most the ordered Retire, SchedulePresent, and
SubmitPendingFrame phases. Backend-live executes those requests and feeds updated observations
back after each action; it no longer encodes asynchronous phase order. A runtime-driver regression
proves dynamic behavior: retirement unlocks Present scheduling, and a resulting in-flight frame
suppresses pending submission in the same service pass. Together with coordinator-owned CPU
cycles and full-state Present prepare\/retire gates, this closes the last Milestone 6 architecture
checkbox.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm in
6,973 ms with 117 of 117 transactions, 40 submissions, 38 retirements, and zero cleanup debt.
Resize-enabled classic and confined GTK passed exact input, pointer selection, committed resize
redraw, normal exit, `first_error=none`, native presentation, and clean teardown. The guarded
native mixed path exported one CPU and one DMA-BUF layer with zero live resources. Milestone 6
implementation is complete; its overall promotion remains coupled to the operator-driven paired
physical X13 GTK evidence still required by Milestone 5.


## 2026-07-18: Post-Milestone-6 Native Stability And Physical Evidence Audit

The documented unattended X13 native stability gate passed 10 of 10 release runs against the
runtime-driver-owned phase state machine. Every retained record passed exact terminal text,
changed pixels, native presentation, callback validation, and zero in-flight or cleanup debt.

The durable Milestone 5 physical GTK store was audited rather than assumed valid. Its classic
record ends at pointer readiness with zero routed pointer events and has no application-session
completion; its confined record is empty; recovery records `emergency=true`. Those artifacts
cannot satisfy the current paired verifier. The remaining daily-driver promotion action is a
fresh local-TTY run of `tools/live_session_milestone5_gtk_hardware_proof.sh`, followed by the
three-class aggregate verifier. It requires a person to arm the independent guard, type exact
text, and physically click each dialog, so it cannot be completed through unattended SSH or
QEMU without weakening the stated acceptance criterion.


## 2026-07-18: GTK Client Exit Hang And Post-Proof Completion Watchdog

The first X13 run with routed pointer buttons accepted exact physical `sophia` text, routed
the OK click, and presented the surface-removal frame, then held a blank screen until the
emergency chord restored the TTY. Reduction found a completion-phase deadline vacuum: once
the text proof completes and a button routes, the keyboard-sequence and pointer-selection
deadlines are disarmed and the global runtime budget intentionally stays out of input proofs,
so any stall after the proof loops without a bound. The loop exit requires the primary
client's reaped exit status; a toolkit that destroys its window but never exits leaves that
term false forever. On a bare text TTY without a session bus address, GTK finalization is the
prime suspect for the missing exit.

The session now bounds the post-removal wait: when the application proof surface is gone and
the client has not exited within five seconds, the loop fails closed with the exact exit-term
states instead of presenting blank frames. Application-proof clients launch under
`dbus-run-session --` when no bus address exists and the runner resolves on `PATH`, giving
the toolkit a bounded per-client bus that exits with the client. The first watchdog draft
armed on proof-complete-plus-button and falsely expired inside the QEMU click-then-submit
sequence; the retained trigger is surface removal, which is the actual abnormal state.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm,
and resize-enabled classic and confined GTK passed exact text, routed button selection,
committed 640x360 redraw, normal exit, `first_error=none`, native presentation, and clean
two-output retirement. Fresh paired physical X13 evidence remains the acceptance gate; if the
watchdog fires there, its record names the missing exit term.


## 2026-07-18: X13 GTK Blank Session Reduced To Tap Policy And Pointer Deadline

A fresh classic hardware run accepted exact physical `sophia` input, committed the 640x360
GTK resize, presented the software cursor, and routed sustained touchpad motion. It emitted no
pointer-button record, no application-session record, and no bounded-completion record before
the independent emergency chord restored the TTY cleanly. X13's libinput report confirmed that
the ELAN touchpad supports tapping but defaults tap-to-click to disabled.

The native path-based libinput owner now enables tap-to-click for every tap-capable admitted
device, verifies the applied state, and exports only reduced device/tap counts. The proof loop
now distinguishes motion observed/routed from button observed/routed. Its selection deadline
remains armed after cursor pixels change and ends only after both a routed button and pointer
pixel evidence; this closes the prior unbounded state where motion canceled the only pointer
deadline. Cursor repaint also fails closed if an application proof produces no composed layer
or only the bounded software-cursor footprint.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm
in 6,880 ms with 19 of 19 input deliveries, 40 submissions, 38 retirements, and zero native
debt. Resize-enabled classic and confined GTK passed exact text, routed button selection,
normal exit, `first_error=none`, and clean two-output retirement. A bounded non-KMS smoke
against the real ELAN path reported `devices=2 tap_capable=1 tap_enabled=1` and completed its
xterm pixel proof. Paired physical X13 evidence remains the acceptance gate before GTK3
software promotion.

## 2026-07-18: Milestone 5 Uses Unattended QEMU Acceptance

Machine-specific X13 capture is no longer an application-promotion gate. The
repeatable acceptance boundary is a diskless, networkless QEMU guest that owns
virtio DRM/KMS, guest console state, and libinput-backed virtio keyboard and
pointer devices. Direct hardware runners remain optional compatibility
diagnostics.

`tools/qemu_milestone5_acceptance.sh` rebuilds the guest and runs strict
two-xterm presentation/input, emergency Ctrl-Alt-Backspace recovery, and classic
plus confined GTK3 profiles without operator input. The first aggregate run
exposed a stale schema-1 poller assertion in emergency recovery; updating the
harness, verifier, and fixtures to the schema-2 tap-policy record closed it. The
rerun passed all four scenarios. The strict three-class baseline then passed

## 2026-07-18: WM API v2 Foundation

Milestones 7 and 8 split interactive policy enablement from daily-driver
promotion. The normative WM contract now fixes Engine ownership of physical
input, nine workspace slots, named session actions, opaque metadata, and
one-visible-workspace-per-output semantics.

The protocol carries a versioned hello, bounded binding registrations, session
descriptor, opaque action activation, workspace activation, and named session
action requests. Engine rejects unsupported capabilities, duplicate bindings,
invalid action/key values, and Ctrl-Alt-Backspace. Its shortcut registry
consumes matching press/release pairs and suppresses repeats without leaking raw
input. The native demo WM performs the startup handshake and exercises focus,
workspace, and terminal actions. Engine now owns per-seat physical modifier
state, consumes registered chords after the emergency chord check, and sends
opaque action activations through the live WM transport. A nine-slot workspace
policy validates workspace swaps, surface moves, visible focus, layout commands,
and advertised session tokens. The profiled legacy bridge registers the bundled
xmonad chord set and translates bounded workspace and named-action requests.
The full offline all-feature suite passes. Atomic delayed-commit persistence,
named-action execution, xmonad focus/layout synthesis, and QEMU remain open.

## 2026-07-18: Normal xmonad Session Launcher

Milestone 7 is archived with its unattended QEMU gate retained as a frozen
regression. Its verifier now has a positive fixture plus negative mutations and
fails explicitly when any guest failure marker is present.

`sophia-live-session --session-mode=normal` now owns a bounded application
registry with explicit startup and named-action mappings. Applications are
spawned without a shell in dedicated process groups; normal exit is nonfatal,
and shutdown sends TERM to the group before a bounded KILL fallback. The
operator launcher selects xmonad compatibility policy, native Sophia WM policy,
or no external WM without placing a WM identity in Engine.

The first Milestone 8 QEMU gate starts one registered xterm, survives an
intentional bridge/xmonad restart with layout preserved, launches a second
registered xterm through xmonad's terminal action, closes it, logs out, and
retires two-output native presentation without cleanup debt. The frozen
Milestone 7 two-xterm gate also passes against the same build.
## 2026-07-22: Xmonad TTY3 Requires Independent Local Recovery

The first physical native-xmonad TTY3 operator attempt produced blank scanout.
Sophia did not provide VT suspend/resume, the wrapper had no independent input
guard or explicit KD/termios restoration, and its documented recovery path
incorrectly depended on switching to another TTY. The operator had to reboot,
which also erased the `/tmp` launcher log.

The xmonad wrapper now applies the established guarded TTY lifecycle before
graphics takeover: one Ctrl-Alt-Backspace chord must arm the independent input
guard, a second chord stops the supervised session even if Sophia input routing
is wedged, and cleanup restores KD mode, termios, and keyd. Guard and recovery
records are durable under the user's state directory. Ctrl-Alt-Fn remains
unsupported until Engine owns a correct VT/DRM suspend-and-resume boundary.

The guarded rerun proved DRM ownership, two-output presentation, WM bridge
startup, and complete emergency restoration, but also retained
`startup_apps=0` and no live `key_observed` marker after Super-Enter. The normal
TTY wrapper had omitted `--session-start=terminal`. Independently, a blank
normal session represented its missing primary child as already exited; the
post-exit proof guard then compared two absent surfaces and suppressed the
entire physical-input poll, including global WM shortcuts. The session now
distinguishes full application routing, shortcut-only polling, and complete
suppression. Empty desktops admit emergency and registered WM chords without
delivering ordinary keys or pointer events to an unfocused client, and the
physical launcher starts Kitty by default.

## 2026-07-22: Kitty Requires A Direct-Mesa GLX Bootstrap

The guarded physical rerun reached xmonad and input recovery, then Kitty 0.48.0
failed with `failed to create GLFW window`. Sophia advertised GLX 1.4 but no
FBConfigs and omitted `GLX_EXT_libglvnd`; libglvnd therefore selected its
indirect vendor instead of Mesa and never reached the already implemented
DRI3/Present path.

Sophia now exposes a depth-32 ARGB TrueColor visual, maps GLVND's vendor query
to `mesa`, and implements a bounded direct-rendering GLX bootstrap: visual and
FBConfig catalogs, client-info negotiation, direct context lifecycle, GLX
window aliases, and drawable attributes. The catalog deliberately contains
only XRGB linear, ARGB linear, and ARGB sRGB configurations with depth/stencil
zero. Indirect GLX rendering, server-side Render/RenderLarge, and GLX
SwapBuffers remain unsupported; Mesa renders client-side and submits through
DRI3/Present. The first live trace additionally showed Mesa using X Sync to
destroy DRI3 fences and GLFW freeing its ARGB colormap, so the bounded Sync
initialization/fence teardown and core colormap teardown paths are retained as
part of the same compatibility slice.

`x-authority-kitty-smoke` now proves the exact live sequence on an AMD render
node: GLVND vendor selection, FBConfig discovery, two direct context/window
lifecycles, depth-32 modifier negotiation, ARGB DRI3 import, one accepted
Present transaction, one committed runtime surface, and clean protocol state
with `first_error=none`. The guarded physical TTY3 capture remains the session
gate; the standalone smoke deliberately terminates its proof window after the
first committed frame because it has no live renderer feedback loop.

## 2026-07-23: Concurrent Present Identity and Physical Cursor Follow-up

The next guarded TTY3 run proved that Super-Enter launched a second terminal,
then the persistent frontend exited with `X11 route queue is full for client
9`. Each X client worker had derived the Engine transaction identity from its
connection-local 16-bit X sequence. Two clients could therefore submit the
same global `TransactionId`; the pending Present registry treated that
collision as queue pressure, and worker completion propagated the client error
to the whole frontend supervisor.

The listener now allocates monotonically increasing 64-bit transaction
identities shared by all of its workers while preserving the X wire sequence
as a client-local `u16`. Duplicate transaction identity and real per-client
pressure are distinct failures. A full Present queue waits for bounded
Complete/Idle progress for up to two seconds, and removal wakes waiters; no
feedback is dropped or reordered. A client-local Present failure disconnects
only that worker, while panics, poisoned shared state, and supervisor failures
remain service-fatal.

The same capture exposed the private legacy-WM server's 1280x720 setup fixture
on physical 2560x1440 and 1920x1080 outputs. Production bridge startup is now
lazy until the first Engine layout request, advertises those actual bounds in
X setup, and emits root ConfigureNotify on later changes. The compatibility
fixture remains available only to standalone smoke callers.

Cursor ownership remains protocol-neutral. A bounded passive snapshot models
visibility, global position, hotspot, ARGB image, and generation without an X
or application identity. The current software path now coalesces motion,
avoids repaint submission while native scanout is in flight, polls at 1 ms
while motion is pending, and reports maximum motion-to-submit latency.
Client-selected classic X cursor shapes and optional KMS cursor-plane
presentation remain separate follow-ups; the Engine contains no Kitty-specific
branch.

## 2026-07-23: Kitty-Only Physical Input Gate

The first run after the Present identity fix appeared completely locked, but
the durable log proved two physical mouse motions were observed and routed and
Super-Enter launched a second Kitty. Both xmonad layouts timed out, and the
cursor path could replace a DMA-BUF application frame with a CPU-only scene.
This was a visible presentation failure, not lost libinput events.

The supported TTY3 gate now starts one automatically focused Kitty with no
external WM while retaining both physical outputs and the independent recovery
guard. Xmonad, Super-Enter, and resize synchronization are deferred until
keyboard, pointer motion, button delivery, drag selection, and clean shutdown
pass together.

DMA-BUF sessions no longer invoke the CPU cursor repaint. Backend-live owns a
64x64 ARGB dumb buffer containing a classic left pointer and uses the DRM
hardware-cursor interface to install it on the active CRTC, move it without a
primary-plane repaint, and transfer it across the extended-horizontal output
layout. Motion is coalesced, presentation latency and hardware update failures
are reduced into the session evidence, and CPU repaint remains available only
for CPU-buffer regression clients.

## 2026-07-23: Production Input Uses Libinput Seat Discovery

The guarded launcher previously guessed one keyboard and the first distinct
mouse from `/dev/input/by-id` and `/dev/input/by-path`. That could not correctly
represent composite receivers, multiple keyboards, touchpads, or hotplug.

Backend-live now supports libinput's udev context and assigns `seat0` by
default. Device-added events classify keyboard, pointer, and touch
capabilities, apply tap-to-click where supported, and update bounded policy
evidence; device-removed events maintain active counts without exposing device
names or paths. Both the independent recovery guard and the Sophia session use
seat discovery. Explicit `--input-devices` remains mutually exclusive with
`--input-seat` and is retained only for deterministic hardware/QEMU proofs.

## 2026-07-23: Kitty Startup Is Bounded Before the First Surface

The first Kitty-only `seat0` run acquired both outputs and discovered fourteen
libinput devices, but remained blank until the independent guard restored the
TTY. The session log contained no focused or committed application surface.
Kitty's desktop-settings request failed after 10.389 seconds because a bare TTY
had no usable portal service; emergency recovery was requested at the same
boundary. The separate non-modesetting Kitty trace still passed 207 X requests,
one DRI3/Present transaction, one runtime surface, and `first_error=none`, so
the direct-Mesa GLX path was not the failing boundary.

The first private-bus attempt activated the host notification and XFCE settings
services without a usable desktop display, adding another nondeterministic
startup path. The Kitty gate now matches the passing standalone trace instead:
Wayland variables are removed, desktop-service bus activation is disabled, and
the no-WM profile forces opaque X11 rendering. The live session accepts a
generic bounded startup deadline and succeeds only after a focused CPU-detail
or DRI3/Present surface crosses actual native presentation. A missing surface,
uncommitted surface, missing visual content, or unpresented frame reports a
distinct reduced stage and returns through the normal TTY cleanup path after
eight seconds.

Native normal sessions initialize an empty output runtime immediately. Physical
pointer motion is polled before a client surface exists, the compositor-owned
classic hardware cursor begins at the primary-output center, and unfocused
keyboard and pointer-button events remain unrouted. This removes the prior
first-surface dependency from cursor feedback without introducing an
application-specific branch in Engine.

The first centered-cursor repair was insufficient. A physical rerun still
showed a frozen inherited pointer beside Sophia's moving pointer because the
atomic display owner attempted to clear cursor state through deprecated legacy
cursor ioctls and discarded every error. Backend-live now discovers cursor
planes compatible with every selected CRTC, atomically detaches them before
first use, retains an ARGB cursor framebuffer, and performs coalesced atomic
attach/move commits. Hardware and software paths share one canonical classic
X11 pointer raster.

The same rerun proved that input discovery was not the keyboard failure:
libinput observed a key and routed pointer motion plus a button. Kitty created
and mapped a surface and submitted one DRI3/Present frame, but no later
authority batch arrived after asynchronous KMS retirement. Initial focus and
startup-content reconciliation lived only in the authority-batch branch, so
the retired surface never gained focus and keyboard delivery remained gated.
Present retirement now carries its transaction and surface back to the session
loop; the shared reconciliation path runs after both authority work and KMS
service, sends X11 focus control, and recognizes the retired frame without
requiring a second Present. Startup diagnostics now report actual committed
surfaces, focus/control state, Present retirement, native submission failures,
callbacks, and per-output in-flight state.

Finally, the physical gate had not actually matched the passing Kitty smoke:
it still loaded the normal user configuration. The guarded profile now uses
`--config NONE` with only the forced X11, opaque-background, and diagnostic
title arguments. Normal Kitty configuration compatibility remains outside the
minimal input gate.

## 2026-07-23: Native Scanout Uses One Frame And One Cursor Owner

The next physical Kitty-only run proved that accepted KMS commits and routed
input were not sufficient visual evidence. Kitty created a 2540x1390 window,
completed DRI3/Present, and received keyboard and pointer routing, while the
operator still saw a frozen pointer beside the moving hardware cursor and no
Kitty content.

The exporter had independent pending CPU, DMA-BUF, and mixed-frame slots.
Consequently a mixed Kitty frame could export first while an older CPU frame
remained eligible for the idle service, and later CPU-only authority work could
replace retained DMA-BUF content. Native CPU composition also continued to
receive the pointer position even though the atomic cursor plane owned cursor
presentation.

Native pending scanout is now a single latest-wins slot per output. A mixed
frame supersedes its unsubmitted CPU precursor, and authority work preserves
the last GPU scanout whenever a committed or pending DMA-BUF layer cannot be
recomposed safely. Physical native sessions select hardware cursor ownership
explicitly and never bake cursor pixels into CPU backgrounds. Mixed submission
resolves the registered primary output by identity, and startup becomes ready
only when the exact Present transaction is the stable displayed content with
no newer primary frame queued or submitted. The Kitty-only physical capture
remains required before restoring xmonad.

## 2026-07-23: Secondary Scanout Cannot Block Primary Present

The first physical run with stable Present provenance showed one correct
hardware cursor but no Kitty pixels. Kitty created a 2540x1390 window and
issued Present, yet startup ended with zero committed surfaces. The primary
output was idle while the secondary output still had an unrelated CPU frame in
flight. The async service used one global in-flight bit, so the secondary
retirement prevented the primary mixed frame from ever being scheduled.

Native async service decisions are now output-scoped. Retirement still polls
every in-flight or cleanup-pending output, but a queued mixed Present is blocked
only by the registered primary output. Pending background frames are submitted
only on outputs that are individually idle, so one blocked output cannot cause
another exporter to run or prevent useful work. Submit, callback, and
retirement diagnostics now carry output identity and submission provenance.
The next guarded Kitty-only capture must prove the primary Present commits
while secondary retirement remains independent.

## 2026-07-24: Present Driver And VT Must Share Output-Scoped Ownership

The next physical run still showed only the hardware cursor. Output-correlated
logs proved the async coordinator retired the primary independently, but the
Present driver retained a second global in-flight early return. The secondary
output therefore continued to veto mixed composition after the coordinator had
correctly admitted it.

The same run left typed `ll` in tty3's input queue, where it appeared after
greetd returned. The Kitty launcher saved and restored KD and termios state but
never entered KD graphics or raw/no-echo mode. That left the console line
discipline active underneath native scanout.

The Present driver now consumes the same tested output-state reduction as the
service coordinator and blocks only on primary in-flight or cleanup state.
After the independent emergency guard is armed and immediately before starting
Sophia, the launcher switches to KD graphics and `stty raw -echo`; its existing
cleanup restores the exact saved KD and termios state on normal, failed, signal,
and emergency exits. Regression tests retain the guard-before-takeover order
and exact restoration commands.

## 2026-07-24: Queued Present Must Reserve The Primary Output

The first physical run after VT takeover showed one working hardware cursor but
no Kitty pixels. The capture proved Kitty created and mapped its 2540x1390
window, submitted Present transaction 1, and routed pointer motion and button
events. It also proved exact KD and termios recovery. Mixed scanout never
submitted: after a primary CPU frame retired, the async service repeatedly
submitted another CPU frame until the startup watchdog expired.

The async coordinator orders Present before pending frames, but a Present phase
is an attempt rather than proof of submission. If that attempt observes a
transiently blocked primary, advancing to the pending-frame phase can fill the
same primary immediately and starve Present indefinitely. A queued Present now
reserves the primary from pending CPU submission. Idle secondary outputs remain
eligible, preserving independent multi-output progress. Regression coverage
proves both the primary reservation and secondary eligibility.

## 2026-07-24: The Opaque sRGB Hypothesis Was Disproved

The next physical run proved mixed scanout and input were functional despite
the display still appearing black. Present transaction 202 reached mixed KMS
scanout, retired, became stable in 708 milliseconds, and accepted the user's
`exit` input. The failure was therefore the content of the imported client
layer rather than scheduling, focus, or VT ownership.

Adding an opaque sRGB framebuffer configuration did not change the client
selection: the next retained request stream still selected FBConfig 3, created
depth-32 windows, and exported ARGB8888 pixmaps. Both physical outputs remained
blank even though the mixed frame retired and keyboard input reached the
client. The speculative configuration and its compatibility claim are removed.

The remaining boundary is pixel content. Native mixed composition now has an
opt-in, one-shot readback that reports only aggregate counts and checksums after
the CPU background and after the DMA-BUF layer. The verifier distinguishes an
unchanged framebuffer, no visible RGB delta, and a visibly changed client
layer. ARGB composition also uses premultiplied source-over blending (`ONE`,
`ONE_MINUS_SRC_ALPHA`) instead of multiplying source RGB by alpha twice. These
changes remain application-agnostic and preserve Engine's protocol-neutral
authority boundary.

The first attempted diagnostic capture contained no pixel-evidence records, so
it could not classify the blank frame. Until the Kitty-only physical gate
passes, that profile now enables the bounded one-shot trace directly rather
than depending on an operator choosing a separate wrapper. An explicit
`status=enabled` record distinguishes missing activation from a failed GL
readback.

The resulting physical capture localized the blank screen further. The CPU
background and the framebuffer after the first Kitty DMA-BUF layer had the
same checksum and zero nonzero RGB pixels, while scanout retired the mixed
transaction normally. Kitty submitted that initial Present before mapping its
top-level window, then allocated a second DRI3 pixmap but submitted no second
Present. This is consistent with the client waiting for Present
Complete/Idle feedback before exposing its rendered window, not with a KMS,
cursor, keyboard-routing, or terminal-shell failure. The Kitty gate now traces
whether each backend completion was actually routed into the X frontend so the
next physical capture can distinguish feedback routing from client-side
consumption.

The next physical capture made that distinction: Complete and Idle were both
generated for the retired frame, but both reported `routed=false`. Standard
Present decode retained the X server frontend's globally allocated transaction
for its pending-feedback registry, while dispatch replaced the transaction
sent to Engine with the client's 16-bit request sequence. Retirement therefore
could not match the pending entry. Standard Present now carries the global
transaction explicitly through decode and dispatch, matching the existing
feedback registry key and avoiding cross-client sequence collisions. A wire
regression uses deliberately different request-sequence and global-transaction
values and requires the resulting surface transaction to retain the global
value.

The first capture after that change still reported `routed=false`, proving the
single-client run did not depend on the transaction distinction. The request
order exposed the actual feedback loss: Kitty selected Present events for a
bootstrap window, selected them again for its main window, then cleared the
bootstrap selection. The frontend stored only one selection per client, so
clearing the old event ID removed the newer main-window selection. Complete
marked the pending presentation finished but found no subscriber; Idle then
removed it without notifying Kitty. Kitty consequently never mapped its main
window or submitted the rendered follow-up pixmap.

Present selections are now retained per client and event ID, and a zero mask
removes only that selection. Complete and Idle route to every matching
window/mask selection. A regression preserves Kitty's observed bootstrap/main/
clear ordering and requires both events on the main event ID.

The same capture confirmed exact KD and termios restoration but returned to
greetd's VT because restarting the service activated its console. The guarded
TTY3 launcher now records its originating VT, restores the display manager,
reactivates that VT with `chvt`, and records the resulting active VT for both
normal and emergency cleanup.

The next physical run reached a visible Kitty command line, routed typed keys,
and continued retiring rendered Present frames with Complete/Idle feedback.
The session terminated only after pointer motion attempted a nonblocking atomic
cursor-plane update while another KMS commit was still in flight. Linux
returned `EBUSY`; the cursor path recognized only `EAGAIN`/`WouldBlock` as
transient and escalated `EBUSY` into a fatal session error. Nonblocking cursor
attach, move, and detach now classify both results as deferred and retry from
the existing dirty-cursor loop after scanout progresses. Other atomic errors
remain fatal.

The following run stayed healthy but did not echo `ll`. Its ordering identified
a separate startup race: Sophia declared focus ready and forwarded the physical
keys roughly one second before GLFW changed the mapped window's core event mask
to include KeyPress and KeyRelease. The X frontend previously fell back to the
focused window even when no window in its ancestor chain had selected keyboard
events, so the client ignored those early records. The input writer now keeps a
physical key boundedly pending for up to five seconds while the focused route
has no keyboard selection, then targets the selected window as soon as the
client installs its mask. This is based solely on standard X11 event selection;
it contains no Kitty-specific policy.

The next operator observation showed `ll` on tty3 only after emergency teardown.
That proves the VT input queue was still receiving the physical keyboard:
`stty raw -echo` disabled canonical processing and echo but did not disconnect
the Linux console keyboard from the VT. The guarded launcher now saves the
console keyboard mode with `KDGKBMODE`, selects `K_OFF` while Sophia owns the
graphical VT, and restores the exact saved mode during every cleanup path.
Evdev remains available to libinput and the independent emergency guard.

## 2026-07-24: Kitty input is blocked after wire delivery

`x-authority-kitty-input-smoke` now reproduces the interactive failure without
owning a VT. It launches Kitty 0.48.0 with `--config NONE`, uses the routed
frontend and render node, waits for two DRI3/Present submissions, snapshots the
client-visible XKB map, focuses the mapped surface, and injects `ll` plus
Return. The gate only passes when the proof shell writes exactly `ll` and Kitty
submits another Present.

The retained diagnostic boundary is narrower than the earlier hardware
hypotheses: keycode 46 resolves to `l`; Kitty's mapped window selected core
KeyPress/KeyRelease; focus control is delivered; all six events are flushed to
the client socket; and Present Complete/Idle routing succeeds. The shell still
receives zero bytes and Kitty submits no post-input frame, while the existing
xterm input smoke passes the same brokered route. Physical libinput discovery,
console echo, Kitty configuration, and xmonad are therefore not the current
root boundary. Promotion remains blocked on Kitty consuming the X11 stream
across Present synchronization.

## 2026-07-24: Kitty Keyboard Root Cause Was Extension Event Aliasing

The strict real-Kitty input gate now passes. Physical input discovery, routing,
focus, event selection, and XCB receipt had all been working. An instrumented
libX11 showed that each core KeyPress/KeyRelease reached its queue and was then
rejected by the installed wire converter. Sophia advertised GLX with traditional
event base zero, so libGLX registered its seventeen extension converters over
core event numbers 0 through 16, including KeyPress 2 and KeyRelease 3.

Sophia now assigns non-core, mutually disjoint traditional event ranges to
RANDR, XFIXES, SYNC, GLX, XKEYBOARD, XInputExtension, and MIT-SHM. The XKB
names reply also reports level-name counts consistent with the two levels
advertised by XkbGetMap. Installed Kitty 0.48.0 consumes routed `ll` plus
Return, writes the exact shell result, and submits three later Presents. This
is protocol-level, application-agnostic behavior; no Kitty policy exists in
the engine.

The subsequent guarded TTY3 run provided the physical promotion proof. Kitty
became visibly ready in 798 ms; physical keyboard input and two pointer-button
transitions were routed; cursor motion-to-submit remained bounded at 13 ms;
Kitty exited with status zero; protocol health was clean; and the originating
TTY modes were restored without emergency recovery. A separate report-field
bug falsely rejected that successful run because the stable-Present readiness
path logged readiness without persisting its elapsed time; both paths now
populate the same readiness measurement.

## 2026-07-24: Architecture Debt Moves Behind Stable Facades

The conformance pass now treats file size as evidence of mixed ownership rather
than a mechanical target. X authority dispatch observations are owned,
value-free records; routing packets and frontend admission/provider contracts
live in separate data modules while the existing crate facade remains stable.
The live-session command now delegates secure Xauthority files, proof
artifacts, X frontend adapters, and process-group supervision to their owning
modules. Its owner loop and physical-input poll boundary receive explicit
channel/resource/startup records instead of 18- and 16-argument call sites.

Native scanout now separates passive DMA-BUF and composition records from
GBM/EGL execution, and the legacy WM bridge separates wire framing from runtime
supervision. X transport reduction tests moved from production source to an
integration test through the public owned-observation API. Focused all-feature
checks and tests pass for the affected crates. The remaining work is the
mutable registry/worker and protocol-family extraction, the live owner/WM/input
split, scanout lifetime/composition execution, legacy bridge server/dispatch,
and the remaining oversized integration fixtures.

## 2026-07-24: Continuous Present Starved WM And GPU Input Readiness

The installed xmonad proof still appeared to lose both keyboard and pointer
input after tracing was disabled. The retained session showed fourteen active
libinput devices, stable mixed Present retirement, a visible hardware cursor,
and Engine focus, but no WM layout commit, applied X11 focus, or physical-input
readiness marker. Disabling tracing changed timing only; it could not repair
either missing state transition.

Initial WM management waited for 500 milliseconds without any authority work.
Kitty's continuing Presents reset that global timer, so xmonad could remain
ready yet never receive its first opaque surface. Unmanaged surfaces are now
submitted whenever no WM transaction is pending, one at a time, independent of
application frame cadence. Startup and proof readiness also track the surface
whose X11 focus was actually acknowledged instead of treating the presence of
an external WM as proof that focus was applied.

The same stable DMA-BUF retirement that satisfied visible startup did not set
the older CPU-only terminal-content flag. The proof therefore never armed and
the session deliberately skipped libinput polling. Focused content readiness
now accepts either CPU visual detail or a stable retired Present belonging to
the focused surface. Libinput is always drained: before proof readiness,
pointer motion updates only the compositor-owned cursor while ordinary keys
and buttons are discarded without entering the exact-text matcher or X11
route. The installed gate now requires exact `sophia` input followed by routed
pointer motion and one button. All decisions use opaque surfaces and generic
presentation facts; Engine contains no Kitty-specific behavior.

The first installed rerun confirmed the WM, focus, cursor polling, and stable
Present changes, but physical input still remained in cursor-only mode. The
proof had a second baseline predicate requiring nonzero CPU-scene pixels even
after the focused stable DMA-BUF surface was accepted. GPU-only Kitty therefore
reached content readiness without satisfying the duplicate CPU gate. Baseline
readiness now consumes the same focused-content fact used by startup and input
arming; CPU composition remains an alternative source rather than an additional
requirement. A regression fixes the GPU-ready/CPU-empty combination.

The next installed run proved that exact physical `sophia` input reached the
shell and all fourteen X11 events flushed. Thirty later authority batches and
stable native Presents followed, but the post-input verifier still required a
CPU-buffer checksum or generation change and timed out a GPU-only terminal.
A stable retired Present on the exact proof surface after input delivery now
provides the corresponding GPU presentation evidence.

That run also exposed a separate cursor ordering error. Once text input was
ready, pointer motion entered full routing mode, but application pointer
delivery remained gated until the later pointer-proof phase. Cursor placement
was incorrectly behind that delivery gate, freezing the compositor-owned
hardware cursor. Placement now occurs before the application-delivery decision,
so cursor motion remains responsive without prematurely routing pointer events.

The following physical run displayed a blinking prompt but did not echo input.
Its retained evidence contained `key_observed` without `key_routed` and ended at
`focus_control_pending`. The WM layout committed before the corresponding
surface entered Engine's committed-surface set; the first focus attempt returned
`UnknownSurface`, but the owner loop consumed the one-shot focus request anyway.
WM focus requests now remain pending on that transient result and are consumed
only after Engine focus and X11 client focus both succeed.

The retry run then committed focus, armed physical input in 800 milliseconds,
and routed keys. It stopped only because the proof matcher required strict
press-release pairs and rejected ordinary keyboard rollover when `a` was
pressed before `i` was released. The proof now validates exact character press
order while independently tracking balanced releases, accepting natural
overlap without weakening modifier, repeat, unexpected-release, or submit-key
checks.

## 2026-07-24: Owner-Loop State And Oversized Tests Split By Domain

The remaining live-session owner-loop state now has explicit delivery,
observation, cursor-update, and metrics records. Input-delivery draining is an
owned phase with one state boundary instead of a macro mutating seven ambient
delivery variables. The 168-line owner-loop facade initializes resources and
state, then delegates lifecycle, authority, input proof, physical input, and
completion to bounded phase owners.

Oversized test programs were split along real ownership seams: live-session
presentation, Engine rendering transactions, runtime process supervision,
atomic-scanout retirement, and native page-flip decoding. The source-layout
audit no longer reports any test program at 800 lines. The split modules reuse
their parent fixtures and preserve the same behavioral assertions; focused
CLI, Engine, runtime, and all-feature backend suites pass.

## 2026-07-24: Physical Promotion Needs Action And Output Evidence

The original physical xmonad verifier proved that xmonad committed some layout
and focus work, that two Kitty processes started, and that the session retired
native frames. Those aggregate facts could not distinguish the requested
focus, layout, workspace, close, click-drag, and two-output workflow from a
shorter interaction that happened to increment the same counters.

The live-session boundary now records the opaque action number after a
physically initiated WM proposal commits. This remains application- and
protocol-neutral: the Engine sees the same opaque policy action and no Kitty
identity. The physical verifier requires the focus-next, next-layout,
workspace-away, workspace-return, and close actions, two pointer-button
transitions, two terminal launches, two native outputs, and an independently
retired page flip on each output. Fixture mutations prove that missing
workspace, output-retirement, cursor, or click-drag evidence is rejected.

These records prove that the requested control path committed; they do not by
themselves prove that a hidden surface received no routed input. That
isolation claim remains a physical gate until delivery evidence correlates
input with the focused visible surface before and after workspace changes.

## 2026-07-24: Independent Recovery Must Not Preempt Owner Cleanup

The independent input guard previously exited 250 milliseconds after detecting
the emergency chord. The TTY wrapper's `wait -n` then returned and cleanup
immediately sent `TERM` to the graphical process group. That could preempt the
live owner loop even when it had independently observed the same chord and was
draining routed input, native scanout, and Present state.

After a guard trigger, the wrapper now gives the live session a bounded
five-second window to finish its in-process emergency path. A schema-3 recovery
record distinguishes `graceful` completion from `fallback_term` and retains the
session exit status alongside KD and termios restoration. The physical
emergency verifier requires guard and owner observations, a status-zero
graceful exit, fully drained input, no native or Present debt, and exact TTY
restoration. Fixture mutations ensure that a TERM fallback cannot be promoted
as a successful emergency capture.

## 2026-07-24: Scroll And Hidden Focus Were Missing Input Semantics

The Firefox promotion audit found that libinput wheel events were dropped
before entering Sophia packets. Input now carries signed, protocol-neutral
horizontal and vertical v120 units through Engine hit-testing. Only the X
frontend maps those units to core X11 scroll buttons, emitting a bounded
press/release pair. The deterministic Firefox workload now proves scroll,
focus-away/focus-return, and a pointer-opened dialog in addition to keyboard,
CLIPBOARD, PRIMARY, resize, and normal exit.

The same audit found that workspace policy cleared its hidden-focus record
without clearing the Engine seat or X frontend focus. A hidden but still
committed surface could therefore remain the keyboard target. Workspace-away
now issues a surface-scoped clear-focus control, waits for X authority
acknowledgement, clears Engine focus, and records the transition. A harmless
key typed before workspace return must be explicitly suppressed for lack of
focus. The physical verifier requires the ordered sequence: workspace away,
focus cleared, key suppressed, workspace returned.

Adding axis routing pushed the mutable route registry over the cohesion
threshold. Resolved-input selection and frozen-input draining now live with
the existing routing-input owner, returning the registry below 1000 lines
without changing its public facade.

## 2026-07-24: Installed Login Lifecycle Is An Evidence Boundary

The development TTY wrapper previously allowed `/tmp` as its runtime root and
did not distinguish build, input-guard, graphics-takeover, session, and return
phases in retained evidence. That was acceptable for bounded development but
could not prove an installed greetd login was independent of repository
fallbacks or identify the exact phase that failed.

The installed entry now declares its stricter contract explicitly: no source
build, no manual service control, an existing absolute user-owned
`XDG_RUNTIME_DIR`, and a real local Linux VT. The shared wrapper records ordered,
content-free lifecycle records from preflight through display-manager handoff.
A fixture-backed verifier rejects temporary runtime state, missing or reordered
phases, and emergency recovery presented as normal logout.

Normal, Firefox, and emergency recorders now retain that lifecycle beside the
release manifest, runtime identity, recovery, guard, and session evidence.
Repeated-cycle verification rechecks the lifecycle of every archived run.
This proves the installed-path contract and makes failures diagnosable; it does
not replace the remaining physical three-login, fallback-session, emergency,
ten-cycle, or soak captures.

## 2026-07-24: Installed Session Entries Follow Greetd Discovery

The first immutable lifecycle release installed its desktop entries below
`/usr/local/share/wayland-sessions`, but this host's explicit tuigreet command
scans `/usr/share/wayland-sessions`. The files were valid yet could not appear
in the menu. The system installer and current-release verifier now use
`/usr/share/wayland-sessions`, matching the configured greetd discovery
boundary. Staging tests continue to override the directory explicitly.

## 2026-07-24: Installed Kitty Baseline Remains Separate From xmonad

The first greetd-launched xmonad capture proved device discovery, 48 routed
keys, focused X authority, repeated Kitty Present retirement, and a responsive
hardware cursor, but the operator could not see usable terminal content. This
is not the earlier missing-device failure: the evidence points to an
xmonad-managed presentation/layout defect after the first configure.

The installed menu now exposes `Sophia Kitty (Baseline)` separately from
`Sophia xmonad (Experimental)`. Both use the same immutable binary, runtime
identity, VT guard, KMS renderer, X frontend, and cleanup path; only the opaque
session profile differs. Re-proving the known-good Kitty profile isolates the
shared installed/greetd boundary before changing xmonad layout behavior. It
does not promote or conceal the failed xmonad capture.

## 2026-07-24: XKB StateNotify Uses Post-Event Modifiers

The installed Kitty baseline exposed two keyboard gaps. First, Helix did not
recognize `:`. A strengthened real-Kitty smoke reproduced it: Kitty's keyboard
trace saw Shift press with no effective modifier, semicolon press as `;`, then
Shift only on semicolon release. Core key events correctly carry pre-event
modifier state, but Sophia incorrectly reused that state in XKB `StateNotify`,
whose effective modifiers describe the post-event state.

Routed key records now carry both values explicitly. Core and XI events retain
pre-event state while XKB notifications use the post-event state produced by
the per-seat xkbcommon machine. The real Kitty gate now types exact `:ll` and
requires shell receipt plus later Presents. A complete pc105 US symbol-table
regression covers printable base/shift pairs and F1 through F12.

Second, kernel VT shortcuts cannot operate while the graphical owner has set
the console keyboard to `K_OFF`; leaving translated console input enabled would
reintroduce typed bytes on the hidden TTY. The protocol-neutral session input
owner now recognizes Ctrl-Alt-F1 through Ctrl-Alt-F12, consumes the function-key
edges, and asks the controlling VT to activate the selected terminal. This is a
session-control action, not an application or X11 shortcut.

The first physical switch attempt exposed a launcher boundary: the graphical
owner is deliberately started with `setsid`, so `/dev/tty` is unavailable
inside it. The helper failed with `ENXIO`; treating that failed control action
as fatal correctly returned to greetd. The wrapper now passes the exact
originating `/dev/ttyN` as `SOPHIA_SESSION_TTY`, and the detached owner opens
that explicit device for VT activation. A launcher regression requires the
device handoff to precede `setsid`.

The next physical run refined that diagnosis: the path was correct, but
reopening `/dev/tty7` after detachment failed with `EACCES`. Device paths are
not durable capabilities across display-manager ownership transitions. The
launcher now duplicates its already-authorized controlling-TTY descriptor
before takeover and passes the descriptor number as
`SOPHIA_SESSION_TTY_FD`. Session-control helpers issue VT ioctls directly on
that inherited descriptor; the path remains only a compatibility fallback.
This keeps VT control in the session-control boundary without adding
terminal- or application-specific behavior to Engine.

The next physical run disproved descriptor inheritance as the final solution.
`VT_ACTIVATE` returned `EPERM`: Linux authorizes that ioctl by controlling-TTY
ownership, which the deliberate `setsid` boundary removes, rather than by the
mere possession of an open descriptor. Sophia then exited with status 1, so
the greetd screen observed after Ctrl-Alt-F3 was greetd reclaiming tty7; the
existing tty3 login had never become active.

VT switching now belongs to a libseat controller. Switch rejection is
nonfatal. A successful switch produces an explicit release boundary that
stops input, drains and releases native scanout, and acknowledges suspension;
acquisition rebuilds both hardware domains and repaints the retained scene.
Kitty, X11 clients, focus, and Engine state remain above that hardware
lifecycle, and Engine gains no application-specific branch.

The first physical return from tty3 exposed an incomplete authority boundary.
libseat delivered disable and enable correctly, but Sophia reopened
`/dev/dri/card*` with ordinary filesystem access after enable. AMDGPU rejected
the initialization with `EACCES`, and the session exited cleanly to greetd.
Login-session ACLs are not the device authority once libseat owns the session.

The live backend now runs the non-`Send` libseat handle on a dedicated broker
thread. KMS card and udev-libinput opens request libseat device leases; backend
objects receive duplicated descriptors while the broker retains and closes
the lease token. Suspension drops input and KMS resources before acknowledging
disable, and acquisition obtains fresh leases before rebuilding them. Direct
device opens remain available only to standalone validation paths that do not
participate in the managed live-session lifecycle.

Physical validation of the installed Kitty baseline then completed the missing
proof. Ctrl-Alt-F3 released the graphical seat and exposed the already-active
text login; the Sophia session remained alive. Repeated Ctrl-Alt-F7 returns
reacquired KMS and input, repainted Kitty, and preserved interactive keyboard
and pointer operation. Switching away again continued to work. This promotes
the libseat-backed Kitty session to the known-good installed baseline while
leaving the full F1-through-F12 matrix and xmonad workflow as separate open
proofs.

The first xmonad VT attempt exposed an ordering race hidden by the quieter
Kitty workload. The xmonad profile had a primary-plane frame in flight when
Sophia called `libseat_switch_session`. Seat authority moved before the owner
drained that frame, so its page-flip callback could no longer arrive. Waiting
500 milliseconds after revocation then failed with `persistent native scanout
remained in flight during teardown` and incorrectly ended the whole session.

Operator-requested VT changes now use a prepare-before-release boundary shared
by every profile. The owner stops input, prevents further native submission,
retires and releases KMS work while the seat is still active, drops the old
leases, and only then requests the switch. A request rejection or missing
disable event rebuilds hardware and repaints instead of ending the session.
An unsolicited disable cannot be drained after authority is gone; that path
immediately detaches native state, reports any abandoned scanout, completes an
already-submitted Present as `Skip`, and preserves queued work for acquisition.

Physical xmonad switching then proved KMS survival but exposed a separate input
state boundary. Ctrl and Alt presses had already reached the focused X client
before the function-key press identified the sequence as a VT chord. Their
physical releases occurred on the text VT, outside Sophia's libinput ownership,
so XKB and the WM shortcut router retained both modifiers after acquisition.
The reopened libinput poller was healthy; application input was interpreted
with stale Ctrl-Alt state.

VT activation now emits synthetic releases for every pressed chord modifier,
clears the WM seat state, and waits up to 500 milliseconds for X Authority to
acknowledge those deliveries before KMS quiesce and `switch_session`. Failure
to flush rejects the switch without ending the graphical session. Suspension
still clears local keyboard state as a second boundary, but no longer relies
on that local reset to repair client-visible XKB state.

The next physical xmonad capture isolated a distinct multi-output retirement
failure. Output 2 submitted its third startup frame but never produced another
page-flip callback; output 1 continued submitting and retiring normally. Both
VT attempts therefore reached the prepare boundary with one permanently
in-flight output and timed out before `switch_session`. Emergency shutdown hit
the same strict drain and returned status 1.

Native suspension now has one data-oriented result for both authority states:
whether all callbacks drained, how many scanouts were abandoned, and which
submitted Present was settled as a Skip. While authority remains active the
owner still attempts exact retirement first. A bounded timeout transitions to
the same detached runtime representation used after unsolicited revocation,
then drops the native leases before requesting the VT switch. Final teardown
uses this operation as well. This keeps the missing kernel callback observable
without allowing it to wedge VT control or emergency recovery, and avoids
duplicating detach/Present-settlement policy across lifecycle callers.

The following physical run proved that boundary: an owner-requested VT switch
timed out with one abandoned scanout, detached before release, resumed both
hardware domains, preserved the action-launched Kitty, and completed final
logout with a fully drained scanout. The remaining status-1 exit was unrelated:
two RANDR `GetOutputProperty` requests used atom `None` and correctly received
`BadAtom`, but the session counted those optional client probes as unexpected.

Protocol reduction now recognizes only the complete probe tuple (`BadAtom`,
RANDR `GetOutputProperty`, atom `None`) as expected. The client-visible reply
does not change, and an unknown nonzero atom remains unexpected. The physical
verifier also now distinguishes Sophia's structured failures from harmless
Kitty/GLFW stderr and uses xmonad's actual action identity `768` for
Super-Enter; action `1` remains focus-next. Mutation fixtures preserve each
distinction so the acceptance gate cannot silently regress to broad error
whitelisting or the former action-ID alias.

The next physical start exposed a false-positive readiness gate rather than an
application-launch failure. Kitty mapped, xmonad committed layout and focus,
and output 1 repeatedly retired mixed Present transactions. Output 2 submitted
its startup frame but never delivered a callback. Despite that partial KMS
state, the owner reported `status=ready`: the CPU fallback examined only the
first output, while the DMA-BUF path treated transaction retirement as visual
proof without inspecting the composed pixels. Completion later reported zero
CPU detail, no output-1 nonzero export, and no output-2 retirement.

Startup readiness now reduces flat evidence for every owned output and requires
at least one callback from each. Mixed composition captures a bounded one-time
GPU readback, carries its nonzero RGB count with the exact submitted
transaction, and becomes stable only after that content retires. A missing
output callback after 750 milliseconds, or retired mixed frames without visible
pixels after 1500 milliseconds, triggers one shared native detach/reopen and
repaint through the existing libseat authority. The eight-second deadline
remains authoritative; a second failure exits through guarded cleanup instead
of accepting a cursor-only desktop. Engine remains protocol- and
application-neutral.

## 2026-07-25: Per-Output Pending Content Is Required For Native Submission

The four-Kitty proof exposed a false clean-shutdown result. Output 2 accepted a
CPU commit after the startup terminal exited but never produced its final
page-flip callback. Logout therefore timed out and abandoned one scanout, while
the focused verifier accepted the detached runtime because its final
`native_in_flight` value was false.

The deeper error preceded logout. A primary-owned mixed Present queued content
only for the primary output but still executed the native scanout tick for
every output, producing repeated secondary submissions with `content=None`.
CPU composition also requeued the unchanged secondary marker whenever only
primary content changed. Native submission now requires explicit pending
content, primary mixed Presents service only their owning output, and typed
CPU-content reduction suppresses a frame only when the same CPU content is
already pending, submitted, or displayed. A mixed-to-CPU correction is never
suppressed by an old checksum.

Native suspension now reports a typed drained, timeout-detach, or revoked-seat
outcome. Forced detach remains a bounded liveness mechanism, but it is not
clean evidence. The four-Kitty verifier requires an exact drain, zero
abandoned scanouts, balanced per-output callbacks and retirements, and no
empty-content submission. Engine remains application-neutral; the CLI only
orchestrates and reports the Engine-owned lifecycle.

## 2026-07-25: Pending Layout Commit Merges Concurrent Surface Admission

The first run after per-output hardening reached the four-window transition,
then exited with `new WM surface is missing from live layout`. Rapid
Super-Enter actions admitted another Kitty while an older resize proposal was
waiting for matching pixels. Authority intake inserted the new surface into
the live layout and unmanaged set, but committing the older proposal replaced
the layout with its pre-admission snapshot. The unmanaged ID survived without
its layer, so the next manage request correctly rejected the inconsistent
state.

Pending layout snapshots now merge every authority observation not owned by
that proposal's requested-size set. A concurrently admitted surface is
preserved for the next blind-WM manage request, and ordinary pixel updates for
unrequested surfaces advance with the pending snapshot. Resize-owned surfaces
remain quarantined until their matching pixels arrive. The merge is a pure
data reducer with integration coverage for insert, replace, and resize-owned
outcomes.

## 2026-07-25: Output Baselines And Launch Pressure Are Separate Lifecycles

The next rapid Super-Enter run did not fail in Engine, KMS submission, or X11
protocol handling. Initial modeset had already displayed the synthetic marker
on the secondary output, but modeset itself produced no page-flip callback.
The new unchanged-CPU-frame reduction saw the same displayed checksum and
suppressed the first event-bearing flip. Output 2 therefore remained at zero
callbacks, startup never reached its all-output proof, and the eight-second
deadline ended the session while several action-launched Kitty clients were
entering resize transactions.

An unchanged displayed frame is now suppressible only after that output has
observed a callback. Before then, the reducer emits a baseline-required outcome
and queues exactly one nonblocking flip; matching pending and submitted frames
remain deduplicated. Startup readiness is a monotonic passive record pinned to
the startup surface rather than whichever later surface owns focus.

Application launch pressure is also bounded independently from visual
authority. The CLI session supervisor retains a sixteen-entry FIFO across
active and queued action applications, waits for one opaque surface admission,
matching pixel retirement, and a settled layout pipeline before spawning the
next, and treats capacity, spawn, exit, or admission timeout as an application
outcome rather than a fatal session error. Global scanout quiescence is not an
admission condition: continuously presenting clients may supersede frames
without invalidating the fact that the new surface was displayed. Logout
cancels pending work. Engine and the blind WM remain application-agnostic.

## 2026-07-25: X11 Controls Leave The Presentation Owner's Wait Path

The delayed native submission during the four-Kitty workload was an ownership
bug rather than a renderer throughput limit. The live owner sent X11
configure, rollback, focus, clear-focus, and close requests and then waited as
long as 500 ms for each acknowledgement. During that wait it could not poll
DRM retirement or service input.

Those requests now enter a fixed-capacity session-control ledger. Configure
and close controls may be in flight concurrently; focus and clear-focus are
serialized globally. Acknowledgements correlate on client, command kind,
transaction, and surface. Channel pressure leaves work queued for a later
owner tick, while rejection, timeout, disconnect, duplicate identity, and
unexpected acknowledgement fail closed.

Engine focus remains authoritative. Client focus becomes applied only after
the matching X authority acknowledgement. While Engine and frontend focus
differ, physical routing retains cursor motion, emergency exit, VT switching,
and WM shortcuts but suppresses client keyboard, button, and axis delivery.
The owner services controls at both tick boundaries and limits its authority
wait to one millisecond while controls remain pending.

The CLI emits a separate `sophia_live_session_control` record with balanced
lifecycle counts, peak depth, queue dwell, and acknowledgement latency. The
four-Kitty verifier requires a drained failure-free ledger and bounds both
latencies to 100 ms. Synchronous initial-modeset evidence also moved from the
backend library to the CLI evidence boundary, removing direct library output.

## 2026-07-25: Focus Suppression Must Preserve Key Symmetry

The first physical run with asynchronous focus controls completed cleanly, and
all 28 controls were delivered without rejection or timeout. Keyboard hardware
also remained active. The remaining apparent keyboard loss was an input-state
ordering defect: a key press could reach the old X client immediately before a
focus handoff, while its physical release arrived during the deliberately
suppressed Engine/frontend focus mismatch. The WM observed that release, but
the X client did not, leaving keys such as Super logically pressed.

The live input boundary now retains a fixed-capacity data record of key presses
actually delivered to each surface, seat, and device. Before a focus,
clear-focus, VT, seat-release, or logout handoff, it sends a release for every
record owned by the old client and includes those releases in normal delivery
accounting. A later physical release without a matching delivered press is
suppressed instead of being sent to the new client. Surface removal clears any
remaining record and updates the local modifier reducer, preventing stale
state from crossing a client exit.

Completion evidence reports peak pressed-key depth, synthetic releases,
suppressed orphan releases, surface-removal cleanup, and final debt. The
four-Kitty verifier requires final pressed-key debt to be zero.

The close-window physical proof refined the ordering requirement. X authority
owns one XKB reducer per seat, not one per surface. Clearing two local records
when Meta-Shift-C removed its surface left Meta and Shift pressed in that
seat-wide reducer, so the replacement Kitty inherited modified input. Control
dispatch is now held behind the exact synthetic-release delivery IDs. A close
or focus request cannot reach its X control writer until every preceding
release has been acknowledged by X authority. Completion and the physical
verifier require both the pressed-key ledger and this release barrier to be
empty, with no keys abandoned during surface removal.

Client-initiated exits add a different boundary: a terminal may destroy its
surface in response to Return before the physical Return release arrives.
There is then no live target for an X event, but the seat-wide XKB reducer must
still observe the release. Routed input now distinguishes ordinary
Engine-selected delivery from a state-only seat update. Surface removal emits
state-only releases for its residual key records; X authority updates XKB
without resolving a surface or emitting an event to the newly focused client.
This preserves global keyboard state without inventing an application target.

## 2026-07-25: Composition Reuse Requires A Lease-Aware Pool

The successful keyboard proof still reported 201 composition-target and GL
pipeline creations for 201 mixed exports. CPU and direct DMA-BUF paths already
returned their persistent target to the renderer after exporting a locked
front buffer. The mixed-composition path instead destroyed its target on every
successful export even though the exported buffer retained an `Arc` lease on
the GBM/EGL surface until scanout retirement.

An attempted optimization returned the context, surface, and GL pipeline to
the per-output target slot after a successful export. The exported front
buffer independently retained the originating surface, and the verifier was
temporarily changed to require zero target recreations.

The first physical run of this lifetime change aborted on the third render
after AMDGPU rejected the command stream. Moving startup proof from a
post-swap CPU map to a pre-swap GL readback produced the same third-render
abort, disproving front-buffer mapping as the root cause.

The invalid lifetime is single-surface reuse while a front buffer from that
GBM surface remains leased to KMS. Mixed composition therefore returns to the
previous fail-safe rule: destroy its rendering target after each successful
export while the exported buffer's independent surface lease survives through
scanout retirement. Future reuse must use a bounded lease-aware pool and only
select a target whose surface has no exported buffer owners. Verbose tracing
still captures one representative composition frame instead of synchronously
reading every frame.

## 2026-07-25: Startup Evidence And Input Waits Follow Their Owners

The first clean run after restoring fail-safe composition lifetime survived
201 mixed frames and completed with balanced callbacks and cleanup. Its strict
verifier still found two evidence and latency defects.

Per-output synchronous-modeset records were printed before
`LiveProductionVisualRuntime` initialized native scanout, while every head
still carried an empty initial-modeset state. The later aggregate
`output_baseline_ready` record observed the correct state, but the detailed
records had already been skipped. Native heads now retain the exact initial
submission identity, and the CLI emits each detailed record at the same
readiness transition as the aggregate record.

The physical input worker dispatch gap remained one millisecond, but an event
could enter its queue immediately after the owner drained input. The owner then
waited as long as 25 ms for X authority work before a composition taking as
long as 75 ms, producing 120 ms of measured queue dwell. Physical-input
presence now selects the existing one-millisecond owner wait budget. Cursor
and control work retain that same budget, while sessions without those
latency-sensitive sources keep the 25 ms idle wait.

## 2026-07-25: Retained Contexts Across Fresh Surfaces Are Unsafe

The fail-safe mixed path bundled four lifetimes into one target: EGL context,
GL pipeline, EGL window surface, and GBM surface. Destroying that bundle after
every successful export protected the leased front buffer but also rebuilt the
context and shaders on every frame. This explained the repeated composition
creation count and left input and page-flip latency coupled to driver setup.

The attempted lifetime split created a distinct GBM/EGL surface for every
export while retaining the EGL context and GL pipeline. Physical startup
presented the first two mixed submissions successfully. The third render then
aborted with `amdgpu: The CS has been rejected ... (-2)`. This falsifies the
assumption that avoiding same-surface reuse alone is sufficient on this stack:
the context and pipeline cannot safely be rebound across independent leased
window surfaces either.

The fail-safe path again destroys the composition context and pipeline after
every successful export while the exported buffer retains its surface through
KMS retirement. Resource evidence still distinguishes complete-target and
frame-surface creation so this invariant is machine-checkable. The next
optimization is a bounded generational pool of complete targets, not a pool of
surfaces behind one context. A slot may become free only through explicit
page-flip retirement, never through reference-count inference.

## 2026-07-25: One-Shot Targets Restore Physical Stability

The first physical cycle after removing every retained GPU target completed
normally with 253 mixed exports. Target, pipeline, frame-surface, and successful
target-retirement counts were all 253. Generation and recovery replacement
were zero, no AMDGPU command stream was rejected, native submissions and
callbacks drained, and the lifecycle returned exit status zero.

The strict gate exposed a separate owner-scheduling defect. Of 254 callback
observations, 248 completed within 20 ms and three exceeded 100 ms. Two
213--214 ms stalls coincided with managed terminal exit and xmonad resize
epochs; a 171 ms stall covered the final blank frame after the startup terminal
exited. Target creation peaked at 4 ms and rendering at 47 ms, while input
queue dwell reached 191 ms. The next fix must prioritize native callback and
input draining across child-exit and layout-transition work. The latency budget
must not be relaxed to hide these isolated starvation events.

## 2026-07-25: Native Retirement Precedes Shortcut RPC

The physical owner previously routed input before servicing native retirement.
A global shortcut could therefore enter the external WM transport's synchronous
request path, whose configured response timeout is 500 ms, while an accepted
KMS callback waited in the native event queue. The same wait could accumulate
physical input in the acquisition queue.

Native service now precedes shortcut routing. Completion evidence separately
records maximum child-reap, input-routing, and WM-request durations, and the
four-Kitty verifier caps each at 100 ms. This is an ordering correction, not a
claim that synchronous WM transport is suitable long term. If the next
physical evidence attributes the remaining input dwell to WM request time, WM
actions move to a bounded typed worker with explicit response correlation and
stale-response rejection.

## 2026-07-25: WM Socket Wait Leaves The Physical Owner

The next two-output, four-Kitty hardware cycle completed without a crash,
AMDGPU rejection, resource replacement, or cleanup debt. It recorded a 180 ms
maximum external-WM request, a 100 ms physical-input phase, 246 ms input queue
dwell, and a 210 ms submit-to-page-flip observation. Child reaping peaked at
25 ms. The correlated peaks identify synchronous WM socket waiting as the owner
starvation source; renderer and target lifetimes remained stable.

WM transport now uses a capacity-one worker channel behind a sixteen-entry
owner request bound. Exactly one packet is in flight. Passive request and
completion records carry the transaction ID; the owner correlates them and
rejects a response when the current layout topology or geometry no longer
matches the request fingerprint. Surface removal remains serialized before its
relayout so the latter is planned from post-removal workspace state. The owner
alone validates and commits proposals and applies focus, workspace, launcher,
close, and logout effects. A neutral empty coordinator batch lets a WM-only
transaction reach Engine without waiting for unrelated X11 traffic; it is not
counted as an X authority batch.

Completion now reports owner timing separately from WM transport depth,
rejections, stale responses, queue dwell, and round-trip latency. The external
round trip retains its 500 ms fail-closed bound; it is no longer misclassified
as owner-thread execution time.

## 2026-07-25: Asynchronous WM Physical Gate Passes

The first two-output four-Kitty cycle after moving WM socket waits off the
owner thread completed normally. Maximum physical-input phase time fell from
100 ms to below the millisecond evidence resolution, input queue dwell fell
from 246 ms to 12 ms, and submit-to-page-flip observation fell from 210 ms to
23 ms. The WM transport reached a 101 ms round trip without holding the owner;
its peak depth was one and it drained with zero rejection, stale response, or
pending request.

The session produced 220 mixed exports with exactly 220 complete targets,
pipelines, and frame surfaces, zero recovery or generation replacement, clean
callback retirement, and clean input, control, protocol, and process teardown.
The fourth-window transition atomically held and committed four surfaces
because xmonad promoted the newly focused window to master. The verifier had
assumed the old master remained fixed and required exactly three changed
surfaces. It now correlates the held transaction with its matching commit and
accepts either three or four changed surfaces while retaining the exact
pixel-matched four-pane geometry checks.

## 2026-07-25: Cursor And Primary KMS Commits Must Be Serialized

The next physical cycle ran the full four-Kitty workload but failed promotion
after one early primary-plane atomic submission was rejected. The failure
followed physical pointer motion; every later submission recovered, 182 mixed
exports retained exact complete-target lifetime, native drain completed, and
the final failure count remained exactly one.

The owner ordering exposed a KMS transaction race. Native service could submit
a nonblocking primary flip, then the cursor path independently issued a
nonblocking cursor-plane commit on the same card. A successful cursor ioctl
only admitted that asynchronous commit; it did not prove the cursor update had
completed before the next owner tick submitted another primary request. The
earlier cursor-side `EBUSY` handling covered only the opposite ordering, where
a cursor update encountered an existing primary commit.

Backend-live now treats primary page-flip state as the admission boundary for
cursor work. A dirty cursor remains coalesced while any primary flip is in
flight. Once admitted, the cursor-only atomic commit is blocking, so it cannot
remain pending across the next primary submission. Completion evidence records
both primary-in-flight deferrals and maximum cursor update duration, and the
physical verifier caps the latter at 100 ms. This is the bounded daily-driver
repair. The long-term graphics path should build primary and cursor plane state
through one per-output atomic KMS transaction owner rather than preserve two
independent commit builders.

The required xmonad real-client preflight exposed an independent proof
regression before the next physical run. Its headless configuration has one
virtual output and intentionally disables native scanout, but startup readiness
waited for the native-only `OutputsPresented` event. The owner now satisfies
that output fact only when native scanout is absent; physical sessions still
require every real output's callback or synchronous modeset evidence. The
preflight verifier also correlates the injected resize transaction directly,
instead of assuming initial placement and the later 960x640 configure collapse
into one asynchronous WM response. The corrected preflight passes with matched
configure delivery, later pixels, exact synthetic input, and clean teardown.

## 2026-07-25: Workspace Policy Must Project Visibility Into Every Consumer

The post-cursor physical cycle validated the KMS repair with zero native submit
failures and a 12 ms maximum cursor update, then exposed an independent
workspace defect. Super-2 committed workspace policy and cleared focus, but the
owner continued presenting all four workspace-1 surfaces. The private xmonad
server also kept their synthetic windows mapped. Launching Kitty on workspace 2
therefore made xmonad tile five cross-workspace windows; all five resize
requests timed out, rollback rejected four queued Presents, and logout later
detached one unretired scanout. Shortcut IDs remained distinct: the log recorded
workspace actions 257 and 258, application action 768, close action 769, and
logout action 772.

Workspace focus is now stored by workspace and projected onto the outputs that
currently display it, so hiding a workspace clears client focus without
destroying the focus that should return with it. The live owner filters
presentation layers through Engine's workspace visibility before composition;
the same order bounds hit-testing and retained mixed layers. Relayout requests
contain only nodes assigned to the active workspace. The blind xmonad bridge
tracks its active workspace and mapped synthetic-window set, issuing explicit
unmap/map transitions so hidden windows cannot influence legacy layout policy.
CPU composition consumes the same ordered visible-surface projection, including
the empty-workspace case. A Present targeting a surface outside that projection
settles as a skip before any native submission; it cannot append itself back
into the mixed frame. These are protocol-neutral state projections; no terminal
or application identity enters Engine or rendering.

The first physical workspace run then proved that filtering composition is not
enough by itself. Super-2 committed action 258, cleared the focused client's
keys and focus, and stopped accepting its Presents, but submitted no replacement
KMS frame. The previously scanned-out workspace therefore remained visible
while keyboard routing was correctly disabled. The CPU-cycle preservation rule
had examined every committed DMA-BUF surface rather than the visible projection
and suppressed the empty-workspace repaint.

GPU preservation now reduces only the ordered visible transaction set. A
visibility-order change cannot be discarded by ordinary CPU coalescing. An
empty projection queues a black CPU frame; a returning projection queues its
retained mixed DMA-BUF frame when available, and otherwise paints the bounded
CPU background until the client supplies new pixels. Thus a workspace commit
always has a concrete scanout consequence instead of leaving the old workspace
on screen.

The follow-up physical cycle exercised workspace 1 seven times, workspace 2
nine times, and workspace 3 twice. Empty projections submitted the stable
blank CPU frame; populated projections restored their retained mixed frame and
focus. The run recorded no layout timeout or resize abort, zero native submit
and retirement failures, a clean native drain, and bounded completion after
122 submissions and 120 asynchronous retirements. The operator confirmed all
three workspaces visually.

Future captures no longer rely on that visual statement alone. Every committed
WM policy state now emits a reduced projection record containing only
transaction, output, workspace, visible-surface count, and whether focus is
present. The strict verifier requires workspace 2 and 3 to commit empty,
requires workspace 1 to return with visible surfaces and focus, and correlates
the focus-clear, suppressed-key, workspace-3, return, and focus-restore order.

The first capture with projection schema 2 populated Kitty independently on
workspaces 1, 2, and 3, committed 25 layouts without a timeout, and closed one
workspace-2 client without disturbing the others. It completed with zero
native submit, retirement, callback, control, or protocol failure. The control
ledger drained 22 enqueued and delivered commands with 17 ms maximum queue
dwell and 14 ms maximum acknowledgement latency; the input ledger drained
2,258 events with no pending key state.

This was a valid workspace and control-ledger proof, but not the complete
standard workflow. The startup Kitty remained open, and the capture contained
no click/drag button route, focus-next action, next-layout action,
hidden-workspace key suppression, or VT lifecycle. The strict verifier
correctly stopped at the missing startup exit instead of treating a clean
logout as evidence for steps that were not performed.

## 2026-07-25: Core Selection Ownership Must Be Queryable

The next physical cycle covered the normal interaction set in a different
order. Mouse selection worked, but Ctrl-Shift-C and Ctrl-Shift-V did not.
Kitty's GLFW layer repeatedly reported that it failed to become owner of the
clipboard selection. This distinguishes the defect from physical key routing:
Kitty received the copy chord and attempted the X11 ownership transition.

The frontend accepted `SetSelectionOwner` into its namespace-aware selection
table, but core `GetSelectionOwner` unconditionally replied with `None`.
GLFW performs the standard set-then-query ownership check and therefore
correctly treated every copy as failed. Core owner queries now return only the
owner visible in the caller's admitted namespace. Classic shared-X clients see
the shared owner, while a confined client cannot discover an owner in another
namespace. The wire regression covers the visible and confined cases.

Normal-session evidence now reports only owner-change and conversion counts;
clipboard content remains redacted. The strict physical gate requires both
operations, rejects GLFW ownership failures, and retains operator confirmation
that the selected text was pasted unchanged.

The first retest acquired clipboard ownership twice with no GLFW ownership
failure, and the operator confirmed that paste appeared to work. It produced
no `ConvertSelection`, however, because the copy and paste occurred inside one
Kitty process; Kitty can reuse its locally held selection without a protocol
round trip. The promotion sequence now pastes into an independently launched
Kitty before the owner exits, so the conversion witness measures the intended
same-namespace X11 path.

That retest also exposed an independent pointer-boundary defect. After paste,
the hardware cursor disappeared. Input remained healthy and the final record
showed 160 successful cursor-plane updates with zero failures, but only 1,202
of 8,158 observed pointer events routed to a surface. Session pointer placement
had applied the libinput accumulator plus its startup offset without confining
the result to any Engine output. The KMS cursor owner deliberately detaches the
cursor plane when given a point outside every output, so that reachable path
matches the reported disappearance without producing a hardware failure.

Engine now provides one output-union confinement system. The live session
projects its existing ordered output topology into that system before physical
input starts. Confinement chooses the nearest valid point across all output
rectangles, including unequal output heights, and corrects the raw-to-logical
offset at the edge so discarded overshoot does not create a sticky boundary.
Integration coverage drives positions past every side and into the dead area
beside a shorter output. Completion evidence now counts intentional hidden
updates separately from successful updates and failures; the strict gate
requires zero. Physical edge/reversal confirmation remains pending.

The first confinement repair still left its accumulated raw position, startup
offset, corrected edge offset, and current logical position in the CLI owner.
That was the right behavior in the wrong authority: the CLI could effectively
choose Engine cursor state, and the confinement helper alone could not prove
that a real input stream reversed immediately after overshoot.

`OutputUnionPointerState` now owns that complete state machine inside Engine.
The live owner supplies only immutable output rectangles, the optional initial
surface geometry, and each raw backend point. Engine returns a logical
placement plus reduced boundary contact/reversal facts; no backend handle,
device identity, client metadata, or pointer coordinate is logged. Deterministic
coverage proves all four edge directions and unequal-output projection.

The rebuilt two-output `xmonad-m7` guest then drove the real virtio-mouse path
hard against the right edge and sent one 96-unit reverse delta. Engine emitted
an output-edge contact followed by an immediate-reversal observation, after
which the complete click-drag focus, keyboard, workspace, bridge-restart,
launch/close/logout, and native-drain workflow passed with zero protocol,
cursor-plane, stale-WM-response, or cleanup failure. This is unattended
evidence for the state machine; the physical gate still requires every edge of
the actual output union and visible hardware-cursor confirmation.

## 2026-07-25: Held-Key Repeat Belongs To Engine Timing And Frontend Semantics

The next physical run retained ordinary keyboard routing but exposed that
holding Backspace or an arrow produced only one action. This is expected from
libinput: it reports physical press and release edges; the display stack must
schedule held-key repeat. Sophia had no such scheduler.

Engine now owns a fixed-capacity, allocation-free repeat clock with one active
repeatable key per seat. The live owner binds that record to the exact focused
surface, seat, device, and physical key. The configured XKB map determines
repeatability, so editing and cursor keys repeat while modifiers and Super do
not. Existing focus, workspace, surface-removal, and VT release barriers cancel
the bound record; a repeat can never migrate to a newly focused client.

The X frontend receives an explicit repeat delivery mode. It emits another
KeyPress with current XKB modifiers without replaying a physical state
transition or reactivating a passive grab. This keeps Engine timing
protocol-neutral, preserves X11 delivery authority, and prevents xmonad global
shortcuts from repeating. Missed timer intervals coalesce to one pulse rather
than bursting after a slow frame. Completion evidence requires the scheduled
and routed counts to match, capacity exhaustion to remain zero, and every seat
to drain.

The first physical capture routed and acknowledged 66 held-key pulses with
zero missed-interval coalescing and zero repeat-seat capacity exhaustion. The
operator confirmed the editing/navigation behavior, and logout left no active
repeat seat or pressed-key debt after all 1,289 expected input deliveries
flushed. The run retained clean input, cursor, protocol, renderer, KMS, and
frontend teardown. It did not complete the broader promotion sequence: the
startup Kitty remained open and the clipboard peer performed no
`ConvertSelection`, so those independent gates remain pending.

## 2026-07-25: Override-Redirect Is A Presentation Role, Not WM Metadata

Unmodified xmobar creates its bar with `CWOverrideRedirect`, requests top
geometry, publishes dock/strut properties, and maps the window. Sophia
previously discarded that attribute, reported it as false in replies and
events, and admitted every rendered surface to the blind xmonad policy stream.
That would tile a status bar as an ordinary application even if all of
xmobar's drawing requests succeeded.

The X frontend now owns the exact override-redirect bit in its passive window
record and returns it through core X11 semantics. Across the authority
boundary it reduces the bit to `SurfacePresentationRole::ClientPositioned`.
The live Engine layout retains client geometry, composition, hit testing, and
top presentation for that role, while withholding the surface from WM
management. No XID, class, title, atom, dock type, or application identity is
sent to xmonad.

The xmonad TTY launcher can now discover an installed xmobar or an executable
built from the operator-selected source checkout, then supervise it as a
secondary X client with a deterministic local config. Xmobar and xmonad remain
unmodified compatibility clients. This first slice intentionally overlays the
bar: interpreting `_NET_WM_STRUT_PARTIAL` as protocol-neutral output
reservations is the next work-area step and requires retained physical
evidence before promotion.

The first real-client trace exposed two generic drawing gaps rather than an
xmobar-specific condition. Xmobar's Cairo backend uses MIT-SHM `GetImage` and
`PutImage` against an offscreen pixmap, followed by core `CopyArea` into its
window. Sophia initially omitted `GetImage` and then acknowledged pixmap
`PutImage` while discarding its bytes, so the final window transaction was
blank. Pixmap dimensions and software pixels are now passive authority data;
upload, readback, and copy use the same drawable buffer path as other software
clients. The unmodified xmobar 0.51.1 smoke subsequently completed 163 requests
across 28 opcodes with two committed transactions, 8,967 nonzero pixel bytes,
and `first_error=none`. The source checkouts for xmobar and xmonad were not
modified.

The first physical session then proved that xmobar stayed supervised, retained
its client-positioned role, and continued publishing nonzero CPU buffers, but
the bar was not visible above Kitty. The defect was in the generic mixed
renderer boundary: all CPU surfaces were flattened into one output-sized
background before the current and retained DMA-BUF layers were appended.
Flattening discarded the per-surface ordering needed for a CPU overlay, so the
later Kitty layer covered valid bar pixels.

Mixed presentation now snapshots CPU surfaces as passive
surface/geometry/buffer records and reduces the Engine presentation order into
one interleaved CPU/DMA-BUF layer sequence. The scheduler owns that immutable
snapshot with each queued Present; the renderer remains unaware of xmobar,
Kitty, X atoms, or window roles. Reducer regressions cover both a CPU overlay
above GPU clients and an ordinary CPU client below the current GPU surface.
The xmobar smoke was also strengthened to require six committed transactions
and nonzero pixels in the newest redraw; the retained run completed 215
requests across 28 opcodes with 25,929 nonzero bytes and no protocol error.
Corrected physical visibility remains pending, and neither client source tree
was modified.

The next physical run rendered the status bar, confirming the corrected
surface order, but Kitty never became visible. Kitty did start, registered
three DMA-BUFs and fences, submitted 19 Presents, and Super-Enter committed a
`LaunchTerminal` action. All 15 attempted mixed native submissions failed
before KMS with `ScanoutExportFailed`, after which the startup focus-control
gate timed out.

The remaining defect was a stale renderer invariant. Its persistent CPU
texture was allocated at output size and `draw_cpu_layer` rejected every other
extent because the previous mixed seam supplied only a flattened full-output
background. A correctly represented status bar is instead a narrow CPU layer.
The native pipeline now tracks the texture's allocated extent, reallocates it
only when the next CPU layer differs, and uses the existing sub-image fast path
for same-sized redraws. This supports arbitrary application-agnostic CPU
layers without allocating a new texture for each bar update. A passive reducer
test locks the reallocate-versus-update decision; corrected physical
bar-plus-Kitty presentation remains pending.

## 2026-07-25: Work Areas Are Engine Policy Facts, Not Dock Metadata

The corrected mixed renderer made xmobar and Kitty visible together, then
exposed that Kitty still began at the root origin beneath the bar. Rendering
was healthy: the run completed 141 mixed exports with matching target,
pipeline, and frame-surface lifetimes and no native failure. The missing
mechanism was layout reservation, not another renderer special case.

The X frontend now decodes exact CARDINAL `_NET_WM_STRUT_PARTIAL` and legacy
`_NET_WM_STRUT` properties in client byte order. Valid partial data takes
precedence; legacy data is the fallback. The authority emits complete,
bounded `SurfaceOutputReservations` replacements keyed by Sophia `SurfaceId`.
Atoms, XIDs, dock types, titles, classes, and application identity stay inside
the frontend. Malformed values remain legal X properties but produce no
reservation.

Engine owns the lifecycle table and pure work-area reducer. Only mapped
`ClientPositioned` surfaces are active. Replacement, deletion, unmap, and
surface removal update the table; same-edge depths take the maximum, different
edges combine, and partial root spans clip independently against each output.
An empty aggregate is rejected so the session preserves its last valid work
area. The full output remains available for composition and pointer hit
testing.

The live WM owner now stores reduced bounds in `WmWorkspaceState` without
changing workspace or focus state. Manage and relayout requests use that
stored rectangle, and the existing WM compatibility bridge forwards it as its
synthetic root bounds. Consequently the implementation is generic across
native WMs and legacy profiles; neither Engine, renderer, live session, nor
bridge contains an xmobar, xmonad, Kitty, or toolkit condition. Physical
bar-plus-Kitty geometry and lifecycle evidence remain the promotion gate.

## 2026-07-26: External WM Focus Ownership Starts Before Reconciliation

The guarded TTY3 pointer-focus run physically confirmed both plain-click focus
and click-drag focus. Each selected the intended Kitty, cross-window copy/paste
worked during the drag workflow, and Engine-owned focused borders made the
handoff visible. Workspace transitions exposed a separate transient: the
status bar briefly received a focused-border outline before the empty
workspace settled.

The defect was in generic initial-focus reconciliation, not xmobar or xmonad.
After external policy cleared Engine and X11 focus, the owner selected and
focused the first committed surface before checking whether an external WM
owned focus policy. It then returned without applying matching client focus.
The result was a hidden Engine focus, a one-frame compositor border, and
`control_plane_only` input suppression because Engine and frontend focus no
longer agreed.

Initial-focus candidate selection now rejects external-WM sessions before any
Engine mutation. A deterministic regression supplies a committed hidden
surface and requires no candidate. The two-output `xmonad-m7` guest then
switched to an empty workspace, recorded `focus=none`, suppressed both primary
button edges with `reason=no_target`, emitted no pointer-focus request or
client delivery, returned with Super-1, preserved focus through one
compatibility-bridge restart, and completed the independent click and drag
proofs plus clean logout. Reduced policy-suppression evidence remains available
for diagnosing future Engine/frontend focus transitions; it carries only mode
and counts.

The follow-up physical session exercised 36 workspace projections. Ten empty
projections retained `focus=none`; 26 populated projections restored focus.
No focused-border composition occurred between an empty projection and its
next legitimate focus restoration, no pointer button was suppressed by the
focus-transition policy, and the session completed with clean protocol, WM,
input, native-scanout, frontend, and namespace state. This closes the transient
status-bar border regression without adding client-specific chrome policy.

## 2026-07-26: Private Legacy-WM Displays Are Leased One-Shot Endpoints

A physical clipboard run exited during startup after the compatibility bridge
reported `no private X display available` ten times. The frontend display was
healthy; the failure was isolated to the bridge's synthetic X facade. Earlier
forced and aborted sessions had left owner-only socket nodes for every display
in the bridge's original `:90..:199` allocation range. Unix socket names remain
occupied after an ungraceful process exit, and the allocator treated each stale
name as a live policy endpoint.

The bridge now separates display-number ownership from the one-shot socket
name. A process-scoped file lease serializes each bounded display number, the
allocation range extends through `:4095`, and the socket name is unlinked as
soon as the configured legacy WM connects. The established Unix connection
continues to carry the synthetic X protocol, while a later abort has no socket
path left to leak. The lease remains held until the WM child and bridge worker
are stopped, and the kernel releases it even if the bridge is killed.

An integration regression launches two isolated fake legacy-WM processes at
once, requires distinct leased display numbers, and verifies that neither
accepted endpoint remains in `/tmp/.X11-unix`. This lifecycle belongs entirely
to the optional legacy-WM adapter; Engine, the real X frontend, renderer, and
blind WM protocol remain unchanged.

The immediate physical rerun confirmed the lifecycle fix. Sophia reached a
focused presented Kitty in 1,012 ms, entered workspace 3, launched two
independent peers, and completed 14 WM requests without a bridge restart or
degraded interval. The same run observed two selection-owner changes and one
selection conversion with content redacted. The operator confirmed that the
exact token was copied from the workspace-3 Kitty and pasted into the
independent workspace-1 Kitty after the workspace transition. The run flushed
all 1,397 expected input deliveries, retired native presentation without a
failure or live fence, logged out normally, restored the TTY exactly, and
revoked the frontend namespace. This closes the physical same-namespace
cross-workspace clipboard gate.

## 2026-07-26: Configuration Is Two Ownership Domains, Not an Override Stack

Sophia now resolves one strict KDL 2 source for session/Engine mechanism and a
separate source for Sophia-native WM policy. The user defaults are
`${XDG_CONFIG_HOME:-$HOME/.config}/sophia/config.kdl` and `wm.kdl`;
`/etc/sophia` and compiled snapshots are ordered fallbacks. There is no
include graph, field merge, KDL 1 fallback, or WM override of Engine policy.
External WMs continue to use their native configuration.

The new `sophia-config` crate contains only bounded passive schema data,
discovery, parsing, snapshots, deltas, last-known-good state, and a
parent-directory inotify source. Atomic editor replacement is therefore
observable without watching a stale inode. Unsafe ownership/mode, files over
one MiB, unknown or duplicate fields, broken references, invalid paths, and
the emergency chord fail closed.

Core candidates apply as one transaction. Application registry, repeat,
fallback chrome, and diagnostic changes are live-safe; mechanism changes mark
the complete candidate pending restart and do not leak its live-safe subset.
The session owner waits for an idle key ledger before replacement. Renderer
entry points now consume the Engine border style stored by the visual runtime
instead of recreating a default at each composition call.

The WM API advances to version 5. Negotiation carries a nonzero policy
generation and bounded chrome preference, while Engine continues to own
geometry, damage, rendering, and scanout. Generation-ordered update/ack
packets and an idle-shortcut reducer establish the hot-update contract.

The supervised transport now completes that contract. The socket worker
forwards immutable unsolicited candidates to the Engine owner and returns the
owner's exact-generation acknowledgement; it never applies policy itself. The
WM suspends new-policy request service until acknowledgement. Both ends also
handle the race where a bounded Engine request is already in the socket: the
worker accepts the intervening policy frame, the WM holds the request, and the
response follows the applied acknowledgement. Socket integration coverage
exercises atomic file replacement, generation delivery, request deferral, and
acknowledgement ordering.
## 2026-07-26: fixed-extent Vulkan smoke exposed a stale-control teardown race

- Physical `vkcube --wsi xcb` evidence advanced far enough to create and frame
  the fixed-extent surface, then the session exited with
  `X11 route targets unknown client 3`.
- The final records show a pointer focus request for the new surface followed
  by the fatal route error. The client worker had disconnected between the
  Engine focus decision and the bounded control-broker delivery.
- A disappearing application is normal frontend lifecycle, not an X authority
  failure. The broker now returns a distinct `ClientGone` control acknowledgement
  for that race, and the live owner retires that stale target without ending
  the graphical session. Backpressure and registry corruption remain fatal.
- The physical run also created a blank frame without a vkcube presentation.
  The dedicated proof launcher now enables redacted X11 and Present tracing so
  the next run can distinguish client-side exit, rejected Present validation,
  and feedback delivery without adding application-specific engine policy.

## 2026-07-26: declared constraints must fence blind-WM proposals before configure

- The traced fixed-extent run completed cleanly and proved that all Present
  rejection paths emitted Complete/Skip, Idle, and actual xshmfence signals.
  Vkcube registered three DMA-BUF images and six fences but stopped submitting
  after Sophia initially configured its fixed 500x500 surface as a 1276x1422
  tile and later recovered it.
- `WM_NORMAL_HINTS` is advisory and xmonad's default tiled layout does not
  enforce it. Treating an external WM proposal as authoritative client size
  therefore lets WM policy violate application constraints.
- `LayoutEpochCoordinator` now reconciles content geometry and configure sizes
  against Engine-owned declared constraints before control delivery. Placement
  remains WM-selected, but constrained extents are clamped and kept inside the
  output work area. Impossible constraints are rejected explicitly.
- This is protocol-neutral Engine policy: it applies to every WM bridge and
  fixed/min/max constrained surface, with no Vulkan, vkcube, or xmonad identity
  in the decision.

## 2026-07-27: truthful X map state is required before deferred admission

- A guarded xmonad run started Kitty and xmobar, but no managed surface became
  focused. Super-Enter queued another launch immediately before the startup
  watchdog exited at `stage=not_focused`.
- The isolated real-Kitty authority probe reproduced the failure without DRM,
  a WM, or a display manager. Sophia observed Kitty's created
  `PolicyManaged` windows and Present buffers, but no map intent.
- `GetWindowAttributes` had always reported every known window as viewable.
  Kitty trusted that reply and omitted `MapWindow`, so the new deferred
  admission protocol had no lifecycle edge from which to emit its request.
- X authority now derives the reply from its stored `XMapState`: created and
  policy-pending windows report unmapped, and only admitted/mapped windows
  report viewable. The real-Kitty probe has a deferred mode that requires one
  map intent, one delivered `AdmitSurface`, continuing Present feedback,
  delivered focus, and consumed routed text.
- The corrected probe passed end to end, and the full offline all-feature
  suite passed. Physical xmonad/vkcube verification remains open.
- The contemporaneous elogind diagnostic was unrelated. Session 193 was the
  valid online greetd `_greeter` session on tty7, not a stale Sophia session.

## 2026-07-27: recovery epochs must preserve admission ownership

- The first physical run after exact queued-Present ownership reached healthy
  KMS output and committed Kitty's recovery layout, but retired zero Kitty
  Presents and exited through the eight-second `not_focused` startup watchdog.
  This was a bounded session failure, not a renderer or kernel crash.
- The initial layout epoch timed out after the frontend had acknowledged
  admission, leaving the surface correctly in `AwaitingPixels`. The retry
  epoch staged and committed the retained pixels, but classified only a fresh
  `PolicyPending` surface as admission-owned. It therefore left both the
  transaction and its Present submission permanently in pre-admission
  quarantine.
- Surface-control staging now treats `ControlPending` and `AwaitingPixels` as
  continuing phases of the same admission. A retry may deliver the necessary
  configure, but its atomic commit also marks the surface managed and releases
  the retained transaction and Present exactly once.
- An all-feature regression reproduces an acknowledged admission entering a
  recovery transaction and requires the retry's finalization set, managed
  transition, transaction release, and Present release.

## 2026-07-28: standalone Vulkan isolated unresolved software Present

- The first standalone `vkcube --wsi xcb` run passed natural-layout admission,
  configure, and focus, then timed out at `no_visual_detail`. Vulkan selected
  llvmpipe. Sophia selected a 500-by-500 `XPixmap` candidate while recording
  zero DMA-BUF registrations and zero Present submissions, so no renderable
  storage could reach composition or KMS.
- A raw X pixmap is not a renderer buffer. The X authority now materializes
  regular software pixmaps into immutable CPU snapshots at Present time. It
  also retains MIT-SHM pixmap bindings and snapshots client-owned shared pixels
  at the same transactional boundary; DRI3 remains the zero-copy path.
- Unresolved pixmaps now fail closed instead of becoming false
  `PresentedBuffer` evidence. Complete-presentation semantics travel as a
  passive, protocol-neutral surface observation independent of CPU or DMA-BUF
  storage, preserving the admission reducer's distinction between a submitted
  frame and an accumulated backing image.
- The whole-pixmap SHM copy is the bounded correctness fallback. Damage-scoped
  persistent mappings remain an explicit post-proof optimization.

## 2026-07-28: software Present feedback must follow composed KMS retirement

- The first materialized software-Present run made the llvmpipe cube visible,
  but it remained on its first frame. The log contained one authority visual
  transaction and a retired nonzero CPU scanout, while Present Complete, Idle,
  and idle-fence-trigger counts all remained zero. The client was correctly
  waiting for permission to reuse its software pixmap.
- Presentation lifetime is now independent of storage kind. A software
  Present carries only its transaction, surface, and optional acquire/idle
  fence handles across the passive authority and production records. It owns
  no fabricated DMA-BUF handle.
- The production runtime registers that lifetime beside the CPU transaction,
  marks it submitted only when the composed primary frame reaches native KMS,
  and emits Complete followed by Idle after the matching page-flip retirement.
  Headless composition settles at its deterministic submission boundary.
- Focused regressions require source-free presentation retirement, actual idle
  fence signaling, authority-to-production observation preservation, and
  Complete-before-Idle routing. The physical verifier now rejects a visible
  but static software frame by requiring at least three authority frames and
  positive Complete, Idle, and idle-fence evidence.

## 2026-07-28: admission release must replace every storage form exactly once

- The first physical run with software feedback failed immediately after
  committing the initial CPU snapshot with `DuplicatePresentation`. The same
  transaction reached production twice: admission projection removed the
  quarantined surface transaction and DMA-BUF Present from the original
  observation, but omitted the equivalent software-Present record. The
  same-iteration admission release then appended its retained copy.
- Projection now applies one quarantine predicate to CPU transactions,
  DMA-BUF Presents, and software Presents. The released admission group is the
  sole owner of the reprojected transaction and presentation lifetime.
- Admission and production validation also reject duplicate software Presents
  for one transaction/surface before renderer resource registration. Focused
  regressions reproduce same-iteration replacement and the defensive failure
  boundary.
- The corrected physical standalone run passed its exact verifier. The
  500-by-500 llvmpipe cube animated through 487 software-Present transactions
  in 17,755 ms; all 487 produced native retirements, Complete, Idle, and
  idle-fence triggers. Native submission and retirement failures remained
  zero, all presentation resources drained, X protocol errors remained zero,
  and the session completed normal cleanup.

## 2026-07-29: retained repaints must reuse renderer image generations

- The latest physical GLX run completed two mixed Presents and routed the
  predecessor Idle correctly. A focus-border repaint then recreated the current
  DMA-BUF's EGLImage and GL texture. The draw blocked for 10.4 seconds in the
  frame completion barrier before AMD reported a guilty-context hard recovery;
  Sophia aborted with status 134 and the GLX client lost its X connection.
- The static gears were therefore not a GLX bootstrap, input, KMS callback, or
  TTY-recovery failure. Only the first two frames retired before a generic
  compositor-only repaint repeated a live import.
- Mixed layers now carry opaque renderer-image generations. A bounded
  renderer-private slot table imports one EGLImage/texture per generation,
  validates the complete DMA-BUF identity on hits, and reuses the texture for
  focus, chrome, workspace, and retained-damage repaint.
- Replacement retirement evicts the predecessor while its native context is
  current before triggering the idle fence. Context recreation and normal
  shutdown clear residency before presentation leases are released.
- Mixed and CPU presentation now reports X Present `Copy`; `Flip` is reserved
  for future direct scanout. Reduced metrics and the GLX reporter require cache
  hits, reject mismatch/capacity debt, and preserve post-KMS cadence evidence.

## 2026-07-31: close cleanup evidence must allow an already-clear key ledger

- The same-commit QEMU M8 session completed all three close actions, normal
  application exits, clean input drain, and normal cleanup, but its verifier
  rejected Firefox because only two closes emitted nonzero `close_surface`
  key-clear records.
- Firefox had already released the same two keys during an earlier focus
  transition. Closing a surface with no remaining pressed keys correctly emits
  no nonzero key-clear record; requiring one for every close made promotion
  depend on input and lifecycle timing rather than final state.
- The verifier still requires three committed close actions, at least two
  demonstrated nonblocking close-time key clears, positive state-only
  releases, and a final key ledger with zero pending keys or release barriers.
  Its negative regression removes every close-time clear and must fail, while a
  new regression removes one clear to represent an already-clean close.

## 2026-07-31: physical latency must begin from the current presented baseline

- The first complete 20-sample physical input-to-photon capture used kernel
  page-flip timestamps with zero fallback or pending correlation and clean
  teardown in every sample, but missed the sub-refresh gate at 18 ms p95
  against a 17 ms budget.
- Input readiness could use focused CPU visual detail or any earlier nonzero
  native frame. Several samples therefore injected while a newer pre-input
  focus/chrome frame was still queued, forcing the measured input frame to wait
  behind unrelated presentation work.
- CPU-backed proofs now require the current scene checksum to be the primary
  output's presented checksum before announcing readiness. Stable GPU content
  retains its independent surface-keyed presentation path, and headless CPU
  proofs remain valid without a native frame. A regression rejects a stale
  native checksum and zero-pixel or zero-export baselines.

## 2026-07-31: retired CPU scanout buffers must retain damage history

- The current-baseline correction removed unrelated pre-input frames, but the
  next physical 20-sample archive still failed at 29 ms p95 and 32 ms maximum.
  Its two slowest frames spent 14 and 22 ms from input dwell to KMS submission;
  both recorded a 20 ms native upload while queue and kernel page-flip clocks
  remained healthy.
- The CPU scanout worker allocated and wrote a complete 3840-by-960 linear GBM
  buffer for every changed frame even though Engine already proved bounded
  output damage. A terminal redraw therefore copied the whole output and
  exposed allocator/write-tail latency on the physical AMD path.
- Retired CPU worker leases now enter a bounded three-buffer free pool with
  their checksum and immutable output-damage snapshot. Reuse computes
  conservative damage against the exact pixels already stored in that buffer,
  maps the linear BO, and copies only those clipped rows. Missing, empty, or
  invalid damage proof falls back to a full repaint, and a failed reusable map
  falls back to the existing allocate-and-write path.
- The focused QEMU KMS regression exercised repeated damage-only reuse,
  reducing maximum native upload to 1 ms while preserving exact input pixels,
  kernel page-flip correlation, zero renderer failures, and clean lease
  teardown. Physical TTY3 p95 remains the authoritative acceptance gate.

## 2026-07-31: pre-input cursor ownership failure is not a latency sample

- The first damage-reuse physical rerun completed 16 independent input proofs
  with clean renderer/KMS teardown and a 2 ms maximum native upload. Sample 17
  exited before physical-input readiness or injection because the initial
  cursor-plane atomic update returned `EACCES`.
- No uinput trigger, injector timestamp, or latency record existed, so the
  failure did not describe input-to-photon performance. Treating it as a sample
  would conflate transient session startup ownership with the measured path.
- The physical runner now makes at most three session-start attempts while
  retaining the same uninjected uinput device. A retry requires the exact
  cursor `EACCES`, no physical-input readiness, no trigger or injector result,
  and no completed latency record. The rejected attempt log is retained beside
  the eventual sample. Any other failure, any failure after readiness, or
  exhaustion of the bound still stops the gate.

## 2026-07-31: one latency sample must produce one input frame

- The retry-capable physical run completed all 20 independent samples with
  exact text, kernel page-flip timestamps, clean teardown, and no retries.
  Damage-only reuse held maximum native upload to 2 ms, but full-chain p95 was
  22 ms against the 17 ms refresh budget.
- The injector's two-millisecond delay after every key transition stretched
  `sophia\n` across roughly 28 ms. Sophia correctly rendered and submitted
  intermediate terminal states. On the two p95-tail samples, the last routed
  press then arrived while one of those earlier states already owned the next
  page flip, adding 8–9 ms between queue dwell and final-frame submission.
- The physical gate now emits the same exact press/release sequence with zero
  spacing. Events still enter through uinput, the normal threaded libinput
  worker, X delivery, xterm, CPU composition, atomic KMS submission, and kernel
  retirement. Coalescing the bounded burst into one visual transaction removes
  an unrelated earlier frame from the isolated input-to-photon measurement;
  exact text, changed pixels, event flush, and exact-frame correlation remain
  mandatory.

## 2026-07-31: unchanged secondary outputs must not be recomposed

- The first zero-spacing physical archive proved zero libinput queue dwell in
  all 20 samples, but failed at 29 ms p95 and 30 ms maximum. The isolated input
  frame consistently spent 11–14 ms before KMS submission, then up to 16 ms
  waiting for the kernel page flip. Exact text, clocks, cleanup, and renderer
  health all passed; native upload remained bounded at 3 ms.
- `LiveProductionCpuScene::frames_for_outputs` rebuilt the diagnostic frame for
  every non-primary output after each primary composition. On the physical
  topology that allocated, cleared, drew, and exactly scanned an unchanged
  1920x1080 CPU frame on the owner path for every xterm update.
- The scene now retains immutable secondary frames by output index and complete
  `HeadlessOutput` descriptor. Primary recomposition clones the retained frame;
  output removal, reorder, size, scale, or identity change invalidates it.
  Regression coverage proves the same pixel allocation survives primary
  recomposition and is replaced after a descriptor change.
- The focused dual-output QEMU GBM/KMS path passed exact input and pointer
  pixels, damage-only buffer reuse, kernel page-flip correlation, and clean
  teardown with 2 ms maximum composition and 1 ms maximum upload. Physical TTY3
  p95 remains the authoritative promotion gate.

## 2026-07-31: retained primary CPU pixels follow normalized output damage

- The first physical archive after secondary-output caching completed all 20
  exact input transactions with clean clocks and teardown, but still measured
  25 ms p95 and 28 ms maximum against the 17 ms budget. Native upload remained
  bounded at 2 ms. The remaining owner-path interval was 9–13 ms from input
  dwell to KMS submission.
- `LiveProductionCpuScene` retained the prior primary allocation but cleared
  and rebuilt all 3840x960 pixels on every changed display list. The xterm
  surface occupied only a bounded part of that output, so the compositor did
  work already excluded by Engine's conservative output-damage proof.
- Primary composition now snapshots the current display list, surfaces, and
  software cursor before drawing, compares it with the retained snapshot, and
  reduces the result through `plan_output_repaint`. Partial plans clear only
  clipped damage and replay every intersecting surface, solid border, and
  cursor pixel in original stacking order. Skip plans retain the allocation
  without a copy. Missing history, invalid proof, full-repaint policy, or an
  incompatible retained allocation uses the existing full-frame path.
- A uniquely owned retained frame is mutated in place. Shared storage is copied
  before a changed partial repaint so an in-flight or observed frame remains
  immutable. Regressions cover outside-damage preservation, removed pixels,
  stacking, clipped damage, old/new cursor extents, shared storage, invalid
  baseline fallback, and the production snapshot route.

## 2026-08-01: Firefox smooth wheel requires an explicit XI2 path

- Core Button4-Button7 records reached Firefox's exact GTK content window and
  decoded correctly in Xlib, but the local page never observed a DOM `wheel`
  event. Firefox links `gdk_disable_multidevice` and leaves GTK on its legacy
  core-device manager unless `MOZ_USE_XINPUT2=1` is set.
- The X `QueryExtension` reply was verified against the installed Xlib/XCB
  protocol headers: `present`, major opcode, first event, and first error occupy
  bytes 8 through 11. An experimental shifted layout made XKB unusable and was
  reverted before the final implementation.
- Sophia now advertises XI2 2.1 master-pointer horizontal and vertical valuator
  plus scroll classes, retains cumulative v120 positions in the X frontend,
  encodes them as XI2 motion valuators, and resolves selections through the
  target window's ancestor chain. Engine routing remains protocol-neutral.
- Firefox's XI-only key selection exposed a separate bounded-backpressure bug:
  the input writer waited five seconds for a core key selection even when an
  XI2 key event was selected. XI selection now satisfies writer readiness, with
  a regression proving that it bypasses only the legacy startup wait.
- The QEMU harness isolates the Firefox surface before both PRIMARY middle-click
  and wheel stages. The clean instrumentation-free run completed all eight
  Firefox stages and emitted
  `sophia_qemu_firefox_m8 schema=3 status=scroll_complete source=wheel axis_route=true keyboard_fallback=false`,
  followed by normal application, authority, renderer, KMS, input, and VM
  teardown. Chromium remains an independent compatibility follow-up because it
  is not installed on the current host.

## 2026-08-01: the physical Firefox gate must carry its own operator state

- The retained physical Firefox launcher described only the older eight-stage
  browser exercise, while its verifier first invoked the broader xmonad TTY3
  verifier. That unrelated verifier required a startup-Kitty exit, desktop
  relaunch, workspace-empty input, a VT round trip, pointer-edge traversal, and
  three terminal launches that the Firefox instructions never requested. A
  correct Milestone 10 run therefore could not satisfy its stated verifier.
- The Milestone 10 runner now gives exactly two Kitty processes independent
  A1/A2/A3 and B1/B2/B3 prompts. Each accepted token changes only a redacted
  title length, allowing the owner loop to record bounded checkpoints before
  Firefox, after normal `Ctrl+Q` exit, and after the restarted browser is closed
  through xmonad. Both terminals retain their visible checkpoint history.
- The offline Firefox page now displays its current next action. The physical
  verifier orders a routed axis event between the PRIMARY and DOM scroll
  stages, orders the physical layout action before the resize stage, requires
  the normal-close/restart/WM-close sequence, and retains the existing strict
  protocol, input-drain, native-presentation, frontend, authority, guard, and
  TTY restoration checks. Mutation fixtures reject missing wheel, Kitty
  retention, forced-close, and cleanup evidence.

## 2026-08-01: xmonad application keys identify semantic slots

- The first self-guided physical run proved both Kitty checkpoint clients and
  then rejected every `Super+F` request as `UnavailableSessionAction`; Firefox
  never spawned. The session descriptor contained terminal and browser launch
  actions but no application-menu launcher.
- The compatibility bridge previously selected the first, second, or third
  launch action remaining in the negotiated descriptor. The xmonad bindings
  themselves are semantic—`Super+Enter` is terminal, `Super+P` is the launcher,
  and `Super+F` is the browser—so filtering an unavailable middle application
  incorrectly shifted later meanings while leaving their keys fixed.
- Translation now maps those three profile actions to stable application IDs
  1, 2, and 3, then applies the existing descriptor admission check. A focused
  regression requires `Super+F` to launch application 3 when applications 1
  and 3 are present and requires the absent application-2 binding to fail
  closed. Engine still receives only the negotiated protocol-level session
  action and remains unaware of xmonad key semantics.

## 2026-08-01: the physical Firefox gate must isolate browser process state

- The first physical run after the stable browser binding launched Firefox and
  loaded the offline page, but only a white client area and the ordinary Sophia
  frame were visible. The parent window committed its CPU backing snapshot
  while a second Firefox surface continuously submitted a 1-by-1 DMA-BUF for a
  1276-by-1422 logical surface; the renderer correctly rejected every mismatch
  instead of scaling it. No interaction stage completed.
- The physical launcher had diverged from the passing QEMU workload: it reused
  the operator's normal Firefox profile and omitted QEMU's native-X,
  single-process, and XI2 controls. That also contradicted the XI2 wheel finding
  above, because the physical browser never received `MOZ_USE_XINPUT2=1`.
- Milestone 10 now creates a private run-local Firefox profile with the same
  bounded preferences used by QEMU, passes it explicitly with `--profile`, and
  forces native X11, disabled e10s/fission, and XI2 for that proof. A launcher
  regression retains the complete configuration. Renderer scale policy and
  protocol-neutral Engine routing are unchanged.

## 2026-08-01: descendant GPU presentation belongs to the X toplevel

- The next physical run retired Firefox's 1280-by-1040 DMA-BUF frames instead
  of rejecting them, but retained each frame as child surface 8388621 at global
  origin. Its managed parent occupied the left 1276-by-1422 pane. The child
  therefore rendered outside the parent placement, overlapped Kitty's hit-test
  region, and a browser click transferred focus and later key delivery to
  Kitty surface 6291472.
- X child geometry is relative to its parent. The X authority already flattened
  descendant software drawing into the root-child presentation surface, but
  standard DRI3 Present bypassed that reduction. DMA-BUF Present now walks the
  retained X hierarchy, targets and advances the managed toplevel transaction,
  translates damage by the accumulated child offset, and exports that reduced
  offset to the protocol-neutral live scheduler. Engine and the renderer still
  receive only surface, geometry, buffer, and damage facts.
- The renderer now intersects a mismatched source with both the surface clip
  and the actual pixel-sized target. A 1280-by-1040 frame in the 1276-by-1422
  pane therefore remains unscaled at the toplevel origin and clips to
  1276-by-1040 rather than claiming a 1276-by-1422 clip. Retirement evidence
  reports unit scale from source-versus-target size, independent of clipping.
- Focus and input now operate on the same managed surface that owns the visual
  transaction; the X input writer remains responsible for selecting the exact
  descendant window. Focused hierarchy and clipping regressions plus the full
  offline all-feature workspace gate pass. A fresh physical Firefox workflow
  remains required before Milestone 10 advances.

## 2026-08-01: the Firefox operator flow needs one checkpoint coordinator

- The next physical run validated the descendant projection fix. Firefox
  surface 8388611 repeatedly retired a 1280-by-1040 frame at toplevel position
  2,16 with unit scale, and the offline page completed loaded, keyboard, and
  clipboard stages. This replaces the prior global-origin child evidence and
  proves ordinary browser keyboard input reaches the correctly placed visual.
- The PRIMARY step did not describe a compositor failure. The middle button
  landed at the minimum screen edge, outside the toplevel's two-pixel chrome
  clearance, and was explicitly suppressed as `no_target`. A later physical
  wheel reached the X route, but the page correctly could not advance past the
  missing ordered PRIMARY stage. The page now tells the operator to keep both
  middle-click and wheel input well inside the colored client area.
- The two independent Kitty probes also each announced the next global action
  as soon as their own A2 or B2 checkpoint completed. Completing B2 before A2
  therefore launched and closed the second Firefox; completing A2 then
  launched an unintended third browser. Each probe now publishes private
  checkpoint markers; Kitty B waits for A1 before the first launch, both probes
  wait at the later barriers, and launch/logout authority belongs only to Kitty
  B. An executable concurrency regression feeds both clients through all three
  barriers and requires exactly two Firefox instructions and one logout
  instruction from the coordinator.
- The run exited normally with clean Firefox status and renderer teardown, but
  its deliberate fail-closed verifier rejected three of eight browser stages,
  zero selection conversions, and three launches. It is diagnostic evidence,
  not a Milestone 10 pass; the synchronized physical workflow must be rerun.

## 2026-08-01: XI2 scroll valuators must not replace pointer X and Y

- The synchronized physical Firefox run reached the deterministic loaded,
  keyboard, clipboard, pointer, and PRIMARY stages. A real wheel event then
  crossed libinput, was observed and routed by Engine, and received an X input
  delivery acknowledgement, while the page never completed its DOM `wheel`
  stage. Firefox presentation continued retiring at the correct toplevel, so
  this isolated the failure after compositor hit testing and before browser
  event handling.
- Sophia described horizontal and vertical scrolling as XI2 valuators 0 and 1
  and set those same bits in XI2 motion events. Xorg reserves valuators 0 and 1
  for relative pointer X and Y; its input-test device places relative
  horizontal and vertical scrolling on valuators 2 and 3. A scroll class names
  an existing valuator rather than replacing the pointer coordinate axes.
- The X frontend now reports four relative pointer valuators, associates the
  preferred horizontal and vertical scroll classes with axes 2 and 3, and sets
  those same valuator-mask bits in XI2 motion events. Cumulative v120 positions
  and legacy Button4-Button7 emulation remain X-frontend state; Engine input
  packets remain protocol-neutral. Wire regressions parse the complete pointer
  class topology and cover simultaneous two-axis value ordering. The physical
  Firefox DOM stage remains the acceptance proof.

## 2026-08-01: GTK's first XI2 scroll value establishes a baseline

- The first physical run after moving scroll valuators to axes 2 and 3 still
  stopped at Firefox step 4. Its finished session log contains exactly one
  `axis_observed`/`axis_routed` packet between PRIMARY and shutdown, proving
  that the corrected wire topology was exercised but could not yet produce a
  nonzero DOM scroll delta.
- GTK's XI2 device path intentionally records the first value received for a
  scroll valuator and returns a zero delta. Only a later absolute valuator
  value can be differenced against that baseline. The QEMU harness previously
  retried as many as ten individual wheel clicks until the page advanced, so a
  second retry silently satisfied this requirement without recording it in the
  evidence contract.
- XI2 2.1 also requires legacy button emulation when a device exposes scroll
  valuators. Sophia already emitted a core Button4-Button7 record, but did not
  emit the corresponding XI2 ButtonPress/Release event. Axis routes now resolve
  smooth-motion and button selections independently and write the button
  detail.
- Relative scroll valuators now advertise unknown bounds as zero/zero. The
  local page tells the operator to scroll through at least two notches and
  displays an explicit baseline message if it observes a zero-delta wheel
  event. QEMU sends exactly two clicks, and both automated and physical gates
  require two new routed-axis records before accepting DOM scroll completion.
  The Milestone 10 item remains open for a fresh physical proof.

## 2026-08-01: Xorg marks button-derived smooth Motion as emulated

- The next live physical run used the release binary built after commit
  `a12d5bd3`, with `MOZ_USE_XINPUT2=1`, and again remained at Firefox step 4.
  The operator then generated five deliberate wheel notches while the browser
  stayed open, but the session log could not distinguish them: schema-3
  `axis_observed` and `axis_routed` were intentionally lifetime one-shot
  markers. The new two-packet physical and QEMU checks had incorrectly treated
  those markers as per-packet evidence.
- The local X server implementation shows a second wire mismatch. When a
  physical Button4-Button7 press is converted into a smooth XI2 Motion event,
  Xorg sets `POINTER_EMULATED`; its XI2 encoder carries that through as
  `XIPointerEmulated`. The later compatibility XI2 ButtonPress/Release events
  are marked the same way. Sophia marked only the button pair, leaving its
  button-derived smooth Motion distinguishable from Xorg.
- Sophia now derives XI2 device-event flags from the protocol-neutral axis
  event and marks both the smooth Motion and compatibility button pair with
  `XIPointerEmulated`; ordinary pointer Motion remains unflagged. The owner
  loop additionally emits schema-9 `axis_batch` records containing only
  observed/routed counts. QEMU waits on the summed routed count, and the
  physical verifier sums only batches ordered between PRIMARY and DOM scroll.
  Mutation fixtures reject a single routed packet, without logging direction,
  values, coordinates, or timing.

## 2026-08-01: GitHub organization registered as sophia-stack-project

- Evaluated candidates for a centralized GitHub organization to host the Sophia display server, Hagia, and related system components.
- The exact names `sophia` and `sophia-stack` are taken on GitHub as user accounts.
- Decided on `sophia-stack-project` as the organization name to perfectly match the `sophia-stack.org` domain and project boundaries. Other system-level candidates like `sophia-window-system` remain reserved.
- The organization registration on GitHub will act as a permanent holder for future repository migrations and ecosystem consolidation when the engine reaches maturity.

## 2026-08-01: Firefox resize completion belongs to page-flip retirement

- The latest physical launch disproved both earlier geometry hypotheses.
  Firefox first retired its 1280-by-1040 admission frame, then produced an
  exact 1276-by-1422 candidate for the xmonad tile. A layout epoch cannot stage
  that candidate unless its observed pixels exactly equal the configured
  target, so mapped `ConfigureWindow` denial and configure delivery were no
  longer the failing boundary.
- Sophia nevertheless logged the layout epoch as committed before the native
  page flip completed. Later same-surface backing transactions then advanced
  the Engine generation, and the already prepared Present transactions were
  correctly rejected as `RejectedStaleSurface` at retirement. The white or
  clipped Firefox window was therefore a split logical/visual commit, not a
  missing browser resize.
- DMA-BUF resize observations now arm a bounded `(transaction, surface)` visual
  candidate instead of updating committed layout size. The standing target
  remains authoritative until an exact successful native retirement; old-size
  Presents remain rejected during that interval. The production runtime owns
  a bounded per-surface content fence while a Present is asynchronous, defers
  only later authority groups touching that surface, and rebases/releases them
  after either successful or controlled rejected retirement. Surface removal,
  native detach, failed submission, and shutdown have explicit non-deadlocking
  cleanup paths. Other surfaces continue independently.
- Runnable regressions prove multiple surfaces can share one layout transaction,
  mismatched or wrong-transaction retirements cannot clear a candidate, a
  resize Present reaches Engine generation 2 before its deferred update reaches
  generation 3, removals bypass the fence, and shutdown discards its backlog.
  The physical verifier now requires ordered `visual_armed` and matching
  `visual_committed` evidence and rejects a stale outcome between them. Offline
  targeted and all-feature live-session tests pass; the physical Firefox rerun
  remains the acceptance boundary.

## 2026-08-01: an armed launch frame precedes its standing resize target

- The next physical run proved that `Super+F` and application launch were not
  the failing boundaries. Firefox started, published surface 8388611, and
  supplied exact 1280-by-1040 Present transaction 1559. Recovery selected that
  frame and emitted `visual_armed`, but no matching native submission or
  `visual_committed` followed; application admission eventually timed out.
- The launch-timeout recovery retained the blind-WM 1276-by-1422 tile as a
  standing target. Present disposition incorrectly preferred that future
  obligation over the already armed 1280-by-1040 recovery candidate and
  classified the candidate as a layout mismatch. This formed a cycle: native
  retirement was required to clear the temporary extent, while the uncleared
  standing target prevented the exact frame from reaching native retirement.
- Present disposition now bypasses a different standing target only for the
  exact armed transaction, surface, and buffer size. A later transaction with
  the same launch-sized buffer remains rejected or fenced, and the standing
  target is still discharged only by its own exact visual retirement. Focused
  tracker and live-admission regressions retain both identities; a fresh
  physical Firefox launch remains the acceptance proof.

## 2026-08-01: resize evidence must preserve the active presentation class

- The next physical run proved the exact 1280-by-1040 Firefox admission frame
  now reaches native retirement and releases its temporary recovery extent.
  Sophia then delivered the standing 1276-by-1422 configure and committed the
  layout without arming a matching visual candidate.
- The matching transaction was a passive CPU backing snapshot. Firefox's
  visible DMA-BUF producer continued submitting 1280-by-1040 frames, which the
  1276-by-1422 layer clipped to 1276-by-1040. The logical layout therefore had
  roughly 382 uncovered rows, matching the physical black lower region and
  clipped browser content.
- Complete visual evidence now establishes a protocol-neutral, monotonic
  requirement for the surface lifetime. Once `PresentedBuffer` is observed,
  `BackingSnapshot` remains available as safe recovery state but cannot stage
  or commit a later resize. An exact presented transaction must follow the
  existing visual-arm and native-retirement path. CPU-only surfaces retain
  synchronous backing resize, and explicit software Present remains valid
  presented evidence so storage-class changes do not deadlock.
- Engine and all-feature live-session regressions reproduce the physical
  1280-by-1040 to 1276-by-1422 sequence, preserve the standing target across
  the rejected backing snapshot, require matching DMA retirement, retain the
  CPU-only path, cover explicit software Present, and clear the requirement on
  surface removal. A fresh physical Firefox run remains the acceptance proof.

## 2026-08-02: xmonad actions require a post-injection response boundary

- The physical Firefox rerun rendered the browser at its exact 1276-by-1422
  tile and advanced to verifier stage 5 of 8. This physically confirms the
  exact-window Present-before-core `ConfigureNotify` fix. The remaining failure
  was `Super+Space`: action 3 committed as WM transaction 6 with four surfaces,
  `moved_surfaces=0`, and `configure_deliveries=0`.
- Engine correctly owned the physical chord and delivered only opaque action 3
  to policy. The defect was inside the xmonad compatibility adapter. It queued
  existing-node root `ConfigureNotify` reconciliation and private Mod1+Space in
  one response collection. The old Tall requests satisfied the expected-window
  set, and the 80 ms quiet fence could close the transaction before xmonad's
  post-key Mirror requests arrived.
- The bridge now drains and discards the complete pre-action reconciliation,
  validates that the supervised WM registered the profile chord, then injects
  the private key press/release and wake event and requires fresh WM activity.
  This remains a policy-adapter concern: X Authority still owns real X clients
  and Engine still owns physical input and opaque action authorization.
- The strengthened real-xmonad run first failed that grab validation and exposed
  an older core-wire defect. `GetKeyboardMapping` carries `firstKeyCode` and
  `count` in its request body, while the fake server read the header padding as
  `firstKeyCode`; its saturating exclusive range also serialized 247 mappings
  after advertising 248. Correct body parsing and complete inclusive keycode
  serialization let unmodified xmonad resolve Space and register Mod1+Space.
- A hermetic fake WM delays its Mirror requests beyond the former quiet period
  and proves the action returns only the post-injection geometry. It resolves
  Space through the full 248-entry keyboard mapping before registration. A
  second fixture omits `GrabKey` and proves fail-closed behavior. The real
  unmodified-xmonad smoke now requires the exact three-window Tall-to-Mirror
  transition, preserving a runnable reference boundary for future batching or
  event-loop optimizations.

## 2026-08-02: physical focus transitions are cross-client X authority work

- The next physical Firefox run confirmed the Mirror action and every repeated
  `Super+J` focus proposal committed correctly. Firefox still could not advance
  its refocus stage because its DOM observed `FocusIn` without the preceding
  `FocusOut`. The routed frontend had stored the previously focused X window
  independently in each client writer and sent focus control only to the new
  client, so the old client could never receive its leave event.
- XLibre's `SetInputFocus`/`DoFocusEvents` path and yserver's native Rust
  crossing reducer both establish the same ownership rule: the X protocol
  authority resolves the old and new window routes and delivers both halves of
  a focus transition. Engine continues to expose only opaque focused
  `SurfaceId` state.
- The X frontend broker now retains one bounded physical-focus route. A
  cross-client change queues `FocusOut` on the old client's control queue and
  `FocusIn` on the new client's queue; a same-client change queues both events
  together so socket order is deterministic. Per-client FIFO ordering also
  preserves repeated A-to-B-to-A transitions while allowing a later optimized
  writer implementation behind the same passive transition packet.
- The same run ended cleanly with status 1 after repeated `Super+J` presses:
  an already in-flight action was correctly deduplicated by the WM owner queue,
  but the physical-input branch treated that expected bounded outcome as
  fatal. Repeated action requests now coalesce nonfatally, emit a reduced
  schema-3 record, and accumulate `action_coalesced` in the schema-2 WM
  transport summary. Capacity rejection remains distinct and bounded.
- A two-client X11 wire regression requires `FocusIn(A)`, then
  `FocusOut(A)/FocusIn(B)`, then `FocusOut(B)/FocusIn(A)`. A focused live-WM
  reducer regression locks the duplicate-to-coalesced disposition, and the
  hardware verifier fixture consumes the versioned transport summary. This
  temporary duplicate-to-coalesced policy was superseded by the ordered owner
  ingress work recorded on 2026-08-08 below.

## 2026-08-02: action reconciliation is an activity fence, not an all-window configure contract

- The next physical run advanced Firefox through the cross-client refocus
  checkpoint and created its attached popup as the fifth surface. Repeated
  `Super+J` then stalled for three seconds and restarted the compatibility
  bridge with `expected=3 configured=1`. The fifth surface remained
  client-positioned and outside the blind policy set; the failure was not
  transient classification or X metadata routing.
- The bridge had treated every synthetic root `ConfigureNotify` generated from
  the three existing layout nodes as a promise that xmonad would answer with a
  `ConfigureWindow` for every node. Core X11 makes no such promise. In the
  full-height layout xmonad emitted the one policy change it needed and then
  became quiet, so waiting for two invented replies turned a valid partial
  reconciliation into a fatal policy-transport timeout.
- Existing-node geometry is now applied as one ordered batch with one coalesced
  root notification. Profiled actions require pre-injection activity and a
  quiet boundary, while only new `MapRequest` windows remain mandatory
  configure admissions. Post-injection activity is still required and remains
  separate, so stale reconciliation cannot satisfy a later action.
- The process-external bridge regression manages three opaque surfaces, answers
  the coalesced pre-action notification with exactly one configure request,
  then proves a private focus chord returns the post-action focus result. A
  second root notification is rejected by the fixture, preserving a batching
  seam for future synthetic-server event-loop optimization.

## 2026-08-02: WM reseed must replay an uncommitted admission before relayout

- The physical run after truthful X11 lifecycle delivery showed that
  `Super+F`, the session action, and Firefox process launch all succeeded.
  Firefox's first policy-managed surface timed out while proving its initial
  layout, which correctly restarted the speculative xmonad bridge. The restart
  then queued a relayout from only the last committed workspace. That state did
  not yet contain Firefox, so transaction 6 committed four visual layers but a
  policy projection of only the two existing Kitty surfaces. The later manage
  retry timed out and withdrew Firefox, making a working `Super+F` chord appear
  to launch nothing.
- A speculative bridge cannot recover an uncommitted `ManageSurface` through a
  committed-state relayout: the manage request is the state transition that
  registers the opaque surface in the WM workspace. Restart recovery now
  replays the oldest pending admission before considering an ordinary relayout.
  The choice is an allocation-free session reducer with crate-boundary tests,
  leaving X11 lifecycle in X Authority and visual proof in Engine.
- Schema-3 reseed evidence distinguishes `manage`, `relayout`, and `none`, so a
  future queue/batching optimization can preserve the ordering contract. A
  fresh physical Firefox launch remains the acceptance proof.
- Full-suite validation also exposed four stale wire-fixture assumptions left
  by the preceding lifecycle change: unselected create/configure/map events,
  Expose after a non-admission configure, and notification after a no-op
  configure. The fixtures now select the events they require, verify shared
  map state through `GetWindowAttributes`, bound blocking reads, and require
  silence for a no-op. The complete 166-case X11 wire target passes.

## 2026-08-02: reseed replay and ordinary admission have different readiness gates

- The immediate physical rerun falsified the first reseed fix. Its new evidence
  emitted `reseed_queued request=relayout`, followed by the same two-surface
  workspace projection and Firefox admission timeout. The selector had reused
  `next_unmanaged_surface`, which deliberately returns no candidate while any
  rollback extent is active. That is correct for scheduling a new admission,
  but wrong after restarting a bridge whose rejected `ManageSurface` must be
  replayed before recovery can settle.
- Restart reseed now selects the oldest known, nonterminal unmanaged surface
  independently of the rollback scheduling gate. Ordinary owner-loop admission
  remains blocked by rollback. One allocation-free reducer owns the shared
  known-surface/retry predicate, and a crate-boundary regression requires a
  first-retry Firefox-shaped candidate to remain replayable while rejecting a
  withdrawn or terminal candidate. Schema-3 evidence must now report
  `request=manage` on this recovery path; physical confirmation remains open.

## 2026-08-02: an unarmed standing-target retirement ends launch recovery

- The physical run confirmed the corrected reseed ordering: xmonad replayed
  Firefox's uncommitted manage request and Sophia admitted its exact
  1280-by-1040 fallback frame. The retained recovery constraint then delivered
  the 1276-by-1422 standing configure, and Firefox retired an exact
  1276-by-1422 PresentedBuffer. Rendering nevertheless stayed clipped to
  1276-by-1040, leaving the observed black lower region and dropping pointer
  focus handoff outside that clip.
- The exact target arrived after the constrained fallback layout had already
  committed, so it intentionally had no armed resize epoch. Native retirement
  accepted only armed candidates and discarded this otherwise conclusive
  frame. The prior regression manually called `record_committed(target)` and
  therefore never exercised the production retirement boundary.
- Native retirement now accepts an unarmed frame only when it exactly matches
  the Engine's outstanding target and the same surface still owns a temporary
  recovery extent. It records that target, clears the extent, and queues one
  constraint relayout. Old-sized frames and unarmed targets without active
  recovery remain unable to bypass a layout epoch. The updated regression
  reproduces the fallback retirement followed by the exact standing-target
  retirement; a fresh physical run remains the acceptance proof.

## 2026-08-02: recovery-hint release must preserve xmonad window identity

- The next physical run confirmed the standing-target fix: Firefox's exact
  1276-by-1422 frame retired, `reason=standing_target_presented` cleared the
  temporary extent, later frames used complete clips, pointer focus handoffs
  succeeded, and the browser proof reached 7/8. The constraint relayout then
  moved the newly focused Firefox from xmonad's full master column into the
  lower-right slave pane.
- The move was not a layout-order or pixel regression. Releasing the temporary
  fixed extent changed the bridge's synthetic `WM_NORMAL_HINTS` profile. The
  bridge represented that mutable property change as `DestroyNotify` followed
  by a new `MapRequest`; xmonad consequently discarded the surface's focus and
  stack identity before remapping it.
- XLibre's core property path delivers `PropertyNotify` under
  `PropertyChangeMask`, and yserver's independent encoder confirms the exact
  32-byte core event layout. The private xmonad bridge now updates the stored
  manage profile in place, emits `PropertyNotify` for `WM_NORMAL_HINTS`, and
  retains the synthetic window ID. A reducer regression covers fixed and
  released profiles without destroy/map events. The real-xmonad smoke changes
  an already focused recovery surface back to resizable and requires the same
  master/focus stack afterward, preserving a seam for later property-routing
  optimization.

## 2026-08-02: manage focus must exist in both Sophia and legacy-WM state

- A live run from the identity-preserving bridge repeated the same visible
  demotion. Firefox committed focused in Sophia at transaction 6, then the
  recovery relayout at transaction 7 immediately returned Kitty as focused and
  master while placing Firefox in the lower-right slave pane. The retained
  synthetic ID therefore fixed a real lifecycle defect but was not the final
  stack-order cause.
- The xmonad profile always appended `FocusSurface(new)` to a manage response,
  but it did not ensure the private xmonad process had performed that focus
  transition. The previous real-xmonad regression hid this split state with a
  separate `FocusRequested` before releasing the constraint. Sophia and X
  Authority consequently focused Firefox while xmonad retained Kitty; the next
  relayout truthfully exposed xmonad's older focus/stack.
- Xmonad-profile manage now performs the same bounded synthetic pointer-focus
  synchronization used by an explicit focus request before returning the
  manage proposal. The real-xmonad smoke no longer contains the masking focus
  request and still requires the recovery surface to remain master/focused
  after hint release. A process-external fixture additionally requires a
  managed surface to remain the queried legacy-WM focus at the very next
  relayout.

## 2026-08-02: fresh-WM recovery must restore committed state before admission replay

- The next physical run loaded the focus synchronization change and still
  placed Firefox in the lower-right pane. Transaction 5 timed out and restarted
  the complete bridge/xmonad process. The fresh peer was seeded with only the
  pending Firefox `ManageSurface`; transaction 6 forced Firefox focus in
  Sophia, but transaction 7 introduced the two previously committed Kitty
  nodes and returned Kitty as xmonad's actual master/focus.
- That replay also projected the Engine's temporary 1280-by-1040 recovery
  extent as fixed `WM_NORMAL_HINTS`. Xmonad's standard `manage` path classifies
  a newly seen fixed-size window as floating. Focus synchronization cannot
  restore tiling membership or admission order after those facts have already
  been lost. The prior real-xmonad smoke reused one process where Firefox was
  already tiled, while its fixture reduced a click to `SetInputFocus`; neither
  represented a fresh StackSet.
- Restart reseed is now ordered: committed policy-managed surfaces are queued
  as a relayout first, and the unresolved admission is queued behind it. The
  relayout derives membership from committed `WmWorkspaceState`, so the pending
  surface cannot leak into the seed. Stable opaque-surface order reconstructs
  the committed xmonad admission sequence before Firefox is managed.
- Declared client constraints and Engine recovery constraints now have distinct
  projections. Blind-WM nodes carry only declared constraints and declared
  resizability; Engine transaction reconciliation continues to apply the
  effective recovery extent to geometry and configure sizes. Intrinsically
  fixed clients still cross the WM boundary unchanged, while a temporary
  recovery fence can no longer change xmonad policy identity.
- The real-xmonad smoke now destroys its first runtime, starts a genuinely
  fresh second xmonad, seeds two committed opaque nodes, replays the third
  admission, and requires that third node to remain tiled, focused, and master
  on the following relayout. Engine, session, and planner regressions lock the
  declared/effective split and the two-phase reseed order without adding an X11
  or WM wire extension.

## 2026-08-02: reseed phases must not share pending visual ownership

- The immediate live repeat proved `Super+F`, action commit, process launch,
  surface discovery, and xmonad's two-phase request order all succeeded.
  Firefox still failed admission after a second restart. The first reseed
  relayout armed Firefox's selected 1280-by-1040 Present even though its WM
  request contained only the two committed Kitty nodes; the queued manage
  response then had no quarantined transaction left to stage and timed out.
- The leak occurred after the WM boundary. `proposal_from_response` applied a
  valid response to `planning_layers()`, which appended every unresolved
  surface independently of the response's candidate `WmWorkspaceState`.
  Committing phase one also discarded the excluded surface's unmanaged/retry
  bookkeeping. X Authority had supplied correct pixels and configure
  acknowledgements, while the fresh xmonad smoke correctly modeled policy
  ordering but did not exercise this visual-admission lifecycle.
- WM response projection now begins with committed layers and adds only
  planning surfaces assigned by that response's candidate workspace state.
  Committed relayout therefore leaves Firefox's admission state, retry count,
  and pre-admission authority group untouched; the following manage response
  alone releases and arms the exact candidate. Unmanaged admission ownership
  survives unrelated commits until candidate assignment, withdrawal, or
  removal.
- The retained lifecycle regression reproduces two committed surfaces, one
  pending PresentedBuffer, a recovery extent and standing target, phase-one
  relayout, and phase-two manage through exact retirement. The physical
  verifier accepts direct admission for future optimization; when a restart is
  observed it requires committed-layout/manage ordering, forbids phase-one
  arming, requires replay arming and retirement, and rejects a second restart.

## 2026-08-02: Firefox rendering and selection need independent physical gates

- The physical follow-up after restoring authority-group FIFO reached the
  exact recovery boundary: Firefox's 1280-by-1040 fallback retired, the
  1276-by-1422 standing target cleared the temporary extent, and subsequent
  native retirements carried complete 1276-by-1422 source, target, and clip
  rectangles in the left master column. The rendering regression was fixed
  even though the combined selection script later reported incomplete.
- That same run completed both direction-specific `CLIPBOARD` handoffs between
  Firefox and Kitty. It recorded seven owner changes and fourteen conversions.
  The operator stopped only when the harness switched to its separate PRIMARY
  choreography, so repeating clipboard work provides no new evidence.
- Rendering now has a one-Kitty/one-Firefox canary with no in-page interaction.
  A dedicated redacted page-ready title is distinct from the existing browser
  and Kitty checkpoints, and the verifier binds the action-created surface to
  a complete full-height left-column retirement. It accepts a future direct-
  admission path; if fallback is used, it requires one retained extent, an
  ordered standing-target clear/commit, and no more than one WM restart.
  Clipboard and PRIMARY remain selection-authority tests rather than rendering
  prerequisites.

## 2026-08-02: use yserver's layered regression model for the Firefox modal

- yserver's validation is layered rather than centered on one application
  script: ordinary Rust unit tests cover protocol encoding and state/event
  reducers; `proptest` covers parsers, properties, resource IDs, and state
  invariants; ignored Vulkan integration tests use pixel oracles, dma-buf
  export/re-import, and a 10,000-iteration FD-leak loop; XTS5 and rendercheck
  provide external protocol/graphics coverage; real Firefox, Chromium, GTK,
  and desktop sessions remain focused dogfood and x11trace comparisons.
- Its Firefox and GTK dialog investigations use the live application to expose
  a symptom, then retain the cause in smaller protocol tests—for example
  Present ConfigureNotify, descendant visibility, XI grab ownership, and
  button-release routing. Sophia already retains the admission-size and
  standing-target causes in Engine/session regressions, so the physical modal
  test should prove only the remaining cross-layer seam instead of replaying
  clipboard, PRIMARY, navigation, resize, and focus work.
- The new dialog canary has one Firefox surface and three monotonic redacted
  checkpoints: ready page, visible DOM modal, and confirmed modal. Both trusted
  clicks publish their own routed pointer batches. The verifier requires a
  complete 1276-by-1422 native retirement after the page checkpoint, after the
  modal checkpoint, and after confirmation. Because HTML `<dialog>` is not an
  X11 toplevel, any new frontend admission, stable-era restart/timeout, new
  recovery extent, incomplete clip, or GDK freeze is a hard failure. Existing
  unrelated surfaces and extra post-proof input do not invalidate the causal
  dialog result. Genuine transient X11 dialogs remain covered by the separate
  floating-policy wire tests.
- The 2026-08-02 physical gate passed: page-ready, modal-ready, and confirmed
  checkpoints each bracketed complete 1276-by-1422 Firefox retirements; routed
  pointer evidence covered both clicks; no surface was admitted after the
  stable page frame; and session, layout, and frontend cleanup were clean. The
  gate result closes the modal seam without requiring another replay of the
  operator sequence.

## 2026-08-02: isolate the remaining PRIMARY authority gate

- Cross-window `CLIPBOARD` and the Firefox dialog seam are accepted physical
  gates. Replaying them inside the old four-transfer selection script adds
  operator cost without increasing evidence for the remaining PRIMARY
  boundary.
- The PRIMARY-only slice starts with a dedicated Firefox source token. Its page
  checkpoint advances only after a trusted DOM selection event covers the
  complete field. Kitty accepts only that exact token through middle-click,
  publishes a redacted receipt checkpoint, and exposes a distinct return
  token; Firefox completes only after the exact return arrives through its
  PRIMARY target.
- The verifier binds each direction to its own ordered selection-owner change
  and conversion interval. Four monotonic redacted checkpoints make stale or
  out-of-order transfers fail closed, while an explicit negative fixture
  rejects any CLIPBOARD checkpoint. This leaves one short physical authority
  test rather than another combined browser workflow.

## 2026-08-02: the focused PRIMARY run isolated the reverse direction

- The live pointer trace delivered physical button 2 to Kitty after the focus
  handoff. Kitty issued PRIMARY conversions; its GLFW diagnostic therefore did
  not implicate the evdev-to-core-button mapping. More importantly, the
  focused coordinator wrote `checkpoint-primary-received` at 20:28:00. That
  checkpoint is emitted only after Kitty reads the exact Firefox token, so the
  Firefox-to-Kitty same-namespace transfer passed during the run.
- After Kitty exposed and selected its return token, owner-change evidence
  advanced and Firefox issued new conversions, but the Firefox confirmed-title
  checkpoint never appeared. `Ctrl+Shift+C/V` exercised CLIPBOARD and was not
  evidence for this remaining PRIMARY direction. The old value-free counters
  cannot distinguish a negative SelectionNotify, a completed property read, or
  an exact-token mismatch.
- XLibre `ProcConvertSelection` and yserver `handle_convert_selection` both
  route SelectionRequest to the current owner and rely on the owner to change
  the requestor property and send SelectionNotify. Sophia's existing wire test
  covered that sequence only in one direction. It now reverses ownership and
  performs the complete property/notify/read/delete sequence back through the
  original requestor. The focused launcher also enables the already-redacted
  live stages (`request_routed`, property notification, notify, and property
  read), while the page reports a nonempty mismatched token separately from no
  paste. This preserves exact-token acceptance without another combined gate.

## 2026-08-02: preserve SendEvent on selection-owner notifications

- The detailed follow-up again proved Firefox-to-Kitty PRIMARY through the
  coordinator's exact-token checkpoint. On the reverse transfer Firefox sent
  its target negotiation and content conversions; Kitty changed the Firefox
  property and Sophia routed successful SelectionNotify events, but Firefox
  never followed them with GetProperty. This placed the failure after routing
  and before the requestor's property read.
- XLibre `ProcSendEvent` unconditionally adds `SEND_EVENT_BIT` to the delivered
  event type. yserver follows the same rule by copying the supplied event and
  setting bit 7 before per-recipient fanout. Sophia's SendEvent decoder instead
  converted the event into a typed SelectionNotify without retaining that
  semantic, and its encoder consequently wrote ordinary event type 31 instead
  of synthetic type `0x9f`. Simple wire clients tolerated the difference;
  Firefox did not accept the owner notification as the expected SendEvent.
- SelectionNotify now carries an explicit synthetic flag. Client SendEvent
  decoding sets it regardless of the template bit, while server-generated
  negative and clipboard-proxy notifications remain ordinary events. The
  existing bidirectional same-namespace regression now asserts exact `0x9f`
  delivery in both directions before reading and deleting each property.

## 2026-08-02: separate routed selection events from flushed delivery

- The first one-Firefox run after preserving the SendEvent bit reached both
  reverse conversion requests. Kitty wrote the Firefox requestor properties
  and Sophia accepted two successful SelectionNotify events into the correct
  routed channel, but Firefox again made no GetProperty request. This proves
  that `notify_routed` was not a sufficient delivery oracle and that the
  standards fix alone did not close the real-client seam.
- Diagnostic protocol writers now emit a redacted record only after the
  recipient Unix socket write and flush succeed. Selection request, notify,
  and clear records include the recipient client, recipient sequence,
  timestamp, resource IDs, atoms, property presence, and synthetic flag, but
  no property payload. The focused verifier requires a flushed synthetic
  property-bearing notify between each conversion and its consumer checkpoint.
- The same run also produced zero title checkpoints even though the dedicated
  page was visible. PRIMARY diagnostic mode now records each observed
  `_NET_WM_NAME` byte length before applying the monotonic checkpoint reducer.
  The next run can therefore distinguish incorrect canary lengths from missing
  metadata without exposing title content or adding another operator step.

## 2026-08-02: accept the causal PRIMARY checkpoints

- The instrumented physical run flushed Firefox's selection request and
  synthetic property-bearing notification to Kitty, which read the exact
  22-byte source token and published its receipt checkpoint. After Kitty took
  PRIMARY ownership, Firefox negotiated targets and received a flushed
  synthetic UTF-8 notification; the page immediately published its exact-token
  confirmation. This closes both real-client directions without another
  operator replay.
- The session reducer nevertheless reported zero of four checkpoints because
  it waited first for a 250-byte initialization title. Firefox coalesced that
  property update before the global metadata observer saw it, while the
  trusted-selection 251-byte title, Kitty's 253-byte receipt, and Firefox's
  252-byte confirmation were all observed in causal order. Page initialization
  is redundant once a trusted full-field selection has armed the source.
- The focused proof now begins at `source_armed` and retains only the three
  causal checkpoints. Its verifier brackets each transfer with the matching
  owner change, conversion, and socket-flushed synthetic notification, so
  removing the unobservable initialization marker does not weaken selection
  authority coverage and avoids coupling the gate to property-update timing.

## 2026-08-03: close the focused Firefox lifecycle gate

- The one-purpose physical slice launched two independent Kitty processes and
  two Firefox processes. Six ordered Kitty checkpoints bracketed the first
  Firefox's normal `Ctrl+Q` exit, the second launch, and its WM-forced close.
- Both Firefox processes reported managed status-zero exits. The forced path
  included the committed `CloseFocused` action before process retirement, so
  it did not pass through a page or harness shortcut.
- The strict verifier passed with zero protocol errors, pending WM/actions/input,
  recovery extents, or constraint relayout. Application groups and frontend
  workers drained, and the namespace and Xauthority were revoked. All focused
  Firefox gates are now closed; only the integrated promotion run remains for
  Milestone 10.

## 2026-08-03: remove selection choreography from Firefox promotion

- The combined promotion script still made the operator shuttle four payloads
  between Firefox and Kitty even after the focused CLIPBOARD and PRIMARY gates
  had closed those boundaries. Repeating exact-token ownership work inside the
  integrated run tested instruction following, not an additional Sophia seam.
- Promotion now has six browser stages: loaded keyboard input, navigated
  document scroll, resize, focus away/return, and dialog confirmation. Its two
  Kitty peers retain only the six before/normal-close/forced-close checkpoints.
  The completion schemas explicitly record `selection_gates=focused`, and the
  verifier rejects any replayed clipboard/primary stage or peer checkpoint.
- The original eight-stage reducer remains authoritative for the QEMU M8 and
  focused selection profiles. Promotion uses a profile-specific source-stage
  mapping, preserving the established title canaries for scroll through dialog
  while reporting compact indices zero through five. This keeps future reducer
  optimization independent from page presentation and avoids falsifying skipped
  selection stages.
- Milestone 10 now requires one integrated physical run after its focused gates.
  Repetition and flake detection move to the unattended installed-session soak
  rather than multiplying a manual release ritual.

## 2026-08-03: complete the shortened physical Firefox promotion

- The first shortened integrated run completed all six Firefox stages and all
  six Kitty retention checkpoints around normal and WM-forced status-zero
  Firefox exits. Session health reported zero protocol errors or pending work,
  layout health was clean, and application, frontend, namespace, and
  Xauthority teardown drained completely.
- The verifier exposed two stale assumptions rather than product failures. A
  damage-idle secondary output retained its proven synchronous startup modeset
  and correctly issued no redundant asynchronous page flip; output liveness is
  now proved per output at startup while the gate separately requires at least
  one asynchronous retirement. This preserves future damage-skip optimization.
- Firefox completed real DOM wheel handling and document displacement after
  one causally ordered post-navigation packet because GTK's XI2 absolute-axis
  baseline had already been established earlier in the same device session.
  The integrated gate now requires that causal packet plus browser-observed
  DOM completion. Focused protocol gates retain the stricter fresh-baseline
  coverage, without forcing redundant operator notches in promotion.
- Super+Space moved two surfaces but resized only Firefox, so the resize epoch
  correctly matched one configured surface while the workspace projection
  retained all three managed windows. Firefox reported its DOM resize after
  receiving ConfigureNotify and presented the exact new-size pixels shortly
  afterward. The verifier now keeps those distinct causal facts instead of
  requiring three resized surfaces or demanding pixel retirement before the
  client could report receiving the configure.
- Native page-flip and X11 focus diagnostics are tracing records and therefore
  carry timestamp/level prefixes in the production log, unlike owner-loop
  proof records. The verifier fixture now models that prefix and matches the
  embedded structured marker, preventing another fixture-only anchoring bug.

## 2026-08-03: separate transition rationale from the implementation audit

- Review of `state-and-transition-discipline.md` found that its transition
  systems, I/O-automata, single-writer, and CALM rationale remains useful, but
  its dated conformance section was becoming a second roadmap. The rationale
  is now evergreen; current gaps remain dated evidence here and become planned
  work only when admitted and ordered in `todo.md`. Milestone 11 remains the
  active roadmap priority.
- The audit confirmed that `AuthorityTransactionIntake::commit` and
  `ProductionSessionCoordinator::{commit_authority_batches,
  replace_committed_surfaces}` can advance or replace state outside the
  prepared-presentation retirement path. `PreparedSurfaceCommit` application
  is protected by coordinator call order rather than a type that binds the
  exact prepared scene and submission to the required output retirements.
- Per-output and backend assembly replacement APIs still expose mutable copies
  shaped like alternate committed-state writers. A future admitted design may
  replace these with immutable, generation-tagged scene projections and a
  retirement capability owned by the Engine coordinator. Any such capability
  must bind the exact candidate, submission, and required output-retirement
  set. A failed retirement is terminal settlement, never authority to commit.
- `PortalRequestGrantLifecycle` enforces central capacity, duplicate, and
  generation rules, while `ClipboardPortal`, `DragAndDropPortal`,
  `FileHandoffPortal`, `ScreenCapturePortal`, `UriOpenPortal`, and
  `NotificationPortal` still insert directly by transfer ID. Consolidating
  those public admission paths is a future hardening candidate, not work
  admitted by this documentation review.
- The repository has deterministic transition tests but no TLA+ module and no
  unified authority-transition ledger. A formal model remains an optional
  validation candidate whose toolchain and reproducible command must be
  admitted before implementation. A future privacy-safe transition ledger or
  trace would be observational only: it may correlate opaque identities,
  generations, actions, settlements, and submissions, but must never replay
  authority effects or retain protocol identities, payloads, metadata, or
  pixels.
- Terminology now reserves committed visual state for post-retirement truth,
  distinguishes earlier accepted or prepared state, treats presentation as
  output-scoped rather than globally simultaneous, and applies CALM locally:
  each authority orders its own non-monotonic decisions; only decisions that
  bind several ownership domains require cross-authority coordination.

## 2026-08-03: admit bounded formal transition validation after installation

- The roadmap now keeps Milestone 11 focused on the installed product path,
  adds one unattended formal-transition gate before the Milestone 12 soak, and
  requires Milestone 13 lifecycle optimizations to extend that model before
  changing frame-slot, coalescing, multi-output, shared-worker, scanout, or
  resource-release semantics. TLA+ adds no physical operator choreography.
- `validation/tla/VisualRetirement.tla` models proposal, preparation,
  submission, output-scoped retirement, rejection, timeout, disconnect,
  removal, and release. Its boundary map names the corresponding Engine and
  frame reducers while explicitly recording that this is not a refinement
  proof and must not normalize the direct-commit gaps found by the preceding
  audit.
- The first three-generation configuration exceeded one million distinct
  states without adding a new ordering class. The retained configuration uses
  two outputs and two generations: the smallest bounds that still exercise
  out-of-order retirement and supersession. TLC v1.7.4 exhaustively checked
  12,348 distinct states to depth 17 with all safety invariants and the
  admitted-work liveness property passing.
- `tools/check_tla.sh` requires an absolute path to the pinned official jar,
  verifies its SHA-256, runs one worker with a fixed fingerprint polynomial,
  and isolates TLC state in a temporary directory. The ordinary command is
  offline. Valid terminal quiescence is not treated as a deadlock, while the
  explicit weak-fairness liveness property remains checked.

## 2026-08-03: install the immutable Milestone 11 release

- Signed commit `ff8cb2f9aa76f7f46601891241b19ff947b2d67e` produced immutable
  release `0.1.0-ff8cb2f9aa76`. Packaging built the optimized Sophia CLI and
  generic X11 WM bridge offline, copied the resolved xmonad executable, and
  recorded the complete artifact manifest and SHA-256 ledger.
- The staged installer and rollback fixture passed, followed by an exact
  unprivileged installation of the release artifact into an isolated prefix.
  The system installation then promoted the same verified artifact to
  `/opt/sophia/releases/0.1.0-ff8cb2f9aa76`, made it the `current` target, and
  retained `0.1.0-21002fe74c2a` as `previous`.
- Every installed manifest digest passes. All twelve public operator commands
  resolve through `/usr/local/bin` into the immutable current release, and the
  xmonad, Kitty-baseline, and Firefox-proof greetd entries execute only
  `/opt/sophia/current/bin/*`. Ordinary login therefore requires no checkout,
  source build, temporary artifact path, privileged service operation, or
  process cleanup.
- This closes the installation mechanism gate, not the physical-login gate.
  The installed release still needs the retained chrome proof, normal login
  and logout captures, independent recovery and fallback evidence, and the
  documented operator handoff required by the remaining Milestone 11 items.

## 2026-08-03: make installed startup failure phases explicit

- Installed launch already emitted ordered entering/complete lifecycle phases,
  but every nonzero exit ended with the same handoff record. `sophia-status`
  tailed that record, so an operator could not distinguish preflight, input
  guard, graphics takeover, session, or restoration failure without reading
  the complete log.
- The session wrapper now carries the verified manifest version and commit into
  the runner. A shared lifecycle helper emits one bounded diagnostic containing
  only that release identity, the exact enumerated phase, installed flag, and
  exit status. The current phase advances before each phase's first side
  effect, while a failed TTY, keyboard, keyd, or termios restoration overrides
  the source phase with `handoff`.
- User-requested Ctrl-Alt-Backspace recovery remains an expected emergency and
  is not mislabeled as a startup failure. A watchdog deadline remains a session
  failure even though it follows the emergency cleanup path. The ordinary
  lifecycle and recovery schemas are unchanged, preserving the existing
  promotion verifiers.
- `sophia-status` now prints the newest diagnostic exactly once beside the
  verified installed manifest and final lifecycle result. The regression drives
  an installed-style noninteractive preflight failure, exercises all five
  allowed phase values, rejects an invalid phase, and proves the status output
  retains no duplicated diagnostic. Packaging carries the helper inside the
  immutable artifact instead of reaching back into the repository.
- Signed commit `09113a7da149a57558deea8076529913f9a62705` was packaged as
  `0.1.0-09113a7da149` and promoted to `/opt/sophia/current`. The complete
  installed digest ledger passes, `/usr/local/bin/sophia-status` resolves into
  that release, the packaged lifecycle helper is present, and
  `0.1.0-ff8cb2f9aa76` remains the immutable rollback target. This closes the
  diagnostic mechanism item; the next installed login supplies its physical
  lifecycle observation without a separate operator sequence.

## 2026-08-03: retained recovery pixels must remain live

- The first normal login through installed release `0.1.0-09113a7da149`
  completed all eight lifecycle records, returned through the display-manager
  handoff with exit status zero, restored the original KD mode and termios,
  left no graphical process, and emitted no failure diagnostic. The session
  reached clean protocol, layout, presentation-resource, and frontend cleanup
  summaries.
- Ordinary use nevertheless exposed a visual liveness failure. A 300-by-300
  GLX surface launched from Kitty displayed one `glxgears` frame and then
  remained static. The surface retired exactly one Present while Firefox
  retired 1,400; the completion summary recorded 188,148 controlled Present
  rejections and two recovered WM layout timeouts.
- The GLX surface had been admitted at its coherent 300-by-300 recovery extent
  while retaining a 1276-by-709 blind-WM target. After the first admission
  frame retired, `present_layout_disposition` compared every newer
  300-by-300 buffer only with the standing target and returned
  `RejectLayoutMismatch`. Immediate skipped feedback let the client run
  unpaced, but no newer GLX buffer could reach Engine preparation or native
  retirement.
- XLibre/Xorg does not discard `PresentPixmap` solely because its pixmap and
  current window extents differ: the non-flip path copies the applicable
  region and settles the request. Yserver independently selects its Copy path
  when a flip is unavailable and keeps Present scheduling separate from its
  ordered Present/core `ConfigureNotify` delivery. Sophia retains its stronger
  atomic geometry rule by displaying these updates only at the already
  coherent recovery geometry; neither reference requires publishing the
  outstanding target before matching pixels exist.
- Engine now owns a protocol-neutral extent classifier with four results:
  unconstrained, exact expected target, explicitly retained recovery extent,
  and mismatch. The live presentation gate schedules newer buffers in the
  retained-recovery class immediately while leaving the standing target,
  committed geometry, and exact-target retirement rules unchanged. Unrelated
  extents still fail closed, and X authority continues to own X11 Present and
  ConfigureNotify semantics without gaining layout policy.
- The crate-boundary regression repeats retained recovery classification,
  proves that it does not discharge either the extent or target, accepts the
  exact target separately, rejects an unrelated extent, and rejects the old
  extent again once recovery is cleared. All Engine layout-epoch tests and the
  all-feature workspace suite pass. A packaged physical GLX rerun remains the
  acceptance boundary.

## 2026-08-03: installation resolves the current release automatically

- Requiring an operator to copy a commit-derived artifact directory into the
  privileged install command made the ordinary release path unnecessarily
  error-prone. The repository already had an exact-current-commit resolver,
  but it was exposed under a second command instead of the documented
  installer.
- `tools/install_live_session.sh` with no argument now delegates to that
  resolver. It packages only a clean current commit when its immutable artifact
  is absent, verifies the manifest commit and full SHA-256 ledger, requests
  privilege only for the default system prefix, and then performs the existing
  atomic install. Supplying one explicit artifact directory remains supported
  for staged validation and recovery tooling.
- The install regression constructs an exact-current-commit artifact in an
  isolated root and exercises the argument-free command through digest
  verification, release promotion, session entry installation, and final
  manifest identity. Existing explicit-artifact install and rollback coverage
  remains unchanged. The operator command is now simply
  `tools/install_live_session.sh`.

## 2026-08-03: cursor motion must not serialize animated primary flips

- The installed retained-recovery repair made a 300-by-300 GLX surface animate,
  but continuous pointer motion reduced presentation from a stable 60 FPS to
  16--46 FPS. The clean completion recorded 5,460 physical pointer events,
  1,376 coalesced moves, 342 hardware updates, 1,859 primary-in-flight cursor
  deferrals, a 16 ms maximum cursor update, zero native submission failures,
  and a 41.390 FPS mean with a 66.718 ms p95 frame interval.
- Backend-live was issuing a synchronous cursor-plane atomic commit for each
  admitted move. That commit waited for a vblank and alternated with the
  nonblocking primary page flip on the same DRM device, so correctness
  serialization itself consumed the animated client's presentation cadence.
- XLibre's modesetting driver uses `drmModeSetCursor2` to install a cursor and
  `drmModeMoveCursor` for steady motion, after querying
  `DRM_CAP_CURSOR_WIDTH` and `DRM_CAP_CURSOR_HEIGHT`. Yserver independently
  documents the same primary/cursor atomic serialization failure and landed
  the same legacy-ioctl repair. Niri avoids the race through one compositor
  frame/KMS owner that folds cursor plane state into output presentation;
  river delegates the equivalent ownership to wlroots.
- Sophia now retains one-time synchronous atomic detachment solely to sanitize
  inherited cursor planes before presentation begins. It then hides every
  selected CRTC through the legacy interface, installs the canonical raster
  with `set_cursor2`, uses `move_cursor` on the active CRTC, and performs an
  ordered hide/install/move when crossing outputs. Controller state advances
  only after each successful ioctl, and teardown hides the cursor before
  destroying its dumb buffer.
- Primary in-flight state may defer this one-time initialization, but it no
  longer defers steady legacy cursor movement. The public backend seam contains
  no atomic commit method, and crate-boundary regressions lock driver-cap
  fallback, ioctl ordering, retryable initialization, partial-failure state,
  and primary-in-flight admission.
- Cursor completion schema 4 reports `path=legacy_ioctl`, initialization time
  and deferrals, steady update time, and successful updates while a primary
  flip is in flight. The bounded GLX proof now requires continuous pointer
  motion, at least 55 FPS, at most a 25 ms p95 frame interval, a steady cursor
  update at most 20 ms, positive cursor/primary overlap, and zero cursor or
  native failures. A future Milestone 13 all-atomic implementation must unify
  primary and cursor state under one per-output transaction owner rather than
  restore independent atomic cursor commits.

## 2026-08-04: Logout must settle its committed WM update

- The pointer-motion rerun validated the legacy cursor repair even though a
  manual Logout preempted the bounded benchmark completion. After physical
  motion was observed and routed, output 1 accepted 1,116 page flips over
  18.600 seconds: 59.945 FPS with an 18.670 ms worst observed interval. The
  prior standalone atomic cursor path had fallen to 41.390 FPS with a 66.718
  ms p95 interval. Native suspension then drained the final scanout and TTY
  recovery restored termios and KD state normally.
- Super-Shift-Q arrived just before the workload's automatic 20-second exit.
  The same committed WM response carried the Logout session action and an
  ordinary `WmTransactionUpdate`. The owner executed Logout and its existing
  exit gate observed empty input-delivery, key-release, and X-control queues,
  but it did not inspect `pending_wm_update`. It therefore left the loop before
  the synthetic coordinator batch could deliver transaction 2 to Engine. The
  final clean-work assertion correctly rejected the remaining update.
- The session shutdown policy now models Running, Draining, and Complete from
  passive queue facts. A requested Logout remains Draining while input,
  key-release, X-control, or WM-update work exists. This gives the existing
  authority/runtime path one bounded owner cycle to consume the committed WM
  update; it neither discards the update nor weakens the final zero-debt
  assertion. Crate-boundary tests reproduce the exact lone-WM-update state and
  retain every pre-existing delivery barrier.

## 2026-08-04: asynchronous Present skips retain the display timeline

- The first default `vkcube --wsi xcb` run after the GLX cursor-cadence repair
  produced a visible but static cube. The 500-by-500 surface submitted one
  Present while its manage transaction held a resize epoch. That epoch timed
  out, aborted the queued Present, retained its pixels through the coherent CPU
  recovery snapshot, and later committed those pixels without receiving a
  second Vulkan frame.
- The abort did route Complete/Skip and Idle exactly once, so resource release
  was not missing. It instead stamped the asynchronous completion with
  `UST=0, MSC=0`. XLibre executes a skipped pixmap Present against the current
  CRTC UST/MSC. Yserver independently replaced zero-clock Present completions
  after real clients rejected them as invalid. Sophia's successful native
  completion path already used the kernel page-flip clock; only policy-driven
  rejection reset the client-visible timeline.
- The protocol-neutral live feedback coordinator now retains the most recent
  successful display sample. Scheduler rejection, supersession, layout
  rollback, native detach, and shutdown skips reuse that sample rather than
  fabricating a new origin. Page-flip success continues to refresh the sample,
  and early startup retains the existing zero fallback until a real display
  sample exists.
- A crate-boundary regression completes one frame at a nonzero kernel sample,
  asynchronously skips its successor, and requires Complete/Skip plus Idle at
  the retained UST/MSC with all presentation resources retired. The packaged
  physical vkcube rerun remains the acceptance boundary.

## 2026-08-04: fixed admission recovery preserves the first Vulkan Present

- Installed release `f007757a` preserved nonzero Present timestamps but did
  not restore vkcube animation. Three independent 500-by-500 launches followed
  the same path: the first Present selected the admission pixels, the blind-WM
  layout timed out, rollback drained that Present, and a CPU snapshot later
  made one static frame visible. No vkcube transaction reached native
  retirement.
- Live process inspection showed that vkcube had not crashed. Its main thread
  and FIFO WSI queue thread were parked on condition variables, its X event
  thread was waiting for another event, and its X socket had no unread data.
  The intervening 300-by-300 GLX workload retired 851 frames, isolating the
  failure to Vulkan's first-Present lifecycle rather than KMS, composition, or
  general DRI3 progress.
- Mesa's X11 WSI records each FIFO Present by serial and waits for that exact
  completion before queuing another image. XLibre reports Skip only after a
  pixmap or window ceases to exist; an ordinary non-flippable pixmap is copied
  and completed. Sophia instead destroyed a still-coherent admission Present,
  reported Skip, and displayed its copied pixels through a second transaction.
  Correct timestamps could not repair that split lifecycle.
- Layout rollback now reconciles each staged Present with Engine's fixed
  recovery extents. A managed resize or mismatched source remains stale and is
  rejected. An admission source whose descriptor exactly matches its recovery
  extent stays layout-fenced. It is released only after the recovered surface
  enters the committed presentation projection, and its previous generation
  is rebased to any CPU snapshot committed during admission. The preserved
  DMA-BUF can then retire normally through KMS and produce one coherent
  Complete/Idle lifecycle.
- Crate-boundary regressions require exact extent matching, continued deferral
  while the surface is absent, generation rebasing at visibility, normal
  eligibility afterward, and rejection of a one-pixel mismatch. The packaged
  physical vkcube rerun remains the acceptance boundary.

## 2026-08-04: surface content must carry pixels through the same order gate

- The post-vkcube audit found that the existing surface fence delayed later
  `SurfaceTransaction` values but not their CPU patch payloads. Renderer
  updates remained envelope-scoped and were applied before fence admission, so
  a stable CPU handle could expose future pixels during an unrelated repaint.
  Software Present did not arm the fence, and a released batch rebased every
  group against the same committed generation.
- Engine now owns a bounded, protocol-neutral `SurfaceContentStream`. It tracks
  exact `SurfaceTransactionKey` owners per surface and retains opaque payloads;
  multi-surface work waits for every owner, later work cannot pass an earlier
  overlapping group, removals remain nonblocking, and shutdown discards bounded
  debt explicitly. The reducer contains no X11 or renderer types.
- A live authority group now carries its CPU buffer mutations beside its
  transactions. Production admission happens before those mutations reach the
  renderer. DMA and software Presents acquire exact content ownership, and
  retirement or rejection releases the FIFO backlog into the next ordinary
  production cycle. Released groups run before new authority work and rebase
  sequentially; their buffer handles remain residency roots while deferred.
- Admission quarantine retains the same grouped pixel payload and removes it
  from the projected batch until release. X authority remains the sole owner of
  Present, SHM, clear, core-drawing, copy, and clipping semantics; Engine sees
  only the reduced surface order. The authority regression verifies one stable
  handle and generations 1 through 4 across Present, SHM, clear, and core draw.
  The Rust stream regressions cover exact settlement, multi-owner FIFO,
  independent progress, removal, capacity, and shutdown.
- `SurfaceContentStream.tla` models the Present owner, three representative
  deferred operations, independent progress, retirement, visible generations,
  and fair drain. The pinned TLA+ Tools 1.7.4 gate explored all 28 distinct
  states and found no safety or liveness error; the three established models
  passed in the same reproducible run.

## 2026-08-04: installed recovery keeps the deadline outside Engine

- The development launcher already placed Sophia in a fresh process group and
  kept its optional wall-clock deadline in the outer shell. That topology is
  the correct recovery boundary: the deadline can terminate the complete
  graphical group, while the surviving launcher still owns the saved keyboard,
  KD, and terminal state. Duplicating this timer in Engine would make recovery
  depend on the component being contained.
- A daily desktop cannot carry a finite wall-clock lifetime. The immutable
  release now adds a separate `Sophia Recovery Proof` greetd entry whose thin
  installed wrapper alone selects a 45-second deadline. The normal xmonad,
  Kitty, and Firefox entries leave the deadline unset.
- Watchdog containment is not a clean logout. Its verifier requires visible
  startup readiness, one exact process-group deadline, an armed but untriggered
  local guard, restored VT state, and an installed status-124 display-manager
  handoff. The recorder archives this evidence separately from status-zero
  login cycles and status-130 graceful emergency-chord runs.
- Staged-install regressions require the wrapper, operator recorder, desktop
  entry, current-release symlinks, and rollback survival. Fixture regressions
  reject a changed deadline, local-chord substitution, graceful-shutdown
  relabeling, a missing status-124 diagnostic, and a normal lifecycle handoff.

## 2026-08-05: direct-map placement must retain compositor chrome

- The first installed no-WM fallback after direct MapWindow ownership was
  restored mapped Kitty and retired 16 animated Presents. Its 2556-by-1422
  content remained at output origin, however, while the focused two-pixel
  compositor ring extended outside that content. The output clipped the
  negative left and top bands; the right and bottom bands remained visible.
- Border generation was symmetric and X Authority's client geometry was
  coherent. The ownership error was in live layout: centering the first
  policy-managed surface depended on a startup-input proof flag that normal
  sessions always disable. No external WM existed to supply another
  placement.
- Direct mapping now also declares Engine ownership of initial placement. The
  first toplevel is centered inside the first output without changing its
  source extent or its authority transaction. Deferred mapping continues to
  leave final placement and chrome clearance to the external WM.
- A live-reducer regression reproduces the installed 2560-by-1440 output and
  2556-by-1422 Kitty content, requires the compositor target at +2+9, verifies
  that a two-pixel outer ring remains within output bounds, and proves the X
  transaction geometry is unchanged.
- Installed commit `a752ca27` confirmed that target for all 14 retired Kitty
  Presents and composed the focused ring with two-pixel clearance. The session
  ended with clean health, zero native submission or retirement failures, and
  no pending native cleanup; the operator confirmed the complete border.
- The automatic fallback archive was marked failed for an independent gate
  condition. Output 2 completed its synchronous startup modeset and recorded
  one nonzero export, but received no later damage and therefore no
  asynchronous page flip. The then-current verifier required an asynchronous
  retirement on both outputs. That promotion-policy question remains separate
  from the accepted direct-map placement fix.
- The fallback verifier now uses the same contract already retained by the
  integrated Firefox gate: exactly two unique synchronous startup-output
  records prove per-output liveness, and at least one asynchronous retirement
  proves active scene progress. Per-output nonzero export summaries and clean
  native drain remain mandatory. The pass fixture now models an idle second
  output; mutations reject a missing or duplicate startup identity and a
  session with no asynchronous retirement. Installed archive `0004` passes
  this corrected session-level verifier without changing its evidence.

## 2026-08-05: Launch capacity has an isolated QEMU workload

- The session launch limit covers active secondary applications plus pending
  admissions. A primary startup client does not consume that ledger. The QEMU
  profile therefore starts one visible Xterm and twelve managed `sleep`
  children, leaving exactly four slots without turning the capacity proof into
  a sixteen-pane layout benchmark.
- One QMP connection sends 32 Super-Enter chords with operator-like key holds.
  Identical actions may coalesce while a WM request is in flight, so acceptance
  does not invent an exact rejection count. It requires four queued and
  admitted launches, 20 through 28 capacity rejections, and a committed-action
  balance that includes the later recovery launch.
- Capacity recovery is causal rather than time-based: the harness waits for a
  managed holder exit, admits one more Xterm, observes its X11 focus
  acknowledgment, then requires a Super-J policy commit and the resulting
  focus acknowledgment. The final rebuilt QEMU run admitted four burst
  launches, rejected 27, admitted the recovery launch, and drained with zero
  admission timeouts, stale WM responses, WM restarts, or native cleanup debt.
- The verifier permits a structured startup record to carry a harmless stderr
  prefix because guest processes share a serial byte stream. It also accepts a
  focus acknowledgment immediately before or after the harness writes its
  derived admission marker, while requiring the real acknowledgment before the
  readiness marker. Mutations reject missing preload, rejection, recovery,
  focus, action, output, and cleanup evidence.

## 2026-08-05: Stale WM replies require a fresh speculative peer

- The external xmonad bridge applies each request to its private model before
  replying. Sophia formerly rejected a response when one of its fingerprinted
  surfaces disappeared, then immediately sent the next queued request to that
  already-mutated peer. A queued removal could therefore be planned against a
  synthetic surface that no longer existed and terminate the session with
  `UnknownSurface`.
- The committed workspace state could also retain a removed surface when the
  pending removal request was discarded during restart. Response-lifetime
  reconciliation now removes only fingerprinted surfaces that vanished from
  the Engine-owned persistent layout. A pending `ManageSurface` remains absent
  from committed state; an already-committed removal is deleted before reseed.
- A stale response now requests transport restart and suppresses queue pumping.
  The owner terminates the speculative peer, clears its in-flight and queued
  protocol work, starts a fresh bridge, and reseeds it from reconciled committed
  state. This is distinct from a later Engine proposal rejection, which already
  carried its source through the commit boundary and restarted there.
- The diskless `xmonad-stale-response` QEMU profile retains two Xterms and maps
  a third whose child exits after 50 ms, inside the bridge's 80 ms quiet period.
  The rebuilt production run observed the action surface and normal exit,
  rejected one stale Manage reply, restarted and reseeded once, preserved both
  persistent surfaces, completed a physical Super-J focus cycle, and logged a
  clean normal shutdown. The transport summary reported one stale response and
  zero pending work; schema 16 reported one WM restart, no degradation, and no
  native submission, retirement, callback, or cleanup failure. Mutation tests
  reject missing causal stages, duplicate restart, incorrect counters, the
  historical `UnknownSurface` error, and an exit that was never surface-backed.

## 2026-08-07: Workspace isolation includes legacy geometry and admission pixels

- Installed preliminary soak attempt `0054` ran for 413,133 milliseconds and
  returned through clean normal logout and exact TTY recovery. Native scanout,
  callbacks, protocol handling, application cleanup, and input drain were
  clean, but the immutable verifier correctly failed the run after four layout
  timeouts and four xmonad-bridge restarts.
- The workspace-specific defect was exact. A Kitty launch on workspace 2
  committed one visible surface while moving and configuring three; the two
  extra surfaces belonged to hidden workspace 1. A later Firefox launch on
  workspace 3 repeated the three-surface configure set and timed out. The
  bridge removed hidden windows from its mapped set and filtered their focus,
  but translated late `ConfigureWindow` requests retained by xmonad's private
  state into real Sophia configure and render commands.
- The compatibility boundary now rejects geometry from every known but
  unmapped synthetic window while continuing to reject unknown windows as
  protocol errors. A pure translation regression mixes hidden and visible
  configure and focus requests. A process-level synthetic-X regression keeps
  stale layout state across unmap, emits both geometries during the next
  admission, and requires only the visible surface to cross the bridge.
- The same run exposed an independent first-admission ordering defect.
  Firefox, glxgears, and vkcube had already produced complete pixels at
  500-by-570, 300-by-300, and 500-by-500 extents, respectively. The first WM
  proposal nevertheless requested the final tile immediately, waited for
  pixels the clients had not produced, timed out, and only then used the safe
  extent that made the retry succeed.
- Admission now primes that existing Engine-owned safe observation as a
  temporary fixed extent before constraint reconciliation. Sophia retains the
  blind WM's different size as a standing target, commits and retires the
  selected pixels, clears the temporary extent, and drives the target through
  the ordinary exact-pixel relayout path. This is an event-driven ordering
  repair; it does not lengthen the two-second deadline, weaken atomic visual
  admission, or teach Engine about X11 or application identity. A short
  installed successor proof remains required before another long soak.
- The previous formal suite did not cover either boundary. `PolicyProjection`
  began after compatibility reduction, while `AdmissionRecovery` required a
  timeout before fallback. `LegacyWmProjection` now explores delayed configure
  and focus around complete workspace replacement. `AdmissionRecovery` now
  separates proactive safe-pixel priming from timeout without an observed
  candidate and proves that observed pending admission becomes managed. The
  pinned TLC gate passes all models; the new projection model explores 270
  distinct states to depth 10 and the revised admission model explores 84
  distinct states to depth 12.

## 2026-08-07: Specula exposed four missing compatibility boundaries

- A commit-pinned, development-only Specula audit examined complete legacy-WM
  projection, delayed Configure/Focus responses, restart/reseed, and safe-pixel
  admission. It used a clean clone of Sophia commit `ef918108` and Specula
  commit `3946f892`; eleven focused configurations found four implementation
  defects. Specula remains outside Cargo, packaging, and the installed session.
- An unmodified legacy WM cannot attach Sophia transaction identity to its X
  requests. A delivered-channel drain therefore cannot prove that a
  socket-buffered or scheduled reply belongs to the next request. Successful
  collection now requires a final quiet boundary; reaching the hard deadline
  is failure. Any request error poisons that private runtime, causing the
  existing supervisor to replace and reseed it before later Engine work.
- Complete workspace packets now replace cached membership exactly while
  preserving stable synthetic XIDs. Direct `AssignWorkspace` mutates that same
  unique membership before returning the Engine command, and workspace
  activation derives mapping from it. Omitted or moved surfaces are unmapped,
  so their delayed Configure and Focus requests remain private.
- A surface may have presentation intent without any complete pixel extent.
  The first such admission timeout is now an expected bounded state: it keeps
  the owner loop and standing target alive, records one retry, and bypasses
  fixed-extent recovery until safe pixels exist. Persistent silence still
  follows the ordinary retry/withdrawal policy.
- Deterministic Rust regressions preserve all four counterexamples. The revised
  `LegacyWmProjection` model checks 2,106 distinct states to depth 11;
  `LegacyWmResponseBoundary` checks 6,417 to depth 39; and
  `PixelSilentAdmission` checks 11 to depth 5 with its liveness properties.
  The pinned project checker passes all models.
- Restart/reseed remained clean in five exhaustive Specula configurations:
  302,541,189 distinct states to depth 40, 4,159 to depth 20, 595 to depth 18,
  and two 90,181-state searches to depth 28. Two initially stronger generated
  invariants were corrected rather than imposed on Sophia: retained fallback
  pixels may differ from a later exact successor, and only expected
  Configure/Focus replies have a current-request obligation.
- Final candidate-identity and ownership-exclusivity simulations each reached
  the full 30-minute watchdog without a violation. They checked 701,155,271
  states across 44,868,415 traces and 707,143,607 states across 50,238,084
  traces, respectively.
- Specula's optional post-validation agent confirmation was stopped after five
  provider-policy false positives on the first benign local X11 reproducer. It
  is not part of the cited evidence; validated traces, exhaustive checks, and
  checked-in deterministic Rust regressions own these conclusions.
- Source control retains the corrected small models, regressions, this result,
  and a clean-clone installer/runner under `validation/specula` and `tools`.
  Generated source copies, patches, transcripts, raw traces, model-checker
  databases, and large logs remain local audit evidence.

## 2026-08-08: Practical xmonad policy remains opaque and Engine-themed

- The retained personal xmonad configuration supplied the practical policy
  vocabulary and IR_Black colors, not an authority model. Sophia now registers
  distinct opaque actions for focus/swap master, swap up/down, shrink/expand,
  master-count, layout reset, floating toggle, and sink. The public WM frame
  did not change; Super chords terminate in Engine and translate to private
  Mod1 chords only inside the compatibility bridge.
- Xmonad keeps a zero-pixel border. A packaged core KDL file makes Engine the
  sole owner of the one-pixel `#ffb6b0`/`#7c7c7c` frame, while xmobar retains
  static IR_Black-derived system counters with no title, class, XID, PID, or
  namespace feed. Release manifest schema 3 binds that core file by SHA-256;
  the verifier continues to read historical schema-2 packages.
- A registered xmonad grab may legitimately leave geometry unchanged. The
  bridge accepts that quiet no-op after a dedicated 250-millisecond settling
  interval, restarts the interval after any response, still requires pointer
  gestures to produce activity, and poisons the private process after deadline
  or disconnect. Process regressions cover every private practical chord and a
  delayed layout response that must not cross into its successor. The revised
  `LegacyWmResponseBoundary` model checks the registered-grab prerequisite
  across 9,489 distinct states to depth 42.
- The protocol-neutral workspace reducer now rejects configure or render
  commands for a surface hidden from every candidate output. Clean completion
  publishes only the redacted zero invariant; it does not retain the rejected
  surface identity.
- One shared shell reducer owns long-soak counts. The installed progress view,
  normal-run recorder, raw session verifier, and archive verifier use it to
  require every practical action once, any physical workspace view and move,
  both pointer gesture modes, workload thresholds, and zero layout timeout,
  resize abort, hidden command, stale response, rejection, or pending work.
  The final redacted summary is checksummed and independently recomputed from
  the raw archive, so progress reporting cannot become a second evidence
  interpretation.

## 2026-08-08: Physical launch stopped before takeover; QEMU caught a prelude false negative

- The first installed practical-profile launch ended in a host reboot while
  the independent input guard was still starting. Its immutable run recorded
  `preflight` and `input_guard=ready`, but never recorded `input_guard=armed`,
  graphics takeover, a session loop, or recovery. This proves Sophia had not
  reached the compositor when the host disappeared; the kernel reboot cause
  remains unclassified because this user session cannot read the previous
  boot's root-only kernel log.
- Reproducing the same startup and practical profile in the isolated QEMU
  harness reached focus, layout, workspace, pointer, launch, restart/reseed,
  and clean logout. The run ended with zero protocol or renderer health debt,
  38 accepted page-flip callbacks, and `sophia_qemu_guest status=complete`.
- That run exposed a harness-only false negative: the generic xmonad prelude
  waited for one visible surface even though the M7 scenario intentionally
  admitted two. The prelude now derives the expected projection cardinality
  from the scenario (two for M7, one for M8) and reuses the same predicate for
  focus and layout. The retained M7 acceptance and verifier regression both
  pass after the correction.
- QEMU remains evidence for protocol and policy semantics only. It does not
  close the physical installed short gate, which still requires a successful
  input-guard arm, DRM/VT takeover, visible pixels, and normal teardown on the
  real host.

## 2026-08-08: Target-resolved input is a pre-schema contract

- The unpublished semantic-hit-testing proposal assigned widget, text, styling,
  and animation behavior to Engine and claimed guarantees without a measurable
  workload. It is replaced by a smaller target-resolved boundary: Engine
  resolves physical input against the interaction snapshot paired with the
  last presented frame.
- Stable generational handles survive visual-only commits when geometry and
  meaning remain exact. Capture is bounded to one target per seat and is
  cancelled by target replacement or removal, authority or seat loss, and
  modal-scope change. Discrete actions are coordinate-free by default;
  continuous controls use paced normalized values. Coordinates require an
  explicit committed grant and remain local to that region.
- The boundary refuses reactive property graphs, widget-specific nodes, Engine
  text widgets and styling tokens, inferred handle reuse, and global click
  shields. Target identity is reserved for later keyboard-navigation and
  accessibility projections without placing their metadata in the input path.
- `TargetResolvedInput` covers committed, submitted, and presented scenes,
  capture validity, modal scope, and disclosure. `TargetInputPacing` covers one
  replaceable continuous slot per seat/target and ordered capture, discrete,
  completion, and cancellation boundaries. They are hand-maintained project
  models informed by scenario-driven Specula work; runtime traces and generated
  scaffolding wait for a shell implementation.
- Shell DMA-BUF transfer remains an unproven candidate. Only explicitly
  optional effects may degrade, mandatory content may not disappear silently,
  and performance goals require workload-specific measurement rather than a
  universal refresh-rate claim. Wire schema, Engine implementation, Hagia, and
  toolkit work remain deferred.
- Target-resolved shell input is not a rename for the implemented application
  route. Application delivery retains `RoutedInputRequest`, including the
  global and local coordinates needed for X11, while the native X Server
  Frontend owns protocol-local grabs and delivery. The private synthetic X
  server remains a legacy-WM policy bridge, not an application frontend.
  Cross-authority precedence among an application grab, shell capture, and a
  security transition remains a pre-schema question.

## 2026-08-08: Input security audit closes the pre-schema arbitration gap

- The first target model overstated its proof. Its release action guarded out
  stale activation, its disclosure action guarded out missing grants, its
  two-scene bound hid A→B→A handle reuse, and the pacing model recorded string
  tags rather than actual stream values. Cancellation could also become
  disabled at a lifetime counter bound. Those models established their own
  construction, not the advertised adversarial properties.
- The audit also found contract gaps outside those abstractions: a recipient
  shell could declare its own coordinate grant; targets lacked presented
  ownership, occlusion, and overlap admission; disclosure was not correlated
  to a live seat/device/contact; output and authority epochs were absent; and
  no rule prevented a frontend grab from carrying application coordinates over
  privileged shell or lock pixels.
- Current application routing is not already harmonized with the shell rule.
  Production input layers derive from committed surfaces, ordinary events are
  re-hit-tested before namespace-local grab lookup, one stalled client queue
  can fail the frontend service, and XID reuse recreates `SurfaceId` with
  generation one. These are runtime debts, not changes made in this
  documentation/formal pass.
- The corrected contract makes application grabs Engine-visible
  profile-scoped route leases. A secure transition revokes them immediately;
  a normal move outside admitted application scope waits for frontend release
  acknowledgement before shell capture. Fresh application and shell selection
  must use the applicable last-presented snapshot before the routes coexist.
- Coordinate disclosure now requires a capability issued by independent
  session or portal policy and bound to authority/session, target generation,
  output/region, seat/device class, precision, rate, expiry, and revocation
  epoch. Normal visual removal becomes effective on presentation; policy or
  security revocation is immediate and discards queued old-epoch data without
  sending a final value to the revoked endpoint.
- Targets are admitted only inside their authority's presented visual
  allocation after occlusion and deterministic overlap ordering. Identity is
  monotonically generational across authority sessions, and capture includes
  the initiating device/contact. Precommitted visual alternatives share fixed
  bounds and meaning; no concrete variant wire node is ratified.
- `TargetResolvedInput` now retains immutable scene history, uses three scene
  generations, models multiple ordered targets and independently issued local
  grants, and records attempted release/disclosure facts for falsifiable
  invariants. `TargetInputPacing` represents optional values so zero and
  no-motion completion are valid, reserves final-boundary capacity, models
  paced flush and fail-closed recipient epochs, and distinguishes normal from
  security cancellation. `InputAuthorityArbitration` separately covers
  presented selection, profile-scoped leases, release acknowledgement,
  reserved shortcuts, secure preemption, and old-epoch queue quarantine.
- Temporary negative controls produced the intended counterexamples: routing
  against committed state violated `CapturedTargetsArePresented`; recyclable
  generations violated `GenerationsNeverRecycle`; missing grant checks
  violated `CoordinatesAreAuthorizedAndLocal`; wrong-device activation
  violated `ActivationsMatchCapturedPresentedTarget`; bypassed topmost or
  output-loss cancellation violated `CapturedTargetsArePresented`; a fabricated
  final value violated `FinalValuePrecedesNormalBoundary`; removing drain
  fairness violated the pacing temporal property; widening a grab violated
  `ApplicationLeasesAreProfileScoped`; and retaining queued input through a
  secure transition violated `SecurityStateHasNoCaptureOrQueuedInput`.
- With the pinned TLA+ Tools 1.7.4 jar, the clean target model completed
  exhaustively across 5,518,840 distinct states to depth 20, pacing completed
  across 19,200 states to depth 14 including its fairness property, and
  arbitration completed across 26,560 states to depth 21. The complete pinned
  Sophia TLA gate passed with all three models registered.

## 2026-08-08: complementary Alloy and Z3 gates bound early architecture choices

- The prior architectural-alignment draft described an aspirational five-tool
  stack as though it were implemented and used absolute language about what
  models and fuzzers guarantee. It is replaced by an evidence policy: TLA+
  owns temporal behavior, Alloy owns bounded relational topology, Z3 owns
  arithmetic obligations, and executable tests own implementation behavior.
  Correspondence is explicit and none of these models is a Rust refinement
  proof.
- `AuthorityTopology.als` checks exact role admission, namespace-local access,
  portal-mediated cross-namespace authority, WM application-metadata blindness,
  and independently issued coordinate capability. `PresentedTargetTopology.als`
  checks visible allocation and occlusion, explicit trust and equal-priority
  selection, modal membership, authority/session/slot/generation uniqueness,
  and independent local grants.
- `TargetGeometryAndDisclosure.smt2` checks containment, intersection clipping,
  quantization, capability-epoch rate limits, and bounded target/outcome
  partitions without claiming zero telemetry. `WmV1WireBounds.smt2` consumes
  record widths, count bounds, message prefixes, and field maxima generated
  directly from `protocol/sophia-wm-v1.kdl`.
- Each protected Alloy assertion produced no counterexample in its explicit
  scope and each weakened attack predicate produced a witness. Each protected
  SMT query returned `unsat` and each weakened prefix, multiplication, clipping,
  quantization, rate-reset, or partition query returned `sat`. The unattended
  offline runner pins the official Alloy 6.2.0 archive by SHA-256, selects
  SAT4J with fixed symmetry, requires stable Z3 4.16.0, rejects errors and
  `unknown`, and optionally compares a local Z3 5.x build without replacing the
  stable gate.
- This evidence does not close the admitted application-input runtime debts or
  ratify shell wire limits. Spin/Promela, dependency-policy enforcement, and
  fuzzing remain follow-on candidates until a specific question, retained
  artifact, expected result, and reproducible runner exist.

## 2026-08-08: WM and shell composition is hardened before schema work

- Separate role sockets and exact UID/PID admission authenticate clients but do
  not preserve WM blindness when one process can combine WM geometry/focus
  authority with shell or broker metadata. The target contract now names the
  protection domain as the security boundary and forbids blind WM policy from
  sharing one with metadata-bearing shell, broker/portal, or application
  frontend roles. Shared source or executables remain possible only through
  separately supervised processes with no ambient cross-domain IPC.
- Opaque action integers also needed provenance. The ratified identity binds
  issuer role/authority and revocation epoch, recipient role/epoch, operation
  class, optional target slot/generation, and recipient-epoch activation
  identity. `ActionCapabilityTopology.als` rejects cross-issuer type confusion,
  stale/revoked use, recipient and target substitution, and activation replay;
  each weakened attack retains a satisfiable witness, and a valid scoped action
  retains a non-vacuous witness.
- A native shell exclusive zone cannot commit independently of application
  projection. The target sequence binds a ready shell candidate/reservation to
  the exact Engine work-area generation, WM snapshot and connection epoch, and
  answering projection before one logical presentation. Normal shell or WM
  failure preserves the previous complete bundle; security surfaces keep their
  independent preemptive path. Tier-0 indicator geometry is instead stable
  session/Engine chrome configuration established before WM policy, so
  descriptor loss clears content without changing the work area.
- `ShellWorkAreaCoordination.tla` checks that sequence across 12,278 distinct
  states to depth 23. Temporary removal of candidate readiness, reservation
  equality, and WM-epoch equality violated the corresponding readiness,
  coherent-bundle, and exact-epoch invariants. All weakenings were restored.
  The complete pinned TLA+ Tools 1.7.4 gate passed, including the existing
  5,518,840-state target-resolved input model.
- The pinned Alloy 6.2.0/SAT4J and stable Z3 4.16.0 architecture gate passed
  every positive property, non-vacuous valid-action witness, retained attack
  witness, and arithmetic query. No local executable Z3 5.x build was present,
  so the optional differential was not run. No production Rust, wire schema,
  shell runtime, sandbox, or application-input routing changed in this pass.

## 2026-08-08: physical WM activations retain identity and FIFO order

- The production live-WM owner queue had contradicted `PolicyLifecycle.tla` by
  treating an opaque action token as an idempotency key. A second physical
  activation of the same registered action was discarded whenever an equal
  request was pending or in flight. Tokens identify policy operations; they do
  not identify physical activation instances.
- The existing sixteen-entry owner bound now retains every admitted action in
  FIFO order. An in-flight request counts against the same bound. Saturation
  remains fail-closed because the Engine shortcut reducer has already consumed
  the chord; the session reports a saturating rejection count and limits
  per-event rejection diagnostics to sixteen records.
- Retaining multiple actions required more than deleting the duplicate check.
  Each action is rebuilt at the transport head against the latest committed
  workspace and layout snapshot while preserving its minted transaction and
  queue position. Thus the second activation observes the state committed by
  the first instead of carrying the stale snapshot captured at ingress.
- Scene refresh and completed pointer-gesture requests retain their existing
  selective duplicate reduction. The completion ledger now reports
  `action_ordered` and keeps `action_coalesced=0` as an explicit compatibility
  assertion. Focused regressions cover equal FIFO values, capacity including
  in-flight work, dequeue-time state rebasing, and verifier rejection of any
  nonzero action-coalescing count.

## 2026-08-08: native application hit-testing advances at presentation

- Production pointer selection previously rebuilt its `LayerSnapshot` list as
  soon as Engine transactions committed, even when the corresponding native
  frame was pending or submitted. A newly exposed or moved surface could
  therefore become a route candidate before those pixels reached scanout.
- Native scanout already carries an immutable `OutputFrameDamageSnapshot`
  through pending, rendering, submitted, and presented states. The visual
  runtime now publishes application input layers from that snapshot only after
  an accepted page flip moves it to presented state. Initial modesets publish
  synchronously; suspend or revocation clears the visible input projection.
  Non-native/headless output ticks retain their immediate commit-and-present
  behavior.
- The owner loop services page-flip retirement before draining physical input,
  so the new snapshot becomes visible to routing at the same owner boundary.
  Focused regressions retain the retired geometry/generation/order and exclude
  a metadata-known but unpresented surface.
- This closes the committed-versus-presented selection hole only for Sophia's
  current primary-output pointer coordinate domain. Per-output pointer domains,
  Engine-visible application grab leases, client queue failure isolation, XID
  generation advancement, and stale focus-handoff revalidation remain release
  blockers before native shell coexistence.

## 2026-08-08: recreated XIDs cannot reuse Sophia input identity

- X wire decoding necessarily represents the client's current resource by its
  raw XID, but CreateWindow had also projected every Sophia `SurfaceId` with
  generation one. Destroying and recreating the same XID could therefore make
  an old deferred input route resolve to the replacement.
- Each admitted X11 client now owns a private generation ledger keyed by its
  resource index. A CreateWindow request receives the next candidate Sophia
  identity, and the ledger advances only after dispatch accepts that creation.
  Rejected creates can retry the same candidate; stale, skipped, or exhausted
  generations fail closed.
- Investigation exposed the other half of the ABA path: DestroyWindow removed
  the Engine surface but left its frontend route registered until disconnect.
  Every successful response carrying removed surfaces now deletes those exact
  surface/window and routed-input entries before later requests are observed.
- Unit coverage checks monotonic admission and overflow. A socket lifecycle
  regression observes create generation one, DestroyWindow retirement, and
  same-XID generation two. A frozen-input regression proves an old thawed route
  is discarded while fresh input to the replacement still routes normally.
  Engine-visible grab leases, slow-client queue isolation, output-local pointer
  domains, and focus-handoff revalidation remain separate work.

## 2026-08-08: private X input backpressure is client-local

- A routed worker's bounded input queue previously returned
  `ClientQueueFull` through `route_pending`, and both persistent frontend loops
  converted that client-local condition into termination of the shared X
  service. One non-reading client could therefore deny service to healthy
  peers without causing unbounded memory growth.
- Saturation now removes the stalled client's complete sender set. The failed
  tracked route receives `RouteRejected`; later routes cannot keep pressuring
  that endpoint, and sender disconnection leads its worker through ordinary
  cleanup. Unknown and already-disconnected input routes are likewise retired
  without widening the failure domain.
- Focused regressions fill both the Engine-resolved and already-client-addressed
  input paths, prove that the broker remains live, and deliver the next event
  to a separate healthy client. Shared registry corruption and shared
  acknowledgement pressure remain service-level failures rather than being
  mislabeled as endpoint backpressure.

## 2026-08-08: focus acknowledgement cannot revive stale pointer routes

- The bounded pointer-focus handoff previously released its complete buffered
  sequence when frontend-applied focus equaled the originally requested
  surface. It did not independently confirm that the surface and every queued
  target still belonged to the current interaction projection and frontend
  route table.
- Release now requires each exact generational `SurfaceId` to remain both
  renderable in the last-presented input snapshot and owned by a current X
  client route. Removal, generation replacement, or route loss cancels the
  handoff atomically before any delivery token is minted.
- The protocol-neutral handoff reducer accepts an authority-supplied membership
  predicate and has a regression proving generation-one buffered input cannot
  release after only generation two remains. The live owner reports stale
  cancellation separately from timeout and capacity cancellation. A security
  authority epoch remains part of the larger Engine-visible grab-lease slice.

## 2026-08-08: native pointer retention is output-local and epoch-fenced

- Native application input now retains one interaction projection and semantic
  epoch per independently retiring output. Pointer placement selects the exact
  output projection; a page flip on one head cannot publish, clear, or advance
  another head's routing state. Buffer-only presentations preserve the epoch,
  while target identity, geometry, stacking, visibility, transform, output, or
  lifecycle changes invalidate it.
- An ordinary or passive-grab press creates a per-seat provisional lease bound
  to an exact lease ID, frontend sequence, control epoch, `SurfaceId`, admission,
  namespace profile, authority session, output, presentation epoch, device, and
  button. The X frontend confirms only after installing its protocol grab.
  Motion and release retain the original surface inside the admitted profile;
  scope exit discards the boundary event and waits for exact release
  acknowledgement rather than reinterpreting it as shell or foreign input.
- VT and external seat-release transitions advance the Engine/frontend control
  epoch. Frontend reduction clears active pointer, keyboard, and server grabs,
  drains frozen routes as rejected, and rejects any bounded-ingress event
  stamped before the transition. This barrier does not wait for cleanup; exact
  lease-release messages remain lifecycle acknowledgements.
- Focused regressions cover independently advancing output epochs, visual-only
  preservation, exact confirmation/release, and queued old-epoch rejection.
  TLC checks the corresponding provisional/active/releasing lifecycle and exact
  frontend sequence. Client-initiated explicit `GrabPointer` and XI requests,
  lock-authority integration, and shell capture remain open; this slice must not
  be described as universal grab arbitration yet.

## 2026-08-08: Hagia enters the session through the public projection contract

- The live session now has an explicit `sophia_wm_v1` selector. It creates the
  owner-only endpoint before spawning Hagia, authorizes that exact child PID,
  and moves blocking negotiation and transfer work into a capacity-one worker.
  API v7 remains separately selectable and is never an automatic fallback.
- Complete Engine scene facts feed the canonical projection reducer. A valid
  Hagia response is staged while frontend sizes and renderable content settle;
  the owner commit promotes the exact staged successor before reporting the
  terminal outcome. New surfaces begin without inferred output membership.
- Session operations use freshly minted, session-local opaque tokens. Repeated
  physical actions retain distinct activation serials and ordered bounded
  admission, while executable identity and raw input remain session-owned.
- A retained headless Kitty gate terminates the first Hagia incarnation,
  verifies connection epoch two, and requires startup, last-layout, session,
  and layout health after replacement. This closes the first live recovery
  slice, not protocol stability, installed-default promotion, multi-output
  behavior, or removal of API v7.

## 2026-08-08: public-policy settlement is coherent under replacement

- Scenario-driven code analysis found two coupled recovery defects in the
  first live Hagia path. Process replacement could discard a staged reducer
  successor while frontend layout settlement still owned its identity, and
  the prepare hook promoted reducer authority before the matching layout
  commit. The latter exposed a transient last-good disagreement and could
  survive a transport failure.
- Restart now terminates the old transport, makes it unavailable, forces the
  exact pending layout through its ordinary timeout/abort reducer, and admits
  a new connection epoch only after settlement ownership clears. Prepare now
  performs non-mutating staged revalidation; the owner-loop commit advances
  reducer and layout authority together.
- Terminal configuration, projection, and session-operation outcomes retain
  one owner-side deferred command when the capacity-one worker channel is
  busy. An old-epoch command is discarded on confirmed transport loss and can
  never cross into the replacement peer.
- `PolicySettlementRecovery` exhaustively checks 224 distinct states to depth
  36. Its invariants cover coherent last-good serials, failure preservation,
  old-owner clearance, and at-most-once terminal delivery. A temporary
  prepare-time promotion produced the expected `LastGoodIsCoherent`
  counterexample before the corrected model passed.
- Sophia now lowers one completed Engine-owned pointer gesture to a final,
  bounded interaction cause. Hagia validates phase, target generation,
  capability, output, and region before storing private floating geometry.
  Hagia regressions also retain repeated action identity, opaque session
  operations, atomic cross-output movement, output return generation, and
  reconnect behavior.
- `tools/check_policy_client_matrix.sh` completes one offline matrix across the
  Rust reference, independent C client, Hagia, and X11 bridge. Packaging can
  include an explicitly supplied Hagia binary and a separate installed login
  profile. A phase-anchored fault after the second submitted Hagia projection
  passed bounded restart, startup, session-health, and layout-health checks.
  Exact live injection after owner-side staging, the physical installed
  workload, shared behavioral freeze corpus, default promotion, and API-v7
  removal remain deliberately open.

## 2026-08-09: Hagia dogfooding is evidence-driven, not time-gated

- The ordinary installed Hagia profile now uses the same guarded TTY,
  libinput, cleanup, emergency-return, and immutable-attempt lifecycle as the
  retained xmonad profile. Every attempt records exact Sophia and Hagia
  executable digests; clean logout passes, Ctrl-Alt-Backspace is classified as
  recovered, and unexpected exit or invalid final health fails.
- A checksummed reducer records cumulative terminal, Firefox, action, pointer,
  checkpoint, and output-topology coverage without requiring elapsed time or
  action quotas. Fixed-duration xmonad artifacts remain reproducible
  compatibility regressions but no longer block the Hagia critical path.
- Bounded preflight exposed three live integration defects. Revision-1 Hagia
  requires both retained operation slots to be admitted; its optional
  capability set omitted `PolicyDirty`; and storing the private checkpoint
  inside the replaceable endpoint directory prevented a peer restart. The
  installed profile now admits its current terminal and Firefox slots, Hagia
  negotiates policy refresh, and Sophia keeps the mode-0700 checkpoint parent
  outside each replaceable endpoint.
- Reconciliation also exposed an ordering race: a queued `PolicyDirty` could
  arrive after the owner began the next projection cycle and be rejected as a
  control message during transfer. The transport worker now forwards dirty
  requests only before projection bytes begin, retaining the strict transfer
  boundary after `ProjectionPending`.
- The populated-checkpoint restart smoke, all client and owner fault points,
  ledger tamper/recovery checks, staged install/rollback checks, Hagia's full
  verifier, Sophia's all-feature suite, and the pinned TLA suite pass. These
  finite regressions establish the installation baseline; ordinary live use
  supplies further scenario evidence and promotes a new bounded gate only when
  an observed defect needs one.

## 2026-08-09: the first installed Hagia run exposed missing Engine reconciliation

- Installed Hagia attempt `0001` reached both physical outputs and admitted
  Kitty, then failed readiness at `layout_pending`. The immutable archive shows
  a public-policy transaction expecting `2560x1440` while Engine retained the
  coherent `1323x1424` admission extent. The transaction timed out, startup
  failed closed, all processes drained, exact TTY state returned, and the
  ledger classified the run as failed rather than erasing it.
- The API-v7 compatibility path reconciled every blind-WM transaction through
  `LayoutEpochCoordinator` after priming an Engine-owned recovery extent. The
  public-policy path primed the same extent but staged Hagia's unreconciled
  proposal directly into both the reducer successor and frontend layout. It
  therefore asked the client for pixels that contradicted Engine's temporary
  constraint and could not commit the startup frame.
- Public proposals now pass through the same protocol-neutral constraint
  reconciliation before reducer staging. Reconciled outer geometry and any
  explicit content-size request remain one candidate, while an omitted public
  content request stays omitted. Focused regressions reproduce the physical
  `2560x1440` to `1323x1424` correction and guard optional-size semantics.
- Kitty launch now disables its remembered OS-window size for deterministic
  managed-session admission. The bounded live Hagia regression observes a
  real constraint adjustment, reaches readiness, survives populated-checkpoint
  replacement, and ends with no layout, protocol, application, or cleanup
  debt. A fresh installed physical run remains the acceptance proof.

## 2026-08-09: the second installed Hagia run exposed repeated public admission ownership

- Installed attempt `0002` proved that the constraint correction itself could
  commit, but startup still failed closed at `not_committed`. In eight seconds
  the archive recorded 3,121 constraint reconciliations, 3,124 layout commits,
  and 3,123 checkpoint saves for the same surface. Super+Enter's ordered action
  committed inside that loop, but the newly queued terminal could not be
  admitted before the startup deadline. Exact TTY recovery again succeeded.
- The owner intentionally keeps a policy-managed surface in visual quarantine
  until its first presented buffer retires. API v7 consumed WM planning
  ownership through the committed candidate workspace assignment, while a
  public projection has no Engine-owned workspace effect. A committed public
  `Manage` therefore left the surface eligible for another `Manage`; those
  immediate no-op commits starved native retirement and repeated indefinitely.
- The exact committed public `Manage` source now consumes planning ownership.
  Its pixels remain independently fenced in `AwaitingRetirement`, so no
  presentation safety is weakened and relayouts cannot consume unrelated
  pending admissions. A focused regression proves the surface is no longer a
  Manage candidate while its visual admission is still pending. The live
  Hagia smoke additionally caps reconciliation, layout-commit, and checkpoint
  counts so a high-speed feedback loop cannot pass merely because the process
  exits cleanly. A replacement physical run remains the acceptance proof.

## 2026-08-09: Hagia restart proof exposed asynchronous keyboard loss

- The first opt-in Hagia policy-restart run reached the actual hardware path:
  two-output startup completed, Hagia saved a nonempty checkpoint, the
  supervisor replaced it at epoch 2, the replacement loaded and reconciled the
  checkpoint, fullscreen geometry survived, and later native page flips
  retired. The run did not pass. The exact text matcher first observed the
  eighth letter because the preceding seven physical keys arrived during the
  short Engine/frontend focus-acknowledgement gap after policy commits.
- `control_plane_only` had intentionally kept emergency, VT, and WM shortcuts
  responsive while dropping client input. Dropping was safe against
  misdelivery but created visible text loss under ordinary rapid interaction.
  Treating the mismatch as a proof typo would have hidden a production race.
- Engine now resolves the control plane once and holds only the unmatched
  client-bound keyboard sequence for the exact seat and generational focus
  target. Matching frontend acknowledgement releases that sequence in order.
  Target replacement, focus/seat or security change, topology transition,
  timeout, and capacity exhaustion discard it atomically. Original libinput
  timing sidecars survive the hold, so latency evidence starts at physical
  ingress rather than acknowledgement. Focused state-machine and live-router
  tests cover ordered release, retarget rejection, timeout, capacity, exact
  text matching, and no premature client delivery. A new physical run remains
  the acceptance proof.

- The replacement run completed all 34 exact text press/release events, their
  terminal pixel change, libinput-to-page-flip timing, bounded shutdown, and
  clean TTY restoration. It still was not a policy-gate pass: the final phrase
  was entered before the post-restart action sequence, so the evidence had no
  ordered physical action commits. It also revealed that startup itself
  reached checkpoint occurrence 4 and triggered the injected fault before the
  documented `Super+Y`, `Super+Right` boundary.
- The gate now faults on deterministic occurrence 6: four startup checkpoints
  followed by the fullscreen and active-output checkpoints. The final phrase
  is explicitly labeled as an immediate exit signal and must follow every
  post-restart action. Caller-specific verification reports the first missing
  pre- or post-restart phase and separately requires exact text, one restart,
  clean session/topology health, drained native ownership, and clean process
  teardown. Generic CPU-layer proof assumptions no longer mask the more
  specific Hagia result.

## 2026-08-09: the layout-cycle proof exposed a stale close acknowledgement

- The first layout-cycle replacement run used X keycode labels for an evdev
  binding table. `Super+F` therefore committed the browser operation instead of
  fullscreen. The guide now uses the actual retained bindings: `Super+Y` for
  fullscreen, `Super+N` for layout cycling, `Super+I` for minimize, and
  `Super+R` for restore.
- The accidentally launched Chromium client could not initialize its GPU path
  and left a presented surface after its X Authority route disappeared.
  Pointer buttons remained fail-closed during the unresolved focus handoff.
  Closing that stale surface returned `UnknownSurface`; Sophia treated the
  expected teardown race as fatal even though the client was already gone.
- A rejected `CloseSurface` acknowledgement with `UnknownSurface` is now
  retired as a stale target, like `ClientGone`. Other command kinds still
  reject `UnknownSurface` as an error, so configure and focus failures cannot
  be hidden. Focused queue tests cover both sides of that distinction, and the
  complete `sophia-cli` suite passes.
- The corrected installed run on Sophia `09337bb2` passed the bounded physical
  policy gate and was archived as promotion record `0003`. The trace commits
  fullscreen and layout-cycle actions before the supervised restart, loads and
  reconciles the nonempty checkpoint in epoch 2, then commits fullscreen,
  layout-cycle, focus, minimize/restore, and output actions afterward. Exact
  text and pixel evidence passed, all native ownership drained, and all 20
  session controls completed without rejection, timeout, or cleanup debt.

## 2026-08-14: asymmetric mirror rendering exposed successor-generation relabeling

- Diagnostic mirror attempt `0001` on source `bbd50ef0` reached direct-CPU
  bootstrap and worker readiness on both DP-1 and DP-2 without an AMDGPU
  command-stream rejection. DP-2 rendered, submitted, and flipped logical frame
  2 while DP-1 was still rendering that frame. DP-2 then completed retained
  frame 3, but the scheduler preferred the still-active group identity 2 and
  attempted to submit the connector twice for generation 2. The lifecycle
  correctly rejected that transition as `Duplicate`; the gate failed at the
  runtime stage and archived the trace rather than promoting it.
- A mirror generation is now reserved when it is queued, before either renderer
  or KMS submission. A connector that has submitted the active generation stays
  fenced even after its early physical callback, and exporter results retain
  the identity of `rendering_content` instead of borrowing a newer pending
  frame. Cleanup-blocked heads preserve the active frame in their exporter and
  defer one latest successor per head; the successor is promoted only after the
  active frame enters rendering or submits. A progress-based 500 ms watchdog
  covers stalls before first KMS submission and reports each connector's KMS,
  cleanup, worker, pending, rendering, and deferred state.
- Regressions cover the observed fast-head sequence and the adjacent case where
  repeated successors arrive while a slow head is cleanup-blocked before worker
  capture. The full all-feature workspace suite passes. Physical acceptance
  remains open until a clean signed source reruns the tty4 gate successfully.

## 2026-08-14: the joined mirror ran continuously but shutdown read primary-only identity

- Diagnostic attempt `0002` on signed source `e3435eb8` completed joined mirror
  generations 2 through 11 on both DP-1 and DP-2. Direct bootstrap, persistent
  workers, asymmetric callback order, repeated CPU updates, and logical damage
  publication all ran without duplicate transitions, watchdog stalls, AMDGPU
  rejection, or process abort. The run failed during bounded shutdown rather
  than visual service.
- On frame 12 the primary head submitted and flipped before the sibling submitted.
  Its physical callback correctly moved the primary's content from submitted to
  presented ownership. The sibling submission then completed the logical submit
  join, but the visual runtime asked the now-empty primary slot for the logical
  frame identity and rejected the drain with `native submit did not retain its
  frame identity`. No frame identity or buffer owner was actually lost.
- The mirror lifecycle now exposes the logically submitted generation while all
  heads have submitted and at least one callback remains. Runtime submission and
  drain observation use that group identity; per-head content remains available
  only as physical ownership evidence. A regression covers primary-flips-before-
  sibling-submit at shutdown.
- The same run revealed that startup readiness printed two physical heads as
  `outputs=2/2`, contradicting the one logical output required by the verifier.
  Startup now AND-joins readiness by `OutputId`, reports `1/1`, and emits exactly
  one logical synchronous-modeset record only after every mirror head has proof.
  Per-head direct-bootstrap and worker-readiness evidence remains unchanged.

## 2026-08-14: typing `ll` exposed missing core X11 PolyRectangle support

- Diagnostic mirror attempt `0003` on signed source `5d40cbae` again bootstrapped
  both heads and joined repeated logical generations with correct `outputs=1/1`
  readiness. Typing `ll` in the proof xterm deterministically made the client
  issue core X11 opcode 67 (`PolyRectangle`). X Authority did not decode that
  request, returned `BadRequest`, and xterm treated the protocol error as fatal
  and exited with status 83. The mirror scheduler had submitted frame 10 on both
  heads; one callback had retired before the client failure.
- X Authority now decodes both byte orders of the 12+8n request, validates the
  drawable, graphics context, namespace confinement, and drawable/GC depth, and
  reports `BadDrawable`, wire `BadGC`, `BadMatch`, or `BadAccess` as appropriate.
  Window rendering uses clipped, non-overlapping outline bands with i32 extents,
  including degenerate rectangles and wide/XOR lines without double-toggled
  corners. Graphics contexts retain their creation depth independently of the
  lifetime of the drawable used to create them.
- A fatal proof client no longer returns directly past native cleanup. Frontend
  intake stops, the owner loop enters bounded completion, partially joined
  mirror callbacks drain, renderer-image and Present ownership shut down, and
  explicit cleanup evidence is emitted before the original client error is
  returned. Cleanup failures are aggregated without masking that original error.

## 2026-08-14: ordinary terminal output requires lossless authority backpressure

- Diagnostic mirror attempt `0004` on signed source `d8b5e861` proved the
  PolyRectangle and fatal-cleanup changes: both heads bootstrapped, joined and
  retired repeated generations, and native suspend drained with zero abandoned
  scanouts. The remaining failure was xterm status 84, reported as a fatal X
  connection I/O error after `ll` produced a burst of terminal drawing updates.
- The production frontend still used the fail-fast observation emitter. Filling
  its 256-batch queue converted a temporary Engine scheduling delay into a
  client-local failure and closed the X connection. This is the same bounded
  transport overload previously exposed by the terminal benchmark, but normal
  interactive shell output cannot be made safe by pacing a proof workload or
  enlarging an eventually finite queue.
- Production now preserves bounded memory and each client's ordered visual facts
  by retaining one current observation per blocked worker and pausing that
  connection's X11 request dispatch until Engine drains capacity. Concurrent
  clients may interleave, as they could before this change. Shutdown explicitly
  cancels that wait. The nonblocking emitter remains available for probes that
  intentionally require a fail-fast `Backpressure` result.

## 2026-08-14: mirror completion identity belongs to the logical output

- Physical mirror attempt `0005` on signed source `265d94dc` exercised the
  lossless authority policy under an `ll` burst: 891 batches entered Engine with
  zero drops, bounded waits resumed in one millisecond, both connectors joined
  through frame 8, and native teardown drained with zero abandoned scanouts.
- The remaining exit 1 came after clean frontend join from a completion check
  that required every physical head checksum to differ. That invariant belongs
  to independent logical outputs. Mirror heads intentionally carry the same
  logical scene checksum even when their native-size scanout buffers differ.
- Completion now requires equality among heads sharing an `OutputId` and
  uniqueness between distinct `OutputId` values. The physical verifier locks
  the same contract while connector-qualified submit, callback, and retirement
  records remain the evidence that each physical chain presented independently.

## 2026-08-15: mirror shutdown drains owners without re-entering scene work

- Diagnostic mirror attempt `0006` on signed source `cabd6b18` rendered and
  retired both physical heads through logical frame 17, then submitted frame 18
  on both. Completion failed because its mirror drain called the full backend
  tick with an empty input while committed xterm surfaces still required layer
  templates. The resulting `InvalidSurface` was a shutdown-only scheduling
  error, not lost KMS ownership.
- Mirror retirement now collects and applies every physical callback, joins the
  logical generation, and retries every head cleanup before reporting an
  aggregated error. The drain path invokes only that ownership work; it cannot
  run Engine scene projection, renderer export, KMS submission, successor
  promotion, or the scheduler watchdog. Normal scheduling also applies accepted
  callbacks before its fallible Engine tick, closing the callback-loss window.
- A drain failure still attempts forced detach. Its typed error preserves the
  original drain error, the detach error or report, and the known abandoned-owner
  count. Completion clears renderer images only after detach is established.
- The same run exposed eighteen DP-2 `OutputMismatch` damage rejections. One
  logical 2560x1440 snapshot had been cloned into a 1920x1080 physical-head
  presentation state even though the pixels were projected. Every mirror queue
  path now prepares a destination-native snapshot before reserving the group
  generation, projecting surface, border, and cursor rectangles with the same
  fit transform as the pixels. Connector-qualified damage evidence must join a
  causal frame on both heads, and any `OutputMismatch` fails physical promotion.

## 2026-08-15: core fixed text and CopyArea now carry xterm pixels losslessly

- The prior text path synthesized a partial uppercase 5x7 alphabet inside an
  8x12 cell and discarded PolyText item structure. That could not prove mixed-
  case terminal output, font shifts, or scrolling correctness. Sophia now
  vendors all 256 glyphs from X.Org's `6x13-ISO8859-1.pcf` data. The upstream
  `font-misc-misc` license identifies this font as public domain: “Public domain
  font. Share and enjoy.” Metrics are consistently 6-pixel advance, 11-pixel
  ascent, and 2-pixel descent in rasterization and QueryFont/ListFontsWithInfo.
- OpenFont accepts `fixed`, `6x13`, the canonical XLFD, and the core `cursor`
  compatibility name; unknown names fail with BadName. The real xterm proof also
  exposed its standard startup open of `nil2` for tiny-font/icon slots, so that
  compatibility name retains the fixed face without importing a host-font
  dependency. GCs resolve and retain a face at creation/change time, so closing
  the font XID cannot invalidate later draws. QueryFont accepts FONTABLE GCs.
  PolyText8 retains signed deltas and request-scoped, MSB-first font shifts;
  malformed items fail length validation and missing shift fonts report BadFont.
- ImageText8 now forces GXcopy/Solid image-string semantics while preserving GC
  plane mask and clipping, fills the full cell background, and draws exact 6x13
  foreground glyphs into either window or pixmap backing. CopyArea validates
  both drawables and the GC, checks all depths, snapshots overlapping sources,
  clips source/destination together, and publishes only the actual destination
  damage. Pixmap PutImage and text no longer succeed without backing pixels.
- The real-xterm render proof pins `-fn 6x13`, white foreground, black
  background, disabled cursor blink, and an eight-row terminal workload that
  prints twelve mixed-case markers. Passing requires four exact white-on-black
  adjacent final rows in a currently live per-surface buffer plus accepted
  same-surface CPU updates in ImageText8→CopyArea→ImageText8 order. Historical or
  superseded buffers, auxiliary surfaces, no-op copies, scattered rows, and
  unordered opcode presence cannot satisfy the proof. Core
  GraphicsExpose/NoExpose event generation is deliberately
  deferred; xterm's pixel scroll path and bounded authority ownership do not
  depend on that optional event breadth.
- Final core-resource review found that fonts, pixmaps, and GCs could overwrite a
  different live resource kind with the same XID even though CreateWindow
  already rejected such collisions. Core creation paths now consult one XID
  namespace, and the shared resource table rejects replacement as a second
  defense. Request-specific failures report BadPixmap, BadGC,
  BadDrawable, or BadValue instead of collapsing into BadWindow. Regressions
  prove collisions preserve the original window, GC, or font.

## 2026-08-15: mirror attempt 0007 stopped before KMS on an undeclared proof app

- The first physical run of signed commit `23880077` produced no native or
  renderer evidence. Argument extensions named application `terminal`, but the
  gate had not declared that application in the normal-session registry, so
  configuration failed immediately with `UnknownApplication("terminal")`.
- The gate now resolves an absolute executable xterm path, explicitly selects
  normal-session mode, declares and starts `terminal`, then applies its bounded
  6x13/white-on-black/scroll arguments. Parser and shell regressions prevent
  terminal arguments from becoming detached from their application declaration
  again.
- Follow-up attempt `0008` also stopped before KMS because the first correction
  combined `--no-config` with the explicit `--desktop-profile`, two intentionally
  exclusive configuration sources. The gate now uses the explicit proof profile
  alone, and its exact parser regression includes that profile argument.

## 2026-08-15: mirror attempt 0009 exposed discarded CPU successors

- Diagnostic attempt `0009` reached both physical heads at their native modes
  and displayed the centered blue focus frame, but the xterm content remained
  black. The X11 path had produced forty CPU updates and a new scene checksum;
  native scanout presented only the older black CPU generation followed by a
  retained-mixed chrome generation. While any mirror generation was active,
  `queue_frame` reported GPU ownership and discarded each newer composed CPU
  frame, and the caller had no later owner from which to recover those pixels.
- A mirror output now owns one immutable active generation and one output-scoped,
  latest-wins successor containing every head's projected frame and damage
  snapshot. Queueing prepares all physical heads before changing state, replaces
  the successor atomically, and promotes it only after the active generation's
  joined retirement. Repeated driving of one mixed Present reuses its reserved
  frame identity. A cleanup-blocked or fast-flipping head therefore cannot drop,
  relabel, or independently advance the successor.
- CPU updates that arrive with a focus/order/chrome change bypass the stale
  retained-CPU shortcut and compose the current pixels. Joined presentation now
  records the logical content source and checksum after confirming every head
  agrees. Physical promotion requires a causally retired CPU generation whose
  checksum equals the final composed CPU checksum and both completion-head
  checksums; a blue retained-only frame can no longer certify terminal pixels.
- Reducer and visual-ordering regressions, the complete backend all-feature
  suite, the CLI binary suite, and the physical-verifier mutation suite pass.
  A clean signed successor must rerun the physical gate; this implementation is
  not itself physical acceptance evidence.

## 2026-08-15: normal mirror retirement must not synthesize a scene tick

- Attempt `0010` on `36842020` completed native presentation and shutdown but
  failed operator confirmation. The next changes made mirror retirement visible
  to the frame service and corrected scaled GL sampling. Attempts `0011` through
  `0013` then failed consistently at first-surface arrival with
  `engine backend tick failed: invalid surface ID`; the final diagnostic named
  committed surface `2097164:1`, one committed surface, and zero templates.
- The paired-scene fixes in `1e459b30` and `29572b21` remain valid, but the
  failing input did not originate in the visual runtime. Normal
  `PollRetirement` called the mirror's full scheduling tick with
  `CompositorBackendTickInput::default()`. That lower layer manufactured an
  empty template list while the output assembly retained the xterm surface.
- Normal mirror retirement now performs callbacks, group joining, logical
  page-flip publication, cleanup retry, and successor promotion only. Engine
  projection and renderer/KMS submission remain exclusive to an explicitly
  supplied scene tick. All scene-producing paths derive templates from the exact
  committed slice installed into the output runtime.
- Controlled owner-loop failures now stop frontend intake and enter the same
  bounded native/presentation completion used for client-fatal failures before
  returning the original error. Diagnostic verification requires that cleanup
  evidence after native bootstrap; signal-terminated processes remain exempt.
  The physical tty4 gate remains required from a clean signed successor.

## 2026-08-15: mirror reduction needs a text-preserving sampling policy

- The latest physical run on signed commit `515d0f52` brought both heads up at
  their native modes and showed the same centered scene, but the 1920x1080 head's
  6x13 terminal text was visibly blocky. The renderer rasterizes the logical
  2560x1440 scene once and reduces it by 0.75 for that head. Bilinear filtering
  avoids nearest-neighbor row loss, but it still softens the contrast of
  one-pixel bitmap-font stems enough to make the scaled terminal materially worse
  than the native-size head.
- Composition sampling is now explicit and observable: exact-size draws retain
  nearest sampling, reductions use a fixed 4x4 Catmull-Rom reconstruction, and
  enlargements use linear filtering. If the sharp shader cannot compile, the
  renderer falls back to linear but emits fallback evidence; the physical gate
  rejects that fallback and requires exact sampling on the 2560x1440 head plus
  sharp reduction on the 1920x1080 head. A CPU reference regression exercises
  mixed-case X.Org 6x13 glyphs at the physical 0.75 ratio.
- The same attempt's status 1 was separate from image quality: one client key was
  still pressed when the runtime deadline ended. Deadline completion now stops
  accepting physical input, synthesizes releases through the existing ordered
  input path, and waits up to 500 ms for delivery and release-barrier
  acknowledgements before native teardown. The gate requires a clean key record
  as well as operator confirmation that both heads show identical, stable text
  and that the reduced head is not blocky or stair-stepped.
- These changes have deterministic renderer, reducer, and verifier coverage, but
  they are not physical acceptance evidence. A clean signed successor must rerun
  the tty4 gate.

## 2026-08-15: XRGB alpha is not transparency on the sharp mirror path

- The physical run of `c2460938` proved that both heads submitted and retired the
  same CPU generations, DP-2 selected the sharp 0.75 reduction without fallback,
  and native teardown was clean. The operator nevertheless saw only the blue
  frame and hardware cursor on DP-2; its terminal interior was black.
- The sharp shader unconditionally constrained reconstructed RGB by sampled
  alpha. X11's XRGB buffers encode colors such as white as `0x00ffffff`: the
  fourth byte is padding, not transparency. Exact sampling happened to remain
  visible because its opaque draw disabled blending, while the sharp program
  converted every XRGB color to black. Frame checksums and projected damage
  describe logical identity and ownership, so they could not expose this raster
  error.
- Textured composition now carries an explicit opaque-versus-premultiplied alpha
  mode. Both normal and sharp programs force alpha to one for XRGB before layer
  opacity and retain `rgb <= alpha` reconstruction clamping only for ARGB. This
  also corrects fractional-opacity XRGB blending instead of limiting the repair
  to the observed full-opacity path.
- The physical gate now enables final-region readback and requires substantial
  gray/white terminal coverage on both native-size compositions, with a bounded
  DP-2-to-DP-1 coverage ratio. Sampling evidence records the alpha mode, and
  mutation tests reject a blue-frame-only secondary head. A new clean signed
  physical run remains required.

## 2026-08-15: per-head planning and output control need two distinct barriers

- The multi-monitor target is now represented by production types rather than
  only prose. `SurfaceContentSet` carries bounded ready raster variants with
  density, transform, fidelity, and damage. Engine captures one immutable
  `OutputSceneSnapshot` and derives one native-size `HeadCompositionPlan` per
  opaque target. Logical viewports are stored independently from native head
  shapes, so unequal-mode mirror members no longer erase the logical output.
- The production CPU transaction constructs these plans from its exact committed
  slice before queueing the existing flattened compatibility frame. This is a
  deliberate shadow cutover: it proves the running caller and rejects invalid
  plans, while the native renderer still has to consume each plan directly.
  Claiming native per-head rasterization before that lowering replaces the flat
  queue would repeat the evidence mistake this architecture exists to prevent.
- Two Engine reducers keep resource and policy settlement separate.
  `OutputPresentationCohort` requires every head candidate to prepare and agree
  on logical identity before the first KMS submit, then joins flips and cleanup.
  `OutputTopologyTransaction` keeps the old topology published through prepare
  and physical apply, forces partial apply into rollback, and commits only after
  every new logical output presents once.
- The first `sophia_output_v1` Rust contract is implemented as its own exclusive
  authenticated role. A complete candidate independently selects each head's
  mode, transform, and VRR policy and groups arbitrary heads into mirrored
  logical outputs while leaving other groups extended. Backend projection
  exposes opaque head and bounded mode identities without leaking card, CRTC, or
  plane objects. Live supervision, renderer-target preparation, KMS
  apply/rebuild, and generated C conformance remain open and are not implied by
  the new transport tests.

## 2026-08-15: CPU composition crosses the per-head lowering seam

- The production CPU adapter now carries every resident CPU content variant
  from its one committed scene read into submission. Engine selects a variant
  separately for each `HeadRenderTarget`; `sophia-renderer-live` lowers that
  exact `HeadCompositionPlan` into native placements, clips, opacity, bars,
  borders, cursor state, and a head-local damage snapshot.
- `LiveProductionNativeScanout::queue_head_composition_frames` admits the set
  transactionally by opaque head. It rejects partial or duplicate coverage,
  logical checksum disagreement, missing damage, and native target mismatch,
  then queues one `HeadComposition` frame per physical owner without applying
  the old whole-output projection. The flat CPU result is retained only for the
  synchronous first modeset; the immediately following cohort uses the new
  path.
- Variant selection and transformation now fail closed: a plan does not fall
  back from an unavailable compatible raster to an arbitrary transformed
  canonical buffer, and non-normal output transforms remain rejected until
  their geometry/raster lowering is implemented. Off-viewport surfaces are
  filtered from extended-output snapshots rather than invalidating the frame.
- This is not the final prepare-all cutover. Renderer export and KMS commit are
  still one operation in the live mirror scheduler, so the Engine cohort
  reducer is not yet its production owner. DMA-BUF and retained renderer-image
  sources also remain on their existing projected mixed path until a per-head
  affine lease resolver can prepare every required target before submission.

## 2026-08-15: output proposals reserve identity before they acquire hardware

- The output wire, native resolver, and topology reducer previously existed as
  three unjoined seams. Calling them independently would either publish a
  candidate before first presentation or consume a fresh `OutputId` when a
  later preparation step rejected it.
- `LiveOutputAuthorityOwner` is now the session orchestration record joining
  those seams without acquiring DRM authority. Validation resolves against a
  cloned allocator and returns an immediate validated settlement. Apply keeps
  the resolved targets, next snapshot, and allocator private while Engine
  tracks every prepared and applied opaque head.
- A candidate publishes only after every replacement logical output reports its
  first presentation. Preparation failure leaves no physical apply to undo;
  partial apply can settle only after rollback; rollback failure is terminal.
  Fresh identities become durable only with the committed snapshot, so rejected
  candidates leave both the visible topology and allocator unchanged.
- This owner deliberately performs no renderer allocation, KMS call, process
  supervision, or frontend update. Those effects remain in the live session and
  are the next cutover; keeping them out of this reducer preserves Engine's
  explicit effect boundary.
- The transport prerequisite for that supervision is now cancellable. Proposal
  bytes accumulate inside `OutputSessionTransport` until one complete bounded
  frame exists, and the optional service polls both accept and connected intake
  without blocking the visual owner. An absent client shuts down normally; a
  split header/payload remains ordered and lossless. The live supervisor still
  needs an explicit assignee and restart handoff before advertising this socket
  to a WM or shell.

## 2026-08-15: the supervised public WM owns the live output role

- Native public-policy startup now binds the exclusive `sophia_output_v1`
  endpoint inside the private policy directory, authorizes the exact supervised
  WM PID, and advertises only that endpoint through `SOPHIA_OUTPUT_SOCKET`.
  Static and legacy policy paths receive no physical-output capability.
- The optional transport service and `LiveOutputAuthorityOwner` are retained by
  the live WM session. Complete proposals are polled in a bounded owner turn;
  validate-only proposals settle through the resolver and topology reducer,
  while apply proposals fail explicitly at preparation rather than mutating a
  subset of native state.
- A supervised WM restart disconnects the old output peer, abandons its active
  and queued proposals, advances the connection epoch, and authorizes the new
  PID before it can reconnect. The published topology survives the role handoff.
  This closes the supervision boundary; renderer-target preparation, live KMS
  apply/rollback, runtime rebuild, and first-presentation publication remain the
  ordered cutover.

## 2026-08-15: live mirror submission now has a real prepare-all barrier

- Native primary-plane setup is split into two affine operations. Preparation
  exports the renderer result, creates/imports the framebuffer and optional mode
  blob, discovers properties, and builds the complete atomic request without
  calling the commit ioctl. Its owner can only be submitted or explicitly
  cancelled against the same DRM device.
- The production multi-head scheduler records one `HeadFrameCandidate` per
  completed native owner in Engine's `OutputPresentationCohort`. A head may not
  submit until the cohort has every required head, so neither a fast renderer nor
  loop ordering can cross the first-submit barrier. Submission, callback, and
  cleanup transitions are reflected back into that same cohort while the older
  physical lifecycle continues to provide card polling and timing evidence.
- A preparation failure cancels every sibling owner before poisoning the logical
  generation. Normal shutdown cancels prepared-but-unsubmitted owners before its
  callback-only drain. Deterministic fake-DRM coverage proves preparation issues
  zero commits, cancellation frees its framebuffer, and two prepared mirror
  heads remain at zero commits until the complete cohort is ready.
- This does not yet implement live topology replacement. Candidate mode objects,
  replacement target pools, KMS modeset apply/rollback, runtime rebuild, and the
  first-presentation publication barrier remain the next effect transaction.

## 2026-08-15: cross-card topology apply needs explicit reverse rollback

- A DRM atomic request is transactional only inside one card. Live output IPC
  may change heads on several cards, so a sequence of successful ioctls is not
  one implicit transaction: a later refusal otherwise leaves a mixed desktop.
- The backend now applies cards in stable index order, preserves `Busy` as a
  retry of the same card, and rolls an accepted prefix back in reverse order. A
  first-card refusal is distinguished as failure without physical mutation.
  Each card submit is one blocking `ALLOW_MODESET` request without a page-flip
  event, so its result is synchronous at the coordinator boundary.
- Rollback composition is derived separately from the still-published authority
  snapshot and the live head generations/native sizes. Provisional candidate
  geometry never becomes rollback input. The next slice attaches affine
  candidate and rollback renderer owners to this coordinator.

## 2026-08-15: rollback resources belong to the prepare-all barrier

- Preparing only candidate framebuffers is insufficient. Once card zero accepts
  a candidate, card one's refusal creates an immediate need for the old topology;
  attempting to allocate rollback resources at that point introduces a failure
  between mutation and recovery.
- A typed live resource cohort now requires exactly one candidate member and one
  rollback owner for every affected opaque head before it reports ready.
  Disabled candidates contribute a discovered connector/CRTC/plane property set
  rather than a fake framebuffer; previously disabled heads do the same on the
  rollback side. Duplicate, unknown, or wrong-disposition
  insertions return the supplied owner instead of dropping it.
- Card/head observations into `LiveOutputAuthorityOwner` now stage through a
  cloned reducer and publish the batch only if every member is accepted. Native
  execution therefore cannot partially advance protocol authority because of a
  malformed later member. The remaining work is the nonblocking renderer-worker
  driver that fills this cohort and transfers accepted owners into rebuilt live
  targets.

## 2026-08-15: live topology preparation uses the real renderer workers

- The session owner now captures one committed scene twice: once through the
  provisional candidate viewports/targets and once through the still-published
  rollback topology. It queues the candidate head frames, polls their ordinary
  persistent renderer workers, then queues and polls every rollback head. Normal
  frame scheduling is quarantined while either pool is active.
- A frame set is admitted only with exact enabled-head candidate coverage, exact
  previously-enabled-head rollback coverage, and damage extents matching each
  native selection.
  A candidate-disabled head has no candidate raster but still has a rollback
  raster and prepared detach properties.
- Failures enter an abort phase: unsubmitted frames are discarded, in-flight
  worker results are polled and their leases released, partial native owners are
  cancelled, and retryable cleanup is retained separately because candidate and
  rollback cancellation may produce two owners for one head. Session completion
  performs the same bounded abort before native suspension.
- The live path deliberately still cancels a fully prepared dual pool and emits
  `kms_submits=0`; applying it before runtime/head-table installation exists would
  expose a physically changed topology with no authoritative owner. The next
  slice is therefore accepted-owner installation plus card apply/rollback, not
  more renderer scaffolding.

## 2026-08-15: rollback pools cover previously disabled heads

- Published rollback projection now represents connected-but-disabled heads as
  disabled members. Their rollback side owns prepared detach properties instead
  of requiring an impossible framebuffer. Candidate enable, candidate disable,
  rollback enable, and rollback disable are all checked against the head's prior
  state before the cohort can become ready.
- The live head record now distinguishes connected-but-disabled heads from active
  logical-output members, while `outputs()` remains a separate logical-output
  table. Startup behavior is unchanged because every discovered startup head
  begins enabled. The production WM path still cancels before KMS: the next slice
  must retain rollback owners across physical apply, output-runtime
  reconstruction, and the first-presentation barrier rather than adding a
  premature finalize operation.

## 2026-08-15: live output IPC crosses the physical publication barrier

- The session now retains candidate and rollback affine owners through an
  ordered per-card modeset transaction. Accepted candidate owners are installed
  directly into rebuilt output runtimes, avoiding a second modeset, and one
  native-size cohort is queued for every replacement logical output.
- Engine, X topology, WM work areas, pointer bounds, and input authority remain
  coordinated around an all-output first-presentation barrier. Only then does
  `sophia_output_v1` publish the new topology and release the rollback pool.
- A renderer/native service failure before that barrier is no longer a session
  escape: physical rollback is requested first, the Engine transaction enters
  `RollingBack`, and completion retains the same recovery record. Supervised WM
  restart or output-peer loss uses the same handshake and defers connection-epoch
  replacement until reverse apply settles.
- Hardware hotplug projects fresh capabilities and an authority snapshot from
  the replacement native owner, but publishes them to the optional output
  service only after replacement scanout presents. Remaining critical work is
  deterministic effect-path failure coverage, per-head DMA-BUF/retained
  lowering, and the signed physical multi-monitor gate.

## 2026-08-16: ordinary per-head submissions are not Present cohorts

- The first signed unequal-head physical run after semantic per-head composition
  rendered, prepared, submitted, and retired native-size frames on both heads.
  The session then failed after the next ordinary `HeadComposition` submission
  because the native completion path unconditionally advanced the DMA-BUF
  Present scheduler and found no in-flight Present cohort. Atomic scanout and
  renderer ownership were not the failing boundary.
- Native submission now classifies the retained content against the
  output-scoped Present frame and transaction before changing Present state.
  A matching `MixedPresent` advances its cohort, an ordinary compositor frame
  remains independent, and mismatched ownership fails closed. The scheduler
  exposes its reserved output frame for this ownership check without treating
  that reservation as submission or presentation evidence.
- Runtime-fatal sessions now emit the final clean outer-cleanup record after all
  client workers, namespace authority, and Xauthority state have been released,
  before returning the original error. This preserves the error while allowing
  the failed physical run to enter the diagnostic archive.

## 2026-08-16: mirror attempt 0019 promotes PutImage replay to the critical path

- Signed source `a5d916279c9fb8cd03415945d0dfeb11515c1a32` completed the
  unequal-mode mirror runtime without a renderer, KMS, protocol, or ownership
  crash. Both heads produced native plans, submissions, callbacks, and
  retirements; native suspend drained with zero abandoned scanouts, renderer
  worker failures were zero, and final session health was clean. The attempt
  failed at visual confirmation, so it remains diagnostic rather than promotion
  evidence.
- The density contract did not pass. The log contains 76
  `sophia_x11_raster_requirement ... status=sampled_fallback` observations. The
  750-density head repeatedly selected the canonical 1000-density handle and
  produced no exact 750-density selection. This explains the visibly softer
  smaller-head text without contradicting native-size final composition: the
  head target was native, but its client content variant was sampled.
- Code inspection found that an accepted X11 `PutImage` marks the surface's
  semantic journal unsupported. A bounded local real-xterm trace confirmed
  opcode 72 during startup, before later ImageText8 and line drawing. Once that
  operation is accepted, late density demand can only publish the canonical
  raster, so the current gate cannot produce an exact derived 750-density
  store.
- Decision: implement bounded `PutImage` ownership and replay inside X
  Authority, treating a complete opaque replacement as a possible journal
  baseline only when canonical semantics are preserved. Cross-drawable
  `CopyArea` follows with explicit source-generation dependencies. Do not make
  Engine interpret X drawing, weaken the exact-density verifier, or label a
  resampled canonical raster as independently rendered content.

## 2026-08-16: bounded PutImage replay lands with a fail-closed retention gate

- Accepted core and MIT-SHM `PutImage` now retains its own bounded pixels in the
  X Authority semantic journal and replays them per density class. This closes
  the boundary that attempt `0019` diagnosed; the physical gate must still be
  re-run to convert it into promotion evidence.
- Decision: retention is gated on an unconditional write rather than on the
  canonical writer's looser behavior. Only tight ZPixmap depth-24/32 rows with
  no left padding, drawn through GXcopy with a full visible plane mask and no
  clip rectangles, are retained; everything else poisons the journal with the
  `unsupported_put_image` cause. The canonical writer ignores the graphics
  context entirely, so a looser gate would also have been internally consistent.
  The strict gate was chosen because a retained command must reproduce the
  canonical drawable on replay, and a conditional write cannot promise that.
  The cost is a possible false negative if a real client uploads through a
  clipped or non-copy context.
- Consequence for the next gate run: a 750-density head that still reports
  `cause=unsupported_put_image` means the gate is too strict for the traced
  client and should be relaxed toward canonical semantics, whereas
  `cause=unsupported_cross_drawable_copy` means the next replay slice is
  required. The cause is therefore the first thing to read from a failed run.
- Replay projects the retained 1x pixels rather than resampling the canonical
  store, so ordering against later text and copy commands is preserved. Each
  destination pixel is a per-channel rational area average over the source
  pixels it covers, which keeps fully covered pixels exact and blends only at
  boundaries. At unit density the projection degenerates to full coverage of one
  source pixel, so a derived 1x store would reproduce the canonical bytes.
- Sampled fallback is now cause-classified and coalesced per surface, emitting
  the first occurrence and each subsequent power of two with a cumulative count.
  The cause stays authority-private; Engine still observes only
  `SurfaceContentFidelity`.

## 2026-08-16: mirror attempt 0020 clears PutImage and exposes requirement lag

- Signed source `f5b3b4855ccea02cc463812e5494a1e7d10d9bbb`, binary sha256
  `ade158fa…`, ran the unequal-mode mirror gate on DP-1 2560x1440 and DP-2
  1920x1080. Both heads composed, queued, submitted, and retired. The run failed
  visual confirmation: DP-2's text was fuzzy rather than sharp.
- The `PutImage` replay slice worked. The evidence contains zero
  `unsupported_put_image` records, so the journal retained xterm's startup
  upload and stayed replayable through later text, line, and scroll traffic.
  That was the attempt `0019` blocker and it is closed.
- The failure moved one layer out. Every raster requirement reported
  `cause=stale_dependency`, and no derived store was ever built. Because
  `satisfy` never ran, the surface's required-class set stayed empty, only the
  canonical 1x variant was ever published, and both heads selected
  `density_millis=1000`. DP-2 therefore downsampled the canonical handle 98
  times, which is exactly the observed fuzziness.
- Diagnosis: Engine builds each requirement from its committed scene
  (`raster_requirements.rs` `reconcile`), while X Authority admits it only when
  the requested generation equals the window's current generation. The authority
  advances a generation per draw and Engine commits at frame cadence, so a
  continuously drawing client leaves the authority permanently ahead. The lag is
  structural, not a race, which is why the failure was total rather than
  intermittent.
- Telemetry defect found in the same pass: generation mismatch and extent
  mismatch both reported `stale_dependency`, so the evidence alone could not say
  which check fired. They are now distinct causes, `stale_content_generation`
  and `logical_extent_mismatch`, and a stale generation logs the authority's
  observed generation beside the requested one.
- A deterministic regression now reproduces the field condition: draws advance
  the authority past a requirement built against an older committed generation,
  the cause is `stale_content_generation` with observed above requested, and the
  identical requirement rebuilt at the current generation is satisfied. That
  isolates the lag as the sole remaining blocker without needing hardware.
- Open decision: whether requirement admission should keep exact-generation
  equality. `apply_surface_raster_requirements` builds a complete replacement
  transaction and advances the generation rather than amending committed
  content, so satisfying against current state and reporting the generation
  actually produced may be the correct invariant. The alternative is retaining
  per-generation journal state so the authority can replay a requested
  generation exactly.

## 2026-08-16: scanout cloning decided as plan-equivalence, geometry only

- Research question: whether Sophia should adopt the macOS mirroring shape —
  a logical-monitor model on top, opportunistic scanout cloning underneath,
  render copy as the universal fallback with automatic invisible switching.
  Mutter is the reference for the logical model (three-level configuration,
  mirror sets constrained to a common mode, so it never needs the strategy
  switch). Weston's DRM backend is the reference for the mechanics
  (atomic-first, `TEST_ONLY` probing, opportunistic plane promotion per
  element, not per head). Neither implements the automatic arbitration; that
  arbitration is the part Sophia would add.
- Decision: adopt the shape, post-promotion. Clone eligibility is equivalence
  of `HeadCompositionPlan` records within one logical output — the whole
  record except head identity and content checksum — plus identical modes
  including refresh and single-card membership. The decision is derived from
  existing passive data, lives in the backend, and is invisible to every
  policy surface.
- Decision: the content checksum is excluded from the predicate. A checksum
  only exists after composition, but the strategy must be chosen before it;
  and once cloned there is one composition, so no second checksum exists to
  compare. Content-inclusive eligibility could therefore promote cloning but
  never detect the need to demote it. Content identity is instead guaranteed
  by compositor determinism given geometry-equivalent plans over one scene
  generation — the same invariant the exact-density gates already rely on —
  and that guarantee is proven once, with evidence, by a dual-render audit in
  the clone gate rather than checked in production.
- Decision: any field later added to the plan record is equivalence-relevant
  by default. Unconsidered head-local state then disables cloning silently
  instead of wrongly preserving it; fail closed points in the safe direction
  without anyone remembering the clone path exists.
- Ordering consequence: the unequal-mode mirror gate exercises exactly the
  configuration cloning can never serve, so proving per-head render copy
  first is the load-bearing half of the macOS design, not a detour. The
  optimizer may decline a clone, never a topology, and every eligibility
  input changes only through `OutputTopologyTransaction`, so switching is
  configuration-cadence by construction.

## 2026-08-16: raster requirements become advisory demand, not generation contracts

- Attempt `0020` evidence closed the diagnosis. Every requirement failed with
  `cause=stale_content_generation`, never `logical_extent_mismatch`, so the
  generation check was the whole blocker. `observed_content_generation=72`
  stayed frozen while Engine's requested generation climbed 1 to 64: xterm did
  its drawing in a startup burst, and Engine then worked through committed
  generations at frame cadence. No requirement ever succeeded — the run
  contains no `variant=2`, no 750 selection, and no
  `sophia_live_surface_raster status=stale_response`, so nothing was produced
  and dropped on the return leg either.
- Root cause: both ends demanded exact generation equality between two
  counters that advance at different rates. X Authority advances a generation
  per draw; Engine commits authority transactions as a strict ordered chain
  (`visual_state.rs` requires `current == previous_committed_generation`) at
  frame cadence. A drawing client therefore leaves every requirement naming a
  generation the authority has already passed, and equality is reachable only
  if the client goes idle long enough for Engine to drain the backlog. The
  failure was total rather than intermittent for that reason.
- Decision: treat a requirement as advisory demand. The authority answers from
  current state, reports the generation the pixels were produced from rather
  than echoing the request, and anchors the reply at that generation.
  `accept_response` now refuses a reply older than the demand or naming a
  different requirement edge, and accepts one that leads it. This is safe
  because `apply_surface_raster_requirements` publishes a complete replacement
  transaction rather than amending committed content, and because replies
  travel the same ordered egress as ordinary draws, so a reply anchored ahead
  of Engine simply commits when the chain reaches it. Requiring a publication
  to match a generation Engine committed frames earlier was asking it to be
  retroactive.
- Rejected alternative: retaining per-generation journal state so the authority
  could replay a requested generation exactly. Correct, but it is real
  machinery bought to reproduce a frame the user stopped looking at several
  frames ago.
- Telemetry lesson, twice over. Collapsing generation and extent mismatch into
  one cause meant the first run could not say which check fired. Then the
  power-of-two coalescer fell silent after occurrence 64, which is exactly
  where Engine's climbing counter passed the authority's frozen one. Emission
  is now transition-sensitive — a changed cause, or the first success after a
  failing run, always emits — and satisfied requirements log at all, which
  they previously did not.

## 2026-08-16: attempt 0021 crashed on a per-frame requirement edge

- Signed source `016347168e5521c7ab4a50fe6f6f411b9ed6bdaa` reached further than
  any prior attempt and then failed at runtime with `production CPU scene is
  missing 1 committed buffer(s)`. The advisory-demand change worked on the
  authority side — replies were produced for the first time — but Engine
  rejected all 70 of them as `stale_response`, and the session died.
- First defect: `reconcile` treated the committed content generation as part of
  the demand's identity, so a drawing client minted a fresh requirement edge
  every frame. A reply produced against edge R therefore always arrived after
  Engine had moved to R+1, and `accept_response` refused it on the exact
  requirement-generation match. Relaxing the content generation alone had only
  relocated the same structural mismatch onto the neighbouring field. The
  demand is now the extent and the classes; the committed generation is only
  the vantage it was observed from, so one edge stays outstanding per distinct
  demand and a single reply is in flight at a time.
- Second defect, and the fatal one: a rejected batch was discarded whole, which
  dropped its `cpu_buffer_updates` along with its transaction. The authority
  had already advanced its generation and retains the derived variants in every
  later content set, so Engine went on to commit a scene naming buffer handles
  it had never received. Refusing a demand must not desynchronize buffer
  ownership: the transaction is now declined while the buffer updates are kept.
  This path had never executed before, because prior builds never produced a
  reply to reject.
- Telemetry lesson, a third time. `report_satisfied` only logged a success that
  ended a failing run, so a requirement satisfied from the first attempt logged
  nothing at all — the run showed 70 rejections and zero evidence that the
  authority side had started working. Steady-state silence is right for
  failures and wrong for a transition that has never been observed before.

## 2026-08-16: the owner loop committed one authority batch per frame

- Attempt `0022` on `f0ab320d` cleared every raster-path defect: zero sampled
  fallbacks, zero stale responses, no crash, and DP-2 selected an exact
  750-density variant with a real second content variant. It still failed
  visual confirmation, and the reason was cadence rather than density. The 750
  variant appeared at committed generations 71 and 72 — the last two frames of
  the run. Everything before it selected the 1000-density handle downsampled.
- Diagnosis: the live owner loop popped exactly one authority batch per
  `'session` iteration and ran one production cycle for it, so xterm's 72-draw
  startup burst needed 72 frames to reach the screen. Engine never imposed
  that: `commit_authority_batches` maps over a whole slice before one compose,
  and `rebase_authority_groups_to_committed` chains
  `previous_committed_generation` across a vector precisely so a same-surface
  run applies in one cycle. The constraint was the loop shape alone. The raster
  reply, anchored at the generation the burst ended on, simply waited its turn
  behind the backlog — correct, and 70 frames late.
- `defer_cpu_frame` did not help: it only skips rasterization, never merges
  commits, and its one-deep lookahead needs the queue already populated, which
  the drain — running only in the `Ok` arm and stopping at the first empty poll
  — frequently failed to provide.
- Decision: one owner iteration now drains and commits every immediately
  available coalescable batch in a single cycle. Two guards keep it fail
  closed. Only batches carrying pure client content may join, so Present work,
  removals, resource traffic, presentation intents, and output reservations
  keep their own cycle in order. And a run is admitted only while the admission
  pipeline is quiescent, because `projected_batch` drains the released-group
  queue: a release landing between two projections of one cycle would emit a
  quarantined group twice. A repeated transaction identity also ends a run,
  since production groups are bucketed per projection call. A raster response
  may open a run but never join one, because merging accepts every response
  before any commit.
- Method note worth keeping: `live_session` is behind the
  `atomic-scanout-live` feature, so a plain `cargo test` compiles none of it.
  Several verification passes in this session reported green without ever
  building the code under change. Feature-gated crates need
  `-p sophia-cli --features atomic-scanout-live`, which `tools/
  check_atomic_scanout_local.sh` already does for its own targets.
- Telemetry: `report_satisfied` reported only recoveries, so a density path
  that worked from the outset logged nothing — attempt `0022` showed 70
  rejections and no evidence the authority had started answering. The first
  satisfied requirement per surface now always reports.

## 2026-08-16: merging must be bounded in transactions, not batches

- Attempt `0023` on `6bdb42b4` failed at `KmsSubmit` with `session runtime
  observation batch exceeds 64 events`. Merging worked, and then overran a
  bound downstream of it.
- Cause: every committed transaction contributes one
  `AuthorityTransactionObserved` to the tick's runtime observation batch, and
  `MAX_SESSION_RUNTIME_OBSERVATION_BATCH` is 64. The merge run was bounded at
  64 *batches*, which is the wrong unit twice over — a batch may carry several
  transactions, and the fixed per-tick observations share the same budget.
- Fix: the run is now bounded by committed transactions, at the runtime
  maximum less a reserve for the fixed per-tick observations, and the constant
  is derived from `MAX_SESSION_RUNTIME_OBSERVATION_BATCH` rather than repeated
  so the two cannot drift. The head batch is exempt: it commits regardless of
  size, exactly as it did before merging existed, so the change can never make
  a previously working single-batch cycle fail. The batch cap remains as a
  second bound on owner-turn duration.
- Retained as a regression: a run assembled from many queued batches must stay
  under the runtime maximum, and one oversized batch must end the run rather
  than drag it past the budget.

## 2026-08-16: coverage was weighted in encoded bytes, not in light

- Attempt `0024` on `2273b550` completed without a crash and validated the
  whole pipeline. Merging engaged as designed — 76 authority batches became 9
  compositions, `merged_batches=68`, `max_merge_run=48` — and the raster
  round-trip closed: a requirement raised at content generation 48 was answered
  from generation 72, accepted, with zero sampled fallbacks and zero stale
  responses. The final held frame on DP-2 selected `density_millis=750
  sampling=exact` from a real second variant. The gate still failed visual
  confirmation: the operator described the text as fuzzy, like out of focus,
  soft with grey-edged glyphs rather than blocky or stair-stepped.
- That description is antialiasing, not a selection or pipeline defect, and it
  led to a real bug. `blend_copy_pixel` mixed gamma-encoded channel bytes
  arithmetically, and `blit_projected_image` averaged them the same way. Half
  coverage of white over black therefore produced about 128, roughly a fifth of
  the intended luminance rather than half. Every stroke narrower than a pixel
  was uniformly under-weighted, which is exactly what reads as out of focus.
- Fix: weight coverage as light. Components are squared before mixing and
  square-rooted after, in integer arithmetic, so replay stays bit-reproducible;
  a true transfer function needs a power the platform may round differently,
  which would put deterministic pixel gates at risk. Full and zero coverage map
  to the endpoints exactly, so canonical-density text remains bit-identical to
  the 1x drawable.
- Open, and larger than the bug: the gate proves per-head density with xterm at
  `-fn 6x13`, a fixed bitmap font. At 750 density its stems are 0.75 pixels
  wide, so no stem can land on a whole pixel and some softness survives any
  correct weighting; binary thresholding instead would produce the blocky
  result the same criterion rejects. For a bitmap font, exact-density replay
  and downsampling are also nearly the same operation, so the architecture's
  advantage is small for precisely this content. Vector content re-renders
  crisply at a native density and would demonstrate it plainly. If the
  corrected weighting does not reach the visual bar, the choice is between
  vendoring additional fixed font sizes so a derived store renders at a size
  matching its density, and changing what the gate measures.

## 2026-08-16: the mirror gate's visual criterion is unreachable for bitmap text

- Attempt `0025` on `31fed705` carried the corrected coverage weighting and
  looked unchanged to the operator: still fuzzy, still out of focus. The
  telemetry was clean — DP-2 selected `density_millis=750 sampling=exact` on
  the held frame, zero sampled fallback, zero stale responses, the burst
  committed in 9 cycles.
- Measurement rather than judgement settled it. The final composed frames
  report `nonzero_rgb_pixels=10036` on DP-2 at 1920x1080 against `9668` on
  DP-1 at 2560x1440: the smaller head lights more pixels than the larger one
  while showing the same content, about 1.85 times the ink density. Crisp
  content scaled down lights fewer pixels. Every stem is being spread across
  neighbours as partial coverage, which is fuzziness measured directly.
- Cause, and it is not a defect: 0.75 is close to the worst ratio for a
  fixed-cell bitmap font. A six-pixel cell becomes 4.5 pixels, so alternate
  characters straddle pixel boundaries, and a one-pixel stem covers three
  quarters of a pixel. No weighting recovers a stem that never occupies a whole
  pixel; thresholding one crisp produces the blocky result the same criterion
  rejects. Fuzzy and blocky are the only outcomes available for this content at
  this ratio.
- Retracted: an earlier suggestion to vendor additional fixed font sizes so a
  derived store renders at a size matching its density. It cannot work for a
  terminal at 0.75, because the advance is 4.5 pixels and no integer-width font
  sits on that grid without drifting across the line or changing the column
  count. Per-density font selection helps at integer ratios only.
- Retained as deterministic evidence rather than argument: replay keeps a
  one-pixel line fully lit at 0.75 where area-resampling the canonical raster
  reaches no fully lit pixel at all, and replayed versus resampled 6x13 glyphs
  land within a few levels of each other. Both comparisons use the same area
  rule, so they isolate the source of the pixels rather than the choice of
  filter. That is the architecture's benefit and its boundary, stated as a
  measurement.
- Two structural conclusions. A mirror always resamples for at least one head,
  because one client renders one raster at one size; the unequal-mode mirror is
  therefore the least favourable demonstration of per-head composition. And
  exact replay needs semantic content, which modern toolkits do not provide —
  they rasterize text themselves and upload pixels. Visual acceptance of
  native-density rendering moves to the extended topology, where a window is
  rendered at its own head's density and nothing is resampled.

## 2026-08-16: provisional topology composition discarded renderer ownership

- The first mixed mirror-plus-extended rerun after moving preparation into the
  session owner crossed the new quiescence barrier correctly. Its last ordinary
  frame retired, no KMS submit occurred, and candidate preparation then failed
  with `MissingSource(DmaBuf { handle: 8 })`. The supervised reference peer
  restarted and reproduced the same deterministic rejection until its restart
  budget was exhausted.
- Cause: ordinary retained composition resolves Engine's committed scene into a
  mixed set of CPU buffers and authority-owned renderer images. Provisional
  topology composition instead rebuilt CPU layers and called the CPU-only head
  lowerer. A committed DMA-BUF handle records logical source identity; it does
  not recreate the renderer-private image that already owns those pixels.
- Decision: candidate and rollback head plans use the same retained mixed source
  set as ordinary frames. The provisional viewport still produces its own
  display list, snapshot, native geometry, target generation, and damage, but
  lowering consumes the already-owned CPU or renderer-image realization. This
  remains read-only until the complete topology resource cohort is prepared and
  preserves the Engine/backend boundary: Engine names committed pixels, while
  the backend retains their renderer realization.

## 2026-08-16: the output proof waited for a scene echo that need not exist

- The next mixed run showed both proof windows tiled on the large output while
  both smaller outputs remained black. This was the unchanged three-output
  startup topology, not a partial modeset: the log contained no output-role
  connection or proposal. All three startup heads presented and retired
  normally.
- Cause: after committing the proposal that first placed both surfaces, the
  reference policy decided whether to start its output role from the preceding
  scene snapshot. That snapshot still described one placed surface. No further
  policy cause was required after the second commit, so waiting for another
  snapshot blocked until the four-second policy socket timeout. The committed
  proposal already carried both authoritative placements.
- The resulting restart exposed a second ordering bug. The output service could
  already be blocked in `accept` when the owner spawned a replacement and
  queued its new PID. A fast replacement connected first and was correctly
  rejected against the old assignee identity, then the dual-role process exited
  again.
- Decision: the proof start barrier counts distinct surfaces in the committed
  proposal itself. Separately, output-service acceptance now has a synchronous
  pre-spawn pause. The owner closes the old connection and waits for that pause,
  spawns the replacement, then installs its exact PID; only processing that PID
  command resumes negotiation. This preserves exclusive-role authentication
  without relying on scheduler timing or weakening the peer check.

## 2026-08-17: the mixed-output gate died asking for a raster of a DMA-BUF

- `tools/run_mixed_output_gate_tty4.sh` failed every run on `3a4398a1` at
  `stage=runtime exit=1` with `failed to satisfy surface raster requirements:
  InvalidResource`. The evidence contains no `sophia_x11_raster_requirement`
  line at all, so the X authority server died answering the first requirement
  it ever received, immediately after
  `sophia_live_output_authority status=first_presented` — the moment the mixed
  topology brought a 750-density head into existence.
- Two defects, one on each side of the boundary. Engine's requirement tracker
  had no content-source filter, and `SurfaceContentSet::singleton` labels every
  single-variant set `AuthorityRaster`, renderer buffers included. So a
  non-1000 head made Engine ask X Authority to produce a 750-density variant of
  vkcube's DMA-BUF. X Authority then answered an unanswerable question by
  failing: a surface with no CPU presentation snapshot returned
  `InvalidResource`, the connection loop propagated it with `?`, and one
  surface's demand ended the display server.
- Why it surfaced only now: the crash needs a DMA-BUF client *and* a
  non-1000-density head. The unequal mirror gate had the head and only CPU
  clients; every other session had renderer clients and only 1000-density
  heads. The mixed gate is the first run with both, so it died deterministically
  at the same instant every time.
- The neighbouring `Invalidate raster replay across Present` change is correct
  and was not the cause. It poisons the journal for a surface that had CPU
  content and then presented, which degrades cleanly to sampled fallback. This
  crash needs a surface that never had a CPU drawable at all, which
  invalidation cannot reach.
- Fixes: Engine raises requirements only for CPU-backed canonical content;
  X Authority reports an absent canonical drawable as the new
  `no_canonical_raster` cause and treats the store's refusals as answers rather
  than runtime errors; and the connection loop no longer propagates a
  per-surface error at all — it warns with `status=refused` and keeps serving.
  The last of those is the one that matters longest: a surface can switch CPU
  to renderer content while a requirement is in flight, so the race outlives
  the demand filter, and no single surface should ever be able to take the
  server down.
- Rejected: relabelling `singleton` content as `SampledFallback` for non-CPU
  sources. That label is load-bearing in head-plan selection, where it would
  turn natively correct renderer buffers into `Fallback` bindings. The demand
  filter is the right place.
- Left as-is deliberately: the malformed-input guards at the top of
  `apply_surface_raster_requirements` (`requirements.validate()` and
  `transaction.is_valid()`) still return errors. Those indicate a programming
  fault rather than a legitimate content state, and the connection loop now
  contains them without dying.
- Retained as a regression that reproduces the exact failure offline: a
  requirement against a window that never core-drew must return
  `no_canonical_raster` rather than an error. Reverting the fix makes it fail
  with the gate's own `InvalidResource`.

## 2026-08-17: the first live output_v1 apply landed, then a timeout killed the WM

- With the unrasterizable-demand fix in place, the mixed-output gate reached
  and passed the milestone it exists for: `sophia_output_v1_reference schema=1
  status=settled kind=Committed topology_epoch=2 heads=3 groups=2`. A proposal
  crossed the wire, formed a mirror group plus an extended output across three
  physical heads, and committed. This is the first live `sophia_output_v1`
  apply on hardware; every prior topology had come from a startup profile.
- The run still failed, downstream of that. After the topology committed, the
  proof surface had to reshape 1920x1080 to 1280x1440 for its new head. The
  client did not adopt in time, so the session timed the transaction out and
  handled it exactly as designed — `layout_timeout ... preserved_layout=true`
  with a clean rollback. The reference policy client then classified `TimedOut`
  as fatal and exited. The supervisor restarted it through epochs 2, 3, and 4;
  the fourth exit exhausted the restart budget, `apply` returned `Ok(None)`,
  and the session died with "public WM supervisor did not restart Hagia".
- Cause: `stateless_reference_projection_decision` grouped `TimedOut` with
  `RejectedInvalid` and `Disconnected`. A timeout names a slow *client*, not a
  faulty proposal or a broken connection. The session preserves the committed
  layout and rolls the proposal back, so re-proposing from a fresh snapshot is
  the entire recovery — the same handling `RejectedStale` already had. The
  conformance corpus already requires this client to recover from a timeout
  and does exercise it, but against the reducer rather than the live loop, so
  the live classification drifted without a test noticing.
- Fix: `TimedOut` joins `RejectedStale` as `RetryFreshSnapshot`. Fatal is now
  reserved for faults retrying cannot repair. The regression was rewritten to
  assert both recoverable outcomes and both fatal ones by name.
- Consequence worth watching on the next run: if the reshape was a transient
  during native-scanout drain, the retry settles and the gate proceeds to its
  visual phase. If it times out indefinitely, the gate will now run to its
  bounded end without a settled `sophia_wm_v1_reference` record instead of
  crash-looping, which distinguishes a slow adoption from a reshape that never
  completes.

## 2026-08-17: a stale projection rejection stranded the public policy owner

- The mixed-output gate on `131efc15` failed with "public WM supervisor did not
  restart Hagia" and lit only two of three monitors. The topology apply itself
  succeeded — `sophia_output_v1_reference status=settled kind=Committed
  topology_epoch=2 heads=3 groups=2` — from a correct three-head start.
- Chain: the commit enqueued one relayout cause; the owner issued one cycle;
  scene facts advanced again before the response landed, so the owner rejected
  it as stale. `settle_rejected_projection` cleared its in-flight state and
  enqueued nothing, leaving an empty queue and no outstanding request. The
  client had mapped that rejection to "retry from a fresh snapshot" and blocked
  in `receive_snapshot`, so it sat until its four-second socket deadline fired,
  exited, and was restarted. Three restarts exhausted
  `RestartPolicy { max_attempts: 3 }` and the session died.
- The two-monitor symptom was downstream of the same cause, not separate. After
  the apply the mirror pair retired three page flips each while the extended
  head retired exactly one: the window manager died before it ever placed a
  surface on the extended output, so that head showed only its
  first-presentation frame.
- Root cause stated as an invariant: the owner re-offers a cycle for exactly
  the outcomes a stateless client retries. The client half already existed; the
  owner half was missing. An invalid rejection deliberately does not re-arm,
  because the scene did not move and re-offering would spin.
- A second site had the identical defect and would have reproduced the same
  death through a layout timeout: `apply_public_commit_result` in
  `wm/commit.rs` cleared the same three fields and queued nothing. Both now
  route through one `settle_public_projection`, so the decision cannot drift
  between them again.
- Re-arming goes through the pending-dirty path rather than a direct enqueue.
  That path already merges into a queued relayout and already defers while one
  is in flight, and it cannot report a full queue — there is no recovery from
  owing a cycle that will not fit.
- A latent defect became reachable and was fixed in the same change: a topology
  update whose relayout cause was rejected as a duplicate dropped the
  replacement affected-output list, leaving a queued cause that could name an
  output which no longer existed. Issuing such a cause fails the session, and
  the re-arm makes a queued relayout far more common.
- Deliberately not added: a consecutive-stale ceiling escalating to a transport
  restart, which is what the legacy path did. The supervisor never feeds
  `ProcessHealthy`, so `restart_attempts` never resets and three lifetime
  restarts always exhaust the budget; escalating would convert a recoverable
  condition into session death, which is the failure being fixed. Recorded as a
  dependency for any future escalation.

## 2026-08-17: the owner's re-arm starved a client that had stopped reading

- The re-arm fix held. Two stale rejections were logged with `rearmed=true` and
  the session survived both, where either one previously killed it, and restarts
  fell from three to one. The run still failed, in two new places.
- First, the reference client stopped servicing its policy connection. Once its
  proof surfaces were placed it blocked for up to ten seconds waiting for the
  topology outcome, deliberately, to avoid generating layout commits while
  native scanout drained. Its own comment claimed the pause could not strand
  anything. It stranded the owner's *write*: the owner, now correctly re-arming,
  kept issuing cycles into a socket nobody was draining until its write deadline
  fired, reported `restart_requested reason=public_transport_failed
  error=Io(...os error 11)`, and restarted the client in the middle of its own
  topology apply. That is why this run contains no
  `sophia_output_v1_reference` line at all: the output client had connected and
  was killed before it could submit. The re-arm did not create the fragility,
  but it supplied the write pressure that reached it.
- The client now collects the topology outcome without ever blocking, and
  tolerates the owner's expected quiet window while a topology transaction
  prepares rather than treating a read deadline as a fault. Re-proposing during
  the drain is harmless because its tiling is a pure projection of the received
  snapshot.
- Second, and initially misdiagnosed: `begin_recovery` failed the session with
  "layout recovery surface has no committed authority size". The first theory
  was that repeated recovery rounds drained the safe observations. That was
  wrong — `record_committed` seeds a safe observation and only `remove` clears
  one, so the absence means the coordinator no longer knows the surface at all.
  The all-or-nothing failure is therefore correct, and an Engine test already
  pinned it; relaxing it would have destroyed a real invariant to paper over a
  caller bug.
- The actual defect was an asymmetry in the caller. It filtered its fixed
  admission set by `safe_size(..).is_some()` but applied no such test to the
  requested sizes, so a surface withdrawn while its resize was outstanding took
  down the recovery of every surface that could still have been recovered. Both
  arguments are now filtered the same way, and the Engine invariant is untouched.

## Bounded-resource saturation as one vocabulary

Seven consecutive physical-gate failures each turned out to be a different
bounded resource whose overflow was fatal: journal capacity, observation batch,
restart budget, transport write deadline, recovery request, owner deadlock, and
finally the native input acquisition queue. Every fix was correct and every one
revealed the next instance, which is the signature of a class defect rather than
seven bugs.

An inventory of the live-session path found 30+ bounded resources: 24 fatal, 6
silent drops, 1 unbounded, and 1 blocking with no deadline, against 6 doing
proper backpressure and 5 degrading with cause. The defect was not a missing
policy. Five dispositions were already in use and already documented. The defect
was that each site picked one ad hoc, so identical resource classes behaved
oppositely -- per-client input queues correctly closed one endpoint while the
shared ingress killed the session; focus handoff correctly discarded a held
sequence while the pressed-key ledger treated the same pressure as fatal;
page-flip decode correctly stopped and reported while its sibling callback queue
was fatal.

The root cause was a misreading, and it is worth naming precisely. `dod.md` said
of the pre-admission group FIFO that "capacity exhaustion or a mismatched member
is an explicit terminal error, never an unbounded allocation." That sentence is
about one FIFO where continuing really would split an atomic group. Only its
second clause generalises. Read as blanket policy it justified 24 fatal sites,
and the fix is not to soften it but to scope it: what a resource does at its
bound is now data attached to the resource, so changing that behaviour changes a
value rather than control flow at a call site. The FIFO itself stays terminal,
which is what makes this a classification rather than a general softening.

`TargetInputPacing.tla` gained the producer it never had. Device acquisition is
the one input the compositor cannot decline to have happen -- the packet already
exists when capacity is examined -- so a full queue is a choice of disposition
rather than an absence of work. Modelling it first was worth more than expected,
because two of the invariants written for it did not survive their own negative
controls. `EscalationCanAlwaysFlush` was a strict consequence of
`BoundaryCapacityIsReserved` and no edit could break one without the other, and
constraining `deferralTicks` in both `TypeOK` and `DeferralIsBounded` meant
`TypeOK` failed first, so the ceiling was never the invariant under test. Both
were removed rather than kept as decoration. The fairness control matters most:
all four new actions sit outside fairness, and deleting the assumption on `Drain`
still fails `QueueEventuallyEmpties`, so admitting a deferral disposition did not
quietly convert progress into a tautology.

Two model results changed the implementation rather than confirming it. The
reserve turned out to be load-bearing in a way the original model only implied:
two slots per active seat is exactly what an endpoint closure spends on its
terminating boundary, so the admission bound is what makes closure always
possible. And conservation at the acquisition boundary -- produced equals
admitted plus discarded plus held -- is what forbids two arrivals collapsing into
one record. A resource that coalesces under pressure reports success while losing
user input, which is the failure mode a gate cannot see.

The verification itself had a defect worth recording. The first several model
runs used the TLA+ jar named by a locally exported variable, which did not match
the checksum `tools/check_tla.sh` pins; the pinned 1.7.4 jar was elsewhere on
disk. The results happened to agree once re-run, but the pin exists precisely so
that agreement is not assumed, and the honest reading is that those runs proved
nothing until repeated under the pinned toolchain.

## Applying the dispositions, and one row of the plan that was wrong

Seven of the ten converted sites went as designed. The libinput acquisition
worker now defers for 50ms before abandoning a batch, and counts what it
abandons; the seven routed-input ingress sites close the endpoint epoch rather
than the session; the XKB worker returns typed errors and waits on a deadline
instead of panicking inside its thread; the present-cadence sampler slides its
window instead of latching an overflow flag; a lost key-timing sidecar is
consumed rather than fatal; and a full pressed-key ledger closes the epoch and
flushes what it holds.

Two of those needed a distinction the plan did not draw. At the acquisition
worker, teardown and a departed recipient are not degradations of a running
session, and a site that counts them as loss reports it on every clean exit --
so the shared batch driver names `Cancelled` and `RecipientGone` separately from
`Saturated`, and only the last carries a discard count. At the timing sidecar,
an *absent* measurement and a *mismatched* one are different failures: absence
is capacity pressure and is consumed, but a sidecar that disagrees with its
event means the serial-to-timing association is wrong, which would make every
latency number untrustworthy. That one stays fatal. Softening it would have been
the same mistake as relaxing the layout-recovery invariant earlier in this work.

The routed-input conversion turned out not to need the depth-tracked reserve the
model suggested. `advance_control_epoch` is a compare-exchange on an atomic
rather than a queued record, so the close is deliverable no matter how full the
queue is. The epoch is therefore itself the terminating boundary, and applying
it clears active grabs and frozen input. Releases still get a short bounded wait
that ordinary input does not, because a lost release leaves a client believing a
key is held, but the reserve that the model proves is load-bearing is discharged
here by an out-of-band mechanism rather than by held-back queue slots.

One row of the plan was wrong and is dropped. It proposed making the session
runtime observation batch `RejectAndConsume` and raising its bound from 64 to
256. Reading `session/reducer.rs` first shows why that would have been a real
defect: `SessionRuntimeObservation` is not telemetry. Every observation drives a
phase transition and emits a command -- `TickStarted` yields `PollXEvents`,
`FrameRendered` yields `SubmitScanout`. Dropping one would skip a frame render or
a scanout submission while reporting success, which is precisely the quiet
degradation this whole change exists to prevent. The existing terminal error is
correct, and the real fix was already in place on the producer side, where
`AUTHORITY_MERGE_TRANSACTION_LIMIT` bounds a merged run so the batch cannot
overflow. Raising the bound to 256 would also have loosened that throttle and
risked the long owner turn its comment warns about. The lesson repeats: check
whether a bounded resource carries state or carries diagnostics before choosing
its disposition, because the two look identical at the call site.

The pressed-key ledger change interacts with the ingress change in a way worth
recording. Because a dropped release now leaves its key recorded as pressed --
truthfully, since the client never saw the release -- pressure on the ledger
went up, not down. That is why the epoch close flushes every held key rather
than only reporting: without the flush the ledger would stay full and refuse
every later press for the same reason forever.

## The frame-service arbitration deadlock

The mixed-output gate stalled at the topology quiescence wait with
`runtime_blocker=runnable_queued_present` and `kms_submits=0` for the full two
seconds. Four guesses at the mechanism were wrong in a row; every one of them
was corrected by instrumentation rather than by reasoning, which is the first
thing worth recording. The decomposed timeout line, then the per-head report,
then the multi-clause runtime report each narrowed it, and only the last one
was specific enough to trace.

The cause was a mutual suppression between a reducer and the handler it feeds.
`drive_gpu_presentation` refused a queued present while the primary output owed
a native frame, and refused it silently -- no pop, no rejection, no log. The
frame-service reducer, meanwhile, withheld `SubmitPendingFrame` from the
primary whenever a present was queued, and that effect is the only thing that
drains such a frame. The pending frame blocked the present; the present blocked
the drain. Nothing re-defers a runnable present, so `has_runnable_queued`
stayed true forever and no submission ever reached the kernel.

The gap existed because the two halves measured different things. The reducer
admitted a present on `native_phase == Idle`, derived from scanout submission
and cleanup alone; the handler refused on `pending_frame`, which additionally
counts queued mirror successors, prepared scanouts, and exporter frames that
have not yet entered a renderer worker. Every state in between satisfied the
reducer and failed the handler.

An existing test asserted the defect as correct. `frame_service.rs` contained
`queued_presentation_reserves_primary_but_not_secondary`, pinning the exact
reservation that caused the stall. This is the second time in this work that a
test encoded the bug it should have caught -- the first was the topology
quarantine, where a test drove the hotplug writer through a policy-change
quarantine. Both tests were named after the mechanism they exercised rather
than the property they protected, which is what let them look correct.

The fix gives the reducer the ordering and makes each handler refusal
unreachable by construction: a present is emitted only when the primary owes
nothing, the primary's drain is never withheld, and software staging is emitted
only when every output is idle. That last one closed a latent session fatal
rather than a stall: `run_native_pending_output` errors outright when staging
is blocked, so the reducer had been able to emit an effect that killed the
session. `EmittedEffectsAreExecutable` in the new
`validation/tla/FrameServiceArbitration.tla` states each handler precondition
as a consequence of its reducer gate, which is what keeps the two from drifting
apart again. Restoring the old reservation in that model violates
`PresentSettles` at exactly the production state.

Two further things surfaced while tracing. A waiting software present makes
every output report a pending frame and suppresses all present dispatch, so the
same wedge exists in the software half; splitting `software_frame_waiting` out
of the per-output flag makes the reducer able to tell "this output owes a
frame" from "something global is pending". And bound software presents settle
only on a real page flip of the head frames they were lowered onto, while
topology installation discards exactly those frames -- so an installed topology
could strand a binding forever, leaving the runtime permanently non-quiescent
and the client permanently without feedback. Both terminals now settle them.

The escalation added alongside is the `BoundedDeferral` disposition from the
capacity vocabulary, applied to a wait that had none. The wait blocks on owners
that can only advance while it waits, so an expiry that merely reports a stall
has not tried to clear it; the first expiry now skips what is runnable and what
is waiting, settling each as `Skipped` so no client is left expecting feedback,
and grants one more window. A second expiry rejects the candidate as before.
The displayed topology is never given up, which is what distinguishes this from
the suspend path it was factored out of.

## A present was judged stable only at a quiet the pipeline no longer has

The mixed-output gate reached the end of the topology lifecycle -- committed,
policy-committed, settled, input restored, no layout timeout, both surfaces
drawing -- and then failed with "persistent live session never reached startup
readiness". Eleven presents retired over the run and every one of them was
reported `superseded`. None was reported `stable`, which is the evidence
readiness waits for.

The predicate asked for three things: that the displayed content name this
transaction and carry nonzero pixels, that nothing newer be submitted, and that
no head be busy with an exporter frame, a prepared scanout, or a scanout
submission. Only the first is about the flip being judged. The other two are
about the instant of judgement, and that instant is chosen badly: stability is
evaluated after a whole `service_native` pass, and the pass that polls a
retirement goes on to submit the successor before it returns. For a mirror
group it is worse still -- retirement itself promotes the coalesced successor
into every head's exporter slot, so the third condition is falsified by the very
retirement being reported. A pipeline that is flowing can satisfy the predicate
only in the gaps between frames, and two kitty clients with the owner pacing at
1 ms under pointer motion leave no such gap for anyone to observe.

The first explanation offered here was wrong, and worth recording as wrong: I
assumed the strict form had been satisfiable before because the frame reducer's
primary reservation kept things quiet. It did not keep anything quiet -- it kept
the exporter slot permanently full, which is precisely the arbitration deadlock
described above. The strict predicate passed in old runs only at genuinely idle
instants, when no present was runnable and nothing composed across the flip.
Fixing the deadlock did not break stability detection; it removed the stall
that had been masquerading as it.

The conjuncts were never argued for. `e2cfe3ac` introduced "no newer primary
frame queued or submitted" when the reducer's reservation made the clause nearly
free, and the two busy terms arrived later under one-line messages with no
recorded rationale. Nothing downstream wants them: startup evidence, the
readiness gate, and the input-pixel proof all want "this transaction's pixels
reached the screen", and the input proof in particular means "the post-input
transaction was displayed", not "the compositor then stopped". The model was
already the looser of the two -- `PresentFrameOwnership.tla` explicitly permits
a successor frame submitted after a Present's retirement is observed, requiring
only that the successor cannot steal or block the captured retirement. The code
was stricter than the specification it is meant to implement.

So a present is stably presented when its page flip retired with it as the
displayed content carrying real pixels, and successors do not unsettle it.
Evaluation can stay where it is because the successor's own flip callback
cannot arrive inside the pass that submitted it -- events are polled at pass
start -- so the displayed content still names the retired transaction when the
question is asked. The two conditions that raced are the two that are gone.

Starvation was ruled out before the predicate was touched: the eleven flips
were spread across the run rather than clustered at its start, so presents were
reaching the kernel throughout. The telemetry line went to `schema=2` at the
same time. Its `pending_primary` field was `!stable` written twice under a name
that suggested an independent fact; it is replaced by the pixel count, which is
what actually distinguishes "shown, but blank" from "not shown".

## The pixel proof was spent before there were any pixels to prove

With stability narrowed to "this transaction was displayed carrying real
pixels", the next run still called every retirement superseded, and now said
why: `nonzero_rgb_pixels=0` on all of them. The presented content was right --
`MixedPresent { transaction: 493 }` on the head that flipped it -- so only the
pixel term was failing.

That term is not a measurement of the frame it is attached to. A renderer
context reads its composition back at most three times and keeps the last
result; a full-framebuffer readback is far too expensive to run per frame. The
log names all three: `sophia_native_composition_frame status=verified` appears
exactly three times per head, all of them in the first hundred milliseconds,
all of them `nonzero_rgb_pixels=0`. Every one measured a composition with zero
layers -- a clear to black, which cannot show anything by construction. The
budget was gone before the first client had drawn, and every present for the
rest of the session carried the zero those three attempts latched. No present
could be judged to have put light on a screen, so startup readiness was
unreachable no matter how well the pipeline ran.

The proof is now spent only where light could appear: an attempt requires a
composition with at least one layer. The budget is named
(`NATIVE_COMPOSITION_PIXEL_PROOF_ATTEMPTS`) rather than written as a literal in
three places, and the stamping site says what the value is -- the head's proof
that it has shown light, not this frame's pixel count. Which is what every
consumer wanted: readiness asks "has this client's content reached a screen",
and answers it with the displayed transaction plus that proof.

## An epoch the session closed itself was reported as a delivery fault

The same run died before any of that mattered, four seconds in, with
`X11 input delivery failed: outcome=RouteRejected client=1`. The pointer moved
while an output policy change closed the input security epoch. Closing it
strands whatever was in flight -- frozen input is drained, and any event
stamped with the old epoch is refused at the registry -- and both paths
reported `RouteRejected`, which the owner loop treats as fatal.

So every pointer motion that overlapped a topology, policy, or seat boundary
was a session-ending race. This is the timeout-is-not-a-fault distinction in
another costume: an outcome that means "the session did this on purpose" was
sharing a name with one that means "this could not be delivered".
`XAuthorityInputDeliveryOutcome::EpochRevoked` now names the first, and the
owner loop retires it the way it retires a departed target -- the event is no
longer expected, and it is reported rather than fatal. `RouteRejected` keeps
its meaning and keeps ending the session: an unresolvable window or an
unmappable button is still a fault.

## The scene was filtered against one output, so a second output showed nothing

The gate finally reached its telemetry stage: readiness at 256 ms, the whole
topology lifecycle committed and settled, health clean,
`content_ready source=stable_present_scanout` twice, and presents reported
`status=stable` with real pixel counts. What it failed on was the extended
head, which never showed a single client pixel in thirty seconds.

Transaction 597 tells the story on its own. It appears exactly once in the
evidence -- `visual_armed epoch=14 surface=4194318 width=1920 height=1080` --
and then never again. Not staged, not composed, not retired, not rejected. Its
sibling on the mirror output, armed in the same instant, committed normally.

The presentation layout is built by filtering every layer through
`surface_visible_on_output(layer, output.id)`, where `output` is the primary
output alone. While the desktop had one logical output that question was
harmless. A mixed topology makes it wrong: the policy placed one surface on
the mirror group and one on the extended output, and the extended one was
filtered out of the scene entirely. From there the deadlock is short. A staged
Present is released only when its surface is in the projection; the surface is
in the projection only if it is composed; and the resize that would place it
on the extended output commits only when that Present retires. The head stayed
blank, and the client -- still waiting on a Present that could never complete
-- stopped drawing, which is what made the terminal look dead to the keyboard.
The keys were observed, routed, and flushed; the window they went to could not
repaint.

One scene spans every output, so the question is whether any live output shows
the surface. `policy_projections_place_surface` now answers it over a set of
outputs, and per-head plans continue to decide by geometry which head shows
what -- that part was always right.

The gate itself needed one correction, in the opposite direction from the
usual. It required the extended head to prepare an unsampled exact frame
during the topology apply and looked for a `head_composition_plan` line to
prove it. No such line can exist there: the first topology frame is composed
from the last committed scene rather than from a fresh engine plan, exactly as
the check own comment says, and plans are emitted where the engine binds scene
layers to a head. The evidence that does exist is the queued candidate, which
names the head mode and its exact mapping. The check now reads that, and
separately requires every plan after the commit -- where plans do exist -- to
be unsampled and free of fallback. Inventing a plan-shaped line for the
install path would have meant deriving the same fact twice, which is the
defect class this log keeps recording.

## A head plan measured one buffer while the compositor held another

Composing the scene for every output moved the failure one layer down: the
session now dies at `SourceSizeMismatch(7)` while lowering a head
composition, immediately after the mirror heads plan cleanly. The extended
head is in the scene at last -- that is what makes this reachable -- and the
surface it must show has a size the compositor cannot supply.

The two records come from different places. A head plan takes each layer's
`source_pixel_size` from the committed content set, which a resize advances at
admission. The pixels come from `displayed_surfaces`, a retained renderer
image refreshed only when that surface's own Present retires. Between those
two moments the committed record names a buffer whose pixels nothing has
composed yet, and the retained image still holds the previous frame. Lowering
compares them and ends the session.

Which record is stale decides the fix, and the error could not say: it carried
a bare buffer handle. It now carries the surface, both sizes, and the handle,
because "7" cannot distinguish a mirror surface pinned to its produced pixels
by a recovery extent from an extended-output surface that answered a configure
the compositor has not drawn yet. Both are live candidates here and they want
opposite treatments -- one wants the plan to bind what can be drawn, the other
wants the head to keep its current frame until the client's Present lands.

## What the mixed gate asks an operator to see

The gate reached its visual confirmation with every mechanical stage passed --
`sophia_mixed_output_evidence status=verified`, the extended head composing
`exact=1 active=1` at 1920x1080 with `exact_nearest` and no resampling across
fourteen compositions -- and was answered no. The operator saw a sharp centre
screen, a sharp right screen, and a fuzzy left one that looked like a
downscaled copy of the centre.

That is exactly what a mirror group is. DP-1 and DP-3 share one logical output
at the primary mode of 2560x1440, there is one client buffer, and DP-3 is a
1920x1080 panel: the copy must be resampled -- `sharp_downscale 1280x1440 ->
960x1080`, which the telemetry reports on every composition. The alternatives
are letterboxing or sizing the group to its smallest member, which would move
the softness onto the larger screen instead.

The prompt asked whether the mirror heads "showed matching content" and left
the operator to decide whether a softer copy counted. It now names each
connector by its role, says the member is expected to look softer, and scopes
the sharpness claim to the extended head where it belongs. An acceptance
question that a correct run can fail is not a check, and the operator was
right to distrust it rather than guess.

## A proof that was never asked for is not a proof that failed

The mixed gate reported `input_pixel_change=false input_text_match=false` on a
run where typing plainly worked -- twenty key events routed and a directory
listing on the extended screen. Neither field was lying, and neither was
answering the question a reader asks. They are results of the physical input
proof, which arms only when a session is given an expected text sequence, and
the mixed gate gives none. So `false` meant "never attempted" here and means
"attempted and failed" elsewhere, in the same field of the same line.

The pointer side already draws that line: `pointer_proof=enabled|disabled` sits
beside `pointer_pixel_change` for exactly this reason. The input side had no
equivalent, so the session now emits `sophia_live_session_input_proof schema=1
status=enabled|disabled` at startup, next to where the running line is written.

It is a separate line rather than a field on the completion line deliberately.
Adding a field there means bumping `sophia_live_session schema=16`, and ten
tools match that schema literally while a verifier gates a dozen checks on its
value -- a cost worth paying when the meaning of an existing field changes, and
not worth paying to add a fact that nothing else reports. A new line states the
missing fact once, duplicates nothing, and breaks no reader. The completion
site now carries a comment pointing at it, so the next person reading two false
booleans does not have to re-derive why.

## A copy of a frame is not the buffer it was copied from

The enriched error answered its question on the first run that hit it:
`SourceSizeMismatch { surface: 2097166, handle: 8, planned: 2560x1440, held:
1280x1440 }`. The mirror surface, its committed buffer grown to the full mirror
width by the layout that followed the topology change, and the compositor
holding a 1280x1440 copy of the frame that had retired moments earlier.

Both records were accurate about different objects. A head plan measures the
surface's committed buffer -- `variant.pixel_size` from the committed content
set. A retained source hands over the compositor's own renderer image, a copy
of an earlier generation, carried under the identity of that committed buffer
because that is what the plan will look for. `retained_composition_source_set`
pairs them: identity from `committed`, pixels from `displayed_surfaces`.
Lowering then compared a measurement of the client's buffer against the size of
the compositor's copy and ended the session when they differed.

They agree only while a surface has not resized, which is why the check
survived this long, and the comparison was never meaningful for that source
kind: a mirror member already draws every retained image at a size of its own,
and placement carries a copy to its head at whatever size the head wants. The
check now applies to CPU and DMA-BUF sources, where the plan measured the very
buffer being handed over and a difference means the wrong one arrived, and not
to renderer images, whose size is their own fact. During a resize the head
therefore shows the previous frame at the new geometry for one flip, until the
Present carrying the new buffer lands -- which is what a resize looks like
everywhere.

Worth noting how this became reachable. Composing the scene for every output
is what let the extended surface present at all; with two surfaces presenting,
a repaint can now find a retained surface whose committed content is newer
than any composition. The defect was always there. Nothing had been able to
reach it.

## The optimized head was already expressible

Implementing the macOS model turned out to be mostly a matter of noticing that
the protocol already carries it. A mirror group is proposed as one logical rect
plus a mapping per member -- `Exact` or `Fit` -- so "optimize for this head"
means sizing the group to that head's mode and marking it exact while the other
members fit themselves to it. The compositor needs no change at all: head plans
are already built against the group's logical size, which is why head 3 has been
reporting `mapping=fit downsampled=1` all along.

So the work was in the reference policy, which hard-coded the primary as exact
and the member as fit, and in the gate, which now exposes the choice as
`SOPHIA_MIXED_OPTIMIZE_FOR_LABEL` and names it in the operator prompt. The
default is the primary, which is what every run so far did without saying so.

One behavioural note worth recording: the group is now sized by the optimized
head's current mode rather than by whatever logical rect the group already had.
Those coincide on this rig, and the former is the meaningful rule -- a group
optimized for a head that is not its size would be a contradiction. It also
keeps the extended output's origin honest, since it starts where the mirror
group now ends.

What this does not do is re-mode anything. Choosing the smaller member means
the larger panel keeps its own mode and receives an upscaled image, which is
macOS's trade. Windows and X instead restrict the desktop to a mode every
member supports so nothing resamples; that remains a separate roadmap item,
because it reaches into topology candidates and rollback rather than into one
policy proposal.

## Optimizing a mirror group for its smaller head shrinks the desktop under a policy

The first run with `--optimize-for=DP-3` applied the topology cleanly --
`first_presented transaction=1 outputs=2`, both heads composed at the new sizes
-- and then died on `InvalidSurfaceGeometry` at the next policy projection,
followed by a cleanup failure: "topology rollback runtime rebind failed:
logical viewport replacement omitted an output".

Optimizing for the member is the first change that makes a logical output
smaller. The mirror group goes from 2560x1440 to 1920x1080 and the desktop
union from 4480 to 3840 wide, so a placement that fitted before the commit may
not fit after it. `validate_output_projection` refuses such a placement, which
is right, and the session ends over it, which is the part worth questioning:
a projection written against the previous topology is stale, not invalid.

The rejection said only `InvalidSurfaceGeometry`, which cannot distinguish a
policy proposing nonsense from a policy that has not yet seen an output shrink
beneath it. It now carries the surface, the placement, the work area, the
output bounds, and whether the placement claimed fullscreen -- the same
treatment `SourceSizeMismatch` needed an hour earlier, for the same reason.
The empty-geometry case in the scene validator became its own variant rather
than sharing a name with the fitting case: one is a malformed surface, the
other is a placement that no longer fits, and reading a bare error could not
tell you which.

## A scene reports where surfaces are; only a proposal asserts where they belong

The enriched rejection named it in one run: surface 2097166 at 1280x1440,
work area 1920x1080, not fullscreen. That 1280x1440 is the old tile -- half of
the 2560x1440 mirror group, from the two-surface layout that was correct until
the group was re-optimized onto its 1080p member. The width still fits. The
height cannot.

The rejection did not come from a policy proposal. `validate_scene` was
deriving a projection from the scene's own surfaces and running it through
`validate_output_projection`, which requires every placement to sit inside its
output. That is a true statement about any layout a policy authors, and a
false one for the instant after an output shrinks beneath surfaces that have
not been laid out again yet. The compositor was describing exactly what was on
the screen and being told the description was invalid.

So the fit rule stays where a placement is asserted -- `validated_candidate`,
which judges what a policy proposes -- and leaves the path that reports what
already exists. A scene keeps its shape checks: valid surfaces, non-empty
geometry, unique identity, declared constraints, transient owners. Nothing was
lost by dropping the projection pass over it; every other check it applied is
satisfied by construction when the projection is built from the scene itself.

This is the same shape as the run before it. A retained copy was being held to
a measurement of the buffer it was copied from; a description was being held to
the standard of a proposal. Both were one rule applied to two different kinds
of claim.

## A raster is measured, not inferred from where it was placed

Three runs of the member-optimized gate produced three different fatals, and
the third named the real one: `SourceSizeMismatch { surface: 2097166, handle:
7, planned: 1920x1080, held: 1280x1440 }` -- both records describing the same
DMA-BUF. `held` was the imported buffer. `planned` came from the committed
content set, and that number had never been measured:

```rust
/// Builds a committed state around a single canonical raster whose
/// pixels span the geometry, which is every current producer's shape.
content: SurfaceContentSet::singleton(source, Size {
    width: geometry.width, height: geometry.height,
})
```

The stated invariant is true only of a producer that has answered the configure
that moved it. Between the configure and the redraw -- which for an X client is
a round trip through its own event loop -- the surface is placed at one size and
drawn at another, and the record asserted the placement as though the client had
reported it. Everything downstream inherited the invention: the head plan's
`source_pixel_size`, the sampling classification computed from it, and the
lowering check that compares a plan against the buffer it was handed.

`LayerSnapshot` and `OutputFrameSurfaceState` now carry `source_size`, and it
is filled from whoever measured the raster: the buffer registry in the live
session (`live_transaction_observed_size`, which already reported the physical
size when a client attached an old buffer under a new declared extent), the
committed content set in the backend projection, and the head binding in the
composition plan. Where a layer names no raster at all -- planning surfaces,
templates -- the field carries the geometry, because nothing samples it.

The mitigation that was on the table is now unnecessary, and would have been
wrong. Dropping the DMA-BUF size check would have removed the only consumer
that noticed the invention while leaving the sampling classification quietly
wrong. With the size measured, that check compares two independent
measurements of one buffer, which is exactly what it should have been doing.

One site keeps deriving size from geometry deliberately: `SurfaceSnapshot`,
where the raster is an X window's own pixmap and the two genuinely coincide.

## Two sizes, both correct, and I picked the wrong one

The raster-size change did not fix the fatal: the next run reported
`planned: 1920x1080, held: 1280x1440` again. The record was still being filled
with the placement, because the live site called
`live_transaction_observed_size`, and that helper answers a different question:

```rust
if source == transaction.target_content_size() {
    logical            // the surface reached its configured extent
} else {
    source             // an old buffer cannot satisfy a new extent
}
```

It reports the *logical* size whenever a client produced the content it
declared, which is exactly right for resize and admission gates -- they ask
whether a surface has reached its configured size, not how many pixels it
holds. Reading it as a measurement put the geometry straight back into the
committed content, and the run failed identically.

`live_transaction_raster_size` now names the other question and reads the
buffer registry, falling back to the declared extent only where no buffer is
registered. The two helpers sit next to each other with their difference
written down, and a test asserts both answers for one transaction whose buffer
does satisfy its extent: raster 1266x1412, observed 1276x1422.

Worth naming the mistake plainly. The commit before this one described
measuring a raster rather than inferring it, and then inferred it from a helper
whose name contains "observed". A function that returns one of two different
quantities depending on a comparison is a hard thing to reuse safely, and the
guard against that is what its callers ask for, not what it is called.

## An answer nobody will read is not a stuck key

With the raster measured, the member-optimized gate ran its full thirty
seconds -- presents flowing, the extended head exact at 1920x1080 with real
pixels, the mirror pair fitting a 1080p group onto a 1440p panel -- and then
failed at the very end:

```
runtime deadline key-release barrier timed out:
pressed=0 pending_deliveries=0 release_barrier_pending=0 policy_requests=1
```

Everything the barrier exists for was empty. The only outstanding obligation
was one projection request the session had asked the policy at the boundary.

That request is waited for on purpose, and the reasoning still holds: a
deadline lands at an arbitrary instant, the last pointer motion before it can
raise a focus request that cannot settle in the same tick, and ending on it
discards the user's final intent. What was wrong is what happens when the wait
expires. A key the session believes a client holds is a fault if nobody
released it -- somebody is left with a stuck key. An answer nobody will read
once the session stops is not a fault, and abandoning it is the correct end.

`AbandonedPolicyRequests` now names that outcome, reported with its count and
not raised as an error, while anything owed to a client still times out as
before. The test that pinned the old behaviour was rewritten rather than
deleted: it still asserts the wait, and now asserts the two different endings.

Three times in this work the same distinction has had to be drawn -- a socket
timeout that is not a dead peer, an epoch the session closed itself, and now an
unanswered question at shutdown. Each time the code had one outcome where the
situation had two.

## A proof that could not survive its own restart

The member-optimized run died three times over in eleven seconds:

```
sophia_live_wm restart_requested reason=public_transport_failed error=TimedOut
sophia_live_wm restarted epoch=2 restarts=1 preserved_layout=true
output_authority rejected transaction=1 error=Projection(InvalidCandidate(StaleTopology))
Error: NonCommittedOutcome(Rejected)      x3
Error: "public WM supervisor did not restart Hagia"
```

Everything before that worked. The mixed topology committed at epoch 2 early in
the run, presents flowed for half a minute, and when the policy transport went
quiet for twelve seconds the session did what it is designed to do: restart the
policy, preserve the layout, carry on. The restart is recovery, and it
succeeded.

What could not survive it was the reference proof. Restarted, it rebuilt the
candidate it had submitted the first time and sent it again -- naming a base
epoch the compositor had already moved past. The compositor refused it as
stale, correctly. The proof reads any non-committed outcome as failure, exited,
was restarted, did the same thing, and exhausted the supervisor.

A supervised process is restarted by definition, so its work has to be
idempotent, and the idempotent form of "apply this topology" is "is this
topology applied?". The proof now asks that first and reports the live epoch as
settled when the answer is yes. Optimizing for the other head is a different
desk and is not mistaken for this one, which the test pins alongside.

The restart itself is left alone. Twelve seconds of silence from a policy
client is a long time, restarting a wedged one is the designed recovery, and
loosening that on one observation would trade a working mechanism for a guess.
It is on the roadmap as something to distinguish if it recurs.

## The desk changed and the transport kept answering with the old one

The idempotence fix did not stop the restart cascade, and the reason is one
line that never appears in any of these logs:
`sophia_live_output_authority status=hardware_snapshot_published`. A snapshot
reaches output clients only through the hardware path -- a rescan or a hotplug
-- and a policy-driven topology commit does not take it.

So the compositor committed epoch 2, advanced its own published snapshot, told
the submitting client the new epoch in its outcome, and left the output
transport service holding epoch 1. That stored copy is what every future
connection is answered with. The restarted proof therefore received a desk
from before the change, saw its topology was not applied -- correctly, given
what it was told -- rebuilt the candidate against epoch 1, and was refused as
stale. Three times.

The idempotence check was still worth having and stays; it just could not
answer a question it was being lied to about. What was missing is that a
committed topology becomes the desk, so the transport's copy has to become it
too. `finish_output_settlement` now publishes the settled snapshot through the
same path the hardware publication uses, which the two now share rather than
each keeping its own transaction counter and degradation warning.

The shape is familiar by now: one fact -- what the outputs currently are --
living in two places, with only one of them updated. The compositor's authority
knew. The service that answers clients did not.

## An update arrived in front of the answer

Publishing the committed topology worked -- `committed_snapshot_published
transaction=2 topology_epoch=2 transport_published=true` -- and immediately
broke the client that had just asked for it:
`Codec(InvalidEnum { field: "message_kind", value: 66 })`. Kind 66 is
`OutputV1Snapshot`, and the client was reading the frame after its proposal as
an outcome, kind 68.

The publication went out before the settlement. A snapshot is an unsolicited
update; an outcome is the answer to a request. Sending the update first put a
frame the client was not waiting for in front of the one it was, and the client
read the next frame as an outcome regardless of what it was.

Both halves were wrong and both are fixed. The settlement now goes first and
the snapshot after it, which is the order a request and an update belong in.
And the reference client consumes updates while it waits rather than tripping
over them, because an update can arrive at any moment and a client that can
only survive quiet is not a client that survives a desktop.

Three runs, three different failures, each one a consequence of fixing the
last: publish nothing and a restarted policy reasons about a desk that is gone;
publish at the wrong moment and the client that asked cannot read the answer;
publish correctly and it is the client's turn to be strict about frames it did
not expect.

## One field cannot hold a raster and the thing it was presented into

Three fixes for `SourceSizeMismatch` addressed real defects and left the crash
standing, because each corrected a *copy* of a number whose original was
invented. The original is here:

```rust
// runtime/render_resources.rs, the DRI3 present path
let target_content_size = Size { width: record.geometry.width,
                                 height: record.geometry.height };  // the window
// pixmap_size was measured thirty lines earlier and discarded
```

That value becomes `SurfaceContentSet::logical_extent`, and `singleton` copies
it into `pixel_size` as well, so a transaction asserts a raster of whatever
size the window happens to be. The engine commits the assertion verbatim
(`authority_transaction.rs`), the head plan reads it as the texture extent, and
the renderer compares it against the buffer it was handed and ends the session.

The model already forbade what was being asserted. `SurfaceContentSet::new`
validates `pixel_size == ceil(logical_extent * density / 1000)`, so a 1280x1440
raster under a 1920x1080 extent is not expressible at any density -- and
`singleton` was constructing `Self { .. }` directly, so the one check that would
have caught it never ran. It builds through `new` now.

What made the obvious fix fail twice is that the same conflation is load-bearing
somewhere else. The resize gate asks "did the client fill what it was asked to
fill?" and answered it by comparing the registered buffer against the declared
extent -- which works only while the declaration is the window. Declaring the
pixmap instead makes that comparison vacuous, and a stale buffer reads as
satisfied: the gate opens early, quietly, which is worse than the crash.

So the transaction states both facts. `content` is the raster and spans what it
says it spans; `presentation_extent` is what the authority filled. They are
equal for a client that has answered its configure, differ for one that has
not, and for an authority projecting a descendant content window onto a larger
policy-managed surface the second is that descendant -- which is why an inset
present still proves its outer extent while a stale present does not. The gate
now compares a measurement against a measurement.

A stale present therefore commits truthfully and is placed and scaled into the
surface geometry for a flip or two, which is what an X server does with a
mismatched Present. Present feedback is untouched: it is bound to the
submission lifecycle, not to the commit.

Two things found on the way. `present_pixels_conflict_with_requested_sizes` had
been dead since it was written -- referenced only by its own test -- and is
deleted. And `sampling_class` compares densities only, so a raster that does
not span its geometry is still reported `Exact`; that is a separate defect,
now on the roadmap, and the mixed gate reads that field.

## A reader that left is not a transport that broke

Publishing the committed topology made the default gate fail where it had been
passing, and the line explains itself once the order is visible:

```
committed_snapshot_published transaction=2 topology_epoch=2 transport_published=true
sophia_output_v1_reference status=settled kind=Committed topology_epoch=2
output_authority status=degraded reason="Io(\\"Connection reset by peer\\")"
```

The proof asks one question, gets its outcome, and its output thread ends --
which is correct, and which is exactly when the snapshot published after the
settlement arrives at a closed socket. The write path mapped every I/O error to
a service failure, so the whole output service ended and took its listening
socket with it.

That also explains the other run. With the socket gone, the restarted policy
found nothing to connect to: `Io(NotFound)` three times, and the supervisor
gave up. One defect, two different-looking failures, both introduced by
publishing to clients who had every right to have left.

The read path had drawn this line already -- an exhausted stream is
`PeerDisconnected`, not an error -- so the fix is to make the write path agree.
A write that fails with a reset, a broken pipe, or an unexpected end retires
that connection the same way a read does: disconnect, tell the owner, advance
the epoch, keep serving. Anything else is still a failure.

This is the fourth time in this work that one outcome has been covering two
situations, and the third for a transport specifically. The pattern is worth
stating plainly: a peer leaving is something a server does every day, and code
that cannot say so ends up ending itself.

## What the mirror scaling choice actually is

A question about the fuzzy screen turned into the design space being written
down, so it is recorded here with the reasoning rather than only the outcome.

A mirror group has one logical size and places its members into it, so at most
one member is pixel-exact. That is not a Sophia limitation: X refuses to clone
unequal modes at all -- `xf86ModesEqual`, "different modes, can't clone" -- and
Windows Duplicate restricts the desktop to a mode every display supports.
Composing per head is what lets Sophia mirror a 1440p and a 1080p panel in the
first place, and the price of that ability is choosing who resamples. macOS
charges the same price and calls the choice "optimize for display"; its
letterboxing answers aspect-ratio mismatch, not resolution mismatch.

Three policies, not two. Optimizing for either member leaves the other
resampled; centring the smaller image inside a border on the larger head
resamples neither. Padding only works in that direction -- showing a 2560x1440
desktop unscaled on a 1080p panel would have to crop a quarter of the picture.

On FSR 1 and NIS: both are spatial upscalers, so they bear only on the
direction where content is smaller than the panel -- the group optimized for
its smaller member, where the larger head is currently on plain bilinear and a
1.33x upscale is well inside their range. Neither does anything for the default
configuration, where the smaller member downscales through a Catmull-Rom
bicubic that is already a decent kernel.

Which surfaced the finding that matters more than either: there is no sRGB
decode anywhere in the composition path. The bicubic weights gamma-encoded
bytes as though they were light, and the sampler sets no sRGB texture format.
Filtering in gamma space is the ordinary cause of muddy edges on resampled
text, it affects the configuration the gate runs by default, and it would
corrupt FSR or NIS exactly as thoroughly. The filter was never the first
problem.

Recorded in `docs/multi-monitor-composition.md` beside the per-head rules,
since it is a property of how a group is composed rather than a story about one
run.

## The socket the next client needed

The disconnect fix did not stop the cascade, and the reason is worth recording
because I stopped one step short of the failure twice.

Writing to a departed client was one of five write sites; I guarded two. The
log then showed the same `Io("Connection reset by peer")` degradation, and I
guessed the read path next -- wrote the classification, wrote a test, and the
test passed with the classification removed. That is the whole answer to
whether I was right: a unix stream reports a departed reader on the *write*
side, and a reader that closed cleanly is an ordinary end of stream, so the
read arm was unreachable code with a test that proved nothing. Both are gone.

Writing a test that reproduces the production chain found the rest of it in one
run. Settling to a client that has left retires the connection -- and then the
owner sends its next command, which met `return Err("output settlement arrived
without a client")`. The service ended, its listening socket went with it, and
the restarted policy found `Io(NotFound)` three times until the supervisor gave
up. The owner commands from its own turn and learns about a departure on the
next one, so an answer for a client that has already gone is ordinary; it is
dropped now.

Every write shares one retirement rule, because which frame happens to meet a
close is timing rather than meaning. The negative controls are the part worth
keeping: a test that passes when the code it covers is deleted is not evidence,
and running that check is what separated the real fix from the invented one.

## The test that could not fail, and the one that could

Four attempts at one defect, and the difference between the failed ones and the
last was never the reasoning -- it was whether the test could tell me I was
wrong.

The reproduction that matters is an ordering: the service writes a snapshot,
the client leaves without reading it, and the *poll that follows* meets the
departure. Every earlier test dropped the client first and published after, so
the write broke the pipe and retired the connection before any read happened.
Those tests passed with the code they covered deleted, which is the definition
of proving nothing, and twice I read that pass as confirmation.

With the order corrected the test reported exactly what the physical runs had:
`Failed { message: "Io(\\"Connection reset by peer (os error 104)\\")" }` where
`Disconnected` belonged. A unix stream has two readers here -- a blocking
`read_exact` for negotiation and proposals, and a non-blocking drain for the
poll loop -- and only the second is on this path. I had classified the first,
deleted the arm for the second as unreachable, and been wrong about which.

A departed peer reaches a socket three ways: a write to it breaks the pipe, a
read with nothing left ends unexpectedly, and a close that discarded frames
still queued for it resets the connection. One predicate covers them because
they are one situation, and every reader and writer now uses it.

The rule this leaves behind: a negative control is not a formality. Deleting
the code under test and watching the test still pass is the cheapest way to
find out that the test and the defect never met.

## A measurement that could not have seen what it was cited for

Before touching the shader I checked what the evidence would show, and it would
have shown nothing. Two sessions of this document, and a paragraph in
`docs/multi-monitor-composition.md`, said the linear-light correction was
measurable from the composition-region pixel populations the renderer already
reports. It was not, and the reason is written in the metric's own comment:
those buckets key on which channels are lit, deliberately, so that a diagnostic
palette stays legible across an intensity conversion while still exposing a
channel swap. Gamma changes intensity and nothing else. Every bucket would have
held still while every resampled pixel underneath it moved.

What remained was `checksum`, an FNV hash over all four bytes. It is maximally
sensitive and completely uninformative: it says the frame differs, never which
direction it went. Had the shader landed first, a wrong gamma and a right one
would have produced the same evidence -- a changed checksum and unmoved
populations -- and the only thing left to judge by would have been whether the
screen looked better to me, which is the impression the measurement existed to
replace.

`luminance_sum` and `luminance_buckets` were added ahead of the shader, alone,
so the baseline is banked with the old filter still in place. The weights are
integer and sum to exactly 256, so the shift is a division that never rounds and
two runs of one frame agree bit for bit the way the checksum beside them does; a
float luma would not have survived that comparison. The histogram is the metric
that judges the change and the sum is for a one-number comparison, because a
mean holds still while the population behind it splits, and a split is precisely
the shape gamma-space filtering makes: edge pixels piled into the low-mid
buckets instead of spread through the middle.

The prediction is recorded before the change rather than after: on a head that
downscales, mean luminance rises and the low-mid buckets depopulate toward the
middle; on a head sampling exactly one-to-one, every metric including the
checksum is byte-identical. That second head is a control the topology hands us
for free, and it is what makes the first head's movement mean something. The
alpha populations are a second control -- a correction to RGB must not move
them.

The rule this leaves behind: a claim that a change is measurable is itself a
claim to check, and the cheapest time to check it is before the change, when the
answer can still reorder the work. This one had been asserted in two documents
and repeated to the user before anyone asked the metric whether it could see.

## What the gate's sharpness verdict actually proves

Reading the composition path for the gamma work turned up something unrelated
and worse. The mixed-output gate reports `extended_text=sharp`, and the evidence
behind that phrase cannot distinguish a sharp frame from a resampled one.

Two derivations, unconnected. The engine's `sampling_class` compares two
`density_millis` scalars -- a property of the output-to-head projection that
knows nothing about any raster's extent. The renderer's
`native_composition_sampling` compares the actual source and target rectangles.
The parameter the renderer names `requested_sampling` is its own value, not the
engine's: the two names are homonyms across a boundary nothing crosses.

They disagree four ways. Content whose `logical_extent` differs from the
geometry it is placed into -- the ordinary state of any surface mid-resize, and
documented as legitimate. Rounding, because the projected density truncates
while the expected pixel size ceilings and each projected edge truncates on its
own, so every scale that is not dyadic-clean lands a pixel off; the engine's own
test happens to use exactly 0.75, where they agree. Mixed-axis projections,
where one scalar is asked to describe an upscale in x and a downscale in y.
And retained renderer images, exempt from the source-size check by design, whose
class is therefore computed from a number that is not the one drawn.

So the string the verifier greps for is satisfiable by a frame the GPU filtered,
and the single renderer-side check in that script rejects `status=fallback` and
`status=unavailable` while passing `requested=sharp_downscale status=active` --
the literal signature of the resample. There is also a quiet consequence beyond
the evidence: the plan's one-pixel repaint dilation for filter footprint is
gated on the plan's class, so it is skipped in exactly the cases where the
renderer is filtering and the footprint does exceed the rect.

It is the same defect as the raster-versus-presentation extent split landed a
few commits earlier, in a different place: one fact derived twice from different
data with nothing forcing agreement. It does not block the gamma work, whose
evidence comes from luminance rather than from this field, so it stays a
separate change rather than being folded in. Recorded here because it was found
by reading, not by failing, and a gate that overstates what it proves is worth
writing down the moment it is noticed.

## Filtering in light, and the filter that would have undone it

The change itself is small: decode each tap before weighting it, re-encode the
sum once at the end. Gamma 2.0 rather than the sRGB curve, matching what
`software/raster_replay.rs` chose for the CPU path and for its stated reason --
a squared approximation is cheap enough to apply to all sixteen taps of a
bicubic and keeps one transfer function in the tree instead of two that would
eventually have to be reconciled.

Two details are the ones that would have been got wrong quietly.

The first is premultiplied alpha. A stored premultiplied value is `E(c)*a`, and
squaring it gives `E(c)^2 * a^2` -- the coverage squared along with the colour.
It has to be unpremultiplied across the decode. Under gamma 2.0 both directions
collapse to something cheap, `v*v/a` in and `sqrt(L*a)` out, which is a small
mercy given it happens sixteen times per fragment. Alpha itself is never
transformed: it is coverage, not light. Getting that wrong would have left every
partially transparent edge wrong by exactly the factor the colour was corrected
by, and the image would have gone on looking plausible.

The second is the sampler. The kernel gathers its own 4x4 footprint at texel
centres, so it needs the texels as stored; leaving `GL_LINEAR` on would have had
the hardware blend them in gamma-encoded space before the shader ever ran. That
failure is invisible in the evidence -- the draw still reports
`sharp_downscale status=active` and produces a partly uncorrected frame. The
filter was chosen at the call site from the requested sampling and the program
inside the draw from whether it had compiled, two derivations with nothing
forcing agreement, which is the defect this codebase keeps meeting. They are one
function now, and the fallback is the only place `LINEAR` survives.

Widening to both directions turned out to cost nothing. Catmull-Rom is an
interpolating kernel -- it passes through its samples -- so it is the textbook
bicubic upsample as well as a reduction filter. One program serves both, which
means one place where light is decoded and re-encoded, and the roadmap item
asking for a real upscale kernel stopped being a prerequisite: Lanczos-2 and
FSR 1 are now a question of whether bicubic is sharp enough, to be asked after
the colour space is right rather than before.

The naming had to move with it. Renaming `LinearUpscale` to `SharpUpscale` left
the degraded path without a name, and looking for one surfaced an existing
conflation: the fallback reported itself as an upscale, so `linear_upscale_draws`
was incremented both by a real enlargement and by a degraded reduction, while a
second counter tallied the fallbacks beside it. Two numbers, neither of which
meant one thing. There is a `LinearFallback` variant now and a single
`linear_fallback_draws`, and any value above zero says the reconstruction shader
is not running.

Three negative controls, because this session already paid for the lesson.
Reverting the tap to weight encoded bytes fails the test that pins the decode.
Setting the reconstruction filter back to `LINEAR` fails the test that counts
which arms keep hardware filtering. Removing the guard on the encode's input
fails the test that pins the clamp ahead of the square root. The arithmetic
test alone would have passed all three, because it mirrors the shader in Rust
rather than running it -- which is exactly the shape of test that proved nothing
four commits ago.

One thing the fixture taught me mid-write. An assertion that saturated pixels do
not move failed on a pixel at 0.00011, which lifts to 0.0105 under the square
root. That is not a saturated pixel drifting; it is a faint Catmull-Rom ring
being corrected, and correcting it is the point. The band was sloppy, not the
code. Worth recording because gamma 2.0 amplifies dim values hard -- a hundredfold
at the bottom of the range -- so ringing that was invisible in encoded space is
visible now. That is what filtering in light means, not a regression, but it is
the first place to look if halos are reported.

## Shaders as shaders

The GLSL was three `const &str` literals in a Rust file. Moving it into
`.vert`/`.frag` files alongside is mostly the obvious tidying -- highlighting,
diffs, a reviewer seeing GLSL as GLSL -- and `include_str!` keeps it embedded at
compile time, so nothing is read at runtime and there is no asset to lose.

The reason worth recording is the one that is specific to this renderer. A
shader that fails to compile is not fatal here, deliberately: the pipeline logs
`status=unavailable`, falls back to the direct program, and the session keeps
running with its filtering uncorrected. That is the right behaviour when a
driver refuses something at startup on a user's machine. It is a bad property
during development, because a typo produces a working session that is quietly
wrong, and the gate would report `status=fallback` in a line nobody reads until
they go looking. With the GLSL in its own files a real front end can compile it
before it ever reaches hardware, which `tools/check_shaders.sh` now does.

Two failure modes were built into that script rather than discovered later. It
refuses to run when no validator is present instead of passing, following
`check_tla.sh`, which requires its pinned jar for the same reason. And it fails
when its search matches no shader sources at all -- a moved directory or a
renamed extension would otherwise report success having compiled nothing, which
is the shape of green that means least.

The move improved the tests it touched. They had greped the Rust file that held
both shaders and asserted `count("uniform float source_is_opaque;") == 2`, a
count standing in for "both programs declare it". Against separate files each
program is asserted directly, so a change that gave one shader the uniform twice
while the other lost it now fails, and the message names which file. All three
negative controls from the previous commit still fail correctly through the new
indirection, which is the point of running them again rather than assuming a
pure move cannot break a test.

The shader text was checked byte-for-byte identical across the move before
anything else ran. A refactor that also changes behaviour is two changes wearing
one commit message.

## The correction, measured, and the region that could not be read

The physical result first. On the two regions where the two runs put comparable
content on screen -- lit populations of 8882 against 8884, and 4722 against 4724,
which is the same frame to within two pixels -- the edge centroid rose from 8494
to 9079 and from 7333 to 8410, seven and fifteen percent. The histogram shape is
the one predicted before the change: the dim buckets halve, the bright ones gain
by a factor of three, and the top bucket is populated for the first time at all.
The controls held exactly: the `exact_nearest` draw stayed at 12000 and the
uniform solid bands at 10000, both to the digit. Both gate modes passed with
`linear_fallback_draws=0`, so the shader was genuinely running, in both
directions.

The part worth recording is that the first region I read said the opposite, and
said it convincingly. Records one through twelve of the 1920x1080 region were
byte-identical between the runs -- not close, identical -- which reads as a
correction that never reached the draw.

It was the measurement. The composition-region trace carries a rect and no
output identity, and this topology has two heads composing a `1920x1080_0_0`
rect: the extended head at `mapping=exact` and the mirror member at
`mapping=fit`. One is filtered and one is not, their records are
indistinguishable, and the collector's last-record-wins kept whichever came
last. The identical histograms were the unfiltered head being compared with
itself, which is exactly what should happen and proves nothing about the change.

I had found this in the reading before any of it was written -- the note said the
region line carries no `head=` or `output=` field -- and then designed a
comparison keyed on the rect anyway. It is the third instance this session of one
record lacking the fact needed to read it, after the raster that could not say
what extent it spanned and the counter that could not say whether a draw was
degraded or enlarged.

Two smaller things the run corrected. The evidence pair was not comparable to
begin with: the second run emitted twelve region records to the first's nineteen
and never reached the frame with the full directory listing, so the headline
region was comparing unlike content on top of everything else. And the solid
bands are not the control I called them -- the trace reads the final framebuffer
at that rect, not the clear that was written there, so anything composed nearby
contributes. The controls that did hold, and that were worth having, were the
exact-sampled draw and the uniform bands.

The conclusion stands on the regions whose geometry only one head has. That is
luck, not method, and the method is the follow-up.

## One rect doing two jobs, found by giving it a second job

Centre-unscaled landed and the scaling was right. The window borders were not:
bands 1440 tall in a 1080-tall logical space, bands at x=4160 on a 2560-wide
framebuffer, and the same border appearing twice at coordinates 320 apart --
2238 against 1918, which is exactly the letterbox offset.

The cause is older than the policy. `project_border` never clipped at all, while
surfaces got a `native_clip` and the cursor got a `clip_to_target`. That much was
a plain omission. The deeper half is that the two things which *were* clipped
were clipped to `target.native_size`, the whole framebuffer, rather than to the
region the scene occupies on it. Every policy until this one projected the scene
across the entire head, so those were the same rectangle and nothing could tell
them apart. Putting a smaller scene inside a larger head separated them for the
first time, and content bounded by the framebuffer began painting into the margin
that exists to hold background alone.

So the border bug was three bugs wearing one symptom: borders unclipped,
surfaces clipped to the wrong bound, and the cursor likewise. Each was confirmed
load-bearing by reverting it alone and watching the test fail.

The part I got wrong and had to correct: bands must be clipped individually
rather than by clipping the `outer` and `inner` rects they are subtracted from,
and I wrote a comment asserting that clipping those first would invent a band
along the clip edge -- a border down the side of a window merely running off the
screen. Enumerating the cases showed that does not happen. Both rects clip to the
same boundary, so the subtraction goes to zero and the band simply vanishes,
which is correct. What actually goes wrong is the opposite: a window lying
entirely outside the scene clips to two degenerate rects whose difference is
still positive, and the band survives at its original off-screen coordinates. The
approach is wrong, the reason I gave was not, and a test written against the
reason I gave passed with the wrong approach in place.

That is the second time this session a test agreed with a claim without
testing it. The first was a mirror of a shader standing in for the shader. This
one was a case that could not distinguish the two implementations, and the way I
found it was by running the wrong implementation against the test rather than by
reasoning about whether it would pass. The case that does distinguish them is in
the test now, and both wrong approaches fail it.

Worth noting what the new policy actually did here: it did not introduce these
defects, it made them observable. A latent conflation stays latent while nothing
in the system can produce the input that separates the two meanings.
