# Desktop Profile Activation Protocol Modeling Brief

## 1. System Overview

Sophia is the trusted session coordinator and Hagia is the separately supervised
policy authority. This target is Category A (Distributed / Message-Passing): the
candidate crosses a process boundary through an owner-only staged file, while
phase changes and acknowledgements must cross the authenticated `sophia_wm_v1`
Unix socket. A local Sophia slot cannot stand in for Hagia's actual state.

The first milestone is transactional startup only. Every authority candidate
must be prepared before graphical resources are opened, and no candidate may be
called active until external Hagia acknowledges the exact generation and digest.
Live reload and cross-session recovery are excluded.

## 2. Existing Boundaries

- Sophia stages one immutable fragment per authority and retains the directory
  for the public-policy session (`wm/public_policy.rs:89-126`).
- Pre-graphical startup prepares seven local slots but deliberately cannot
  activate them (`wm/profile_preparation.rs:146-177`). The policy slot is a
  handoff proxy, not evidence of Hagia activation.
- The display socket, Xauthority, seat, devices, and scanout are opened only
  after `prepare_public_launch` returns (`live_session.rs:207-247`).
- Hagia currently starts later in `from_public_config`, after output construction
  (`wm/session.rs:284-299`, `wm/public_policy.rs:606-661`).
- Hagia validates the staged `HAGIA_POLICY_CANDIDATE` before connecting and
  builds its adapter before the policy loop (`src/hagia.nim:47-70`,
  `src/sophia/policy_client.nim:606-640`).
- Transport negotiation authenticates the peer and binds a nonzero connection
  epoch (`policy_transport.rs:130-171`).
- Sophia then requires Hagia's unrelated action/chrome `PolicyConfiguration`
  as the first control message (`policy_transport_worker.rs:128-156`).

## 3. Scenarios

### Scenario 1: Local Proxy Mistaken For External Activation

**Mechanism**: Sophia can mechanically activate its local policy slot without
proof that Hagia retained or activated the candidate.

**Evidence**: the production executor owns only local slots and its activation
method intentionally returns false (`wm/profile_preparation.rs:60-75,146-177`).
A local-success experiment passed unit tests but violated this authority boundary
and was discarded.

**Model**: distinguish `localPolicyProxy` from `hagiaPhase`; require an
authenticated Hagia acknowledgement before global promotion.

**Priority**: Critical.

### Scenario 2: Stale Acknowledgement Across Epochs

**Mechanism**: Hagia may acknowledge preparation and disconnect before
activation. An old acknowledgement must not settle a replacement process.

**Evidence**: Sophia admits control messages against the negotiated epoch
(`policy_transport.rs:210-247`); the worker reports failure asynchronously and
the supervisor owns restart (`policy_transport_worker.rs:67-93`). Historical
policy recovery needed stale-response hardening (`c1745b4e`, `8ed1789e`).

**Model**: commands and acknowledgements carry epoch, transaction, generation,
and digest. Permit disconnect in every phase; old-epoch completion is a no-op.

**Priority**: Critical.

### Scenario 3: File And Wire Identity Diverge

**Mechanism**: Hagia loads a file before negotiation while Sophia later names a
candidate on the socket. The two identities can differ.

**Evidence**: fragments embed the shared generation and digest
(`src/config/coordinator.nim:228-253`). Rejected generation reuse was a confirmed
model-checking defect; generations are now monotonic (`ff34787`).

**Model**: separate `loadedIdentity` and `requestedIdentity`; acknowledge only
an exact nonzero match. Paths and profile contents stay off wire.

**Priority**: High.

### Scenario 4: Barrier Conflicts With Policy Configuration

**Mechanism**: Hagia immediately sends action/chrome configuration after
negotiation, but profile activation must precede normal policy traffic.

**Evidence**: Hagia calls `installConfiguration` before its first snapshot
(`src/sophia/policy_client.nim:638-645`); Sophia rejects any other first message
(`policy_transport_worker.rs:141-156`). Kinds 42/43 are Hagia-authored chrome and
actions, not the Sophia-authored desktop profile (`ipc/wm_v1.rs:1401-1549`).

**Model**: add a distinct negotiated capability and require
`prepare -> prepared -> activate -> active` before existing configuration.
Never overload kinds 42/43 or their generation namespace.

**Priority**: High.

### Scenario 5: Failure During Partial Activation

**Mechanism**: local owners activate sequentially after all prepare, then Hagia
can reject, time out, or disconnect. Graphics must remain unopened and every
candidate must roll back.

**Evidence**: the pure coordinator preserves the prior active identity until all
completions settle (`src/config/coordinator.nim:136-170`), and production tests
already cover all seven preparation failure positions.

