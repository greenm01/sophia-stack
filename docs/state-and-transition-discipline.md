# State and Transition Discipline

**Role:** non-normative architecture rationale.

This document explains the computer-science foundations behind Sophia's
authority, transaction, and snapshot rules. It does not define a new protocol
and does not override the normative contracts in [Architecture](architecture.md),
[Engine Architecture](engine-architecture.md), or
[Data-Oriented Design](dod.md).

Sophia deliberately separates protocol compatibility, spatial policy,
namespace transfer policy, rendering, and physical presentation. That
separation preserves X11 freedom without making X11 objects the compositor's
native state, and it permits policy components to evolve without taking
display authority. The cost is additional asynchronous coordination and more
failure boundaries.

Four foundations keep that cost bounded:

1. transition systems and TLA+ describe legal state changes and the safety and
   liveness properties that must survive every ordering;
2. I/O automata describe independently owned components that communicate by
   observations, proposals, and outcomes;
3. single-writer authority and explicit linearization points prevent
   split-brain state; and
4. the CALM principle identifies which facts can flow without coordination and
   which decisions require serialization.

The State-Action-Model pattern provides useful vocabulary for a subset of this
shape, but it is not the foundation of Sophia and is not a universal Engine
execution model.

## The Complexity Being Managed

Sophia's process and authority boundaries create several deliberate risks:

| Architectural choice | Resulting risk | Primary control |
| --- | --- | --- |
| Protocol frontend is separate from Engine | delayed, duplicated, reordered, or stale observations | I/O automata and generation-checked packets |
| WM proposes policy outside Engine | policy response may describe an obsolete scene | single-writer Engine validation and transaction IDs |
| Rendering and KMS complete asynchronously | prepared state may be mistaken for visible state | explicit visual linearization at page-flip retirement |
| Namespace and portal policy are separate authorities | a component may accidentally infer or acquire another authority's privilege | typed inputs, fail-closed transitions, and ownership invariants |
| Engine is the visual authority | unnecessary serialization could turn it into a universal bottleneck | CALM classification and domain-local state |
| Frontends remain extensible | protocol-local sequencing may leak into the product core | protocol-neutral automata and reduced facts |

The answer is not to merge these roles. It is to refuse to distribute one
kind of authoritative truth among them.

## Current Conformance and Known Gaps

As of 2026-08-01, Sophia already implements much of this discipline:

- `PreparedSurfaceCommit` separates a candidate from committed state;
- retirement-time baseline validation rejects stale candidates while retaining
  unrelated newer commits;
- WM policy is planned against a cloned candidate and applied atomically;
- typed IDs, generations, bounded packets, immutable snapshots, and explicit
  outcomes are established boundary conventions;
- the central portal lifecycle rejects duplicate IDs, enforces capacity, and
  ties grants to generations; and
- deterministic tests cover stale transactions, timeout preservation,
  unrelated commit retention, portal revocation, focus validity, and output
  retirement ordering.

The remaining gaps are architectural hardening work, not a reason to replace
Engine with a global SAM, TEA, actor, or message-loop abstraction.

### 1. The Retirement Gate Is Not Yet Universal

The prepared Present path promotes a candidate only after matching page-flip
retirement. Some general authority paths can still advance a value named
`committed_surfaces` before composition and physical retirement:

- `AuthorityTransactionIntake::commit` calls the direct
  `HeadlessEngine::commit_surface_transactions` helper;
- `ProductionSessionCoordinator::commit_authority_batches` runs before frame
  composition and KMS submission; and
- `ProductionSessionCoordinator::replace_committed_surfaces` and the matching
  backend-assembly method permit wholesale snapshot replacement.

The target is one visible transition for every application visual source,
including software-rendered X11 content:

```text
observe -> prepare -> compose candidate -> submit -> retire -> commit
```

If an implementation needs accepted logical state before retirement, that
state must be named and stored as accepted or prepared state. It is not
committed visual truth and cannot drive hit-testing, committed focus,
successful presentation feedback, or resource release.

### 2. Commit Authority Is Not Yet Sealed by a Retirement Witness

