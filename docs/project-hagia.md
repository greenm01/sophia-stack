# Project Hagia

**Status:** active first native policy implementation

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

Hagia is a clean standalone repository. It carries no Triad history,
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

The trusted session now prepares the profile's terminal, browser, startup, and
logout selectors as a typed session candidate. Selectors resolve only against
the session's bounded registered-application catalog; executable paths remain
session-owned, ambiguous selectors fail, and explicit session command-line
mappings remain superior. Before a normal `sophia_wm_v1` session starts, the
coordinator also rejects any shortcut whose required session capability is not
available. Hagia receives only the resulting opaque operation slots.

The coordinator also prepares the input fragment as a typed startup candidate.
Keyboard RMLVO, repeat timing, and initial Caps Lock and Num Lock state overlay
Sophia's effective input configuration, while explicit CLI RMLVO values remain
superior. Pointer natural scrolling, acceleration profile and speed,
left-handed mode, middle-button emulation, and bounded wheel scaling lower to
a backend-owned libinput policy. Unsupported requested device settings reject
graphical startup; a later hot-plug configuration failure terminates native
input acquisition instead of silently accepting an ineffective profile. Live
input reload and device-scoped transactional rollback remain deferred with the
cross-authority activation protocol.

Output profile values now have a separate typed preparation boundary as well.
The coordinator validates bounded exact connector identities, preferred or
explicit modes, fixed-point scale, position, transform, enablement, startup
focus uniqueness, and VRR policy before staging fragments. Preparation neither
opens DRM nor mutates live topology. A pure authority-local reconciler can now
combine that candidate with an immutable capability snapshot, resolve rounded
refresh requests deterministically, and reject unknown/disconnected connectors,
unsupported scale/transform/VRR, overlaps, ambiguous modes, or an all-dark
result. The trusted snapshot and pre-I/O activation plan are now supplied;
atomic KMS testing, activation settlement, and rollback execution are the next
output-authority tranche.

One shared preparation function now validates the shortcut, session, input,
and output candidates at profile load, staging revalidation, and live-session
configuration. `sophia config check --desktop-profile=/absolute/path` invokes
that same read-only boundary without constructing a graphical session, so a
migrated profile can be checked by the trusted coordinator rather than only by
Hagia's structural parser.

Sophia now also carries the trusted pure activation barrier using the same
state transitions as Hagia's exhaustively checked reference model. It emits
deterministically ordered prepare, activate, and generation-wide rollback
effects for all seven authorities; preserves the active generation during
partial or rejected work; ignores stale completions; and prevents rejected
generation reuse. This is lifecycle parity, not live reload: no authority
protocol consumes those effects yet. An injected startup executor seam now
maps every effect to only its matching authority handler and converts the
result back into the exact typed reducer message. Production handlers remain
unpopulated, so transport and filesystem work are still absent from reduction
and no partial activation path exists.

The startup-only driver over that seam now completes the successful two-phase
barrier or cancels the remaining phase on first failure and runs
generation-wide rollback. Rollback continues past an individual recovery
failure so every participant receives cleanup; the returned error retains the
exact failed effect and pending-authority set. Tests enumerate every authority
as the prepare, activation, and rollback failure point. This deterministic
schedule refines the existing model-checked lifecycle without adding a live
reload interleaving.

The authority side now has one reusable pure participant transition model
rather than seven bespoke implementations. It retains active, candidate,
previous-active, latest generation, and the last admitted full key as bounded
logical state. Prepare consumes generations monotonically; activate and
rollback are exact-key idempotent; rollback restores the prior identity; older
cleanup is inert; and a digest collision at the current generation fails
closed. An unseen generation-wide rollback becomes a no-state tombstone when
the startup driver skipped that participant after an earlier prepare failure.
The coordinator-to-participant refinement tests now execute through seven
authority-local candidate slots rather than identity-only participant models.
They enumerate every authority as the prepare, activation, and rollback
failure, proving global/local identity and payload convergence plus exact
recovery divergence. The same executor also consumes the exact owner-safe
fragments produced by staging and proves that every admitted semantic payload
becomes active under the shared key. This model is ready for authority-owned
handlers, but does not install them. During transactional startup the graphical
launch gate can hide partial local activation. Live reload cannot use that
assumption and remains deferred until a separate global visibility and recovery
protocol is proved.

