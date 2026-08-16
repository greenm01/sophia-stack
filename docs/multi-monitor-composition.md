# Multi-Monitor Per-Head Composition

**Role:** normative target architecture for composing logical visual state onto
physical display heads.

[Architecture](architecture.md) defines Sophia's authority boundaries.
[Engine Architecture](engine-architecture.md)'s Atomic Visual Path owns visual
ordering.
[Compositor Graphics](compositor-graphics.md) defines renderer-neutral
compositor content. [Data-Oriented Design](dod.md) defines the passive records
that cross these boundaries, and [Renderer Import
Boundary](renderer-import-boundary.md) defines native client-buffer admission.
This document specializes those contracts for extended desktops, mirror groups,
and other logical-output topologies with more than one physical display head.

Normative target behavior that is not implemented is labelled as such. The
current mirror path remains useful physical evidence, but composing one logical
frame and then scaling that flattened result is not the target architecture.

## Decision

Sophia has one authoritative logical scene and a distinct native composition
for every physical head.

A logical output is an Engine and policy fact. A physical head is a presentation
target below the policy boundary. One logical output normally has one head and
may have several when mirrored. Several logical outputs form an extended
desktop. The WM continues to see only logical outputs; it never receives
physical mode, scale, transform, connector, CRTC, card, mode-object,
framebuffer, render-target, or head-count identity.

Mirroring is semantic rather than framebuffer identity:

- every successfully retired mirror cohort contains the same logical scene
  generation, surface set, stacking, geometry, focus, and compositor state on
  every required head; transiently, a faster head may still scan the prior
  cohort while a sibling finishes;
- each head receives its own native-size render target, composition plan,
  framebuffer, KMS ownership, and page-flip observation;
- Engine-owned graphics are rasterized for the head rather than scaled from a
  previously flattened output;
- client raster content uses the best ready authority-owned variant for the
  head, with explicit resampling when no matching variant exists; and
- the logical output retires only after every required head retires that scene
  generation.

This contract does not promise that arbitrary client pixels can be recreated at
another resolution. A raster buffer contains pixels, not the semantic operations
that produced them. Native-sharp client content therefore requires another
ready content variant from its owning authority. Sophia must report a fallback
as resampled rather than disguising it as per-head rasterization.

## Ownership

### Sophia Engine

Engine owns:

- the immutable logical scene generation;
- logical-output views, surface placement, stacking, focus, and hit-testing;
- derivation of one renderer-neutral composition plan per required head;
- deterministic selection of a surface content variant for each head plan;
- head-local damage planning and the atomic relationship between client
  content, compositor graphics, cursor state, and submission;
- the required-head set for each logical output generation; and
- promotion of visual state, input geometry, and protocol feedback after the
  applicable retirement set completes.

Engine does not own X11 drawing requests, client font semantics, renderer
textures, GBM objects, DRM objects, or connector naming.

### Protocol authorities

A protocol authority owns the client-visible resource and rendering semantics
that can produce surface pixels. It emits a bounded, protocol-neutral content
set for one surface generation. The active X Server Frontend may produce
multiple variants for server-rendered core content. It does not expose XIDs,
drawing requests, fonts, GCs, or pixmap object graphs to Engine.

X Authority retains one canonical protocol-visible drawable. Derived raster
variants are authority-private caches, not additional X resources: they cannot
change `GetImage`, event ordering, GC behavior, drawable lifetime, or any other
client-visible X11 result. If an accepted operation cannot be reproduced
faithfully for a derived variant, the authority invalidates that variant and
publishes the canonical raster alone.

An authority that has only one client buffer emits one variant. Absence of an
exact variant does not authorize Engine to infer or replay protocol operations.

### Native renderer

The renderer owns:

- per-head render-target pools and complete native target slots;
- texture import, immutable source sharing, atlases, and raster caches;
- lowering a head composition plan into its native target;
- head-specific sampling, color conversion, clipping, and compositor-primitive
  rasterization; and
- reduced import, sampling-quality, cache, and failure observations.

