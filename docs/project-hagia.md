# Project Hagia

**Status:** design note for future work

## The Name

Hagia is a working name for a standalone Sophia-native spatial-policy project.
The reference is Hagia Sophia: Holy Wisdom. Its long-term purpose is to carry
Triad's useful policy and desktop experience from River to Sophia without
turning either Sophia or Triad into a compatibility layer.

## The Decision

Triad should remain a window manager for River. River is not an incidental
backend in Triad today; its window-management protocol, input facilities,
protocol surfaces, output management, and session behavior have shaped the
program. Keeping Triad on that foundation protects a useful project and avoids
turning every Sophia experiment into a compatibility burden for River users.

Hagia begins as a clean standalone repository. It carries no Triad history,
River or Wayland dependency, inherited binary, configuration surface, or build
scaffolding. A later Hagia milestone may deliberately port Triad's useful
data-oriented models, layout mathematics, tag semantics, Janet support, and
shell-facing ideas after they are reviewed against Sophia's authority
boundaries. That port is not current Sophia work, and the two projects need
not share an abstraction merely to make it easier.

## The Architectural Idea

Hagia should be Sophia's first demanding native policy client, not a privileged
replacement for Sophia Engine.

The useful part of Triad's design already resembles a Sophia policy component:
state enters as data, update functions transform the model, and layout
projection returns positions. Its logical IDs are distinct from River handles,
and its layout algorithms do not need to own client connections, rendering, or
physical input.

A complete Hagia environment would probably be a small family of components:

| Responsibility | Owner |
| --- | --- |
| Tags, layout structures, focus policy, scrolling behavior, and Janet layouts | Hagia spatial-policy process |
| Tabs, overview, switchers, previews, and other visible desktop furniture | Optional Hagia shell process |
| Hit-testing, physical input, animation, rendering, and scanout | Sophia Engine |
| Launching, locking, output configuration, capture, and data transfer | Sophia session services and portals |
| Application identity and metadata-based rule matching | A trusted metadata or classification broker |

This is not needless process splitting. These roles receive different
information and exercise different powers. Keeping them distinct allows Hagia,
xmonad, qtile, an Xfce-style environment, and a future all-Sophia desktop to use
the same protocol family without granting them all compositor authority.

“Sophia native” should therefore describe a family of narrow interfaces, not
one large engine protocol.

## What Can Survive the Fork

Much of Triad's policy core is worth carrying forward:

- the explicit model/update/projection flow;
- typed logical identities and generation-aware external mappings;
- tag-first organization and per-tag layout state;
- scrolling, algorithmic tiling, frame, BSP, and split-tree layouts;
- pure, validated Janet layout evaluation with native fallbacks;
- deterministic snapshots, reducer commands, and crash recovery;
- native IPC concepts, provided that each socket exposes only data its process
  is allowed to know.

River handles and Sophia surface IDs should remain adapter-local. Hagia's model
should continue to use its own stable logical identities.

## Where the Present Sophia Policy API Is Enough

Sophia's current policy API already provides opaque surface identities,
capabilities, state, constraints, output bounds, size requests, focus requests,
placements, crops, and atomic transactions. That is sufficient for a valuable
first experiment:

1. receive a complete Sophia layout snapshot;
2. reduce it into Hagia's policy model;
3. run a built-in or Janet layout;
4. return validated surface sizes, positions, stacking, crops, and focus;
5. reconnect after a policy restart and rebuild from a complete snapshot.

Ordinary tiling maps directly. Scrolling layouts also fit at the level of
geometry: Hagia can retain its virtual strip and viewport calculations, then
place or crop the surfaces that belong in the visible projection.

That first experiment should be deliberately plain. It need not reproduce all
of Triad before it can prove the boundary.

## Decisions From the Fork Review

### Tags, Views, and Output Ownership

