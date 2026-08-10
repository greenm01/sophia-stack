---------------- MODULE base ----------------
EXTENDS FiniteSets, Naturals, Sequences, TLC

CONSTANTS LocalAuthorities, Generations, Digests, Transactions, Epochs,
          ConfigGenerations, NoGeneration, NoDigest, NoTransaction, NoEpoch

ASSUME /\ LocalAuthorities # {}
       /\ NoGeneration = 0
       /\ NoGeneration \notin Generations
       /\ NoDigest \notin Digests
       /\ NoTransaction = 0
       /\ NoTransaction \notin Transactions
       /\ NoEpoch = 0
       /\ NoEpoch \notin Epochs

ProfileIdentity(g, d) ==
    [namespace |-> "desktopProfile", generation |-> g, digest |-> d]
NoProfile == ProfileIdentity(NoGeneration, NoDigest)
ProfileIdentities ==
    {ProfileIdentity(g, d) : g \in Generations, d \in Digests}

ConfigurationIdentity(g) ==
    [namespace |-> "policyConfiguration", generation |-> g]

CommandKinds == {"prepare", "activate", "rollback"}
AckKinds == {"prepared", "active", "rolledBack"}
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

Phases ==
    {"idle", "preparing", "prepared", "activating", "rollingBack",
     "active", "failed"}
HagiaPhases ==
    {"absent", "loaded", "preparePending", "prepared",
     "activatePending", "active", "rollbackPending", "rolledBack",
     "disconnected"}

VARIABLES phase, candidate, activeProfile, latestGeneration,
          policyProxyPrepared, localPrepared, localActivated, rollbackPending,
          hagiaPhase, hagiaLoaded, connectionEpoch, outstanding,
          commands, acks, hagiaPreparedAccepted, hagiaActiveAccepted,
          hagiaRollbackAccepted, preparedEpoch, activeEpoch,
          rejectedProfiles, promotedProfiles, graphicsOpen,
          configurationQueued, configurationAccepted, configurationIdentity

vars == <<phase, candidate, activeProfile, latestGeneration,
          policyProxyPrepared, localPrepared, localActivated, rollbackPending,
          hagiaPhase, hagiaLoaded, connectionEpoch, outstanding,
          commands, acks, hagiaPreparedAccepted, hagiaActiveAccepted,
          hagiaRollbackAccepted, preparedEpoch, activeEpoch,
          rejectedProfiles, promotedProfiles, graphicsOpen,
          configurationQueued, configurationAccepted, configurationIdentity>>

Init ==
    /\ phase = "idle"
    /\ candidate = NoProfile
    /\ activeProfile = NoProfile
    /\ latestGeneration = NoGeneration
    /\ policyProxyPrepared = FALSE
    /\ localPrepared = {}
    /\ localActivated = {}
    /\ rollbackPending = {}
    /\ hagiaPhase = "absent"
    /\ hagiaLoaded = NoProfile
    /\ connectionEpoch = NoEpoch
    /\ outstanding = NoOutstanding
    /\ commands = {}
    /\ acks = {}
    /\ hagiaPreparedAccepted = FALSE
    /\ hagiaActiveAccepted = FALSE
    /\ hagiaRollbackAccepted = FALSE
    /\ preparedEpoch = NoEpoch
    /\ activeEpoch = NoEpoch
    /\ rejectedProfiles = {}
    /\ promotedProfiles = {}
    /\ graphicsOpen = FALSE
    /\ configurationQueued = FALSE
    /\ configurationAccepted = FALSE
    /\ configurationIdentity = ConfigurationIdentity(NoGeneration)

