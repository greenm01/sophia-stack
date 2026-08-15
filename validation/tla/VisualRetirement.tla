-------------------------- MODULE VisualRetirement --------------------------
EXTENDS Integers, FiniteSets

(***************************************************************************
 * A bounded authority model for Sophia's visual transition lifetime.       *
 *                                                                         *
 * Generations are immutable visual candidates. Each candidate names one   *
 * or more logical outputs, and every logical output is backed by one or    *
 * more physical heads. Preparation, submission, callbacks, and leases are  *
 * head-scoped. Logical retirement joins the complete head set of one       *
 * output, while unrelated outputs retain independent commit clocks.        *
 *                                                                         *
 * A transaction may span outputs without claiming simultaneous physical    *
 * presentation. Each output commits at its own last-head callback, but     *
 * transaction feedback and matching input state wait until every required  *
 * output has retired that generation. An active candidate reserves all of  *
 * its required outputs, so a successor cannot advance one member before    *
 * that feedback join completes.                                           *
 ****************************************************************************)

CONSTANTS Outputs, Generations, NoGeneration

ASSUME /\ Outputs # {}
       /\ Generations # {}
       /\ Generations \subseteq (Nat \ {0})
       /\ NoGeneration = 0

(***************************************************************************
 * One output is a two-head mirror group; every other output has one head.  *
 * The checked configuration therefore covers joint mirror retirement and   *
 * independent output progress in the smallest useful state space.          *
 ****************************************************************************)
MirroredOutput == CHOOSE o \in Outputs : TRUE
HeadsOf(o) == IF o = MirroredOutput
                 THEN {<<o, 1>>, <<o, 2>>}
                 ELSE {<<o, 1>>}
Heads == UNION {HeadsOf(o) : o \in Outputs}
HeadOutput(head) == CHOOSE o \in Outputs : head \in HeadsOf(o)

Phases == {"absent", "proposed", "preparing", "prepared", "submitted", "settled"}
Outcomes == {
    "none",
    "committed",
    "rejected",
    "superseded",
    "timed_out",
    "disconnected",
    "removed",
    "head_lost"
}
OutputOutcomes == {"not_required", "pending", "committed", "superseded", "failed"}
TerminalOutcomes == Outcomes \ {"none"}
FailureOutcomes == {"rejected", "timed_out", "disconnected", "removed"}

VARIABLES
    phase,
    required,
    prepared,
    submitted,
    flipped,
    lost,
    inFlight,
    outputOutcome,
    outcome,
    committed,
    inputGeneration,
    feedback,
    released

vars == <<
    phase,
    required,
    prepared,
    submitted,
    flipped,
    lost,
    inFlight,
    outputOutcome,
    outcome,
    committed,
    inputGeneration,
    feedback,
    released
>>

RequiredHeads(g) == UNION {HeadsOf(o) : o \in required[g]}

(***************************************************************************
 * Once a candidate submits any head, every output named by that candidate  *
 * is reserved until the candidate settles. Disjoint-output candidates may  *
 * remain active concurrently.                                             *
 ****************************************************************************)
CandidateOutputsAvailable(g) ==
    \A other \in Generations \ {g} :
        (phase[other] = "submitted" /\ outcome[other] = "none") =>
            required[g] \cap required[other] = {}

HeadFreeForGeneration(g, head) ==
    \A other \in Generations \ {g} : head \notin inFlight[other]

InitialOutputOutcome == [o \in Outputs |-> "not_required"]

Init ==
    /\ phase = [g \in Generations |-> "absent"]
    /\ required = [g \in Generations |-> {}]
    /\ prepared = [g \in Generations |-> {}]
    /\ submitted = [g \in Generations |-> {}]
    /\ flipped = [g \in Generations |-> {}]
    /\ lost = [g \in Generations |-> {}]
    /\ inFlight = [g \in Generations |-> {}]
    /\ outputOutcome = [g \in Generations |-> InitialOutputOutcome]
    /\ outcome = [g \in Generations |-> "none"]
    /\ committed = [o \in Outputs |-> NoGeneration]
    /\ inputGeneration = [o \in Outputs |-> NoGeneration]
    /\ feedback = {}
    /\ released = {}

