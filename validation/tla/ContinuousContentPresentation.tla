----------------- MODULE ContinuousContentPresentation -----------------
EXTENDS Integers, FiniteSets

(***************************************************************************
 * Continuous post-readiness CPU content progress for one primary head.     *
 *                                                                          *
 * This model begins at the readiness baseline established in               *
 * live_session/owner_loop/lifecycle.rs:1074-1090. Each later client update  *
 * is latest-wins, matching cpu_visual_progress.rs:55-84. Composition and    *
 * retirement remain separate owner steps, matching                         *
 * owner_loop/authority_production.rs:107-110,193-196 and                    *
 * cpu_visual_progress.rs:86-149.                                           *
 *                                                                          *
 * Generations abstract logical checksums. Timing stays empirical in the     *
 * physical reporter; this model owns settlement identity and eventual      *
 * progress under the same productive-action fairness assumptions.          *
 *************************************************************************)

CONSTANT MaxUpdate

ASSUME MaxUpdate \in Nat \ {0}

Updates == 1..MaxUpdate
NoGeneration == 0

VARIABLES
    latest,
    accepted,
    composed,
    inFlight,
    displayed,
    callbacks,
    retired,
    presented,
    superseded

vars == <<
    latest,
    accepted,
    composed,
    inFlight,
    displayed,
    callbacks,
    retired,
    presented,
    superseded
>>

Pending == accepted \ (presented \cup superseded)

Init ==
    /\ latest = NoGeneration
    /\ accepted = {}
    /\ composed = NoGeneration
    /\ inFlight = NoGeneration
    /\ displayed = NoGeneration
    /\ callbacks = {}
    /\ retired = {}
    /\ presented = {}
    /\ superseded = {}

(***************************************************************************
 * One or more CPU changes enter one production turn. The implementation     *
 * increments every accepted update, settles the old pending update and all  *
 * but the newest same-turn update as superseded, then retains one newest     *
 * pending identity (cpu_visual_progress.rs:55-84). One abstract action per   *
 * generation is sufficient because each action exposes the same interleave  *
 * between intake, composition, submission, and retirement.                  *
 *************************************************************************)
AcceptUpdate ==
    /\ latest < MaxUpdate
    /\ LET next == latest + 1
           previouslyPending == Pending
       IN /\ latest' = next
          /\ accepted' = accepted \cup {next}
          /\ superseded' = superseded \cup previouslyPending
    /\ UNCHANGED << composed, inFlight, displayed, callbacks, retired,
                    presented >>

(***************************************************************************
 * Negative control: omit settlement of the previously pending generation.   *
 * This is not an alternate implementation; the negative-control checker     *
 * substitutes it for AcceptUpdate and expects accounting to fail.           *
 *************************************************************************)
AcceptUpdateWithoutSupersession ==
    /\ latest < MaxUpdate
    /\ LET next == latest + 1
       IN /\ latest' = next
          /\ accepted' = accepted \cup {next}
    /\ UNCHANGED << composed, inFlight, displayed, callbacks, retired,
                    presented, superseded >>

(***************************************************************************
 * The owner composes the newest unsettled logical content. Deferred          *
 * composition is a real scheduling window, so it is a separate action from  *
 * intake and submission (authority_production.rs:114-171,193-196;            *
 * cpu_visual_progress.rs:86-101).                                            *
 *************************************************************************)
ComposeLatest ==
    /\ latest \in Pending
    /\ composed # latest
    /\ composed' = latest
    /\ UNCHANGED << latest, accepted, inFlight, displayed, callbacks,
                    retired, presented, superseded >>

(***************************************************************************
 * Native submission owns at most one primary request. A composition that    *
 * was superseded before submission is not eligible; already submitted work  *
 * may still retire and must not settle its successor.                        *
 *************************************************************************)
SubmitComposition ==
    /\ inFlight = NoGeneration
    /\ composed \in Pending
    /\ inFlight' = composed
    /\ UNCHANGED << latest, accepted, composed, displayed, callbacks,
                    retired, presented, superseded >>

(***************************************************************************
 * The kernel flip and the owner reduction are separate asynchronous stages. *
 * The flip changes what the primary head shows and queues the exact          *
 * generation for reduction (native scanout service observed by               *
 * cpu_visual_progress.rs:103-138).                                           *
 *************************************************************************)
KernelFlip ==
    /\ inFlight # NoGeneration
    /\ displayed' = inFlight
    /\ retired' = retired \cup {inFlight}
    /\ callbacks' = callbacks \cup {inFlight}
    /\ inFlight' = NoGeneration
    /\ UNCHANGED << latest, accepted, composed, presented, superseded >>

(***************************************************************************
 * Exact retirement settles an update as presented only when the callback's  *
 * generation is still the pending latest generation. A stale callback is    *
 * drained but cannot claim a newer update (cpu_visual_progress.rs:139-148).  *
 *************************************************************************)
