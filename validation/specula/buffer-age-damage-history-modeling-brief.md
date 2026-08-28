# Buffer-Age Damage History Modeling Brief

## 1. System Overview

Sophia's native renderer path is Category B (Concurrent / Runtime). Three
complete target slots per physical head are promoted and proven: a slot is
identified by a stable slot number plus a monotonically increasing incarnation,
and only return of the exact lease token makes it reusable. This brief
supersedes the exclusion of buffer age recorded in
`native-frame-slot-retirement-modeling-brief.md` sections 3.2 and 6.3, which
deliberately deferred it until the slot lifetime itself was settled.

The proposed optimization retains, per slot, a description of the content last
written into that slot. When a repaint acquires a slot whose retained content is
still valid, it repaints only the damage accumulated since that content was
written rather than the whole output. This is buffer age: a slot's content is
some number of frames old, and the work owed to it is the union of every frame's
damage since.

The correctness question is not whether damage is smaller. It is whether a slot
repainted from history holds exactly what a full repaint would have produced.
Under-computed accumulated damage leaves a region stale, and the resulting frame
is presentable, self-consistent, and wrong. That failure survives every check
the slot-retirement model already makes, because slot ownership is undisturbed.

Sophia already contains three narrower forms of this mechanism, and the model
should describe the general one they are instances of:

- `production_cpu_scene.rs` retains a bounded three-deep frame ring and
  computes a repaint baseline from it, returning nothing when the plan says
  full.
- `exporter/worker/damage.rs` rewrites a reused CPU buffer from a single
  previous checksum and snapshot, falling back to a full-size rectangle when
  either side is missing.
- `compositor_graphics.rs` tracks pending, rendering, submitted, and presented
  output frames, which is buffer age of exactly one.

The mixed and DMA-BUF paths have no such mechanism and re-render the whole slot
every frame, though the damage snapshot already reaches the worker.

One open question governs the implementation and not the model. A slot's frame
surface is an EGL/GBM window surface driven by `eglSwapBuffers`
(`gbm_platform/scanout/render.rs`), so the slot does not own one buffer: the
surface rotates through its own set, and the buffer handed back on a later
render into the same slot may be two or three swaps old rather than one frame
old. "The content of slot N" is therefore an abstraction over something the
driver owns. An implementation that keys history by slot alone will
under-compute damage exactly when the surface has more than one back buffer,
which is the normal case.

Two shapes resolve it, and the model deliberately does not choose between them:
query `EGL_BUFFER_AGE_EXT` per acquired back buffer and use the age the driver
reports, or stop swapping and manage each slot's buffer explicitly so slot
identity and buffer identity coincide. The model describes what a repaint owes
given some content age; whichever mechanism supplies that age must be correct
about it, and a wrong age produces exactly the stale-region defect
`RepaintMatchesFullRepaint` rules out.

## 2. Scenarios

### Scenario 1: A Slot Is Reused After Intervening Frames

**Mechanism**: slot 1 holds the content of generation A. Generations B and C are
rendered into slots 2 and 3. Generation D acquires slot 1, whose content is two
frames old. The work owed is the union of the damage of B, C, and D, not D's
damage alone.

**Evidence**:

- Code analysis: `LiveRendererFrameSlotPool::try_acquire` selects round-robin
  from `next_slot`, so a reacquired slot is routinely several generations stale
  (`exporter/frame_slots.rs`).
- Code analysis: the worker's existing CPU reuse compares against exactly one
  previous snapshot, which is correct only at age one
  (`exporter/worker/damage.rs`).

**Affected code paths**: slot acquisition, worker render entry, damage
computation, and the composition scissor.

**Suggested modeling approach**: give each slot an abstract per-region content
map and each frame a damage region set. Admit a partial repaint only when the
damage applied is a superset of the regions where the slot's content differs
from the scene being painted.

### Scenario 2: A Rebuilt Bundle Invalidates Its History

