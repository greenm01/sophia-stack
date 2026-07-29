# Architecture

**Role:** normative system architecture.

This document defines Sophia's authority boundaries and intended process
relationships. [Data-Oriented Design](dod.md) defines the records that cross
those boundaries. [Namespaces and Portals](namespaces-and-portals.md) defines
the trust and transfer model. [Engine Architecture](engine-architecture.md)
defines the domains inside Engine and the precise scope of its compositor
role. [Compositor Graphics](compositor-graphics.md) defines the
renderer-neutral compositor-owned display list and its native lowering.
Subsystem documents record implementation detail and current coverage.

Sophia is an X-centric product with an Engine-centered architecture. Sophia
Engine owns physical input, the scene graph, atomic visual commits, rendering,
and scanout. The active X Server Frontend terminates X11 and translates it into
namespace-checked Sophia facts. The WM proposes blind layout policy. Portals
mediate deliberate namespace crossings. Engine boundaries remain
protocol-neutral so future translators can be evaluated without redesigning
visual authority.

## System Shape

```text
================================================================================
                         HARDWARE AND KERNEL
================================================================================
 [ physical input devices ]                                  [ display output ]
            │                                                        ▲
            │ libinput                                               │ DRM/KMS
            ▼                                                        │

================================================================================
                    SOPHIA ENGINE: VISUAL AUTHORITY
================================================================================
 ┌────────────────────────────────────────────────────────────────────────────┐
 │ Scene graph | hit-testing | atomic commits | rendering | frame scheduling  │
 └───────────────┬────────────────────┬───────────────────────┬────────────────┘
                 │ opaque policy data │ reduced portal facts  │ sanitized chrome
                 ▼                    ▼                       ▼
        ┌────────────────┐   ┌─────────────────┐   ┌──────────────────────────┐
        │   SOPHIA WM    │   │  PORTAL BROKER  │   │ METADATA BROKER / SHELL │
        │ blind policy   │   │ policy + grants │   │ redacted presentation   │
        └───────┬────────┘   └────────┬────────┘   └──────────────────────────┘
                │ layout proposals    │ executor commands
                ▼                     ▼

================================================================================
                         PROTOCOL FRONTEND
================================================================================
 ┌────────────────────────────────────────────────────────────────────────────┐
 │ Sophia X Server Frontend: X11 resources, selections, grabs, protocol state │
 └───────────────────────────────┬────────────────────────────────────────────┘
                                 │ namespace-checked transactions,
                                 │ lifecycle, routed input, configure
                                 ▲

================================================================================
                         ADMITTED CLIENT NAMESPACES
================================================================================
 ┌────────────────────────────────────┐     ┌─────────────────────────────────┐
 │ Namespace A: trusted/shared        │  X  │ Namespace B: confined           │
 │ X terminal | trusted local tools   │     │ browser | chat | untrusted app  │
 └────────────────────────────────────┘     └─────────────────────────────────┘
```

The session supervisor surrounds these components. It owns process lifetime,
namespace allocation, client admission, authorization material, broker health,
and recovery. It does not absorb protocol semantics, portal policy, rendering,
or layout policy.

## Current And Target State

### Implemented

- The native Sophia X Server Frontend accepts bounded concurrent local X11
  clients, owns X resource tables, emits `SurfaceTransaction` batches, routes
  client-targeted input/control, and renders two real xterms through Engine and
  KMS. The retained paired Milestone 3 proof passes under both classic-shared
  and fresh zero-capability confined profiles with physical keyboard and
  pointer input, authenticated RandR delivery, configure-plus-pixels resize,
  and clean teardown. Its X13 runs report 94/90 ms startup readiness and 13 ms
  maximum composition.
