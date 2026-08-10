---------------- MODULE TraceData ----------------

\* Deterministic design trace for the not-yet-implemented protocol. It proves
\* trace-wrapper completeness only; implementation conformance requires NDJSON
\* emitted by the future opt-in evidence hooks.

TraceState(phaseValue, candidateGeneration, candidateDigest,
           activeGeneration, activeDigest, latestGenerationValue,
           proxyPrepared, preparedCount, activatedCount, rollbackCount,
           hagiaPhaseValue, loadedGeneration, loadedDigest, epoch,
           outstandingKind, commandCount, ackCount, preparedAccepted,
           activeAccepted, rollbackAccepted, preparedEpochValue, activeEpochValue,
           rejectedCount, promotedCount, graphics, configQueued,
           configAccepted, configGeneration) ==
    [phase |-> phaseValue,
     candidateGeneration |-> candidateGeneration,
     candidateDigest |-> candidateDigest,
     activeGeneration |-> activeGeneration,
     activeDigest |-> activeDigest,
     latestGeneration |-> latestGenerationValue,
     policyProxyPrepared |-> proxyPrepared,
     localPreparedCount |-> preparedCount,
     localActivatedCount |-> activatedCount,
     rollbackPendingCount |-> rollbackCount,
     hagiaPhase |-> hagiaPhaseValue,
     loadedGeneration |-> loadedGeneration,
     loadedDigest |-> loadedDigest,
     connectionEpoch |-> epoch,
     outstandingKind |-> outstandingKind,
     commandCount |-> commandCount,
     ackCount |-> ackCount,
     hagiaPreparedAccepted |-> preparedAccepted,
     hagiaActiveAccepted |-> activeAccepted,
     hagiaRollbackAccepted |-> rollbackAccepted,
     preparedEpoch |-> preparedEpochValue,
     activeEpoch |-> activeEpochValue,
     rejectedCount |-> rejectedCount,
     promotedCount |-> promotedCount,
     graphicsOpen |-> graphics,
     configurationQueued |-> configQueued,
     configurationAccepted |-> configAccepted,
     configurationGeneration |-> configGeneration]

Event(name, generation, digest, authority, messageKind, epoch, transaction, outcome,
      configGeneration, state) ==
    [tag |-> "trace",
     event |-> [name |-> name,
                generation |-> generation,
                digest |-> digest,
                authority |-> authority,
                messageKind |-> messageKind,
                epoch |-> epoch,
                transaction |-> transaction,
                outcome |-> outcome,
                configurationGeneration |-> configGeneration,
                state |-> state]]

S1 == TraceState("preparing", 1, "digest-a", 0, "none", 1,
                 TRUE, 0, 0, 0, "absent", 0, "none", 0, "none", 0, 0,
                 FALSE, FALSE, FALSE, 0, 0, 0, 0, FALSE, FALSE, FALSE, 0)
S2 == TraceState("preparing", 1, "digest-a", 0, "none", 1,
                 TRUE, 1, 0, 0, "absent", 0, "none", 0, "none", 0, 0,
                 FALSE, FALSE, FALSE, 0, 0, 0, 0, FALSE, FALSE, FALSE, 0)
S3 == TraceState("preparing", 1, "digest-a", 0, "none", 1,
                 TRUE, 1, 0, 0, "loaded", 1, "digest-a", 1, "none", 0, 0,
                 FALSE, FALSE, FALSE, 0, 0, 0, 0, FALSE, FALSE, FALSE, 0)
S4 == TraceState("preparing", 1, "digest-a", 0, "none", 1,
                 TRUE, 1, 0, 0, "preparePending", 1, "digest-a", 1,
                 "prepare", 1, 0, FALSE, FALSE, FALSE, 0, 0, 0, 0,
                 FALSE, FALSE, FALSE, 0)
S5 == TraceState("preparing", 1, "digest-a", 0, "none", 1,
                 TRUE, 1, 0, 0, "prepared", 1, "digest-a", 1,
                 "prepare", 0, 1, FALSE, FALSE, FALSE, 0, 0, 0, 0,
                 FALSE, FALSE, FALSE, 0)
