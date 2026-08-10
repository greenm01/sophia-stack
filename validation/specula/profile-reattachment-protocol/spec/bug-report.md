# Bug Report — Desktop Profile Reattachment Protocol

## Summary

- Scenarios tested: 5
- Bugs found: 0
- Configs run: `MC_hunt_configuration_before_active.cfg`,
  `MC_hunt_stale_epoch.cfg`, `MC_hunt_identity_mismatch.cfg`,
  `MC_hunt_failure_preservation.cfg`, and
  `MC_hunt_fresh_correlation.cfg`
- Scope: proposed post-promotion reattachment design; implementation
  conformance is not yet claimed

## Not Reproduced

| Scenario | Config | Distinct states | Result |
|---|---|---:|---|
| Configuration before Active | `MC_hunt_configuration_before_active.cfg` | 57 | No violation |
| Old-epoch acknowledgement | `MC_hunt_stale_epoch.cfg` | 6,226 | No violation |
| Loaded identity mismatch | `MC_hunt_identity_mismatch.cfg` | 23 | No violation |
| Failure mutates committed session | `MC_hunt_failure_preservation.cfg` | 568 | No violation |
| Epoch-scoped correlation reuse | `MC_hunt_fresh_correlation.cfg` | 4,732 | No violation |

The standard bounded configuration also completed 48,679 distinct states, and
the base configuration completed 7,908. The focused configurations are small
finite proofs targeted at the modeling brief's concrete mechanisms; their
results do not substitute for future socket and restart regressions.