**Mechanism**: an output size change or a retryable target failure rebuilds a
free slot's context, pipeline, and frame surface. The pixels the slot held are
gone, but its slot number and incarnation may be unchanged.

**Evidence**:

- Code analysis: size changes and retryable target failures rebuild a free
  bundle, while the slot itself is not necessarily reallocated
  (`gbm_platform/scanout/context.rs`).
- Historical: the CPU scene already clears its retained frames on the
  invalidating transitions rather than trusting their age
  (`production_cpu_scene.rs`).

**Affected code paths**: target bundle rebuild, slot history recording, and the
full-repaint fallback.

**Suggested modeling approach**: model history validity as a separate flag from
slot ownership, cleared by rebuild, and require that a slot with invalid history
is only ever written by a full repaint.

### Scenario 3: History Outlives Its Incarnation

**Mechanism**: slot 1 records content at incarnation 7. The slot is released and
reused at incarnation 8 by a different generation. A repaint then reads slot 1's
history without checking which incarnation wrote it.

**Evidence**:

- Model: the promoted slot model already proves the release path against this
  ABA shape (`StaleReleasePreservesOwner`, `ReleasedTokenIsNotCurrent`).
- Code analysis: history has no owner today, so nothing yet establishes that the
  same discipline governs reads as governs releases.

**Affected code paths**: history recording, history lookup, and slot reuse.

**Resolution**: not modelled, because the premise is wrong. A slot's buffer keeps
its content across release and reacquisition, and that persistence is exactly
what makes buffer age worth anything; history therefore dies with the bundle or
with an incomplete write, not with the lease. `VisualRetirementSlots` already
owns the lease identity that rejects a stale release. What remains of this
scenario is the real question of where content age comes from, recorded in
section 1: a surface that swaps through several back buffers makes "the content
of slot N" a claim only the driver can settle.

### Scenario 4: Deferred Work Is Superseded Before It Is Written

**Mechanism**: a fourth generation defers at capacity. A newer generation
replaces it in the latest-wins pending cell. The deferred generation's damage
must still be accounted for by whatever is eventually written, because its
scene changes really happened.

**Evidence**:

- Model: deferral is an ordinary bounded state in the promoted model, and
  superseded deferred work is explicitly not a failure.
- Code analysis: the exporter retains one latest-wins pending frame, so
  intermediate generations can be skipped entirely
  (`exporter/worker.rs`).

**Affected code paths**: deferral, latest-wins replacement, and damage
accumulation across skipped generations.

**Suggested modeling approach**: accumulate damage per generation independently
of whether that generation is ever rendered, so a skipped generation still
contributes to what a later slot write owes.

### Scenario 5: Mirror Heads Keep Separate Histories

**Mechanism**: two heads present the same logical output. Each owns its own
three-slot pool. Head 1's slot may be one frame old while head 2's is three.

**Evidence**:

- Model: the promoted model already carries two heads and out-of-order
  retirement.
- Code analysis: each physical-head worker owns its own pool, and the schema-7
  record sums its watermark across them, which archive `0001` observed as six.

**Affected code paths**: per-head worker state, per-head damage snapshots, and
the head composition plan.

**Suggested modeling approach**: none. Index history by head as well as slot in
the implementation, and prove the isolation by test: a second modelled pool
repeats the indexing the single-pool model already checks.

### Scenario 6: A Partial Repaint Fails Midway

**Mechanism**: a damage-limited repaint fails after writing some of its damage
region. The slot now holds neither the old content nor the new.

**Evidence**:

- Code analysis: retryable target failures already exist as a distinct outcome
  from success and from permanent failure.
- Model: the promoted model requires failed work to retain every kernel-owned
  slot, which says nothing about what the slot's pixels then contain.

**Affected code paths**: repaint failure handling, history recording, and
recovery.

**Suggested modeling approach**: record history only on a complete write, so a
failed repaint leaves the slot's history invalid and forces the next write to be
full.

