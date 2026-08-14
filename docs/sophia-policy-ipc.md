# Sophia Policy IPC

**Role:** normative target contract for external policy and shell interfaces.
**Status:** experimental implementation. The codecs, session-owned transport,
transfer reducers, canonical Engine projection reducer, and supervised Hagia
session path are implemented. API v7 remains available during migration;
`sophia_wm_v1` is not yet stable.

Sophia exposes replaceable desktop policy through local, language-neutral IPC.
The protocol is the extension point. Hagia, the X11 WM bridge, and later shells
are ordinary clients of separately authorized interfaces; none has a private
Engine entry point.

## Ownership

The session runtime owns endpoint creation, peer admission, process lifetime,
and bounded transport I/O. Engine owns physical input, authoritative scene
state, proposal validation, atomic commit, rendering, and scanout. A policy
client owns only its private model and may submit bounded proposals.

Interfaces are role-specific, but endpoint separation alone is not an
isolation boundary. A **protection domain** is the set of processes that can
exchange authority through ambient IPC, shared writable memory, inherited
descriptors, debugging, or equivalent unsupervised channels. The session may
grant several compatible roles to one domain only when their composition does
not violate a blindness invariant.

The spatial-policy role may not share a protection domain with a
metadata-bearing shell, metadata/portal broker, or application frontend. A
desktop may reuse one executable, repository, or language by launching
separate supervised, sandboxed processes with no ambient cross-domain IPC;
those facts never combine capabilities. An Engine-rendered tier-0 indicator is
not a shell endpoint and does not create this conflict.

The spatial-policy interface never exposes XIDs, protocol objects, namespaces,
titles, classes, PIDs, paths, client pixels, raw input, renderer handles, portal
payloads, tags, or policy-private view identities. A shell interface may later
receive broker-approved presentation data, but that grant cannot confer focus
or placement authority.

## Dependency And Wire Policy

The wire specification is independently implementable. A client does not need
to link a Sophia library, use Rust, run a generator, or adopt Wayland, CBOR, or
another serialization runtime.

Sophia retains its fixed 24-byte little-endian envelope:

```text
u32 magic              "SOPH"
u16 frame_version      1
u16 message_kind
u64 transaction_id
u32 payload_len
u32 reserved           0
[payload bytes]
```

The frame version describes only this envelope. Each role negotiates its own
interface family and revision inside bounded handshake packets. Unknown frame
versions, message kinds, enum values, nonzero reserved fields, excessive
counts, truncated frames, and trailing bytes fail closed.

Stable layouts have one checked-in declarative schema. Repository tooling may
generate Rust and C codecs, documentation tables, and golden vectors, but the
generated outputs are checked in and normal builds do not run the generator.
The schema supports only fixed integers, opaque IDs, strict booleans, enums,
flags, bounded vectors, bounded bytes or UTF-8 text, optionals, records, and
tagged unions. It has no maps, recursion, floats, implicit defaults, or
unbounded fields.

The current draft schema is `protocol/sophia-wm-v1.kdl`. Its retained
[wire tables](generated/sophia-wm-v1-wire.md), Rust codec, allocation-free C99
codec, and valid and malformed corpora are one generated unit. Run
`tools/check_policy_protocol.sh` to reject generator drift and exercise both
language implementations against the shared bytes.

Snapshot output, surface, and binding records and projection output and
placement records are fixed-width schema entries. Output-record order carries
stacking order indirectly through each output's following placement count.
Revision 1 admits only the identity transform; later additive revisions may
name more transforms without introducing floating-point wire values.

Each snapshot surface names its current committed output; zero means hidden.
Each snapshot output names its visible focused surface, if any. Each projection
request carries the complete ordered set of affected output IDs, not merely a
count. These fields let a newly connected policy distinguish hidden surfaces,
reconstruct output membership and focus, and answer the exact atomic scope
selected by Engine.

## Endpoints And Admission

The Sophia session creates one socket per role beneath a mode-0700 session
directory in `$XDG_RUNTIME_DIR`. The WM path is advertised as
`SOPHIA_WM_SOCKET`; a future shell path will use `SOPHIA_SHELL_SOCKET`. Sockets
are owner-only. The session accepts only the expected supervised peer with
matching credentials and permits one active client for each exclusive role.

The client sends `ClientHello` first, carrying its minimum and maximum supported
revision and its requested capability bits. The server returns `ServerWelcome`
with the selected revision and capabilities, a server-owned connection epoch, and
effective limits. No other message is legal before this exchange succeeds.

`ClientHello` deliberately carries **no interface-family field**. The family is
already determined twice over — by the role socket the client connected to and by
the message kind itself — so a family field would add a permanent wire field with
no information gain. This is a settled decision, not an omission: adding one after
a revision freezes is impossible, so it is recorded here rather than left to be
rediscovered.

