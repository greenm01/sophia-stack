---------------- MODULE base ----------------
EXTENDS FiniteSets, Naturals, Sequences, TLC

CONSTANTS Generations, Digests, Transactions, Epochs,
          InitialGeneration, InitialDigest, InitialEpoch,
          NoGeneration, NoDigest, NoTransaction, NoEpoch, InitialLayoutToken

ASSUME /\ InitialGeneration \in Generations
       /\ InitialDigest \in Digests
       /\ InitialEpoch \in Epochs
       /\ NoGeneration = 0
       /\ NoGeneration \notin Generations
       /\ NoDigest \notin Digests
       /\ NoTransaction = 0
       /\ NoTransaction \notin Transactions
       /\ NoEpoch = 0
       /\ NoEpoch \notin Epochs
       /\ InitialLayoutToken > 0

ProfileIdentity(generation, digest) ==
    [generation |-> generation, digest |-> digest]
NoProfile == ProfileIdentity(NoGeneration, NoDigest)
ProfileIdentities ==
    {ProfileIdentity(generation, digest) :
        generation \in Generations, digest \in Digests}
InitialProfile == ProfileIdentity(InitialGeneration, InitialDigest)

EpochIdentity(epoch, identity) ==
    [epoch |-> epoch, identity |-> identity]
EpochIdentities ==
    {EpochIdentity(epoch, identity) :
        epoch \in Epochs, identity \in ProfileIdentities}

CommandKinds == {"prepare", "activate"}
AckKinds == {"prepared", "active"}
Outcomes == {"accepted", "rejected"}

Command(kind, epoch, transaction, identity) ==
    [kind |-> kind,
     epoch |-> epoch,
     transaction |-> transaction,
     identity |-> identity]

Ack(kind, epoch, transaction, identity, outcome) ==
    [kind |-> kind,
     epoch |-> epoch,
     transaction |-> transaction,
     identity |-> identity,
     outcome |-> outcome]

Commands ==
    {Command(kind, epoch, transaction, identity) :
        kind \in CommandKinds,
        epoch \in Epochs,
        transaction \in Transactions,
        identity \in ProfileIdentities}

Acks ==
    {Ack(kind, epoch, transaction, identity, outcome) :
        kind \in AckKinds,
        epoch \in Epochs,
        transaction \in Transactions,
        identity \in ProfileIdentities,
        outcome \in Outcomes}

NoOutstanding ==
    [kind |-> "none", epoch |-> NoEpoch, transaction |-> NoTransaction,
     identity |-> NoProfile]

Configuration(epoch, proven) == [epoch |-> epoch, proven |-> proven]
Configurations ==
    {Configuration(epoch, proven) : epoch \in Epochs, proven \in BOOLEAN}

CoordinatorPhases ==
    {"active", "disconnected", "ready", "awaitingPrepared", "prepared",
     "awaitingActive", "rejected", "degraded"}
HagiaPhases == {"active", "absent", "loaded", "prepared"}

VARIABLES activeProfile, baselineProfile, graphicsOpen, layoutToken,
          coordinatorPhase, hagiaPhase, loadedProfile,
          currentEpoch, previousEpoch, epochHistory,
          outstanding, usedTransactions, commands, acks,
          loadedByEpoch, acceptedActive,
          configurationMessages, acceptedConfigurations,
          configurationAllowed, lastCompletionEpoch, attempts

vars ==
    <<activeProfile, baselineProfile, graphicsOpen, layoutToken,
      coordinatorPhase, hagiaPhase, loadedProfile,
      currentEpoch, previousEpoch, epochHistory,
      outstanding, usedTransactions, commands, acks,
      loadedByEpoch, acceptedActive,
      configurationMessages, acceptedConfigurations,
      configurationAllowed, lastCompletionEpoch, attempts>>

Init ==
    /\ activeProfile = InitialProfile
    /\ baselineProfile = InitialProfile
    /\ graphicsOpen = TRUE
    /\ layoutToken = InitialLayoutToken
    /\ coordinatorPhase = "active"
    /\ hagiaPhase = "active"
    /\ loadedProfile = InitialProfile
    /\ currentEpoch = InitialEpoch
    /\ previousEpoch = NoEpoch
    /\ epochHistory = <<InitialEpoch>>
    /\ outstanding = NoOutstanding
    /\ usedTransactions = {}
    /\ commands = {}
    /\ acks = {}
    /\ loadedByEpoch = {EpochIdentity(InitialEpoch, InitialProfile)}
    /\ acceptedActive = {EpochIdentity(InitialEpoch, InitialProfile)}
    /\ configurationMessages = {}
    /\ acceptedConfigurations = {}
    /\ configurationAllowed = TRUE
    /\ lastCompletionEpoch = InitialEpoch
    /\ attempts = 0