Propose(g, outs) ==
    /\ phase[g] = "absent"
    /\ outs \in SUBSET Outputs
    /\ outs # {}
    /\ \A o \in outs : g > committed[o]
    /\ phase' = [phase EXCEPT ![g] = "proposed"]
    /\ required' = [required EXCEPT ![g] = outs]
    /\ outputOutcome' = [outputOutcome EXCEPT
        ![g] = [o \in Outputs |-> IF o \in outs THEN "pending" ELSE "not_required"]]
    /\ UNCHANGED <<
        prepared, submitted, flipped, lost, inFlight, outcome, committed,
        inputGeneration, feedback, released
        >>

PrepareHead(g, head) ==
    LET newPrepared == prepared[g] \cup {head}
    IN
    /\ phase[g] \in {"proposed", "preparing", "prepared", "submitted"}
    /\ outcome[g] = "none"
    /\ head \in RequiredHeads(g) \ prepared[g]
    /\ prepared' = [prepared EXCEPT ![g] = newPrepared]
    /\ phase' = [phase EXCEPT
        ![g] = IF phase[g] = "submitted"
                   THEN "submitted"
                   ELSE IF RequiredHeads(g) \subseteq newPrepared
                           THEN "prepared"
                           ELSE "preparing"]
    /\ UNCHANGED <<
        required, submitted, flipped, lost, inFlight, outputOutcome, outcome,
        committed, inputGeneration, feedback, released
        >>

SubmitHead(g, head) ==
    LET output == HeadOutput(head)
    IN
    /\ phase[g] \in {"preparing", "prepared", "submitted"}
    /\ outcome[g] = "none"
    /\ head \in prepared[g] \ submitted[g]
    /\ output \in required[g]
    /\ HeadsOf(output) \subseteq prepared[g]
    /\ g > committed[output]
    /\ CandidateOutputsAvailable(g)
    /\ \A member \in HeadsOf(output) : HeadFreeForGeneration(g, member)
    /\ phase' = [phase EXCEPT ![g] = "submitted"]
    /\ submitted' = [submitted EXCEPT ![g] = @ \cup {head}]
    /\ inFlight' = [inFlight EXCEPT ![g] = @ \cup {head}]
    /\ UNCHANGED <<
        required, prepared, flipped, lost, outputOutcome, outcome, committed,
        inputGeneration, feedback, released
        >>

(***************************************************************************
 * A physical callback only transfers the head lease. Logical output state  *
 * changes in RetireOutput after every required head has flipped.           *
 ****************************************************************************)
FlipHead(g, head) ==
    /\ head \in inFlight[g]
    /\ flipped' = [flipped EXCEPT ![g] = @ \cup {head}]
    /\ inFlight' = [inFlight EXCEPT ![g] = @ \ {head}]
    /\ UNCHANGED <<
        phase, required, prepared, submitted, lost, outputOutcome, outcome,
        committed, inputGeneration, feedback, released
        >>

(***************************************************************************
 * Retirement commits unconditionally, because a stale generation cannot     *
 * reach this action: SubmitHead refuses a generation the output has already  *
 * passed, and the whole-cohort reservation keeps the output's committed      *
 * generation fixed for as long as this one holds it. A regression in either  *
 * guard would move an output's committed generation backwards here, which    *
 * OutputGenerationDominatesHistory catches. Relabelling the retirement       *
 * instead would hide it.                                                     *
 ****************************************************************************)
RetireOutput(g, output) ==
    /\ outcome[g] = "none"
    /\ output \in required[g]
    /\ outputOutcome[g][output] = "pending"
    /\ HeadsOf(output) \subseteq flipped[g]
    /\ outputOutcome' = [outputOutcome EXCEPT ![g][output] = "committed"]
    /\ committed' = [committed EXCEPT ![output] = g]
    /\ UNCHANGED <<
        phase, required, prepared, submitted, flipped, lost, inFlight, outcome,
        inputGeneration, feedback, released
        >>

