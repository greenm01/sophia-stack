---------------------- MODULE VisualRetirementSlots ----------------------
EXTENDS Integers, FiniteSets

(***************************************************************************
 * Focused extension of VisualRetirement for a three-slot native target     *
 * pool. The parent model owns cross-output transaction and mirror joining. *
 * This model keeps one mirrored logical output so four generations can     *
 * explore displayed + submitted + prepared ownership, bounded deferral,    *
 * exact release, reuse, and an ABA-shaped stale return.                     *
 *************************************************************************)

CONSTANTS Generations, Slots, Heads, NoGeneration, NoSlot

ASSUME /\ Generations # {}
       /\ Generations \subseteq (Nat \ {0})
       /\ Slots # {}
       /\ Cardinality(Slots) = 3
       /\ Heads # {}
       /\ Cardinality(Heads) = 2
       /\ NoGeneration = 0
       /\ NoSlot \notin Slots

Phases == {"absent", "proposed", "deferred", "rendering", "prepared",
           "submitted", "settled"}
Outcomes == {"none", "committed", "failed", "superseded"}
TerminalOutcomes == Outcomes \ {"none"}

VARIABLES
    phase,
    outcome,
    submitted,
    flipped,
    lost,
    inFlight,
    committed,
    slotOwner,
    slotIncarnation,
    tokenSlot,
    tokenIncarnation,
    released,
    deferredCount,
    staleReleaseObserved

vars == <<
    phase,
    outcome,
    submitted,
    flipped,
    lost,
    inFlight,
    committed,
    slotOwner,
    slotIncarnation,
    tokenSlot,
    tokenIncarnation,
    released,
    deferredCount,
    staleReleaseObserved
>>

FreeSlots == {slot \in Slots : slotOwner[slot] = NoGeneration}
OwnsSlot(g) ==
    /\ tokenSlot[g] \in Slots
    /\ g \notin released
    /\ slotOwner[tokenSlot[g]] = g
    /\ slotIncarnation[tokenSlot[g]] = tokenIncarnation[g]

Active(g) ==
    \/ phase[g] \in {"rendering", "prepared", "submitted"}
    \/ inFlight[g] # {}
    \/ committed = g

Init ==
    /\ phase = [g \in Generations |-> "absent"]
    /\ outcome = [g \in Generations |-> "none"]
    /\ submitted = [g \in Generations |-> {}]
    /\ flipped = [g \in Generations |-> {}]
    /\ lost = [g \in Generations |-> {}]
    /\ inFlight = [g \in Generations |-> {}]
    /\ committed = NoGeneration
    /\ slotOwner = [slot \in Slots |-> NoGeneration]
    /\ slotIncarnation = [slot \in Slots |-> 0]
    /\ tokenSlot = [g \in Generations |-> NoSlot]
    /\ tokenIncarnation = [g \in Generations |-> 0]
    /\ released = {}
    /\ deferredCount = 0
    /\ staleReleaseObserved = {}

Propose(g) ==
    /\ phase[g] = "absent"
    /\ g > committed
    /\ \A older \in Generations : older < g => phase[older] # "absent"
    /\ phase' = [phase EXCEPT ![g] = "proposed"]
    /\ UNCHANGED <<
        outcome, submitted, flipped, lost, inFlight, committed, slotOwner,
        slotIncarnation, tokenSlot, tokenIncarnation, released, deferredCount,
        staleReleaseObserved
        >>

(***************************************************************************
 * Allocation increments the slot incarnation before any rendering begins. *
 * A freed slot may be reused, but the old generation's token stays in the   *
 * history so DeliverStaleRelease can exercise the ABA-shaped return.        *
 *************************************************************************)
AcquireSlot(g, slot) ==
    /\ phase[g] \in {"proposed", "deferred"}
    /\ outcome[g] = "none"
    /\ slot \in FreeSlots
    /\ slotIncarnation[slot] < Cardinality(Generations)
    /\ phase' = [phase EXCEPT ![g] = "rendering"]
    /\ slotOwner' = [slotOwner EXCEPT ![slot] = g]
    /\ slotIncarnation' = [slotIncarnation EXCEPT ![slot] = @ + 1]
    /\ tokenSlot' = [tokenSlot EXCEPT ![g] = slot]
    /\ tokenIncarnation' = [tokenIncarnation EXCEPT
        ![g] = slotIncarnation[slot] + 1]
    /\ UNCHANGED <<
        outcome, submitted, flipped, lost, inFlight, committed, released,
        deferredCount, staleReleaseObserved
        >>

DeferAtCapacity(g) ==
    /\ phase[g] = "proposed"
    /\ FreeSlots = {}
    /\ phase' = [phase EXCEPT ![g] = "deferred"]
    /\ deferredCount' = deferredCount + 1
    /\ UNCHANGED <<
        outcome, submitted, flipped, lost, inFlight, committed, slotOwner,
        slotIncarnation, tokenSlot, tokenIncarnation, released,
        staleReleaseObserved
        >>

