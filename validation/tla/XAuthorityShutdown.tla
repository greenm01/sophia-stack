------------------------ MODULE XAuthorityShutdown ------------------------
EXTENDS Naturals, FiniteSets

(***************************************************************************
 * Bounded X Authority shutdown and final CPU-update settlement.           *
 *                                                                         *
 * This Category B model keeps the independently scheduled boundaries in   *
 * x11_socket/connection/server.rs and live_session/owner_loop separate:   *
 * admission closure, producer-held ordered egress, Engine intake, frontend *
 * closure, composition, submission, retirement, and terminal completion.  *
 *                                                                         *
 * Producers retain their own current transaction. There is no relay-owned  *
 * collection: MaxProducers therefore bounds all ordered egress outside the *
 * Engine channel. StopAndDisconnect remains executable while a producer   *
 * waits, and normal completion remains unavailable until the exact latest  *
 * accepted update has retired or has been superseded.                     *
 *************************************************************************)

CONSTANTS MaxProducers, MaxUpdates

ASSUME MaxProducers \in Nat \ {0}
    /\ MaxUpdates \in Nat \ {0}
    /\ MaxUpdates > MaxProducers

Updates == 1..MaxUpdates
Phases == {"running", "quiescing", "cancelling", "done"}

VARIABLES phase, issued, held, delivered, accepted, superseded,
          composed, inFlight, presented, frontendOpen, commandResponsive,
          ownerDraining, cancelled

vars == <<phase, issued, held, delivered, accepted, superseded,
          composed, inFlight, presented, frontendOpen, commandResponsive,
          ownerDraining, cancelled>>

Pending == accepted \ (presented \cup superseded)

Init ==
    /\ phase = "running"
    /\ issued = 0
    /\ held = {}
    /\ delivered = 0
    /\ accepted = {}
    /\ superseded = {}
    /\ composed = 0
    /\ inFlight = 0
    /\ presented = {}
    /\ frontendOpen = TRUE
    /\ commandResponsive = TRUE
    /\ ownerDraining = TRUE
    /\ cancelled = FALSE

Produce ==
(** x11_socket/connection/server.rs: X11CoreTraceObserver producer callback. **)
    /\ phase = "running"
    /\ issued < MaxUpdates
    /\ Cardinality(held) < MaxProducers
    /\ issued' = issued + 1
    /\ held' = held \cup {issued + 1}
    /\ UNCHANGED <<phase, delivered, accepted, superseded, composed,
                    inFlight, presented, frontendOpen, commandResponsive,
                    ownerDraining, cancelled>>

DeliverExpected ==
(** server.rs: XAuthorityOrderedEgress::submit_blocking/try_submit. **)
    /\ ownerDraining
    /\ delivered + 1 \in held
    /\ delivered' = delivered + 1
    /\ held' = held \ {delivered + 1}
    /\ accepted' = accepted \cup {delivered + 1}
    /\ superseded' = superseded \cup Pending
    /\ UNCHANGED <<phase, issued, composed, inFlight, presented,
                    frontendOpen, commandResponsive, ownerDraining,
                    cancelled>>

StopAccepting ==
(** server.rs: XServerFrontendServiceCommand::StopAccepting branch. **)
    /\ phase = "running"
    /\ phase' = "quiescing"
    /\ UNCHANGED <<issued, held, delivered, accepted, superseded, composed,
                    inFlight, presented, frontendOpen, commandResponsive,
                    ownerDraining, cancelled>>