- X resources and selection state are namespace-keyed. Protocol values and a
  runtime registry model immutable profiles, directional portal capabilities,
  admission provenance, and revocation. After X setup authentication, the
  frontend asks session policy for an immutable context before allocating X
  client/resource identity. The live classic session admits same-UID Unix peers
  into distinct registry admissions that deliberately share its namespace,
  then revokes each admission after connection cleanup. The supervisor creates
  a fresh owner-only Xauthority file and cookie for every live session, passes
  only its path to clients, and removes it on teardown. Denial uses native X11
  setup failure. `sophia-live-session` explicitly selects classic-shared or a
  confined group with zero ambient portal capabilities. A socket regression
  assigns simultaneous clients distinct confined namespaces and proves a
  cross-namespace window map, property mutation, and selection ownership return
  `BadAccess`; selection conversion fails normally and rejected property writes
  emit no metadata candidate. Foreign event-mask selection also returns
  `BadAccess`, cannot change the receiving worker's authority-local key target,
  and brokered input reaches only the addressed client's private queue. Classic
  same-namespace existing-resource behavior remains available. Session
  supervision can revoke one admission by opaque `ClientAdmissionId`; socket
  shutdown then runs the worker's ordinary route, resource, surface, and lease
  cleanup without terminating its peers.
- Live native-X setup and populated RandR resource replies now derive their
  bounded root/output/mode facts from an Engine output-topology snapshot. Engine
  hit-testing also crosses the boundary as a `RoutedInputRequest`; the frontend
  resolves the owning client and applies deterministic authority-local XKB
  modifier state before emitting core and selected XI2 events. Live topology
  updates produce mask-selected RandR notifications, and surface resize keeps
  new geometry quarantined until matching pixels commit.
- A policy-managed X11 `MapWindow` first emits a protocol-neutral
  `SurfacePresentationIntent`; it does not make the window client-visible.
  Engine and the blind WM can therefore plan from geometry, constraints, role,
  and generation before any pixel buffer exists. After Engine accepts the
  proposal, one `AdmitSurface` control configures and maps the X window, and
  matching authority pixels complete the atomic visual commit. Override-redirect
  and standalone frontend paths retain their ordinary immediate-map behavior.
- `sophia-portal` has deterministic reducers for clipboard, drag-and-drop, file
  handoff, screen capture, URI open, and notifications. Owner-only bounded
  broker IPC, policy-provider IPC, expiry/revocation lifecycle, and the first
  native-X `CLIPBOARD`/`PRIMARY` source-proxy executor are complete. The other
  portal kinds do not yet have complete native executors.
- Standard DRI3 1.2 carries FD-bearing `Open`, modifier-bearing multi-plane
  pixmaps, xshmfences, and Present submissions through bounded frontend batches.
  A Mesa RADV `vkcube` trace reaches an Engine transaction without an X11 error.
  The reusable renderer-private DMA-BUF registry and cloneable Present feedback
  router now feed the persistent mixed CPU/DMA-BUF renderer and page-flip
  retirement path. Physical AMDGPU evidence requires one complete render target
  per mixed export. The context and pipeline are destroyed after export rather
  than rebound; the exported buffer retains its GBM/EGL surface through
  page-flip retirement. Binding a retained context to a fresh surface still
  caused the third command stream to be rejected. The paired software and
  CPU-plus-Vulkan hardware gate now
  passes through Engine-owned KMS with controlled rejection recovery and exact
  resource retirement.
- XLibre is absent from the production workspace and launcher. Its frozen
  prototype remains under `research/xlibre` as historical evidence.
- The former Wayland frontend is absent from the production workspace,
  dependency graph, launcher, and validation gates. Its frozen prototype and
  last Kitty/SHM and controlled DMA-BUF evidence remain under
  `research/wayland` as proof that Engine transactions were not X-shaped.

### Production Session Loop

The physical owner services native page-flip retirement before routing new
shortcut actions. External WM packets cross a bounded typed queue with at most
one request in flight; a dedicated transport worker owns the blocking socket
round trip. The owner retains policy authority: it correlates the response,
rejects proposals whose layout fingerprint became stale, validates the opaque
layout transaction, and applies workspace and session effects only after the
atomic layout commit. The worker owns no scene, focus, workspace, application,
or protocol state.

