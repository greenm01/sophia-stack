# Desktop Profile Activation Brief Coverage

This audit maps the modeling brief to the checked base and hunting artifacts.
It records the deliberate startup-only boundary instead of claiming coverage of
post-promotion restart behavior that the protocol has not yet specified.

## Scenarios

| Brief scenario | Base actions or state | Focused configuration |
|---|---|---|
| 1. Local proxy mistaken for external activation | `policyProxyPrepared`, `hagiaPreparedAccepted`, `hagiaActiveAccepted`, `SophiaAcceptProfileActive` | `MC_hunt_local_proxy.cfg` |
| 2. Stale acknowledgement across epochs | `DisconnectHagia`, `DiscardStaleAck`, `preparedEpoch`, `activeEpoch` | `MC_hunt_stale_epoch.cfg` |
| 3. File and wire identity diverge | `LaunchAndNegotiateHagia`, `HagiaHandleProfilePrepare` | `MC_hunt_identity_mismatch.cfg` |
| 4. Barrier conflicts with policy configuration | `HagiaQueuePolicyConfiguration`, `SophiaAcceptPolicyConfiguration` | `MC_hunt_configuration_order.cfg` |
| 5. Failure during partial activation | reject, timeout, disconnect, rollback, and failed-startup actions | `MC_hunt_partial_failure.cfg` |
| 6. Restart after successful startup | explicitly excluded after `phase = "active"` | `MC_hunt_restart_scope.cfg` proves the scoped transition system cannot restart after promotion |

## Safety Invariants

| Brief invariant | Defined in | Wired in | Enabled configurations |
|---|---|---|---|
| `GraphicsRequireGlobalActivation` | `base.tla` | `MC.tla` imports `base` | `MC_hunt_local_proxy.cfg` |
| `HagiaAckMatchesLoadedCandidate` | `base.tla` | `MC.tla` imports `base` | `MC_hunt_identity_mismatch.cfg` |
| `CompletionMatchesConnection` | `base.tla` | `MC.tla` imports `base` | `MC_hunt_stale_epoch.cfg` |
| `ConfigurationFollowsProfileActivation` | `base.tla` | `MC.tla` imports `base` | `MC_hunt_configuration_order.cfg` |
| `FailureCannotPromote` | `base.tla` | `MC.tla` imports `base` | stale-epoch, identity-mismatch, and partial-failure hunts |
| `NoPartialVisibility` | `base.tla` | `MC.tla` imports `base` | local-proxy and partial-failure hunts |
| `IdentityNamespacesAreDistinct` | `base.tla` | `MC.tla` imports `base` | `MC_hunt_configuration_order.cfg` |

The standard `MC.cfg` enables structural invariants and deliberately comments
out scenario invariants. Every scenario invariant is enabled above in at least
one focused hunt configuration read directly during this audit.

## Model-Checkable Findings

| Finding | Reachable trigger | Expected invariant | Hunt configuration |
|---|---|---|---|
| MC1 local proxy completion | local proxy is prepared before any external ACK | `GraphicsRequireGlobalActivation` | `MC_hunt_local_proxy.cfg` |
| MC2 old-epoch completion | disconnect advances epoch while an ACK can remain queued | `CompletionMatchesConnection` | `MC_hunt_stale_epoch.cfg` |
| MC3 early configuration | Hagia may queue configuration immediately after its active ACK | `ConfigurationFollowsProfileActivation` | `MC_hunt_configuration_order.cfg` |
| MC4 disconnect during partial activation | disconnect, rejection, and timeout remain enabled after any local subset | `FailureCannotPromote` | `MC_hunt_partial_failure.cfg` |

## Execution Record

The base configuration exhaustively checked 372,409 distinct states. The
standard MC configuration and all six focused configurations completed without
an invariant violation on 2026-08-10. The checked model is a design proof for
the proposed startup protocol, not evidence that the not-yet-implemented wire
path conforms to it; trace validation remains required after implementation.
