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

\* Connectors backing one logical output. An output with both is a mirror group;
\* an output with one is the ordinary desktop. Head names are per-output labels,
\* not global connectors, which is why both outputs draw from the same set.
Heads == {"primary", "peer"}

VARIABLES phase, sceneGeneration, outputLive, outputGeneration, outputHeads,
          requestBase, stagedBase, stagedHeads, candidateState,
          reducerState, layoutState, settlementLog, nextSettlement,
          rightEverLost

vars == <<phase, sceneGeneration, outputLive, outputGeneration, outputHeads,
          requestBase, stagedBase, stagedHeads, candidateState,
          reducerState, layoutState, settlementLog, nextSettlement,
          rightEverLost>>

LiveOutputs == {output \in Outputs : outputLive[output]}

NoHeads == [output \in Outputs |-> {}]

\* A candidate is current when the scene it was staged against is still the scene
\* -- and the head sets are part of that scene. Without the head conjunct an
\* output could keep its identity, lose a connector, and still take a commit
\* computed for the wider group.
CandidateCurrent ==
    /\ stagedBase = sceneGeneration
    /\ LiveOutputs = Outputs
    /\ stagedHeads = outputHeads

Init ==
    /\ phase = "idle"
    /\ sceneGeneration = 1
    /\ outputLive = [output \in Outputs |-> TRUE]
    /\ outputGeneration = [output \in Outputs |-> 1]
    /\ outputHeads = [output \in Outputs |-> Heads]
    /\ requestBase = 0
    /\ stagedBase = 0
    /\ stagedHeads = NoHeads
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
    /\ UNCHANGED <<sceneGeneration, outputLive, outputGeneration, stagedBase, candidateState, reducerState, layoutState, settlementLog, nextSettlement, rightEverLost, outputHeads, stagedHeads>>

\* PolicyProjectionReducer::stage_proposal validates into a clone
\* (crates/sophia-engine/src/policy_projection.rs:246-264).
StageCompleteProposal ==
    /\ phase = "requested"
    /\ requestBase = sceneGeneration
    /\ LiveOutputs = Outputs
    /\ stagedBase' = requestBase
    \* The candidate is staged against a head set, and that is what later has to
    \* still be true. Recording it is what lets a commit be refused for a reason
    \* no output list can express.
    /\ stagedHeads' = outputHeads
    /\ candidateState' = nextSettlement
    /\ phase' = "staged"
    /\ UNCHANGED <<sceneGeneration, outputLive, outputGeneration, requestBase, reducerState, layoutState, settlementLog, nextSettlement, rightEverLost, outputHeads>>

\* LivePublicPolicyState installs the complete candidate in PersistentLiveLayout
\* (crates/sophia-session/src/live_session/wm/public_policy.rs:547-584).
SubmitFrontendLayout ==
    /\ phase = "staged"
    /\ phase' = "frontend"
    /\ UNCHANGED <<sceneGeneration, outputLive, outputGeneration, requestBase, stagedBase, candidateState, reducerState, layoutState, settlementLog, nextSettlement, rightEverLost, outputHeads, stagedHeads>>

\* revalidate_staged is non-mutating and compares the canonical scene base
\* (crates/sophia-engine/src/policy_projection.rs:266-283).
PrepareCurrentCandidate ==
    /\ phase = "frontend"
    /\ CandidateCurrent
    /\ phase' = "prepared"
    /\ UNCHANGED <<sceneGeneration, outputLive, outputGeneration, requestBase, stagedBase, candidateState, reducerState, layoutState, settlementLog, nextSettlement, rightEverLost, outputHeads, stagedHeads>>

\* Output topology observation advances the scene before any future policy
\* settlement. Identity generations are tracked by public_policy.rs:143-153.
ObserveRightOutputLoss ==
    /\ outputLive["right"]
    /\ sceneGeneration < MaxSceneGeneration
    /\ outputLive' = [outputLive EXCEPT !["right"] = FALSE]
    /\ sceneGeneration' = sceneGeneration + 1
    /\ rightEverLost' = TRUE
    /\ UNCHANGED <<phase, outputGeneration, requestBase, stagedBase, candidateState, reducerState, layoutState, settlementLog, nextSettlement, outputHeads, stagedHeads>>

ObserveRightOutputReturn ==
    /\ ~outputLive["right"]
    /\ sceneGeneration < MaxSceneGeneration
    /\ outputGeneration["right"] < MaxSceneGeneration
    /\ outputLive' = [outputLive EXCEPT !["right"] = TRUE]
    /\ outputGeneration' =
           [outputGeneration EXCEPT !["right"] = @ + 1]
    /\ sceneGeneration' = sceneGeneration + 1
    /\ UNCHANGED <<phase, requestBase, stagedBase, candidateState, reducerState, layoutState, settlementLog, nextSettlement, rightEverLost, outputHeads, stagedHeads>>