Triad permits richer membership than Sophia's present
one-surface/one-workspace model. Sophia should not adopt Triad's tag bit mask as
a universal desktop abstraction.

A better boundary is a complete **output projection**. Hagia keeps tags and
stable `ViewId` values as private policy state. For a particular output, it
requests a visible ordered set of eligible surfaces and supplies their
placements. Engine validates the complete affected-output replacement and
retains final authority over visibility and focus.

This model could express tags, conventional workspaces, scrolling collections,
freeform desktops, and single-application sessions without naming any of them
in the Engine contract.

Hagia's initial private model is fixed as follows:

- every surface has a nonempty tag set and one home output;
- every view has a stable Hagia `ViewId` and nonempty selected tag set;
- each output owns an ordered view list, one active view, focus history, and
  reconnect affinity;
- a surface is eligible when its home output matches and its tags intersect the
  active view's selected tags; and
- Hagia resolves all membership to a projection in which a surface appears on
  at most one output.

Engine stores neither the tag sets nor `ViewId`. It preserves only the last
committed projection while policy is absent. Hagia may atomically checkpoint
its bounded private model in the session runtime directory, keyed to the
current Sophia session, and reconcile it with the full live-surface snapshot
after restart. A different WM receives no Hagia state.

Output removal migrates private views and surfaces to a surviving primary
output while retaining reconnect affinity. Output return may restore those
views when their identities still match. Mirroring is outside the first
protocol because it changes presentation, input, and resource semantics.

This combines River's non-monolithic policy boundary with Niri's useful
distinction between stable workspace identity and positional order. River's
classic tag intersection remains a private Hagia policy rule, not a Sophia
wire type.

### Application Rules Without Metadata Leakage

Triad rules can inspect application IDs, titles, PIDs, parent relationships, and
presentation hints. Sophia's spatial-policy process is intentionally blind to
those facts.

The likely answer is a trusted classification broker. Hagia could register
bounded rule predicates or refer to rules stored by the session. The broker
would evaluate private metadata and return an opaque result such as “rule 7
matched,” a placement class, or an approved policy label. Hagia would learn the
policy consequence, not the title, executable, namespace, or PID that produced
it.

This contract needs care. It must not become a metadata side channel expressed
as a conveniently verbose set of labels.

Parent and dialog relationships deserve separate treatment. A reduced opaque
parent surface may be useful for safe placement and focus, provided that the
frontend and Engine validate namespace and lifetime rules before exposing it.

### Pointer Operations

Triad supports interactive move, resize, drag, overview, and scrolling
operations. Sophia should not solve this by sending raw pointer streams or
compositor grabs to Hagia.

A bounded interaction protocol is a better fit:

- Engine performs hit-testing and begins an approved operation;
- Hagia receives an opaque target, operation kind, and reduced local geometry;
- Engine sends bounded motion updates rather than device events;
- Hagia proposes new layout targets;
- either side may commit or cancel the interaction;
- Engine retains routing, grabs, cursor ownership, and validation.

Keyboard and gesture bindings can continue to arrive as opaque registered
actions. The policy process need not know which physical device produced them.

### Animation

Triad currently keeps both target and current viewport offsets. In Sophia,
Hagia should normally send target geometry while Engine owns interpolation,
frame timing, damage, and presentation.

A future protocol may need bounded transition hints—duration class, easing
class, or immediate placement—but Hagia should not become a second frame
scheduler. The Engine must also be able to finish or abandon a transition when
Hagia crashes.

### Shell and Chrome

Frame tabs, BSP preselection, drop previews, overview graphics, recent-window
previews, and switchers are not merely spatial policy. They combine drawing,
hit targets, and, in some cases, application metadata.

An optional `hagia-shell` process could own those features through Sophia's
shell and display-list interfaces. It could ask Engine to turn a shell
interaction into an opaque policy action. That preserves a coherent Hagia
experience without quietly granting shell metadata to the spatial-policy
process.

