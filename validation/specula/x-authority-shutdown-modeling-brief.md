# X Authority Shutdown Modeling Brief

## 1. System Overview

The live X path is Category B concurrent runtime code. X client workers,
frontend command handling, the Engine owner, CPU composition, native
submission, and kernel retirement advance independently. A clean terminal
session must close admission without losing already-accepted authority work,
settle the latest CPU update on the display, and retain an independently
reachable forced-cancellation path.

The physical failure that motivated this model combined two ownership bugs:

- the frontend relay drained a bounded ingress channel into an unbounded
  `BTreeMap`, so a worker could exit while thousands of batches remained owned
  by a separate egress thread; and
- the service returned after `StopAccepting` when its workers reached zero,
  then blocked joining that egress thread. The later `StopAndDisconnect`
  command could no longer be received, while the Engine owner had already
  stopped draining and left its latest CPU update pending.

The repaired design has no relay-owned collection or relay thread. Each
producer retains at most its current transaction, an ordered coordinator
admits only the exact next ticket to the bounded Engine channel, and
coordinator cancellation remains reachable through the frontend command loop.
Successful session termination enters a two-second owner quiescence phase:
close admission, continue authority and native service, observe frontend
channel drain, and complete only after the exact latest CPU update settles.

## 2. Scenarios

### 2.1 Owner backpressure after the client exits

A worker produces a transaction while the Engine channel is full. The X client
then exits and the supervisor sends `StopAccepting`. The producer still owns
the batch, so the frontend remains live and can receive `StopAndDisconnect`.
Cancellation clears the producer and the service terminates within a bounded
test timeout.

Source and regression:

- `crates/sophia-x-authority/src/x11_socket/connection/server.rs`,
  `XAuthorityOrderedEgress::submit_blocking`;
- `crates/sophia-x-authority/tests/x11_wire/routed_service.rs`,
  `routed_service_cancellation_releases_authority_backpressured_worker`.

### 2.2 Ordered producers lose their Engine receiver

The expected producer discovers that the Engine receiver disappeared while a
later producer is waiting for its transaction turn. Transport failure becomes
coordinator-wide cancellation, wakes every waiter, and is reported as a
service error instead of leaving the later producer asleep.

Source and regression:

- `XAuthorityOrderedEgress::transport_disconnected` and both submission paths;
- `routed_service_authority_disconnect_releases_backpressured_worker`.

### 2.3 Final CPU update arrives during shutdown

The successful client exit closes frontend admission but does not end Engine
ownership. The owner continues to receive all producer-held batches, compose
the latest update, service native output, and observe exact primary retirement.
Only frontend drain, an empty local authority queue, no pending CPU update, and
no native work permit completion.

Source and regression:

- `crates/sophia-session/src/live_session/owner_loop_state.rs`,
  `SessionQuiescence::decision`;
- `owner_loop/authority.rs` and `owner_loop/physical_input_phase.rs`;
- `live_session::tests::session_quiescence_requires_frontend_authority_cpu_and_native_drain`;
- `live_session::cpu_visual_progress::tests::latest_wins_updates_are_all_accounted_after_retirement`.

## 3. Modeling Boundary

### 3.1 Model

- bounded producer ownership and strict transaction-ticket order;
- graceful admission closure and independent forced cancellation;
- Engine acceptance with latest-wins supersession;
- CPU composition, one native in-flight update, and exact retirement;
- normal completion versus cancelled completion.

### 3.2 Do Not Model

- X11 request bytes, resources, windows, or raster payloads;
- thread identities, mutex internals, channel implementation, and sleep
  intervals;
- pixels, checksums, DRM objects, or page-flip timestamps;
- the wall-clock value of the two-second fallback. Rust tests own the reducer
  boundary and physical evidence owns elapsed performance.

## 4. State and Action Correspondence

| Model | Rust |
|---|---|
| `held` | one `XAuthorityBoundedEgressEnvelope` retained by each active producer |
| `delivered` | coordinator `next_ticket - 1` |
| `accepted` | batches consumed by the live-session Engine owner |
| `superseded` | latest-wins CPU updates replaced before retirement |
| `inFlight` | native output work awaiting exact retirement |
| `frontendOpen` | routed frontend service still owns its authority sender |
| `commandResponsive` | service loop can still receive `StopAndDisconnect` |
| `Produce` | traced X worker or raster producer creates one envelope |
| `DeliverExpected` | `submit_blocking` or `try_submit` advances the next ticket |
| `StopAccepting` | service command closes only new admission |
| `CloseDrainedFrontend` | no active workers and no pending raster envelope |
| `ComposeLatest` | owner composes the newest accepted CPU state |
| `RetireExact` | primary native retirement updates `CpuVisualProgress` |
| `FinishNormal` | `SessionQuiescenceDecision::Complete` |
| `StopAndDisconnect` | coordinator cancellation plus worker shutdown |

## 5. Checked Properties

- `OrderedDelivery`: no transaction overtakes an earlier ticket.
- `BoundedProducerOwnership`: producer-held work never exceeds the configured
  producer bound; there is no relay-owned accumulation.
- `SettlementsAreDisjoint`: an update is never both presented and superseded.
- `AcceptedUpdatesAccounted`: every accepted update is presented, superseded,
  or the current pending update.
- `NormalCompletionIsSettled`: normal completion retains no pending update.
- `NoUncancellableEgress`: producer-held work implies command handling is
  still reachable.

The default configuration checks 751 generated states, 302 distinct states,
and a complete state graph depth of 19.

## 6. Retained Negative Controls

- `XAuthorityShutdownPrematureExit.cfg` enables the original service-exit
  shape and must violate `NoUncancellableEgress`.
- `XAuthorityShutdownUnboundedIngress.cfg` removes the producer bound and must
  violate `BoundedProducerOwnership`.

`tools/check_tla.sh` requires each control to fail with that exact invariant.
A control that passes, or fails for another reason, fails the repository gate.

## 7. Implementation-Only Checks

- The observed Engine channel may have zero capacity; cancellation must still
  complete within one second in the deterministic regression.
- Wait, resume, transport failure, and shutdown telemetry preserve client and
  transaction identity even after a delivered batch moves into the channel.
- `StopAccepting` continues routing already-owned clients while refusing new
  admission.
- Quiescence suppresses new physical input and gives complete settlement at
  the deadline precedence over cancellation.
- Timeout telemetry names authority, CPU, and native blockers before issuing
  `StopAndDisconnect`.
- The physical terminal gate remains the final proof that the last CPU update
  reaches glass and greetd restoration completes on real hardware.