The current apply APIs accept a `PreparedSurfaceCommit`, but the type itself
does not prove that its exact output frame retired. Correct call order is
maintained by the production coordinator rather than enforced by the type
boundary.

The retirement reducer should produce a typed witness carrying the output,
frame serial, transaction, submission generation, and retirement outcome.
Only the owning Engine coordinator should be able to consume that witness with
the matching prepared candidate. Direct commit and apply helpers should then be
restricted to that owner.

### 3. Output State Copies Still Resemble Alternate Writers

Per-output live runtimes currently install copied committed-surface vectors
through replacement methods. These copies are intended as projections, but
their mutable API shape resembles another source of truth and permits callers
to manufacture a replacement snapshot.

Engine should publish one immutable, generation-tagged committed scene
snapshot. Output, rendering, and backend consumers may retain or project that
snapshot read-only. If a consumer needs local installation state, its type and
method names must identify it as a projection that cannot flow back into
Engine authority.

### 4. Boundary Outcomes Need Complete Dispositions

Sophia's packets already distinguish commit, stale rejection, invalid
rejection, and timeout. Some internal paths still collapse failed readiness,
supersession, disconnect, backpressure, and other terminal causes into broader
outcomes.

Each I/O-automaton boundary needs an explicit disposition for accepted,
duplicate or idempotent retry, stale, superseded, malformed, unauthorized, not
ready, timed out, disconnected, and backpressured work. Public wire enums need
not grow without compatibility evidence, but Engine must retain an exact
internal reason for tests, settlement, and privacy-safe diagnostics.

### 5. Portal Reducers Can Bypass Central Lifecycle Rules

`PortalLifecycleCoordinator` provides bounded capacity, duplicate rejection,
generation checks, and single-use grants. Individual portal reducers also own
public maps and currently insert a request by transfer ID directly, which can
replace an existing record if those reducers are used outside the coordinator.

The lifecycle coordinator should become the sole public admission path, or
every reducer must enforce the same uniqueness, capacity, generation, and
terminal-state rules. No portal entry point may weaken the central lifecycle by
being easier to call.

### 6. The Formal Model Does Not Yet Exist

The repository has deterministic transaction and integration tests but no TLA+
module or equivalent executable state-machine model. The first model should be
small: propose, prepare, submit, retire, reject, timeout, disconnect, remove,
and release. It should model bounded surfaces, candidates, submissions,
committed generations, focus, and referenced buffers rather than implementation
types or graphics details.

The model should check the safety and liveness properties listed below and
produce counterexample traces that can be translated into deterministic Rust
regressions. It belongs in validation only after its toolchain and reproducible
command are documented.

### 7. Transition Replay Is Not Yet a First-Class Diagnostic

Sophia has structured logs and deterministic frame-command replay, but not one
bounded authority-transition trace spanning proposal, preparation, submission,
retirement, rejection, and release.

A privacy-safe trace should retain only the owning authority, opaque target,
previous and candidate generations, action, outcome, internal reason, and
related submission identity. It must not retain namespace identity, XIDs,
application metadata, portal payloads, or pixel content. Such traces would
connect physical failures, deterministic regression tests, and the formal
model without weakening isolation.

## Common Transition Shape

The public boundaries follow one conceptual direction:

```text
observations and requests
          |
          v
bounded proposals
          |
          v
owning authority validates and reduces
          |
          v
prepared candidate and explicit effects
          |
          v
external execution and returned observation
          |
          v
commit or reject, then publish snapshots and feedback
```

This is a reasoning model, not a requirement that every subsystem use one
framework or one global event loop. Engine hot paths remain explicit
data-oriented systems. A WM or portal policy reducer may use TEA style. A
protocol frontend retains the private tables and sequencing needed to honor
its native protocol. What stays constant is ownership: an effect cannot mutate
another authority's state, and its result returns as a new typed observation.

## 1. Transition Systems and TLA+

A transition system describes behavior as a sequence of states connected by
named actions. TLA+ adds a practical language for specifying those actions and
checking safety and liveness across many possible interleavings.