\* profile_preparation.rs:8-22 and public_policy.rs:89-126.
\* Retain the immutable identity and prepare only Sophia's staging proxy.
StageDesktopProfile(g, d) ==
    /\ phase = "idle"
    /\ g \in Generations
    /\ d \in Digests
    /\ g > latestGeneration
    /\ candidate' = ProfileIdentity(g, d)
    /\ latestGeneration' = g
    /\ policyProxyPrepared' = TRUE
    /\ phase' = "preparing"
    /\ UNCHANGED <<activeProfile, localPrepared, localActivated,
                    rollbackPending, hagiaPhase, hagiaLoaded, connectionEpoch,
                    outstanding, commands, acks, hagiaPreparedAccepted,
                    hagiaActiveAccepted, hagiaRollbackAccepted, preparedEpoch,
                    activeEpoch, rejectedProfiles, promotedProfiles,
                    graphicsOpen, configurationQueued, configurationAccepted,
                    configurationIdentity>>

\* profile_preparation.rs:110-177. Each Sophia-owned authority prepares
\* independently; policy is excluded because Hagia owns that completion.
PrepareLocalAuthority(authority) ==
    /\ phase = "preparing"
    /\ authority \in LocalAuthorities \ localPrepared
    /\ localPrepared' = localPrepared \cup {authority}
    /\ UNCHANGED <<phase, candidate, activeProfile, latestGeneration,
                    policyProxyPrepared, localActivated, rollbackPending,
                    hagiaPhase, hagiaLoaded, connectionEpoch, outstanding,
                    commands, acks, hagiaPreparedAccepted, hagiaActiveAccepted,
                    hagiaRollbackAccepted, preparedEpoch, activeEpoch,
                    rejectedProfiles, promotedProfiles, graphicsOpen,
                    configurationQueued, configurationAccepted,
                    configurationIdentity>>

\* public_policy.rs:588-650 and policy_transport.rs:130-171.
\* The supervised peer loads one staged identity and negotiates one epoch.
LaunchAndNegotiateHagia(loaded, epoch) ==
    /\ phase = "preparing"
    /\ hagiaPhase = "absent"
    /\ loaded \in ProfileIdentities
    /\ epoch \in Epochs
    /\ hagiaLoaded' = loaded
    /\ connectionEpoch' = epoch
    /\ hagiaPhase' = "loaded"
    /\ UNCHANGED <<phase, candidate, activeProfile, latestGeneration,
                    policyProxyPrepared, localPrepared, localActivated,
                    rollbackPending, outstanding, commands, acks,
                    hagiaPreparedAccepted, hagiaActiveAccepted,
                    hagiaRollbackAccepted, preparedEpoch, activeEpoch,
                    rejectedProfiles, promotedProfiles, graphicsOpen,
                    configurationQueued, configurationAccepted,
                    configurationIdentity>>

\* Modeling brief scenario 4; this new send must precede today's
\* policy_transport_worker.rs:141-156 configuration receive.
SendProfilePrepare(transaction) ==
    /\ phase = "preparing"
    /\ hagiaPhase = "loaded"
    /\ transaction \in Transactions
    /\ outstanding = NoOutstanding
    /\ LET message == Command("prepare", connectionEpoch, transaction, candidate)
       IN /\ commands' = commands \cup {message}
          /\ outstanding' = message
    /\ hagiaPhase' = "preparePending"
    /\ UNCHANGED <<phase, candidate, activeProfile, latestGeneration,
                    policyProxyPrepared, localPrepared, localActivated,
                    rollbackPending, hagiaLoaded, connectionEpoch, acks,
                    hagiaPreparedAccepted, hagiaActiveAccepted,
                    hagiaRollbackAccepted, preparedEpoch, activeEpoch,
                    rejectedProfiles, promotedProfiles, graphicsOpen,
                    configurationQueued, configurationAccepted,
                    configurationIdentity>>