## 3. Modeling Recommendations

### 3.1 Model

- A small abstract region partition of one output, with a generation mark per
  region rather than pixel values.
- Per-slot content maps, and per-slot history validity independent of ownership.
- History bound to the exact slot incarnation that recorded it.
- Per-generation damage regions accumulated across skipped and deferred work.
- Full repaint and damage-limited repaint as distinct admitted transitions.
- Bundle rebuild, partial-write failure, and slot reuse as invalidating events.
- One head. Per-head history is the same indexing property applied twice, and
  a second pool of three slots over a region map costs states without adding a
  distinct failure mode, so scenario 5 is discharged by test rather than by
  model.

### 3.2 Do Not Model

- Pixel values, colour, sampling, or scissor rectangle arithmetic.
- EGL, GBM, DRM, framebuffer, or DMA-BUF numeric handles.
- The bounded CPU buffer cache beneath target-slot lifetime.
- Renderer-image capture targets, which do not enter the scanout slot pool.
- Damage coalescing heuristics, rectangle counts, and coverage thresholds; the
  model governs what a repaint must cover, not how the planner chooses to
  express it.
- Wall-clock deadlines, worker channels, and thread scheduling.

## 4. Proposed Extensions

| Extension | Variables | Purpose | Scenario |
|-----------|-----------|---------|----------|
| Slot content | `slotContent` | Abstract per-region content held by each slot | 1, 6 |
| Scene content | `scene`, `sceneGeneration` | The content a full repaint would produce | 1, 4 |
| Damage journal | `damageOf` | Regions each generation changed, including skipped ones | 1, 4 |
| History validity | `slotHistoryValid`, `slotGeneration` | Rebuild, partial failure, and never-written states | 2, 6 |

## 5. Proposed Invariants

| Invariant | Type | Description | Targets |
|-----------|------|-------------|---------|
| RepaintMatchesFullRepaint | Safety | A written slot's content equals the scene it was written for | 1, 4 |
| DamageCoversDivergence | Safety | An admitted partial repaint covers every region where the slot differed | 1, 4 |
| FailedWriteInvalidatesHistory | Safety | An incomplete write leaves no usable history behind | 6 |
| InvalidHistoryClaimsNoGeneration | Safety | An invalidated slot claims no generation to repaint against | 2, 6 |
| PartialWriteIsReachable | Reachability | The optimization is admissible at all, not merely safe | 1 |

## 6. Findings Pending Verification

### 6.1 Model-Checkable

- A slot reused after skipped generations must repaint their accumulated damage.
- A rebuilt bundle must force a full repaint even though its slot is unchanged.
- Superseded deferred work must still contribute damage to a later write.
- A partially written slot must not be treated as holding either content.

### 6.2 Testable Only

- One head's fresher history must not shorten another head's repaint; the pools
  are per head and the model checks one of them.
- The pool records content only on a complete export and clears it on rebuild.
- The shared damage helper falls back to a full-output rectangle wherever the
  retained snapshot is absent or the plan says full.
- The mixed and DMA-BUF export paths accept and honour a damage region rather
  than re-rendering the slot.
- Damage hits, full-repaint fallbacks, and history invalidations appear in the
  native persistent-resource record, and its schema bump reaches every verifier
  and fixture that reads the previous one.
- A run whose history is never valid produces identical pixels to one whose
  history is always valid, at different cost.
- The age a slot's content actually has matches the age its history claims,
  across a surface that swaps through more than one back buffer.

### 6.3 Out Of Scope

- Shared workers across heads or outputs.
- Direct client scanout and hardware cursor planes.
- Destination-buffer preservation for the `skip` observation in
  `compositor-graphics.md`, which is a separate question about suppressing a
  frame rather than shortening one.
- Dynamic output topology reconstruction beyond preserving existing VT and
  mirror cleanup semantics.