Sophia's visual lifecycle is naturally expressed in this form:

```text
proposed -> validated -> prepared -> submitted -> retired -> committed
              |             |           |
              +-----------> rejected <--+
                            or timed out
```

The diagram is intentionally abstract. A real transaction can contain several
surfaces and output frames, and domain-local facts may advance while its visual
candidate waits. The essential distinction is that a proposal, a prepared
candidate, a submitted frame, and committed visual state are different facts.

Useful abstract variables include:

- the current committed surface generation and buffer;
- pending candidates and their source transaction IDs;
- submitted output frames and their referenced resources;
- current committed focus and placement;
- active namespace grants and revocations; and
- completion, rejection, timeout, and disconnect observations.

Useful abstract actions include proposal intake, validation, readiness,
submission, page-flip retirement, timeout, revocation, surface removal, and
process disconnect. The model should omit Rust types, DMA-BUF formats, X11
opcodes, and rendering algorithms unless one of them changes the ordering
property under study.

This foundation controls combinatorial risk. Instead of testing only the
expected sequence, a model can ask whether a stale page flip, revocation, WM
timeout, and frontend disconnect in any order can violate a Sophia invariant.
It does not replace implementation tests; it identifies the state combinations
those tests must cover.

## 2. I/O Automata

An I/O automaton has input actions, output actions, and private internal
actions. Multiple automata compose by matching outputs from one component to
inputs of another while retaining private state ownership.

Sophia's major automata are:

| Automaton | Representative inputs | Representative outputs |
| --- | --- | --- |
| Protocol frontend | client requests, Engine control, admission facts | surface transactions, lifecycle facts, route outcomes |
| Engine | authority observations, input facts, WM proposals, backend observations | controls, snapshots, render work, presentation outcomes |
| WM policy | opaque layout snapshots and policy events | bounded layout and focus proposals |
| Portal policy | reduced request facts, user decisions, revocations | deny, grant, revoke, or executor commands |
| Session supervisor | process exits, admission requests, health observations | identities, authorization material, restart or teardown effects |
| Renderer and live backend | immutable plans and submission effects | import, submission, completion, and failure observations |

The table describes logical authorities, not required process boundaries. Two
automata may temporarily share a process without gaining access to each
other's state.

Every cross-automaton contract should make these cases explicit:

- identity and generation of the target;
- bounded size and cardinality;
- duplicate and retry behavior;
- stale or out-of-order handling;
- rejection and timeout outcomes;
- disconnect and restart behavior; and
- backpressure when the receiver cannot accept more work.

This foundation controls hidden synchrony. A frontend must not assume that the
next Engine message answers its newest request. Engine must not assume that a
WM response still describes the current workspace. A KMS completion is an
observation tagged to submitted work, not ambient permission to commit the
newest candidate.

## 3. Single-Writer Authority and Linearization

Each authoritative fact has one writer. Other components may hold immutable
snapshots, opaque IDs, or execution handles, but they cannot independently
advance that fact.