\* A mirror group loses one connector while the logical output survives. The
\* output set does not change, so this is invisible above the head layer -- and it
\* still has to advance the canonical scene, or an in-flight candidate computed
\* for the wider group would commit onto a group that no longer has those heads.
ObserveHeadLoss(output, head) ==
    /\ outputLive[output]
    /\ head \in outputHeads[output]
    /\ Cardinality(outputHeads[output]) > 1
    /\ sceneGeneration < MaxSceneGeneration
    /\ outputHeads' = [outputHeads EXCEPT ![output] = @ \ {head}]
    /\ sceneGeneration' = sceneGeneration + 1
    /\ UNCHANGED <<phase, outputLive, outputGeneration, requestBase, stagedBase, stagedHeads, candidateState, reducerState, layoutState, settlementLog, nextSettlement, rightEverLost>>

ObserveHeadReturn(output, head) ==
    /\ outputLive[output]
    /\ head \notin outputHeads[output]
    /\ sceneGeneration < MaxSceneGeneration
    /\ outputHeads' = [outputHeads EXCEPT ![output] = @ \union {head}]
    /\ sceneGeneration' = sceneGeneration + 1
    /\ UNCHANGED <<phase, outputLive, outputGeneration, requestBase, stagedBase, stagedHeads, candidateState, reducerState, layoutState, settlementLog, nextSettlement, rightEverLost>>

\* A request or unprepared candidate superseded by topology is rejected
\* without changing either authoritative last-good projection.
RejectSupersededCandidate ==
    /\ phase \in {"requested", "staged", "frontend"}
    /\ \/ requestBase # sceneGeneration
       \/ LiveOutputs # Outputs
       \/ (stagedBase # 0 /\ stagedHeads # outputHeads)
    /\ settlementLog' = Append(settlementLog,
           [outcome |-> "stale",
            base |-> IF stagedBase = 0 THEN requestBase ELSE stagedBase,
            current |-> sceneGeneration,
            live |-> LiveOutputs,
            heads |-> stagedHeads,
            currentHeads |-> outputHeads])
    /\ nextSettlement' = nextSettlement + 1
    /\ phase' = "idle"
    /\ requestBase' = 0
    /\ stagedBase' = 0
    /\ stagedHeads' = NoHeads
    /\ candidateState' = 0
    /\ UNCHANGED <<sceneGeneration, outputLive, outputGeneration, reducerState, layoutState, rightEverLost, outputHeads>>

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
               live |-> LiveOutputs,
               heads |-> stagedHeads,
               currentHeads |-> outputHeads])
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
    /\ stagedHeads' = NoHeads
    /\ candidateState' = 0
    /\ UNCHANGED <<sceneGeneration, outputLive, outputGeneration, rightEverLost, outputHeads>>

Next ==
    \/ IssueCrossOutputRequest
    \/ StageCompleteProposal
    \/ SubmitFrontendLayout
    \/ PrepareCurrentCandidate
    \/ ObserveRightOutputLoss
    \/ ObserveRightOutputReturn
    \/ \E output \in Outputs, head \in Heads : ObserveHeadLoss(output, head)
    \/ \E output \in Outputs, head \in Heads : ObserveHeadReturn(output, head)
    \/ RejectSupersededCandidate
    \/ SettlePreparedCandidate

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ phase \in {"idle", "requested", "staged", "frontend", "prepared"}
    /\ sceneGeneration \in 1..MaxSceneGeneration
    /\ outputLive \in [Outputs -> BOOLEAN]
    /\ outputGeneration \in [Outputs -> 1..MaxSceneGeneration]
    /\ outputHeads \in [Outputs -> (SUBSET Heads) \ {{}}]
    /\ stagedHeads \in [Outputs -> SUBSET Heads]
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
                /\ entry.heads = entry.currentHeads

\* The head layer's whole claim: a staged candidate whose base is still current
\* cannot be holding a stale head set. This is what makes the scene generation
\* load-bearing rather than decorative -- it must move on every head-set change,
\* not only when an output appears or disappears.
StagedHeadsCannotGoStaleSilently ==
    (stagedBase # 0 /\ stagedBase = sceneGeneration) => stagedHeads = outputHeads

ReappearedOutputIsFresh ==
    outputLive["right"] /\ rightEverLost => outputGeneration["right"] > 1

=============================================================================