(***************************************************************************
 * Supersession happens before the kernel, not after it. An output owns one   *
 * active generation and one latest-wins successor, so a candidate still      *
 * waiting to submit an output that a newer generation has already committed  *
 * loses that output and can never present it. This is the only way an        *
 * output leaves "pending" without retiring, and it is what lets a candidate  *
 * spanning several outputs settle when one of them moved on without it.      *
 ****************************************************************************)
SupersedeOutput(g, output) ==
    /\ outcome[g] = "none"
    /\ output \in required[g]
    /\ outputOutcome[g][output] = "pending"
    /\ committed[output] >= g
    /\ outputOutcome' = [outputOutcome EXCEPT ![g][output] = "superseded"]
    /\ UNCHANGED <<
        phase, required, prepared, submitted, flipped, lost, inFlight, outcome,
        committed, inputGeneration, feedback, released
        >>

CompleteCandidate(g) ==
    /\ outcome[g] = "none"
    /\ required[g] # {}
    /\ \A o \in required[g] : outputOutcome[g][o] \in {"committed", "superseded"}
    /\ phase' = [phase EXCEPT ![g] = "settled"]
    /\ IF \A o \in required[g] : outputOutcome[g][o] = "committed"
          THEN
              /\ outcome' = [outcome EXCEPT ![g] = "committed"]
              /\ inputGeneration' = [o \in Outputs |->
                    IF o \in required[g] THEN g ELSE inputGeneration[o]]
              /\ feedback' = feedback \cup {g}
          ELSE
              /\ outcome' = [outcome EXCEPT ![g] = "superseded"]
              /\ UNCHANGED <<inputGeneration, feedback>>
    /\ UNCHANGED <<
        required, prepared, submitted, flipped, lost, inFlight, outputOutcome,
        committed, released
        >>

FailedOutputs(g) == [o \in Outputs |->
    IF outputOutcome[g][o] = "pending" THEN "failed" ELSE outputOutcome[g][o]]

(***************************************************************************
 * Head loss is distinct from protocol-side surface removal. It never counts *
 * as a flip, and already accepted sibling work remains in flight until its   *
 * real callback or controlled detach drains the lease.                      *
 ****************************************************************************)
LoseHead(g, head) ==
    /\ head \in inFlight[g]
    /\ lost' = [lost EXCEPT ![g] = @ \cup {head}]
    /\ inFlight' = [inFlight EXCEPT ![g] = @ \ {head}]
    /\ IF outcome[g] = "none"
          THEN
              /\ phase' = [phase EXCEPT ![g] = "settled"]
              /\ outputOutcome' = [outputOutcome EXCEPT ![g] = FailedOutputs(g)]
              /\ outcome' = [outcome EXCEPT ![g] = "head_lost"]
          ELSE
              UNCHANGED <<phase, outputOutcome, outcome>>
    /\ UNCHANGED <<
        required, prepared, submitted, flipped, committed, inputGeneration,
        feedback, released
        >>

Settle(g, result) ==
    /\ phase[g] \in {"proposed", "preparing", "prepared", "submitted"}
    /\ outcome[g] = "none"
    /\ result \in FailureOutcomes
    /\ phase' = [phase EXCEPT ![g] = "settled"]
    /\ outputOutcome' = [outputOutcome EXCEPT ![g] = FailedOutputs(g)]
    /\ outcome' = [outcome EXCEPT ![g] = result]
    /\ UNCHANGED <<
        required, prepared, submitted, flipped, lost, inFlight, committed,
        inputGeneration, feedback, released
        >>

(***************************************************************************
 * The last conjunct outlives failure. A generation that committed one output *
 * and then lost a head elsewhere is terminal but still on that screen, so it  *
 * stays retained until a successor commits the output. Failing is not what     *
 * frees a buffer; being replaced is.                                          *
 ****************************************************************************)