Startup candidate ownership is now narrower and DRY: Sophia validates the
generation/digest identity of all seven raw candidates once at the shared
preparation boundary and returns one typed shortcut, session, input, and output
bundle. Live-session assembly partitions that transient bundle once into
session, input, and output owner records plus the shortcut transfer payload;
policy setup creates the shortcut owner's slot without parsing the profile a
second time. The prepared-profile aggregate still returns the bundle beside the
raw provenance-bearing generation and exact activation key in one pass, but no
long-lived coordinator copy remains after partitioning.

Sophia now also mirrors Hagia's staged-candidate admission constraints at its
authority boundary without sharing implementation code. One loader consumes
the existing staged format into the canonical raw candidate DTO only when the
owner-safe file, assigned authority, and exact coordinator generation/digest
all match. This supplies a reusable handler input for local or process-backed
authorities while preserving Hagia's independent policy-reader evidence and
performing no activation.

The next reusable handler layer is also pure: a generic authority-local slot
couples one participant identity model with one active/candidate payload owner.
It prepares canonical typed candidates or an admitted staged fragment, rejects
authority/key/payload conflicts without mutation, promotes payload only with a
matching participant activation, and restores the previous payload on
rollback. This centralizes payload/identity synchronization without creating a
coordinator-owned copy of all authority state or wiring live effects.

Session-owned preparation now routes through one typed overlay rather than
mutating the trusted application registry inline. CLI application additions,
arguments, startup order, and action selectors remain superior to the desktop
profile; the canonical session candidate and overlay are applied to a clone and
fail without changing accepted state. This provides the deterministic local
prepare operation needed by a future session participant while leaving global
activation and watched reload disconnected. Trusted startup now instantiates
that participant's generic slot, prepares the canonical typed session payload,
and derives the effective application configuration from the retained slot.
The slot remains `Prepared` until a future global barrier owns promotion.
One shared prepared-slot constructor removes repeated initialization sequencing.
The public shortcut owner now uses it as well and resolves its registrations
from its own retained prepared payload. These remain separate authority-owned
slots, not a coordinator collection, and neither is promoted yet.

The transient typed startup bundle is now partitioned once. Session, input, and
output enter cohesive owner records backed by their separate prepared slots;
shortcut remains only until transfer to its public owner. Current
keyboard/pointer overlay, output reconciliation, and shortcut resolution read
the owner payloads, removing the long-lived centralized bundle without changing
activation or hardware behavior.

The public Hagia launch path now receives one prebuilt linear launch context.
Trusted startup creates it before display or device setup, stages and re-admits
all seven exact owner-safe fragments, and retains the prepared shortcut owner.
Later policy launch consumes that context instead of recreating directory or
fragment state. Early failure cleans it up before Hagia or graphical clients
start; no authority is activated by this change.

Sophia's synchronous coordinator driver now has a separate typed prepare-only
entry point. It drains all seven prepare effects, rolls the generation back on
any failure, and stops exactly at `Prepared` without activation. The full
startup driver reuses that entry point, preventing a second orchestration path
when the production owners are connected before graphical launch. Public Hagia
startup now makes that connection through a fixed-field dispatcher borrowing
the named authority owners. It runs before display, seat, device, or process
setup, retains the coordinator and participants at the exact prepared key, and
rolls every owner back if any position rejects. Its activation handler is
deliberately fail-closed; launch visibility, recovery, and watched reload remain
separate later milestones.

Protocol revision 3 now defines fixed-size, generated profile prepare,
activate, and rollback records plus typed Rust and C codecs. The matching Hagia
decoder is checked against the same golden corpus. Sophia deliberately does not
advertise the `profile_activation` capability yet: schema availability does
not claim that the external authority has prepared or activated a candidate.
An authority-local pure transport reducer now correlates each command with its
exact epoch, transaction, generation, digest, phase, and closed outcome. Stale
completions are inert, rejected activation requires an explicit rollback, and
disconnect discards the outstanding operation. Negotiation tests continue to
prove the capability is omitted even when a client requests it.
The runtime transport now also has an explicit startup-only opt-in constructor
and typed send/completion paths. A private Unix-socket test drives prepare,
activate, and rollback through the pure reducer with exact correlation. The
installed supervised-UID constructor remains non-opt-in, so this plumbing has
no production effect until the pre-graphics owner is restructured around the
global barrier.