Renderer-private handles remain below the Engine boundary. Source textures and
caches may be shared when their ownership permits it; final head targets and
their scanout leases are permanently head-exclusive. This preserves the
complete-target lifetime rule in [Architecture](architecture.md) and avoids the
AMDGPU lifetime failures already observed when incomplete native owners were
shared or reused across presentation lanes.

### Live backend

The target backend owns physical connector/card identity, native mode capability,
render-device transport, framebuffer creation, KMS requests, callbacks, and
resource retirement. It projects those facts upward through an opaque head
identity and reduced target capabilities. It does not choose scene content,
variant policy, layout, or logical retirement requirements.

This boundary is implemented for head identity: the backend mints
`RenderHeadId` when it builds page-flip sessions, retains the
card/connector/CRTC/name mapping privately, and Engine records carry only
reduced `HeadRenderTarget` facts. No physical identity enters
`sophia-protocol` or the WM wire.

## Passive Target Records

The names below define the target data shape. They are passive records, not
objects with callbacks or hidden authority.

### `RenderHeadId`

A session-scoped opaque typed ID for one physical presentation target. The
backend retains the card, connector, CRTC, and plane mapping. `RenderHeadId`
may cross the Engine/backend boundary for planning and observations, but it
does not cross the WM, portal, metadata, shell-policy, or application-protocol
boundary.

### `HeadRenderTarget`

Describes one current native target:

- `head: RenderHeadId`;
- owning `OutputId`;
- target/topology generation;
- native pixel size and transform;
- refresh and reduced presentation capabilities; and
- renderer/KMS transport and format capability identities in reduced form.

A mode, transform, device, or capability change creates a new target
generation. Work prepared for an older target is stale and cannot be relabelled.

### `SurfaceContentSet`

Describes the bounded raster content admitted for one committed surface
generation:

- surface and committed content generation;
- logical content extent;
- one or more `SurfaceContentVariant` records; and
- authority-owned ready variant and damage identity.

The set is immutable after admission. Supplying a replacement or additional
variant creates a new authority transaction and generation; it cannot mutate a
submitted frame. The list is bounded and deduplicated by density/transform
class. Its implementation must introduce a dedicated named capacity alongside
the record; logical-output and connector-table capacities are not that bound.

### `SurfaceContentVariant`

Describes one authority-asserted realization of the content:

- stable variant identity within the content generation;
- protocol-neutral `BufferSource`;
- pixel size, logical extent, scale/density, and source transform;
- ready-only publication plus synchronization identity inherited from the
  committing authority transaction; and
- variant damage.

All variants in a set belong to the same surface, namespace, transaction, and
content generation. Engine validates those identities and bounds but cannot
prove that opaque pixels are semantically equal; that assertion remains the
protocol authority's responsibility.

Existing single `BufferSource` transactions normalize into a one-variant set.
This preserves current frontend compatibility while making degradation
explicit.

### `SurfaceRasterRequirements`

Engine may publish a bounded, protocol-neutral request describing useful raster
classes for one surface:

- surface and committed-content generation;
- requirement/topology generation;
- logical content extent; and
- ordered target density and transform classes.

The record contains no output, head, connector, mode-object, or framebuffer
identity. It is advisory: the owning authority may answer later with a new
`SurfaceContentSet`, while Engine continues using the best ready variant. A
response for an older content, layout, or requirement generation is rejected as
stale. The renderer cannot issue this request directly or block a frame while
waiting for it.

### `OutputSceneSnapshot`

Captures one immutable logical-output view of the authoritative scene:

- `OutputId` and scene generation;
- logical viewport and output transform policy;
- ordered surface generations, immutable content-set identities, geometry,
  clips, and opacity;
- Engine-owned compositor display-list nodes;
- software-cursor state; and
- logical damage.

It contains no physical target size, framebuffer, renderer handle, or selected
content variant. It is the fan-out point shared by all heads of the output.

### `HeadCompositionPlan`

Lowers one `OutputSceneSnapshot` for one `HeadRenderTarget`:

- scene, output, head, and target generations;
- the exact rational logical-to-head transform;
- ordered `HeadLayerBinding` values naming selected content variants and their
  sampling class;
