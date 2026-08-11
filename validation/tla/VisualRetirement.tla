-------------------------- MODULE VisualRetirement --------------------------
EXTENDS Integers, FiniteSets

(***************************************************************************
 * A bounded authority model for Sophia's visual transition lifetime.       *
 *                                                                         *
 * Generations stand for immutable prepared candidates. Outputs are the     *
 * logical, output-scoped presentation requirements of each candidate. Each *
 * logical output is backed by one or more heads, so a mirror group is a    *
 * logical output with more than one head. This model intentionally omits   *
 * protocol objects, pixels, renderer handles, and KMS details.            *
 *                                                                         *
 * The head layer exists because mirroring makes a logical output's         *
 * retirement a join over its heads: one framebuffer is scanned out by      *
 * every head in the group, so the buffer stays leased until the last head  *
 * flips. Presentation therefore remains output-scoped, and retirement is   *
 * joint within a mirror group and independent between groups. Sophia still *
 * claims no globally simultaneous multi-output retirement instant.         *
 ****************************************************************************)

CONSTANTS Outputs, Generations, NoGeneration

ASSUME /\ Outputs # {}
       /\ Generations # {}
       /\ Generations \subseteq (Nat \ {0})
       /\ NoGeneration = 0

(***************************************************************************
 * The head topology is a definition rather than a model value so the       *
 * configuration stays limited to simple sets. One output is a two-head     *
 * mirror group and every other output drives a single head, which exercises *
 * joint retirement and independent retirement in the same run. CHOOSE is   *
 * deterministic, so the mirrored output is fixed for the whole check.      *
 ****************************************************************************)
MirroredOutput == CHOOSE o \in Outputs : TRUE
HeadsOf(o) == IF o = MirroredOutput
                 THEN {<<o, 1>>, <<o, 2>>}
                 ELSE {<<o, 1>>}
Heads == UNION {HeadsOf(o) : o \in Outputs}

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
    flipped,
    lost,
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
    flipped,
    lost,
    inFlight,
    outcome,
    committed,
    inputGeneration,
    feedback,
    released
>>

(***************************************************************************
 * Every head of a submitted output takes the lease. A logical output is    *
 * retired only when all of its heads have flipped, which is what makes a   *
 * mirror group retire jointly.                                            *
 ****************************************************************************)
SubmittedHeads(g) == UNION {HeadsOf(o) : o \in submitted[g]}
RetiredOutputs(g) == {o \in Outputs : HeadsOf(o) \subseteq flipped[g]}

Init ==
    /\ phase = [g \in Generations |-> "absent"]
    /\ required = [g \in Generations |-> {}]
    /\ submitted = [g \in Generations |-> {}]
    /\ flipped = [g \in Generations |-> {}]
    /\ lost = [g \in Generations |-> {}]
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
        submitted, flipped, lost, inFlight, outcome, committed,
        inputGeneration, feedback, released
        >>

Prepare(g) ==
    /\ phase[g] = "proposed"
    /\ outcome[g] = "none"
    /\ phase' = [phase EXCEPT ![g] = "prepared"]
    /\ UNCHANGED <<
        required, submitted, flipped, lost, inFlight, outcome, committed,
        inputGeneration, feedback, released
        >>

Submit(g, output) ==
    /\ phase[g] \in {"prepared", "submitted"}
    /\ outcome[g] = "none"
    /\ output \in required[g] \ submitted[g]
    /\ phase' = [phase EXCEPT ![g] = "submitted"]
    /\ submitted' = [submitted EXCEPT ![g] = @ \cup {output}]
    /\ inFlight' = [inFlight EXCEPT ![g] = @ \cup HeadsOf(output)]
    /\ UNCHANGED <<
        required, flipped, lost, outcome, committed, inputGeneration, feedback,
        released
        >>

(***************************************************************************
 * One head completes its page flip. The candidate settles only once every  *
 * required output has every one of its heads flipped, so a partially       *
 * flipped mirror group can never be mistaken for a retired output.         *
 ****************************************************************************)
Flip(g, head) ==
    LET newFlipped == flipped[g] \cup {head}
        newInFlight == inFlight[g] \ {head}
        newRetired == {o \in Outputs : HeadsOf(o) \subseteq newFlipped}
    IN
    /\ head \in inFlight[g]
    /\ flipped' = [flipped EXCEPT ![g] = newFlipped]
    /\ inFlight' = [inFlight EXCEPT ![g] = newInFlight]
    /\ IF outcome[g] = "none" /\ required[g] \subseteq newRetired
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
    /\ UNCHANGED <<required, submitted, lost, released>>

(***************************************************************************
 * A head disappears while holding the lease, which is how mirror-group     *
 * member loss reaches this boundary. The lease is dropped without counting  *
 * as a flip, and the candidate fails closed rather than committing a        *
 * partial group: a surviving-head topology is a new candidate, not a        *
 * salvaged one. The buffer becomes releasable only once the remaining heads *
 * drain.                                                                   *
 ****************************************************************************)
LoseHead(g, head) ==
    /\ head \in inFlight[g]
    /\ lost' = [lost EXCEPT ![g] = @ \cup {head}]
    /\ inFlight' = [inFlight EXCEPT ![g] = @ \ {head}]
    /\ IF outcome[g] = "none"
          THEN
              /\ phase' = [phase EXCEPT ![g] = "settled"]
              /\ outcome' = [outcome EXCEPT ![g] = "removed"]
          ELSE
              UNCHANGED <<phase, outcome>>
    /\ UNCHANGED <<
        required, submitted, flipped, committed, inputGeneration, feedback,
        released
        >>