EpochsIn(history) == {history[index] : index \in 1..Len(history)}

StrictlyIncreasing(history) ==
    \A left, right \in 1..Len(history) :
        left < right => history[left] < history[right]

ExpectedAckKind(kind) == IF kind = "prepare" THEN "prepared" ELSE "active"

ExactOutstandingAck(reply) ==
    /\ outstanding # NoOutstanding
    /\ reply.kind = ExpectedAckKind(outstanding.kind)
    /\ reply.epoch = outstanding.epoch
    /\ reply.transaction = outstanding.transaction
    /\ reply.identity = outstanding.identity
    /\ reply.epoch = currentEpoch
    /\ reply.identity = activeProfile

CanAcceptConfiguration(message) ==
    /\ message.epoch = currentEpoch
    /\ message.proven
    /\ coordinatorPhase = "active"
    /\ hagiaPhase = "active"
    /\ configurationAllowed
    /\ EpochIdentity(message.epoch, activeProfile) \in acceptedActive

\* public_policy.rs:1029-1064 and policy_profile_handoff.rs:130-133.
\* A lost worker clears per-connection admission but preserves committed state.
DisconnectHagia ==
    /\ currentEpoch # NoEpoch
    /\ coordinatorPhase # "degraded"
    /\ previousEpoch' = currentEpoch
    /\ currentEpoch' = NoEpoch
    /\ coordinatorPhase' = "disconnected"
    /\ hagiaPhase' = "absent"
    /\ loadedProfile' = NoProfile
    /\ outstanding' = NoOutstanding
    /\ commands' = {}
    /\ configurationMessages' = {}
    /\ configurationAllowed' = FALSE
    /\ lastCompletionEpoch' = NoEpoch
    /\ UNCHANGED <<activeProfile, baselineProfile, graphicsOpen, layoutToken,
                    epochHistory, usedTransactions, acks, loadedByEpoch,
                    acceptedActive, acceptedConfigurations, attempts>>

\* public_policy.rs:1066-1106 and policy_transport.rs:158-200.
\* Each authenticated replacement gets a strictly newer epoch and reloads one
\* retained authority-local candidate. Scenario 2 and scenario 3.
BeginReplacement(loaded, epoch) ==
    /\ coordinatorPhase \in {"disconnected", "rejected"}
    /\ currentEpoch = NoEpoch
    /\ loaded \in ProfileIdentities
    /\ epoch \in Epochs
    /\ epoch > previousEpoch
    /\ epoch \notin EpochsIn(epochHistory)
    /\ coordinatorPhase' = "ready"
    /\ hagiaPhase' = "loaded"
    /\ loadedProfile' = loaded
    /\ currentEpoch' = epoch
    /\ epochHistory' = Append(epochHistory, epoch)
    /\ loadedByEpoch' = loadedByEpoch \cup {EpochIdentity(epoch, loaded)}
    /\ attempts' = attempts + 1
    /\ usedTransactions' = {}
    /\ outstanding' = NoOutstanding
    /\ commands' = {}
    /\ configurationMessages' = {}
    /\ configurationAllowed' = FALSE
    /\ lastCompletionEpoch' = NoEpoch
    /\ UNCHANGED <<activeProfile, baselineProfile, graphicsOpen, layoutToken,
                    previousEpoch, acks, acceptedActive,
                    acceptedConfigurations>>

