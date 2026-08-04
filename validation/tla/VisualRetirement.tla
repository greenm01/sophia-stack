-------------------------- MODULE VisualRetirement --------------------------
EXTENDS Integers, FiniteSets

(***************************************************************************
 * A bounded authority model for Sophia's visual transition lifetime.       *
 *                                                                         *
 * Generations stand for immutable prepared candidates. Outputs are the     *
 * output-scoped presentation requirements of each candidate. This model    *
 * intentionally omits protocol objects, pixels, renderer handles, and KMS  *
 * details.                                                                 *
 ****************************************************************************)

CONSTANTS Outputs, Generations, NoGeneration

ASSUME /\ Outputs # {}
       /\ Generations # {}
       /\ Generations \subseteq (Nat \ {0})
       /\ NoGeneration = 0

Phases == {"absent", "proposed", "prepared", "submitted", "settled"}
Outcomes == {
    "none",
    "committed",
    "rejected",
    "superseded",
    "timed_out",
    "disconnected",
    "removed"
}

TerminalOutcomes == Outcomes \ {"none"}

VARIABLES
    phase,
    required,
    submitted,
    retired,
    inFlight,
    outcome,
    committed,
    inputGeneration,
    feedback,
    released

vars == <<
    phase,
    required,
    submitted,
    retired,
    inFlight,
    outcome,
    committed,
    inputGeneration,
    feedback,
    released
>>

Init ==
    /\ phase = [g \in Generations |-> "absent"]
    /\ required = [g \in Generations |-> {}]
    /\ submitted = [g \in Generations |-> {}]
    /\ retired = [g \in Generations |-> {}]
    /\ inFlight = [g \in Generations |-> {}]
    /\ outcome = [g \in Generations |-> "none"]
    /\ committed = NoGeneration
    /\ inputGeneration = NoGeneration
    /\ feedback = {}
    /\ released = {}

Propose(g, outs) ==
    /\ phase[g] = "absent"
    /\ g > committed
    /\ outs \in SUBSET Outputs
    /\ outs # {}
    /\ phase' = [phase EXCEPT ![g] = "proposed"]
    /\ required' = [required EXCEPT ![g] = outs]
    /\ UNCHANGED <<
        submitted, retired, inFlight, outcome, committed,
        inputGeneration, feedback, released
        >>

Prepare(g) ==
    /\ phase[g] = "proposed"
    /\ outcome[g] = "none"
    /\ phase' = [phase EXCEPT ![g] = "prepared"]
    /\ UNCHANGED <<
        required, submitted, retired, inFlight, outcome, committed,
        inputGeneration, feedback, released
        >>

Submit(g, output) ==
    /\ phase[g] \in {"prepared", "submitted"}
    /\ outcome[g] = "none"
    /\ output \in required[g] \ submitted[g]
    /\ phase' = [phase EXCEPT ![g] = "submitted"]
    /\ submitted' = [submitted EXCEPT ![g] = @ \cup {output}]
    /\ inFlight' = [inFlight EXCEPT ![g] = @ \cup {output}]
    /\ UNCHANGED <<
        required, retired, outcome, committed, inputGeneration, feedback,
        released
        >>

Retire(g, output) ==
    LET newRetired == retired[g] \cup {output}
        newInFlight == inFlight[g] \ {output}
    IN
    /\ output \in inFlight[g]
    /\ retired' = [retired EXCEPT ![g] = newRetired]
    /\ inFlight' = [inFlight EXCEPT ![g] = newInFlight]
    /\ IF outcome[g] = "none" /\ newRetired = required[g]
          THEN
              /\ phase' = [phase EXCEPT ![g] = "settled"]
              /\ IF g > committed
                    THEN
                        /\ outcome' = [outcome EXCEPT ![g] = "committed"]
                        /\ committed' = g
                        /\ inputGeneration' = g
                        /\ feedback' = feedback \cup {g}
                    ELSE
                        /\ outcome' = [outcome EXCEPT ![g] = "superseded"]
                        /\ UNCHANGED <<committed, inputGeneration, feedback>>
          ELSE
              UNCHANGED <<phase, outcome, committed, inputGeneration, feedback>>
    /\ UNCHANGED <<required, submitted, released>>

