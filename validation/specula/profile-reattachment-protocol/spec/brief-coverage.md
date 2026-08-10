# Desktop Profile Reattachment Brief Coverage

This audit maps the reattachment modeling brief to the checked base and focused
hunting artifacts. It covers recovery of the already-active policy authority;
startup activation and live profile reload remain separate protocols.

## Scenarios

| Brief scenario | Base actions or state | Focused configuration |
|---|---|---|
| 1. Replacement sends configuration immediately | `HagiaSendPolicyConfiguration`, `SophiaAcceptPolicyConfiguration`, `SophiaRejectPrematureConfiguration` | `MC_hunt_configuration_before_active.cfg` |
| 2. Old-epoch acknowledgement settles replacement | `InjectDelayedAcknowledgement`, `DiscardStaleAcknowledgement`, exact outstanding tuple | `MC_hunt_stale_epoch.cfg` |
| 3. Replacement loads a different candidate | `BeginReplacement`, `loadedByEpoch`, participant prepare comparison | `MC_hunt_identity_mismatch.cfg` |
| 4. Failure mutates committed session state | immutable profile, graphics, and layout sentinels across disconnect, timeout, rejection, retry, and degrade | `MC_hunt_failure_preservation.cfg` |
| 5. Correlation tuple reuse across incarnations | per-epoch `usedTransactions`, strictly increasing `epochHistory`, outstanding correlation | `MC_hunt_fresh_correlation.cfg` |

## Safety Invariants

| Brief invariant | Defined in | Enabled configurations |
|---|---|---|
| `ReattachmentRequiresExactActiveIdentity` | `base.tla` | identity-mismatch hunt |
| `ReattachmentUsesFreshEpoch` | `base.tla` | stale-epoch and fresh-correlation hunts |
| `ConfigurationRequiresReattachedActive` | `base.tla` | configuration-before-active and stale-epoch hunts |
| `ReattachmentCannotChangeGlobalProfile` | `base.tla` | identity-mismatch and failure-preservation hunts |
| `StaleReattachmentAckIsInert` | `base.tla` | configuration-before-active, stale-epoch, and fresh-correlation hunts |
| `ReattachmentFailurePreservesSession` | `base.tla` | failure-preservation hunt |
| `LoadedCandidateBoundsAcknowledgement` | `base.tla` | identity-mismatch hunt |

The standard `MC.cfg` enables structural invariants and deliberately comments
out scenario invariants. Every brief safety invariant is enabled in at least one
focused configuration listed above.

## Model-Checkable Findings

| Finding | Reachable trigger | Expected invariant | Hunt configuration |
|---|---|---|---|
| RM1 configuration before Active | bounded premature configuration send on a replacement epoch | `ConfigurationRequiresReattachedActive` | configuration-before-active |
| RM2 old-epoch Active settles replacement | bounded delayed acknowledgement after epoch advance | `StaleReattachmentAckIsInert` | stale-epoch |
| RM3 different loaded digest acknowledges active | mismatched replacement candidate reaches typed prepare rejection | `LoadedCandidateBoundsAcknowledgement` | identity-mismatch |
| RM4 recovery promotes another identity | mismatched load plus retry while active key remains frozen | `ReattachmentCannotChangeGlobalProfile` | identity-mismatch |
| RM5 failure disturbs graphics or layout | disconnect and timeout across two bounded attempts, then degrade | `ReattachmentFailurePreservesSession` | failure-preservation |

## Scope Notes

- The success-path `TraceData.tla` reaches exact Active reattachment and
  configuration admission. It is a design trace, not implementation
  conformance evidence.
- Bounded restart exhaustion is represented by `DegradeFailedReattachment`;
  supervisor wall-clock backoff and OS process identifiers are intentionally
  absent.
- No action admits a new profile identity. Cross-authority live reload is
  intentionally outside this transition system.

## Execution Record

On 2026-08-10 the base configuration exhaustively checked 7,908 distinct
states and the standard bounded configuration checked 48,679. The five hunts
checked 57 configuration-order, 6,226 stale-epoch, 23 identity-mismatch, 568
failure-preservation, and 4,732 fresh-correlation states. The complete design
trace checked 11 distinct states. Every run completed without an invariant or
temporal-property violation.
