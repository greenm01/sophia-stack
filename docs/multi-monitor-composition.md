# Multi-Monitor Per-Head Composition

**Role:** normative target architecture for composing logical visual state onto
physical display heads.

[Architecture](architecture.md) defines Sophia's authority boundaries.
[Engine Architecture](engine-architecture.md) owns the ordered visual path.
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
connector, CRTC, card, mode-object, framebuffer, or render-target identity.

Mirroring is semantic rather than framebuffer identity:

- every head presents the same logical scene generation, surface set, stacking,
  geometry, focus, and compositor state;
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
caches may be shared when their ownership permits it; final head targets may
not be shared by the initial architecture.

### Live backend

The backend owns physical connector/card identity, native mode capability,
render-device transport, framebuffer creation, KMS requests, callbacks, and
resource retirement. It projects those facts upward through an opaque head
identity and reduced target capabilities. It does not choose scene content,
variant policy, layout, or logical retirement requirements.

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
- authority-owned readiness and damage identity.

The set is immutable after admission. Supplying a replacement or additional
variant creates a new authority transaction and generation; it cannot mutate a
submitted frame. The implementation bound is the live physical-head capacity,
currently sixteen, rather than an independently unbounded variant list.

### `SurfaceContentVariant`

Describes one authority-asserted realization of the content:

- stable variant identity within the content generation;
- protocol-neutral `BufferSource`;
- pixel size, logical extent, scale/density, and source transform;
- readiness/synchronization state; and
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
It carries the exact head-plan identity, target descriptor, and affine lease
token it realizes. Renderer/backend-private state behind that token owns the
native target and lease until KMS submission transfers the applicable
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
  reducers, and `OutputPresentationCohort`.
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
                       │              (advisory)
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
[Compositor Graphics](compositor-graphics.md); failure to produce mandatory
content fails the candidate.

### Client content selection

Variant choice is a pure bounded reducer. It applies this total preference:

1. exact target density, extent, and compatible transform;
2. a compatible variant that avoids upscaling, with the least excess density;
3. the compatible remaining variant with the least scale error; then
4. stable variant identity as the tie-breaker.

The resulting binding records `Exact`, `Downsampled`, or `Upsampled`. Sampling
is applied to that client layer while the rest of the scene remains native; a
whole output is never resampled to compensate for one missing client variant.
Sampling filters and their failure policy remain renderer capabilities, but the
quality class and fallback occurrence are reduced Engine-visible evidence.

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
surface/content generations. Per-head checksums, damaged-pixel counts, and
framebuffer bytes are expected to differ and must not be used as the mirror
join identity.

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

All required heads of a mirror group must prepare successfully before its first
KMS submission. Preparation includes rendering/export, framebuffer creation,
lease acquisition, and every validation possible without accepting the real
page flip. It does not pretend the later kernel commit cannot fail. Submission
remains per head, matching Sophia's established X-style KMS design. Once any
submission is accepted, a later failure poisons the cohort: already accepted
work drains, no successor can be relabelled into the failed generation, and the
logical candidate does not commit.

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
outputs. A transaction affecting several logical outputs may prepare them as
one immutable candidate, while their KMS retirements remain separate physical
instants. Engine publishes transaction feedback and cross-output input state
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
cohort. The candidate fails closed, accepted head submissions drain, and a
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
- exact/downsampled/upscaled client-binding counts;
- independently rasterized compositor-primitive counts;
- head-local repaint mode and damage; and
- the final logical output join.

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

Before production scheduling changes, the bounded visual-retirement model must
include per-head preparation and distinct target leases. Negative controls must
show that submitting before all required heads prepare, retiring after one head,
or releasing one head's target while it remains in flight violates the model.

## Current And Target State

### Implemented

- Mirroring is one logical output backed by multiple physical heads and remains
  invisible to WM policy.
- Each current mirror head owns a native-size buffer, KMS submission, callback,
  and resource lifetime; logical retirement joins the callbacks.
- CPU, mixed CPU/DMA-BUF, retained renderer images, compositor solids, cursor
  coordinates, and damage snapshots fan out to the heads.
- The current renderer can apply exact, reduction, and enlargement sampling and
  reports the selected path.

### Transitional limitation

The production CPU scene is still composed at one output size. Mirror queueing
then projects that flat frame, or projects already assembled mixed layers, into
each head. That preserves native scanout ownership and logical retirement but
does not provide per-head scene lowering or native client-content selection.
The distinguished primary output also remains embedded in composition and
retained-scene paths. Sampling improvements can reduce artifacts, but they do
not satisfy this target architecture.