Settle(g, result) ==
    /\ phase[g] \in {"proposed", "prepared", "submitted"}
    /\ outcome[g] = "none"
    /\ result \in {"rejected", "timed_out", "disconnected", "removed"}
    /\ phase' = [phase EXCEPT ![g] = "settled"]
    /\ outcome' = [outcome EXCEPT ![g] = result]
    /\ UNCHANGED <<
        required, submitted, flipped, lost, inFlight, committed,
        inputGeneration, feedback, released
        >>

Release(g) ==
    /\ g \notin released
    /\ outcome[g] \in TerminalOutcomes
    /\ g # committed
    /\ inFlight[g] = {}
    /\ released' = released \cup {g}
    /\ UNCHANGED <<
        phase, required, submitted, flipped, lost, inFlight, outcome,
        committed, inputGeneration, feedback
        >>

Progress(g) ==
    \/ Prepare(g)
    \/ \E output \in Outputs : Submit(g, output)
    \/ \E head \in Heads : Flip(g, head)
    \/ \E result \in {"rejected", "timed_out", "disconnected", "removed"} :
        Settle(g, result)

Next ==
    \/ \E g \in Generations :
        \E outs \in SUBSET Outputs : Propose(g, outs)
    \/ \E g \in Generations : Progress(g)
    \/ \E g \in Generations : \E head \in Heads : LoseHead(g, head)
    \/ \E g \in Generations : Release(g)

Spec == Init /\ [][Next]_vars

(***************************************************************************
 * Weak fairness is an explicit environment assumption: once an admitted    *
 * generation continually has a legal progress action, the scheduler and    *
 * backend eventually take one. It does not assume that failed hardware     *
 * reports success. Head loss is deliberately outside the fairness          *
 * assumption, because nothing guarantees a connector disappears.           *
 ****************************************************************************)
FairSpec == Spec /\ \A g \in Generations : WF_vars(Progress(g))

TypeOK ==
    /\ phase \in [Generations -> Phases]
    /\ required \in [Generations -> SUBSET Outputs]
    /\ submitted \in [Generations -> SUBSET Outputs]
    /\ flipped \in [Generations -> SUBSET Heads]
    /\ lost \in [Generations -> SUBSET Heads]
    /\ inFlight \in [Generations -> SUBSET Heads]
    /\ outcome \in [Generations -> Outcomes]
    /\ committed \in Generations \cup {NoGeneration}
    /\ inputGeneration \in Generations \cup {NoGeneration}
    /\ feedback \subseteq Generations
    /\ released \subseteq Generations

(***************************************************************************
 * The head sets of distinct logical outputs are disjoint. Mirroring adds   *
 * heads to one logical output; it never makes two logical outputs share a  *
 * head. Without this, one group's flip could mark another group's output    *
 * retired.                                                                 *
 ****************************************************************************)
HeadsPartitionOutputs ==
    \A o1, o2 \in Outputs :
        o1 # o2 => HeadsOf(o1) \cap HeadsOf(o2) = {}

SubmissionAccounting ==
    \A g \in Generations :
        /\ submitted[g] \subseteq required[g]
        /\ flipped[g] \subseteq SubmittedHeads(g)
        /\ lost[g] \subseteq SubmittedHeads(g)
        /\ flipped[g] \cap lost[g] = {}
        /\ inFlight[g] = SubmittedHeads(g) \ (flipped[g] \cup lost[g])

CommittedAfterExactRetirement ==
    \A g \in Generations :
        outcome[g] = "committed" =>
            /\ required[g] # {}
            /\ required[g] \subseteq RetiredOutputs(g)

(***************************************************************************
 * The mirroring safety property. A committed candidate has flipped every    *
 * head of every required output, so no mirror group is ever committed half  *
 * presented.                                                               *
 *                                                                         *
 * This reads like a restatement of CommittedAfterExactRetirement and is    *
 * not. That invariant is expressed through RetiredOutputs, so it goes blind *
 * exactly when that definition is itself wrong. This one names the heads    *
 * directly and therefore still fails if retirement is ever redefined to     *
 * discount a head -- which is the likely shape of a mirroring regression.   *
 * A negative control that makes loss silently shrink a group violates this  *
 * invariant while leaving CommittedAfterExactRetirement satisfied.          *
 ****************************************************************************)
MirrorGroupCommitsOnlyWithEveryHead ==
    \A g \in Generations :
        outcome[g] = "committed" =>
            \A o \in required[g] : HeadsOf(o) \subseteq flipped[g]

PartialMirrorGroupNeverCommits ==
    \A g \in Generations :
        (\E o \in required[g] : HeadsOf(o) \cap lost[g] # {}) =>
            outcome[g] # "committed"

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

(***************************************************************************
 * The buffer-lifetime guarantee mirroring depends on -- that no generation  *
 * is released while any head still scans it out, including heads whose      *
 * mirror-group siblings already flipped -- needs no separate invariant.     *
 * ReleasedResourcesAreTerminal states it directly on inFlight, which is now *
 * head-scoped, and there is no derived definition in between that could go  *
 * wrong independently.                                                     *
 ****************************************************************************)

AdmittedEventuallySettles ==
    \A g \in Generations :
        (phase[g] # "absent") ~> (outcome[g] \in TerminalOutcomes)

=============================================================================
