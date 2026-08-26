# Sophia Window Manager API

**Role:** normative spatial-policy boundary and compatibility contract.
**Status:** API v7 and the experimental `sophia_wm_v1` Hagia session path are
both implemented. API v7 remains the compatibility fallback during migration;
the public interface is not stable.

Sophia has one spatial-policy role. A native WM speaks that role directly. A
classical X11 WM speaks only to the private synthetic X server inside
`sophia-x11-wm-bridge`; the bridge translates its policy into the same Sophia
interface. Neither path is an application authority or an alternate
compositor.

The language-neutral endpoint, framing, versioning, and stability rules are in
[Sophia Policy IPC](sophia-policy-ipc.md). This document defines the policy
facts allowed across that connection.

## Ownership

Engine owns physical input, shortcut matching, authoritative visibility and
focus, scene validation, atomic commit, rendering, and scanout. The session
owns endpoint admission, process supervision, application launch, logout, and
protocol-specific polite-close execution. A WM owns only its private policy
model and proposes bounded changes.

A WM never receives:

- physical input streams, grabs, client sockets, or protocol handles;
- XIDs, namespaces, titles, classes, PIDs, paths, or credentials;
- client pixels, renderer or DRM handles, or portal payloads; or
- another WM's tags, views, trees, columns, stacks, or checkpoint.

Engine preserves the last committed projection while policy is absent,
incompatible, malformed, timed out, or restarting.

## Form-Neutral Policy

“WM” names the spatial-policy slot, not a required desktop form. A client may
tile, scroll, stack, float, combine those approaches, or implement a
single-application session. Sophia packets carry opaque identities,
capabilities, constraints, state, and geometry; they do not carry a tiling
tree, workspace array, tag mask, scrolling column, or global stacking model.

Shell, metadata, portal, and session powers remain separate. A combined
desktop may implement several clients, but independently authorized endpoints
are insufficient when those clients can collude through one process. The blind
spatial-policy role must occupy a different supervised protection domain from
metadata-bearing shell, broker, portal, and application-frontend roles. One
source tree or executable may launch those processes; they may not retain
ambient IPC that recombines their authority.

## Current Experimental API v7

The retained compatibility path implements `WM_API_VERSION = 7`. It uses the
common Sophia frame version 1 and has these negotiated capabilities:

- registered opaque bindings;
- nine Engine-owned workspaces and output/workspace activation;
- opaque session actions; and
- bounded Engine-rendered focus-ring and frame policy.

The WM creates the present filesystem socket and Engine connects. `WmHello`
carries a client-selected policy generation, capabilities, bindings, and
chrome policy. `WmSessionDescriptor` returns workspaces, active output mappings,
and session actions. Manage, relayout, remove, action, focus, and reduced
pointer-gesture requests receive command lists containing workspace assignment,
size, focus, placement, workspace activation, floating state, and session
actions.

API v7 remains supported only during migration. It is not the first public
interface, and its workspace ownership, client-hosted socket, and version
number must not be frozen as ecosystem architecture. The checked-in xmonad
compatibility profile no longer uses it: its runner selects `sophia_wm_v1` and
the bridge returns complete public output projections.

The live session selects the public path with
`--wm-interface=sophia_wm_v1`. That path uses the session-hosted endpoint,
complete snapshots and projections, canonical staged reducer, and connection
epochs described below. The selector is explicit so public-path failures
cannot silently downgrade to API v7.

## Public `sophia_wm_v1` Negotiation

The Sophia session hosts an owner-only WM endpoint and admits one supervised
client. `ClientHello` names `sophia_wm_v1`, the client's maximum revision, and
requested capabilities. `ServerWelcome` selects a supported revision and
capability subset and supplies a server-owned connection epoch plus effective
limits.

Capabilities are orthogonal and fail closed. The initial set covers registered
bindings, opaque session actions, reduced pointer interactions, and bounded
Engine chrome policy. Unsupported bits reject negotiation. An operation is
legal only when its selected revision and negotiated capability admit it.

Revision 2 separates action registration from physical shortcut ownership.
The policy client advertises a bounded catalog containing one nonzero opaque
action ID, a unique semantic name of at most 128 bytes, and an optional
session-operation slot. It never supplies keycodes or modifier masks. The
trusted session coordinator resolves the shortcut authority's prepared profile
against that catalog, then gives Engine only normalized chords and opaque IDs.
Unknown names, unavailable operation slots, unsupported pointer gestures,
excessive registrations, duplicate chords, and the emergency chord fail
closed. The slot, not an action-number range, associates a committed
activation with one session-owned capability.