\* hagia.nim:47-70. The future decoder compares the request with the already
\* validated staged candidate before acknowledging it.
HagiaHandleProfilePrepare(message) ==
    /\ message \in commands
    /\ message.kind = "prepare"
    /\ message.epoch = connectionEpoch
    /\ hagiaPhase = "preparePending"
    /\ commands' = commands \ {message}
    /\ LET outcome == IF message.identity = hagiaLoaded
                      THEN "accepted" ELSE "rejected"
           reply == Ack("prepared", message.epoch, message.transaction,
                        message.identity, outcome)
       IN /\ acks' = acks \cup {reply}
          /\ hagiaPhase' = IF outcome = "accepted" THEN "prepared" ELSE "loaded"
    /\ UNCHANGED <<phase, candidate, activeProfile, latestGeneration,
                    policyProxyPrepared, localPrepared, localActivated,
                    rollbackPending, hagiaLoaded, connectionEpoch, outstanding,
                    hagiaPreparedAccepted, hagiaActiveAccepted,
                    hagiaRollbackAccepted, preparedEpoch, activeEpoch,
                    rejectedProfiles, promotedProfiles, graphicsOpen,
                    configurationQueued, configurationAccepted,
                    configurationIdentity>>

ExactAck(reply, kind) ==
    /\ outstanding # NoOutstanding
    /\ reply.kind = kind
    /\ reply.epoch = outstanding.epoch
    /\ reply.transaction = outstanding.transaction
    /\ reply.identity = outstanding.identity
    /\ reply.epoch = connectionEpoch
    /\ reply.identity = candidate

\* policy_transport.rs:236-247 provides the existing epoch/transaction
\* admission shape; this completion additionally checks the profile identity.
SophiaAcceptProfilePrepared(reply) ==
    /\ phase = "preparing"
    /\ reply \in acks
    /\ ExactAck(reply, "prepared")
    /\ reply.outcome = "accepted"
    /\ localPrepared = LocalAuthorities
    /\ acks' = acks \ {reply}
    /\ outstanding' = NoOutstanding
    /\ hagiaPreparedAccepted' = TRUE
    /\ preparedEpoch' = connectionEpoch
    /\ phase' = "prepared"
    /\ UNCHANGED <<candidate, activeProfile, latestGeneration,
                    policyProxyPrepared, localPrepared, localActivated,
                    rollbackPending, hagiaPhase, hagiaLoaded, connectionEpoch,
                    commands, hagiaActiveAccepted, hagiaRollbackAccepted,
                    activeEpoch, rejectedProfiles, promotedProfiles,
                    graphicsOpen, configurationQueued, configurationAccepted,
                    configurationIdentity>>

\* coordinator.nim:130-132,154-170. Any exact rejection starts full rollback.
SophiaRejectProfileCompletion(reply) ==
    /\ phase \in {"preparing", "activating"}
    /\ reply \in acks
    /\ \/ ExactAck(reply, "prepared")
       \/ ExactAck(reply, "active")
    /\ reply.outcome = "rejected"
    /\ acks' = acks \ {reply}
    /\ outstanding' = NoOutstanding
    /\ rollbackPending' = LocalAuthorities
    /\ rejectedProfiles' = rejectedProfiles \cup {candidate}
    /\ phase' = "rollingBack"
    /\ preparedEpoch' = NoEpoch
    /\ activeEpoch' = NoEpoch
    /\ UNCHANGED <<candidate, activeProfile, latestGeneration,
                    policyProxyPrepared, localPrepared, localActivated,
                    hagiaPhase, hagiaLoaded, connectionEpoch, commands,
                    hagiaPreparedAccepted, hagiaActiveAccepted,
                    hagiaRollbackAccepted, promotedProfiles, graphicsOpen,
                    configurationQueued, configurationAccepted,
                    configurationIdentity>>

\* coordinator.nim:136-145. Activation starts only after every real owner
\* prepared; the policy proxy alone is not sufficient.
BeginProfileActivation ==
    /\ phase = "prepared"
    /\ policyProxyPrepared
    /\ localPrepared = LocalAuthorities
    /\ hagiaPreparedAccepted
    /\ hagiaPhase = "prepared"
    /\ phase' = "activating"
    /\ UNCHANGED <<candidate, activeProfile, latestGeneration,
                    policyProxyPrepared, localPrepared, localActivated,
                    rollbackPending, hagiaPhase, hagiaLoaded, connectionEpoch,
                    outstanding, commands, acks, hagiaPreparedAccepted,
                    hagiaActiveAccepted, hagiaRollbackAccepted, preparedEpoch,
                    activeEpoch, rejectedProfiles, promotedProfiles,
                    graphicsOpen, configurationQueued, configurationAccepted,
                    configurationIdentity>>

