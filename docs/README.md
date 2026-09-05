# Sophia Documentation Map

Sophia documentation is divided by purpose. A document's role determines what
wins when prose disagrees.

## Proposed Constitution

- [Specification](specification.md) is the **unratified discussion draft** for
  project invariants, hard non-goals, compatibility admission, and amendment
  rules. It does not override the normative architecture documents yet.

## Orientation

- [Building on Sophia](building-on-sophia.md) is the map for third-party
  developers: which component owns what, which protocol each piece speaks, and
  how a window manager, a shell, or a full desktop environment composes from
  them. Start here; every section links to the document that owns its detail.

## Normative Architecture

- [Architecture](architecture.md) defines process ownership and the boundaries
  between Engine, protocol authorities, runtime, WM, portals, and chrome.
- [Engine Architecture](engine-architecture.md) defines the domains inside
  Sophia Engine, their ordered visual data flow, the current Rust module map,
  and the precise scope of the compositor role.
- [Compositor Graphics](compositor-graphics.md) defines the renderer-neutral
  display list, native primitive lowering, cached text strategy, damage rules,
  and Niri architectural reference for compositor-owned content.
- [Multi-Monitor Per-Head Composition](multi-monitor-composition.md) defines
  how one logical Engine scene becomes distinct native compositions for
  mirrored and extended display heads, including content variants, head-local
  damage, scheduling, and joined retirement.
- [Namespaces and Portals](namespaces-and-portals.md) defines session identity,
  admission, isolation profiles, capabilities, portal lifecycle, and
  cross-namespace failure behavior.
- [Data-Oriented Design](dod.md) defines the packet, snapshot, typed-ID, and
  private-state rules used across those boundaries.
- [Style Guide](style-guide.md) defines source-layout and implementation
  discipline.
- [Development Tooling](development-tooling.md) defines production/development
  dependency direction, canonical checks, and presentation ownership.
- [Configuration](configuration.md) defines the two KDL 2 ownership domains,
  source precedence, strict validation, and transactional hot reload.
- [Scripting Sophia](scripting.md) defines the unimplemented target contract
  for session-owned scripting, generic WM/shell integration, caller authority,
  namespace boundaries, and the proposed `sophia msg` CLI.

Normative documents describe both current and target contracts. They must label
unimplemented target behavior explicitly.

A developer implementing a native replaceable role starts with the [Sophia
Native Protocol Family](sophia-policy-ipc.md) for the common wire and lifecycle
contract, then reads the role-specific specification for the facts and
candidates that role may exchange.

- [Sophia WM API](sophia-wm-api.md) defines the versioned, metadata-blind native
  policy contract shared by Sophia WMs and legacy compatibility profiles.
- [Sophia Native Protocol Family](sophia-policy-ipc.md) is the developer entry
  point for the shared language-neutral envelope, negotiation, lifecycle,
  source hierarchy, evolution rules, and per-role stability discipline.
- [Sophia Indicator Descriptor](sophia-indicator-descriptor.md) defines the
  policy-authored desktop status carried on the layout commit, the bounds that
  cannot change later, and the rendering tiers that consume it.
- [WM v1 Freeze Surface](wm-v1-freeze-surface.md) enumerates which retained port
  rows can force a `sophia_wm_v1` layout change, what each expansion move costs,
  and the decisions that must be settled before the freeze forecloses them.
- [Triad Port Ledger Pointer](triad-port-ledger-pointer.md) locates the external
  freeze gate that lives in the Hagia repository and summarizes its row states.
- [Target-Resolved Input](target-resolved-input.md) defines presented-state
  target resolution, profile-scoped application arbitration, bounded capture,
  paced continuous values, independently authorized region-local coordinate
  disclosure, and revocation epochs for future shell interaction.

## Architecture Rationale

- [Architectural Alignment And Evidence Policy](architectural-alignment.md)
  assigns temporal, relational, arithmetic, and executable claims to their
  active validation gates and records what those gates do not prove.
- [Sophia and Wayland](sophia-vs-wayland.md) compares protocol boundaries,
  failure domains, input disclosure, and evolution policy without treating a
  typical implementation as universal or target Sophia behavior as shipped.
- [State and Transition Discipline](state-and-transition-discipline.md)
  explains how transition systems, I/O automata, single-writer authority, and
  CALM make Sophia's separated authorities manageable. It also records the
  limited explanatory relationship to State-Action-Model. Rationale explains
  the normative architecture but does not override it; current conformance and
  implementation gaps belong in the dated research log and admitted todo.

## Subsystem Contracts And Current Status

- [Sophia X Server Frontend](sophia-x-authority.md) records the native X11
  frontend boundary, implemented surface, and remaining production gaps.
- [Sophia Window Manager API](sophia-wm-api.md) defines the native,
  language-neutral spatial-policy protocol. Legacy X11 WMs are porting
  references, not supported Sophia policy clients.
- [Renderer Import Boundary](renderer-import-boundary.md), [Live Backend
  Dependency Policy](live-backend-dependency-policy.md), and [Live Session
  Bootstrap](live-session-bootstrap.md) define backend/runtime seams.
- [Installed Operations](operations.md) defines the supported installed-host
  boundary and the status, stop, recovery, fallback, and rollback procedures.

Subsystem documents may describe implementation details, but they may not
override the ownership and trust rules in the normative architecture.

## Evidence And Active Work

- [X11 Compatibility Matrix](x11-compatibility-matrix.md) is the admission
  record for native X11 client behavior.
- [Validation](validation.md) lists reproducible validation commands and gates,
  including the bounded temporal models under `validation/tla` and the
  complementary relational/arithmetic models under `validation/architecture`.
- [Active Roadmap](../todo.md) contains current milestone progress, incomplete
  work, and measurable exits.
- [Active Research Log](research-log.md) contains current investigations and
  retained evidence.
- [Project Hagia](project-hagia.md) is the design note for a standalone
  Sophia-native spatial-policy project.
- [Sophia Shell Interface Direction](sophia-shell-v1-direction.md) is the design
  note recording how `sophia_shell_v1` should be specified, the external shell
  evidence that method draws on, and how the experimental shell and WM
  contracts cooperate without sharing authority.

## Historical Material

- [Roadmap History](roadmap-history.md) archives completed milestones.
- [Roadmap Snapshot (2026-08-30)](roadmap-archive-2026-08-30.md) preserves the
  complete pre-cleanup active roadmap verbatim, including superseded ordering,
  detailed evidence narratives, and unresolved candidate work.
- [Research Log Archive](research-log-archive.md) preserves completed or
  superseded experiments.
- `research/xlibre/` preserves the retired XLibre prototype and its regression
  lessons outside the production workspace.
- `research/wayland/` preserves the retired Wayland frontend, tools, fixtures,
  and final subsystem contract outside the production workspace.

Historical documents are evidence, not current architecture. XLibre bridge
types, XComposite mirror paths, and prototype routed-input extensions must not
be cited as active Sophia interfaces.