- native placements, clips, compositor primitives, and cursor state; and
- head-local repaint work.

The plan is complete and immutable. A renderer must not consult current scene
state, a primary head, or a newer variant while executing it.

### `HeadFrameCandidate` and `OutputPresentationCohort`

`HeadFrameCandidate` is the reduced descriptor for one prepared native result.
The passive identity and outcome cross through `sophia-engine`; the native owner
behind them remains in `sophia-renderer-live` and the applicable native renderer.
The descriptor carries the exact head-plan identity, target descriptor, and
affine lease token it realizes. Renderer/backend-private state behind that token
owns the native target and lease until KMS submission transfers the applicable
ownership; native handles do not enter the passive Engine record.

`OutputPresentationCohort` is the logical join record:

- output and scene generation;
- fixed required-head set;
- prepared, submitted, flipped, lost, and cleanup state by head; and
- terminal outcome and logical timing evidence.

It does not own native buffers directly. Renderer candidates and backend
submissions own their private resources; the cohort decides when their common
logical generation may be reported as presented.

## Target Module Boundaries

The record ownership maps onto the existing crates rather than creating a
parallel multi-monitor subsystem:

- `sophia-protocol` carries `SurfaceContentSet`,
  `SurfaceContentVariant`, and `SurfaceRasterRequirements`, because those
  passive values cross the protocol-authority/Engine boundary.
- `sophia-engine` owns `RenderHeadId`, `HeadRenderTarget`,
  `OutputSceneSnapshot`, `HeadCompositionPlan`, pure variant/geometry/damage
  reducers, the passive `HeadFrameCandidate` identity/outcome, and
  `OutputPresentationCohort`.
- `sophia-renderer-live` and its native implementations own target pools,
  imports, caches, lowering, and the native owner behind a
  `HeadFrameCandidate`; they return only reduced identities and outcomes.
- `sophia-backend-live` owns the physical head table, renderer/export workers,
  KMS submissions, callback routing, cleanup, and translation between opaque
  head IDs and card-local native objects.

The production session coordinator orders these owners through the existing
runtime driver. It must not add a second scene, presentation scheduler, or
mirror-only composition authority.

## Ordered Data Flow

```text
protocol transactions          backend topology observations
        │                                  │
        ▼                                  ▼
SurfaceContentSet       logical output ↔ HeadRenderTarget set
        │                                  │
        └──────────────┬───────────────────┘
                       ▼
             immutable Engine visual state
                       ├──── SurfaceRasterRequirements ────► authority
                       │              (advisory)               │
                       ◄──── later SurfaceContentSet generation┘
                       ▼
              OutputSceneSnapshot
                       │ fan out before rasterization
          ┌────────────┼─────────────┐
          ▼            ▼             ▼
    head plan A   head plan B   head plan C
          │            │             │
          ▼            ▼             ▼
    native target A native target B native target C
          │            │             │
          ▼            ▼             ▼
       KMS A          KMS B         KMS C
          │            │             │
          └────────────┼─────────────┘
                       ▼
       output/head retirement reduction
                       │
                       ▼
 committed visual state, input state, feedback, release
```

The fan-out point is load-bearing. Flattening `OutputSceneSnapshot` into pixels
before the split destroys the information needed for native compositor
rasterization, variant selection, head-local damage, and target-specific color
or transform work.

## Per-Head Composition Rules

### Logical-to-head mapping

One pure rational transform maps the logical output view to each head. Fit,
cover, and exact policies select that transform; they do not select a prebuilt
source framebuffer. The same transform implementation projects layer edges,
clips, damage, software cursors, hardware-cursor coordinates, and reverse input
coordinates. Rectangle edges are projected before widths are derived so
adjacent logical geometry cannot gain inconsistent seams through repeated
rounding.

Configuration owns the operator's fit, cover, or exact choice. Engine validates
and normalizes that choice and derives the transform; the backend executes the
immutable head plan. The migration may generalize the existing edge-based
`project_mirror_child_rect` reducer, but must not reuse whole-frame extent
projection for individual layers.

