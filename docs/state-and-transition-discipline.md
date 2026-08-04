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

1. transition systems describe legal state changes and the safety and
   liveness properties that must survive every ordering; bounded TLA+ models
   provide executable checks for selected transition domains;
2. I/O automata describe independently owned components that communicate by
   observations, proposals, and outcomes;
3. single-writer authority and explicit linearization points prevent
   split-brain state; and
4. the CALM principle identifies which facts can flow without coordination,
   which decisions must be ordered by their owning authority, and which
   cross-authority decisions require coordination.

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

## Relationship to Current Status

This rationale is intentionally evergreen. Current conformance, implementation
gaps, and candidate hardening work belong in the dated
[Research Log](research-log.md) and the admitted roadmap in [Todo](../todo.md).
A gap recorded during an audit is not roadmap work until the todo admits and
orders it.

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
accepted or prepared state and explicit effects
          |
          v
external execution and tagged observation
          |
          v
terminal settlement, then authoritative snapshots and feedback
```

This is a reasoning model, not a requirement that every subsystem use one
framework or one global event loop. Engine hot paths remain explicit
data-oriented systems. A WM or portal policy reducer may use TEA style. A
protocol frontend retains the private tables and sequencing needed to honor
its native protocol. What stays constant is ownership: an effect cannot mutate
another authority's state, and its result returns as a new typed observation.

## 1. Transition Systems and Bounded TLA+ Formalization

A transition system describes behavior as a sequence of states connected by
named actions. TLA+ is one practical language for specifying those actions and
checking safety and liveness across many possible interleavings. Sophia carries
a bounded executable [visual-retirement model](../validation/tla/README.md);
the transition system remains the architectural foundation, while TLA+ is a
validation tool for selected domains rather than a universal Engine execution
model.

Sophia's visual lifecycle is naturally expressed in this form:

```text
proposed -> validated -> prepared -> submitted -> retired -> committed
              |             |           |
              +-----------> rejected <--+
                            or timed out
```

The diagram is intentionally abstract. Here, `committed` specifically means
committed visual state after retirement; an authority may accept domain-local
nonvisual state earlier when its own contract permits that. A real transaction
can contain several surfaces and output frames, and domain-local facts may
advance while its visual candidate waits. The essential distinction is that a
proposal, accepted state, a prepared candidate, a submitted frame, and
committed visual state are different facts.

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

Every cross-automaton contract should make the cases applicable to it explicit:

- identity and generation of the target;
- bounded size and cardinality;
- duplicate and retry behavior;
- stale or out-of-order handling;
- acceptance and rejection outcomes;
- timeout, disconnect, and restart behavior when work can outlive either end;
- backpressure when the receiver cannot accept more work; and
- exactly one relevant terminal settlement for every admitted item.

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
a tagged observation. Page-flip retirement is the output-scoped transition
that validates presentation of the exact candidate submitted for that output.
Engine may then promote state and feedback whose contract depends only on that
output. State or effects that span several required outputs wait until the
owning coordinator has settled every required, tagged output-retirement
observation. Sophia does not claim one globally simultaneous retirement instant
across outputs. Until the applicable retirement set is complete, the previous
coherent committed state remains authoritative wherever retirement is still
pending.

This foundation controls split brain. A WM cannot believe its requested layout
is already displayed, a frontend cannot infer visibility from client traffic,
and a renderer cannot select scene policy merely because it owns a native
buffer.

## 4. CALM and the Coordination Budget

The CALM principle connects consistency with monotonicity. A monotonic
computation only accumulates conclusions as it learns more facts. A
non-monotonic computation may retract or replace an earlier conclusion and
therefore must be ordered by the authority that owns the affected fact. It
requires cross-authority coordination only when the decision spans authority
domains.

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

This foundation controls over-coordination. Each authority orders only the
non-monotonic decisions that change facts it owns; Engine does not need to
serialize every readiness bit, log entry, portal decision, or protocol-local
operation. Coordination crosses authorities only where one decision must bind
facts from several ownership domains. The result is a coordination budget:
centralize only where accepting one conclusion invalidates another across the
same authoritative decision.

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
7. Each page-flip retirement linearizes the visible change for its output.
   Engine accepts only the observation for the matching prepared candidate. It
   advances input geometry and emits feedback when their applicable retirement
   requirements are complete. If the scene requires several outputs, full
   presentation is the conjunction of those output-scoped observations, not a
   globally simultaneous instant.
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
3. **Which automaton receives and emits it?** Define the applicable identity or
   generation, duplicate and retry behavior, and how every admitted item
   reaches exactly one relevant terminal settlement. A boundary need not
   expose dispositions that cannot occur in its contract.
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
