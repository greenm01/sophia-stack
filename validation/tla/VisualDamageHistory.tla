---------------------- MODULE VisualDamageHistory ----------------------
EXTENDS Integers, FiniteSets

(***************************************************************************
 * Bounded buffer-age damage history for the promoted three-slot native     *
 * target pool. `VisualRetirementSlots` owns slot ownership, exact release, *
 * and ABA-shaped stale returns; this model owns what a reused slot already *
 * contains and what a repaint into it therefore owes.                      *
 *                                                                          *
 * Scenario correspondence and the implementation-only checks live in        *
 * validation/specula/buffer-age-damage-history-modeling-brief.md.           *
 *                                                                          *
 * The output is an abstract region partition and content is a generation    *
 * mark per region, never pixels. A slot repainted from history must end up  *
 * holding exactly what a full repaint would have produced; under-computed   *
 * accumulated damage leaves a stale region in a frame that is otherwise     *
 * presentable and self-consistent, which is the defect this model exists    *
 * to make unreachable.                                                      *
 *                                                                          *
 * One head, and no lease incarnation. A slot's buffer keeps its content      *
 * across release and reacquisition -- that persistence is what makes buffer  *
 * age worth anything -- so history dies with the bundle or with an           *
 * incomplete write, not with the lease. `VisualRetirementSlots` owns the     *
 * lease identity that rejects a stale release. Per-head history is the same  *
 * indexing property applied twice; the brief records both as testable.       *
 *************************************************************************)

CONSTANTS Generations, Slots, Regions, NoGeneration

ASSUME /\ Generations # {}
       /\ Generations \subseteq (Nat \ {0})
       /\ Slots # {}
       /\ Cardinality(Slots) = 3
       /\ Regions # {}
       /\ NoGeneration = 0

MaxGeneration == Cardinality(Generations)
GenerationMarks == Generations \cup {NoGeneration}

VARIABLES
    scene,
    sceneGeneration,
    damageOf,
    slotContent,
    slotGeneration,
    slotHistoryValid,
    partialWriteAdmitted

vars == <<
    scene,
    sceneGeneration,
    damageOf,
    slotContent,
    slotGeneration,
    slotHistoryValid,
    partialWriteAdmitted
>>

(***************************************************************************
 * The work a slot owes is the union of every generation's damage after the *
 * one its content was written for, including generations that were never   *
 * rendered because they deferred or were superseded. A slot never written  *
 * owes the whole output.                                                    *
 *************************************************************************)
AccumulatedDamage(s) ==
    UNION { damageOf[g] : g \in { h \in Generations :
                                    /\ h > slotGeneration[s]
                                    /\ h =< sceneGeneration } }

Divergence(s) == { r \in Regions : slotContent[s][r] # scene[r] }

UsableHistory(s) == slotHistoryValid[s]

Init ==
    /\ scene = [r \in Regions |-> NoGeneration]
    /\ sceneGeneration = NoGeneration
    /\ damageOf = [g \in Generations |-> {}]
    /\ slotContent = [s \in Slots |-> [r \in Regions |-> NoGeneration]]
    /\ slotGeneration = [s \in Slots |-> NoGeneration]
    /\ slotHistoryValid = [s \in Slots |-> FALSE]
    /\ partialWriteAdmitted = FALSE

(***************************************************************************
 * A new scene generation changes a nonempty set of regions. Whether it is   *
 * ever rendered is deliberately not modelled: its damage is owed either     *
 * way, which is what makes a superseded deferral different from a frame     *
 * that never happened.                                                      *
 *************************************************************************)
AdvanceScene ==
    /\ sceneGeneration < MaxGeneration
    /\ \E d \in (SUBSET Regions) \ {{}} :
        LET g == sceneGeneration + 1 IN
        /\ scene' = [r \in Regions |-> IF r \in d THEN g ELSE scene[r]]
        /\ damageOf' = [damageOf EXCEPT ![g] = d]
        /\ sceneGeneration' = g
    /\ UNCHANGED << slotContent, slotGeneration, slotHistoryValid,
                    partialWriteAdmitted >>

WriteFull(s) ==
    /\ sceneGeneration # NoGeneration
    /\ slotContent' = [slotContent EXCEPT ![s] = scene]
    /\ slotGeneration' = [slotGeneration EXCEPT ![s] = sceneGeneration]
    /\ slotHistoryValid' = [slotHistoryValid EXCEPT ![s] = TRUE]
    /\ UNCHANGED << scene, sceneGeneration, damageOf, partialWriteAdmitted >>

(***************************************************************************
 * The optimization. A partial write is admissible only against usable       *
 * history, and it paints the accumulated damage rather than the whole slot. *
 *************************************************************************)
WritePartial(s) ==
    /\ sceneGeneration # NoGeneration
    /\ UsableHistory(s)
    /\ LET d == AccumulatedDamage(s) IN
        /\ slotContent' = [slotContent EXCEPT ![s] =
                [r \in Regions |-> IF r \in d THEN scene[r] ELSE slotContent[s][r]]]
    /\ slotGeneration' = [slotGeneration EXCEPT ![s] = sceneGeneration]
    /\ slotHistoryValid' = [slotHistoryValid EXCEPT ![s] = TRUE]
    /\ partialWriteAdmitted' = TRUE
    /\ UNCHANGED << scene, sceneGeneration, damageOf >>

