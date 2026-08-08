# Sophia Window Manager API

**Role:** normative spatial-policy boundary and compatibility contract.
**Status:** API v7 is the current installed experimental implementation;
the draft `sophia_wm_v1` transport, codecs, reducer, and proof clients exist,
but its target semantics are incomplete and the interface is not stable.

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

The production tree currently implements `WM_API_VERSION = 7`. It uses the
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
number must not be frozen as ecosystem architecture.

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

Bindings contain one nonzero opaque action ID, normalized evdev keycode, and a
bounded modifier mask. Engine rejects duplicate chords or action IDs, invalid
modifiers, excessive registrations, and the emergency chord. Policy updates
use a strictly increasing client configuration generation and apply only at a
shortcut-idle boundary.

## Complete Scene Snapshot

Engine sends one generation-tagged snapshot containing:

- up to 16 opaque outputs with their full and policy work rectangles;
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

A response repeats the connection epoch, transaction ID, and base snapshot
generation. It lists every output affected by the decision. Each listed output
contains a complete ordered projection of its visible surfaces; list order is
bottom-to-top stacking order.

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
fresh cycle. The request contains no placement and cannot mutate Engine state;
Engine replies with an ordinary complete snapshot and projection request. One
pending request may coalesce by unioning its affected outputs, while action
activations retain their bounded order.

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

`sophia_wm_v1` becomes stable only after the independently implemented Hagia
client, the X11 WM bridge, and a C conformance client pass identical negotiation,
snapshot, projection, action, focus, multi-output, rejection, timeout, restart,
and last-layout tests. Stable revisions remain supported according to
[Sophia Policy IPC](sophia-policy-ipc.md).