Owner completion evidence separates child-reap and physical-input phase maxima
from WM transport round-trip and queue-dwell measurements. A slow policy
process may delay a policy result up to its bounded timeout, but it cannot hold
page-flip retirement or physical input on the owner thread.

The live X11 path has completed the Milestone 4 presentation work: standard
DRI3/Present registrations, acquire-fence gating, mixed CPU/GPU composition,
KMS submission, page-flip-driven Complete/Idle feedback, controlled rejection
recovery, and exact resource retirement pass on retained hardware evidence.
Milestone 5 application promotion uses the repeatable, unattended classic and
confined QEMU session gate; machine-specific runs are compatibility diagnostics.

The physical validation stack also has a single-client production profile. It
launches one approved native-X application directly, uses the external
reference WM's metadata-blind `natural` policy, and retains the normal deferred
admission and native scanout path:

```text
 [single validation client]
            │ X11 / DRI3 / Present
            ▼
 [Sophia X authority] ── opaque intent ──► [natural-size policy]
            │                                  │ centered geometry
            └──────── transaction facts ◄──────┘
                            │
                            ▼
 [Engine admission] → [renderer] → [DRM/KMS retirement]
```

This is an isolation profile, not an alternate compositor architecture. The
selected executable belongs only to session launch configuration. The policy
sees no executable identity, Engine remains protocol-neutral, and a successful
run exercises the same production transaction and scanout owners used by a
full desktop.

Renderer-live owns the CPU scene. Backend-live owns native scanout, the concrete
per-output runtime set, and `LiveProductionVisualRuntime`, which contains
Engine commit/preparation, composition, Present scheduling, KMS
submission/retirement, cleanup, and reduced feedback sequencing. Production X
polls one backend service entry point; backend-live decides retirement, queued
Present scheduling, and pending native submission order and returns a reduced
tick. `runtime_driver` owns CPU cycle order, per-submission Present preparation
against committed visual state, the prepared retirement commit-before-feedback gate, and dynamic
asynchronous KMS phase order from reduced observations. Backend-live executes
requested retire, Present-schedule, and pending-submit phases; it is not a
second phase coordinator. The production session-loop architecture target and
unattended xmonad daily-driver gate are complete.

The production target is one protocol-neutral session coordinator in
`sophia-engine::runtime_driver`. It owns the ordered visual state machine while
calling narrow adapters that retain their existing authority:

1. accept a bounded authority envelope, preserve its ordered atomic transaction
   groups, and transfer native registrations immediately into renderer-private
   ownership;
2. validate, commit, or prepare transactions against Engine committed state;
3. compose one immutable frame from the resulting Engine snapshot;
4. ask the live backend to submit and retire that frame through KMS; and
5. route protocol feedback only after the matching retirement.

The coordinator owns sequencing, not foreign state. X resources remain in the
protocol authority; imported images and fences remain in the renderer; GBM, DRM,
and KMS objects remain in the backend; session runtime retains process and
recovery policy. The CLI constructs these adapters, launches supervised
processes, observes proof criteria, requests shutdown, and does no frame work.

`PreparedSurfaceCommit` remains the asynchronous Present gate. Preparation
snapshots protocol-neutral state without changing the committed scene. Matching
page-flip retirement revalidates and merges the candidate, then permits Present
Complete before Idle and release. Rejection, timeout, disconnect, surface
removal, changed baselines, or backpressure discard pending work, preserve the
last committed geometry-plus-pixels state, and retire native resources exactly
once. No failure path may infer presentation from client traffic or send
feedback before backend retirement.

Each queued Present owns exactly one matching `SurfaceTransaction`. Persistent
scene state is the Engine's committed snapshot, not a table of historical
transactions. Preparation rebases only the queued surface's causal generation;
unrelated surfaces enter the immutable frame through the committed baseline and
retain their own generations and transaction history. Input and focus
projection advance from committed state after the matching retirement, never
from a pending Present candidate.

