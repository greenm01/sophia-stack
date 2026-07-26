# Project Hagia

**Status:** design note for future work

## The Name

Hagia is a working name for a Sophia-native descendant of Triad. The reference
is Hagia Sophia: Holy Wisdom. The name gives the new project a clear family
resemblance without pretending that it is still the same program.

## The Decision

Triad should remain a window manager for River. River is not an incidental
backend in Triad today; its window-management protocol, input facilities,
protocol surfaces, output management, and session behavior have shaped the
program. Keeping Triad on that foundation protects a useful project and avoids
turning every Sophia experiment into a compatibility burden for River users.

Hagia should begin as a fork. It may retain Triad's data-oriented model, layout
algorithms, tags, configuration language, Janet support, and shell-facing ideas,
but it should be free to divide responsibilities according to Sophia's
authority boundaries. The two projects may continue to exchange improvements
where their designs still agree. They need not share an abstraction merely to
make that exchange easier.

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

## Questions the Fork Exposes

### Tags, Workspaces, and Views

Triad permits richer membership than Sophia's present
one-surface/one-workspace model. Sophia should not adopt Triad's tag bit mask as
a universal desktop abstraction.

A better candidate is an opaque **view projection**. Hagia would keep tags as
private policy state. For a particular output, it would request a visible set
of eligible surfaces and provide their placements. Engine would validate the
request and retain final authority over visibility and focus.

This model could express tags, conventional workspaces, scrolling collections,
freeform desktops, and single-application sessions without naming any of them
in the Engine contract.

Questions to settle include:

- whether a view needs a stable Sophia ID or can remain policy-private;
- how Engine restores the last committed projection while policy is absent;
- how a surface appearing in several policy tags is represented without
  duplicating the surface;
- which component owns the output-to-view association;
- how dynamic creation and removal remain bounded.

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

## A Possible Progression

### 1. Geometry Proof

Run Hagia as a blind external policy process. Support manage, relayout, remove,
registered actions, placement, sizing, focus, and restart. Carry over a small
selection of layouts, including one scrolling layout and one Janet layout.

Do not make metadata rules, pointer operations, shell overlays, or full Triad
IPC prerequisites for this proof.

### 2. Daily-Driver Spatial Policy

Develop the view model needed for dynamic tags and richer visibility. Add
bounded pointer interactions, stable restoration, state requests such as
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

## Relationship to Triad

Triad remains the River window manager. Hagia is the Sophia-native descendant.
The fork may begin with shared history, but its purpose is architectural
freedom, not a permanent downstream patch set.

Good layout mathematics, reducer fixes, configuration improvements, and
language-independent test cases may travel in either direction. River protocol
work stays in Triad. Sophia authority and native-policy work stays in Hagia.
When a useful change cannot cross that line cleanly, duplication is preferable
to a false common abstraction.