FinishRender(g) ==
    /\ phase[g] = "rendering"
    /\ OwnsSlot(g)
    /\ phase' = [phase EXCEPT ![g] = "prepared"]
    /\ UNCHANGED <<
        outcome, submitted, flipped, lost, inFlight, committed, slotOwner,
        slotIncarnation, tokenSlot, tokenIncarnation, released, deferredCount,
        staleReleaseObserved
        >>

(***************************************************************************
 * The focused model submits the complete mirror cohort in one action, then  *
 * lets its two physical callbacks arrive independently. VisualRetirement    *
 * separately checks partial head submission.                                *
 *************************************************************************)
Submit(g) ==
    /\ phase[g] = "prepared"
    /\ outcome[g] = "none"
    /\ inFlight[g] = {}
    /\ \A other \in Generations \ {g} : inFlight[other] = {}
    /\ g > committed
    /\ phase' = [phase EXCEPT ![g] = "submitted"]
    /\ submitted' = [submitted EXCEPT ![g] = Heads]
    /\ inFlight' = [inFlight EXCEPT ![g] = Heads]
    /\ UNCHANGED <<
        outcome, flipped, lost, committed, slotOwner, slotIncarnation,
        tokenSlot, tokenIncarnation, released, deferredCount,
        staleReleaseObserved
        >>

FlipHead(g, head) ==
    /\ head \in inFlight[g]
    /\ flipped' = [flipped EXCEPT ![g] = @ \cup {head}]
    /\ inFlight' = [inFlight EXCEPT ![g] = @ \ {head}]
    /\ UNCHANGED <<
        phase, outcome, submitted, lost, committed, slotOwner,
        slotIncarnation, tokenSlot, tokenIncarnation, released, deferredCount,
        staleReleaseObserved
        >>

LoseHead(g, head) ==
    /\ head \in inFlight[g]
    /\ lost' = [lost EXCEPT ![g] = @ \cup {head}]
    /\ inFlight' = [inFlight EXCEPT ![g] = @ \ {head}]
    /\ phase' = [phase EXCEPT ![g] = "settled"]
    /\ outcome' = [outcome EXCEPT ![g] = "failed"]
    /\ UNCHANGED <<
        submitted, flipped, committed, slotOwner, slotIncarnation, tokenSlot,
        tokenIncarnation, released, deferredCount, staleReleaseObserved
        >>

Retire(g) ==
    /\ phase[g] = "submitted"
    /\ outcome[g] = "none"
    /\ flipped[g] = Heads
    /\ phase' = [phase EXCEPT ![g] = "settled"]
    /\ outcome' = [outcome EXCEPT ![g] = "committed"]
    /\ committed' = g
    /\ UNCHANGED <<
        submitted, flipped, lost, inFlight, slotOwner, slotIncarnation,
        tokenSlot, tokenIncarnation, released, deferredCount,
        staleReleaseObserved
        >>

FailBeforeSubmit(g) ==
    /\ phase[g] \in {"rendering", "prepared"}
    /\ outcome[g] = "none"
    /\ phase' = [phase EXCEPT ![g] = "settled"]
    /\ outcome' = [outcome EXCEPT ![g] = "failed"]
    /\ UNCHANGED <<
        submitted, flipped, lost, inFlight, committed, slotOwner,
        slotIncarnation, tokenSlot, tokenIncarnation, released, deferredCount,
        staleReleaseObserved
        >>

Supersede(g) ==
    /\ phase[g] \in {"proposed", "deferred", "rendering", "prepared"}
    /\ outcome[g] = "none"
    /\ \E newer \in Generations : newer > g /\ phase[newer] # "absent"
    /\ phase' = [phase EXCEPT ![g] = "settled"]
    /\ outcome' = [outcome EXCEPT ![g] = "superseded"]
    /\ UNCHANGED <<
        submitted, flipped, lost, inFlight, committed, slotOwner,
        slotIncarnation, tokenSlot, tokenIncarnation, released, deferredCount,
        staleReleaseObserved
        >>

(***************************************************************************
 * Failure is not release. An exact token becomes reusable only after all    *
 * accepted heads drain and a newer generation has replaced any displayed   *
 * owner.                                                                    *
 *************************************************************************)
ReleaseSlot(g) ==
    LET slot == tokenSlot[g]
    IN
    /\ outcome[g] \in TerminalOutcomes
    /\ g \notin released
    /\ inFlight[g] = {}
    /\ committed # g
    /\ OwnsSlot(g)
    /\ slotOwner' = [slotOwner EXCEPT ![slot] = NoGeneration]
    /\ released' = released \cup {g}
    /\ UNCHANGED <<
        phase, outcome, submitted, flipped, lost, inFlight, committed,
        slotIncarnation, tokenSlot, tokenIncarnation, deferredCount,
        staleReleaseObserved
        >>

(***************************************************************************
 * A duplicate old return after reuse is observable but cannot mutate the    *
 * current slot. Removing the UNCHANGED slotOwner conjunct immediately       *
 * violates ActiveGenerationOwnsSlot in the ABA state.                       *
 *************************************************************************)