Release(g) ==
    /\ g \notin released
    /\ outcome[g] \in TerminalOutcomes
    /\ inFlight[g] = {}
    /\ \A o \in Outputs : committed[o] # g
    /\ released' = released \cup {g}
    /\ UNCHANGED <<
        phase, required, prepared, submitted, flipped, lost, inFlight,
        outputOutcome, outcome, committed, inputGeneration, feedback
        >>

Progress(g) ==
    \/ \E head \in Heads : PrepareHead(g, head)
    \/ \E head \in Heads : SubmitHead(g, head)
    \/ \E head \in Heads : FlipHead(g, head)
    \/ \E output \in Outputs : RetireOutput(g, output)
    \/ \E output \in Outputs : SupersedeOutput(g, output)
    \/ CompleteCandidate(g)
    \/ \E result \in FailureOutcomes : Settle(g, result)

Next ==
    \/ \E g \in Generations :
        \E outs \in SUBSET Outputs : Propose(g, outs)
    \/ \E g \in Generations : Progress(g)
    \/ \E g \in Generations : \E head \in Heads : LoseHead(g, head)
    \/ \E g \in Generations : Release(g)

Spec == Init /\ [][Next]_vars

(***************************************************************************
 * Fairness is stated per action rather than over the whole progress          *
 * disjunction, because Settle is enabled in every non-terminal state: one    *
 * lumped assumption would let arbitrary failure discharge the obligation,    *
 * and any settlement property would then hold without a single productive    *
 * step ever being taken.                                                     *
 *                                                                            *
 * Settle is therefore deliberately absent here. Failure stays permitted and  *
 * never required, so the settlement property below has to be satisfied by    *
 * work that actually advances: preparing, submitting, retiring, superseding, *
 * completing. LoseHead is absent for the same reason it always was --        *
 * nothing guarantees a connector disappears. FlipHead is assumed fair        *
 * because a kernel that accepted a page flip does deliver its callback; that *
 * is the one environment obligation this model does take.                    *
 ****************************************************************************)
FairSpec == Spec /\ \A g \in Generations :
    /\ WF_vars(\E head \in Heads : PrepareHead(g, head))
    /\ WF_vars(\E head \in Heads : SubmitHead(g, head))
    /\ WF_vars(\E head \in Heads : FlipHead(g, head))
    /\ WF_vars(\E output \in Outputs : RetireOutput(g, output))
    /\ WF_vars(\E output \in Outputs : SupersedeOutput(g, output))
    /\ WF_vars(CompleteCandidate(g))

TypeOK ==
    /\ phase \in [Generations -> Phases]
    /\ required \in [Generations -> SUBSET Outputs]
    /\ prepared \in [Generations -> SUBSET Heads]
    /\ submitted \in [Generations -> SUBSET Heads]
    /\ flipped \in [Generations -> SUBSET Heads]
    /\ lost \in [Generations -> SUBSET Heads]
    /\ inFlight \in [Generations -> SUBSET Heads]
    /\ outputOutcome \in [Generations -> [Outputs -> OutputOutcomes]]
    /\ outcome \in [Generations -> Outcomes]
    /\ committed \in [Outputs -> Generations \cup {NoGeneration}]
    /\ inputGeneration \in [Outputs -> Generations \cup {NoGeneration}]
    /\ feedback \subseteq Generations
    /\ released \subseteq Generations

HeadsPartitionOutputs ==
    \A o1, o2 \in Outputs :
        o1 # o2 => HeadsOf(o1) \cap HeadsOf(o2) = {}

PreparationAccounting ==
    \A g \in Generations :
        /\ prepared[g] \subseteq RequiredHeads(g)
        /\ submitted[g] \subseteq prepared[g]

SubmissionAccounting ==
    \A g \in Generations :
        /\ flipped[g] \subseteq submitted[g]
        /\ lost[g] \subseteq submitted[g]
        /\ flipped[g] \cap lost[g] = {}
        /\ inFlight[g] = submitted[g] \ (flipped[g] \cup lost[g])