The connection epoch distinguishes process incarnations. The transport worker
tags every decoded item with that epoch, and every client proposal repeats it.
Closing or replacing a connection invalidates all partially assembled and
queued work from the earlier epoch.

Matching an expected UID and PID authenticates an endpoint; it does not prove
protection-domain separation. Before any metadata-bearing shell role is
installed, session supervision must also enforce the forbidden role
compositions above, close unneeded inherited descriptors, and apply the
project's chosen process-isolation mechanism. The current draft socket tests
cover exact peer admission only and make no sandbox claim.

## Versioning

The first public spatial-policy family is `sophia_wm_v1`. Revisions within an
interface are additive. A server sends only messages and record content admitted
by the selected revision and capabilities. An incompatible redesign uses a new
interface family rather than changing an existing layout.

### The Forward-Compatibility Rule

When `sophia_wm_v1` freezes, three clauses govern every later addition. They are
stated together because each one only holds if the others do.

1. **The frozen revision is final for record layouts and enum vocabularies.**
   Field order, field width, record size, and the set of admitted discriminants in
   any server-to-client enum or bitfield cannot change. Unknown discriminants are
   rejected, not skipped, and an enum value sits at a fixed offset inside a
   fixed-width record where no side channel reaches it. Client-to-server
   discriminants remain additive, because a server accepting more never breaks an
   older client.
2. **New WM-side facts arrive as capability-gated extension chunks.** An extension
   chunk carries a record kind from the reserved extension range **`0xFF00` through
   `0xFFFF`, in both the snapshot and projection transfers**, and is *not* counted
   by any `*Begin` count field, which is what keeps it out of the frozen message
   layouts. Ordinary record kinds stay sequentially allocated from 1, so the two
   ranges cannot collide. An extension chunk must carry at least one item, must
   append last so chunk ordinals stay dense, and must be sent only to a client that
   negotiated the governing capability. Receivers continue to reject unknown record
   kinds; that fail-closed behavior is preserved precisely because gating
   guarantees they are never sent one they did not ask for.

   The schema generator enforces this partition: an ordinary record declaration in
   `0xFF00`+ is rejected before bindings, facts, or corpora can be regenerated.
3. **New authorities take new interface families.** Shell, broker, portal, and
   session interfaces get their own family, their own role socket, and their own
   revision line. They never appear as placeholder messages inside the WM
   interface. The 24-byte envelope is role-neutral, so this costs the WM interface
   nothing.

#### Broker-issued classifications are extension-chunk content

Clause 2 is not hypothetical: it has one named first user. Broker-issued surface
classifications — the reduced, opaque form of what a host's window rules ask for —
cross as `(surface, classification)` records in a capability-gated extension chunk,
never as fields of `SnapshotSurface`.

The reason is that such rules are mostly parametric. A default workspace, a column
proportion, a named scratchpad, and a floating position are values, not classes, and
no bitfield or enum carries them. `SnapshotSurface.kind` and `capability_bits`
therefore stay reserved for what a surface *is*, and remain free for that purpose.

Two consequences follow, and both are the point:

- The classification vocabulary is not frozen with the revision. A rule family
  recognized after the freeze is a new record in the chunk, gated by the capability
  a client already negotiated.
- A client that negotiates nothing receives no classifications and behaves exactly
  as it did before they existed. Placement rules are advisory to policy; a WM that
  ignores them still produces a coherent desktop.

Ordinary record kinds stay sequentially allocated from 1 so they cannot collide with
the reserved range. Classifications never carry title, app ID, PID, path, or the
match expressions that produced them.

The rule depends on outbound capability gating. A producer that ignores the
negotiated capability set can send a frozen client something it must reject, which
would make clause 2 unsound and clause 1 unenforceable. Outbound gating is
therefore a prerequisite of the freeze, not an optimization, and one pinning test
must assert that the default capability set produces a byte-identical stream.

Because gated-off chunks are elided rather than emitted empty, adding a gated
extension chunk must leave the default-capability byte stream unchanged. That is
the property the pinning test checks.

Golden corpora pin the default capability set plus each capability in isolation.
They do not enumerate capability combinations; without that bound the corpus grows
multiplicatively with every new bit.

Once an interface revision is declared stable, later Sophia releases continue
to accept it. Old wire records are decoded at the session boundary and reduced
to the current internal model; Engine does not accumulate version branches.
Retiring a stable revision requires an explicit security amendment to the
project specification.

The existing Rust WM API v7 is an experimental implementation contract. It is
not `sophia_wm_v1` and receives no permanent compatibility promise.

## Bounded Transfers

One frame remains limited to 64 KiB. Complete snapshots and projections may
therefore use strict begin/chunk/end transfers. Every chunk repeats the
transaction and connection identity and carries an exact ordinal. The begin
record declares all category counts; the end is accepted only after those
counts and ordinals match exactly.

