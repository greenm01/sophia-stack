# Dynamic Output Topology Lifecycle Modeling Brief

## 1. System Overview

Sophia's native session output path is a Rust ownership-transfer runtime. It is
Category B (Concurrent / Runtime): a udev-facing producer may report connector
change while the single session owner holds per-output KMS sessions, renderer
targets, page-flip work, presented input projections, X RandR state, and a
public-policy settlement. The owner loop is the only component permitted to
replace that bundle. Notifications are rescan hints rather than authority;
only a complete owner-built topology may become authoritative. If no complete
replacement can be presented, application state survives but routed input is
quarantined and only Engine-reserved recovery controls remain available.

## 2. Scenarios

### Scenario 1: Notification And Owner Snapshot Diverge

**Mechanism**: Connector notifications can burst or become stale before the
owner reaches its rescan boundary. Treating a notification as a topology delta
would allow partial or out-of-order mutation.

**Evidence**:

- Code analysis: the live loop currently captures `outputs` and primary
  `output` once at startup (`owner_loop.rs:150-154`), while the X topology
  conversion fixes its generation at one (`config/output.rs:36-41`).
- Code analysis: native page-flip routing is built from the startup output set
  (`startup/page_flip_poller.rs:4-56`); there is no authoritative hotplug
  producer.

**Affected code paths**: live-session startup, native scanout construction,
output topology conversion, and the future DRM monitor.

**Suggested modeling approach**: keep a one-slot rescan request separately
from the complete observed topology. Permit later notices to replace pending
work, but require the owner to mint the transition epoch from a complete
snapshot.

**Priority**: High. A partial snapshot can misroute input or publish a root
layout that does not match owned KMS heads.

### Scenario 2: Revocation, Quiescence, And Rebuild Are Split

**Mechanism**: Input revocation, page-flip retirement, renderer teardown, and
replacement construction are separately observable boundaries.

**Evidence**:

- Historical: `3af596cd` added scanout quiescence before VT switches;
  `1d18f261` fixed stalled suspension; `d29e2f2c` fixed owner initialization
  before VT restore; `518d7395` preserved renderer images across resume.
- Code analysis: VT release advances the application input epoch before
  `suspend_native_scanout` (`owner_loop/lifecycle.rs:41-73`), while resume
  currently rejects any changed topology (`owner_loop/lifecycle.rs:96-115`).
- Code analysis: suspend may force-detach after a bounded timeout and clears
  presented input projections (`production_visual_runtime/native.rs:104-188`).

**Affected code paths**: `advance_application_input_security_epoch`,
`suspend_native_scanout`, `resume_native_scanout`, VT release/resume, and
startup native recovery.

**Suggested modeling approach**: split revoke, quiesce, rebuild, and publish
into distinct actions. Carry old in-flight work and callback epochs explicitly.

**Priority**: High. Accepting one late callback or routed event after resource
replacement can attach old authority to a new output generation.

### Scenario 3: Four Consumers Settle One Topology Before Input Release

**Mechanism**: scanout ownership, pointer bounds, X RandR, and public-policy
snapshots have independent update APIs. They settle sequentially, but every
publication must carry one complete owner epoch and input must remain
quarantined until the bundle is coherent and presented.

**Evidence**:

- Code analysis: pointer confinement reads the startup `outputs` vector
  (`owner_loop/physical_input_phase.rs:61`); X Authority independently accepts
  monotonic topology snapshots (`x11_socket/frontend/service.rs:54-70`); the
  public policy separately observes complete output descriptors
  (`wm/public_policy.rs:148-161`).
- Historical: `d84bc2e9` unified output frame damage tracking and `b3185b85`
  hardened per-output shutdown, demonstrating repeated cross-output ownership
  defects.

**Affected code paths**: owner loop output state, pointer placement, X frontend
service commands, and `LivePublicPolicyState::observe_outputs`.

**Suggested modeling approach**: model consumer epochs and output sets
separately, then use one logical publication-completion action after replacement
scanout exists and old work is quiesced. This action abstracts the end of
sequential IPC settlement; intermediate implementation states carry no routed
input authority.

**Priority**: High. Mixed epochs create visible/input disagreement even if
each consumer is locally valid.

### Scenario 4: Policy Settlement And First Presentation Fence Input

**Mechanism**: a replacement topology can exist before Hagia has committed a
complete projection and before that projection has reached the display.

**Evidence**:

- Code analysis: policy output observation queues a relayout but a prior
  request may still be in flight (`wm/public_policy.rs:148-221`); scene
  observation normally occurs only when the next request is issued
  (`wm/public_policy.rs:806-832`).
- Code analysis: application hit testing is correctly derived from per-output
  presented input projections, which advance on native retirement
  (`production_visual_runtime/projection.rs:47-82`).

**Affected code paths**: public policy scene observation, staged settlement,
native frame retirement, and physical-input routing mode selection.

**Suggested modeling approach**: make old policy results stale as soon as the
owner observes the replacement topology. Keep input disabled through policy
commit and exact first presentation.

**Priority**: High. Early input enablement reintroduces click-through against
unpresented layout.

### Scenario 5: No Output Or Failed Rebuild Recovers Fail-Closed

**Mechanism**: output loss may temporarily produce no usable head, and rebuild
can fail after old resources are detached.

**Evidence**:

- Code analysis: `OutputTopologySnapshot::validate` rejects an empty topology;
  native scanout startup can report `NoOutputs`; current resume treats changed
  topology as a fatal session error.
