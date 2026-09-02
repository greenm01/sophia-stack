----------------- MODULE ContinuousContentPresentation -----------------
EXTENDS Integers, FiniteSets

(***************************************************************************
 * Continuous post-readiness CPU content progress for one primary head.     *
 *                                                                          *
 * This model begins at the readiness baseline established by the live       *
 * session owner. A client update is latest-wins only while it remains in     *
 * the unbound tracker cell. Composition transfers that identity to an exact *
 * native frame owner; later intake cannot supersede that queued identity.    *
 * Native displacement, submission, flip, and exact callback reduction are   *
 * separate owner steps.                                                     *
 *                                                                          *
 * Generations abstract exact frame-plus-checksum identities. Timing stays    *
 * empirical in the physical reporter; this model owns settlement identity   *
 * and eventual progress under productive-action fairness assumptions.       *
 *************************************************************************)

CONSTANT MaxUpdate

ASSUME MaxUpdate \in Nat \ {0}

Updates == 1..MaxUpdate
NoGeneration == 0

VARIABLES
    latest,
    accepted,
    unbound,
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
    unbound,
    composed,
    inFlight,
    displayed,
    callbacks,
    retired,
    presented,
    superseded
>>

Pending == accepted \ (presented \cup superseded)

PendingOwners ==
    ({unbound, composed, inFlight} \ {NoGeneration}) \cup callbacks

NativeOwners ==
    ({composed, inFlight} \ {NoGeneration}) \cup callbacks

Init ==
    /\ latest = NoGeneration
    /\ accepted = {}
    /\ unbound = NoGeneration
    /\ composed = NoGeneration
    /\ inFlight = NoGeneration
    /\ displayed = NoGeneration
    /\ callbacks = {}
    /\ retired = {}
    /\ presented = {}
    /\ superseded = {}

(***************************************************************************
 * One or more CPU changes enter one production turn. The implementation     *
 * accounts all but the newest same-turn update before this abstract action. *
 * A later turn may replace only the still-unbound identity. Exact frames in  *
 * composed, inFlight, or callbacks remain native-owned.                     *
 *************************************************************************)
AcceptUpdate ==
    /\ latest < MaxUpdate
    /\ LET next == latest + 1
       IN /\ latest' = next
          /\ accepted' = accepted \cup {next}
          /\ unbound' = next
          /\ superseded' =
                 IF unbound # NoGeneration
                 THEN superseded \cup {unbound}
                 ELSE superseded
    /\ UNCHANGED << composed, inFlight, displayed, callbacks, retired,
                    presented >>

(***************************************************************************
 * Negative control: overwrite the unbound cell without settling its prior    *
 * identity. The accounting invariant must reject the ownerless generation.   *
 *************************************************************************)
AcceptUpdateWithoutSupersession ==
    /\ latest < MaxUpdate
    /\ LET next == latest + 1
       IN /\ latest' = next
          /\ accepted' = accepted \cup {next}
          /\ unbound' = next
    /\ UNCHANGED << composed, inFlight, displayed, callbacks, retired,
                    presented, superseded >>

(***************************************************************************
 * Composition transfers the exact newest unbound identity into the native    *
 * queued owner. A still-composed predecessor may be explicitly displaced;   *
 * that displacement, not unrelated later intake, settles it as superseded.   *
 *************************************************************************)
ComposeLatest ==
    /\ unbound \in Pending
    /\ composed' = unbound
    /\ unbound' = NoGeneration
    /\ superseded' =
           IF composed # NoGeneration
           THEN superseded \cup {composed}
           ELSE superseded
    /\ UNCHANGED << latest, accepted, inFlight, displayed, callbacks,
                    retired, presented >>

(***************************************************************************
 * Native submission transfers the exact composed identity to the primary    *
 * in-flight owner. Submitted work may still retire after newer intake.       *
 *************************************************************************)