PrepareAllBeforeSubmit ==
    \A g \in Generations :
        \A head \in submitted[g] : HeadsOf(HeadOutput(head)) \subseteq prepared[g]

ExclusiveHeadInFlight ==
    \A g1, g2 \in Generations :
        g1 # g2 => inFlight[g1] \cap inFlight[g2] = {}

ExclusiveOutputCohortInFlight ==
    \A g1, g2 \in Generations :
        g1 # g2 =>
            \A o \in Outputs :
                inFlight[g1] \cap HeadsOf(o) # {} =>
                    inFlight[g2] \cap HeadsOf(o) = {}

ActiveOutputCohortsExclusive ==
    \A g1, g2 \in Generations :
        g1 # g2 /\ phase[g1] = "submitted" /\ outcome[g1] = "none"
                /\ phase[g2] = "submitted" /\ outcome[g2] = "none" =>
            required[g1] \cap required[g2] = {}

OutputCommitAfterHeadRetirement ==
    \A g \in Generations :
        \A o \in required[g] :
            outputOutcome[g][o] = "committed" => HeadsOf(o) \subseteq flipped[g]

MirrorGroupCommitsOnlyWithEveryHead ==
    \A g \in Generations :
        outcome[g] = "committed" =>
            \A o \in required[g] : HeadsOf(o) \subseteq flipped[g]

PartialMirrorGroupNeverCommits ==
    \A g \in Generations :
        lost[g] # {} => outcome[g] # "committed"

TerminalCandidatesSettleRequiredOutputs ==
    \A g \in Generations :
        outcome[g] \in TerminalOutcomes =>
            \A o \in required[g] : outputOutcome[g][o] # "pending"

OutputGenerationDominatesHistory ==
    \A g \in Generations :
        \A o \in required[g] :
            /\ outputOutcome[g][o] = "committed" => committed[o] >= g
            /\ outputOutcome[g][o] = "superseded" => committed[o] >= g

(***************************************************************************
 * The successor rule read from the other side: losing an output is a thing  *
 * that happens to work still waiting for the kernel. Once any head of a      *
 * cohort is submitted, that cohort's fate is its own -- it commits or it     *
 * fails, and no newer generation can relabel it.                            *
 ****************************************************************************)
SubmittedOutputsAreNeverSuperseded ==
    \A g \in Generations :
        \A o \in Outputs :
            outputOutcome[g][o] = "superseded" =>
                submitted[g] \cap HeadsOf(o) = {}

SupersededOutputsNeverPresent ==
    \A g \in Generations :
        \A o \in Outputs :
            outputOutcome[g][o] = "superseded" =>
                /\ committed[o] # g
                /\ inputGeneration[o] # g

SupersededCandidatesNeverPublishFeedback ==
    \A g \in Generations : outcome[g] = "superseded" => g \notin feedback

InputNeverLeadsVisual ==
    \A o \in Outputs : inputGeneration[o] <= committed[o]

CurrentCommittedTransactionPublishesInput ==
    \A g \in Generations :
        outcome[g] = "committed" =>
            \A o \in required[g] : committed[o] = g => inputGeneration[o] = g

FeedbackMatchesCommit ==
    \A g \in Generations : (g \in feedback) <=> (outcome[g] = "committed")

ActiveResourcesRetained ==
    \A g \in Generations :
        (phase[g] \in {"proposed", "preparing", "prepared", "submitted"}
         \/ inFlight[g] # {}) => g \notin released

CommittedResourcesRetained ==
    \A o \in Outputs : committed[o] # NoGeneration => committed[o] \notin released

ReleasedResourcesAreTerminal ==
    \A g \in released :
        /\ outcome[g] \in TerminalOutcomes
        /\ inFlight[g] = {}
        /\ \A o \in Outputs : committed[o] # g

AdmittedEventuallySettles ==
    \A g \in Generations :
        (phase[g] # "absent") ~> (outcome[g] \in TerminalOutcomes)

=============================================================================