CloseDrainedFrontend ==
(** server.rs: routed frontend's drained return condition. **)
    /\ phase = "quiescing"
    /\ held = {}
    /\ frontendOpen
    /\ frontendOpen' = FALSE
    /\ commandResponsive' = FALSE
    /\ UNCHANGED <<phase, issued, held, delivered, accepted, superseded,
                    composed, inFlight, presented, ownerDraining, cancelled>>

(** live_session/owner_loop/authority_production.rs: latest-wins composition. **)
ComposeLatest ==
    /\ Pending # {}
    /\ LET latest == CHOOSE update \in Pending :
                         \A other \in Pending : other <= update
       IN composed' = latest
    /\ UNCHANGED <<phase, issued, held, delivered, accepted, superseded,
                    inFlight, presented, frontendOpen, commandResponsive,
                    ownerDraining, cancelled>>

SubmitLatest ==
(** live_session visual runtime: native submission service. **)
    /\ composed \in Pending
    /\ inFlight = 0
    /\ inFlight' = composed
    /\ UNCHANGED <<phase, issued, held, delivered, accepted, superseded,
                    composed, presented, frontendOpen, commandResponsive,
                    ownerDraining, cancelled>>

RetireExact ==
(** live_session/owner_loop/authority.rs: exact primary retirement observation. **)
    /\ inFlight # 0
    /\ presented' = IF inFlight \in Pending
                     THEN presented \cup {inFlight}
                     ELSE presented
    /\ inFlight' = 0
    /\ UNCHANGED <<phase, issued, held, delivered, accepted, superseded,
                    composed, frontendOpen, commandResponsive, ownerDraining,
                    cancelled>>

FinishNormal ==
(** owner_loop/physical_input_phase.rs: SessionQuiescenceDecision::Complete. **)
    /\ phase = "quiescing"
    /\ ~frontendOpen
    /\ held = {}
    /\ Pending = {}
    /\ inFlight = 0
    /\ phase' = "done"
    /\ ownerDraining' = FALSE
    /\ UNCHANGED <<issued, held, delivered, accepted, superseded, composed,
                    inFlight, presented, frontendOpen, commandResponsive,
                    cancelled>>

StopAndDisconnect ==
(** server.rs: StopAndDisconnect plus coordinator-wide cancellation. **)
    /\ phase \in {"running", "quiescing"}
    /\ commandResponsive
    /\ phase' = "cancelling"
    /\ held' = {}
    /\ frontendOpen' = FALSE
    /\ commandResponsive' = FALSE
    /\ cancelled' = TRUE
    /\ UNCHANGED <<issued, delivered, accepted, superseded, composed,
                    inFlight, presented, ownerDraining>>

FinishCancelled ==
    /\ phase = "cancelling"
    /\ phase' = "done"
    /\ ownerDraining' = FALSE
    /\ UNCHANGED <<issued, held, delivered, accepted, superseded, composed,
                    inFlight, presented, frontendOpen, commandResponsive,
                    cancelled>>

Next ==
    \/ Produce
    \/ DeliverExpected
    \/ StopAccepting
    \/ CloseDrainedFrontend
    \/ ComposeLatest
    \/ SubmitLatest
    \/ RetireExact
    \/ FinishNormal
    \/ StopAndDisconnect
    \/ FinishCancelled

Spec == Init /\ [][Next]_vars

(***************************************************************************
 * Negative controls. PrematureServiceExit is the physical failure: the     *
 * command loop disappears with producer-held work, so cancellation can no  *
 * longer release it. ProduceWithoutBound recreates the relay-owned growth. *
 *************************************************************************)
PrematureServiceExit ==
    /\ phase = "quiescing"
    /\ held # {}
    /\ frontendOpen
    /\ frontendOpen' = FALSE
    /\ commandResponsive' = FALSE
    /\ UNCHANGED <<phase, issued, held, delivered, accepted, superseded,
                    composed, inFlight, presented, ownerDraining, cancelled>>

ProduceWithoutBound ==
    /\ phase = "running"
    /\ issued < MaxUpdates
    /\ issued' = issued + 1
    /\ held' = held \cup {issued + 1}
    /\ UNCHANGED <<phase, delivered, accepted, superseded, composed,
                    inFlight, presented, frontendOpen, commandResponsive,
                    ownerDraining, cancelled>>

PrematureExitNext == Next \/ PrematureServiceExit
PrematureExitSpec == Init /\ [][PrematureExitNext]_vars

UnboundedIngressNext ==
    \/ ProduceWithoutBound
    \/ DeliverExpected
    \/ StopAccepting
    \/ CloseDrainedFrontend
    \/ ComposeLatest
    \/ SubmitLatest
    \/ RetireExact
    \/ FinishNormal
    \/ StopAndDisconnect
    \/ FinishCancelled

UnboundedIngressSpec == Init /\ [][UnboundedIngressNext]_vars

TypeOK ==
    /\ phase \in Phases
    /\ issued \in 0..MaxUpdates
    /\ held \subseteq Updates
    /\ delivered \in 0..MaxUpdates
    /\ accepted \subseteq Updates
    /\ superseded \subseteq Updates
    /\ composed \in 0..MaxUpdates
    /\ inFlight \in 0..MaxUpdates
    /\ presented \subseteq Updates
    /\ frontendOpen \in BOOLEAN
    /\ commandResponsive \in BOOLEAN
    /\ ownerDraining \in BOOLEAN
    /\ cancelled \in BOOLEAN

OrderedDelivery == delivered = Cardinality(accepted)
BoundedProducerOwnership == Cardinality(held) <= MaxProducers
SettlementsAreDisjoint == presented \intersect superseded = {}
AcceptedUpdatesAccounted == accepted = presented \cup superseded \cup Pending
NormalCompletionIsSettled == phase = "done" /\ ~cancelled => Pending = {}
NoUncancellableEgress == held # {} => commandResponsive

=============================================================================
