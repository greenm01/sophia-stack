# Target-Resolved Input

**Role:** normative target contract for shell and Engine interaction.
**Status:** ratified pre-schema prerequisite; `sophia_shell_v1` remains deferred.

Sophia Engine owns physical input and resolves it against the interaction
snapshot belonging to the last frame actually presented. A shell commits
bounded target records with its immutable visual snapshot; it receives
resolved actions, not an undifferentiated stream of global pointer data. This
is a data-minimization boundary, not a claim that interaction reveals nothing.

This document fixes the semantic contract. It does not define wire
discriminants, add an Engine runtime, change Hagia, or introduce a toolkit or
reactive framework. Full immutable snapshots remain normative. A future delta
encoding is only a transport optimization and must name and validate its base
generation.

## Presented-State Resolution

Engine maintains separate committed, submitted, and presented scene slots.
Physical input is resolved only against the interaction snapshot paired with
the last presented pixels. A newer committed or submitted scene cannot receive
input until presentation advances to it. Engine may build private spatial
indexes or renderer-specific acceleration structures, but they are derived
state and never part of the public contract.

Each interactive region has an opaque, stable generational target handle, an
opaque action, geometry, authority, seat scope, and interaction kind. Handle
identity is explicit: Engine never infers reuse from equal geometry, action,
ordering, or content. A visual-only commit may retain a handle and its exact
meaning while changing precommitted visual variants. Replacing meaning or
geometry requires a new target generation.

A press captures the resolved presented target for one seat. Capture is
bounded to at most one target per seat. Release activates only when the
captured handle, action, owning authority, seat, and permitted modal scope are
still valid in presented state. Removal, regeneration, authority revocation,
seat removal, or a modal-scope change cancels once and clears capture. Stale
or cancelled capture can never activate.

Dismissal is modeled through modal scope and explicit target membership, not a
global invisible click shield. Regions outside an authorized modal scope do
not become shell targets merely to detect an outside click.

## Event Classes

Discrete targets emit coordinate-free actions by default. Activation says
which committed action was selected; it does not disclose the pointer path,
output-global position, scale, or unrelated geometry.

Continuous targets emit normalized values in a target-local domain. Engine
may update the corresponding shell-owned display-list visuals locally and
paces a replaceable update slot per `(seat, target)`. Motion and intermediate
values may coalesce into that one bounded slot. Discrete actions, capture
start, completion, and cancellation are ordered boundaries and cannot be
coalesced away. Completion or cancellation first preserves the final
normalized value, then terminates the stream.

Hover and pressed feedback choose among visual variants committed with the
target. A variant may change pixels or cursor presentation, but cannot change
target geometry, action meaning, modal scope, or authority. Continuous visuals
remain shell-owned display-list content; Engine selects or updates committed
parameters rather than acquiring widget semantics or styling authority.

## Exceptional Local Coordinates

Coordinate delivery is exceptional. A shell must commit an explicit grant on
a particular interactive region before Engine may disclose coordinates for
that region. Disclosed values are region-local and clipped to that region.
The grant does not authorize output-global coordinates, motion outside the
region, or coordinates for another target. Revoking or replacing the granted
region ends its disclosure and cancels any capture that depended on it.

This permits evidence-backed interactions such as a color field without
turning ordinary panels into global motion observers. The measurable default
is coordinate-free discrete events, bounded normalized continuous events, and
zero coordinate-bearing events without a live local grant.

## Stable Boundary And Reserved Projections

The public boundary contains passive snapshots, target handles, actions,
normalized values, capture boundaries, and narrowly granted local coordinates.
It explicitly excludes:

- reactive property graphs, callbacks, bindings, and observer machinery;
- widget-specific nodes such as sliders, buttons, graphs, or text fields;
- Engine text widgets, caret engines, layout trees, or toolkit services;
- inferred target reuse or identity derived from geometry or action equality;
- global click-shield targets or ambient outside-click observation; and
- Engine styling tokens, theme roles, widget state, or animation policy.

The same target identity is reserved for later keyboard-navigation and
accessibility projections. Those projections may associate separately owned,
bounded metadata with a target, but accessibility labels, traversal graphs,
and application identity do not enter the physical-input hot path here.

## Performance And Failure Contract

The contract bounds retained capture and continuous-update state; it does not
promise a universal refresh rate or latency on every GPU, output, or workload.
Pacing targets must be stated with a workload, hardware class, and measurement
method. Backpressure replaces intermediate continuous values but never drops
an ordered boundary or its final value.

Only effects explicitly declared optional may be omitted or degraded. Text,
controls, trust indications, and other mandatory content cannot disappear
silently. A shell-to-Engine DMA-BUF transfer might eventually reduce animated
texture cost, but it remains an unproven candidate until ownership, fencing,
damage, fallback, and measured power/bandwidth behavior are established.

## Formal Correspondence

`validation/tla/TargetResolvedInput.tla` models committed, submitted, and
presented interaction snapshots; generational handles; per-seat capture;
visual-only commits; replacement, revocation, modal scope, and local grants.
It corresponds to Engine's frame-presentation ledger and indicator targets.

`validation/tla/TargetInputPacing.tla` models the routed-input coalescer: one
replaceable continuous slot per seat/target and ordered capture, discrete,
completion, and cancellation records. Its scenarios include Noctalia-style
buttons, modal panels, sliders, and region-local continuous controls.

The models use scenario-driven Specula methodology to challenge coverage, but
remain small, hand-maintained Sophia models. Generated Specula scaffolding,
trace validation, and runtime instrumentation wait for a real shell runtime.