Revision 3 adds the optional `profile_activation` capability without changing
normal snapshot/projection settlement. Before policy configuration or a first
snapshot, Sophia names one staged desktop-profile generation and fixed 32-byte
digest through prepare and activate commands. Hagia acknowledges only the exact
candidate it loaded from Sophia's owner-only fragment. Every command and
completion carries the connection epoch and a nonzero transaction; rejection,
timeout, disconnect, or rollback leaves the graphical startup gate closed.
Prepare, activate, and rollback outcomes use a profile-specific closed enum and
do not share the action/chrome configuration generation namespace. Every
`sophia_wm_v1` startup requires the capability and negotiates it before
graphical resources are opened. The former `--wm-profile-activation` proof
switch is accepted as a compatibility no-op; it cannot bypass or strengthen
the mandatory barrier. Supervised restart reattaches the same generation and
digest under a fresh connection epoch.

## Complete Scene Snapshot

Engine sends one generation-tagged snapshot containing:

- one explicit active output and up to 16 opaque outputs with their full and
  policy work rectangles;
- up to 1,024 live manageable opaque surfaces;
- current visible focus per output when one exists; and
- advertised opaque session-action tokens.

A surface record contains:

- `SurfaceId` and surface generation;
- broad toplevel, dialog, utility, popup, or unknown kind;
- reduced transient owner when valid;
- movable, resizable, focusable, closable, and fullscreen capabilities;
- frontend presentation requests and current committed state;
- minimum, maximum, and exact-size constraints; and
- current committed outer geometry.

The snapshot contains no workspace, tag, `ViewId`, application identity, or
policy-private layout state. It is complete even when some surfaces are hidden.
Chunking is a wire detail and cannot expose a partial snapshot to the policy
reducer.

Engine sends a fresh snapshot after a relevant scene or output change. One
request may also carry an opaque action activation, reduced focus request, or
bounded interaction request as its cause. The cause does not weaken snapshot
validation.

Each non-idempotent action activation is ordered and delivered once. A bounded
queue may reject new activations under pressure, but it must not merge repeated
focus, movement, view, or layout actions. Replaceable scene refreshes and
continuous interaction geometry may coalesce to their latest state.

## Projection Proposal

A request repeats the latest strictly admitted private policy generation. A
response repeats the connection epoch, transaction ID, and base snapshot
generation, and explicitly selects one live active output. It lists every
output affected by the decision. Each listed output contains a complete ordered
projection; list order is bottom-to-top stacking order.

Each projected surface may specify:

- requested client-content size;
- committed outer geometry;
- optional crop; and
- protocol-neutral transform.

The proposal may name one focused surface for each affected output. A surface
that was visible on an affected output and is absent from its replacement is
hidden. An unlisted output retains its committed projection. Moving one
surface between outputs therefore requires both outputs in the same proposal.

Engine validates the complete candidate before mutation:

- connection epoch, transaction, snapshot, output, and surface generations
  must be current;
- every output and surface must exist and remain authorized;
- counts, geometry, crop, transform, sizes, and constraints must be valid;
- no output or surface may be duplicated;
- a surface may appear on at most one output; and
- focus must name a visible focusable surface.

Changing the active output requires both the old and new output in the
affected set. Fullscreen geometry must equal the full output rectangle.
Ordinary, maximized, and minimized geometry must remain inside the work area,
and a minimized placement cannot hold focus. A minimized placement remains in
the semantic projection but is omitted from the renderer's layer candidate.

Success commits all affected outputs as one logical scene transaction.
Committed, stale, invalid, and timed-out outcomes are explicit. A rejected
proposal does not launch a process, change focus, partially change visibility,
or alter the last committed projection.

An accepted proposal may require frontend configure and content settlement
before it becomes committable. Engine retains the prior coherent scene during
that interval and sends `committed` only after authoritative frontend state and
renderable content match the candidate. If settlement changes a relevant fact,
Engine requests policy again from a fresh complete snapshot rather than
silently rewriting the proposal.

A committed policy response settles the cause that raised it, and the settlement
stands until snapshot-visible facts change. The owner requests policy again on a
fact change, never on a timer and never because the condition that raised the
cause still holds. A window manager may answer a surface's `Manage` request by
placing nothing at all -- a layout that shows one window does exactly that, and so
does a policy that maps a surface minimized. Those are answers rather than
failures, and re-asking them is not patience but a loop: the session commits, the
scene re-projects, and the same question returns on the next turn for as long as
policy keeps its layout. A surface waiting on such an answer is admitted by
whichever later proposal does place it, whatever cause raised that proposal, so
the settlement withholds a request rather than the surface.