\* coordinator.nim:146-161. Local owner activation remains sequential and
\* invisible because live_session.rs:220 has not opened graphics.
ActivateLocalAuthority(authority) ==
    /\ phase = "activating"
    /\ authority \in LocalAuthorities \ localActivated
    /\ localActivated' = localActivated \cup {authority}
    /\ UNCHANGED <<phase, candidate, activeProfile, latestGeneration,
                    policyProxyPrepared, localPrepared, rollbackPending,
                    hagiaPhase, hagiaLoaded, connectionEpoch, outstanding,
                    commands, acks, hagiaPreparedAccepted, hagiaActiveAccepted,
                    hagiaRollbackAccepted, preparedEpoch, activeEpoch,
                    rejectedProfiles, promotedProfiles, graphicsOpen,
                    configurationQueued, configurationAccepted,
                    configurationIdentity>>

\* Modeling brief scenario 5. Hagia activation is deliberately the final
\* authority effect, after every Sophia-owned authority is locally active.
SendProfileActivate(transaction) ==
    /\ phase = "activating"
    /\ localActivated = LocalAuthorities
    /\ hagiaPhase = "prepared"
    /\ transaction \in Transactions
    /\ outstanding = NoOutstanding
    /\ LET message == Command("activate", connectionEpoch, transaction, candidate)
       IN /\ commands' = commands \cup {message}
          /\ outstanding' = message
    /\ hagiaPhase' = "activatePending"
    /\ UNCHANGED <<phase, candidate, activeProfile, latestGeneration,
                    policyProxyPrepared, localPrepared, localActivated,
                    rollbackPending, hagiaLoaded, connectionEpoch, acks,
                    hagiaPreparedAccepted, hagiaActiveAccepted,
                    hagiaRollbackAccepted, preparedEpoch, activeEpoch,
                    rejectedProfiles, promotedProfiles, graphicsOpen,
                    configurationQueued, configurationAccepted,
                    configurationIdentity>>

\* Future Hagia policy_client startup branch. An exact retained prepare is
\* required; fault injection may still return a typed rejection.
HagiaHandleProfileActivate(message, outcome) ==
    /\ message \in commands
    /\ message.kind = "activate"
    /\ message.epoch = connectionEpoch
    /\ message.identity = hagiaLoaded
    /\ hagiaPhase = "activatePending"
    /\ outcome \in Outcomes
    /\ commands' = commands \ {message}
    /\ LET reply == Ack("active", message.epoch, message.transaction,
                        message.identity, outcome)
       IN acks' = acks \cup {reply}
    /\ hagiaPhase' = IF outcome = "accepted" THEN "active" ELSE "prepared"
    /\ UNCHANGED <<phase, candidate, activeProfile, latestGeneration,
                    policyProxyPrepared, localPrepared, localActivated,
                    rollbackPending, hagiaLoaded, connectionEpoch, outstanding,
                    hagiaPreparedAccepted, hagiaActiveAccepted,
                    hagiaRollbackAccepted, preparedEpoch, activeEpoch,
                    rejectedProfiles, promotedProfiles, graphicsOpen,
                    configurationQueued, configurationAccepted,
                    configurationIdentity>>