### Target

- Split logical scene snapshots from physical render targets.
- Carry bounded surface content variants through the authority transaction and
  committed-state lifecycle.
- Derive per-head plans, damage ledgers, native targets, and compositor raster
  work before rendering.
- Replace all primary-derived and flat-output mirror composition paths with the
  common per-head planner used by mirrored and extended outputs.
- Add authority-owned native variants for server-rendered X11 core content
  without moving X semantics into Engine.
- Enforce prepare-all mirror cohorts, joined retirement, and explicit sampling
  evidence in deterministic and physical acceptance gates.

## Review Notes

**Role:** appended analysis of this document against the docs corpus, the
checked models, and the code at `29572b21` (2026-08-15). The text above is
unchanged; nothing here is normative until folded back into the sections it
annotates.

### Corrections To This Document

1. **The variant bound cites a constant that does not exist** (`SurfaceContentSet`,
   "the live physical-head capacity, currently sixteen"). Sixteen is the
   logical-output table bound — `LIVE_RENDERED_OUTPUT_CAPACITY`
   (`crates/sophia-backend-live/src/runtime/output_state.rs:5`),
   `MAX_DRM_KMS_OUTPUTS` (`crates/sophia-engine/src/drm.rs:4`),
   `DESKTOP_OUTPUT_MAX_NAMED` (`crates/sophia-config/src/output_candidate.rs:10`)
   — reused as a connector cap during selection. No per-head constant exists.
   Separately, bounding a per-surface variant list by head count is a
   non-sequitur: variants are density/transform classes, and sixteen heads do
   not imply sixteen densities. [Data-Oriented Design](dod.md)'s "bounded
   ordered content variants" is the defensible form; if a number is wanted it
   needs its own named constant and its own justification.

2. **The model-gap sentence overstates what is missing.** "the bounded
   visual-retirement model must include per-head preparation and distinct
   target leases" — distinct leases and retire-after-one-head are already
   modelled: `inFlight` is head-scoped
   (`validation/tla/VisualRetirement.tla:274`), `RetiredOutputs` requires every
   head (`:98`), and `LoseHead` fails closed (`:183-196`). [Engine
   Architecture](engine-architecture.md) says so explicitly. What is genuinely
   absent is **per-head preparation**: `Prepare(g)` is generation-scoped
   (`:125`) and `Submit(g, output)` is output-scoped (`:134`), so "submitting
   before all required heads prepare" and a partial submit *within* a group are
   both inexpressible today. The sentence should name only the real gap, and
   cite `validation/tla/VisualRetirement.tla` by path.

3. **Engine already holds raw physical identity, and this document writes as if
   it never did.** `DrmKmsOutputDescriptor { connector_id, crtc_id }` lives in
   `crates/sophia-engine/src/drm.rs:22-27`, is fed real hardware ids by the
   backend (`production_session/native_scanout.rs:263-277`), and drives Engine
   frame clocks. The Ownership section's claims — Engine "does not own …
   connector naming", the backend "projects those facts upward through an
   opaque head identity" — describe a boundary the code predates. This belongs
   in the Target list as an explicit migration (introduce `RenderHeadId`,
   remove connector/CRTC ids from the Engine registry), not as an assumed
   invariant. `sophia-protocol` is clean. A seed exists:
   `LibdrmNativeOutputSlot` (`drm/native_page_flip/route.rs:7-25`), though its
   route record currently travels alongside the raw `connector_id`.

4. **The shipped completion gate contradicts the checksum rule stated here.**
   This document is right that per-head checksums are expected to differ and
   must not be the mirror join identity. The runtime join complies (frame id
   plus required-connector set,
   `persistent_native_scanout/state.rs:242-290`). But
   `validate_native_output_checksums`
   (`crates/sophia-cli/src/native_output_completion.rs:31-40`) and
   `tools/verify_mirror_group_physical.sh` assert per-head checksum
   **equality** — which passes only because every head is stamped with the same
   source-frame checksum (`renderer_images.rs:303`). The moment per-head scene
   lowering lands, that gate fails by design. Migrating the completion identity
   to scene generation plus ordered content generations is a **prerequisite**
   of the target architecture and should be listed as one.

