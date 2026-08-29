# Desktop Profile Activation Protocol Instrumentation

This mapping is a design handoff for the future opt-in, schema-versioned
evidence sink. The protocol is not implemented yet, so the listed future hook
points identify the owning source boundary rather than claiming current trace
coverage. `TraceData.tla` is explicitly a design trace and must be replaced by
rendered implementation NDJSON before conformance is claimed.

## 1. Trace Event Schema

Every record uses the existing evidence envelope with `schema=1`, `tag="trace"`,
and an `event` object. Evidence remains opt-in and contains no raw Sophia handle,
application identifier, metadata, configuration value, source path, or KDL text.

Common event fields:

| Field | Type | Meaning |
|---|---|---|
| `name` | closed string | One spec action name |
| `generation` | nonzero u64 or 0 | Desktop profile generation |
| `digest` | redacted fixed digest token | Desktop profile digest; never source content |
| `authority` | closed string | Local authority for local prepare/activate/rollback |
| `messageKind` | closed string | prepare/prepared/activate/active/rollback/rolledBack/configuration |
| `epoch` | nonzero u64 or 0 | Sophia connection epoch |
| `transaction` | nonzero u64 or 0 | Epoch-scoped transport correlation |
| `outcome` | closed string | accepted/rejected/none |
| `configurationGeneration` | u64 | Separate Hagia action/chrome generation |
| `state` | object | Post-action state listed below |

Every event captures the complete post-state required by
`Trace.tla:ValidatePostState`: coordinator phase; candidate, active, latest,
loaded, configuration, prepared, and active identities; proxy-prepared flag;
local prepared/activated/rollback counts; Hagia phase; outstanding kind;
command/ack counts; all three Hagia acknowledgement flags; rejected/promoted
counts; graphical gate; and configuration queued/accepted flags.

Digest capture uses the already computed canonical profile digest. The evidence
sink's redaction policy must preserve equality within a run while preventing raw
candidate values from entering the record.

## 2. Action-to-Code Mapping

### Sophia staging and local-owner actions

| Spec action / event | Code location | Trigger point | Parameters |
|---|---|---|---|
| `StageDesktopProfile` | `crates/sophia-session/src/live_session/wm/public_policy.rs:89-126` | After exact fragments and local policy proxy are retained | generation, digest |
| `PrepareLocalAuthority` | `crates/sophia-session/src/live_session/wm/profile_preparation.rs:110-177` | After one typed local prepare result is reduced | authority, identity |
| `BeginProfileActivation` | future startup coordinator adjacent to `wm/profile_preparation.rs:34-58` | After six local owners and external Hagia are prepared | identity, epoch |
| `ActivateLocalAuthority` | future activation executor adjacent to `wm/profile_preparation.rs:146-177` | After one local activation result is reduced | authority, identity |
| `RollbackLocalAuthority` | `wm/profile_preparation.rs:166-176` plus future activation executor | After one idempotent local rollback settles | authority, identity |
| `CompleteFailedStartup` | future startup gate before `live_session.rs:220` | After every reachable participant rolls back | identity, terminal phase |

### Sophia process and transport actions

| Spec action / event | Code location | Trigger point | Parameters |
|---|---|---|---|
| `LaunchAndNegotiateHagia` | split future pre-graphics owner from `wm/public_policy.rs:606-661`; negotiation at `policy_transport.rs:130-171` | After authenticated hello/welcome and before any profile command | loaded identity, epoch |
| `SendProfilePrepare` | future `PolicyWmSessionTransport` method near `policy_transport.rs:194-208` | After the full frame is flushed | identity, epoch, transaction |
| `SophiaAcceptProfilePrepared` | future decoder branch near `policy_transport.rs:210-247` | After exact capability/epoch/transaction/identity admission and reducer settlement | identity, epoch, transaction, accepted |
| `SophiaRejectProfileCompletion` | same future decoder and coordinator branch | After an exact typed rejection begins rollback | message kind, identity, epoch, transaction, rejected |
| `SendProfileActivate` | future worker phase before current configuration receive at `policy_transport_worker.rs:141-156` | After local activation completes and the frame is flushed | identity, epoch, transaction |
| `SophiaAcceptProfileActive` | future worker completion before `PolicyTransportEvent::Configuration` | After exact active ACK promotes the candidate and opens the launch gate | identity, epoch, transaction, accepted |
| `DisconnectHagia` | `policy_transport_worker.rs:83-87` and future pre-graphics process owner | After epoch advances and rollback begins | old identity, new epoch |
| `TimeoutProfileOperation` | future bounded prepare/activate receive around `policy_transport.rs:130-171` | When the deadline begins rollback | outstanding kind and identity |
| `SendProfileRollback` | future transport method next to prepare/activate sends | After rollback frame flush | identity, epoch, transaction |
| `SophiaAcceptProfileRolledBack` | future decoder branch near `policy_transport.rs:210-247` | After exact rollback ACK settlement | identity, epoch, transaction, accepted |
| `DiscardStaleAck` | future decoder admission branch near `policy_transport.rs:210-247` | After a mismatched completion is consumed without reducer mutation | message kind, identity, epoch, transaction, outcome |
| `SophiaAcceptPolicyConfiguration` | `policy_transport_worker.rs:141-193` | After kind 42 is admitted in the post-profile-active phase | configuration generation |

### Hagia actions

| Spec action / event | Code location | Trigger point | Parameters |
|---|---|---|---|
| `HagiaHandleProfilePrepare` | future startup branch after candidate load at `src/hagia.nim:47-70`, before `src/sophia/policy_client.nim:638-645` | After exact file/wire comparison and ACK flush | identity, epoch, transaction, outcome |
| `HagiaHandleProfileActivate` | same future startup branch | After retained-prepared validation and active ACK flush | identity, epoch, transaction, outcome |
| `HagiaQueuePolicyConfiguration` | `src/sophia/policy_client.nim:638-645` | After kind 42 is sent, which must follow the active ACK | configuration generation and active identity |
| `HagiaHandleProfileRollback` | future startup branch before normal policy loop | After exact candidate discard and rollback ACK flush | identity, epoch, transaction, accepted |

## 3. Special Considerations

- Emit one event per spec action after the action's state mutation and any
  corresponding frame flush. Do not merge send and receive events.
- Sophia and Hagia write separate evidence streams. A deterministic renderer
  orders them by epoch, transaction, message phase, and per-process sequence;
  wall-clock timestamps are diagnostic only.
- The epoch changes before `DisconnectHagia` is captured, matching the model.
  A stale ACK event retains its original epoch and cannot mutate coordinator
  identity or phase.
- The action/chrome configuration generation is intentionally captured in a
  different field and namespace from the desktop profile generation.
- Failure traces must exercise prepare rejection, activate rejection, timeout,
  disconnect after prepared ACK, every local activation position, connected
  rollback, disconnected cleanup, and stale ACK discard.
- Post-promotion restart is outside this instrumentation tranche. Do not emit a
  startup activation event to describe future exact-key reattachment.
- The design trace contains only the success path. It validates all captured
  state fields, while the unused failure wrappers remain ready for real traces.