SubmitComposition ==
    /\ inFlight = NoGeneration
    /\ composed \in Pending
    /\ inFlight' = composed
    /\ composed' = NoGeneration
    /\ UNCHANGED << latest, accepted, unbound, displayed, callbacks,
                    retired, presented, superseded >>

(***************************************************************************
 * The kernel flip and owner reduction are separate asynchronous stages. The  *
 * flip changes what the primary shows and queues the exact generation for    *
 * callback reduction.                                                       *
 *************************************************************************)
KernelFlip ==
    /\ inFlight # NoGeneration
    /\ displayed' = inFlight
    /\ retired' = retired \cup {inFlight}
    /\ callbacks' = callbacks \cup {inFlight}
    /\ inFlight' = NoGeneration
    /\ UNCHANGED << latest, accepted, unbound, composed, presented,
                    superseded >>

(***************************************************************************
 * Exact callback reduction settles only its matching queued identity.        *
 *************************************************************************)
ReduceRetirement(g) ==
    /\ g \in callbacks
    /\ callbacks' = callbacks \ {g}
    /\ presented' = IF g \in Pending THEN presented \cup {g} ELSE presented
    /\ UNCHANGED << latest, accepted, unbound, composed, inFlight, displayed,
                    retired, superseded >>

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
    /\ UNCHANGED << latest, accepted, unbound, composed, inFlight, displayed,
                    retired, superseded >>

(***************************************************************************
 * Negative control for the physical-gate regression: accepting a successor  *
 * settles every pending identity, including frames still held by a native    *
 * owner. NativeOwnersAreNotSuperseded must reject that ownership split.      *
 *************************************************************************)
AcceptUpdateSupersedesNativeOwners ==
    /\ latest < MaxUpdate
    /\ NativeOwners # {}
    /\ LET next == latest + 1
       IN /\ latest' = next
          /\ accepted' = accepted \cup {next}
          /\ unbound' = next
          /\ superseded' = superseded \cup Pending
    /\ UNCHANGED << composed, inFlight, displayed, callbacks, retired,
                    presented >>

Spec == Init /\ [][Next]_vars

(***************************************************************************
 * Each independent productive stage is weakly fair. Callback fairness is     *
 * stated per exact identity rather than over a disjunction that a different  *
 * callback could satisfy forever.                                           *
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
    /\ unbound \in 0..MaxUpdate
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
 * these. tools/check_tla.sh requires each focused configuration to find its  *
 * named violation.                                                          *
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

NativeOwnerSupersessionNext ==
    \/ AcceptUpdate
    \/ AcceptUpdateSupersedesNativeOwners
    \/ ComposeLatest
    \/ SubmitComposition
    \/ KernelFlip
    \/ \E g \in Updates : ReduceRetirement(g)

NativeOwnerSupersessionSpec ==
    Init /\ [][NativeOwnerSupersessionNext]_vars

SettlementsAreDisjoint ==
    /\ presented \cap superseded = {}
    /\ presented \cap Pending = {}
    /\ superseded \cap Pending = {}

(***************************************************************************
 * Every accepted update is settled or held by exactly one abstract pipeline  *
 * owner class. Unlike the former latest-only rule, multiple exact native     *
 * owners may coexist while queued frames advance toward retirement.          *
 *************************************************************************)
AllAcceptedUpdatesAccounted ==
    /\ accepted = presented \cup superseded \cup Pending
    /\ Pending = PendingOwners

(***************************************************************************
 * Intake cannot revoke a frame that composition, submission, or callback     *
 * reduction still owns. Only an action that also removes that native owner    *
 * may settle the frame as superseded.                                        *
 *************************************************************************)
NativeOwnersAreNotSuperseded ==
    NativeOwners \cap superseded = {}

(***************************************************************************
 * Retirement identity, not merely a later callback, licenses presentation.   *
 *************************************************************************)
PresentedUpdatesRetired == presented \subseteq retired

PipelineGenerationsWereAccepted ==
    /\ (unbound # NoGeneration => unbound \in accepted)
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