\* policy_profile_handoff.rs:138-183 and policy_transport.rs:412-432.
\* The coordinator's immutable identity is the only reattachment command key.
SendProfilePrepare(transaction) ==
    /\ coordinatorPhase = "ready"
    /\ hagiaPhase = "loaded"
    /\ transaction \in Transactions \ usedTransactions
    /\ outstanding = NoOutstanding
    /\ LET message == Command("prepare", currentEpoch, transaction, activeProfile)
       IN /\ commands' = commands \cup {message}
          /\ outstanding' = message
    /\ usedTransactions' = usedTransactions \cup {transaction}
    /\ coordinatorPhase' = "awaitingPrepared"
    /\ UNCHANGED <<activeProfile, baselineProfile, graphicsOpen, layoutToken,
                    hagiaPhase, loadedProfile, currentEpoch, previousEpoch,
                    epochHistory, acks, loadedByEpoch, acceptedActive,
                    configurationMessages, acceptedConfigurations,
                    configurationAllowed, lastCompletionEpoch, attempts>>

\* Hagia profile_handoff.nim:96-121 and policy_client.nim:172-200.
\* The participant accepts prepare only for its exact loaded epoch and identity.
HagiaHandleProfilePrepare(message) ==
    /\ message \in commands
    /\ message.kind = "prepare"
    /\ message.epoch = currentEpoch
    /\ coordinatorPhase = "awaitingPrepared"
    /\ hagiaPhase = "loaded"
    /\ commands' = commands \ {message}
    /\ LET accepted == message.identity = loadedProfile
           outcome == IF accepted THEN "accepted" ELSE "rejected"
           reply == Ack("prepared", message.epoch, message.transaction,
                        message.identity, outcome)
       IN /\ acks' = acks \cup {reply}
          /\ hagiaPhase' = IF accepted THEN "prepared" ELSE "loaded"
    /\ UNCHANGED <<activeProfile, baselineProfile, graphicsOpen, layoutToken,
                    coordinatorPhase, loadedProfile, currentEpoch, previousEpoch,
                    epochHistory, outstanding, usedTransactions, loadedByEpoch,
                    acceptedActive, configurationMessages,
                    acceptedConfigurations, configurationAllowed,
                    lastCompletionEpoch, attempts>>

\* policy_profile_handoff.rs:186-227 and policy_transport.rs:312-335.
\* Exact correlation settles; typed rejection ends this attempt. Scenario 2.
SophiaSettleProfilePrepared(reply) ==
    /\ coordinatorPhase = "awaitingPrepared"
    /\ reply \in acks
    /\ ExactOutstandingAck(reply)
    /\ acks' = acks \ {reply}
    /\ outstanding' = NoOutstanding
    /\ coordinatorPhase' =
         IF reply.outcome = "accepted" THEN "prepared" ELSE "rejected"
    /\ lastCompletionEpoch' = currentEpoch
    /\ UNCHANGED <<activeProfile, baselineProfile, graphicsOpen, layoutToken,
                    hagiaPhase, loadedProfile, currentEpoch, previousEpoch,
                    epochHistory, usedTransactions, commands, loadedByEpoch,
                    acceptedActive, configurationMessages,
                    acceptedConfigurations, configurationAllowed, attempts>>

\* policy_profile_handoff.rs:138-183 and policy_transport.rs:412-432.
\* Activation follows an exact prepared completion and uses a fresh transaction.
SendProfileActivate(transaction) ==
    /\ coordinatorPhase = "prepared"
    /\ hagiaPhase = "prepared"
    /\ transaction \in Transactions \ usedTransactions
    /\ outstanding = NoOutstanding
    /\ LET message == Command("activate", currentEpoch, transaction, activeProfile)
       IN /\ commands' = commands \cup {message}
          /\ outstanding' = message
    /\ usedTransactions' = usedTransactions \cup {transaction}
    /\ coordinatorPhase' = "awaitingActive"
    /\ UNCHANGED <<activeProfile, baselineProfile, graphicsOpen, layoutToken,
                    hagiaPhase, loadedProfile, currentEpoch, previousEpoch,
                    epochHistory, acks, loadedByEpoch, acceptedActive,
                    configurationMessages, acceptedConfigurations,
                    configurationAllowed, lastCompletionEpoch, attempts>>