Each direction permits one transfer in flight and one coalesced latest pending
snapshot. Partial, duplicate, reordered, excessive, or timed-out transfers are
discarded without changing committed state. The first WM interface supports at
most 16 outputs, 1,024 manageable surfaces, and 256 binding registrations.

Coalescing applies only to replaceable scene refreshes and continuous reduced
interaction geometry. Non-idempotent action activations use a bounded ordered
queue and are never merged merely because they carry the same opaque action
token.

The Rust boundary is split deliberately. `sophia-runtime` hosts the
owner-only socket, authenticates the exact supervised peer, negotiates a
connection epoch, and assembles transfers. `sophia-protocol` owns generated
records and semantic conversion. `sophia-engine` owns the canonical atomic
projection reducer. The session selects this path explicitly with
`--wm-interface=sophia_wm_v1`; omitting the selector retains API v7.

The public session binds the endpoint before spawning its one policy process,
authorizes the exact child PID, and keeps blocking wire I/O in a bounded
worker. Each cycle sends a complete Engine snapshot and one exact projection
request. A validated proposal remains staged while frontend configure and
renderable-content obligations settle. Only the owner-loop commit promotes
that staged reducer successor and returns `committed`; timeout, invalidation,
disconnect, or replacement retains the previous complete projection.

The X frontend refines this generic settlement with protocol-visible
presentation state. Fullscreen, maximized, minimized, and restored candidates
do not promote until the corresponding `_NET_WM_STATE` and `WM_STATE` update
has been flushed and acknowledged. Rejection restores the prior frontend value.
No X atom or ICCCM/EWMH rule enters the public policy schema.

The dormant Rust reference client completes a full authenticated snapshot,
request, and proposal cycle over this transport. The generic X11 bridge's v7
response is covered through the canonical projection adapter. A standalone
C99 client, linked only to the retained C codec and the system C library,
completes the same session cycle and commits through the same reducer.

## Stable Spatial Semantics

`sophia_wm_v1` uses complete scene snapshots and complete affected-output
projections as its semantic baseline. Transport implementations may coalesce
or later encode equivalent deltas, but they must preserve that behavior.

A snapshot contains one explicit active output, output geometry, and all live
manageable surfaces. Surface
records carry opaque identity, generation, capabilities, constraints, reduced
transient relationships, frontend presentation requests, and current committed
geometry and state. They contain no workspace, tag, view, layout-tree, or
application identity.

A request carries the latest admitted policy-private generation. A proposal
names its base snapshot generation, explicitly selects one live active output,
and completely replaces each
listed output's ordered projection. Order defines stacking. Each placement may
request content size, outer geometry, crop, and transform. Visibility follows
projection membership; a live surface absent from every output is hidden.
Moving a surface between outputs requires both outputs in one proposal.

Engine validates the complete candidate before mutation. The active connection
epoch, transaction, snapshot generation, output and surface generations,
counts, capabilities, constraints, geometry, uniqueness, and focus must all be
valid. A surface may appear on at most one output, and focus must name a visible
focusable surface. One surface shared by two logical outputs stays inexpressible
and raises `DuplicateSurface`; mirroring does not work that way, as the output
logical-space contract below records.

### The Output Logical Space Is The Whole Contract

`SnapshotOutput` carries `output`, `generation`, `focus`, `bounds`, and
`work_area`. It carries no scale, transform, mode, connector identity, or enabled
flag, and it has no reserved space. That is deliberate and, once the revision
freezes, permanent: widening the record is a layout change, so every fact below
must reach policy through the logical rectangle or not at all.

- **Scale never crosses.** Engine owns display scaling and hands policy a logical
  space already expressed in it. A policy-side scale, such as Hagia's fixed-point
  column-width ratio, is a private layout parameter that happens to share the name.
- **Transform never crosses.** Rotation is absorbed by Engine presenting
  pre-rotated logical bounds. A rotated output is an output with different bounds.
- **Mode and timing never cross.** Refresh, pixel clock, and connector timing are
  output-authority concerns with no policy meaning.
- **Enablement is expressed by omission.** A disabled output is absent from the
  complete snapshot rather than present with a flag, which the permanent 16-output
  maximum keeps bounded.
- **Mirroring is invisible.** One logical output backed by N connectors projects as
  exactly one `SnapshotOutput`; no connector identity or head count crosses.

The pressure on this contract comes from the output authority, which handles every
one of those facts and will be tempted to publish them. It must not. A policy that
needs to know an output's mode has been handed the wrong abstraction.

An active-output change is valid only when the affected set contains both the
old and new output. Fullscreen placements equal full output bounds; all other
presentation states remain within the work area; minimized placements cannot
hold focus and do not enter the render-layer candidate. `PolicyDirty` admits
only a newer private generation and coalesces its nonempty live-output scope
without dropping a refresh that arrives during older in-flight work.