Layout changes use a separate Engine-owned epoch coordinator. It joins opaque
WM size proposals to authority-observed content extents and keeps declared
surface constraints separate from temporary recovery constraints. A timed-out
epoch preserves the complete committed scene. If a surface has safe content at
another extent, Engine may publish `min_size == max_size == safe_extent` and
`resizable = false` for one bounded blind-WM replan. Engine supplies no
position, floating decision, client identity, or protocol object. Late pixels
from the abandoned extent remain fenced until the authority returns to the
safe content extent.

Admission is a state machine beside, not inside, committed visual state:
`PolicyPending`, `ControlPending`, `AwaitingPixels`, `AwaitingRetirement`, then
`Managed`. Planning records never contain a synthetic buffer and never enter
the committed scene.
An admission control acknowledgement proves only that protocol configure/map
effects were delivered; visual admission still waits for a concrete buffer at
the accepted content extent. DMA-BUF admission selects one exact
surface/transaction/buffer Present group and remains unfocusable until its
page flip retires. A retained extent or previously committed size may guide a
recovery proposal, but it cannot satisfy an admission surface's visual
readiness: every admission-finalization proposal must own a newly staged
concrete transaction for that surface. A layout timeout does not reset that
lifecycle:
retry epochs retain `ControlPending` or `AwaitingPixels` surfaces in their
admission-finalization set; an `AwaitingRetirement` surface is already owned by
its exact visual candidate and cannot be replanned. Only the eventual matching
visual retirement may release quarantined pixels and Present feedback.

For X11, the protocol authority derives this boundary from its window tree.
Only a non-override-redirect direct child of the X root is policy-managed.
Descendants and override-redirect windows are client-positioned.
`MapSubwindows` applies only to the requested window's direct children; it must
not admit the requested parent or unrelated namespace windows. Parent links,
map state, and tree queries remain X authority facts and do not cross the blind
WM boundary.

Software drawing to an X descendant is translated and accumulated into its
root child's immutable presentation buffer. Engine receives the toplevel
Sophia surface and translated damage, not a separate WM node for every X child.
The accumulated extent derives from concrete child buffers and never grows
merely because policy configured a larger toplevel.

Pixels observed before managed admission remain authority evidence, not visual
truth. The session retains ordered transaction-homogeneous groups in a bounded
quarantine, excludes them from renderer intake, and releases complete groups at
accepted geometry only when the admission state and concrete extent agree.
Transactions and Present submissions retain their causal ID across that
boundary; they are never injected into or relabelled as the frontend envelope
that happens to trigger release. Overflow or mixed identity is terminal and
fail-closed. A DMA-BUF surface transaction and Present submission have exact
one-to-one surface/buffer cardinality. Their buffer and fence release facts stay
deferred while quarantine references them, and backend intake registers the
resources, begins presentation ownership, then applies release. These cold-path
records do not add buffers to the passive Engine admission table.

Present scheduling is classified per submission. A buffer matching the pending
layout epoch is staged with that epoch. A known wrong-size buffer for an
already-managed surface is rejected, while a complete pre-admission Present is
retained as recovery evidence at its natural extent. Submissions for unrelated
surfaces remain immediately eligible. A layout epoch must not become a
session-wide Present barrier. The scheduler shares one immutable authority
batch and retains only bounded submission records rather than cloning the full
batch per queued Present.

Authority and committed surface extents are client-content geometry. Before a
layout node crosses the WM boundary, Engine converts both geometry and
constraints to outer allocations using the active chrome clearance. The
matching WM response is inset through the inverse operation exactly once.
Chrome therefore cannot turn an exact recovery constraint into a smaller
client configure.

The legacy-X11 WM bridge translates those same generic constraints into
synthetic ICCCM `WM_NORMAL_HINTS`. A manage-time constraint-profile change
replaces the private synthetic window so an unmodified legacy WM reevaluates
its ordinary size-hint policy. Native WMs consume the same WM API facts without
the ICCCM translation.

### Configure, Visual-Candidate, And Rendering State