The shell and policy sockets must remain honest about their contents. A
shell-facing snapshot may contain approved labels and icons; a policy snapshot
must not acquire those fields simply because both components come from the same
repository.

### Session Services

Triad presently performs work that belongs outside a Sophia policy process:
arbitrary process spawning, screen locking, screenshots, monitor power,
keyboard-layout changes, pointer warping, capture reporting, and output
configuration.

Hagia should request those operations through advertised session capabilities,
portals, or dedicated authorities. Application launching should use opaque
session tokens rather than executable paths and arguments supplied by policy.
Capture and screenshots should use portal decisions. Output and input
configuration should have their own authority and validation.

## Implementation Progression

### 1. Geometry Proof

Create Hagia as a standalone repository and run it as a blind external policy
process. Implement the public Sophia wire independently in Nim. During Sophia
protocol development, keep this client deliberately narrow: complete snapshots,
output projections, registered actions, placement, sizing, focus, removal, and
restart. Add an ordinary tiling layout, a scrolling layout, or a bounded Janet
layout only when a Sophia protocol milestone needs the independent proof.

The standalone repository's independent Nim envelope and record decoder passes
Sophia's retained valid and malformed corpus. Its proof client also passes one
authenticated complete snapshot/request/projection/outcome cycle through the
canonical Engine reducer. This remains a dormant boundary proof; private
tag/view policy is not yet part of a live session.

Do not make metadata rules, pointer operations, shell overlays, or full Triad
IPC prerequisites for this proof.

### 2. Daily-Driver Spatial Policy

Complete the tag-plus-view model, multi-output migration and return, bounded
pointer interactions, stable private restoration, state requests such as
fullscreen and minimize, and declarative transition targets.

This phase should prove that the policy process may crash or restart without
destroying applications, input ownership, or the last committed frame.

### 3. The Hagia Experience

Add the optional shell, trusted rule classification, session integrations,
portals, and carefully separated IPC projections. Reintroduce Triad features
only through the boundary that properly owns each one.

## Things Not to Do

- Do not turn Hagia into a second compositor hidden behind the word “native.”
- Do not expose Wayland objects, XIDs, client sockets, pixels, renderer handles,
  physical input streams, titles, classes, PIDs, or paths to spatial policy.
- Do not put Triad tags, columns, or trees into Sophia's universal protocol.
- Do not weaken Sophia's boundaries merely to preserve Triad configuration
  compatibility.
- Do not require Triad and Hagia to evolve in lockstep.
- Do not move shell metadata into the policy IPC because a combined program
  would find that convenient.

## Architectural Tests

Hagia is on the right side of the boundary if these statements remain true:

1. Hagia's spatial-policy process can crash and restart without taking
   applications or the desktop session with it.
2. Sophia Engine can replace Hagia with a tiling, scrolling, freeform, or
   single-application policy without changing its compositor authority.
3. An Xfce-style desktop can combine the same spatial-policy boundary with
   separate shell, metadata, portal, and session components.
4. Hagia can retain its own tags, trees, columns, rules, and configuration
   vocabulary without making them Sophia concepts.
5. No protocol grants a component data merely because an earlier monolithic
   window manager happened to possess it.
6. A C client and the Rust X11 WM bridge can implement the same public wire
   without importing Hagia, Nim, River, or Triad types.

## Relationship to Triad

Triad remains the River window manager. Hagia is its planned standalone
Sophia-native successor and eventual port, with independent history,
dependencies, releases, and compatibility policy. That migration begins only
after Sophia's policy boundary is stable enough to require it; until then,
Hagia remains a small independent conformance client.

Good layout mathematics, reducer fixes, configuration improvements, and
language-independent test cases may be deliberately ported in either direction.
River protocol work stays in Triad. Sophia authority and native-policy work
stays in Hagia. When a useful change cannot cross that line cleanly, duplication
is preferable to a false common abstraction.