Settle(g, result) ==
    /\ phase[g] \in {"proposed", "prepared", "submitted"}
    /\ outcome[g] = "none"
    /\ result \in {"rejected", "timed_out", "disconnected", "removed"}
    /\ phase' = [phase EXCEPT ![g] = "settled"]
    /\ outcome' = [outcome EXCEPT ![g] = result]
    /\ UNCHANGED <<
        required, submitted, retired, inFlight, committed, inputGeneration,
        feedback, released
        >>

Release(g) ==
    /\ g \notin released
    /\ outcome[g] \in TerminalOutcomes
    /\ g # committed
    /\ inFlight[g] = {}
    /\ released' = released \cup {g}
    /\ UNCHANGED <<
        phase, required, submitted, retired, inFlight, outcome, committed,
        inputGeneration, feedback
        >>

Progress(g) ==
    \/ Prepare(g)
    \/ \E output \in Outputs : Submit(g, output)
    \/ \E output \in Outputs : Retire(g, output)
    \/ \E result \in {"rejected", "timed_out", "disconnected", "removed"} :
        Settle(g, result)

Next ==
    \/ \E g \in Generations :
        \E outs \in SUBSET Outputs : Propose(g, outs)
    \/ \E g \in Generations : Progress(g)
    \/ \E g \in Generations : Release(g)

Spec == Init /\ [][Next]_vars

(***************************************************************************
 * Weak fairness is an explicit environment assumption: once an admitted    *
 * generation continually has a legal progress action, the scheduler and    *
 * backend eventually take one. It does not assume that failed hardware     *
 * reports success.                                                         *
 ****************************************************************************)
FairSpec == Spec /\ \A g \in Generations : WF_vars(Progress(g))

TypeOK ==
    /\ phase \in [Generations -> Phases]
    /\ required \in [Generations -> SUBSET Outputs]
    /\ submitted \in [Generations -> SUBSET Outputs]
    /\ retired \in [Generations -> SUBSET Outputs]
    /\ inFlight \in [Generations -> SUBSET Outputs]
    /\ outcome \in [Generations -> Outcomes]
    /\ committed \in Generations \cup {NoGeneration}
    /\ inputGeneration \in Generations \cup {NoGeneration}
    /\ feedback \subseteq Generations
    /\ released \subseteq Generations

SubmissionAccounting ==
    \A g \in Generations :
        /\ submitted[g] \subseteq required[g]
        /\ retired[g] \subseteq submitted[g]
        /\ inFlight[g] = submitted[g] \ retired[g]

CommittedAfterExactRetirement ==
    \A g \in Generations :
        outcome[g] = "committed" =>
            /\ required[g] # {}
            /\ retired[g] = required[g]

CommittedGenerationDominatesHistory ==
    \A g \in Generations :
        /\ outcome[g] = "committed" => committed >= g
        /\ outcome[g] = "superseded" => committed >= g

InputMatchesCommitted == inputGeneration = committed

FeedbackMatchesCommit ==
    \A g \in Generations : (g \in feedback) <=> (outcome[g] = "committed")

ActiveResourcesRetained ==
    \A g \in Generations :
        (phase[g] \in {"proposed", "prepared", "submitted"}
         \/ inFlight[g] # {}) => g \notin released

CommittedResourceRetained ==
    committed # NoGeneration => committed \notin released

ReleasedResourcesAreTerminal ==
    \A g \in released :
        /\ outcome[g] \in TerminalOutcomes
        /\ inFlight[g] = {}
        /\ g # committed

AdmittedEventuallySettles ==
    \A g \in Generations :
        (phase[g] # "absent") ~> (outcome[g] \in TerminalOutcomes)

=============================================================================