Sophia follows river's separation between scheduled policy, client configure,
and rendering state, adapted to Sophia's stronger authority boundary. A WM
proposal is not a buffer, a delivered configure is not a commit, and an
authority-observed buffer is not rendered merely because its extent is known.

```text
                    opaque constraints and geometry
              ┌────────────────────────────────────────┐
              ▼                                        │
       ┌────────────┐   control   ┌────────────┐        │
       │ requested  │────────────▶│ configured │        │
       │ WM extent  │             │ client     │        │
       └────────────┘             └─────┬──────┘        │
              ▲                        │ authority       │
              │ timeout/replan         │ observation     │
              │                        ▼                 │
       ┌──────┴──────┐           ┌────────────┐         │
       │ exact safe  │◀──────────│ candidate  │         │
       │ constraint  │ evidence  │ quarantined│         │
       └─────────────┘           └─────┬──────┘         │
                                      │ Engine prepare  │
                                      ▼                 │
                                ┌────────────┐           │
                                │ committed  │           │
                                │ visual     │           │
                                └─────┬──────┘           │
                                      │ KMS page flip    │
                                      ▼                 │
                                ┌────────────┐           │
                                │ rendered   │───────────┘
                                │ and focused│
                                └────────────┘
```

The Engine reduces complete authority observations into one passive
`SafeSurfaceObservation` per surface. The record carries extent, evidence
class, source transaction, and Engine observation sequence. During admission,
a complete presented buffer outranks an accumulated software-backing snapshot
regardless of arrival order. This prevents a policy-sized background clear
from replacing the natural extent and transaction of a client frame. Within
one evidence class the newest observation wins. After admission, normal
committed ordering resumes.

This reducer does not make the Engine understand X11. The X authority owns the
meaning and order of Present, core drawing, SHM drawing, clears, and backing
storage. A raw `XPixmap` is authority-local identity, not renderable evidence:
the authority resolves DRI3 pixmaps to DMA-BUF storage and snapshots software
or MIT-SHM pixmaps to immutable CPU storage before emitting the transaction.
The observation batch separately marks surfaces produced by a complete
presentation request, so presented-frame evidence does not depend on the
storage type. Accumulated core/SHM drawing remains backing-snapshot evidence.
Xserver is the reference for the longer-term content rule: those operations
form one ordered logical window stream rather than unrelated whole-window
owners.

Niri supplies a compatible policy precedent, not Sophia's transaction model:
fixed client constraints can cause a window to open floating. Sophia publishes
generic exact constraints through the blind WM API; an unmodified legacy WM
may float the constrained node through ordinary ICCCM behavior. Engine never
selects a floating position or inspects application identity.

Admission quarantine follows the same order. The selected complete Present
supersedes older groups whose content it fully covers; older Presents receive
Skip/Idle settlement and older backing-only groups are discarded. Newer groups
remain fenced until the selected page flip retires, then drain in original
authority order. Removal, timeout, and disconnect settle the same bounded
records without resource debt.

There is no compatibility-only committed-snapshot production adapter. Every
active client path enters through authority transactions. XLibre and Wayland
remain historical evidence and cannot become alternate production loops.

## Load-Bearing Ownership Rules

| Component | Owns | Must not own |
| --- | --- | --- |
| Sophia Engine | physical input, scene graph, hit-testing, committed visual state, damage, frame scheduling, rendering, DRM/KMS presentation | client protocol parsing, protocol-local resources, portal decisions, WM policy |
| X Server Frontend | X11 sockets, authentication adapter, XIDs, atoms, properties, selections, focus, grabs, events, drawing readiness | physical devices, workspaces, final layout, portal policy, renderer imports, scanout |
| Session runtime | process supervision, namespace registry, admission, authorization publication, bounded I/O coordination, recovery | protocol meaning, visual policy, portal allow/deny decisions |
| Spatial policy process (current Sophia WM) | layout, workspace, focus and launch policy through opaque handles | XIDs, protocol objects, namespaces, titles, PIDs, payloads, rendering |
| Portal broker | bounded transfer policy, decision and grant lifecycle | protocol object tables, payload rendering, DRM/KMS, client-global visibility |
| Portal executor | concrete payload/handle transfer and native protocol completion | policy decisions or ambient namespace authority |
| Metadata broker/shell | sanitization and compositor-owned chrome | WM identity disclosure or client-protocol authority |

