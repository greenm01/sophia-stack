---------------- MODULE MC ----------------
EXTENDS base

CONSTANTS ReplacementLimit, MismatchLimit, DisconnectLimit, TimeoutLimit,
          DelayedAckLimit, PrematureConfigurationLimit, ConfigurationLimit

VARIABLE faultCounts

faultVars == <<faultCounts>>
mcVars == <<vars, faultCounts>>

MCInit ==
    /\ Init
    /\ faultCounts =
         [replacement |-> 0,
          mismatch |-> 0,
          disconnect |-> 0,
          timeout |-> 0,
          delayedAck |-> 0,
          prematureConfiguration |-> 0,
          configuration |-> 0]

\* base.tla:BeginReplacement; public_policy.rs:1066-1106. Process creation is
\* externally triggered and bounded, while its later protocol steps are reactive.
MCBeginExactReplacement(epoch) ==
    /\ faultCounts.replacement < ReplacementLimit
    /\ BeginReplacement(activeProfile, epoch)
    /\ faultCounts' = [faultCounts EXCEPT !.replacement = @ + 1]

\* Modeling brief scenario 3. A retained-file/wire mismatch is independently
\* bounded so its focused hunt excludes unrelated failure mechanisms.
MCBeginMismatchedReplacement(loaded, epoch) ==
    /\ faultCounts.replacement < ReplacementLimit
    /\ faultCounts.mismatch < MismatchLimit
    /\ loaded # activeProfile
    /\ BeginReplacement(loaded, epoch)
    /\ faultCounts' =
         [faultCounts EXCEPT
             !.replacement = @ + 1,
             !.mismatch = @ + 1]

\* public_policy.rs:1015-1064. Worker/process failure is a bounded input.
MCDisconnectHagia ==
    /\ faultCounts.disconnect < DisconnectLimit
    /\ DisconnectHagia
    /\ faultCounts' = [faultCounts EXCEPT !.disconnect = @ + 1]

\* policy_transport_worker.rs:128-143. A bounded receive timeout is an input;
\* cleanup and a later replacement start remain reactive.
MCTimeoutProfileOperation ==
    /\ faultCounts.timeout < TimeoutLimit
    /\ TimeoutProfileOperation
    /\ faultCounts' = [faultCounts EXCEPT !.timeout = @ + 1]

\* Modeling brief scenario 2. A delayed prior-incarnation completion is the
\* bounded message-delay mechanism under test.
MCInjectDelayedAcknowledgement(kind, epoch, transaction) ==
    /\ faultCounts.delayedAck < DelayedAckLimit
    /\ InjectDelayedAcknowledgement(kind, epoch, transaction)
    /\ faultCounts' = [faultCounts EXCEPT !.delayedAck = @ + 1]

\* Modeling brief scenario 1. Premature normal traffic is bounded separately.
MCSendPrematureConfiguration ==
    /\ faultCounts.prematureConfiguration < PrematureConfigurationLimit
    /\ coordinatorPhase # "active"
    /\ HagiaSendPolicyConfiguration
    /\ faultCounts' =
         [faultCounts EXCEPT !.prematureConfiguration = @ + 1]

\* The normal post-active configuration send is also an external input.
MCSendProvenConfiguration ==
    /\ faultCounts.configuration < ConfigurationLimit
    /\ coordinatorPhase = "active"
    /\ HagiaSendPolicyConfiguration
    /\ faultCounts' = [faultCounts EXCEPT !.configuration = @ + 1]

MCNext ==
    \/ MCDisconnectHagia
    \/ \E epoch \in Epochs : MCBeginExactReplacement(epoch)
    \/ \E loaded \in ProfileIdentities, epoch \in Epochs :
           MCBeginMismatchedReplacement(loaded, epoch)
    \/ \E transaction \in Transactions :
           /\ SendProfilePrepare(transaction)
           /\ UNCHANGED faultCounts
    \/ \E message \in commands :
           /\ HagiaHandleProfilePrepare(message)
           /\ UNCHANGED faultCounts
    \/ \E reply \in acks :
           /\ SophiaSettleProfilePrepared(reply)
           /\ UNCHANGED faultCounts
    \/ \E transaction \in Transactions :
           /\ SendProfileActivate(transaction)
           /\ UNCHANGED faultCounts
    \/ \E message \in commands :
           /\ HagiaHandleProfileActivate(message)
           /\ UNCHANGED faultCounts
    \/ \E reply \in acks :
           /\ SophiaSettleProfileActive(reply)
           /\ UNCHANGED faultCounts
    \/ MCSendPrematureConfiguration
    \/ MCSendProvenConfiguration
    \/ \E message \in configurationMessages :
           /\ SophiaAcceptPolicyConfiguration(message)
           /\ UNCHANGED faultCounts
    \/ \E message \in configurationMessages :
           /\ SophiaRejectPrematureConfiguration(message)
           /\ UNCHANGED faultCounts
    \/ \E kind \in AckKinds, epoch \in Epochs, transaction \in Transactions :
           MCInjectDelayedAcknowledgement(kind, epoch, transaction)
    \/ \E reply \in acks :
           /\ DiscardStaleAcknowledgement(reply)
           /\ UNCHANGED faultCounts
    \/ MCTimeoutProfileOperation
    \/ /\ DegradeFailedReattachment
       /\ UNCHANGED faultCounts

MCTypeOK ==
    /\ TypeOK
    /\ faultCounts \in
         [replacement : 0..ReplacementLimit,
          mismatch : 0..MismatchLimit,
          disconnect : 0..DisconnectLimit,
          timeout : 0..TimeoutLimit,
          delayedAck : 0..DelayedAckLimit,
          prematureConfiguration : 0..PrematureConfigurationLimit,
          configuration : 0..ConfigurationLimit]

MCView == vars

MCSpec == MCInit /\ [][MCNext]_mcVars

=============================================================================
