# Sophia Policy IPC

**Role:** normative target contract for external policy and shell interfaces.
**Status:** target architecture; the draft codecs, session-owned transport,
transfer reducers, and Engine projection reducer are implemented behind tests.
The installed API v7 path is unchanged, and the public interface is not stable.

Sophia exposes replaceable desktop policy through local, language-neutral IPC.
The protocol is the extension point. Hagia, the X11 WM bridge, and later shells
are ordinary clients of separately authorized interfaces; none has a private
Engine entry point.

## Ownership

The session runtime owns endpoint creation, peer admission, process lifetime,
and bounded transport I/O. Engine owns physical input, authoritative scene
state, proposal validation, atomic commit, rendering, and scanout. A policy
client owns only its private model and may submit bounded proposals.

Interfaces are role-specific. A process may connect to more than one interface
only when the session grants each role separately. Sharing an executable,
repository, or language never combines capabilities.

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

The client sends `ClientHello` first, naming an interface family, its maximum
supported revision, and requested capability bits. The server returns
`ServerWelcome` with the selected revision and capabilities, a server-owned
connection epoch, and effective limits. No other message is legal before this
exchange succeeds.

The connection epoch distinguishes process incarnations. The transport worker
tags every decoded item with that epoch, and every client proposal repeats it.
Closing or replacing a connection invalidates all partially assembled and
queued work from the earlier epoch.

## Versioning

The first public spatial-policy family is `sophia_wm_v1`. Revisions within an
interface are additive. A server sends only messages admitted by the selected
revision and capabilities. An incompatible redesign uses a new interface
family rather than changing an existing layout.

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

The draft Rust boundary is split deliberately. `sophia-runtime` hosts the
owner-only socket, authenticates the exact supervised peer, negotiates a
connection epoch, and assembles transfers. `sophia-protocol` owns generated
records and semantic conversion. `sophia-engine` owns the canonical atomic
projection reducer. None of these draft modules is selected by the installed
v7 session yet.

The dormant Rust reference client completes a full authenticated snapshot,
request, and proposal cycle over this transport. The generic X11 bridge's v7
response is covered through the canonical projection adapter. A standalone
C99 client, linked only to the retained C codec and the system C library,
completes the same session cycle and commits through the same reducer.

## Stable Spatial Semantics

`sophia_wm_v1` uses complete scene snapshots and complete affected-output
projections as its semantic baseline. Transport implementations may coalesce
or later encode equivalent deltas, but they must preserve that behavior.

A snapshot contains output geometry and all live manageable surfaces. Surface
records carry opaque identity, generation, capabilities, constraints, reduced
transient relationships, frontend presentation requests, and current committed
geometry and state. They contain no workspace, tag, view, layout-tree, or
application identity.

A proposal names its base snapshot generation and completely replaces each
listed output's ordered projection. Order defines stacking. Each placement may
request content size, outer geometry, crop, and transform. Visibility follows
projection membership; a live surface absent from every output is hidden.
Moving a surface between outputs requires both outputs in one proposal.

Engine validates the complete candidate before mutation. The active connection
epoch, transaction, snapshot generation, output and surface generations,
counts, capabilities, constraints, geometry, uniqueness, and focus must all be
valid. A surface may appear on at most one output, and focus must name a visible
focusable surface. Mirroring is a separate future capability.

Commit, stale rejection, invalid rejection, and timeout are explicit terminal
outcomes. Rejection, malformed input, transport failure, or policy restart
preserves the last committed projection. A replacement policy receives a full
Engine snapshot; it never receives its predecessor's private state.

Registered shortcuts and session operations use advertised opaque tokens.
Engine matches physical input and sends only the action token plus reduced
policy context. A client cannot provide executable paths, arguments, signals,
protocol handles, or raw input.

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
desktop experience, but that product migration is deferred. Its current role
is only to challenge this protocol as an independent client.

The first public revision is declared stable only after all three paths prove
negotiation, capabilities, complete and chunked transfers, actions, geometry,
constraints, visibility, multi-output moves, focus, stale rejection, atomic
failure, timeout, crash, restart, and last-layout preservation. Thereafter an
archived v1 client remains a compatibility gate for every release.