\* Hagia profile_handoff.nim:122-136 and policy_client.nim:215-230.
\* Active acknowledgement requires the participant's exact prepared identity.
HagiaHandleProfileActivate(message) ==
    /\ message \in commands
    /\ message.kind = "activate"
    /\ message.epoch = currentEpoch
    /\ coordinatorPhase = "awaitingActive"
    /\ commands' = commands \ {message}
    /\ LET accepted ==
             /\ hagiaPhase = "prepared"
             /\ message.identity = loadedProfile
           outcome == IF accepted THEN "accepted" ELSE "rejected"
           reply == Ack("active", message.epoch, message.transaction,
                        message.identity, outcome)
       IN /\ acks' = acks \cup {reply}
          /\ hagiaPhase' = IF accepted THEN "active" ELSE hagiaPhase
    /\ UNCHANGED <<activeProfile, baselineProfile, graphicsOpen, layoutToken,
                    coordinatorPhase, loadedProfile, currentEpoch, previousEpoch,
                    epochHistory, outstanding, usedTransactions, loadedByEpoch,
                    acceptedActive, configurationMessages,
                    acceptedConfigurations, configurationAllowed,
                    lastCompletionEpoch, attempts>>

\* policy_profile_handoff.rs:186-227. The future public worker must perform
\* this settlement before policy_transport_worker.rs:141-156. Scenario 1.
SophiaSettleProfileActive(reply) ==
    /\ coordinatorPhase = "awaitingActive"
    /\ reply \in acks
    /\ ExactOutstandingAck(reply)
    /\ acks' = acks \ {reply}
    /\ outstanding' = NoOutstanding
    /\ coordinatorPhase' =
         IF reply.outcome = "accepted" THEN "active" ELSE "rejected"
    /\ acceptedActive' =
         IF reply.outcome = "accepted"
         THEN acceptedActive \cup {EpochIdentity(currentEpoch, activeProfile)}
         ELSE acceptedActive
    /\ configurationAllowed' = (reply.outcome = "accepted")
    /\ lastCompletionEpoch' = currentEpoch
    /\ UNCHANGED <<activeProfile, baselineProfile, graphicsOpen, layoutToken,
                    hagiaPhase, loadedProfile, currentEpoch, previousEpoch,
                    epochHistory, usedTransactions, commands, loadedByEpoch,
                    configurationMessages, acceptedConfigurations, attempts>>

\* policy_transport_worker.rs:141-156 and Hagia policy_client.nim:202-230.
\* A message records whether Hagia sent it after its local Active phase.
HagiaSendPolicyConfiguration ==
    /\ currentEpoch # NoEpoch
    /\ LET proven ==
             /\ coordinatorPhase = "active"
             /\ hagiaPhase = "active"
             /\ configurationAllowed
           message == Configuration(currentEpoch, proven)
       IN /\ message \notin configurationMessages
          /\ configurationMessages' = configurationMessages \cup {message}
    /\ UNCHANGED <<activeProfile, baselineProfile, graphicsOpen, layoutToken,
                    coordinatorPhase, hagiaPhase, loadedProfile,
                    currentEpoch, previousEpoch, epochHistory, outstanding,
                    usedTransactions, commands, acks, loadedByEpoch,
                    acceptedActive, acceptedConfigurations,
                    configurationAllowed, lastCompletionEpoch, attempts>>

\* Future admission gate immediately before policy_transport_worker.rs:141-156.
SophiaAcceptPolicyConfiguration(message) ==
    /\ message \in configurationMessages
    /\ CanAcceptConfiguration(message)
    /\ configurationMessages' = configurationMessages \ {message}
    /\ acceptedConfigurations' = acceptedConfigurations \cup {message}
    /\ UNCHANGED <<activeProfile, baselineProfile, graphicsOpen, layoutToken,
                    coordinatorPhase, hagiaPhase, loadedProfile,
                    currentEpoch, previousEpoch, epochHistory, outstanding,
                    usedTransactions, commands, acks, loadedByEpoch,
                    acceptedActive, configurationAllowed,
                    lastCompletionEpoch, attempts>>

\* policy_transport_worker.rs:148-150,169-170. Premature normal traffic fails
\* this connection rather than being buffered until activation. Scenario 1.
SophiaRejectPrematureConfiguration(message) ==
    /\ message \in configurationMessages
    /\ ~CanAcceptConfiguration(message)
    /\ configurationMessages' = configurationMessages \ {message}
    /\ UNCHANGED <<activeProfile, baselineProfile, graphicsOpen, layoutToken,
                    coordinatorPhase, hagiaPhase, loadedProfile,
                    currentEpoch, previousEpoch, epochHistory, outstanding,
                    usedTransactions, commands, acks, loadedByEpoch,
                    acceptedActive, acceptedConfigurations,
                    configurationAllowed, lastCompletionEpoch, attempts>>