Bars introduced by fit are explicit Engine background in the head plan. Cover
and exact may clip content only through their named policy. A target-size or
mapping change invalidates the prior damage baseline and forces a full repaint.

### Compositor-owned content

Renderer-neutral display-list nodes remain semantic until the head plan is
lowered. Borders, fills, shadows, cached compositor text, images, and software
cursors are rasterized at the head's native density. Cache identity includes
the target density, transform, color-relevant capability generation, and every
existing content/style key.

Mandatory compositor content cannot disappear on one head. A supported reduced
fallback is allowed only under the degradation contract in
[Compositor Graphics](compositor-graphics.md). Failure to produce mandatory
content for any required head fails the complete candidate before submission.

### Client content selection

Variant choice is a pure bounded reducer. It applies this total preference:

1. exact target density, extent, and compatible transform;
2. a compatible variant that avoids upscaling, with the least excess density;
3. the compatible remaining variant with the least scale error; then
4. stable variant identity as the tie-breaker.

The requested and effective binding classes are each `Exact`, `Downsampled`, or
`Upsampled`. Sampling is applied to that client layer while the rest of the scene
remains native; a whole output is never resampled to compensate for one missing
client variant. `Active`, `Fallback`, and `Unavailable` are outcomes, not fourth
sampling classes. Sampling filters and failure policy remain renderer
capabilities, but requested class, effective class, and outcome are reduced
Engine-visible evidence.

Server-rendered X11 core content may gain native variants inside X Authority by
applying the accepted protocol semantics to authority-owned variant storage in
response to current raster requirements. This is a target capability, not
permission for Engine to ingest or replay X11 drawing commands.
Client-uploaded bitmaps and DRI3 content remain single-raster unless their
owning path supplies a genuine additional variant.

### Damage

Damage has two stages:

1. logical scene damage names changed surface generations, geometry, stacking,
   compositor nodes, and cursor state; and
2. each head plan projects that damage, adds old and new variant extents, and
   expands it by the selected sampling filter's footprint.

Every head owns its own `pending → rendering → submitted → presented` damage
baseline. Native sizes, selected variants, transforms, and color paths may
differ, so one head's damage snapshot cannot be cloned into another's ledger.
Incomplete damage proof becomes a full head repaint, never stale pixels.

Logical content identity is the scene generation plus its ordered committed
surface/content generations. A diagnostic logical-content checksum is equal
across the cohort, and the join refuses heads that disagree on it: one scene
composed once cannot yield two logical checksums. Optional native head-pixel
checksums, damaged-pixel counts, and framebuffer bytes are expected to differ
and must not be used as the mirror join identity.

## Scheduling and Retirement

For one presentation cohort, the order is:

```text
scene accepted
  → all required head plans built
  → all required head candidates prepared
  → per-head KMS submissions
  → per-head callbacks and cleanup
  → logical output retirement
```

All required heads of one logical-output cohort must prepare successfully before
that cohort's first KMS submission. This is a per-output barrier: unrelated
logical outputs retain independent preparation, submission, and frame-clock
progress. Preparation includes rendering/export, framebuffer creation, lease
acquisition, and every validation possible without accepting the real page flip.
It does not pretend the later kernel commit cannot fail. Submission remains per
head, matching the per-output KMS/worker ownership established in
[Engine Architecture](engine-architecture.md). Once any
submission is accepted, a later failure poisons the cohort: already accepted
work drains, no successor can be relabelled into the failed generation, and the
logical candidate does not commit.

Initial modesetting follows the same rule. Before the first modeset of a mirror
cohort, every required head has a prepared `HeadFrameCandidate` containing the
same real committed scene generation at that head's native target. A valid
direct-CPU lowering may satisfy this requirement; a blank, speculative, or
primary-scaled bootstrap cannot. The cohort records that its first submissions
are initial modesets rather than creating a separate startup lifecycle.

A mirror output owns one active generation and one output-scoped latest-wins
successor. It advances at the slowest required head. Faster heads may continue
scanning their previous buffer while waiting, but they cannot advance to a
newer logical generation independently. Asynchronous secondary advancement is
outside the initial contract.

