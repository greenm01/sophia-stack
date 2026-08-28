# Native Frame-Slot Retirement Modeling Brief

## 1. System Overview

Sophia's native renderer path is Category B (Concurrent / Runtime). Each
physical head has a renderer worker, while the session owner coordinates the
heads that form one logical output. A worker may finish rendering before KMS
accepts a flip, KMS callbacks may retire mirror heads in either order, and a
newer immutable frame may replace work that has not entered the kernel. The
resource boundary is therefore not rendering completion: a complete native
target remains owned until the exact page-flip lease that refers to it retires.

The proposed optimization gives each physical head three complete target
slots. A slot contains its render context, pipeline, frame surface, and import
cache. It is identified by a stable slot number plus a monotonically increasing
incarnation. A worker lease carries that exact token. Only return of the exact
lease token makes the slot reusable; renderer completion, Rust reference
counts, failure of a sibling mirror head, VT change, or a stale callback do not.

## 2. Scenarios

### Scenario 1: Three Live Generations Fill The Pool

**Mechanism**: the currently displayed generation retains one slot, a submitted
successor retains another, and a third generation can be rendering or prepared.
A fourth request arrives before any page-flip retirement.

**Evidence**:

- Code analysis: native scanout separately owns displayed, submitted, prepared,
  and cleanup buffer owners (`production_session/native_scanout.rs`).
- Ratified roadmap: `todo.md` Milestone 14 requires three generational complete
  frame-surface slots and bounded deferral when all are retained.

**Affected code paths**: worker submission, renderer target selection, pending
frame coalescing, and persistent-resource metrics.

**Suggested modeling approach**: model three slots per output, permit one exact
generation token per occupied slot, and represent a fourth proposal as deferred
without a slot. Once an exact old lease retires, the latest deferred generation
may acquire the newly free slot.

### Scenario 2: A Stale Release Arrives After Slot Reuse

**Mechanism**: slot 1 carries generation A at incarnation 7. A's exact release
frees it and generation D reuses slot 1 at incarnation 8. A duplicate or delayed
release for `slot 1/incarnation 7` then arrives.

**Evidence**:

- Code analysis: worker leases currently return only a monotonically allocated
  lease identifier through `WorkerCommand::Release`; the retained-buffer map
  removes by that identifier (`exporter/worker.rs`).
- Historical: `97d65aac` added renderer-worker draining during cleanup and
  `9ff6de53` joined renderer workers at shutdown, demonstrating that release and
  teardown cross an asynchronous worker boundary.

**Affected code paths**: `NativeGbmRendererWorkerScanoutLease::drop`, the worker
command loop, slot allocation, and VT/shutdown draining.

**Suggested modeling approach**: store both slot and incarnation in every
lease. Exact release changes an occupied slot to free. A stale token is consumed
or ignored but never mutates the current owner. Permit slot reuse so the model
actually reaches the ABA-shaped state.

### Scenario 3: Mirror Heads Retire Out Of Order

**Mechanism**: one logical output has two physical heads. Both head leases carry
the same logical generation, but one callback retires before the other. A later
generation may be ready while the first head still scans the older generation.

**Evidence**:

- Code analysis: each physical head owns its own exporter and KMS callback
  state, while logical presentation joins all head completions
  (`production_session/native_scanout.rs`).
- Existing model: `VisualRetirement.tla` already requires all heads of a mirror
  output to flip before that output commits and treats head loss separately.
**Affected code paths**: per-head displayed/submitted owners, mirror-group
completion, failure cleanup, and logical feedback.

**Suggested modeling approach**: retain the existing head-scoped flip model.
Attach the logical generation to one slot per participating output and allow
callbacks in either order. Do not release a logical generation while any head
or committed output still refers to it.

### Scenario 4: Partial Failure And VT Cleanup Drain Exact Owners

**Mechanism**: preparation or submission succeeds for some heads and fails for
another, or VT release begins while work is pending. Accepted KMS work still
owns its slot until callback or controlled detach; work that never entered KMS
may be dropped immediately.

**Evidence**:

- Code analysis: page-flip cleanup owners are distinct from displayed and
  submitted owners, and suspension performs a bounded quiescence before forced
  detach (`production_session/native_scanout.rs`).
- Historical: `3af596cd` added scanout quiescence before VT switches;
  `1d18f261` fixed stalled suspension; `97d65aac` drains the renderer worker.
- Existing model: failure never implies release; `Release` additionally requires
  no in-flight head and no output still committed to the generation.

