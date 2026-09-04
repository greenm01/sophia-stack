# Engine Architecture

**Role:** normative Sophia Engine architecture.

This document defines the domains inside Sophia Engine, their ordered data
flow, and the precise scope of the compositor role. [Architecture](architecture.md)
defines system-wide process ownership. [Data-Oriented Design](dod.md) defines
the records and state discipline used across these domains. [Compositor
Graphics](compositor-graphics.md) defines the graphics pipeline for
Engine-owned visual content. [Multi-Monitor Per-Head
Composition](multi-monitor-composition.md) defines the target split between a
logical output scene and its physical-head render plans.

The domain boundaries in this document are normative. The Rust module map
describes the current implementation and may evolve without changing those
boundaries.

## Component And Role Terminology

**Sophia Engine** is the component name. It is the session's sole visual
authority and owns:

- physical input interpretation, focus, hit-testing, and routing;
- authoritative surface, output, and visual state;
- validation and atomic application of visual transactions;
- frame scheduling, damage, composition, rendering, and presentation;
- output topology, work areas, and scanout state.

**Compositor** describes a role inside Engine: combining admitted client
buffers with Engine-owned graphics into a frame and presenting it. The
compositor role begins with the authoritative visual snapshot and ends when
the frame is submitted for scanout.

Engine is therefore broader than a narrow composition pipeline, because it
also owns input, visual transactions, and output state. It is narrower than a
conventional all-in-one Wayland compositor, because it does not own application
protocol semantics, WM layout policy, portals, metadata policy, or general
shell policy. Those are separate Sophia authorities.

Use these terms consistently:

- **Sophia Engine** for the component or crate;
- **compositor core** for the visual-state-to-present portion of Engine;
- **composition pipeline** for frame construction and rendering;
- **Sophia session stack** for the complete multi-process product.

## Domain Map

The diagram is intentionally plain text so it remains readable on a TTY.

```text
 external facts and proposals
 ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
 │ live backend │  │ X frontend   │  │ Sophia WM    │  │ chrome and   │
 │ device facts │  │ surface facts│  │ blind layout │  │ portal facts │
 └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘
        └──────────────────┴──────────┬───────┴──────────────────┘
                                     ▼
 +------------------------- SOPHIA ENGINE --------------------------+
 |                                                                  |
 |  Runtime Driver                                                   |
 |  ordered intake, bounded work, transition orchestration          |
 |       │                                                          |
 |       ├───────────────┐                                          |
 |       ▼               ▼                                          |
 |  Input Domain     WM Boundary                                    |
 |  focus, repeat,   reduced facts out, validated proposals in      |
 |  pointer, routing       │                                        |
 |       │                 │                                        |
 |       └────────┬────────┘                                        |
 |                ▼                                                 |
 |  Output Domain + Authoritative Visual State                      |
 |  topology, work areas, surfaces, focus, stacking, damage         |
 |                │                                                 |
 |                │ immutable baseline / prepared candidate         |
 |                ▼                                                 |
 |  =================== COMPOSITOR CORE ==========================  |
 |  Frame Domain -> Render Planning -> Live Backend Boundary        |
 |  epochs/timing    display list,       import, render, submit,     |
 |  and retirement   damage, composition presentation observations  |
 |  =============================================================  |
 |                                                                  |
 +------------------------------┬-----------------------------------+
                                ▼
                       DRM/KMS and display output
```

Facts enter Engine as bounded, typed records. No external component mutates
Engine state directly. Engine validates proposals, retains the coherent
committed baseline, builds frames from one immutable prepared candidate, and
publishes committed visual state only after matching retirement.

## Domain Ownership

### Runtime Driver

The runtime driver is the ordered transition coordinator. It drains bounded
observations, routes them to the owning domain, advances Engine exactly once
per transition, and exposes effects for execution. It owns ordering and
backpressure, not the private policy or state of the domains it coordinates.

### Input

The input domain owns Engine-side device-event reduction, keyboard repeat,
pointer state, focus, hit-testing, and target routing. The live backend owns
kernel-facing device access; protocol frontends own application-facing input
encoding. Neither is allowed to choose Engine focus or hit-test results.

