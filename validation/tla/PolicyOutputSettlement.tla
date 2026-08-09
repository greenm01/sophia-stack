--------------------- MODULE PolicyOutputSettlement ---------------------
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************
 * Output topology changes during a staged multi-output policy settlement. *
 * Complete affected-output proposals are one logical candidate. Output     *
 * loss or return advances the canonical scene before revalidation, so a    *
 * candidate based on the older topology receives one stale terminal result *
 * and cannot partially replace either last-good projection.                *
 *************************************************************************)

CONSTANTS MaxSceneGeneration, MaxSettlements

ASSUME /\ MaxSceneGeneration \in Nat
       /\ MaxSceneGeneration >= 3
       /\ MaxSettlements \in (Nat \ {0})

Outputs == {"left", "right"}

VARIABLES phase, sceneGeneration, outputLive, outputGeneration,
          requestBase, stagedBase, candidateState,
          reducerState, layoutState, settlementLog, nextSettlement,
          rightEverLost

vars == <<phase, sceneGeneration, outputLive, outputGeneration,
          requestBase, stagedBase, candidateState,
          reducerState, layoutState, settlementLog, nextSettlement,
          rightEverLost>>

LiveOutputs == {output \in Outputs : outputLive[output]}

CandidateCurrent ==
    /\ stagedBase = sceneGeneration
    /\ LiveOutputs = Outputs

Init ==
    /\ phase = "idle"
    /\ sceneGeneration = 1
    /\ outputLive = [output \in Outputs |-> TRUE]
    /\ outputGeneration = [output \in Outputs |-> 1]
    /\ requestBase = 0
    /\ stagedBase = 0
    /\ candidateState = 0
    /\ reducerState = [output \in Outputs |-> 0]
    /\ layoutState = [output \in Outputs |-> 0]
    /\ settlementLog = <<>>
    /\ nextSettlement = 1
    /\ rightEverLost = FALSE

\* PolicyProjectionReducer::issue_request_with_cause
\* crates/sophia-engine/src/policy_projection.rs:160-201.
IssueCrossOutputRequest ==
    /\ phase = "idle"
    /\ LiveOutputs = Outputs
    /\ nextSettlement <= MaxSettlements
    /\ requestBase' = sceneGeneration
    /\ phase' = "requested"
    /\ UNCHANGED <<sceneGeneration, outputLive, outputGeneration,
                    stagedBase, candidateState, reducerState, layoutState,
                    settlementLog, nextSettlement, rightEverLost>>

\* PolicyProjectionReducer::stage_proposal validates into a clone
\* (crates/sophia-engine/src/policy_projection.rs:246-264).
StageCompleteProposal ==
    /\ phase = "requested"
    /\ requestBase = sceneGeneration
    /\ LiveOutputs = Outputs
    /\ stagedBase' = requestBase
    /\ candidateState' = nextSettlement
    /\ phase' = "staged"
    /\ UNCHANGED <<sceneGeneration, outputLive, outputGeneration,
                    requestBase, reducerState, layoutState, settlementLog,
                    nextSettlement, rightEverLost>>

\* LivePublicPolicyState installs the complete candidate in PersistentLiveLayout
\* (crates/sophia-cli/src/commands/live_session/wm/public_policy.rs:547-584).
SubmitFrontendLayout ==
    /\ phase = "staged"
    /\ phase' = "frontend"
    /\ UNCHANGED <<sceneGeneration, outputLive, outputGeneration,
                    requestBase, stagedBase, candidateState, reducerState,
                    layoutState, settlementLog, nextSettlement,
                    rightEverLost>>

\* revalidate_staged is non-mutating and compares the canonical scene base
\* (crates/sophia-engine/src/policy_projection.rs:266-283).
PrepareCurrentCandidate ==
    /\ phase = "frontend"
    /\ CandidateCurrent
    /\ phase' = "prepared"
    /\ UNCHANGED <<sceneGeneration, outputLive, outputGeneration,
                    requestBase, stagedBase, candidateState, reducerState,
                    layoutState, settlementLog, nextSettlement,
                    rightEverLost>>

\* Output topology observation advances the scene before any future policy
\* settlement. Identity generations are tracked by public_policy.rs:143-153.
ObserveRightOutputLoss ==
    /\ outputLive["right"]
    /\ sceneGeneration < MaxSceneGeneration
    /\ outputLive' = [outputLive EXCEPT !["right"] = FALSE]
    /\ sceneGeneration' = sceneGeneration + 1
    /\ rightEverLost' = TRUE
    /\ UNCHANGED <<phase, outputGeneration, requestBase, stagedBase,
                    candidateState, reducerState, layoutState, settlementLog,
                    nextSettlement>>