**Affected code paths**: partial mirror submission, native failure reduction,
VT suspend/resume, controlled detach, and worker shutdown.

**Suggested modeling approach**: preserve failure and head-loss actions, but
make slot release depend on the same exact ownership predicates as ordinary
retirement. Model stale release delivery as an explicit no-op.

### Scenario 5: Deferred Work Is Superseded Before Capacity Returns

**Mechanism**: generation D is deferred because all three slots are occupied;
generation E then replaces it in the exporter's latest-wins pending cell before
a slot is freed.

**Evidence**:

- Code analysis: the live exporter already retains one latest-wins pending
  rendered frame while a worker request is in flight (`discovery.rs`).
- Research record: native pending scanout was deliberately unified into one
  latest-wins slot; renderer `Pending` is a normal bounded state, not failure.

**Suggested modeling approach**: permit a deferred candidate to settle as
superseded when a newer generation exists. Slot exhaustion must not produce a
failure outcome or publish feedback.

## 3. Modeling Recommendations

### 3.1 Model

- Three stable slot identifiers and monotonically increasing incarnations.
- Exact token acquisition, page-flip retirement, release, and slot reuse.
- A displayed, submitted, and prepared generation occupying all three slots.
- A fourth generation deferred at capacity and later admitted or superseded.
- Two-head mirror retirement in either callback order.
- Failure, head loss, stale release, and release after replacement.
- Independent logical outputs in the existing small cross-output configuration.

### 3.2 Do Not Model

- EGL, GBM, DRM, framebuffer, or DMA-BUF numeric handles.
- Pixel content, damage rectangles, buffer age, or direct-scanout eligibility.
- Worker channel implementation, wall-clock deadlines, or thread scheduling.
- CPU BO recycling; it is a separate bounded cache beneath target-slot lifetime.
- Renderer-image capture targets, which do not enter the scanout slot pool.

## 4. Proposed Extensions

| Extension | Variables | Purpose | Scenario |
|-----------|-----------|---------|----------|
| Slot ownership | `slotOwner`, `generationSlot` | At most one generation owns a slot | 1 |
| Incarnations | `slotIncarnation`, `generationToken` | Reject ABA-shaped stale release | 2 |
| Deferral | `deferred` | Capacity is pending, not failure | 1, 5 |
| Exact return | `releaseTokens` | Separate current from stale release | 2, 4 |
| Existing head layer | `inFlight`, `flipped`, `lost` | Preserve mirror and failure joins | 3, 4 |

## 5. Proposed Invariants

| Invariant | Type | Description | Targets |
|-----------|------|-------------|---------|
| SlotOwnersAreUnique | Safety | One output slot has at most one generation owner | 1 |
| ActiveGenerationOwnsSlot | Safety | Prepared, submitted, in-flight, or committed work retains its slot | 1, 3 |
| SlotTokenMatchesOwner | Safety | Every generation token equals the slot's current incarnation | 2 |
| StaleReleasePreservesOwner | Safety | An old token cannot clear or relabel a reused slot | 2, 4 |
| DeferredOwnsNoSlot | Safety | Capacity-deferred work cannot mutate a live target | 1, 5 |
| FailureDoesNotReleaseInFlight | Safety | Failed work retains every kernel-owned slot | 4 |
| AvailableCapacityAdmitsLatest | Liveness | Stable latest deferred work eventually acquires a released slot | 1, 5 |

## 6. Findings Pending Verification

### 6.1 Model-Checkable

- A fourth live generation must defer while three target slots are retained.
- Exact release followed by reuse must advance the slot incarnation.
- Delivery of an older release token after reuse must preserve the new owner.
- The first mirror-head callback must not make the generation releasable.
- A failed or disconnected generation remains retained while any accepted head
  is in flight or any output still displays it.
- Latest-wins replacement of deferred work must not count as export failure.

### 6.2 Testable Only

- Every worker-produced lease carries its slot token into `Drop`.
- The worker release command validates lease ID, slot ID, and incarnation.
- A free compatible slot reuses its complete context/pipeline/surface bundle;
  an incompatible free slot rebuilds that bundle.
- Slot capacity, reuse, deferral, and stale-release counts appear in native
  persistent-resource metrics.
- CPU direct export, mixed export, and DMA export obey the same three-slot lease
  ceiling without merging their distinct pixel paths.

### 6.3 Out Of Scope

- Buffer-age damage history and incremental repaint.
- Shared workers across heads or outputs.
- Direct client scanout and hardware cursor planes.
- Dynamic output topology reconstruction beyond preserving existing VT and
  mirror cleanup semantics.
