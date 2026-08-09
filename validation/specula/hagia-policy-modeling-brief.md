# Hagia Public-Policy Settlement Modeling Brief

## 1. System Overview

Sophia's live Hagia path is a local supervised message-passing protocol written
in Rust, with an independently implemented Nim peer. It is Category A
(Distributed / Message-Passing): the critical behavior is split across an
Engine-owned reducer, a session owner loop, a bounded transport worker, and a
replaceable process connected by authenticated messages. The owner loop is the
only layout authority, while the worker performs blocking wire I/O. Complete
scene snapshots and projection proposals replace workspace-shaped API-v7
state. Frontend configure and renderable-content settlement occur after policy
validation but before the terminal projection outcome.

## 2. Scenarios

### Scenario 1: Transport Loss During Staged Frontend Settlement

**Mechanism**: Process replacement and frontend settlement are independently
scheduled, so restart must abort the exact pending layout before discarding its
staged reducer successor.

**Evidence**:

- Historical: `e24f8818` recovered legacy request-transport disconnects;
  `8dac227c` required restart/reseed after stale responses; `a2fdf4f6` hardened
  recovery boundaries after repeated admission failures.
- Code analysis: projection validation installs a staged successor before
  layout settlement (`wm/public_policy.rs:511`); process polling can observe
  exit while that layout remains pending (`wm/public_policy.rs:672`); terminal
  settlement consumes the staged identity (`wm/commit.rs:119`).
- Confirmed implementation defects: the initial public path cleared `staged`
  during restart while `PersistentLiveLayout::pending` retained its settlement
  identity, and prepare promoted reducer authority before the corresponding
  layout commit. The retained implementation adds an abort-before-restart
  barrier and makes prepare a non-mutating staged revalidation.

**Affected code paths**: `poll_public_request`, `poll_public_restart`,
`prepare_public_layout_commit`, `service_layout_progress!`,
`apply_public_commit_result`, `PersistentLiveLayout::expire_pending`.

**Suggested modeling approach**: distinguish staged reducer state, pending
frontend state, transport liveness, abort, reducer promotion, layout commit,
terminal outcome, and replacement epoch. Split crash, abort, promotion, commit,
and restart into separate actions.

**Priority**: High. A failure can terminate the compositor session or expose
policy/layout disagreement; the cross-product is well suited to model checking.

### Scenario 2: Terminal Outcome Backpressure And Connection Replacement

**Mechanism**: A logical terminal result is produced on the owner loop but sent
through a capacity-one worker channel and an independently failing socket.

**Evidence**:

- Historical: `d40d483b` bounded mixed-session handoffs and made their terminal
  ownership explicit; `8dac227c` found response attribution across replacement.
- Code analysis: every committed, rejected, timed-out, and session-operation
  outcome crosses `PolicyTransportCommand`; a full channel and a disconnected
  channel share the nonblocking failure result. The current worktree retains
  one deferred command until accepted or invalidated by epoch replacement.

**Affected code paths**: `LivePublicPolicyState::submit_or_defer`,
`flush_deferred_command`, `PolicyTransportWorker::try_command`,
`run_policy_transport`, `apply_public_commit_result`.

**Suggested modeling approach**: add a one-slot owner-to-worker channel,
terminal-pending identity, delivered-terminal set, and connection epoch. Model
full-channel retry separately from transport loss.

**Priority**: High. Dropping a terminal result can stall the peer or cause an
operation to be repeated after reconnect.

### Scenario 3: Ordered Actions And Session Operations

**Mechanism**: Replaceable scene work and continuous geometry may coalesce,
while physical action activations and session operations must remain ordered,
capability-scoped, and exactly once.

**Evidence**:

- Historical: `bfff582e` fixed action storms; `d40d483b` bounded handoffs.
- Code analysis: action causes retain distinct activation serials; session
  operations are accepted only after a committed projection and use fresh
  session-local opaque tokens. Live physical injection does not yet exercise
  every operation and saturation boundary.

**Affected code paths**: `queue_cause`, `enqueue_action`,
`PolicyRequestCause::Action`, session-operation validation and commit.

**Suggested modeling approach**: retain ordered activation identities, bounded
admission, operation capability epoch, and a terminal log. Do not model keycode
translation.

**Priority**: High. This is the authority boundary for launch, close, and
logout.

### Scenario 4: Output Identity And Atomic Multi-Output Projection

**Mechanism**: Output disappearance/reappearance, focus, membership, work-area
changes, and cross-output moves must share one generation-fenced snapshot.

**Evidence**:

- Historical: `5055a661` fixed workspace scanout visibility; `ef918108`
  isolated admission from stale workspace state.
- Code analysis: the public path maintains an output-generation ledger and
  complete affected-output proposals, but only focused deterministic tests
  cover reappearance; the live multi-output cross-product remains open.