The existing atomic scanout owner now exposes a read-only capability projection
for that adapter: stable Engine output identity, exact kernel connector name and
ID, bounded advertised timings, selected/default timing, and the result of VRR
property discovery. It reuses the already-owned libdrm device and selection;
there is no second sysfs topology reader and no ioctl that changes state.
A pure coordinator adapter now joins those facts to Engine outputs by stable
`OutputId` and constructs the immutable configuration topology DTO. It rejects
missing/duplicate identities and selected-mode/pixel-size disagreement,
preserves Engine semantic order, derives checked nonoverlapping current
positions, and advertises only capabilities the current backend implements:
integer compositor scale, normal transform, and VRR only after complete
property discovery. The migrated two-output candidate reconciles against this
projection in tests. A pure activation planner then joins the reconciliation
back to stable Engine `OutputId` values and retains each exact current state as
its rollback target. It rejects zero/duplicate outputs, connector aliases,
fabricated reconciliations, and any capability drift, while exposing no native
KMS handle. Native-session startup now performs projection, reconciliation,
and plan preparation immediately after the atomic owner is created and before
any graphical client launches; failure aborts startup. The result is explicitly
recorded as prepared but not applied. Atomic testing, multi-output apply, and
rollback execution remain deferred.

The output handoff now also has a pure authority-local settlement coordinator.
It retains the immutable plan only while an exact generation/digest attempt is
testing, applying, or rolling back; terminal settlement discards the plan.
Typed effect completions advance only in their matching phase, duplicate or
stale completions are inert, test rejection needs no rollback, and apply
failure requires an explicit rollback completion. A rollback failure preserves
both the activation and recovery causes. The backend executor is intentionally
not connected yet, so this milestone adds no KMS mutation or policy-only reload
path.

## Implementation Progression

### 1. Geometry Proof

Create Hagia as a standalone repository and run it as a blind external policy
process. Implement the public Sophia wire independently in Nim. During early
Sophia protocol development, keep the bootstrap client narrow: complete
snapshots, output projections, registered actions, placement, sizing, focus,
removal, and restart. That profile proves the boundary but does not define the
revision-3 feature ceiling. Revision 3 remains experimental until the retained
Triad behavior is ported across Hagia policy, Hagia Shell, Sophia services, and
the required brokers/portals. Additional retained layouts,
candidate-validated configuration, and bounded Janet behavior therefore remain
on the freeze path even though they need not block daily use of the bootstrap
profile.

The standalone repository's independent Nim envelope and record decoder passes
Sophia's retained valid and malformed corpus. Its proof client also passes
authenticated multi-cycle snapshot/request/projection/outcome exchanges
through the canonical Engine reducer. Hagia now has private nine-view policy,
ordered actions, scrolling columns, configuration, and a long-running client.
The native slice now also carries explicit output focus, column consume/expel,
bounded focus and minimized histories, floating/fullscreen/maximize/minimize
reduction, generational policy refreshes, and an owner-only, bounded,
atomically replaced session checkpoint. A restored checkpoint is only a
candidate: Hagia validates its private indexes and reconciles it against the
next complete Sophia snapshot before proposing anything. After that candidate
first commits, Hagia emits one geometry-free `PolicyDirty` request for the
complete live output set and verifies the resulting fresh cycle at the next
private generation. Sophia's installed physical-workload proof has passed for
this retained slice; the old xmonad session is regression evidence, not a
prerequisite. Broader Triad parity, configuration recovery, and the shared
revision-freeze corpus remain separate gates.

The live Sophia session supplies `HAGIA_POLICY_CHECKPOINT` inside its
owner-only policy endpoint directory. The file survives supervised Hagia child
replacement, then session teardown removes it before releasing the endpoint
directory. It is never transferred to another policy process or treated as
portable configuration.