\* policy_transport.rs:312-335 and policy_profile_handoff.rs:191-207.
\* Models a delayed completion already emitted by an earlier incarnation.
InjectDelayedAcknowledgement(kind, epoch, transaction) ==
    /\ kind \in AckKinds
    /\ epoch \in EpochsIn(epochHistory) \ {currentEpoch}
    /\ transaction \in Transactions
    /\ EpochIdentity(epoch, activeProfile) \in loadedByEpoch
    /\ LET reply == Ack(kind, epoch, transaction, activeProfile, "accepted")
       IN /\ reply \notin acks
          /\ acks' = acks \cup {reply}
    /\ UNCHANGED <<activeProfile, baselineProfile, graphicsOpen, layoutToken,
                    coordinatorPhase, hagiaPhase, loadedProfile,
                    currentEpoch, previousEpoch, epochHistory, outstanding,
                    usedTransactions, commands, loadedByEpoch, acceptedActive,
                    configurationMessages, acceptedConfigurations,
                    configurationAllowed, lastCompletionEpoch, attempts>>

\* policy_profile_handoff.rs:191-207. A non-exact completion is consumed with
\* no phase, identity, or admission mutation. Scenario 2.
DiscardStaleAcknowledgement(reply) ==
    /\ reply \in acks
    /\ ~ExactOutstandingAck(reply)
    /\ acks' = acks \ {reply}
    /\ UNCHANGED <<activeProfile, baselineProfile, graphicsOpen, layoutToken,
                    coordinatorPhase, hagiaPhase, loadedProfile,
                    currentEpoch, previousEpoch, epochHistory, outstanding,
                    usedTransactions, commands, loadedByEpoch, acceptedActive,
                    configurationMessages, acceptedConfigurations,
                    configurationAllowed, lastCompletionEpoch, attempts>>

\* policy_transport_worker.rs:128-143 and public_policy.rs:1015-1064.
\* A bounded receive failure clears only the failed connection attempt.
TimeoutProfileOperation ==
    /\ coordinatorPhase \in {"awaitingPrepared", "awaitingActive"}
    /\ previousEpoch' = currentEpoch
    /\ currentEpoch' = NoEpoch
    /\ coordinatorPhase' = "disconnected"
    /\ hagiaPhase' = "absent"
    /\ loadedProfile' = NoProfile
    /\ outstanding' = NoOutstanding
    /\ commands' = {}
    /\ configurationMessages' = {}
    /\ configurationAllowed' = FALSE
    /\ lastCompletionEpoch' = NoEpoch
    /\ UNCHANGED <<activeProfile, baselineProfile, graphicsOpen, layoutToken,
                    epochHistory, usedTransactions, acks, loadedByEpoch,
                    acceptedActive, acceptedConfigurations, attempts>>

\* public_policy.rs:1085-1094. Exhaustion degrades policy service while the
\* already committed graphical session and profile remain intact. Scenario 4.
DegradeFailedReattachment ==
    /\ coordinatorPhase \in {"disconnected", "rejected"}
    /\ attempts > 0
    /\ previousEpoch' =
         IF currentEpoch = NoEpoch THEN previousEpoch ELSE currentEpoch
    /\ currentEpoch' = NoEpoch
    /\ coordinatorPhase' = "degraded"
    /\ hagiaPhase' = "absent"
    /\ loadedProfile' = NoProfile
    /\ outstanding' = NoOutstanding
    /\ commands' = {}
    /\ configurationMessages' = {}
    /\ configurationAllowed' = FALSE
    /\ lastCompletionEpoch' = NoEpoch
    /\ UNCHANGED <<activeProfile, baselineProfile, graphicsOpen, layoutToken,
                    epochHistory, usedTransactions, acks, loadedByEpoch,
                    acceptedActive, acceptedConfigurations, attempts>>