- Ratified policy: application processes and last committed logical state are
  retained, ordinary input remains quarantined, and monitoring continues.

**Affected code paths**: monitor health, native scanout construction, owner
transition state, and recovery routing mode.

**Suggested modeling approach**: include an `awaiting` state with no published
candidate, allow later rescan/rebuild, and require input to remain disabled.

**Priority**: High. Terminating the desktop or routing against retired pixels
is unacceptable; bounded degraded recovery is required.

## 3. Modeling Recommendations

### 3.1 Model

- Replaceable notification versus complete owner observation (Scenario 1).
- Input epoch advancement, old scanout retirement, and stale callbacks
  (Scenario 2).
- Atomic consumer publication (Scenario 3).
- Stale policy rejection and presentation-fenced input enablement (Scenario 4).
- No-output/rebuild-failure recovery (Scenario 5).

### 3.2 Do Not Model

- udev property byte strings or libdrm object identifiers: deterministic Rust
  tests own filtering and resource discovery.
- Pixel contents and renderer import formats: model only resource ownership and
  the first-presentation boundary.
- Hagia tags, layouts, or checkpoint bytes: the canonical policy models and
  independent Nim tests own those details.
- Physical connector timing: the model checks ordering; the explicit TTY gate
  owns hardware evidence.

## 4. Proposed Extensions

| Extension | Variables | Purpose | Scenario |
|-----------|-----------|---------|----------|
| Replaceable rescan | `noticePending`, `observedEpoch`, `transitionEpoch` | Separate hints from complete owner truth | 1 |
| Ownership transfer | `phase`, `oldWork`, `scanoutEpoch`, `callbackEpoch` | Split revoke, retire, rebuild, and callback admission | 2 |
| Consumer bundle | `pointerEpoch`, `randrEpoch`, `policyEpoch`, `ownedEpoch` | Prevent mixed publication | 3 |
| Presentation fence | `policyCommittedEpoch`, `presentedEpoch`, `inputEnabled` | Require exact committed and presented replacement | 4 |
| Degraded recovery | `candidateLive`, `phase = "awaiting"` | Retain state without routing against absent output | 5 |

## 5. Proposed Invariants

| Invariant | Type | Description | Targets |
|-----------|------|-------------|---------|
| InputRequiresPresentedPolicy | Safety | Routed input is enabled only for the owned epoch whose policy layout was presented | 2, 4, 5 |
| PublishedTopologyIsAtomic | Safety | at the modeled publication-completion barrier, scanout, pointer, RandR, and policy consumers share the same epoch | 3 |
| OldWorkCannotCrossEpoch | Safety | callbacks and policy outcomes from an older epoch never become authoritative | 2, 4 |
| ReturnedOutputGenerationAdvances | Safety | a removed raw output identity returns only at a higher generation | 1, 3 |
| AwaitingRecoveryIsQuarantined | Safety | no-output or rebuild failure never enables application input | 5 |
| ValidReplacementCanRecover | Liveness | a stable replacement can progress from awaiting recovery to presented state | 5 |

## 6. Findings Pending Verification

### 6.1 Model-Checkable

| ID | Description | Expected invariant violation | Scenario |
|----|-------------|------------------------------|----------|
| MC1 | Can a later notice arrive after rebuild but before publication and allow an older candidate to publish? | PublishedTopologyIsAtomic | 1, 3 |
| MC2 | Can a late page-flip callback or policy outcome cross replacement if only its raw output/request identity matches? | OldWorkCannotCrossEpoch | 2, 4 |
| MC3 | Can input resume after RandR publication but before policy presentation? | InputRequiresPresentedPolicy | 3, 4 |
| MC4 | Can a no-output failure accidentally preserve the old input-enabled flag? | AwaitingRecoveryIsQuarantined | 5 |

### 6.2 Test-Verifiable

| ID | Description | Suggested test approach |
|----|-------------|-------------------------|
| TV1 | udev burst filtering and bounded coalescing | fake event source plus capacity-one monitor tests |
| TV2 | complete equal/add/remove/return/descriptor-change reduction | pure owner topology reducer tests |
| TV3 | X RandR acknowledgement and public-policy scene advancement | routed frontend and policy integration tests |
| TV4 | quiescence timeout, no-output wait, later rebuild, and first-frame enablement | deterministic native owner harness, then TTY gate |

### 6.3 Code-Review-Only

| ID | Description | Suggested action |
|----|-------------|------------------|
| CR1 | owner-loop output state is spread through textual include files | centralize topology epoch and transition methods before adding more consumers |
| CR2 | renderer-image reuse across changed heads is driver-sensitive | discard incompatible retained images and rehydrate from Engine-owned state until measured evidence permits narrower reuse |

## 7. Reference Pointers

- Analysis coverage: 170 local history entries matched correctness, stale,
  output, scanout, seat, or recovery terms across the owner/backend slice. The
  four ownership fixes cited above and current implementation paths were read
  in detail. Remote issue/PR enumeration was unavailable, so no external issue
  claims are used.
- Core sources: `live_session/owner_loop.rs`,
  `live_session/owner_loop/lifecycle.rs`,
  `production_visual_runtime/native.rs`, and
  `live_session/wm/public_policy.rs`.
- Existing models: `PolicyOutputSettlement.tla`,
  `InputAuthorityArbitration.tla`, `VisualRetirement.tla`, and
  `PolicySettlementRecovery.tla`.