Pointer appearance is configurable presentation policy, not a backend constant.
Application frontends request bounded semantic shapes such as `left_ptr` or
`text`; trusted session or shell configuration selects the cursor theme and
nominal size. Engine resolves each shape to validated pixels, dimensions, and a
hotspot, with a deterministic built-in fallback when an asset is absent. The
backend receives only that resolved presentation payload for composition or a
cursor plane. A WM cannot gain raw-input, renderer, or KMS authority by styling
the pointer. Reproducible comparisons pin the same theme and size for every
stack.

### WM Boundary

Engine exports only reduced, metadata-blind policy facts to a WM. A WM returns
layout proposals. Engine validates surface identity, geometry, work-area
constraints, and transaction freshness before accepting a proposal. A WM
never mutates scene state and never becomes an application protocol authority.

### Output

The output domain owns logical topology, modes, scale, refresh state, work-area
reservations, variable-refresh policy, and presentation capabilities. A work
area is protocol-neutral output state. Bars, panels, or protocol frontends may
propose reservations, but no one client or WM defines Engine output geometry.

### Visual State

Visual state is the private authoritative scene model: admitted surfaces,
placement, stacking, focus, Engine-owned chrome, output association, and
damage-relevant generations. State changes become visible atomically. Rendering
must not observe a partially applied surface or WM transaction.

Pending authority transactions are not persistent scene records. A queued
presentation retains its exact surface candidate, while the committed scene
retains protocol-neutral geometry, buffer, damage, and generation. Preparing a
presentation joins that one candidate to the committed baseline; it does not
relabel or recommit unrelated surfaces under the presentation transaction.

