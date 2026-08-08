# Target-Resolved Input

**Role:** normative target and arbitration contract for shell and Engine
interaction.
**Status:** ratified pre-schema prerequisite; `sophia_shell_v1` remains
deferred.

Sophia has one Engine-owned physical-input authority and two delivery
contracts. Application input selects a visual surface and retains the
coordinates required by X11. Future shell input selects a bounded target and
discloses an action, normalized value, or independently authorized local
coordinate. The routes may share private spatial indexes, but they retain
different public identities, lifecycle rules, protocol semantics, and
disclosure budgets.

This document fixes semantic requirements rather than wire records. It adds no
Engine runtime, Hagia behavior, UI toolkit, reactive framework, or protocol
discriminants. Full immutable snapshots remain normative. A future delta is a
transport optimization that must name and validate its base generation.

## Relationship To Application Input

The implemented native-X path selects a Sophia `SurfaceId` and constructs a
`RoutedInputRequest` containing a serial, seat, device, time, event kind,
selected surface, and both global and surface-local coordinates. The native X
Server Frontend resolves that passive route to a client and applies X11 focus,
grabs, event masks, XKB/XI state, and profile checks. Global coordinates are
deliberate X11 root-coordinate data; coordinate erasure is not this path's
security mechanism.

Confined profiles isolate resource tables, event queues, and application
authority by namespace. `classic-shared` deliberately retains shared-X
authority inside its admitted application profile. Neither profile grants an
application authority over shell, trust, lock, or another profile's pixels.
The native X Server Frontend—not the legacy-WM bridge's private synthetic X
server—delivers input to applications. No non-X application frontend exists.

The current application path rebuilds input layers from committed renderable
surfaces, before matching pixels are known to have been presented. It also
re-hit-tests ordinary pointer events before the frontend applies its
namespace-local grab state. Those are implementation facts, not guarantees to
copy into the shell contract. Before application and shell routes can coexist,
application selection must use the applicable last-presented scene and active
grabs must become Engine-visible route leases. The runtime correction remains
deferred and is an explicit release blocker.

| Dimension | Application surface routing | Target-resolved shell input |
| --- | --- | --- |
| Status | implemented native-X path with admitted hardening debt | ratified pre-schema contract |
| Public identity | generational `SurfaceId` | opaque authority/session/slot/generation target identity |
| Disclosure | global and surface-local coordinates required by X11 | coordinate-free actions by default; normalized values; capability-bounded local coordinates |
| Protocol semantics | X frontend owns X11 focus, event masks, XKB/XI state, and client delivery | Engine owns target selection and bounded capture; no toolkit semantics |
| Selection state | currently committed/renderable; presentation coupling required before coexistence | interaction snapshot paired with the applicable last-presented frame |
| Isolation | profile and namespace admission; `classic-shared` remains shared inside its application profile | authority, session, presentation, target, seat/device/contact, modal, and disclosure bounds |

## Per-Seat Arbitration

Engine applies this precedence for each physical event:

1. A security-attention, lock, session-switch, or authority-revocation
   transition advances the control epoch, clears application leases and shell
   capture, and quarantines queued old-epoch input before new secure UI becomes
   eligible.
2. A statically reserved Engine shortcut is consumed locally and is not routed
   as application or shell input.
3. One existing valid route lease or shell capture retains the seat. The two
   states are mutually exclusive and pointer boundary crossing cannot create a
   second owner.
4. With no retained owner, Engine resolves the event against the applicable
   output's last-presented interaction snapshot.
5. With no eligible presented application surface or shell target, Engine
   delivers nothing.

The following decision flow is normative semantic precedence for the future
coexistence boundary, not a required implementation control-flow shape. It
covers pointer or touch events that require spatial selection. Application
keyboard delivery follows Engine focus rather than spatial hit-testing, but it
still shares control epochs, reserved shortcuts, authority lifecycle, and
frontend protocol rules with this boundary.

```text
[ Physical pointer/touch event: seat + device/contact ]
                         |
                         v
           [ Sophia Engine per-seat arbitration ]
                         |
                         +-- security/control epoch transition
                         |     Revoke leases and captures, quarantine
                         |     old-epoch input, and deliver no old input.
                         |
                         +-- reserved Engine shortcut
                         |     Consume locally; route nothing.
                         |
                         `-- otherwise: inspect retained seat owner
                               |
                               +-- application route lease
                               |     |
                               |     +-- valid and inside admitted app scope
                               |     |     Emit RoutedInputRequest to the X
                               |     |     frontend with global and local data.
                               |     |
                               |     `-- scope exited
                               |           Request frontend release and route
                               |           neither contract until its ack.
                               |           Discard this event; never replay it.
                               |
                               +-- shell target capture
                               |     |
                               |     +-- valid
                               |     |     Emit target-resolved action, paced
                               |     |     value, or granted region-local data.
                               |     |
                               |     `-- invalidated
                               |           Cancel once; emit no stale delivery
                               |           and do not reinterpret the same
                               |           boundary as a fresh selection.
                               |
                               `-- no retained owner
                                     Resolve the applicable output's
                                     last-presented interaction snapshot.
                                          |
                                          +-- application surface
                                          |     Emit a passive RoutedInputRequest.
                                          |     This alone creates no route lease;
                                          |     a frontend grab requires a separate
                                          |     Engine-visible request and ack.
                                          +-- shell target
                                          |     Emit the applicable target-resolved
                                          |     event. Selection alone creates no
                                          |     capture; a qualifying press may.
                                          `-- no eligible choice
                                                Deliver nothing.