\* coordinator.nim:146-161 and live_session.rs:207-220. Only the final exact
\* external acknowledgement promotes and releases the graphical launch gate.
SophiaAcceptProfileActive(reply) ==
    /\ phase = "activating"
    /\ reply \in acks
    /\ ExactAck(reply, "active")
    /\ reply.outcome = "accepted"
    /\ localActivated = LocalAuthorities
    /\ acks' = acks \ {reply}
    /\ outstanding' = NoOutstanding
    /\ hagiaActiveAccepted' = TRUE
    /\ activeEpoch' = connectionEpoch
    /\ activeProfile' = candidate
    /\ promotedProfiles' = promotedProfiles \cup {candidate}
    /\ graphicsOpen' = TRUE
    /\ phase' = "active"
    /\ UNCHANGED <<candidate, latestGeneration, policyProxyPrepared,
                    localPrepared, localActivated, rollbackPending, hagiaPhase,
                    hagiaLoaded, connectionEpoch, commands,
                    hagiaPreparedAccepted, hagiaRollbackAccepted, preparedEpoch,
                    rejectedProfiles, configurationQueued,
                    configurationAccepted, configurationIdentity>>

\* policy_client.nim:638-645. FIFO permits Hagia to queue configuration after
\* emitting ProfileActive; Sophia still accepts it only after global promotion.
HagiaQueuePolicyConfiguration(generation) ==
    /\ hagiaPhase = "active"
    /\ generation \in ConfigGenerations
    /\ ~configurationQueued
    /\ configurationQueued' = TRUE
    /\ configurationIdentity' = ConfigurationIdentity(generation)
    /\ UNCHANGED <<phase, candidate, activeProfile, latestGeneration,
                    policyProxyPrepared, localPrepared, localActivated,
                    rollbackPending, hagiaPhase, hagiaLoaded, connectionEpoch,
                    outstanding, commands, acks, hagiaPreparedAccepted,
                    hagiaActiveAccepted, hagiaRollbackAccepted, preparedEpoch,
                    activeEpoch, rejectedProfiles, promotedProfiles,
                    graphicsOpen, configurationAccepted>>

\* policy_transport_worker.rs:141-156. The desired worker phase rejects message
\* kind 42 until the profile activation acknowledgement has settled.
SophiaAcceptPolicyConfiguration ==
    /\ phase = "active"
    /\ graphicsOpen
    /\ hagiaActiveAccepted
    /\ configurationQueued
    /\ configurationAccepted' = TRUE
    /\ configurationQueued' = FALSE
    /\ UNCHANGED <<phase, candidate, activeProfile, latestGeneration,
                    policyProxyPrepared, localPrepared, localActivated,
                    rollbackPending, hagiaPhase, hagiaLoaded, connectionEpoch,
                    outstanding, commands, acks, hagiaPreparedAccepted,
                    hagiaActiveAccepted, hagiaRollbackAccepted, preparedEpoch,
                    activeEpoch, rejectedProfiles, promotedProfiles,
                    graphicsOpen, configurationIdentity>>

\* policy_transport_worker.rs:83-87 and public_policy.rs:676-680. Before global
\* promotion a disconnect is terminal for startup and advances the epoch so any
\* already queued acknowledgement is stale.
DisconnectHagia(nextEpoch) ==
    /\ phase \in {"preparing", "prepared", "activating"}
    /\ hagiaPhase \notin {"absent", "disconnected"}
    /\ nextEpoch \in Epochs
    /\ nextEpoch > connectionEpoch
    /\ connectionEpoch' = nextEpoch
    /\ hagiaPhase' = "disconnected"
    /\ outstanding' = NoOutstanding
    /\ commands' = {}
    /\ rollbackPending' = LocalAuthorities
    /\ rejectedProfiles' = rejectedProfiles \cup {candidate}
    /\ preparedEpoch' = NoEpoch
    /\ activeEpoch' = NoEpoch
    /\ phase' = "rollingBack"
    /\ UNCHANGED <<candidate, activeProfile, latestGeneration,
                    policyProxyPrepared, localPrepared, localActivated,
                    hagiaLoaded, acks, hagiaPreparedAccepted,
                    hagiaActiveAccepted, hagiaRollbackAccepted,
                    promotedProfiles, graphicsOpen, configurationQueued,
                    configurationAccepted, configurationIdentity>>