Commit, stale rejection, invalid rejection, and timeout are explicit terminal
outcomes. Rejection, malformed input, transport failure, or policy restart
preserves the last committed projection. A replacement policy receives a full
Engine snapshot; it never receives its predecessor's private state.

Registered shortcuts and session operations use advertised opaque tokens.
Engine matches physical input and sends only the action token plus reduced
policy context. A client cannot provide executable paths, arguments, signals,
protocol handles, or raw input.

### Continuous Policy Interactions

Revision 3's interaction vocabulary is permanently `Move`, `Resize`, `Drag`,
and `Scroll`. `Begin`, `Update`, `End`, and `Cancel` are the complete phase
vocabulary. Move, resize, and drag carry output-local geometry and use axis zero.
Scroll carries a signed delta pair in `interaction_x`/`interaction_y`, leaves
width and height zero, and identifies a horizontal or vertical source axis in
`interaction_axis`. A non-cancelled zero scroll is invalid; cancellation may
carry a zero delta because it terminates authority rather than motion.

This consumes the `u16` formerly named `reserved_cause` without changing the
fixed `ProjectionRequest` layout. The Rust semantic packet exposes the axis so
independent clients do not have to retain wire-only state. It is still reduced
policy input: Engine owns physical events, capture, pacing, and cancellation.
Replaceable `Update` values may coalesce only for the same exact target, kind,
axis, capture, and authority epoch. Ordered begin/end and ordinary cancellation
never coalesce. The live owner now applies that rule to Engine-captured move and
resize: it retains at most one latest queued update behind the in-flight request.
A security transition clears local capture, discards queued values for that
interaction, and prioritizes one `Cancel`; Hagia treats cancellation as a no-op on
spatial state rather than applying its payload as a final delta. A policy restart
also advances the locally observed capture epoch before physical input resumes.
Drag and scroll have fixed wire meanings but do not yet have live Engine producers.

An opaque action token is not a global `u64` authority. Acceptance is scoped by
issuer role and authority, issuer connection and revocation epochs, recipient
role and authority epoch, operation class, and—when the operation names
something—its opaque slot and generation. Each physical activation also has a
recipient-epoch-local identity for duplicate rejection. A broker-issued
toplevel action cannot be reinterpreted as a policy or session action; stale,
expired, revoked, wrong-recipient, wrong-generation, and duplicate activations
fail closed. The experimental public session mints fresh session-local tokens.
A committed binding explicitly names an optional operation slot; no numeric
action range has operation semantics. The session validates connection epoch,
the bound and advertised slot, opaque token, target permission, and exact
request identity, and executes the corresponding session-owned operation
only after the projection transaction commits. API-v7 tokens do not claim this
encoding.

## Shell Interfaces

The protocol family reserves a distinct shell role and endpoint, not placeholder
shell messages in the WM interface. `sophia_shell_v1` will be modeled and
specified only when retained shell workflows establish its smallest useful
display-list, hit-target, presentation-data, and action vocabulary.

Its minimum boundary is already fixed: Engine retains rendering and hit-testing;
brokers retain metadata sanitization; the shell receives only authorized
presentation facts and emits bounded shell proposals or opaque actions. Shell
authority cannot set application placement or focus, and WM authority cannot
acquire shell metadata.

Shell reservations also cannot race policy projection as independent commits.
The target transaction chain is shell candidate and reservation, exact Engine
work-area generation, exact WM snapshot and projection, then one coherent
logical presentation. A stale or unready member rejects the candidate while
the prior presented bundle remains intact. Security surfaces use their own
preemptive authority path and do not wait for an optional desktop shell or WM.

## Evidence Before Stability

Wire conformance requires identical golden vectors in generated Rust and C,
malformed-frame mutation coverage, and an independently compiled C client.
Semantic conformance runs the same black-box corpus against the Rust reference
WM, Hagia, and the X11 WM bridge.

Hagia is a standalone Nim repository with no Triad history or River/Wayland
runtime dependency. Its independent envelope and record decoder passes the
same retained valid and malformed corpus, and its proof client completes one
authenticated snapshot/request/projection/outcome cycle through the canonical
reducer. Hagia is the eventual Sophia-native port of Triad's useful policy and
desktop experience. It now also runs as the explicitly selected experimental
live policy client; broader product migration and protocol stability remain
deferred.

The first public revision is declared stable only after the retained Triad
desktop behavior has been ported through its correctly separated authorities
and no open retained workflow can still falsify the WM contract. All three WM
paths then prove negotiation, capabilities, complete and chunked transfers,
actions, geometry, constraints, visibility, multi-output moves, focus, stale
rejection, atomic failure, timeout, crash, restart, and last-layout
preservation. Thereafter an archived v1 client remains a compatibility gate for
every release.