5. **One phenomenon, four vocabularies.** `Exact`/`Downsampled`/`Upsampled`
   (Client content selection), "upscaled" (Observability), "reduction and
   enlargement" (Implemented), and an implicit fallback class ("renderer
   fallback reported as exact"). The code has a fourth class in earnest:
   `sharp_downscale_fallbacks` beside exact/downscale/upscale counters
   (`crates/sophia-renderer-native-egl/src/gl.rs:557-564`). Pick the typed
   triple, name `Fallback` explicitly, and use them everywhere.

6. **The Decision section states a transient as an invariant.** "every head
   presents the same logical scene generation" is contradicted four sections
   later by the (correct) allowance that faster heads keep scanning their
   previous buffer while the group advances at the slowest head. The precise
   form already exists in the same list — "the logical output retires only
   after every required head retires that scene generation" — so the first
   bullet should be qualified to retirement rather than presentation.

7. **The degradation-contract citation claims more than its source states.**
   [Compositor Graphics](compositor-graphics.md)'s Degradation Contract says
   mandatory content "cannot disappear **silently**" — a disclosure rule; this
   document upgrades it to "failure to produce mandatory content **fails the
   candidate**" — a frame-failure rule — and adds a per-head dimension the
   source does not carry. The source is also itself labelled Target, and a
   second, competing degradation statement exists in the same document. Either
   state the frame-failure rule here as this document's own normative addition,
   or amend Compositor Graphics; do not leave it as a citation.

8. **The blind-WM list omits the three facts the freeze contract is most
   emphatic about.** Scale, transform, and mode never cross to policy
   ([Sophia Policy IPC](sophia-policy-ipc.md): "Transform never crosses");
   this document forbids only physical-object identities. Since
   `OutputSceneSnapshot` carries an "output transform policy" and the head plan
   carries a transform, the likely misreading is that transform is
   policy-visible. Add scale, transform, and mode to the never-crosses list.

9. **Stale sentence in a sibling document resolves this conflict the wrong
   way.** [WM v1 Freeze Surface](wm-v1-freeze-surface.md) still says mirroring
   is "same-mode-only, because no plane scaling exists anywhere in the tree —
   mismatched modes must fail closed at reconcile time." The code, the physical
   gate, and this document all admit unequal-mode groups. The freeze doc's
   sentence is the stale one and needs an amendment note; a reader reconciling
   the two today has no way to know which side won.

10. **Smaller precision items.** "ordered visual path" names no section —
    [Engine Architecture](engine-architecture.md)'s is "Atomic Visual Path".
    "Sophia's established X-style KMS design" cites nothing; the nearest
    normative text is [Architecture](architecture.md)'s per-output KMS/worker
    section. `HeadFrameCandidate` gets a record heading but no crate in the
    module-boundary table. The data-flow diagram shows the advisory
    `SurfaceRasterRequirements` edge but not the return path (a later
    `SurfaceContentSet` a generation on), which the staleness rule depends on.
    "final head targets may not be shared **by the initial architecture**"
    reads as transitional; [Architecture](architecture.md)'s AMDGPU evidence
    suggests it is closer to permanent — cite it. The "Transitional
    limitation" heading breaks this document's own Title Case convention.

### Divergences From The Checked Models

11. **Output-scoped versus global generation clock — the deepest divergence,
    settle it in the model first.** This document requires an output-scoped
    latest-wins successor and outputs that "do not join merely because they
    share a session." `VisualRetirement.tla` has one global `committed` scalar
    and decides supersession by `g > committed` (`:162`), with
    `CommittedGenerationDominatesHistory` and `InputMatchesCommitted` baking
    the global order in. Reachable counterexample under the checked config: a
    generation that fully retires on an unrelated output after a numerically
    later generation committed elsewhere is labelled `superseded` and denied
    feedback — precisely what the Extended Desktops section forbids. Adopting
    this document's clock means re-scoping those invariants; per the project's
    model-first habit, that happens before the scheduler is written.

12. **The model currently admits what this document forbids.** Nothing stops
    the same head sitting in `inFlight` of two generations at once — `Submit`
    has no cross-generation guard — so "advance at the slowest head" is
    violated by an admitted trace. The fix is cheap and doubles as the negative
    control: guard `Submit` on the head being free in every generation, and add
    `inFlight[g1] ∩ inFlight[g2] = {}` as an invariant.

13. **Two prepare-all scopes are stated and they are different guards.** The
    Scheduling pipeline orders prepare-all cohort-wide; the mirror-group
    sentence scopes it per output. `∀ o ∈ required: HeadsOf(o) ⊆ prepared` and
    `HeadsOf(output) ⊆ prepared` are different preconditions. Pick one before
    modelling; the per-output reading matches "unrelated outputs retain
    independent frame clocks".

