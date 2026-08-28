# Shared Renderer Worker Modeling Brief

## 1. System Overview

Every physical head owns a complete renderer worker today. Each holds its own
`sophia-render-gpu` thread against its own `dup` of the same DRM card file, its
own `gbm::Device` and EGL display, its own store of imported renderer images,
its own three-slot target pool, and its own damage history
(`scanout/rendered_scanout/exporter/worker.rs`,
`exporter/discovery.rs`, `production_session/native_scanout.rs`). Two heads on
one card therefore import the same client DMA-BUF twice and keep two complete
GL stacks against one device. Mirror heads, which by construction share a card
session, are no exception.

Milestone 14 asks for one worker per DRM/render-device group, preserving one
latest pending request per output, bounded response demultiplexing, explicit
per-output retirement tokens, and bounded inter-output service skew under
concurrent producers. The grouping already exists: `LiveProductionNativeGroup`
is a card session, one per selected card, and a mirror group may not span
cards. The unit of coalescing is therefore given, not invented.

The prior slot brief placed "shared workers across heads or outputs" out of
scope, and the milestone requires the model before the semantics change. This
brief is that predicate.

## 2. Scenarios

### Scenario 1: Two Outputs Contend For One Thread

**Mechanism**: both heads of one card have a composed generation ready. One
thread can render one of them at a time, so the other waits behind it. Today
each head has a thread and neither waits; after coalescing, service order is a
decision the system makes on every frame.

**Evidence**:

- Code analysis: the exporter submits only when `WorkerPoll::Idle`, and the
  worker refuses a second `Render` while `in_flight.is_some()`
  (`exporter/worker.rs`, `exporter/discovery.rs`).
- Physical evidence: the 2026-08-28 latency run measured dwell-to-submit p99 at
  7 ms with a per-head worker; the same budget must survive sharing.

**Affected code paths**: worker command queue, exporter poll ladder, frame
service arbitration order.

**Suggested modeling approach**: one queue, one render at a time, and a
per-output count of how many times a waiting request was passed over. Bound
that count by the number of siblings rather than by any duration.

### Scenario 2: A Result Must Name Its Output

**Mechanism**: with one request outstanding, the next result on the channel was
necessarily the answer to it, and a mismatched request id was a fault
(`worker.rs` quarantines on mismatch). Two outputs sharing the thread make the
same message stream ambiguous, and a result applied to the wrong output would
attach one head's rendered buffer to another head's scanout lease.

**Evidence**:

- Code analysis: `WorkerResult` carries `request_id` and no output identity;
  `WorkerOutcome::Exported` carries a lease id and slot token that the exporter
  attributes to itself by position.
- Historical: mirror callbacks had the same defect class by output rather than
  by head, fixed by matching on head identity (`poll_group_callbacks`).

**Affected code paths**: `WorkerCommand::Render`, `WorkerResult`, lease
construction, `NativeGbmRendererWorker::poll`.

**Suggested modeling approach**: an explicit routing target for each finished
render, and a fault flag set if a result ever reaches an output that did not
request it. Keep the reply cell per output so routing is structural rather than
a check performed after the fact.

### Scenario 3: One Output Draws Continuously While The Other Waits

**Mechanism**: a client repainting every frame on one head keeps offering new
generations. Its pending cell supersedes in place, but its *submissions* still
enter the shared queue. Nothing in the per-head design had to prevent one head
from monopolising a thread, because no head shared one.

**Evidence**:

- Code analysis: `replace_pending_frame` is latest-wins per exporter, but
  service order across exporters is decided by the caller, and
  `OutputFrameServiceReducer` currently selects with `.min()` over an ordered
  set of output ids — strictly lowest-id-first within a pass.
- Roadmap: the two-output concurrent-producer workload is a separate row that
  explicitly waits on this one, and the existing render-contention gate is
  documented as single-active-output evidence that may not be cited for
  inter-output fairness.

**Affected code paths**: shared queue service order, frame service reducer.

**Suggested modeling approach**: allow either output to compose without bound
(subject only to a state-space budget) and check that a waiting request is
never passed over more times than it has siblings. A fixed-order scan should
break the invariant; taking the queue in order should not.

### Scenario 4: Composition Continues While A Render Is In Flight

**Mechanism**: the pending cell is filled without consulting the worker, so a
new generation can arrive for an output whose previous one is still rendering.
It supersedes the cell and waits for the in-flight request to complete.

**Evidence**:

- Code analysis: `pending_frame()` reports true when *either* the cell is
  occupied or the worker is in flight; `discard_pending_frame` refuses while in
  flight — the two states are deliberately distinguished.