DeliverStaleRelease(g) ==
    LET slot == tokenSlot[g]
    IN
    /\ g \in released
    /\ slot \in Slots
    /\ slotOwner[slot] # NoGeneration
    /\ slotOwner[slot] # g
    /\ tokenIncarnation[g] < slotIncarnation[slot]
    /\ g \notin staleReleaseObserved
    /\ staleReleaseObserved' = staleReleaseObserved \cup {g}
    /\ UNCHANGED <<
        phase, outcome, submitted, flipped, lost, inFlight, committed,
        slotOwner, slotIncarnation, tokenSlot, tokenIncarnation, released,
        deferredCount
        >>

Progress(g) ==
    \/ \E slot \in Slots : AcquireSlot(g, slot)
    \/ DeferAtCapacity(g)
    \/ FinishRender(g)
    \/ Submit(g)
    \/ \E head \in Heads : FlipHead(g, head)
    \/ Retire(g)
    \/ Supersede(g)
    \/ ReleaseSlot(g)

Next ==
    \/ \E g \in Generations : Propose(g)
    \/ \E g \in Generations : Progress(g)
    \/ \E g \in Generations : FailBeforeSubmit(g)
    \/ \E g \in Generations : DeliverStaleRelease(g)

Spec == Init /\ [][Next]_vars

FairSpec == Spec /\ \A g \in Generations :
    /\ WF_vars(\E slot \in Slots : AcquireSlot(g, slot))
    /\ WF_vars(DeferAtCapacity(g))
    /\ WF_vars(FinishRender(g))
    /\ WF_vars(Submit(g))
    /\ WF_vars(\E head \in Heads : FlipHead(g, head))
    /\ WF_vars(Retire(g))
    /\ WF_vars(Supersede(g))
    /\ WF_vars(ReleaseSlot(g))

TypeOK ==
    /\ phase \in [Generations -> Phases]
    /\ outcome \in [Generations -> Outcomes]
    /\ submitted \in [Generations -> SUBSET Heads]
    /\ flipped \in [Generations -> SUBSET Heads]
    /\ lost \in [Generations -> SUBSET Heads]
    /\ inFlight \in [Generations -> SUBSET Heads]
    /\ committed \in Generations \cup {NoGeneration}
    /\ slotOwner \in [Slots -> Generations \cup {NoGeneration}]
    /\ slotIncarnation \in [Slots -> 0..Cardinality(Generations)]
    /\ tokenSlot \in [Generations -> Slots \cup {NoSlot}]
    /\ tokenIncarnation \in [Generations -> 0..Cardinality(Generations)]
    /\ released \subseteq Generations
    /\ deferredCount \in Nat
    /\ staleReleaseObserved \subseteq Generations

HeadAccounting ==
    \A g \in Generations :
        /\ flipped[g] \subseteq submitted[g]
        /\ lost[g] \subseteq submitted[g]
        /\ flipped[g] \cap lost[g] = {}
        /\ inFlight[g] = submitted[g] \ (flipped[g] \cup lost[g])

ExclusiveMirrorCohortInFlight ==
    \A g1, g2 \in Generations :
        g1 # g2 => inFlight[g1] = {} \/ inFlight[g2] = {}

SlotOwnersAreUnique ==
    \A g1, g2 \in Generations :
        g1 # g2 /\ OwnsSlot(g1) /\ OwnsSlot(g2) => tokenSlot[g1] # tokenSlot[g2]

ActiveGenerationOwnsSlot ==
    \A g \in Generations : Active(g) => OwnsSlot(g)

SlotTokenMatchesOwner ==
    \A slot \in Slots :
        slotOwner[slot] # NoGeneration =>
            /\ tokenSlot[slotOwner[slot]] = slot
            /\ tokenIncarnation[slotOwner[slot]] = slotIncarnation[slot]
            /\ slotOwner[slot] \notin released

DeferredOwnsNoSlot ==
    \A g \in Generations : phase[g] = "deferred" => tokenSlot[g] = NoSlot

CapacityIsBounded ==
    Cardinality({slot \in Slots : slotOwner[slot] # NoGeneration}) <= 3

CommittedGenerationRetainsSlot ==
    committed # NoGeneration => OwnsSlot(committed)

MirrorCommitsAfterEveryHead ==
    \A g \in Generations : outcome[g] = "committed" => flipped[g] = Heads

FailureDoesNotReleaseInFlight ==
    \A g \in Generations : inFlight[g] # {} => g \notin released

ReleasedTokenIsNotCurrent ==
    \A g \in released :
        /\ outcome[g] \in TerminalOutcomes
        /\ inFlight[g] = {}
        /\ committed # g
        /\ slotOwner[tokenSlot[g]] # g

AcquiredEventuallySettles ==
    \A g \in Generations : OwnsSlot(g) ~> outcome[g] \in TerminalOutcomes

DeferredEventuallyLeavesCapacityWait ==
    \A g \in Generations : phase[g] = "deferred" ~> phase[g] # "deferred"

=============================================================================