Layout recovery also keeps candidate evidence separate from committed state.
The epoch domain reduces complete authority observations into a passive record
of source transaction, natural extent, evidence class, and observation
sequence. During admission, a complete presented buffer outranks an accumulated
backing snapshot. The record can constrain a blind-WM replan, but only its exact
transaction may supply admission pixels and only retirement may promote it.
The complete state diagram is in
[Configure, Visual-Candidate, And Rendering State](architecture.md#configure-visual-candidate-and-rendering-state).

For decorated managed surfaces, visual state retains the WM-owned outer
allocation and Engine-derived client-content geometry as one committed fact.
The chrome clearance is stable across focus changes. A style-width reload that
changes clearance follows the same prepare/commit/rollback path as a client
resize; render planning cannot combine candidate chrome with old client
geometry.

### Frame

The frame domain owns immutable output-damage snapshots, frame epochs,
scheduling decisions, page-flip state, presentation feedback, and resource
retirement. Its snapshot combines ordered client generations and geometry,
Engine display-list nodes, and software-cursor bounds without protocol
metadata or renderer handles. It converts visual changes and
backend observations into bounded frame work without absorbing renderer or
protocol state. Its output-scoped service reducer observes one immutable
record per output and emits named retirement or submission effects. The live
backend executes those effects and reobserves native state; it does not infer
scheduling policy from aggregate readiness booleans.

The target multi-monitor frame domain derives a target-independent logical
output scene and then one immutable plan and damage ledger for each physical
head. A logical-output cohort joins those ledgers for mirroring. Physical head
identity remains private to Engine and backend coordination and never becomes a
WM policy fact.

The presentation queue, prepared Engine candidate, and submitted output frame
are separate lifetimes. A matching page-flip retirement is the only observation
that can satisfy presentation for its output. Engine promotes visual and input
state and permits successful protocol feedback only when their applicable
output-retirement requirements are complete.

### Render Planning

Render planning lowers one immutable visual snapshot -- either the committed
baseline or a validated prepared candidate -- into a target-independent
logical output scene, then lowers that scene into renderer-neutral work for
each required physical head: selected client-content variants, placements,
Engine-owned display-list primitives, clips, opacity, and head-local damage.
It never observes a partially updated scene and does not perform protocol
requests, layout policy, or kernel I/O. It must not flatten a logical output
before the head split or derive another head's work from a distinguished
primary render target.

### Live Backend Boundary

The live backend boundary converts planned work into native import, rendering,
and DRM/KMS submission operations, then returns typed observations. Backend
handles and graphics API objects remain below this boundary; they must not leak
into Engine's authoritative domain records.

## Atomic Visual Path

Every visible update follows one direction:

```text
observations/proposals
        │
        ▼
 validate and reduce
        │
        ▼
 accepted state / prepared visual candidate
        │
        ▼
 frame decision -> output scene -> per-head plans -> backend execution
        │
        ▼
 tagged presentation observation
        │
        ▼
 retirement validation
        │
        ▼
 committed visual snapshot + input/feedback/resource effects
```

This ordering is the architectural invariant behind flicker-free multi-window
updates. Domain-local nonvisual state may advance earlier under its owning
contract. Visual geometry, buffers, focus, hit-testing, successful presentation
feedback, and retirement-dependent resource release cannot advance from a
prepared candidate. Input, output reservations, WM layouts, surface buffers,
and Engine-owned chrome join one immutable candidate before frame planning and
become committed visual truth only through matching retirement. Presentation
is output-scoped; Sophia does not claim a globally simultaneous multi-output
retirement instant. Subsystem-specific shortcuts around that path are
forbidden.

A logical output may be backed by more than one head, which is what output
mirroring is. Preparation is a group barrier: no head may submit a generation
until every required head has a valid native candidate for it. Presentation is
then **primary-owned and independently paced**: the configured primary head's
callback retires the logical output generation, while every other head takes
the newest completely prepared generation on its own vblank and coalesces any
intermediate generations it missed. Each head's target stays leased until that
head has moved off it, so physical release of a generation remains a last-head
barrier even though logical presentation does not. Distinct logical outputs
also remain independent on their own page-flip timelines.
Sophia continues to claim no globally simultaneous multi-output retirement
instant. The scene-to-head fan-out and client-content quality contract are
defined in [Multi-Monitor Per-Head
Composition](multi-monitor-composition.md).

A head that disappears mid-flight drops its lease without counting as a flip,
and the candidate fails closed instead of committing a partial group; a
surviving-head topology is a new candidate, not a salvaged one.
`validation/tla/VisualRetirement.tla` carries the topology-retirement layer.
`validation/tla/MirrorHeadPacing.tla` checks independent successor selection,
primary progress, bounded lag, and last-head generation retention.

## Current Rust Module Map

| Domain | Current `sophia-engine` modules |
| --- | --- |
| Public facade and orchestration | `lib.rs`, `engine/`, `session.rs` |
| Runtime driver | `runtime_driver/` |
| Input | `input/` |
| WM boundary | `wm.rs`, `wm_policy.rs`, `engine/wm_transaction.rs` |
| Output | `output.rs` |
| Visual state and layout application | `visual_state.rs`, `engine/layout.rs` |
| Frame lifecycle | `frame/` |
| Render planning | `render.rs`, `engine/rendering.rs` |
| Live backend boundary | `backend_assembly.rs`, `live_backend.rs`, `drm.rs` |
| Engine-owned chrome records | `chrome/` |

This map is transitional rather than a promise that each domain will always be
one file or directory. Domain cohesion and ownership determine module splits;
file length is only a warning signal.

Live-session supervision and external-socket coordination live in
`sophia-session`, outside the command-line crate. Reusable visual transition
logic belongs in Engine; process launch and recovery belong in the session
layer; user-facing command selection and presentation belong in `sophia-cli`.
The session library reports evidence through a host-installed callback and does
not print directly, so extracting lifecycle ownership does not create a second
presentation boundary.

## Data-Oriented Constraints

Engine domains follow the project-wide DOD rules:

- exchange immutable packets, snapshots, typed IDs, generations, and effects;
- keep authoritative state private to exactly one owning domain;
- use bounded batches and explicit overflow behavior;
- keep hot transition paths contiguous and allocation-aware;
- do not pass behavior-rich protocol, WM, renderer, or backend objects across
  domain boundaries;
- test the records and transitions at boundaries rather than coupling tests to
  internal object graphs.

These constraints permit native X today and future admitted protocol
frontends, as well as different rendering and display backends, without
duplicating the visual authority or composition pipeline.