\* Modeling brief scenario 5. A bounded wait failure has the same fail-closed
\* coordinator result as disconnect but does not invent a new connection epoch.
TimeoutProfileOperation ==
    /\ phase \in {"preparing", "activating"}
    /\ outstanding # NoOutstanding
    /\ outstanding' = NoOutstanding
    /\ commands' = {}
    /\ rollbackPending' = LocalAuthorities
    /\ rejectedProfiles' = rejectedProfiles \cup {candidate}
    /\ preparedEpoch' = NoEpoch
    /\ activeEpoch' = NoEpoch
    /\ phase' = "rollingBack"
    /\ UNCHANGED <<candidate, activeProfile, latestGeneration,
                    policyProxyPrepared, localPrepared, localActivated,
                    hagiaPhase, hagiaLoaded, connectionEpoch, acks,
                    hagiaPreparedAccepted, hagiaActiveAccepted,
                    hagiaRollbackAccepted, promotedProfiles, graphicsOpen,
                    configurationQueued, configurationAccepted,
                    configurationIdentity>>

\* coordinator.nim:94-102,162-170. Local rollback is idempotent and covers
\* every owner even if it had not completed prepare or activation.
RollbackLocalAuthority(authority) ==
    /\ phase = "rollingBack"
    /\ authority \in rollbackPending
    /\ rollbackPending' = rollbackPending \ {authority}
    /\ localPrepared' = localPrepared \ {authority}
    /\ localActivated' = localActivated \ {authority}
    /\ UNCHANGED <<phase, candidate, activeProfile, latestGeneration,
                    policyProxyPrepared, hagiaPhase, hagiaLoaded,
                    connectionEpoch, outstanding, commands, acks,
                    hagiaPreparedAccepted, hagiaActiveAccepted,
                    hagiaRollbackAccepted, preparedEpoch, activeEpoch,
                    rejectedProfiles, promotedProfiles, graphicsOpen,
                    configurationQueued, configurationAccepted,
                    configurationIdentity>>

\* Future rollback wire path. Connected Hagia discards the exact retained
\* candidate; disconnected startup needs no acknowledgement from a dead peer.
SendProfileRollback(transaction) ==
    /\ phase = "rollingBack"
    /\ hagiaPhase \in {"loaded", "prepared", "active"}
    /\ transaction \in Transactions
    /\ outstanding = NoOutstanding
    /\ LET message == Command("rollback", connectionEpoch, transaction, candidate)
       IN /\ commands' = commands \cup {message}
          /\ outstanding' = message
    /\ hagiaPhase' = "rollbackPending"
    /\ UNCHANGED <<phase, candidate, activeProfile, latestGeneration,
                    policyProxyPrepared, localPrepared, localActivated,
                    rollbackPending, hagiaLoaded, connectionEpoch, acks,
                    hagiaPreparedAccepted, hagiaActiveAccepted,
                    hagiaRollbackAccepted, preparedEpoch, activeEpoch,
                    rejectedProfiles, promotedProfiles, graphicsOpen,
                    configurationQueued, configurationAccepted,
                    configurationIdentity>>

\* Future Hagia rollback decoder. Exact identity is required before the local
\* retained candidate is discarded.
HagiaHandleProfileRollback(message) ==
    /\ message \in commands
    /\ message.kind = "rollback"
    /\ message.epoch = connectionEpoch
    /\ message.identity = hagiaLoaded
    /\ hagiaPhase = "rollbackPending"
    /\ commands' = commands \ {message}
    /\ LET reply == Ack("rolledBack", message.epoch, message.transaction,
                        message.identity, "accepted")
       IN acks' = acks \cup {reply}
    /\ hagiaPhase' = "rolledBack"
    /\ UNCHANGED <<phase, candidate, activeProfile, latestGeneration,
                    policyProxyPrepared, localPrepared, localActivated,
                    rollbackPending, hagiaLoaded, connectionEpoch, outstanding,
                    hagiaPreparedAccepted, hagiaActiveAccepted,
                    hagiaRollbackAccepted, preparedEpoch, activeEpoch,
                    rejectedProfiles, promotedProfiles, graphicsOpen,
                    configurationQueued, configurationAccepted,
                    configurationIdentity>>

