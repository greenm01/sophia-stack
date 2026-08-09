---------------------- MODULE PolicyRefreshLifecycle ----------------------
EXTENDS Naturals, FiniteSets

(***************************************************************************
 * PolicyDirty generation admission, affected-output coalescing, explicit  *
 * active-output proposals, and atomic frontend/reducer settlement. A dirty *
 * generation admitted during an older request remains pending for a later  *
 * complete refresh; it is never mistaken for a duplicate relayout.         *
 *************************************************************************)

CONSTANT MaxGeneration

ASSUME MaxGeneration \in Nat \ {0}

Outputs == {"left", "right"}

VARIABLES phase, policyGeneration, dirtyOutputs, requestGeneration,
          requestOutputs, stagedActiveOutput, committedActiveOutput,
          layoutActiveOutput, lastOutcome

vars == <<phase, policyGeneration, dirtyOutputs, requestGeneration,
          requestOutputs, stagedActiveOutput, committedActiveOutput,
          layoutActiveOutput, lastOutcome>>

Init ==
    /\ phase = "idle"
    /\ policyGeneration = 1
    /\ dirtyOutputs = {}
    /\ requestGeneration = 0
    /\ requestOutputs = {}
    /\ stagedActiveOutput = "none"
    /\ committedActiveOutput = "left"
    /\ layoutActiveOutput = "left"
    /\ lastOutcome = "none"

AdmitDirty(generation, outputs) ==
    /\ generation \in 2..MaxGeneration
    /\ generation > policyGeneration
    /\ outputs \subseteq Outputs
    /\ outputs # {}
    /\ policyGeneration' = generation
    /\ dirtyOutputs' = dirtyOutputs \cup outputs
    /\ UNCHANGED <<phase, requestGeneration, requestOutputs,
                    stagedActiveOutput, committedActiveOutput,
                    layoutActiveOutput, lastOutcome>>

IssueRefresh ==
    /\ phase = "idle"
    /\ dirtyOutputs # {}
    /\ phase' = "requested"
    /\ requestGeneration' = policyGeneration
    /\ requestOutputs' = dirtyOutputs
    /\ dirtyOutputs' = {}
    /\ stagedActiveOutput' = "none"
    /\ lastOutcome' = "none"
    /\ UNCHANGED <<policyGeneration, committedActiveOutput, layoutActiveOutput>>

StageProposal(output) ==
    /\ phase = "requested"
    /\ output \in Outputs
    /\ output # committedActiveOutput =>
          {output, committedActiveOutput} \subseteq requestOutputs
    /\ phase' = "staged"
    /\ stagedActiveOutput' = output
    /\ UNCHANGED <<policyGeneration, dirtyOutputs, requestGeneration,
                    requestOutputs, committedActiveOutput,
                    layoutActiveOutput, lastOutcome>>

RejectInvalidProposal ==
    /\ phase = "requested"
    /\ phase' = "idle"
    /\ requestGeneration' = 0
    /\ requestOutputs' = {}
    /\ stagedActiveOutput' = "none"
    /\ lastOutcome' = "invalid"
    /\ UNCHANGED <<policyGeneration, dirtyOutputs,
                    committedActiveOutput, layoutActiveOutput>>

SettleProposal ==
    /\ phase = "staged"
    /\ phase' = "idle"
    /\ committedActiveOutput' = stagedActiveOutput
    /\ layoutActiveOutput' = stagedActiveOutput
    /\ requestGeneration' = 0
    /\ requestOutputs' = {}
    /\ stagedActiveOutput' = "none"
    /\ lastOutcome' = "committed"
    /\ UNCHANGED <<policyGeneration, dirtyOutputs>>

AbortProposal ==
    /\ phase \in {"requested", "staged"}
    /\ phase' = "idle"
    /\ requestGeneration' = 0
    /\ requestOutputs' = {}
    /\ stagedActiveOutput' = "none"
    /\ lastOutcome' = "aborted"
    /\ UNCHANGED <<policyGeneration, dirtyOutputs,
                    committedActiveOutput, layoutActiveOutput>>

Next ==
    \/ \E generation \in 2..MaxGeneration, outputs \in SUBSET Outputs :
          AdmitDirty(generation, outputs)
    \/ IssueRefresh
    \/ \E output \in Outputs : StageProposal(output)
    \/ RejectInvalidProposal
    \/ SettleProposal
    \/ AbortProposal

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ phase \in {"idle", "requested", "staged"}
    /\ policyGeneration \in 1..MaxGeneration
    /\ dirtyOutputs \subseteq Outputs
    /\ requestGeneration \in 0..MaxGeneration
    /\ requestOutputs \subseteq Outputs
    /\ stagedActiveOutput \in Outputs \cup {"none"}
    /\ committedActiveOutput \in Outputs
    /\ layoutActiveOutput \in Outputs
    /\ lastOutcome \in {"none", "committed", "invalid", "aborted"}

ActiveOutputSettlesAtomically == committedActiveOutput = layoutActiveOutput

NewerDirtyIsRetained ==
    phase \in {"requested", "staged"} /\ policyGeneration > requestGeneration =>
        dirtyOutputs # {}

RequestNeverNamesFuturePolicy ==
    phase \in {"requested", "staged"} => requestGeneration <= policyGeneration

StagedSwitchHasCompleteScope ==
    phase = "staged" /\ stagedActiveOutput # committedActiveOutput =>
        {stagedActiveOutput, committedActiveOutput} \subseteq requestOutputs

=============================================================================
