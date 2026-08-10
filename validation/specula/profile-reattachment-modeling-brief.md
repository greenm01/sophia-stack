# Desktop Profile Reattachment Protocol Modeling Brief

## 1. System Overview

Sophia is the trusted session coordinator and Hagia is a separately supervised
policy authority. This target is Category A (Distributed / Message-Passing): an
active graphical session can outlive the Hagia process, while a replacement
Hagia obtains a new authenticated connection epoch and reloads Sophia's retained
policy fragment.

Startup profile activation is modeled separately. This brief covers only the
post-promotion recovery boundary required before the activation capability can
be enabled in production. Reattachment proves that a replacement process owns
the exact already-active policy identity before Sophia admits normal policy
configuration on the replacement connection.

## 2. Existing Boundaries

- The public policy session retains the immutable profile fragments and the
  policy authority slot after startup (`wm/public_policy.rs`).
- A failed policy worker is dropped, the reducer is disconnected, the
  connection epoch advances, and the supervised Hagia process is restarted
  without tearing down the graphical session (`wm/public_policy.rs`).
- `ProcessSupervisor` bounds default restart attempts and terminates its child
  on drop (`supervisor/policy.rs`, `supervisor/process.rs`).
- Every replacement socket is authenticated and assigned a fresh nonzero epoch
  (`policy_transport.rs`).
- The ordinary worker currently requires `PolicyConfiguration` as the first
  post-negotiation control message (`policy_transport_worker.rs`).
- Hagia validates the staged candidate before entering the policy loop
  (`src/hagia.nim`, `src/sophia/policy_client.nim`).
- The typed profile handoff reducer already binds acknowledgements to epoch,
  transaction, generation, digest, and phase. Its current production entry
  point is deliberately opt-in.

## 3. Historical Archaeology

The recovery path has required repeated stale-response and settlement
hardening. Relevant commits are `8d1b2106`, `3159a32a`, `3cb22ef7`,
`c6152681`, `c1745b4e`, and the pre-graphics launch refactor `9e0bf264`.
The common constraint is that worker failure may discard incomplete settlement
state, but must preserve committed layout and graphical resources.

A repository-wide search found no matching GitHub issues or open pull requests
for Hagia restart, reconnect, or policy reattachment. Coverage for this narrow
analysis is therefore six historical commits, zero issues, and zero open pull
requests.

## 4. Scenarios

### Scenario 1: Replacement Sends Configuration Immediately

**Mechanism**: a newly negotiated Hagia connection sends action/chrome policy
configuration before proving that it loaded and activated the retained profile.

**Consequence**: policy generated under an unverified candidate can influence
an otherwise active graphical session.

**Model**: configuration admission is connection-epoch local and remains false
until exact reattachment reaches `Active`.

**Priority**: Critical.

### Scenario 2: Old-Epoch Acknowledgement Settles Replacement

**Mechanism**: a delayed `Prepared` or `Active` acknowledgement from the dead
process arrives after a replacement connection is established.

**Consequence**: Sophia could admit configuration without proof from the
current authenticated peer.

**Model**: every completion must match the current epoch, transaction, active
generation, digest, and expected phase. Stale completions are inert.

**Priority**: Critical.

### Scenario 3: Replacement Loads A Different Candidate

**Mechanism**: the retained file and wire identity diverge, or a different
candidate is staged while the graphical session still reflects the old active
profile.

**Consequence**: reattachment becomes an unmodeled live reload and can expose a
cross-authority mixed generation.

**Model**: the replacement may acknowledge only the immutable global active
identity. Mismatch rejects reattachment and cannot change global state.

**Priority**: Critical.

### Scenario 4: Failure Mutates Committed Session State

**Mechanism**: rejection, timeout, disconnect, or exhausted restart attempts
reuse startup rollback and disturb already-committed local authorities,
graphics, or layout.

**Consequence**: an isolated policy-process failure tears down or partially
rewrites a healthy graphical session.

**Model**: reattachment failure prevents configuration on that connection but
leaves the active profile, local authority state, graphics, and committed
layout unchanged.

**Priority**: High.

### Scenario 5: Correlation Tuple Reuse Across Incarnations

**Mechanism**: a replacement reuses both the connection epoch and a transaction
identifier associated with a prior incarnation. A transaction number alone may
repeat because correlations are epoch-scoped.

**Consequence**: correlation becomes ambiguous in evidence and implementations
that retain outstanding completion state.

**Model**: use a strictly newer connection epoch for each authenticated
incarnation and reject transaction reuse within that epoch. A replacement gets
a fresh per-connection reducer, so the same numeric transaction under a newer
epoch remains unambiguous.

**Priority**: High.

