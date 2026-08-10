---------------- MODULE Trace ----------------
EXTENDS base, TLC, TraceData

IOEnv == [default |-> TRUE]
JsonFile ==
    IF "JSON" \in DOMAIN IOEnv THEN IOEnv.JSON
    ELSE "../traces/reattachment.ndjson"

\* TraceData is the explicitly labeled design trace. A deterministic evidence
\* renderer will replace it with ndJsonDeserialize(JsonFile) after hooks exist.
ASSUME Len(TraceLog) > 0

VARIABLE l
traceVars == <<vars, l>>
logline == TraceLog[l]
IsEvent(name) == l <= Len(TraceLog) /\ logline.event.name = name
EventIdentity ==
    ProfileIdentity(logline.event.generation, logline.event.digest)

ValidatePostState ==
    /\ activeProfile' = InitialProfile
    /\ baselineProfile' = InitialProfile
    /\ graphicsOpen' = TRUE
    /\ layoutToken' = InitialLayoutToken
    /\ coordinatorPhase' = logline.event.state.coordinatorPhase
    /\ hagiaPhase' = logline.event.state.hagiaPhase
    /\ loadedProfile'.generation = logline.event.state.loadedGeneration
    /\ loadedProfile'.digest = logline.event.state.loadedDigest
    /\ currentEpoch' = logline.event.state.currentEpoch
    /\ previousEpoch' = logline.event.state.previousEpoch
    /\ epochHistory' = logline.event.state.epochHistory
    /\ outstanding'.kind = logline.event.state.outstandingKind
    /\ outstanding'.transaction = logline.event.state.outstandingTransaction
    /\ Cardinality(usedTransactions') =
         logline.event.state.usedTransactionCount
    /\ Cardinality(commands') = logline.event.state.commandCount
    /\ Cardinality(acks') = logline.event.state.ackCount
    /\ Cardinality(loadedByEpoch') = logline.event.state.loadedEpochCount
    /\ Cardinality(acceptedActive') = logline.event.state.activeEpochCount
    /\ Cardinality(configurationMessages') =
         logline.event.state.configurationMessageCount
    /\ Cardinality(acceptedConfigurations') =
         logline.event.state.acceptedConfigurationCount
    /\ configurationAllowed' = logline.event.state.configurationAllowed
    /\ lastCompletionEpoch' = logline.event.state.lastCompletionEpoch
    /\ attempts' = logline.event.state.attempts

TraceDisconnectHagia ==
    /\ IsEvent("DisconnectHagia")
    /\ DisconnectHagia
    /\ ValidatePostState
    /\ l' = l + 1

TraceBeginReplacement ==
    /\ IsEvent("BeginReplacement")
    /\ BeginReplacement(EventIdentity, logline.event.epoch)
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

TraceSophiaSettleProfilePrepared ==
    /\ IsEvent("SophiaSettleProfilePrepared")
    /\ \E reply \in acks :
         /\ reply = Ack("prepared", logline.event.epoch,
                        logline.event.transaction, EventIdentity, "accepted")
         /\ SophiaSettleProfilePrepared(reply)
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
         /\ HagiaHandleProfileActivate(message)
    /\ ValidatePostState
    /\ l' = l + 1

TraceSophiaSettleProfileActive ==
    /\ IsEvent("SophiaSettleProfileActive")
    /\ \E reply \in acks :
         /\ reply = Ack("active", logline.event.epoch,
                        logline.event.transaction, EventIdentity, "accepted")
         /\ SophiaSettleProfileActive(reply)
    /\ ValidatePostState
    /\ l' = l + 1

TraceHagiaSendPolicyConfiguration ==
    /\ IsEvent("HagiaSendPolicyConfiguration")
    /\ HagiaSendPolicyConfiguration
    /\ ValidatePostState
    /\ l' = l + 1

TraceSophiaAcceptPolicyConfiguration ==
    /\ IsEvent("SophiaAcceptPolicyConfiguration")
    /\ \E message \in configurationMessages :
         /\ message.epoch = logline.event.epoch
         /\ message.proven
         /\ SophiaAcceptPolicyConfiguration(message)
    /\ ValidatePostState
    /\ l' = l + 1

TraceInit == Init /\ l = 1

TraceAdvance ==
    \/ TraceDisconnectHagia
    \/ TraceBeginReplacement
    \/ TraceSendProfilePrepare
    \/ TraceHagiaHandleProfilePrepare
    \/ TraceSophiaSettleProfilePrepared
    \/ TraceSendProfileActivate
    \/ TraceHagiaHandleProfileActivate
    \/ TraceSophiaSettleProfileActive
    \/ TraceHagiaSendPolicyConfiguration
    \/ TraceSophiaAcceptPolicyConfiguration

TraceNext ==
    \/ TraceAdvance
    \/ /\ l > Len(TraceLog)
       /\ UNCHANGED traceVars

TraceMatched == <>(l > Len(TraceLog))
TraceView == traceVars
TraceSpec ==
    TraceInit /\ [][TraceNext]_traceVars /\ WF_traceVars(TraceAdvance)

=============================================================================