**Model**: split participant completions, allow Hagia failure at each boundary,
and make startup failure terminal. Rollback is idempotent; rollback failure still
prohibits graphics.

**Priority**: Critical.

### Scenario 6: Restart After Successful Startup

**Mechanism**: a replacement Hagia has no in-memory phase even though it can
reload Sophia's retained active fragment.

**Evidence**: public policy retains `_profile_fragments` and recreates worker and
connection epochs on restart (`wm/public_policy.rs:51-52,671-680`).

**Model**: exclude post-promotion restart. Specify exact-key reattachment
separately; do not silently reuse startup activation.

**Priority**: Medium now, critical before restart integration.

## 4. Modeling Recommendations
### 4.1 Model

- Sophia phase, local prepared/activated sets, and graphical gate.
- Hagia loaded, prepared, and active identity as separate state.
- Connection epoch, transaction, generation, digest, and expected phase.
- Prepare/activate acknowledgement, rejection, timeout, and disconnect.
- Stale completion after an epoch change and rollback of every local participant.

### 4.2 Do Not Model

- KDL parsing, provenance text, include limits, or filesystem bytes.
- Raw Sophia handles, application metadata, layouts, checkpoints, or pixels.
- Watched reload, runtime candidate admission, or post-promotion Hagia restart.

## 5. Minimal Protocol Shape To Verify

| Direction | Typed message | Required fields |
|---|---|---|
| Sophia -> Hagia | `ProfilePrepare` | epoch, transaction, generation, digest |
| Hagia -> Sophia | `ProfilePrepared` | epoch, transaction, generation, digest, outcome |
| Sophia -> Hagia | `ProfileActivate` | epoch, transaction, generation, digest |
| Hagia -> Sophia | `ProfileActive` | epoch, transaction, generation, digest, outcome |
| Sophia -> Hagia | `ProfileRollback` | epoch, transaction, generation, digest |
| Hagia -> Sophia | `ProfileRolledBack` | epoch, transaction, generation, digest, outcome |

Use a fixed 32-byte digest decoded at staging and a profile-specific closed
outcome enum. Decoders reject zero identity fields, reserved bits, trailing
bytes, wrong capability, and phase-inappropriate messages.

## 6. Proposed Invariants

| Invariant | Description | Targets |
|---|---|---|
| `GraphicsRequireGlobalActivation` | Graphics require every local owner and Hagia to acknowledge the exact candidate | 1, 5 |
| `HagiaAckMatchesLoadedCandidate` | Hagia acknowledges only its loaded generation and digest | 3 |
| `CompletionMatchesConnection` | Completion matches epoch, transaction, identity, and phase | 2 |
| `ConfigurationFollowsProfileActivation` | Message 42 cannot precede profile activation | 4 |
| `FailureCannotPromote` | Reject, timeout, disconnect, and rollback never promote | 2, 5 |
| `NoPartialVisibility` | Local partial activation cannot open graphics | 5 |
| `IdentityNamespacesAreDistinct` | Profile and action-configuration generations cannot conflate | 4 |

## 7. Findings Pending Verification

### 7.1 Model-Checkable
| ID | Description | Expected violation |
|---|---|---|
| MC1 | Local proxy completes without Hagia's exact ACK | `GraphicsRequireGlobalActivation` |
| MC2 | Old-epoch ACK settles a replacement peer | `CompletionMatchesConnection` |
| MC3 | Configuration begins while only prepared | `ConfigurationFollowsProfileActivation` |
| MC4 | Disconnect during partial activation opens graphics | `FailureCannotPromote` |

### 7.2 Test-Verifiable
| ID | Description | Test approach |
|---|---|---|
| TV1 | Rust/Nim codec parity and malformed rejection | Shared revision corpus |
| TV2 | Exact staged-file identity comparison | Owner-only fragment fixtures |
| TV3 | Every failure position rolls back | Deterministic fake transport executor |
| TV4 | Configuration follows activation | Socket-pair integration test |

### 7.3 Code-Review-Only

| ID | Description | Action |
|---|---|---|
| CR1 | Pre-graphics process lifetime and cleanup | Split prepared process owner from graphical session |
| CR2 | Post-startup supervised reattachment | Ratify before enabling recovery on the capability |

## 8. Reference Pointers

- Sophia: `live_session.rs`, `policy_transport_worker.rs`,
  `wm/profile_preparation.rs`, `wm/public_policy.rs`, `policy_transport.rs`,
  `ipc/wm_v1.rs`, and `protocol/sophia-wm-v1.kdl`.
- Hagia: `src/hagia.nim`, `src/sophia/policy_client.nim`,
  `src/config/coordinator.nim`, and `.specula-output/spec/base.tla`.