The normative ownership table remains in
[Architecture](architecture.md#load-bearing-ownership-rules). In theoretical
terms, the main domains are:

- Engine owns committed visual placement, stacking, physical focus, frame
  policy, and active visual generations;
- a protocol frontend owns protocol-local objects, ordering, selections,
  grabs, and client-visible protocol semantics;
- the WM owns private policy state but only proposes changes to Engine;
- portal policy owns grant decisions while the executor owns bounded transfer
  execution; and
- the live backend owns kernel-facing objects while returning observations to
  Engine.

Single writer does not mean one global writer. It means one writer for each
fact. Moving X resource tables, portal payload state, WM data structures, or
native graphics handles into Engine would weaken rather than strengthen this
rule by erasing domain ownership.

A linearization point is the transition at which an operation becomes the one
authoritative result observers must agree on. Sophia has domain-specific
linearization points; it does not claim that every session operation is
globally linearizable.

For application visuals, proposal acceptance and candidate preparation are not
visibility. The live backend executes a prepared output submission and returns
a tagged observation. Page-flip retirement is the transition that promotes the
prepared candidate into committed visual and input state and permits successful
protocol feedback. Until then, the previous coherent committed state remains
authoritative.

This foundation controls split brain. A WM cannot believe its requested layout
is already displayed, a frontend cannot infer visibility from client traffic,
and a renderer cannot select scene policy merely because it owns a native
buffer.

## 4. CALM and the Coordination Budget

The CALM principle connects consistency with monotonicity. A monotonic
computation only accumulates conclusions as it learns more facts. A
non-monotonic computation may retract or replace an earlier conclusion and
therefore needs an ordering or coordination point.

Sophia should classify monotonicity inside an explicit generation or epoch.
Crossing that boundary may invalidate facts that were monotonic within it.

Examples of facts that can usually accumulate without a global visual commit
are:

- a fence for one immutable candidate has signaled;
- damage for one generation has expanded by union;
- a tagged backend capability observation has arrived;
- an append-only diagnostic outcome has been recorded; and
- another member of a fixed transaction group has become ready.

Examples of non-monotonic decisions are:

- one buffer replaces the currently displayed buffer;
- focus moves from one surface to another;
- z-order or workspace membership changes;
- a surface or namespace grant is removed or revoked;
- a resource becomes eligible for release; and
- one candidate is selected while competing or stale candidates are rejected.

The observations that motivate these decisions may arrive independently. The
decision itself must pass through the authority that owns the affected fact.

This foundation controls over-coordination. Engine does not need to serialize
every readiness bit, log entry, or protocol-local operation. It serializes the
non-monotonic decisions that change visual truth, security authority, routing,
or resource lifetime. The result is a coordination budget: centralize only
where accepting one conclusion invalidates another.

## Worked Example: Atomic Resize

Consider a managed X11 surface moving to a new allocation:

1. Engine sends an opaque layout snapshot to the WM. The snapshot is an output
   of the Engine automaton, not shared scene state.
2. The WM returns a transaction-tagged size and placement proposal. Engine is
   the single writer and may reject it if the surface generation or workspace
   is stale.
3. Engine routes an accepted configure intent to the X frontend. The frontend
   owns the native X11 configure sequence; delivery does not commit pixels.
4. The frontend later emits a candidate containing matching geometry, storage,
   damage, readiness, and source generation. Readiness observations may
   accumulate monotonically for that immutable candidate.
5. Engine validates the complete candidate against the committed baseline and
   prepares an output frame. Selecting the candidate over the previous buffer
   is non-monotonic and remains Engine-owned.
6. The renderer and live backend execute the immutable plan. Import or KMS
   failure returns a tagged observation and leaves the previous committed
   visual state intact.
7. Page-flip retirement linearizes the visible change. Engine promotes the
   matching prepared candidate, advances input geometry with it, and emits
   successful presentation feedback.
8. A late completion, timeout, disconnect, or older authority observation is
   reduced against transaction and generation identity. It cannot overwrite
   the committed result.

I/O automata define the boundaries in this example. Single-writer authority
defines who may decide. CALM identifies which intermediate facts can arrive
without coordination. A transition-system model checks that every possible
ordering preserves the same safety properties.

## Safety and Liveness Properties

Safety states what must never happen. Core candidate properties are:

- a stale transaction never changes committed state;
- displayed client geometry has matching committed pixels;
- a focused surface is current, visible, focusable, and authorized;
- a buffer is not released while committed, submitted, or otherwise
  referenced;
- page-flip retirement promotes only the exact submitted candidate;
- a failed or timed-out visual transition preserves the last coherent commit;
- a cross-namespace operation cannot complete without a current matching
  grant; and
- protocol-local identity never becomes ambient Engine or WM authority.

Liveness states what must eventually happen under named assumptions. Useful
candidate properties are:

- an admitted ready transaction eventually commits or receives a terminal
  rejection, assuming the output and scheduler continue making progress;
- every submitted frame eventually retires or receives a bounded failure,
  assuming the backend reports completion;
- a WM timeout cannot block later layout work;
- a disconnected authority eventually settles or releases its owned
  transactions and resources; and
- revocation eventually prevents new use without requiring unrelated
  namespaces to stop.

Liveness claims must name their fairness and hardware assumptions. Sophia
cannot prove that a failed GPU, dead kernel, or permanently stopped process
will make progress. It can require that such failure becomes bounded,
observable, and unable to corrupt already committed state.

## Relation to State-Action-Model

In the original State-Action-Model pattern, an Action translates an event into
proposed values, the Model alone accepts or rejects those values, and State is
a derived representation of the Model plus the predicate for any next action.
That vocabulary resembles a Sophia boundary:

| SAM term | Limited Sophia analogy |
| --- | --- |
| Action | frontend translation, WM proposal, portal decision, or another bounded proposer |
| Model | the authority that alone accepts changes to its owned facts |
| State | an immutable snapshot, outcome, or set of permitted next effects derived after reduction |
| Next action | an explicit effect whose result returns later as a new observation |

The analogy stops there:

- Sophia has multiple authority-local state machines, not one global Model.
- Engine hot paths use explicit tables, systems, queues, and transactions, not
  a universal SAM loop.
- A protocol response may be a proposal; it is not authoritative merely
  because request/response wiring delivered it.
- External effects cannot re-enter and mutate state invisibly. They return
  tagged observations through the owning transition coordinator.
- TEA remains useful for deterministic policy reducers, as described in
  [Data-Oriented Design](dod.md#tea-where-it-applies), but neither TEA nor SAM
  is imposed on rendering, hit-testing, frame scheduling, or backend execution.

SAM also compares Action, Model, and State to Paxos proposer, acceptor, and
learner roles. For Sophia this is only an analogy. Engine is the single visual
acceptor; it does not run ballots, quorums, or replicated consensus. Consensus
would become relevant only if Sophia attempted to replicate authoritative
visual state across independently failing Engine instances, which is not the
architecture.

## Feature Review Checklist

Every new boundary, packet, or stateful feature should answer:

1. **Who is the sole writer?** Name the authority and the exact fact it owns.
2. **Is the update monotonic within its generation?** If not, name the
   serialization or decision point.
3. **Which automaton receives and emits it?** Define identities, stale handling,
   rejection, timeout, disconnect, and backpressure.
4. **Which safety or liveness property covers it?** State the failure that must
   remain impossible or the progress that must remain bounded.

Additional warning signs are:

- two components can independently decide the current value of one fact;
- a cache can be mutated without an authoritative generation;
- an effect can update state without returning through a typed observation;
- a protocol-local object or sequence becomes required Engine knowledge;
- candidate, submitted, and committed state are represented by one mutable
  record;
- an unrelated monotonic observation must wait for a global transaction; or
- a liveness claim omits the failure and fairness assumptions on which it
  depends.

If the four answers are explicit and the warning signs are absent, Sophia can
add mechanisms without turning architectural freedom into shared-state
complexity.

## Primary References

- Jean-Jacques Dubray, [State-Action-Model](https://jdubray.github.io/sam/).
- Leslie Lamport, [The TLA+ Home Page](https://lamport.azurewebsites.net/tla/tla.html)
  and [Computation and State Machines](https://lamport.azurewebsites.net/pubs/state-machine.pdf).
- Nancy Lynch and Mark Tuttle,
  [An Introduction to Input/Output Automata](https://groups.csail.mit.edu/tds/papers/Lynch/CWI89.pdf).
- Maurice Herlihy and Jeannette Wing,
  [Linearizability: A Correctness Condition for Concurrent Objects](https://cs.brown.edu/people/mph/HerlihyW90/p463-herlihy.pdf).
- Joseph Hellerstein and Peter Alvaro,
  [Keeping CALM: When Distributed Consistency is Easy](https://arxiv.org/abs/1901.01930).
- Leslie Lamport,
  [Paxos Made Simple](https://lamport.azurewebsites.net/pubs/paxos-simple.pdf),
  for the consensus mechanism that the SAM analogy references but Sophia does
  not implement.
