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
 * accepted update has retired or has been superseded, including explicit  *
 * lifecycle supersession when its owning surface is removed.              *
 *************************************************************************)

CONSTANTS MaxProducers, MaxUpdates

ASSUME MaxProducers \in Nat \ {0}
    /\ MaxUpdates \in Nat \ {0}
    /\ MaxUpdates > MaxProducers

Updates == 1..MaxUpdates
Phases == {"running", "quiescing", "cancelling", "done"}

VARIABLES phase, issued, held, delivered, accepted, superseded,
          composed, inFlight, presented, frontendOpen, commandResponsive,
          ownerDraining, cancelled, surfaceLive

vars == <<phase, issued, held, delivered, accepted, superseded,
          composed, inFlight, presented, frontendOpen, commandResponsive,
          ownerDraining, cancelled, surfaceLive>>

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
    /\ surfaceLive = TRUE

Produce ==
(** x11_socket/connection/server.rs: X11CoreTraceObserver producer callback. **)
    /\ phase = "running"
    /\ issued < MaxUpdates
    /\ Cardinality(held) < MaxProducers
    /\ issued' = issued + 1
    /\ held' = held \cup {issued + 1}
    /\ UNCHANGED <<phase, delivered, accepted, superseded, composed,
                    inFlight, presented, frontendOpen, commandResponsive,
                    ownerDraining, cancelled, surfaceLive>>

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
                    cancelled, surfaceLive>>

StopAccepting ==
(** server.rs: XServerFrontendServiceCommand::StopAccepting branch. **)
    /\ phase = "running"
    /\ phase' = "quiescing"
    /\ UNCHANGED <<issued, held, delivered, accepted, superseded, composed,
                    inFlight, presented, frontendOpen, commandResponsive,
                    ownerDraining, cancelled, surfaceLive>>

CloseDrainedFrontend ==
(** server.rs: routed frontend drain; live_session/shutdown.rs classifies the
    resulting disconnect identically from blocking receive and opportunistic
    drain. **)
    /\ phase = "quiescing"
    /\ held = {}
    /\ frontendOpen
    /\ frontendOpen' = FALSE
    /\ commandResponsive' = FALSE
    /\ UNCHANGED <<phase, issued, held, delivered, accepted, superseded,
                    composed, inFlight, presented, ownerDraining, cancelled,
                    surfaceLive>>

(** live_session/owner_loop/authority_production.rs: latest-wins composition. **)
ComposeLatest ==
    /\ surfaceLive
    /\ Pending # {}
    /\ LET latest == CHOOSE update \in Pending :
                         \A other \in Pending : other <= update
       IN composed' = latest
    /\ UNCHANGED <<phase, issued, held, delivered, accepted, superseded,
                    inFlight, presented, frontendOpen, commandResponsive,
                    ownerDraining, cancelled, surfaceLive>>

(** live_session visual runtime: native submission service. **)
SubmitLatest ==
    /\ surfaceLive
    /\ composed \in Pending
    /\ inFlight = 0
    /\ inFlight' = composed
    /\ UNCHANGED <<phase, issued, held, delivered, accepted, superseded,
                    composed, presented, frontendOpen, commandResponsive,
                    ownerDraining, cancelled, surfaceLive>>

(** owner_loop/physical_input_phase.rs: exact primary retirement observation. **)
RetireExact ==
    /\ inFlight # 0
    /\ presented' = IF inFlight \in Pending
                     THEN presented \cup {inFlight}
                     ELSE presented
    /\ inFlight' = 0
    /\ UNCHANGED <<phase, issued, held, delivered, accepted, superseded,
                    composed, frontendOpen, commandResponsive, ownerDraining,
                    cancelled, surfaceLive>>

(** A committed surface removal lifecycle-supersedes its pending update. **)
RemoveSurface ==
    /\ phase = "quiescing"
    /\ held = {}
    /\ Pending # {}
    /\ surfaceLive
    /\ surfaceLive' = FALSE
    /\ superseded' = superseded \cup Pending
    /\ UNCHANGED <<phase, issued, held, delivered, accepted, composed,
                    inFlight, presented, frontendOpen, commandResponsive,
                    ownerDraining, cancelled>>