**Affected code paths**: `observe_public_output_generations`, `snapshot`,
`update_public_work_areas`, `PolicyProjectionReducer`.

**Suggested modeling approach**: extend output identity with live/generation
state, require both outputs in atomic moves, and model output loss between stage
and settlement.

**Priority**: High. Output ABA or partial moves can misroute focus and pixels.

## 3. Modeling Recommendations

### 3.1 Model

- Crash/abort/restart at every staged settlement phase (Scenario 1).
- The replaceable capacity-one terminal command and epoch invalidation
  (Scenario 2).
- Ordered activation and operation completion alongside restart (Scenario 3).
- Output loss/reappearance during a staged multi-output move (Scenario 4).

### 3.2 Do Not Model

- Unix socket byte encoding: retained Rust/C/Nim corpora and malformed-frame
  tests are more precise.
- Pixel contents and renderer timing: model only the boolean frontend/content
  readiness boundary; visual ownership has separate TLA+ models.
- Executable launch details, logging, and CLI parsing: deterministic Rust and
  live-session tests are sufficient.
- API-v7 command translation: it remains a migration adapter with separate
  legacy models and must not shape the public contract.

## 4. Proposed Extensions

| Extension | Variables | Purpose | Scenario |
|-----------|-----------|---------|----------|
| Settlement phases | `phase`, `candidate`, `frontendReady` | Split validation, frontend wait, reducer promotion, and layout commit | 1 |
| Transport replacement | `transportLive`, `epoch`, `restartPending` | Prevent restart while old settlement owns state | 1, 2 |
| Terminal slot | `terminalPending`, `terminalDelivered` | Prove retry without duplication | 2, 3 |
| Output lifecycle | `outputLive`, `outputGeneration` | Fence output ABA and staged moves | 4 |

## 5. Proposed Invariants

| Invariant | Type | Description | Targets |
|-----------|------|-------------|---------|
| LastGoodIsCoherent | Safety | Committed layout and reducer serial always describe one logical projection | 1 |
| RestartHasNoOldOwner | Safety | A new epoch starts only after old pending/staged/terminal ownership is cleared | 1, 2 |
| TerminalAtMostOnce | Safety | One request identity has at most one delivered terminal outcome | 2, 3 |
| FailedSettlementPreservesLayout | Safety | Crash, timeout, and rejection cannot change last-good layout | 1 |
| AcceptedActionsRemainOrdered | Safety | Accepted activation identities terminate in admission order | 3 |
| ReappearedOutputIsFresh | Safety | Reappearing raw output identity has a greater generation | 4 |
| RestartEventuallyReissues | Liveness | After abort and replacement, a fresh scene request is eventually issued | 1, 2 |

## 6. Findings Pending Verification

### 6.1 Model-Checkable

| ID | Description | Expected invariant violation | Scenario |
|----|-------------|------------------------------|----------|
| MC1 | Can prepare-time reducer promotion expose unequal last-good states before layout commit? The negative control did; production now revalidates without promotion. | LastGoodIsCoherent | 1 |
| MC2 | Can a deferred terminal command survive epoch replacement and be delivered to the new peer? | RestartHasNoOldOwner / TerminalAtMostOnce | 2 |
| MC3 | Can output loss during a staged cross-output move commit only one side? | LastGoodIsCoherent | 4 |

### 6.2 Test-Verifiable

| ID | Description | Suggested test approach |
|----|-------------|-------------------------|
| TV1 | Crash at negotiation, snapshot, projection, frontend wait, terminal outcome, and operation phases | Phase-controlled Hagia subprocess fixture plus retained live session gate |
| TV2 | Repeated actions, saturation, launch, close, and logout remain ordered | Deterministic live injection and evidence reducer |
| TV3 | Multi-output loss/reappearance advances generation and preserves focus/membership | Reducer and headless live topology scenarios |

### 6.3 Code-Review-Only

| ID | Description | Suggested action |
|----|-------------|------------------|
| CR1 | `public_policy.rs` is near the source cohesion limit | Split state/snapshot, request, settlement, recovery, and proposal code before expansion |
| CR2 | Public capability negotiation currently assumes Hagia configuration ordering | Document and narrow the installed profile or negotiate the optional capability explicitly |

## 7. Reference Pointers

- Analysis coverage: 58 matching bug/recovery commits on `master` touched the
  broad WM/owner/reducer slice; the six settlement-relevant commits cited above
  were inspected in detail. The repository has no issue discussion corpus for
  this feature yet.
- Core sources: `wm/public_policy.rs`, `wm/commit.rs`,
  `policy_transport_worker.rs`, `wm/layout.rs`,
  `sophia-engine/src/policy_projection.rs`.
- Existing models: `PolicyLifecycle.tla`, `PolicyProjection.tla`,
  `VisualRetirement.tla`, and `AdmissionRecovery.tla`.