S6 == TraceState("prepared", 1, "digest-a", 0, "none", 1,
                 TRUE, 1, 0, 0, "prepared", 1, "digest-a", 1,
                 "none", 0, 0, TRUE, FALSE, FALSE, 1, 0, 0, 0,
                 FALSE, FALSE, FALSE, 0)
S7 == TraceState("activating", 1, "digest-a", 0, "none", 1,
                 TRUE, 1, 0, 0, "prepared", 1, "digest-a", 1,
                 "none", 0, 0, TRUE, FALSE, FALSE, 1, 0, 0, 0,
                 FALSE, FALSE, FALSE, 0)
S8 == TraceState("activating", 1, "digest-a", 0, "none", 1,
                 TRUE, 1, 1, 0, "prepared", 1, "digest-a", 1,
                 "none", 0, 0, TRUE, FALSE, FALSE, 1, 0, 0, 0,
                 FALSE, FALSE, FALSE, 0)
S9 == TraceState("activating", 1, "digest-a", 0, "none", 1,
                 TRUE, 1, 1, 0, "activatePending", 1, "digest-a", 1,
                 "activate", 1, 0, TRUE, FALSE, FALSE, 1, 0, 0, 0,
                 FALSE, FALSE, FALSE, 0)
S10 == TraceState("activating", 1, "digest-a", 0, "none", 1,
                  TRUE, 1, 1, 0, "active", 1, "digest-a", 1,
                  "activate", 0, 1, TRUE, FALSE, FALSE, 1, 0, 0, 0,
                  FALSE, FALSE, FALSE, 0)
S11 == TraceState("active", 1, "digest-a", 1, "digest-a", 1,
                  TRUE, 1, 1, 0, "active", 1, "digest-a", 1,
                  "none", 0, 0, TRUE, TRUE, FALSE, 1, 1, 0, 1,
                  TRUE, FALSE, FALSE, 0)
S12 == TraceState("active", 1, "digest-a", 1, "digest-a", 1,
                  TRUE, 1, 1, 0, "active", 1, "digest-a", 1,
                  "none", 0, 0, TRUE, TRUE, FALSE, 1, 1, 0, 1,
                  TRUE, TRUE, FALSE, 1)
S13 == TraceState("active", 1, "digest-a", 1, "digest-a", 1,
                  TRUE, 1, 1, 0, "active", 1, "digest-a", 1,
                  "none", 0, 0, TRUE, TRUE, FALSE, 1, 1, 0, 1,
                  TRUE, FALSE, TRUE, 1)

TraceLog == <<
    Event("StageDesktopProfile", 1, "digest-a", "none", "none", 0, 0, "none", 0, S1),
    Event("PrepareLocalAuthority", 1, "digest-a", "shell", "none", 0, 0, "none", 0, S2),
    Event("LaunchAndNegotiateHagia", 1, "digest-a", "none", "none", 1, 0, "none", 0, S3),
    Event("SendProfilePrepare", 1, "digest-a", "none", "prepare", 1, 1, "none", 0, S4),
    Event("HagiaHandleProfilePrepare", 1, "digest-a", "none", "prepare", 1, 1, "accepted", 0, S5),
    Event("SophiaAcceptProfilePrepared", 1, "digest-a", "none", "prepared", 1, 1, "accepted", 0, S6),
    Event("BeginProfileActivation", 1, "digest-a", "none", "none", 1, 0, "none", 0, S7),
    Event("ActivateLocalAuthority", 1, "digest-a", "shell", "none", 1, 0, "none", 0, S8),
    Event("SendProfileActivate", 1, "digest-a", "none", "activate", 1, 1, "none", 0, S9),
    Event("HagiaHandleProfileActivate", 1, "digest-a", "none", "activate", 1, 1, "accepted", 0, S10),
    Event("SophiaAcceptProfileActive", 1, "digest-a", "none", "active", 1, 1, "accepted", 0, S11),
    Event("HagiaQueuePolicyConfiguration", 1, "digest-a", "none", "configuration", 1, 0, "none", 1, S12),
    Event("SophiaAcceptPolicyConfiguration", 1, "digest-a", "none", "configuration", 1, 0, "none", 1, S13)
>>

=============================================================================