(** owner_loop/physical_input_phase.rs: SessionQuiescenceDecision::Complete. **)
FinishNormal ==
    /\ phase = "quiescing"
    /\ ~frontendOpen
    /\ held = {}
    /\ Pending = {}
    /\ inFlight = 0
    /\ phase' = "done"
    /\ ownerDraining' = FALSE
    /\ UNCHANGED <<issued, held, delivered, accepted, superseded, composed,
                    inFlight, presented, frontendOpen, commandResponsive,
                    cancelled, surfaceLive>>

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
                    inFlight, presented, ownerDraining, surfaceLive>>

FinishCancelled ==
    /\ phase = "cancelling"
    /\ phase' = "done"
    /\ ownerDraining' = FALSE
    /\ UNCHANGED <<issued, held, delivered, accepted, superseded, composed,
                    inFlight, presented, frontendOpen, commandResponsive,
                    cancelled, surfaceLive>>

Next ==
    \/ Produce
    \/ DeliverExpected
    \/ StopAccepting
    \/ CloseDrainedFrontend
    \/ ComposeLatest
    \/ SubmitLatest
    \/ RetireExact
    \/ RemoveSurface
    \/ FinishNormal
    \/ StopAndDisconnect
    \/ FinishCancelled

Spec == Init /\ [][Next]_vars

(***************************************************************************
 * Negative controls. PrematureServiceExit is the physical failure: the     *
 * command loop disappears with producer-held work, so cancellation can no  *
 * longer release it. ProduceWithoutBound recreates relay-owned growth.     *
 * RemoveWithoutSettlement proves removal cannot silently abandon pending  *
 * visible work.                                                           *
 *************************************************************************)
PrematureServiceExit ==
    /\ phase = "quiescing"
    /\ held # {}
    /\ frontendOpen
    /\ frontendOpen' = FALSE
    /\ commandResponsive' = FALSE
    /\ UNCHANGED <<phase, issued, held, delivered, accepted, superseded,
                    composed, inFlight, presented, ownerDraining, cancelled,
                    surfaceLive>>

ProduceWithoutBound ==
    /\ phase = "running"
    /\ issued < MaxUpdates
    /\ issued' = issued + 1
    /\ held' = held \cup {issued + 1}
    /\ UNCHANGED <<phase, delivered, accepted, superseded, composed,
                    inFlight, presented, frontendOpen, commandResponsive,
                    ownerDraining, cancelled, surfaceLive>>

PrematureExitNext == Next \/ PrematureServiceExit
PrematureExitSpec == Init /\ [][PrematureExitNext]_vars

RemoveWithoutSettlement ==
    /\ phase = "quiescing"
    /\ held = {}
    /\ Pending # {}
    /\ surfaceLive
    /\ surfaceLive' = FALSE
    /\ UNCHANGED <<phase, issued, held, delivered, accepted, superseded,
                    composed, inFlight, presented, frontendOpen,
                    commandResponsive, ownerDraining, cancelled>>

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
RemovalWithoutSettlementNext == Next \/ RemoveWithoutSettlement
RemovalWithoutSettlementSpec == Init /\ [][RemovalWithoutSettlementNext]_vars

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
    /\ surfaceLive \in BOOLEAN
OrderedDelivery == delivered = Cardinality(accepted)
BoundedProducerOwnership == Cardinality(held) <= MaxProducers
SettlementsAreDisjoint == presented \intersect superseded = {}
AcceptedUpdatesAccounted == accepted = presented \cup superseded \cup Pending
NormalCompletionIsSettled == phase = "done" /\ ~cancelled => Pending = {}
NoUncancellableEgress == held # {} => commandResponsive
PendingHasLiveOwner == Pending = {} \/ surfaceLive
=============================================================================