## 5. Modeling Recommendations

### 5.1 Model

- Immutable global active profile identity.
- Graphics and committed layout as preservation sentinels.
- Process incarnation and strictly advancing connection epoch.
- Replacement loaded identity and per-epoch transaction history.
- Prepare, activate, reject, timeout, disconnect, and bounded retry actions.
- Delayed acknowledgements from prior epochs.
- Per-epoch configuration admission.

### 5.2 Do Not Model

- Startup preparation or local authority activation; assume they completed.
- New profile generation, watched reload, or cross-authority reconfiguration.
- KDL bytes, file paths, raw handles, application metadata, or pixel contents.
- Supervisor wall-clock backoff, OS process identifiers, or socket buffering.
- Layout algorithms; model only that the committed layout sentinel is stable.

## 6. Minimal Reattachment Shape

1. Begin with one globally active profile and an open graphical session.
2. The active Hagia process crashes or disconnects.
3. Sophia creates a replacement authenticated connection with a newer epoch
   and fresh per-connection reducer, while keeping the global profile unchanged.
4. Sophia sends `ProfilePrepare` for the exact active identity.
5. Hagia accepts only when its loaded identity is exact, then replies
   `ProfilePrepared` with the current correlation tuple.
6. Sophia sends `ProfileActivate` for the same tuple.
7. Only an exact `ProfileActive` enables normal configuration on that epoch.
8. Any failure keeps configuration disabled. A bounded retry starts with a new
   epoch and per-epoch correlation scope but the same global active identity.

This intentionally reuses the typed profile wire and pure phase reducer. It is
not startup activation: it does not prepare local authorities, promote a new
generation, open graphics, or roll back the active desktop profile.

## 7. Proposed Invariants

| Invariant | Description |
|---|---|
| `ReattachmentRequiresExactActiveIdentity` | Prepared and Active acknowledgements name the immutable global active identity |
| `ReattachmentUsesFreshEpoch` | Each authenticated replacement has an epoch newer than the previous incarnation |
| `ConfigurationRequiresReattachedActive` | Normal policy configuration is admitted only after exact Active acknowledgement on the current epoch |
| `ReattachmentCannotChangeGlobalProfile` | No recovery action changes the global active generation or digest |
| `StaleReattachmentAckIsInert` | Prior-epoch or wrong-transaction completions cannot change phase or admission |
| `ReattachmentFailurePreservesSession` | Reject, timeout, disconnect, and retry leave graphics and committed layout unchanged |
| `LoadedCandidateBoundsAcknowledgement` | Hagia cannot acknowledge an identity different from its loaded retained fragment |

## 8. Findings Pending Verification

### 8.1 Model-Checkable

| ID | Description | Expected invariant |
|---|---|---|
| RM1 | Configuration arrives before replacement Active | `ConfigurationRequiresReattachedActive` |
| RM2 | An old-epoch Active settles the replacement | `StaleReattachmentAckIsInert` |
| RM3 | A different loaded digest acknowledges active | `LoadedCandidateBoundsAcknowledgement` |
| RM4 | Recovery promotes a different global identity | `ReattachmentCannotChangeGlobalProfile` |
| RM5 | Timeout or retry closes graphics or changes layout | `ReattachmentFailurePreservesSession` |

### 8.2 Test-Verifiable

| ID | Description | Test approach |
|---|---|---|
| RT1 | Replacement requests capability and loads retained identity | Private Unix-socket integration test |
| RT2 | Configuration is rejected before Active | Worker state-machine test |
| RT3 | Exact Active unlocks existing configuration path | Worker state-machine test |
| RT4 | Stale completion after epoch advance is inert | Pure reducer and socket test |
| RT5 | Failed attempt preserves committed layout | Public-policy restart regression |

### 8.3 Code-Review-Only

| ID | Description | Action |
|---|---|---|
| RR1 | Startup and reattachment orchestration remain distinct | Keep global activation owner separate from per-connection proof |
| RR2 | Restart exhaustion policy | Retain current bounded supervisor behavior; do not add session teardown |
| RR3 | Live reload exclusion | Reject any reattachment identity other than the retained active key |

## 9. Reference Pointers

- Sophia: `wm/public_policy.rs`, `policy_transport_worker.rs`,
  `policy_transport.rs`, `policy_profile_handoff.rs`, `supervisor/policy.rs`,
  and `supervisor/process.rs`.
- Hagia: `src/hagia.nim`, `src/sophia/policy_client.nim`, and
  `src/sophia/profile_handoff.nim`.
- Prior proof: `validation/specula/profile-activation-protocol/` and
  `validation/specula/profile-activation-protocol-modeling-brief.md`.