Head preparation may execute concurrently through a bounded worker set. The
bound is derived from admitted live heads and target slots; the scheduler does
not spawn unbounded work per layer, surface, or frame. Backpressure preserves
the active cohort and coalesces only its complete latest successor.

Retirement remains joint within a mirror group and independent between logical
outputs. A transaction affecting several logical outputs may derive their
cohorts from one immutable candidate, while each cohort keeps its own
prepare-before-submit barrier and its KMS retirement remains a separate physical
instant. Engine publishes transaction feedback and cross-output input state
only after the applicable logical-output retirement set completes. Sophia does
not claim globally simultaneous multi-output presentation.

Page-flip timing remains physical evidence. The logical completion time is no
earlier than the last required head callback; the logical sequence is the scene
or cohort generation rather than a fabricated combination of CRTC-local
sequences.

## Extended Desktops and Spanning Surfaces

The same path serves an extended desktop. Each logical output derives its own
`OutputSceneSnapshot` from one authoritative Engine scene read, then lowers it
for every head backing that output. There is no rendering meaning attached to
"primary output"; primary selection may remain an operator/configuration
default but cannot decide scene size, buffer ownership, damage, or content for
another output.

A surface intersecting several logical output views is clipped independently
into each view and bound to the best content variant for each head. It retains
one committed surface generation. Geometry, focus, and feedback whose contract
spans those outputs advance only after their applicable retirements complete.
Unrelated outputs retain independent frame clocks and do not join merely because
they share a session or render device.

## Topology, Failure, and Resource Lifetime

The required-head set and every target generation are frozen when a cohort is
prepared. A mode change, transform change, render-device change, or head-set
change makes the cohort stale.

A lost mirror head never counts as a flip and never silently shrinks an active
cohort. It settles with a distinct `head_lost` outcome rather than the surface
removal outcome. The candidate fails closed, accepted head submissions drain, and a
surviving topology becomes a new Engine candidate. Losing an independent
output is isolated unless an in-flight transaction names that output in its
applicable retirement set.

Renderer target slots are keyed by head, target generation, size, format, and
normalized modifier/capability set. The complete target follows
`free → rendering → scanout leased → retiring → free`. Page-flip retirement or
an explicit controlled detach is the only authority that releases the scanout
lease.

Immutable source storage and imports may be shared across head plans without
copying CPU pixels per head. Those shared sources remain leased until the last
candidate that references them retires or reaches a controlled terminal
failure. Reference counts do not replace the explicit presentation lifecycle.

Cross-card composition obeys the same records. The renderer either imports a
complete source through the declared independent-file transport or uses the
admitted CPU fallback. It may not submit a renderer-local handle on another
card or degrade one mirror head silently.

## Observability and Acceptance

Reduced evidence must make the architecture distinguishable from output-level
scaling without exposing pixels or client metadata. A physical proof records,
for one common scene generation:

- one plan, render completion, KMS submission, callback, and retirement per
  required head;
- each head's native target size and target generation;
- requested and effective exact/downsampled/upsampled client-binding counts plus
  active/fallback/unavailable outcomes;
- independently rasterized compositor-primitive counts;
- head-local repaint mode and damage; and
- the final logical output join.

Every fail-closed rejection names the reduced output, head, surface, generation,
or target that failed; a subjectless renderer or lifecycle error is insufficient
production evidence. All `OutputSceneSnapshot` values for one transaction are
derived from one immutable Engine scene read rather than independently sampling
mutable state.

The evidence must reject a missing head, duplicate head, target-size mismatch,
source-sized damage on another head, whole-output resampling, partial logical
retirement, renderer fallback reported as exact, or resource ownership left
after teardown. Logs retain typed IDs and reduced counts only; client titles,
glyphs, pixels, paths, and renderer-native handles remain private.

Deterministic coverage must include:

- unequal-size mirror heads with one scene generation and distinct native
  targets;