(***************************************************************************
 * An output size change or a retryable target failure rebuilds a free       *
 * slot's bundle. The slot number survives; its pixels do not.               *
 *************************************************************************)
RebuildBundle(s) ==
    /\ slotHistoryValid' = [slotHistoryValid EXCEPT ![s] = FALSE]
    /\ slotContent' = [slotContent EXCEPT ![s] = [r \in Regions |-> NoGeneration]]
    /\ slotGeneration' = [slotGeneration EXCEPT ![s] = NoGeneration]
    /\ UNCHANGED << scene, sceneGeneration, damageOf, partialWriteAdmitted >>

(***************************************************************************
 * A partial write that fails after painting some of its damage. The slot    *
 * holds neither the old content nor the new, so it must record no usable    *
 * history rather than claim the generation it was aiming at.                *
 *************************************************************************)
FailPartialWrite(s) ==
    /\ sceneGeneration # NoGeneration
    /\ UsableHistory(s)
    /\ \E d \in SUBSET AccumulatedDamage(s) :
        /\ d # AccumulatedDamage(s)
        /\ slotContent' = [slotContent EXCEPT ![s] =
                [r \in Regions |-> IF r \in d THEN scene[r] ELSE slotContent[s][r]]]
    /\ slotHistoryValid' = [slotHistoryValid EXCEPT ![s] = FALSE]
    /\ slotGeneration' = [slotGeneration EXCEPT ![s] = NoGeneration]
    /\ UNCHANGED << scene, sceneGeneration, damageOf, partialWriteAdmitted >>

Next ==
    \/ AdvanceScene
    \/ \E s \in Slots : WriteFull(s)
    \/ \E s \in Slots : WritePartial(s)
    \/ \E s \in Slots : RebuildBundle(s)
    \/ \E s \in Slots : FailPartialWrite(s)

Spec == Init /\ [][Next]_vars

FairSpec == Spec /\ WF_vars(AdvanceScene)
                 /\ \A s \in Slots : WF_vars(WriteFull(s))

TypeOK ==
    /\ scene \in [Regions -> GenerationMarks]
    /\ sceneGeneration \in 0..MaxGeneration
    /\ damageOf \in [Generations -> SUBSET Regions]
    /\ slotContent \in [Slots -> [Regions -> GenerationMarks]]
    /\ slotGeneration \in [Slots -> 0..MaxGeneration]
    /\ slotHistoryValid \in [Slots -> BOOLEAN]
    /\ partialWriteAdmitted \in BOOLEAN

(***************************************************************************
 * The property the optimization must not break: a slot brought up to the    *
 * current scene holds what a full repaint would have produced. A partial    *
 * write whose damage missed a region violates this and nothing else.        *
 *************************************************************************)
RepaintMatchesFullRepaint ==
    \A s \in Slots :
        (slotHistoryValid[s] /\ slotGeneration[s] = sceneGeneration)
            => slotContent[s] = scene

(***************************************************************************
 * The same defect stated over the damage rather than the result, so a       *
 * counterexample names the region that was owed instead of the frame that   *
 * was wrong.                                                                *
 *************************************************************************)
DamageCoversDivergence ==
    \A s \in Slots :
        UsableHistory(s) => Divergence(s) \subseteq AccumulatedDamage(s)

(***************************************************************************
 * A slot with no usable history claims no generation to repaint against, so *
 * the accumulated damage it owes is the whole output and its next write is  *
 * necessarily full. This is how "incomplete history forces a full repaint"  *
 * is enforced: by leaving nothing for a partial write to be relative to.    *
 *************************************************************************)
InvalidHistoryClaimsNoGeneration ==
    \A s \in Slots : ~slotHistoryValid[s] => slotGeneration[s] = NoGeneration

(***************************************************************************
 * The optimization has to be reachable, not merely safe. A model in which   *
 * no partial write can ever be admitted would satisfy every safety property *
 * above and describe a compositor that never saved any work.                *
 *************************************************************************)
PartialWriteIsReachable == ~partialWriteAdmitted

=============================================================================