From a logged-in tty4, `tools/hagia-proof` is the one-shot current-checkout
launcher. It requires clean Sophia and Hagia trees, builds both exact commits
before takeover, validates the compiled profile, resolves the configured
terminal and browser executables, and then enters the guarded gate. The
underlying build launcher is `tools/run_current_hagia_policy_gate_tty4.sh`; the
opt-in installed gate is
`tools/hagia_policy_physical_gate.sh`. It requires
an explicit arm variable, real Hagia and Kitty binaries, a named input seat,
and two connected outputs. The operator exercises fullscreen, native layout
cycling, maximize, minimize/restore, and active-output actions around one
checkpoint-triggered supervised restart; the verifier requires ordered commits,
nonempty checkpoint load/reconciliation, output-change evidence, physical text,
and clean session health. Restart injection is correlated to the first committed
layout-cycle action after the committed fullscreen action, and waits for the
following nonempty checkpoint before killing the authorized Hagia PID. It does
not depend on a global checkpoint count, because legitimate settlement cycles
may add checkpoints without advancing the operator procedure. The wrapper then
`exec`s Hagia so the policy endpoint continues to authenticate the exact PID
supervised by Sophia. Kitty displays one instruction at a time and advances
only after the corresponding committed-action evidence appears, so the operator does not have
to carry instructions across the TTY transition; the final phrase is not shown
until every required action and the restart have been observed. The minimize
instruction pairs its temporarily invisible restore chord on the same screen,
so hiding the proof surface cannot hide the operation needed to return it.
After the restore commits, both the guide and final verifier require Hagia's
next private checkpoint to remain nonempty. This binds the action record to
retained surface ownership instead of accepting a committed no-op. Sophia's
complete policy snapshot retains every policy-owned surface even when its
render layer is hidden or its last client-map observation predates Engine
admission, provided its frontend authority route remains live. Unrouted
authority observations never become policy capabilities; an explicit
withdrawal or removal ends snapshot ownership.
The guide receives an owner-only proof-result path from Sophia. After reading
the exact final line itself, it records only that bounded line at the path;
Sophia independently requires the physical key sequence, X11 delivery,
semantic result, changed terminal pixels, and presented-frame correlation.
Before materializing any returned placement, Sophia refreshes the canonical
scene from current Engine facts. A withdrawal that races an in-flight Hagia
projection therefore advances the scene generation, retires that response as
stale, and permits a later complete cycle; a response cannot resurrect or
configure the withdrawn surface.
Client text typed while a post-policy focus acknowledgement is pending is
retained by Engine's bounded exact-target keyboard handoff, while Hagia's
reserved chords remain outside that queue. Non-text modifier transitions that
surround those reserved chords retain their ordinary client delivery but do
not enter the exact text-producing proof sequence. Merely checking in this gate
is not physical evidence. The live gate records action admission immediately
and committed policy settlement separately; its log must be retained and
reviewed after an authorized hardware run.
After accepting the final phrase, the guide remains alive until Sophia's exact
input proof ends the session; it does not manufacture a surface withdrawal
while completion evidence is still settling.

The gate passed physically on 2026-08-09 with two outputs and real Kitty. One
supervised restart preserved layout and loaded, reconciled, and refreshed a
nonempty checkpoint. All nine ordered policy actions committed, including
fullscreen, two maximize transitions, minimize/restore, and both active-output
directions. The exact 34-event text sequence changed terminal pixels and was
presented 24 ms after its final libinput ingress; all 52 action-plus-text X11
transitions flushed. Shutdown retained zero pending WM, action, or input work,
zero unexpected protocol errors, no live native ownership, and clean frontend,
namespace, Xauthority, and application-group teardown.

The installed guide has a ten-minute physical-sequence safety deadline inside
an eleven-minute global runtime ceiling, not a soak or minimum-duration
criterion. Successful completion still terminates immediately; the ceilings
only prevent an abandoned exclusive DRM/input run from persisting indefinitely
while leaving enough time to read each physical instruction. Other physical
proofs retain the fail-fast 15-second sequence default unless they explicitly
request a bounded override.

Do not put metadata rules, raw pointer operations, shell overlays, or full
Triad IPC in the WM interface. Their retained product behaviors still belong
to the overall port gate through their separately authorized components.

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