- exact and missing client variants alongside native compositor text;
- CPU, DMA-BUF, renderer-image, solid, and cursor layers;
- a surface spanning differently scaled extended outputs;
- filter-footprint damage and per-head baseline divergence;
- callback reordering and one head flipping before its sibling;
- render failure before submission and KMS failure after a partial submit;
- head loss and topology-generation replacement; and
- proof that WM and protocol-facing records contain no physical head identity.

Acceptance also requires production reachability. Every new passive target
record lands with a named producer and consumer on the running session path, a
test that exercises that path, and a negative test that fails when either end is
unwired. A type or isolated reducer with no production caller is incomplete.

The bounded visual-retirement model in
[`validation/tla/VisualRetirement.tla`](../validation/tla/VisualRetirement.tla)
carries this contract: per-head preparation, output-scoped committed
generations, exclusive head ownership across generations, distinct head leases,
and last-head retirement. Supersession is output-scoped and happens before the
kernel, matching the latest-wins successor above -- a candidate loses an output
a newer generation already committed, and only an output with no submitted head
can be lost. Negative controls show that submitting before all required heads
prepare, retiring after one head, overlapping one head across generations or
splitting one output cohort across generations, submitting a generation the
output has already passed, relabelling a submitted cohort, or releasing one
head's target while it remains in flight each violates the model. Scheduling
changes extend that model before they reach production.

## Current-To-Target Terminology

The target records replace existing responsibilities; they do not create a
parallel multi-monitor subsystem.

| Architecture term | Current implementation seed | Target owner |
| --- | --- | --- |
| logical output | `OutputId`, policy-facing `SnapshotOutput` | Engine |
| physical render head | opaque `RenderHeadId` (implemented); backend head table maps native identity | opaque `RenderHeadId`; backend maps native identity |
| head target | native selection/exporter state | `HeadRenderTarget`; Engine sees reduced capability only |
| presentation cohort | `LiveProductionMirrorGroupLifecycle` plus `NativeMirrorGrouping` | `OutputPresentationCohort` in Engine |
| fit policy | `DesktopMirrorFit` mapped to `NativeMirrorFit` | configuration choice normalized by Engine |
| sampling | `NativeCompositionSampling` plus fallback counters | requested/effective class plus outcome evidence |
| logical checksum | source-frame checksum copied to every head | diagnostic `logical_content_checksum` |
| head pixel checksum | not currently produced | optional head-local evidence, never join identity |

## Current And Target State

### Implemented

- Mirroring is one logical output backed by multiple physical heads and remains
  invisible to WM policy.
- Each current mirror head owns a native-size buffer, KMS submission, callback,
  and resource lifetime; logical retirement joins the callbacks.
- CPU, mixed CPU/DMA-BUF, retained renderer images, compositor solids, cursor
  coordinates, and damage snapshots fan out to the heads.
- The current renderer can apply exact, downsampled, and upsampled sampling and
  reports requested/effective paths and fallback outcomes.
- Content crosses the authority boundary as a bounded `SurfaceContentSet` of
  density-deduplicated `SurfaceContentVariant` records with a dedicated named
  capacity; every current producer normalizes into a one-variant set and the
  committed state retains the whole set. Sets contain ready-only variants;
  per-variant damage, fidelity, and transform class are validated and consumed.
- Head identity is opaque end to end: the backend mints session-scoped
  `RenderHeadId`s, Engine's `EngineHeadRegistry` holds generation-stamped
  `HeadRenderTarget` records grouped by logical output, and mirror
  lifecycles, callback routing, and per-head evidence are head-keyed while
  connector/CRTC integers stay in the backend's private head table.
- `OutputSceneSnapshot`, `HeadCompositionPlan`, and the pure fit/cover/exact
  planner are implemented in Engine. The production CPU transaction builds
  every head plan from the exact committed slice, resolves the selected CPU
  variant, and lowers each plan into its own native-size mixed frame. The
  backend queues those frames by opaque head and rejects incomplete coverage,
  duplicate heads, checksum disagreement, or head-local damage with the wrong
  native extent.