\* Future coordinator completion path; exact epoch, transaction, and identity
\* admission is shared with prepare/activate.
SophiaAcceptProfileRolledBack(reply) ==
    /\ phase = "rollingBack"
    /\ reply \in acks
    /\ ExactAck(reply, "rolledBack")
    /\ reply.outcome = "accepted"
    /\ acks' = acks \ {reply}
    /\ outstanding' = NoOutstanding
    /\ hagiaRollbackAccepted' = TRUE
    /\ UNCHANGED <<phase, candidate, activeProfile, latestGeneration,
                    policyProxyPrepared, localPrepared, localActivated,
                    rollbackPending, hagiaPhase, hagiaLoaded, connectionEpoch,
                    commands, hagiaPreparedAccepted, hagiaActiveAccepted,
                    preparedEpoch, activeEpoch, rejectedProfiles,
                    promotedProfiles, graphicsOpen, configurationQueued,
                    configurationAccepted, configurationIdentity>>

\* policy_transport.rs:210-247. Any nonmatching completion is consumed as a
\* typed no-op and cannot settle the current outstanding operation.
DiscardStaleAck(reply) ==
    /\ reply \in acks
    /\ \/ outstanding = NoOutstanding
       \/ reply.epoch # connectionEpoch
       \/ reply.transaction # outstanding.transaction
       \/ reply.identity # outstanding.identity
       \/ reply.kind \notin AckKinds
    /\ acks' = acks \ {reply}
    /\ UNCHANGED <<phase, candidate, activeProfile, latestGeneration,
                    policyProxyPrepared, localPrepared, localActivated,
                    rollbackPending, hagiaPhase, hagiaLoaded, connectionEpoch,
                    outstanding, commands, hagiaPreparedAccepted,
                    hagiaActiveAccepted, hagiaRollbackAccepted, preparedEpoch,
                    activeEpoch, rejectedProfiles, promotedProfiles,
                    graphicsOpen, configurationQueued, configurationAccepted,
                    configurationIdentity>>

\* coordinator.nim:162-170. A failed startup settles only after all reachable
\* participants have discarded the candidate; graphics remain closed.
CompleteFailedStartup ==
    /\ phase = "rollingBack"
    /\ rollbackPending = {}
    /\ \/ hagiaPhase = "disconnected"
       \/ /\ hagiaPhase = "rolledBack"
          /\ hagiaRollbackAccepted
    /\ phase' = "failed"
    /\ policyProxyPrepared' = FALSE
    /\ localPrepared' = {}
    /\ localActivated' = {}
    /\ UNCHANGED <<candidate, activeProfile, latestGeneration,
                    rollbackPending, hagiaPhase, hagiaLoaded, connectionEpoch,
                    outstanding, commands, acks, hagiaPreparedAccepted,
                    hagiaActiveAccepted, hagiaRollbackAccepted, preparedEpoch,
                    activeEpoch, rejectedProfiles, promotedProfiles,
                    graphicsOpen, configurationQueued, configurationAccepted,
                    configurationIdentity>>

Next ==
    \/ \E g \in Generations, d \in Digests : StageDesktopProfile(g, d)
    \/ \E authority \in LocalAuthorities : PrepareLocalAuthority(authority)
    \/ \E loaded \in ProfileIdentities, epoch \in Epochs :
           LaunchAndNegotiateHagia(loaded, epoch)
    \/ \E transaction \in Transactions : SendProfilePrepare(transaction)
    \/ \E message \in commands : HagiaHandleProfilePrepare(message)
    \/ \E reply \in acks : SophiaAcceptProfilePrepared(reply)
    \/ \E reply \in acks : SophiaRejectProfileCompletion(reply)
    \/ BeginProfileActivation
    \/ \E authority \in LocalAuthorities : ActivateLocalAuthority(authority)
    \/ \E transaction \in Transactions : SendProfileActivate(transaction)
    \/ \E message \in commands, outcome \in Outcomes :
           HagiaHandleProfileActivate(message, outcome)
    \/ \E reply \in acks : SophiaAcceptProfileActive(reply)
    \/ \E generation \in ConfigGenerations :
           HagiaQueuePolicyConfiguration(generation)
    \/ SophiaAcceptPolicyConfiguration
    \/ \E epoch \in Epochs : DisconnectHagia(epoch)
    \/ TimeoutProfileOperation
    \/ \E authority \in LocalAuthorities : RollbackLocalAuthority(authority)
    \/ \E transaction \in Transactions : SendProfileRollback(transaction)
    \/ \E message \in commands : HagiaHandleProfileRollback(message)
    \/ \E reply \in acks : SophiaAcceptProfileRolledBack(reply)
    \/ \E reply \in acks : DiscardStaleAck(reply)
    \/ CompleteFailedStartup