- Model finding: an early draft of `SharedWorkerService.tla` forbade this, and
  the effect was to make the submission gate's negative control unable to fail,
  because the environment was silently enforcing the gate.

**Affected code paths**: `replace_pending_frame`, supersession counters.

**Suggested modeling approach**: composition must be permitted in every state
where the code permits it, including during an in-flight render. Model
fidelity here is what makes the submission gate load-bearing.

### Scenario 5: Device State Shared, Output State Separate

**Mechanism**: the EGL display, GBM device, and renderer-image store belong to
the device and should exist once. Target slots, damage history, pixel-proof
budget, and reusable CPU buffers belong to an output and must not be shared —
CPU buffer reuse is currently matched on size alone, which would silently
cross-feed two same-sized outputs' content.

**Evidence**:

- Code analysis: `NativeGbmRenderedScanoutContext` holds both kinds of state in
  one struct; `LiveRendererFrameSlotPool` is one per worker; `WorkerSlotDamage`
  documents itself as "one per worker, which is one per physical head".
- Doc: retained renderer-image identities are realized per physical renderer
  store and one head never treats another's cache identity or scanout lease as
  locally usable.

**Affected code paths**: renderer context construction, slot pools, damage
history, image store, free CPU buffer list.

**Suggested modeling approach**: out of scope for this model. The split is a
structural refactor whose correctness is carried by the existing slot model
(pools become per output within a worker, unchanged in their own semantics) and
by deterministic Rust regressions, not by new temporal properties.

## 3. Modeling Recommendations

### 3.1 Model

- One shared worker, one FIFO command queue, one render at a time.
- A latest-wins pending cell per output, superseding in place.
- A submission gate admitting at most one outstanding request per output.
- Explicit routing of each result to the output that requested it, with a fault
  flag for misdelivery.
- A pass-over count per output, bounded by sibling count.
- Liveness under fairness on service only, never on the environment.

### 3.2 Do Not Model

- Slot incarnations, stale releases, and the ABA case — `VisualRetirementSlots`
  owns these and this row does not change them.
- Any duration, deadline, or refresh-relative bound. The harness holds that no
  model expresses timing; the physical latency gate owns that half.
- EGL, GBM, import caching, and renderer-image identity — resource plumbing
  whose correctness is byte-level and belongs to tests and the buffer-age
  equivalence proof.
- Mirror cohort join and per-head pacing — `MirrorHeadPacing` owns them, and
  they sit above the worker.
- Multi-card topologies. Groups are per card by construction.

## 4. Proposed Extensions

| Extension | Variables | Purpose | Scenario |
| --- | --- | --- | --- |
| Shared service queue | `queue`, `rendering`, `renderingGen` | One thread serving several outputs in order | 1, 3 |
| Per-output request cells | `pending`, `inFlight` | Latest-wins input and the one-outstanding gate | 1, 4 |
| Per-output reply cells | `reply`, `misrouted` | Response demultiplexing as structure, not inspection | 2 |
| Service accounting | `served`, `passedOver` | Skew stated against an output that is actually waiting | 3 |

## 5. Proposed Invariants

| Invariant | Type | Description | Targets |
| --- | --- | --- | --- |
| `ResponsesRouteToTheirOutput` | Safety | No result reaches an output that did not request it | 2 |
| `OneInFlightPerOutput` | Safety | An output occupies the worker at most once | 1, 4 |
| `PendingIsLatestPerOutput` | Safety | The pending cell holds one generation, never a queue | 4 |
| `ServiceSkewBounded` | Safety | A waiting output is passed over at most once per sibling | 3 |
| `EveryOutputIsServed` | Liveness | Offered work is eventually rendered for its output | 1, 3 |

## 6. Findings Pending Verification

### 6.1 Model-Checkable

- Whether FIFO service alone bounds skew, or whether an explicit round-robin is
  required. (Checked: FIFO suffices at two outputs; the no-FIFO control fails
  at depth 11.)
- Whether the per-output submission gate is implied by other constraints.
  (Checked: it is load-bearing once composition may proceed during a render.)

### 6.2 Testable Only

- That a release carrying one output's slot token cannot free another output's
  slot once pools are per output within a worker.
- That a misrouted reply is treated as a fault rather than delivered, matching
  the existing quarantine-on-mismatch behaviour.
- That renderer images are imported once per device rather than once per head,
  which is the resource win the row exists for.
- That per-output pixel-proof budgets remain per output after the context
  split, so one head's capture cannot latch another's.

### 6.3 Out Of Scope

- Cross-card worker sharing.
- Inter-output fairness under concurrent *producers*, which is its own roadmap
  row and needs a workload driving both outputs of one group.
- Direct scanout and cursor-plane rows.