14. **Head loss and surface removal share one terminal outcome.** `LoseHead`
    settles as `"removed"`, which the model README maps to surface removal.
    The Observability section wants a missing head distinguishable in reduced
    evidence; give head loss its own outcome.

### Accuracy Of Current And Target State (Verified At `29572b21`)

15. The Implemented list is accurate, verified claim by claim: per-head
    exporters, submissions, callbacks, and lifetimes with a joined logical
    retirement (`GroupReady` in `persistent_native_scanout/state.rs:264-296`);
    CPU, mixed, retained-image, solid, cursor, and damage fan-out — damage
    **is** projected per head (`renderer_images.rs:438-456`), and cursor
    coordinates **are** scaled per head (`persistent_native_scanout/cursor.rs:53-68`);
    sampling **is** reported (`sophia_native_composition_sampling` log,
    counters, and gate assertions), with two caveats worth recording: the log
    line is deduplicated per class rather than per frame, and the record
    carries no head identity. The Transitional limitation is likewise accurate:
    one-size CPU scene, secondaries receiving clamped marker frames rather
    than scenes (`production_cpu_scene.rs:500-545`), primary embedded at a
    dozen call sites.

16. Prepare-all is correctly listed as Target only: the shipped loop submits
    each head the moment its own export lands and defers siblings
    (`native_scanout.rs:1261-1279`) — the first physical run showed exactly
    this, one connector submitting 8ms before its sibling's export finished.

### Recommended Improvements

17. **Add a reachability clause to Acceptance.** Three components this cycle
    were built correctly, fully tested, and wired to nothing (the mirror
    grouping, the gate's profile source, the metadata broker). The acceptance
    list rejects many wrong states but not "record exists with no production
    producer or consumer." Require each target record to land with a named
    caller in a running session, and its absence to be a review-blocking fact.

18. **Name the evidence conventions this architecture depends on.** Two
    session-proven rules belong in Observability: every fail-closed rejection
    names its subject (the silent `InvalidSurface` raiser cost three hardware
    round-trips before it carried a surface id), and both views of a scene are
    taken from one read (`29572b21` fixed the class; `OutputSceneSnapshot` as
    specified makes it unrepresentable, which is an argument **for** this
    document worth stating).

19. **Resolve the fit-policy ownership now, not at migration time.** The
    fit/cover/exact vocabulary already ships as operator configuration
    (`DesktopMirrorFit`, `output_candidate.rs:51`) executed in the backend
    (`NativeMirrorFit`, `drm/native_kms/mirror_projection.rs:22-42`), with a
    manual enum mapping in `live_session.rs`. That is the backend "choosing
    layout" the Ownership section forbids. Either the Transitional limitation
    names it, or the plan-derivation move to Engine is listed as the migration
    that unifies the two enums. Also: `configuration.md` does not document
    `mirror-fit` at all — an operator-facing option is undocumented.

20. **Carry a terminology bridge.** None of the target record names exist in
    code, which is fine and labelled — but readers need the mapping to find
    what does exist: logical output ↔ `OutputId` / policy-facing
    `SnapshotOutput`; physical head ↔ connector (today a raw `u32`; slot seed
    `LibdrmNativeOutputSlot`); `OutputPresentationCohort` ↔
    `LiveProductionMirrorGroupLifecycle` + `NativeMirrorGrouping`;
    fit policies ↔ `DesktopMirrorFit`/`NativeMirrorFit`; sampling classes ↔
    the four `gl.rs` counters. One table prevents a parallel vocabulary
    hardening into a parallel implementation.

21. **State the combined startup rule.** Prepare-all (this document) plus
    "native KMS initialization waits for the first committed-state frame"
    ([Renderer Import Boundary](renderer-import-boundary.md)) together mean a
    mirror group's first modeset can only happen after every required head has
    rendered a real committed frame. Neither document says this outright, the
    cohort states carry no modeset arm, and the current bootstrap
    (`sophia_live_mirror_bootstrap`) predates the rule. Spell it out in
    Scheduling.

22. **Rounding rule migration hazard.** The existing `project_mirror_rect`
    scales extents and centres — correct for today's single whole-frame rect,
    and exactly the "widths derived before edges" pattern the Logical-to-head
    mapping section forbids for per-layer plans. When the per-head planner
    projects individual layers, it must switch to edge projection or adjacent
    surfaces will seam; worth a sentence in the mapping section so the
    existing helper is not lifted as-is.
