# Engine Architecture

**Role:** normative Sophia Engine architecture.

This document defines the domains inside Sophia Engine, their ordered data
flow, and the precise scope of the compositor role. [Architecture](architecture.md)
defines system-wide process ownership. [Data-Oriented Design](dod.md) defines
the records and state discipline used across these domains. [Compositor
Graphics](compositor-graphics.md) defines the graphics pipeline for
Engine-owned visual content.

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
 |                │ atomic committed snapshot                       |
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
Engine state directly. Engine validates proposals, applies accepted changes to
private authoritative state, publishes a committed visual snapshot, and builds
frames only from that snapshot.

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

### Frame

The frame domain owns frame epochs, scheduling decisions, page-flip state,
presentation feedback, and resource retirement. It converts visual changes and
backend observations into bounded frame work without absorbing renderer or
protocol state. Its output-scoped service reducer observes one immutable
record per output and emits named retirement or submission effects. The live
backend executes those effects and reobserves native state; it does not infer
scheduling policy from aggregate readiness booleans.

### Render Planning

Render planning lowers one committed visual snapshot into renderer-neutral
composition work: client-buffer placements, Engine-owned display-list
primitives, clips, opacity, and damage. It does not perform protocol requests,
layout policy, or kernel I/O.

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
 private state transition
        │
        ▼
 committed visual snapshot
        │
        ▼
 frame decision -> render plan -> backend execution
        │
        ▼
 presentation observation -> retirement
```

This ordering is the architectural invariant behind flicker-free multi-window
updates. Input, output reservations, WM layouts, surface buffers, and
Engine-owned chrome join the same committed snapshot before frame planning.
Subsystem-specific shortcuts around that path are forbidden.

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

Some live-session supervision and external-socket coordination currently
remain in `sophia-cli`. That is an implementation seam, not permission for the
CLI to acquire visual authority. Reusable visual transition logic belongs in
Engine; process launch, recovery, and user-facing commands belong in the
session/runtime layer.

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
