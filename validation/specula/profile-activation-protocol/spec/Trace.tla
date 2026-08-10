---------------- MODULE Trace ----------------
EXTENDS base, TLC, TraceData

IOEnv == [default |-> TRUE]
JsonFile ==
    IF "JSON" \in DOMAIN IOEnv THEN IOEnv.JSON
    ELSE "../traces/startup.ndjson"

\* TraceData is currently the explicitly labeled design trace. After protocol
\* implementation, a deterministic renderer will translate JsonFile into the
\* same TraceLog shape without changing these action wrappers.
ASSUME Len(TraceLog) > 0

VARIABLE l
traceVars == <<vars, l>>
logline == TraceLog[l]
IsEvent(name) == l <= Len(TraceLog) /\ logline.event.name = name

EventIdentity ==
    ProfileIdentity(logline.event.generation, logline.event.digest)

ValidatePostState ==
    /\ phase' = logline.event.state.phase
    /\ candidate'.generation = logline.event.state.candidateGeneration
    /\ candidate'.digest = logline.event.state.candidateDigest
    /\ activeProfile'.generation = logline.event.state.activeGeneration
    /\ activeProfile'.digest = logline.event.state.activeDigest
    /\ latestGeneration' = logline.event.state.latestGeneration
    /\ policyProxyPrepared' = logline.event.state.policyProxyPrepared
    /\ Cardinality(localPrepared') = logline.event.state.localPreparedCount
    /\ Cardinality(localActivated') = logline.event.state.localActivatedCount
    /\ Cardinality(rollbackPending') = logline.event.state.rollbackPendingCount
    /\ hagiaPhase' = logline.event.state.hagiaPhase
    /\ hagiaLoaded'.generation = logline.event.state.loadedGeneration
    /\ hagiaLoaded'.digest = logline.event.state.loadedDigest
    /\ connectionEpoch' = logline.event.state.connectionEpoch
    /\ outstanding'.kind = logline.event.state.outstandingKind
    /\ Cardinality(commands') = logline.event.state.commandCount
    /\ Cardinality(acks') = logline.event.state.ackCount
    /\ hagiaPreparedAccepted' = logline.event.state.hagiaPreparedAccepted
    /\ hagiaActiveAccepted' = logline.event.state.hagiaActiveAccepted
    /\ hagiaRollbackAccepted' = logline.event.state.hagiaRollbackAccepted
    /\ preparedEpoch' = logline.event.state.preparedEpoch
    /\ activeEpoch' = logline.event.state.activeEpoch
    /\ Cardinality(rejectedProfiles') = logline.event.state.rejectedCount
    /\ Cardinality(promotedProfiles') = logline.event.state.promotedCount
    /\ graphicsOpen' = logline.event.state.graphicsOpen
    /\ configurationQueued' = logline.event.state.configurationQueued
    /\ configurationAccepted' = logline.event.state.configurationAccepted
    /\ configurationIdentity'.generation =
         logline.event.state.configurationGeneration

TraceStageDesktopProfile ==
    /\ IsEvent("StageDesktopProfile")
    /\ StageDesktopProfile(logline.event.generation, logline.event.digest)
    /\ ValidatePostState
    /\ l' = l + 1

TracePrepareLocalAuthority ==
    /\ IsEvent("PrepareLocalAuthority")
    /\ PrepareLocalAuthority(logline.event.authority)
    /\ ValidatePostState
    /\ l' = l + 1

TraceLaunchAndNegotiateHagia ==
    /\ IsEvent("LaunchAndNegotiateHagia")
    /\ LaunchAndNegotiateHagia(EventIdentity, logline.event.epoch)
    /\ ValidatePostState
    /\ l' = l + 1

TraceSendProfilePrepare ==
    /\ IsEvent("SendProfilePrepare")
    /\ SendProfilePrepare(logline.event.transaction)
    /\ ValidatePostState
    /\ l' = l + 1

TraceHagiaHandleProfilePrepare ==
    /\ IsEvent("HagiaHandleProfilePrepare")
    /\ \E message \in commands :
         /\ message = Command("prepare", logline.event.epoch,
                              logline.event.transaction, EventIdentity)
         /\ HagiaHandleProfilePrepare(message)
    /\ ValidatePostState
    /\ l' = l + 1

TraceSophiaAcceptProfilePrepared ==
    /\ IsEvent("SophiaAcceptProfilePrepared")
    /\ \E reply \in acks :
         /\ reply = Ack("prepared", logline.event.epoch,
                        logline.event.transaction, EventIdentity,
                        logline.event.outcome)
         /\ SophiaAcceptProfilePrepared(reply)
    /\ ValidatePostState
    /\ l' = l + 1

TraceBeginProfileActivation ==
    /\ IsEvent("BeginProfileActivation")
    /\ BeginProfileActivation
    /\ ValidatePostState
    /\ l' = l + 1

TraceActivateLocalAuthority ==
    /\ IsEvent("ActivateLocalAuthority")
    /\ ActivateLocalAuthority(logline.event.authority)
    /\ ValidatePostState
    /\ l' = l + 1

TraceSendProfileActivate ==
    /\ IsEvent("SendProfileActivate")
    /\ SendProfileActivate(logline.event.transaction)
    /\ ValidatePostState
    /\ l' = l + 1

TraceHagiaHandleProfileActivate ==
    /\ IsEvent("HagiaHandleProfileActivate")
    /\ \E message \in commands :
         /\ message = Command("activate", logline.event.epoch,
                              logline.event.transaction, EventIdentity)
         /\ HagiaHandleProfileActivate(message, logline.event.outcome)
    /\ ValidatePostState
    /\ l' = l + 1

TraceSophiaAcceptProfileActive ==
    /\ IsEvent("SophiaAcceptProfileActive")
    /\ \E reply \in acks :
         /\ reply = Ack("active", logline.event.epoch,
                        logline.event.transaction, EventIdentity,
                        logline.event.outcome)
         /\ SophiaAcceptProfileActive(reply)
    /\ ValidatePostState
    /\ l' = l + 1

TraceHagiaQueuePolicyConfiguration ==
    /\ IsEvent("HagiaQueuePolicyConfiguration")
    /\ HagiaQueuePolicyConfiguration(logline.event.configurationGeneration)
    /\ ValidatePostState
    /\ l' = l + 1

TraceSophiaAcceptPolicyConfiguration ==
    /\ IsEvent("SophiaAcceptPolicyConfiguration")
    /\ SophiaAcceptPolicyConfiguration
    /\ ValidatePostState
    /\ l' = l + 1

TraceSophiaRejectProfileCompletion ==
    /\ IsEvent("SophiaRejectProfileCompletion")
    /\ \E reply \in acks :
         /\ reply = Ack(logline.event.messageKind, logline.event.epoch,
                        logline.event.transaction, EventIdentity,
                        logline.event.outcome)
         /\ SophiaRejectProfileCompletion(reply)
    /\ ValidatePostState
    /\ l' = l + 1

TraceDisconnectHagia ==
    /\ IsEvent("DisconnectHagia")
    /\ DisconnectHagia(logline.event.epoch)
    /\ ValidatePostState
    /\ l' = l + 1

TraceTimeoutProfileOperation ==
    /\ IsEvent("TimeoutProfileOperation")
    /\ TimeoutProfileOperation
    /\ ValidatePostState
    /\ l' = l + 1

TraceRollbackLocalAuthority ==
    /\ IsEvent("RollbackLocalAuthority")
    /\ RollbackLocalAuthority(logline.event.authority)
    /\ ValidatePostState
    /\ l' = l + 1

TraceSendProfileRollback ==
    /\ IsEvent("SendProfileRollback")
    /\ SendProfileRollback(logline.event.transaction)
    /\ ValidatePostState
    /\ l' = l + 1

TraceHagiaHandleProfileRollback ==
    /\ IsEvent("HagiaHandleProfileRollback")
    /\ \E message \in commands :
         /\ message = Command("rollback", logline.event.epoch,
                              logline.event.transaction, EventIdentity)
         /\ HagiaHandleProfileRollback(message)
    /\ ValidatePostState
    /\ l' = l + 1

TraceSophiaAcceptProfileRolledBack ==
    /\ IsEvent("SophiaAcceptProfileRolledBack")
    /\ \E reply \in acks :
         /\ reply = Ack("rolledBack", logline.event.epoch,
                        logline.event.transaction, EventIdentity,
                        logline.event.outcome)
         /\ SophiaAcceptProfileRolledBack(reply)
    /\ ValidatePostState
    /\ l' = l + 1

TraceDiscardStaleAck ==
    /\ IsEvent("DiscardStaleAck")
    /\ \E reply \in acks :
         /\ reply = Ack(logline.event.messageKind, logline.event.epoch,
                        logline.event.transaction, EventIdentity,
                        logline.event.outcome)
         /\ DiscardStaleAck(reply)
    /\ ValidatePostState
    /\ l' = l + 1

TraceCompleteFailedStartup ==
    /\ IsEvent("CompleteFailedStartup")
    /\ CompleteFailedStartup
    /\ ValidatePostState
    /\ l' = l + 1

TraceInit == Init /\ l = 1

TraceNext ==
    \/ TraceStageDesktopProfile
    \/ TracePrepareLocalAuthority
    \/ TraceLaunchAndNegotiateHagia
    \/ TraceSendProfilePrepare
    \/ TraceHagiaHandleProfilePrepare
    \/ TraceSophiaAcceptProfilePrepared
    \/ TraceBeginProfileActivation
    \/ TraceActivateLocalAuthority
    \/ TraceSendProfileActivate
    \/ TraceHagiaHandleProfileActivate
    \/ TraceSophiaAcceptProfileActive
    \/ TraceHagiaQueuePolicyConfiguration
    \/ TraceSophiaAcceptPolicyConfiguration
    \/ TraceSophiaRejectProfileCompletion
    \/ TraceDisconnectHagia
    \/ TraceTimeoutProfileOperation
    \/ TraceRollbackLocalAuthority
    \/ TraceSendProfileRollback
    \/ TraceHagiaHandleProfileRollback
    \/ TraceSophiaAcceptProfileRolledBack
    \/ TraceDiscardStaleAck
    \/ TraceCompleteFailedStartup
    \/ /\ l > Len(TraceLog)
       /\ UNCHANGED traceVars

TraceMatched == WF_traceVars(TraceNext) => <>(l > Len(TraceLog))
TraceView == vars

=============================================================================