TypeOK ==
    /\ phase \in Phases
    /\ candidate \in ProfileIdentities \cup {NoProfile}
    /\ activeProfile \in ProfileIdentities \cup {NoProfile}
    /\ latestGeneration \in Generations \cup {NoGeneration}
    /\ policyProxyPrepared \in BOOLEAN
    /\ localPrepared \subseteq LocalAuthorities
    /\ localActivated \subseteq LocalAuthorities
    /\ rollbackPending \subseteq LocalAuthorities
    /\ hagiaPhase \in HagiaPhases
    /\ hagiaLoaded \in ProfileIdentities \cup {NoProfile}
    /\ connectionEpoch \in Epochs \cup {NoEpoch}
    /\ outstanding \in Commands \cup {NoOutstanding}
    /\ commands \subseteq Commands
    /\ acks \subseteq Acks
    /\ hagiaPreparedAccepted \in BOOLEAN
    /\ hagiaActiveAccepted \in BOOLEAN
    /\ hagiaRollbackAccepted \in BOOLEAN
    /\ preparedEpoch \in Epochs \cup {NoEpoch}
    /\ activeEpoch \in Epochs \cup {NoEpoch}
    /\ rejectedProfiles \subseteq ProfileIdentities
    /\ promotedProfiles \subseteq ProfileIdentities
    /\ graphicsOpen \in BOOLEAN
    /\ configurationQueued \in BOOLEAN
    /\ configurationAccepted \in BOOLEAN
    /\ configurationIdentity \in
         {ConfigurationIdentity(g) : g \in ConfigGenerations \cup {NoGeneration}}

GraphicsRequireGlobalActivation ==
    graphicsOpen =>
        /\ phase = "active"
        /\ activeProfile = candidate
        /\ activeProfile \in promotedProfiles
        /\ localActivated = LocalAuthorities
        /\ hagiaActiveAccepted
        /\ activeEpoch = connectionEpoch

HagiaAckMatchesLoadedCandidate ==
    \A reply \in acks :
        reply.outcome = "accepted" => reply.identity = hagiaLoaded

CompletionMatchesConnection ==
    /\ preparedEpoch = NoEpoch \/ preparedEpoch = connectionEpoch
    /\ activeEpoch = NoEpoch \/ activeEpoch = connectionEpoch

ConfigurationFollowsProfileActivation ==
    configurationAccepted =>
        /\ phase = "active"
        /\ graphicsOpen
        /\ hagiaActiveAccepted

FailureCannotPromote ==
    phase \in {"rollingBack", "failed"} =>
        /\ activeProfile = NoProfile
        /\ ~graphicsOpen

NoPartialVisibility ==
    graphicsOpen => localActivated = LocalAuthorities

IdentityNamespacesAreDistinct ==
    configurationIdentity.namespace # candidate.namespace

RejectedNeverPromoted ==
    rejectedProfiles \cap promotedProfiles = {}

PreparedRequiresExactExternalAck ==
    phase \in {"prepared", "activating", "active"} =>
        /\ hagiaPreparedAccepted
        /\ preparedEpoch = connectionEpoch

Spec == Init /\ [][Next]_vars

=============================================================================
