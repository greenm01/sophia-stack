# Desktop Profile Reattachment Protocol Instrumentation

This mapping is a design handoff for the opt-in, schema-versioned evidence sink.
The orchestration is not wired into production yet, so `TraceData.tla` is a
design trace and does not claim implementation conformance.

## 1. Trace Event Schema

Each NDJSON record uses `schema=1`, `tag="trace"`, and an `event` object. It
contains no raw Sophia handle, process identifier, application identifier,
metadata, KDL text, source path, or unredacted profile value.

Common event fields:

| Field | Type | Meaning |
|---|---|---|
| `name` | closed string | One spec action name |
| `epoch` | u64 | Authenticated connection epoch, or zero after disconnect |
| `transaction` | u64 | Profile-specific correlation, or zero when not applicable |
| `generation` | u64 | Retained active profile generation |
| `digest` | fixed redacted token | Equality-preserving active profile digest |
| `state` | object | Complete post-action state used by `ValidatePostState` |

The state object captures coordinator and Hagia phases; loaded identity; current
and previous epoch; complete epoch history; outstanding kind and transaction;
used-transaction, command, acknowledgement, loaded-epoch, active-epoch,
configuration-message, and accepted-configuration counts; admission flag; last
completion epoch; and attempt count. The immutable active identity, graphics
sentinel, and layout token are also validated on every event.

## 2. Action-to-Code Mapping

### Sophia recovery and transport actions

| Spec action / event | Code location | Trigger point | Parameters |
|---|---|---|---|
| `DisconnectHagia` | `crates/sophia-cli/src/commands/live_session/wm/public_policy.rs:1029-1064`; `crates/sophia-runtime/src/policy_profile_handoff.rs:130-133` | After the dead worker is removed and per-connection admission is cleared | old epoch, active identity |
| `BeginReplacement` | `public_policy.rs:1066-1106`; `crates/sophia-runtime/src/policy_transport.rs:158-200` | After authenticated negotiation records the fresh epoch and retained loaded identity | new epoch, loaded identity, attempt |
| `SendProfilePrepare` | `policy_profile_handoff.rs:138-183`; `policy_transport.rs:412-432` | After the typed prepare frame is flushed | epoch, transaction, active identity |
| `SophiaSettleProfilePrepared` | `policy_profile_handoff.rs:186-227`; `policy_transport.rs:312-335` | After exact completion reduction | epoch, transaction, identity, outcome |
| `SendProfileActivate` | same reducer and transport send paths | After the typed activate frame is flushed | epoch, fresh transaction, active identity |
| `SophiaSettleProfileActive` | same completion paths; future gate before `policy_transport_worker.rs:141-156` | After exact Active reduction changes per-epoch admission | epoch, transaction, identity, outcome |
| `SophiaAcceptPolicyConfiguration` | future gate immediately before `policy_transport_worker.rs:141-156` | After kind 42 is admitted for the current reattached epoch | epoch |
| `SophiaRejectPrematureConfiguration` | `policy_transport_worker.rs:148-150,169-170` | When normal traffic is consumed as an out-of-phase failure | epoch |
| `DiscardStaleAcknowledgement` | `policy_profile_handoff.rs:191-207` | After a mismatched completion is classified stale without phase mutation | message kind, epoch, transaction, identity |
| `TimeoutProfileOperation` | future bounded handoff around `policy_transport_worker.rs:128-143` | After timeout clears only per-connection state | epoch, outstanding tuple |
| `DegradeFailedReattachment` | `public_policy.rs:1085-1094` | After bounded supervisor exhaustion preserves committed layout | attempt count |

### Hagia participant actions

| Spec action / event | Code location | Trigger point | Parameters |
|---|---|---|---|
| `HagiaHandleProfilePrepare` | `src/sophia/profile_handoff.nim:96-121`; `src/sophia/policy_client.nim:172-200` | After reducer completion and Prepared frame flush | epoch, transaction, identity, outcome |
| `HagiaHandleProfileActivate` | `profile_handoff.nim:122-136`; `policy_client.nim:215-230` | After reducer completion and Active frame flush | epoch, transaction, identity, outcome |
| `HagiaSendPolicyConfiguration` | future reattachment branch before the existing normal policy loop | Immediately after kind 42 is flushed | epoch, whether local Active was established |

`InjectDelayedAcknowledgement` is model-only fault injection. An implementation
trace represents it using a real old-epoch completion delivered after the
replacement epoch is installed; it must not add a synthetic production hook.

## 3. Special Considerations

- Emit one post-state event per reducer or transport boundary. Do not merge
  send, receive, and settle operations.
- Sophia and Hagia streams use per-process sequence numbers. A deterministic
  renderer orders events by epoch, transaction, phase, and sequence; timestamps
  remain diagnostic only.
- Digest redaction must preserve equality within one evidence set so file/wire
  identity comparison remains trace-checkable.
- A failed reattachment never emits startup rollback or profile-promotion
  events. It clears only the replacement epoch and may emit a later retry.
- Capture the full epoch history and current epoch's used-transaction count
  from test-only orchestration state. Production evidence may instead derive
  these fields in the deterministic renderer from typed begin events.