For X11 surfaces, a public-policy presentation transition also crosses a
bounded frontend control. The frontend installs `_NET_WM_STATE` plus ICCCM
`WM_STATE`, emits selected `PropertyNotify` events, flushes them, and only then
acknowledges the control. That acknowledgement is a commit prerequisite. A
timed-out or invalidated proposal queues restoration of the last committed
frontend state; client `ChangeProperty`, `DeleteProperty`, and delete-on-read
cannot overwrite these Engine-owned feedback properties.

Physical outputs retire independently. The logical projection transaction
feeds the existing visual-preparation and retirement model; it does not claim
that separate displays flip at one simultaneous physical instant.

## Actions And Interactions

Engine matches registered physical chords before client routing, emits one
activation on the initial press, suppresses repeat activations until release,
and exposes only the action ID and reduced snapshot context. No rejected action
falls through to application input.

Session actions are advertised opaque tokens. A WM may request an advertised
token with an optional opaque target. It cannot supply an executable, argument,
environment, signal, or protocol handle.

The token is meaningful only under its session issuer, issuer/revocation
epochs, WM recipient epoch, operation class, and optional target generation.
It is not interchangeable with a broker, shell, or policy action carrying the
same integer representation. Activation identities are ordered and deduplicated
inside the recipient epoch; reconnect does not replay an ambiguous request.

An unmodified primary press on an unfocused visible surface may produce a
reduced focus request. Engine retains the ordered input handoff until Engine
focus and frontend protocol focus settle on the same exact generational
surface. Before release, every buffered target is revalidated against both the
last-presented interaction projection and current frontend route membership.
Timeout, removal, replacement, or route loss drops the complete handoff instead
of routing a prefix to stale focus.

Move, resize, drag, and scrolling interactions remain Engine-owned grabs.
Policy receives only the target, operation kind, and bounded geometry update
and returns an ordinary projection proposal. It never receives raw motion,
button payloads, device identity, or cursor authority.

A policy whose private state or validated configuration changes may request a
fresh cycle with a strictly increasing private generation. The request contains
no placement and cannot mutate Engine state; Engine replies with an ordinary
complete snapshot and projection request carrying the admitted generation. One
pending request coalesces by unioning its affected outputs. A newer generation
arriving during an in-flight relayout remains pending for one later complete
refresh; it is not duplicate-elided. Action activations retain their bounded
order.

## Recovery And Replacement

The connection epoch changes whenever the active policy process changes. Work
decoded from an older connection cannot commit after replacement, even if it
reuses a transaction or surface generation.

Disconnect, malformed transfer, timeout, and crash discard incomplete work and
leave applications and committed visuals alive. A replacement negotiates from
the beginning and receives a complete current snapshot. Engine does not store
or interpret the predecessor's workspaces, tags, views, or layout history.

A WM may maintain a private session-local checkpoint. It must reconcile that
checkpoint against the new snapshot and discard stale opaque IDs. Switching to
a different WM never transfers that checkpoint.

For the installed Hagia profile, the session provides an owner-only checkpoint
path inside the policy endpoint directory. Atomic replacement lets it survive
only supervised child restarts within that session; session teardown removes
it before endpoint cleanup. The checkpoint is implementation-private and is
not a `sophia_wm_v1` record or durable user configuration.

## Legacy X11 WM Profiles

The compatibility bridge owns synthetic-X lifecycle and generic translation.
A bounded profile supplies one named upstream WM/version, frozen configuration,
captured private-X request surface, and action map. Xmonad is the first profile,
not a universal compatibility claim.

Classical workspaces, layouts, and focus stacks remain inside the bridge and
its supervised WM. The bridge converts the selected visible synthetic windows
into the same complete Sophia projection used by a native policy client. It
cannot expose real X Authority clients or properties, draw application pixels,
receive raw input, execute arbitrary commands, or acquire metadata.

Every admitted profile passes the same semantic conformance suite as a native
client. Profiles that require real client metadata, global X server ownership,
or fake-server drawing are rejected rather than widening Engine or the bridge.

## Stability Gate

`sophia_wm_v1` remains experimental until the retained Triad desktop behavior
has been ported through Hagia's correctly separated policy, shell, session,
broker, and portal authorities. Only after that product port stops exposing
missing WM facts or operations do the independently implemented Hagia client,
X11 WM bridge, and C conformance client run identical negotiation, snapshot,
projection, action, focus, multi-output, rejection, timeout, restart, and
last-layout freeze tests. Those offline tests now pass, including an immutable
archived C99 client. Stability still waits for the retained physical output
apply/rollback archive recorded by the port ledger. Stable revisions remain
supported according to [Sophia Policy IPC](sophia-policy-ipc.md).