- `OutputPresentationCohort` and `OutputTopologyTransaction` are implemented as
  Engine reducers. They enforce prepare-before-submit, joint flip/cleanup,
  fail-closed head loss, partial-apply rollback, and a first-presentation barrier
  before topology publication. The live multi-head scheduler now renders and
  creates each head's framebuffer/import/blob and atomic request as an affine
  prepared owner, records those candidates in `OutputPresentationCohort`, and
  performs no KMS submit until the complete required-head set is prepared.
  Preparation failure cancels every prepared owner before the generation is
  poisoned; shutdown cancels prepared owners before callback-only drain.
- The exclusive `sophia_output_v1` Rust wire and authenticated transport exist
  with bounded capability snapshots and complete topology proposals. Backend
  projection binds capabilities to opaque heads and resolves independently
  selected modes into mixed mirror and extended groups. A session-side candidate
  owner now joins that projection to `OutputTopologyTransaction`: validation is
  nonmutating, fresh identities remain provisional through preparation and
  rollback, and the replacement snapshot stays private until every new logical
  output presents. Live-session supervision, physical effect execution, and
  generated language-neutral conformance remain cutover work.
- The output transport also has an optional cancellable service loop and
  incrementally buffered proposal intake. No-client startup and shutdown are
  bounded, and a client may pause between frame header and payload without
  blocking the visual owner. A native live session now binds that service in
  the selected public WM's private policy directory, authorizes the exact
  supervised PID, advertises it through `SOPHIA_OUTPUT_SOCKET`, and replaces
  the assignee and connection epoch on a supervised restart. Complete
  proposals reach the session-side authority owner. Validate-only proposals
  settle normally. Apply proposals now cross into the visual/session owner as
  immutable effect contracts; that owner binds every enabled and disabled head
  to its existing card/connector/CRTC/plane, resolves each requested mode
  against the live DRM master, and retains per-head target generations without
  mutating the published topology. It still rejects at target preparation until
  replacement renderer/KMS and rollback ownership are wired.
- Resolved output candidates retain root-space logical viewports as well as
  `HeadlessOutput` extents. The visual runtime can therefore capture one
  committed root scene and independently lower every provisional extended or
  mirrored head at native size. Topology-specific renderer preparation creates
  framebuffer/import/mode-blob owners without committing; those owners can join
  one card-scoped atomic modeset containing both enabled and explicitly disabled
  heads, then become ordinary retirement owners only after kernel acceptance.
  The card executor submits that complete change as one blocking modeset, and a
  passive coordinator orders cards deterministically. If a later card refuses
  the candidate, the accepted prefix rolls back in reverse card order. The
  published snapshot is projected independently back into the live heads'
  current native sizes and generations, so rollback composition cannot inherit
  a provisional viewport or mode.

### Transitional Limitation

CPU-authority frames now consume per-head plans in production. The old flat CPU
frame remains only as the synchronous startup modeset baseline; the following
cohort is independently composed. DMA-BUF Present and retained renderer-image
paths still assemble a primary-oriented mixed frame and project it per head,
because their affine image leases do not yet have the per-head source resolver
needed by the common lowerer. They fail closed if routed through the CPU-only
lowerer rather than disappearing or being reported as native.

The native scheduler now has the prepare-all barrier, and live topology planning
creates replacement selections for enabled heads while representing omitted
connected heads as explicit disable effects. Candidate CPU frames and native
resource owners can now be prepared, and the per-card apply request is defined,
and the cross-card apply/rollback order is explicit. The live owner does not yet
schedule the candidate-plus-rollback exporter pools or install/reconcile their
accepted owners. The distinguished primary output also remains
in retained scene and Present paths.
Configuration currently maps `DesktopMirrorFit` manually to backend-owned
`NativeMirrorFit`.

### Target

- Split logical scene snapshots from physical render targets.
- Derive per-head plans, damage ledgers, native targets, and compositor raster
  work before rendering.
- Replace all primary-derived and flat-output mirror composition paths with the
  common per-head planner used by mirrored and extended outputs.
- Add authority-owned native variants for server-rendered X11 core content
  without moving X semantics into Engine.
- Enforce prepare-all mirror cohorts, joined retirement, and explicit sampling
  evidence in deterministic and physical acceptance gates.