No component may acquire another row's authority merely because it currently
runs in the same process. Crate and process boundaries may evolve; ownership
does not.

## Protocol Frontends

A protocol frontend parses one client protocol, owns its object/resource
tables, enforces the admitted namespace context, and reduces client-visible
state into Sophia records. It may own protocol focus, grabs, selections,
configure/ack state, buffer readiness, and lifecycle behavior.

A frontend emits:

- `AuthoritySurface` and `SurfaceTransaction` values;
- surface removal and lifecycle facts;
- sanitized metadata candidates;
- bounded portal request facts;
- accept/reject acknowledgements for Engine-routed input and control.

No frontend may duplicate Engine's scene graph or infer presentation from
client traffic. Engine output snapshots and presentation retirement flow back to
the authority so it can produce native protocol replies and events.

### Sophia X Server Frontend

Use **Sophia X Server Frontend** for the component and **X11** for its
client-facing API and wire protocol. “X Authority” is acceptable shorthand for
the protocol role and current crate name.

The frontend follows the Phoenix strategy, not Phoenix code: implement a modern
X server cleanly, retain the established X11 API, and expand only from real
application evidence. Sophia is not designing an “X12” or a speculative native
application protocol. If repeated product evidence eventually exceeds X11,
Sophia may extract a native interface from proven Engine mechanisms through an
explicit specification decision.

Modernization happens beneath X11 through Sophia-owned atomic commits,
Engine-owned presentation, explicit buffer readiness, session-selected
namespaces, and narrowly justified extensions. Classic shared-X is a supported
profile rather than a defect to be silently removed.

The detailed contract is in
[sophia-x-authority.md](sophia-x-authority.md). Real-client admission is tracked
in [x11-compatibility-matrix.md](x11-compatibility-matrix.md).

### Future Compatibility Frontends

No non-X application frontend is currently supported or planned. A future
compatibility translator may be admitted only from named product evidence. It
must reduce to existing Sophia transactions, routed input, namespaces, and
portals; remain subordinate to Engine and session policy; and must not import
another protocol's shell, workspace, physical-input, presentation, or
compositor-extension architecture. Architectural openness is permission to
evaluate a translator later, not a compatibility promise.

### XLibre Boundary

XLibre is not a production dependency or an active integration track. Its
XComposite mirror, namespace experiment, and routed-input extension are
historical lessons under `research/xlibre`.

A future XLibre provider may be reconsidered only when measured native-X gaps
justify its authority and maintenance cost. Such a provider could own broad X11
semantics, but it could never own Sophia DRM/KMS, physical input, layout,
namespace admission, portal policy, or session control.

## Atomic Visual Transactions

Sophia commits geometry and matching pixels together. An authority may retain
pending geometry, buffers, damage, and protocol synchronization, but Engine
presents only a coherent `CommittedSurfaceState`.

The default slow-client behavior is:

- preserve the last committed good surface;
- do not stretch stale pixels into new geometry;
- do not expose half-rendered buffers or black resize borders;
- reject stale generations and invalid readiness;
- degrade only through explicit bounded timeout policy.

X core drawing, SHM, Render, and Present/DRI3 must reduce to the same readiness
model. Any future frontend must do the same. Prototype `LayoutEpochState`,
XComposite readiness inference, and the retired Wayland adapter are historical
compatibility mechanisms, not the permanent native-X seam.

Renderer imports and KMS handles remain backend-private. An authority may pass
an opaque buffer handle with explicit ownership, dimensions, format, damage,
and fence/readiness facts; it never submits scanout.

## Native Performance Direction

Optimization preserves the same authority boundaries. Engine produces one
immutable, contiguous frame plan per output generation. The renderer keeps
contexts, pipelines, imports, and reusable frame surfaces in bounded
generation-keyed storage; the backend remains the owner of KMS submission and
page-flip retirement.

