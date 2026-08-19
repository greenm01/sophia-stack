------------------------- MODULE MirrorHeadPacing -------------------------
EXTENDS Integers

(***************************************************************************
 * Heads of one mirror group advancing at their own refresh rates.          *
 *                                                                          *
 * `VisualRetirement.tla` states today's rule: a logical output retires when *
 * every one of its heads has flipped. That makes a group advance at its     *
 * slowest screen, which is wrong on ordinary hardware -- 144Hz beside 60Hz  *
 * is a normal desk, and holding the fast panel to 60 to keep a mirror       *
 * coherent throttles the client's frame callbacks too.                      *
 *                                                                          *
 * This module states the successor rule instead: each head takes the newest *
 * generation it has not shown, skipping whatever it missed, and the group's  *
 * primary head owns present feedback. Screens may therefore disagree for a   *
 * while. What may never happen is the thing that disagreement makes          *
 * possible: freeing pixels a slower screen is still reading.                 *
 *************************************************************************)

CONSTANTS MaxGen

ASSUME MaxGen \in Nat /\ MaxGen > 0

Heads == {"primary", "secondary"}
Gens == 0..MaxGen
NoFlip == -1

VARIABLES produced, displayed, inflight, released, committed

vars == <<produced, displayed, inflight, released, committed>>

Init ==
    /\ produced = 0
    /\ displayed = [head \in Heads |-> 0]
    /\ inflight = [head \in Heads |-> NoFlip]
    /\ released = {}
    /\ committed = 0

(***************************************************************************
 * The compositor produces generations without asking any screen first.     *
 * It is unfair on purpose: nothing obliges a client to keep drawing.       *
 *************************************************************************)
Produce ==
    /\ produced < MaxGen
    /\ produced' = produced + 1
    /\ UNCHANGED <<displayed, inflight, released, committed>>

(***************************************************************************
 * A head takes the newest generation it has not shown. Coalescing is the    *
 * whole point: a screen that fell behind rejoins at the front rather than   *
 * replaying history it can no longer display in time.                       *
 *                                                                           *
 * The guard names only this head's state. Stated that way the enabledness   *
 * invariant below is provably true, which is what makes a future joint-     *
 * advance conjunct impossible rather than merely discouraged.               *
 *************************************************************************)
Submit(head) ==
    /\ inflight[head] = NoFlip
    /\ produced > displayed[head]
    /\ inflight' = [inflight EXCEPT ![head] = produced]
    /\ UNCHANGED <<produced, displayed, released, committed>>

(***************************************************************************
 * Feedback is the primary head's own screen state. A secondary that has not *
 * caught up does not delay the client's completion.                         *
 *************************************************************************)
Flip(head) ==
    /\ inflight[head] # NoFlip
    /\ displayed' = [displayed EXCEPT ![head] = inflight[head]]
    /\ committed' = IF head = "primary" THEN inflight[head] ELSE committed
    /\ inflight' = [inflight EXCEPT ![head] = NoFlip]
    /\ UNCHANGED <<produced, released>>

(***************************************************************************
 * The rule that independent pacing makes load-bearing. Under joint          *
 * retirement every head left a generation at the same moment, so "nothing   *
 * is reading it" followed from the group having moved on. It no longer      *
 * does: the newest generation also stays, because it is what a head that    *
 * has not caught up will take next.                                         *
 *************************************************************************)
Release(gen) ==
    /\ gen \in Gens
    /\ gen \notin released
    /\ gen < produced
    /\ \A head \in Heads: displayed[head] # gen /\ inflight[head] # gen
    /\ released' = released \cup {gen}
    /\ UNCHANGED <<produced, displayed, inflight, committed>>

HeadProgress(head) == Submit(head) \/ Flip(head)

Next ==
    \/ Produce
    \/ \E head \in Heads: HeadProgress(head)
    \/ \E gen \in Gens: Release(gen)

Spec == Init /\ [][Next]_vars
FairSpec == Spec /\ \A head \in Heads: WF_vars(HeadProgress(head))

TypeOK ==
    /\ produced \in Gens
    /\ displayed \in [Heads -> Gens]
    /\ inflight \in [Heads -> Gens \cup {NoFlip}]
    /\ released \subseteq Gens
    /\ committed \in Gens

(***************************************************************************
 * No screen ever reads freed pixels. This is the invariant the whole change *
 * is answerable to: with heads at different generations, "released" can no  *
 * longer mean "the group has moved on".                                     *
 *************************************************************************)
NoScannedGenerationIsReleased ==
    \A head \in Heads:
        /\ displayed[head] \notin released
        /\ inflight[head] \notin released

(***************************************************************************
 * Skipping ahead is allowed; going back is not. A screen that fell behind   *
 * must rejoin in front of what it already showed.                           *
 *************************************************************************)
HeadsNeverShowOlderWork ==
    \A head \in Heads: inflight[head] # NoFlip => inflight[head] > displayed[head]

(***************************************************************************
 * Feedback is one screen's state, not a quorum of screens.                  *
 *************************************************************************)
FeedbackIsItsOwnScreen == committed = displayed["primary"]

(***************************************************************************
 * The fast screen is never gated on the slow one. Like                      *
 * `EmittedEffectsAreExecutable` in FrameServiceArbitration, this is         *
 * provable from the guard as written -- which is exactly why it is worth    *
 * stating: adding a conjunct about another head's progress falsifies it.    *
 *************************************************************************)
PrimarySubmitNeverBlocked ==
    (inflight["primary"] = NoFlip /\ produced > displayed["primary"])
        => ENABLED Submit("primary")

(***************************************************************************
 * The lag a mirror is allowed is bounded by production, not permanent.      *
 * The gate's matching-content criterion rests on this.                      *
 *************************************************************************)
AllHeadsConverge ==
    (produced = MaxGen) ~> (\A head \in Heads: displayed[head] = MaxGen)

=============================================================================
