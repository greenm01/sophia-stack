---------------- MODULE MC ----------------
EXTENDS base

CONSTANTS StageLimit, MismatchLimit, RejectLimit, DisconnectLimit, TimeoutLimit

VARIABLE faultCounts

faultVars == <<faultCounts>>
mcVars == <<vars, faultCounts>>

MCInit ==
    /\ Init
    /\ faultCounts =
         [stage |-> 0,
          mismatch |-> 0,
          reject |-> 0,
          disconnect |-> 0,
          timeout |-> 0]

\* base.tla:StageDesktopProfile; profile admission is the bounded external
\* startup request that begins one immutable generation.
MCStageDesktopProfile(g, d) ==
    /\ faultCounts.stage < StageLimit
    /\ StageDesktopProfile(g, d)
    /\ faultCounts' = [faultCounts EXCEPT !.stage = @ + 1]

\* base.tla:LaunchAndNegotiateHagia; exact staged-file loading is the normal
\* path and remains reactive once startup has begun.
MCLaunchExactHagia(epoch) ==
    /\ candidate \in ProfileIdentities
    /\ LaunchAndNegotiateHagia(candidate, epoch)
    /\ UNCHANGED faultCounts

\* Modeling brief scenario 3; a divergent staged-file identity is bounded
\* independently so the identity hunt can enable it without unrelated faults.
MCLaunchMismatchedHagia(loaded, epoch) ==
    /\ faultCounts.mismatch < MismatchLimit
    /\ loaded # candidate
    /\ LaunchAndNegotiateHagia(loaded, epoch)
    /\ faultCounts' = [faultCounts EXCEPT !.mismatch = @ + 1]

\* base.tla:HagiaHandleProfileActivate; accepted activation is reactive.
MCHagiaAcceptProfileActivate(message) ==
    /\ HagiaHandleProfileActivate(message, "accepted")
    /\ UNCHANGED faultCounts

\* Modeling brief scenario 5; typed activation rejection is a bounded fault.
MCHagiaRejectProfileActivate(message) ==
    /\ faultCounts.reject < RejectLimit
    /\ HagiaHandleProfileActivate(message, "rejected")
    /\ faultCounts' = [faultCounts EXCEPT !.reject = @ + 1]

\* policy_transport_worker.rs:83-87; disconnect is bounded while cleanup is not.
MCDisconnectHagia(epoch) ==
    /\ faultCounts.disconnect < DisconnectLimit
    /\ DisconnectHagia(epoch)
    /\ faultCounts' = [faultCounts EXCEPT !.disconnect = @ + 1]

\* Modeling brief scenario 5; only initiation of the wait failure is bounded.
MCTimeoutProfileOperation ==
    /\ faultCounts.timeout < TimeoutLimit
    /\ TimeoutProfileOperation
    /\ faultCounts' = [faultCounts EXCEPT !.timeout = @ + 1]

MCNext ==
    \/ \E g \in Generations, d \in Digests : MCStageDesktopProfile(g, d)
    \/ \E authority \in LocalAuthorities :
           /\ PrepareLocalAuthority(authority)
           /\ UNCHANGED faultCounts
    \/ \E epoch \in Epochs : MCLaunchExactHagia(epoch)
    \/ \E loaded \in ProfileIdentities, epoch \in Epochs :
           MCLaunchMismatchedHagia(loaded, epoch)
    \/ \E transaction \in Transactions :
           /\ SendProfilePrepare(transaction)
           /\ UNCHANGED faultCounts
    \/ \E message \in commands :
           /\ HagiaHandleProfilePrepare(message)
           /\ UNCHANGED faultCounts
    \/ \E reply \in acks :
           /\ SophiaAcceptProfilePrepared(reply)
           /\ UNCHANGED faultCounts
    \/ \E reply \in acks :
           /\ SophiaRejectProfileCompletion(reply)
           /\ UNCHANGED faultCounts
    \/ /\ BeginProfileActivation
       /\ UNCHANGED faultCounts
    \/ \E authority \in LocalAuthorities :
           /\ ActivateLocalAuthority(authority)
           /\ UNCHANGED faultCounts
    \/ \E transaction \in Transactions :
           /\ SendProfileActivate(transaction)
           /\ UNCHANGED faultCounts
    \/ \E message \in commands : MCHagiaAcceptProfileActivate(message)
    \/ \E message \in commands : MCHagiaRejectProfileActivate(message)
    \/ \E reply \in acks :
           /\ SophiaAcceptProfileActive(reply)
           /\ UNCHANGED faultCounts
    \/ \E generation \in ConfigGenerations :
           /\ HagiaQueuePolicyConfiguration(generation)
           /\ UNCHANGED faultCounts
    \/ /\ SophiaAcceptPolicyConfiguration
       /\ UNCHANGED faultCounts
    \/ \E epoch \in Epochs : MCDisconnectHagia(epoch)
    \/ MCTimeoutProfileOperation
    \/ \E authority \in LocalAuthorities :
           /\ RollbackLocalAuthority(authority)
           /\ UNCHANGED faultCounts
    \/ \E transaction \in Transactions :
           /\ SendProfileRollback(transaction)
           /\ UNCHANGED faultCounts
    \/ \E message \in commands :
           /\ HagiaHandleProfileRollback(message)
           /\ UNCHANGED faultCounts
    \/ \E reply \in acks :
           /\ SophiaAcceptProfileRolledBack(reply)
           /\ UNCHANGED faultCounts
    \/ \E reply \in acks :
           /\ DiscardStaleAck(reply)
           /\ UNCHANGED faultCounts
    \/ /\ CompleteFailedStartup
       /\ UNCHANGED faultCounts

MCTypeOK ==
    /\ TypeOK
    /\ faultCounts \in
         [stage : 0..StageLimit,
          mismatch : 0..MismatchLimit,
          reject : 0..RejectLimit,
          disconnect : 0..DisconnectLimit,
          timeout : 0..TimeoutLimit]

\* Modeling brief scenario 6. This startup-only model has no transition that
\* changes Hagia epoch or authority state after graphical promotion.
PostPromotionRestartExcluded ==
    phase = "active" =>
        /\ connectionEpoch = activeEpoch
        /\ hagiaPhase = "active"

MCView == vars

MCSpec == MCInit /\ [][MCNext]_mcVars

=============================================================================