```

This is not a diagram of the current runtime. The native-X application path
still selects from committed renderable input layers and applies frontend-local
grab state after ordinary Engine re-hit-testing. Last-presented application
selection, Engine-visible route leases, and target-resolved shell delivery must
be implemented before this coexistence flow can ship.

An X frontend grab is reduced to an Engine-visible lease naming its admitted
profile, namespace authority, surface generation, and presentation/control
epochs. A confined lease may continue only across application regions owned by
that namespace. A `classic-shared` lease may continue across application
surfaces in that shared profile. Neither lease covers shell, foreign-profile,
trust, lock, or secure-session regions.

Leaving the authorized lease scope starts an ordered release handshake. Engine
withholds both out-of-scope application input and a replacement shell capture
until the frontend acknowledges release. A security transition does not wait:
Engine revokes locally, stops delivery, discards old-epoch queued events, and
treats frontend acknowledgement as cleanup rather than continuing authority.
This deliberately narrows desktop-wide X grab compatibility at privileged
boundaries.

## Presented Snapshots And Target Admission

Engine retains separate committed and submitted scenes plus a presented scene
epoch for every output. A committed or submitted target cannot receive input
until its pixels and interaction snapshot become the applicable presented
state. Output removal, disablement, remap, suspend, session switch, or retirement
invalidates the presentation epoch and every dependent route or capture.

A target identity contains an owning authority and authority-session epoch,
opaque slot, and monotonically advancing generation. A generation cannot be
recycled while any live or stale reference could exist. Engine never infers
reuse from equal geometry, action, ordering, or pixels. A visual-only commit
may preserve identity only when geometry, action, interaction kind, authority,
modal membership, seat scope, and disclosure reference remain exact.

Each target is a flat record, not a widget. At admission Engine:

- clips it to the same authority's presented visual allocation;
- applies presented occlusion and trust precedence before it is eligible;
- resolves overlap by committed scene order and rejects equal-priority
  ambiguity;
- prevents a lower-trust or foreign target from intercepting higher-trust or
  foreign pixels; and
- rejects the complete snapshot when its implementation-advertised target
  count, region complexity, or overlap bounds are exceeded.

The allocation may include intentional transparent padding, but it must remain
local to the authority's visible layout allocation. It cannot become a global
invisible click shield or an interaction-only overlay over application or trust
content.

## Capture And Activation

A press may capture one presented target for one seat. The capture records the
initiating device and button/contact as well as target identity, action,
authority/session epoch, output/presentation epoch, modal scope, and any
disclosure capability version. A release from another device or contact cannot
activate or cancel that capture.

Release activates only if the exact captured identity and meaning remain
eligible in current presented state. Target removal or regeneration, spatial
ineligibility, authority/session loss, seat or initiating-device loss, output
epoch loss, modal-scope change, or capability revocation cancels once and
clears local capture. A cancelled token cannot activate. Authority reconnect
creates a new session epoch and never revives queued input, a route lease, or a
capture from the prior epoch.

Modal scope and explicit target membership constrain selection. Regions
outside an authorized modal scope do not become targets merely to observe an
outside click. The eventual dismissal request remains a schema question.

## Event Classes, Pacing, And Failure

Discrete targets emit coordinate-free actions by default. Activation discloses
the committed decision, not pointer history, global position, scale, or
unrelated geometry. Records have monotonic serials and are delivered at most
once within one authority epoch. Ambiguous delivery is not replayed after
reconnect; the new epoch makes loss explicit rather than risking duplicate
activation.

The action reference is issuer-scoped rather than globally typed by its wire
integer. It binds issuer role and authority epochs, recipient authority epoch,
operation class, and target generation. Therefore a metadata-broker action
cannot become a WM or session action through token collision, forwarding, or
reconnect. Expiry, revocation, wrong recipient or generation, and repeated
activation identity are rejected before dispatch.

Continuous targets use a target-local normalized domain, including valid zero
and endpoint values. One active stream exists per seat and has one replaceable
pending value. An explicit pacing tick may flush at most one intermediate value
for that stream; no universal frequency is promised. Capture begin, discrete
actions, completion, and ordinary cancellation are ordered records. Stream
admission reserves capacity for its final value and terminating boundary, so
later motion cannot consume that capacity. Completion and ordinary
cancellation work even without motion and preserve the initial or latest value
immediately before the boundary.

Security cancellation is different: revocation clears capture and pending
state locally and emits no final data to the revoked endpoint. If an endpoint
stops acknowledging its bounded ordered queue, saturation is endpoint failure.
Engine revokes the authority epoch and clears capture rather than coalescing a
boundary, retaining input without a bound, or terminating the entire frontend
because one client stopped reading. Eventual draining is promised only for a
live endpoint that continues to acknowledge delivery.

## Independently Authorized Local Coordinates

The shell cannot grant coordinates to itself. An independently authorized
session or portal policy broker issues a revocable capability; the shell may
only reference it from a target. Engine is the enforcement point. A capability
is bound to its issuer and version, recipient authority/session, target
generation, output and local region, permitted seat/device class, precision,
rate budget, expiry, and revocation epoch.

Coordinate delivery requires a live matching capture and capability. Values
are clipped and quantized to the capability's target-local region. The
capability authorizes neither output-global coordinates, motion outside that
region, another target, a finer precision, nor a higher event rate.

There are two revocation barriers:

- Removing or replacing a target or capability reference in a normal visual
  commit becomes effective when that replacement snapshot is presented. Until
  then, the old pixels and their old interaction snapshot remain paired.
- Broker, authority, session, seat/device, output, or security revocation is an
  immediate control-epoch barrier. Capture and pending data clear before the
  replacement frame, and queued events carrying the old epoch are discarded at
  delivery.

The measurable default is coordinate-free discrete events, bounded normalized
continuous events, and zero coordinate-bearing events without an independently
issued live capability.

## Precommitted Visual Alternatives

Hover and pressed feedback select among default, hover, and pressed visual
alternatives committed in the same immutable scene as the target. This is a
semantic group, not a settled `NodeVariant` wire type. Every alternative is
resource-ready before presentation, shares fixed bounds, clip, damage ceiling,
z-order, and target identity, and cannot alter action meaning, modal scope,
authority, disclosure, or mandatory trust content.

For shared output content, pressed wins when any eligible seat holds capture;
otherwise hover wins when any eligible seat hovers; otherwise Engine selects
the default. Cursor presentation remains per seat. Only an explicitly optional
alternative may fall back to the default. Continuous-value visuals remain
shell-owned display-list commits; Engine does not acquire widget state,
styling, text layout, or animation policy.

## Data-Minimization Threat Model

This boundary prevents ambient coordinate and motion disclosure and makes
exceptional disclosure measurable by capability, precision, rate, and region.
It does not make an authorized target owner ignorant of the action identifiers
or normalized domains it defined. A malicious shell can encode spatial meaning
in many small targets or action IDs. Admission quotas bound that availability
and resolution abuse, while authorization determines which shell may own a
region; neither mechanism is described as absolute privacy.

The public boundary explicitly excludes reactive property graphs, callbacks,
bindings, widget-specific nodes, Engine text widgets, inferred target reuse,
global click shields, Engine styling tokens, theme roles, caret engines, and
toolkit layout trees.

Target identity is reserved for later keyboard-navigation and accessibility
projections. Separately owned bounded metadata may refer to it, but labels,
traversal graphs, and application identity do not enter this physical-input hot
path.

## Performance And Formal Correspondence

Pacing and latency targets require a workload, hardware class, and measurement
method. Only effects explicitly declared optional may degrade. Text, controls,
trust indications, and other mandatory content cannot disappear silently. A
shell-to-Engine DMA-BUF path remains an unproven candidate until ownership,
fencing, damage, fallback, and measured power/bandwidth behavior exist.

`validation/tla/TargetResolvedInput.tla` covers immutable scene history,
per-output presentation, multiple target ordering, ownership eligibility,
monotonic generations, seat/device/contact capture, modal and lifecycle loss,
independently issued grants, local coordinates, and once-only cancellation.

`validation/tla/TargetInputPacing.tla` covers valid zero and no-motion streams,
one replaceable continuous slot, paced flush, reserved ordered boundaries,
normal final-value delivery, at-most-once draining, endpoint failure, and
security cancellation without post-revocation output.

`validation/tla/InputAuthorityArbitration.tla` covers committed, submitted, and
presented route choices; profile-scoped frontend leases; release
acknowledgement; mutually exclusive shell capture; reserved shortcuts; secure
preemption; and stale control-epoch quarantine.

`validation/architecture/alloy/PresentedTargetTopology.als` separately searches
bounded static ownership, occlusion, trust-order, modal, identity, and grant
relations. `validation/architecture/smt/TargetGeometryAndDisclosure.smt2`
checks the arithmetic form of containment, clipping, quantization, rate
budgets, and distinguishable-outcome quotas. These complementary models do not
prove the temporal TLA+ properties or select concrete schema limits.

These are small, hand-maintained project models informed by scenario-driven
Specula analysis. Generated scaffolding, trace validation, and runtime
instrumentation remain deferred until a shell runtime exists.
