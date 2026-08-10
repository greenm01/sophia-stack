---------------- MODULE TraceData ----------------

\* Deterministic design trace for the proposed reattachment orchestration. It
\* proves wrapper completeness only. Implementation conformance requires the
\* future opt-in evidence events described by instrumentation-spec.md.

TraceState(coordinator, hagia, loadedGeneration, loadedDigest,
           currentEpoch, previousEpoch, epochHistory,
           outstandingKind, outstandingTransaction,
           usedCount, commandCount, ackCount, loadedCount, activeCount,
           configurationMessageCount, acceptedConfigurationCount,
           configurationAllowed, lastCompletionEpoch, attempts) ==
    [coordinatorPhase |-> coordinator,
     hagiaPhase |-> hagia,
     loadedGeneration |-> loadedGeneration,
     loadedDigest |-> loadedDigest,
     currentEpoch |-> currentEpoch,
     previousEpoch |-> previousEpoch,
     epochHistory |-> epochHistory,
     outstandingKind |-> outstandingKind,
     outstandingTransaction |-> outstandingTransaction,
     usedTransactionCount |-> usedCount,
     commandCount |-> commandCount,
     ackCount |-> ackCount,
     loadedEpochCount |-> loadedCount,
     activeEpochCount |-> activeCount,
     configurationMessageCount |-> configurationMessageCount,
     acceptedConfigurationCount |-> acceptedConfigurationCount,
     configurationAllowed |-> configurationAllowed,
     lastCompletionEpoch |-> lastCompletionEpoch,
     attempts |-> attempts]

Event(name, epoch, transaction, generation, digest, state) ==
    [tag |-> "trace",
     event |-> [name |-> name,
                epoch |-> epoch,
                transaction |-> transaction,
                generation |-> generation,
                digest |-> digest,
                state |-> state]]

S1 == TraceState("disconnected", "absent", 0, "none", 0, 1, <<1>>,
                 "none", 0, 0, 0, 0, 1, 1, 0, 0, FALSE, 0, 0)
S2 == TraceState("ready", "loaded", 1, "digest-a", 2, 1, <<1, 2>>,
                 "none", 0, 0, 0, 0, 2, 1, 0, 0, FALSE, 0, 1)
S3 == TraceState("awaitingPrepared", "loaded", 1, "digest-a", 2, 1,
                 <<1, 2>>, "prepare", 1, 1, 1, 0, 2, 1, 0, 0, FALSE, 0, 1)
S4 == TraceState("awaitingPrepared", "prepared", 1, "digest-a", 2, 1,
                 <<1, 2>>, "prepare", 1, 1, 0, 1, 2, 1, 0, 0, FALSE, 0, 1)
S5 == TraceState("prepared", "prepared", 1, "digest-a", 2, 1, <<1, 2>>,
                 "none", 0, 1, 0, 0, 2, 1, 0, 0, FALSE, 2, 1)
S6 == TraceState("awaitingActive", "prepared", 1, "digest-a", 2, 1,
                 <<1, 2>>, "activate", 2, 2, 1, 0, 2, 1, 0, 0, FALSE, 2, 1)
S7 == TraceState("awaitingActive", "active", 1, "digest-a", 2, 1,
                 <<1, 2>>, "activate", 2, 2, 0, 1, 2, 1, 0, 0, FALSE, 2, 1)
S8 == TraceState("active", "active", 1, "digest-a", 2, 1, <<1, 2>>,
                 "none", 0, 2, 0, 0, 2, 2, 0, 0, TRUE, 2, 1)
S9 == TraceState("active", "active", 1, "digest-a", 2, 1, <<1, 2>>,
                 "none", 0, 2, 0, 0, 2, 2, 1, 0, TRUE, 2, 1)
S10 == TraceState("active", "active", 1, "digest-a", 2, 1, <<1, 2>>,
                  "none", 0, 2, 0, 0, 2, 2, 0, 1, TRUE, 2, 1)

TraceLog == <<
    Event("DisconnectHagia", 1, 0, 1, "digest-a", S1),
    Event("BeginReplacement", 2, 0, 1, "digest-a", S2),
    Event("SendProfilePrepare", 2, 1, 1, "digest-a", S3),
    Event("HagiaHandleProfilePrepare", 2, 1, 1, "digest-a", S4),
    Event("SophiaSettleProfilePrepared", 2, 1, 1, "digest-a", S5),
    Event("SendProfileActivate", 2, 2, 1, "digest-a", S6),
    Event("HagiaHandleProfileActivate", 2, 2, 1, "digest-a", S7),
    Event("SophiaSettleProfileActive", 2, 2, 1, "digest-a", S8),
    Event("HagiaSendPolicyConfiguration", 2, 0, 1, "digest-a", S9),
    Event("SophiaAcceptPolicyConfiguration", 2, 0, 1, "digest-a", S10)
>>

=============================================================================