ObserveRightOutputReturn ==
    /\ ~outputLive["right"]
    /\ sceneGeneration < MaxSceneGeneration
    /\ outputGeneration["right"] < MaxSceneGeneration
    /\ outputLive' = [outputLive EXCEPT !["right"] = TRUE]
    /\ outputGeneration' =
           [outputGeneration EXCEPT !["right"] = @ + 1]
    /\ sceneGeneration' = sceneGeneration + 1
    /\ UNCHANGED <<phase, requestBase, stagedBase, candidateState,
                    reducerState, layoutState, settlementLog,
                    nextSettlement, rightEverLost>>

\* A request or unprepared candidate superseded by topology is rejected
\* without changing either authoritative last-good projection.
RejectSupersededCandidate ==
    /\ phase \in {"requested", "staged", "frontend"}
    /\ \/ requestBase # sceneGeneration
       \/ LiveOutputs # Outputs
    /\ settlementLog' = Append(settlementLog,
           [outcome |-> "stale",
            base |-> IF stagedBase = 0 THEN requestBase ELSE stagedBase,
            current |-> sceneGeneration,
            live |-> LiveOutputs])
    /\ nextSettlement' = nextSettlement + 1
    /\ phase' = "idle"
    /\ requestBase' = 0
    /\ stagedBase' = 0
    /\ candidateState' = 0
    /\ UNCHANGED <<sceneGeneration, outputLive, outputGeneration,
                    reducerState, layoutState, rightEverLost>>

\* commit_staged rechecks the same base immediately before promotion
\* (crates/sophia-engine/src/policy_projection.rs:286-306). The frontend and
\* reducer candidates settle together; a stale prepared candidate changes
\* neither side.
SettlePreparedCandidate ==
    /\ phase = "prepared"
    /\ LET valid == CandidateCurrent IN
       /\ settlementLog' = Append(settlementLog,
              [outcome |-> IF valid THEN "committed" ELSE "stale",
               base |-> stagedBase,
               current |-> sceneGeneration,
               live |-> LiveOutputs])
       /\ reducerState' =
              IF valid THEN [output \in Outputs |-> candidateState]
              ELSE reducerState
       /\ layoutState' =
              IF valid THEN [output \in Outputs |-> candidateState]
              ELSE layoutState
    /\ nextSettlement' = nextSettlement + 1
    /\ phase' = "idle"
    /\ requestBase' = 0
    /\ stagedBase' = 0
    /\ candidateState' = 0
    /\ UNCHANGED <<sceneGeneration, outputLive, outputGeneration,
                    rightEverLost>>

Next ==
    \/ IssueCrossOutputRequest
    \/ StageCompleteProposal
    \/ SubmitFrontendLayout
    \/ PrepareCurrentCandidate
    \/ ObserveRightOutputLoss
    \/ ObserveRightOutputReturn
    \/ RejectSupersededCandidate
    \/ SettlePreparedCandidate

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ phase \in {"idle", "requested", "staged", "frontend", "prepared"}
    /\ sceneGeneration \in 1..MaxSceneGeneration
    /\ outputLive \in [Outputs -> BOOLEAN]
    /\ outputGeneration \in [Outputs -> 1..MaxSceneGeneration]
    /\ requestBase \in 0..MaxSceneGeneration
    /\ stagedBase \in 0..MaxSceneGeneration
    /\ candidateState \in 0..MaxSettlements
    /\ reducerState \in [Outputs -> 0..MaxSettlements]
    /\ layoutState \in [Outputs -> 0..MaxSettlements]
    /\ Len(settlementLog) <= MaxSettlements
    /\ nextSettlement \in 1..(MaxSettlements + 1)
    /\ rightEverLost \in BOOLEAN

LastGoodIsAtomic == reducerState = layoutState

CommittedTopologyWasCurrent ==
    \A index \in 1..Len(settlementLog) :
        LET entry == settlementLog[index] IN
            entry.outcome = "committed" =>
                /\ entry.base = entry.current
                /\ entry.live = Outputs

ReappearedOutputIsFresh ==
    outputLive["right"] /\ rightEverLost => outputGeneration["right"] > 1

=============================================================================