ReduceRetirement(g) ==
    /\ g \in callbacks
    /\ callbacks' = callbacks \ {g}
    /\ presented' = IF g \in Pending THEN presented \cup {g} ELSE presented
    /\ UNCHANGED << latest, accepted, composed, inFlight, displayed, retired,
                    superseded >>

Next ==
    \/ AcceptUpdate
    \/ ComposeLatest
    \/ SubmitComposition
    \/ KernelFlip
    \/ \E g \in Updates : ReduceRetirement(g)
(***************************************************************************
 * Negative control: let any old callback settle the newest pending update.   *
 * PresentedUpdatesRetired must reject this stale-retirement substitution.    *
 *************************************************************************)
ReduceStaleAsLatest(g) ==
    /\ g \in callbacks
    /\ callbacks' = callbacks \ {g}
    /\ presented' = IF Pending # {} THEN presented \cup {latest} ELSE presented
    /\ UNCHANGED << latest, accepted, composed, inFlight, displayed, retired,
                    superseded >>


Spec == Init /\ [][Next]_vars

(***************************************************************************
 * Each independent productive stage is weakly fair. In particular, drain    *
 * fairness is stated per exact callback instead of over a disjunction that   *
 * a different callback could satisfy forever.                               *
 *************************************************************************)
FairSpec ==
    /\ Spec
    /\ WF_vars(AcceptUpdate)
    /\ WF_vars(ComposeLatest)
    /\ WF_vars(SubmitComposition)
    /\ WF_vars(KernelFlip)
    /\ \A g \in Updates : WF_vars(ReduceRetirement(g))

TypeOK ==
    /\ latest \in 0..MaxUpdate
    /\ accepted \subseteq Updates
    /\ composed \in 0..MaxUpdate
    /\ inFlight \in 0..MaxUpdate
    /\ displayed \in 0..MaxUpdate
    /\ callbacks \subseteq Updates
    /\ retired \subseteq Updates
    /\ presented \subseteq Updates
    /\ superseded \subseteq Updates

AcceptedPrefix == accepted = 1..latest

(***************************************************************************
 * Executable negative-control specifications. The normal checker never uses  *
 * these. tools/check_tla.sh runs their focused configurations and requires   *
 * TLC to find the named violation.                                           *
 *************************************************************************)
NoDrainFairnessSpec ==
    /\ Spec
    /\ WF_vars(AcceptUpdate)
    /\ WF_vars(ComposeLatest)
    /\ WF_vars(SubmitComposition)
    /\ WF_vars(KernelFlip)

NoCompositionFairnessSpec ==
    /\ Spec
    /\ WF_vars(AcceptUpdate)
    /\ WF_vars(SubmitComposition)
    /\ WF_vars(KernelFlip)
    /\ \A g \in Updates : WF_vars(ReduceRetirement(g))

UnaccountedSupersessionNext ==
    \/ AcceptUpdateWithoutSupersession
    \/ ComposeLatest
    \/ SubmitComposition
    \/ KernelFlip
    \/ \E g \in Updates : ReduceRetirement(g)

UnaccountedSupersessionSpec ==
    Init /\ [][UnaccountedSupersessionNext]_vars

StaleRetirementNext ==
    \/ AcceptUpdate
    \/ ComposeLatest
    \/ SubmitComposition
    \/ KernelFlip
    \/ \E g \in Updates : ReduceStaleAsLatest(g)

StaleRetirementSpec == Init /\ [][StaleRetirementNext]_vars

SettlementsAreDisjoint ==
    /\ presented \cap superseded = {}
    /\ presented \cap Pending = {}
    /\ superseded \cap Pending = {}

(***************************************************************************
 * This is the model form of accepted = presented + superseded + pending in   *
 * cpu_visual_progress.rs:151-183. Latest-wins permits at most one pending    *
 * update, and that update must be the newest accepted generation.            *
 *************************************************************************)
AllAcceptedUpdatesAccounted ==
    /\ accepted = presented \cup superseded \cup Pending
    /\ Cardinality(Pending) =< 1
    /\ Pending \subseteq {latest}

(***************************************************************************
 * Retirement identity, not merely a later callback, is what licenses a      *
 * presented settlement. This rejects a stale callback settling the current  *
 * pending update.                                                            *
 *************************************************************************)
PresentedUpdatesRetired == presented \subseteq retired

PipelineGenerationsWereAccepted ==
    /\ (composed # NoGeneration => composed \in accepted)
    /\ (inFlight # NoGeneration => inFlight \in accepted)
    /\ (displayed # NoGeneration => displayed \in retired)
    /\ callbacks \subseteq retired

(***************************************************************************
 * Once the bounded source has produced its final update, productive-stage    *
 * fairness requires every accepted update to become presented or superseded. *
 *************************************************************************)
ContinuousPresentation ==
    (latest = MaxUpdate) ~> (Pending = {})

=============================================================================
