# Desktop Profile Reattachment Verification Changelog

## Pre-convergence Construction

- [fix-spec] `InjectDelayedAcknowledgement`: restricted accepted delayed
  completions to an identity actually loaded in that historical epoch. The
  earlier fault action could synthesize an acknowledgement no Hagia reducer
  could emit.
- [fix-inv] `PreparedStateRequiresCurrentCompletion`: allowed Hagia's local
  phase to be Active while Sophia is still awaiting the corresponding Active
  completion. The previous invariant omitted this real send/settle window.
- [fix-spec] per-epoch transactions: reset `usedTransactions` when a fresh
  authenticated replacement begins. Correlation is epoch-scoped; global
  transaction non-reuse was stronger than the implementation architecture.

## Round 1 - Trace Validation

- [fix-spec] `TraceSpec`: added weak fairness specifically to `TraceAdvance`.
  Without it, the temporal checker could stutter before consuming the final
  design-trace event even though the cursor action remained enabled.

## Round 1 - Model Checking

- The standard structural configuration completed without a violation after
  the trace fix; no further spec or invariant changes were required.

## Result

Converged in one validation round after construction fixes. The base model,
standard bounded model, five focused hunts, and complete design trace pass. Bug
hunting found no model-checking bugs. This is design convergence only; runtime
conformance waits for production orchestration and opt-in implementation traces.