The warmed path targets no context, shader, or surface allocation. Damage and
buffer-age history reduce composition work, while incomplete history forces a
full repaint. One newest pending frame and one KMS submission are retained per
output so slow rendering cannot create an unbounded queue. If measurement
later requires a renderer worker, it consumes only immutable bounded frame
commands and returns opaque rendered buffers; protocol and WM state remain on
their existing owners.

Client DMA-BUF imports are renderer-owned, generation-keyed resources. The
renderer frame boundary carries only an opaque image generation plus the cold-import
descriptor. The native output context imports once, reuses the resident texture
for compositor-only repaints, validates the descriptor on every hit, and
evicts the predecessor before backend-live releases its idle fence. Context
reset clears native residency while the backend's still-live buffer lease
allows a bounded re-import. Neither the X frontend nor the WM can observe or
control the cache.

Direct scanout and hardware planes are backend capabilities, not alternate
authority paths. Engine first proves scene eligibility, the backend validates
the exact format/modifier through an atomic test, and any rejection returns to
mixed composition without losing committed visual state.

XLibre and mature Wayland compositors may provide external comparison runs and
implementation lessons. They do not enter Sophia's workspace, production
dependency graph, protocol surface, or session loop.

## Input

Sophia Engine reads physical devices, applies global shortcuts and chrome
hit-testing, walks the actual transformed scene, and selects a `SurfaceId` plus
target-local coordinates. It sends that route to the owning authority.

The X frontend then applies X11 focus, grabs, event masks, XKB/XI state, and
namespace checks. A future frontend would remain responsible for its own
protocol-local delivery rules after the same Engine route.

When an unmodified primary press selects an unfocused visible surface, Engine
first sends the opaque target through the blind spatial-policy interface. It
retains the ordered press, drag motion, and release in a bounded handoff until
the frontend acknowledges protocol focus on that same surface. Raw pointer
events and protocol-local identity never enter the WM.

The authority returns a reduced delivery acknowledgement. Engine never writes
arbitrary client events or receives a client connection handle. Route failure
does not fall back to synthetic input.

Input delivery stays off the WM path. The WM may choose focus policy in
response to the reduced opaque click target, but it does not receive motion,
button payloads, key events, or protocol identity.

## Spatial Policy And Chrome

The WM consumes immutable snapshots keyed by opaque `SurfaceId` values and
emits `LayoutTransaction` proposals. It never sees XIDs, protocol object IDs,
namespace IDs, titles, classes, PIDs, paths, icons, credentials, or portal
payloads.

“WM” is the name of the current spatial-policy role, not a preferred kind of
interface. A policy process may tile, scroll, stack, float, combine those
approaches, run a single-application session, or try a model for which the
usual window-manager vocabulary is a poor fit. The packets carry geometry,
capabilities, and state; the policy process keeps its own trees, columns,
stacks, or other private model.

A complete environment is not folded into that role. For example, xmonad or
qtile may occupy the policy slot with only a small companion shell. An
Xfce-style environment divides work among spatial policy, panels and
decorations, session services, portals, and ordinary applications. A future
Sophia-native compositor/Engine protocol would still meet these same authority
boundaries. The examples differ in form, not in who is allowed to own input,
protocol state, rendering, or scanout.

Engine mints transaction IDs, validates every proposal, and keeps the last
committed layout when the WM is absent, malformed, timed out, or restarting. A
valid but rejected layout is a policy failure, not a transport failure.

Client-owned shell surfaces may reserve bounded root-relative edge spans. A
protocol frontend parses its native mechanism and emits only
`SurfaceOutputReservations` keyed by opaque `SurfaceId`; it does not send
atoms, object IDs, window types, metadata, or application identity. Engine
activates reservations only while the surface is mapped with the
`ClientPositioned` presentation role, clips partial spans against each output,
and reduces same-edge reservations by maximum depth. Opposing edges combine.
An invalid aggregate preserves the last valid work area.