Next ==
    \/ DisconnectHagia
    \/ \E loaded \in ProfileIdentities, epoch \in Epochs :
           BeginReplacement(loaded, epoch)
    \/ \E transaction \in Transactions : SendProfilePrepare(transaction)
    \/ \E message \in commands : HagiaHandleProfilePrepare(message)
    \/ \E reply \in acks : SophiaSettleProfilePrepared(reply)
    \/ \E transaction \in Transactions : SendProfileActivate(transaction)
    \/ \E message \in commands : HagiaHandleProfileActivate(message)
    \/ \E reply \in acks : SophiaSettleProfileActive(reply)
    \/ HagiaSendPolicyConfiguration
    \/ \E message \in configurationMessages :
           SophiaAcceptPolicyConfiguration(message)
    \/ \E message \in configurationMessages :
           SophiaRejectPrematureConfiguration(message)
    \/ \E kind \in AckKinds, epoch \in Epochs, transaction \in Transactions :
           InjectDelayedAcknowledgement(kind, epoch, transaction)
    \/ \E reply \in acks : DiscardStaleAcknowledgement(reply)
    \/ TimeoutProfileOperation
    \/ DegradeFailedReattachment

TypeOK ==
    /\ activeProfile \in ProfileIdentities
    /\ baselineProfile \in ProfileIdentities
    /\ graphicsOpen \in BOOLEAN
    /\ layoutToken \in Nat
    /\ coordinatorPhase \in CoordinatorPhases
    /\ hagiaPhase \in HagiaPhases
    /\ loadedProfile \in ProfileIdentities \cup {NoProfile}
    /\ currentEpoch \in Epochs \cup {NoEpoch}
    /\ previousEpoch \in Epochs \cup {NoEpoch}
    /\ epochHistory \in Seq(Epochs)
    /\ Len(epochHistory) > 0
    /\ outstanding \in Commands \cup {NoOutstanding}
    /\ usedTransactions \subseteq Transactions
    /\ commands \subseteq Commands
    /\ acks \subseteq Acks
    /\ loadedByEpoch \subseteq EpochIdentities
    /\ acceptedActive \subseteq EpochIdentities
    /\ configurationMessages \subseteq Configurations
    /\ acceptedConfigurations \subseteq Configurations
    /\ configurationAllowed \in BOOLEAN
    /\ lastCompletionEpoch \in Epochs \cup {NoEpoch}
    /\ attempts \in Nat

\* Scenario 3: a successful reattachment can name only the immutable active key.
ReattachmentRequiresExactActiveIdentity ==
    \A accepted \in acceptedActive : accepted.identity = activeProfile

\* Scenario 2 and scenario 5: authenticated incarnations never reuse epochs.
ReattachmentUsesFreshEpoch == StrictlyIncreasing(epochHistory)

\* Scenario 1: every admitted configuration carries proof from an epoch that
\* completed exact reattachment.
ConfigurationRequiresReattachedActive ==
    \A message \in acceptedConfigurations :
        /\ message.proven
        /\ EpochIdentity(message.epoch, activeProfile) \in acceptedActive

\* Scenario 3: reattachment is recovery, never implicit live reload.
ReattachmentCannotChangeGlobalProfile == activeProfile = baselineProfile

\* Scenario 2: stale settlement cannot unlock admission for a current peer.
StaleReattachmentAckIsInert ==
    configurationAllowed =>
        /\ coordinatorPhase = "active"
        /\ hagiaPhase = "active"
        /\ currentEpoch = lastCompletionEpoch
        /\ EpochIdentity(currentEpoch, activeProfile) \in acceptedActive

\* Scenario 4: recovery failures do not disturb committed session sentinels.
ReattachmentFailurePreservesSession ==
    /\ graphicsOpen
    /\ layoutToken = InitialLayoutToken
    /\ activeProfile = InitialProfile

\* Scenario 3: every accepted participant acknowledgement is bounded by the
\* candidate loaded for that acknowledgement's epoch.
LoadedCandidateBoundsAcknowledgement ==
    \A reply \in acks :
        reply.outcome = "accepted" =>
            EpochIdentity(reply.epoch, reply.identity) \in loadedByEpoch

OutstandingCorrelationIsCurrent ==
    outstanding # NoOutstanding =>
        /\ outstanding.epoch = currentEpoch
        /\ outstanding.identity = activeProfile
        /\ outstanding.transaction \in usedTransactions

PreparedStateRequiresCurrentCompletion ==
    coordinatorPhase \in {"prepared", "awaitingActive"} =>
        /\ lastCompletionEpoch = currentEpoch
        /\ hagiaPhase \in {"prepared", "active"}

=============================================================================