The full output rectangle remains the composition and hit-test space for the
client-positioned surface. The reduced work rectangle is policy input for
managed surfaces. Native Sophia WMs and the optional X11 WM bridge receive the
same `bounds` field, so neither Engine nor the bridge contains an xmobar,
xmonad, dock, or toolkit branch.

Manage state such as size, focus, fullscreen, and workspace assignment is
separate from compositor-only render state such as position, z-order, crop,
opacity, and transforms. Engine owns frame-clock interpolation and cancellation;
the WM does not drive animation frame by frame.

Titles, icons, attention, trust badges, and close actions belong to the metadata
broker and compositor shell. Chrome output is sanitized and generation-checked.
Polite close is routed to the owning authority as `WM_DELETE_WINDOW` or
`xdg_toplevel.close`; process termination remains later session policy.

The optional [X11 WM Bridge](sophia-x11-wm-bridge.md) may present a synthetic
X11 facade to a legacy WM such as xmonad while speaking the normal blind Sophia
WM protocol. It is policy compatibility, not an application X server or a path
around namespace boundaries.

## Namespaces And Portals

Namespace identity is session policy, not protocol metadata. The supervisor
assigns an immutable `ClientAdmissionContext`; authorities enforce it; Engine
and WM do not reinterpret it.

Classic trusted X clients may deliberately share one namespace. Confined
clients receive separate namespaces and explicit capabilities. Cross-namespace
lookup, delivery, and transfer fail closed unless a live portal grant authorizes
one bounded operation.

Portal policy receives only source/target namespaces, transfer kind,
generation, bounded metadata, and lifetime. Runtime executors retain protocol
request context, data, handles, and UI effects. Denial maps to native protocol
failure and never freezes the session or fabricates input.

The complete admission, capability, grant-lifecycle, and first X11
clipboard/PRIMARY contract is in
[namespaces-and-portals.md](namespaces-and-portals.md).

## Runtime And Process Failure

Runtime adapters translate external readiness and completion into bounded
`SessionRuntimeObservation` values. The data-only session reducer may request
polling, WM policy, rendering, scanout, portal drain, chrome presentation, or
restart. It does not parse X11, read portal payloads, render, or own native file
descriptors.

Supervised components report bounded health state and generations. Runtime
policy owns restart delay and give-up decisions. Engine preserves committed
visual state while a WM or broker restarts. A broker restart revokes grants
whose executor continuity cannot be proven.

Logs are a boundary. Default diagnostics may contain opaque IDs, generations,
counts, outcomes, and timings. They must not contain raw XIDs, namespace IDs,
credentials, titles, PIDs, paths, payloads, icons, or buffer contents.

## Development Order

The native-X foundations, production session loop, paired software/GPU gates,
namespace and portal reference flows, xmonad policy bridge, and Firefox
daily-driver soak are complete. The next milestone is selected from measured
native-X daily-driver gaps. New X11 compatibility remains probe-driven; future
application protocols are outside the active roadmap. XLibre and Wayland stay
archived unless a later specification decision admits a bounded provider.

## Reference Boundaries

- Phoenix: clean-room modern X server strategy and evidence-driven coverage.
- XLibre: historical X11 delivery, namespace, and compatibility lessons.
- niri and the retired Smithay frontend: historical backend, renderer, and test
  lessons without adopting their compositor-policy architecture.
- picom: historical XComposite/Damage and buffer-lifetime lessons.
- river: external policy-protocol and crash-isolation lessons.
- macOS WindowServer/Core Animation: transaction-first presentation and
  fail-closed visual integrity.

The WM policy boundary is versioned independently at Sophia WM API v4. Engine
owns physical shortcut matching, workspace visibility, transaction validation,
and opaque session actions. A WM registers bounded keycode/modifier chords to
opaque action IDs during startup and receives only opaque layout nodes and
action activations. Native Sophia WMs consume this contract directly; legacy
X11 WMs require the metadata-blind compatibility bridge and a bounded profile.
