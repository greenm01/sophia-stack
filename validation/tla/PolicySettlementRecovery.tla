-------------------- MODULE PolicySettlementRecovery --------------------
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************
 * Public Hagia settlement, terminal-outcome retry, and supervised restart. *
 * A staged reducer successor is revalidated without promotion. Layout and   *
 * reducer serials advance together only when frontend settlement commits.   *
 *************************************************************************)

CONSTANTS MaxEpoch, MaxRequest

ASSUME /\ MaxEpoch \in (Nat \ {0})
       /\ MaxRequest \in (Nat \ {0})

VARIABLES phase, transportLive, epoch, request, nextRequest, staged,
          frontendPending, prepared, reducerSerial, layoutSerial,
          restartPending, abortReducer, abortLayout,
          ownerTerminal, ownerTerminalEpoch,
          workerTerminal, workerTerminalEpoch, delivered

vars == <<phase, transportLive, epoch, request, nextRequest, staged,
          frontendPending, prepared, reducerSerial, layoutSerial,
          restartPending, abortReducer, abortLayout,
          ownerTerminal, ownerTerminalEpoch,
          workerTerminal, workerTerminalEpoch, delivered>>

TerminalId(connectionEpoch, requestId) ==
    ((connectionEpoch - 1) * MaxRequest) + requestId

DeliveredItems == {delivered[index] : index \in 1..Len(delivered)}

Init ==
    /\ phase = "idle"
    /\ transportLive = TRUE
    /\ epoch = 1
    /\ request = 0
    /\ nextRequest = 1
    /\ staged = 0
    /\ frontendPending = FALSE
    /\ prepared = FALSE
    /\ reducerSerial = 0
    /\ layoutSerial = 0
    /\ restartPending = FALSE
    /\ abortReducer = 0
    /\ abortLayout = 0
    /\ ownerTerminal = 0
    /\ ownerTerminalEpoch = 0
    /\ workerTerminal = 0
    /\ workerTerminalEpoch = 0
    /\ delivered = <<>>

IssueRequest ==
    /\ transportLive
    /\ phase = "idle"
    /\ ownerTerminal = 0
    /\ workerTerminal = 0
    /\ nextRequest <= MaxRequest
    /\ request' = nextRequest
    /\ nextRequest' = nextRequest + 1
    /\ phase' = "requested"
    /\ UNCHANGED <<transportLive, epoch, staged, frontendPending, prepared,
                    reducerSerial, layoutSerial, restartPending,
                    abortReducer, abortLayout, ownerTerminal,
                    ownerTerminalEpoch, workerTerminal, workerTerminalEpoch,
                    delivered>>

StageProposal ==
    /\ transportLive
    /\ phase = "requested"
    /\ staged' = request
    /\ phase' = "staged"
    /\ UNCHANGED <<transportLive, epoch, request, nextRequest,
                    frontendPending, prepared, reducerSerial, layoutSerial,
                    restartPending, abortReducer, abortLayout, ownerTerminal,
                    ownerTerminalEpoch, workerTerminal, workerTerminalEpoch,
                    delivered>>

SubmitFrontendLayout ==
    /\ transportLive
    /\ phase = "staged"
    /\ frontendPending' = TRUE
    /\ phase' = "frontend"
    /\ UNCHANGED <<transportLive, epoch, request, nextRequest, staged,
                    prepared, reducerSerial, layoutSerial, restartPending,
                    abortReducer, abortLayout, ownerTerminal,
                    ownerTerminalEpoch, workerTerminal, workerTerminalEpoch,
                    delivered>>

\* Revalidation is non-mutating: premature reducer promotion would violate
\* LastGoodIsCoherent before the frontend layout commits.
PreparePublicLayoutCommit ==
    /\ transportLive
    /\ phase = "frontend"
    /\ frontendPending
    /\ staged = request
    /\ prepared' = TRUE
    /\ phase' = "prepared"
    /\ UNCHANGED <<transportLive, epoch, request, nextRequest, staged,
                    frontendPending, reducerSerial, layoutSerial,
                    restartPending, abortReducer, abortLayout, ownerTerminal,
                    ownerTerminalEpoch, workerTerminal, workerTerminalEpoch,
                    delivered>>

ApplyCommittedLayout ==
    /\ transportLive
    /\ phase = "prepared"
    /\ prepared
    /\ ownerTerminal = 0
    /\ reducerSerial' = reducerSerial + 1
    /\ layoutSerial' = layoutSerial + 1
    /\ frontendPending' = FALSE
    /\ prepared' = FALSE
    /\ staged' = 0
    /\ ownerTerminal' = TerminalId(epoch, request)
    /\ ownerTerminalEpoch' = epoch
    /\ phase' = "terminal"
    /\ UNCHANGED <<transportLive, epoch, request, nextRequest,
                    restartPending, abortReducer, abortLayout,
                    workerTerminal, workerTerminalEpoch, delivered>>

TransportLoss ==
    /\ transportLive
    /\ transportLive' = FALSE
    /\ restartPending' = TRUE
    /\ abortReducer' = reducerSerial
    /\ abortLayout' = layoutSerial
    /\ ownerTerminal' = 0
    /\ ownerTerminalEpoch' = 0
    /\ workerTerminal' = 0
    /\ workerTerminalEpoch' = 0
    /\ UNCHANGED <<phase, epoch, request, nextRequest, staged,
                    frontendPending, prepared, reducerSerial, layoutSerial,
                    delivered>>

AbortPendingSettlement ==
    /\ restartPending
    /\ phase \in {"requested", "staged", "frontend", "prepared"}
    /\ reducerSerial = abortReducer
    /\ layoutSerial = abortLayout
    /\ phase' = "idle"
    /\ request' = 0
    /\ staged' = 0
    /\ frontendPending' = FALSE
    /\ prepared' = FALSE
    /\ UNCHANGED <<transportLive, epoch, nextRequest, reducerSerial,
                    layoutSerial, restartPending, abortReducer, abortLayout,
                    ownerTerminal, ownerTerminalEpoch, workerTerminal,
                    workerTerminalEpoch, delivered>>

FlushDeferredTerminal ==
    /\ transportLive
    /\ ownerTerminal # 0
    /\ workerTerminal = 0
    /\ workerTerminal' = ownerTerminal
    /\ workerTerminalEpoch' = ownerTerminalEpoch
    /\ ownerTerminal' = 0
    /\ ownerTerminalEpoch' = 0
    /\ UNCHANGED <<phase, transportLive, epoch, request, nextRequest, staged,
                    frontendPending, prepared, reducerSerial, layoutSerial,
                    restartPending, abortReducer, abortLayout, delivered>>

DeliverTerminal ==
    /\ transportLive
    /\ workerTerminal # 0
    /\ workerTerminalEpoch = epoch
    /\ workerTerminal \notin DeliveredItems
    /\ delivered' = Append(delivered, workerTerminal)
    /\ workerTerminal' = 0
    /\ workerTerminalEpoch' = 0
    /\ UNCHANGED <<phase, transportLive, epoch, request, nextRequest, staged,
                    frontendPending, prepared, reducerSerial, layoutSerial,
                    restartPending, abortReducer, abortLayout, ownerTerminal,
                    ownerTerminalEpoch>>

FinishTerminal ==
    /\ phase = "terminal"
    /\ ownerTerminal = 0
    /\ phase' = "idle"
    /\ request' = 0
    /\ UNCHANGED <<transportLive, epoch, nextRequest, staged,
                    frontendPending, prepared, reducerSerial, layoutSerial,
                    restartPending, abortReducer, abortLayout, ownerTerminal,
                    ownerTerminalEpoch, workerTerminal, workerTerminalEpoch,
                    delivered>>

RestartTransport ==
    /\ restartPending
    /\ phase = "idle"
    /\ ownerTerminal = 0
    /\ workerTerminal = 0
    /\ epoch < MaxEpoch
    /\ epoch' = epoch + 1
    /\ transportLive' = TRUE
    /\ restartPending' = FALSE
    /\ nextRequest' = 1
    /\ UNCHANGED <<phase, request, staged, frontendPending, prepared,
                    reducerSerial, layoutSerial, abortReducer, abortLayout,
                    ownerTerminal, ownerTerminalEpoch, workerTerminal,
                    workerTerminalEpoch, delivered>>

Next ==
    \/ IssueRequest
    \/ StageProposal
    \/ SubmitFrontendLayout
    \/ PreparePublicLayoutCommit
    \/ ApplyCommittedLayout
    \/ TransportLoss
    \/ AbortPendingSettlement
    \/ FlushDeferredTerminal
    \/ DeliverTerminal
    \/ FinishTerminal
    \/ RestartTransport

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ phase \in {"idle", "requested", "staged", "frontend", "prepared", "terminal"}
    /\ transportLive \in BOOLEAN
    /\ epoch \in 1..MaxEpoch
    /\ request \in 0..MaxRequest
    /\ nextRequest \in 1..(MaxRequest + 1)
    /\ staged \in 0..MaxRequest
    /\ frontendPending \in BOOLEAN
    /\ prepared \in BOOLEAN
    /\ reducerSerial \in Nat
    /\ layoutSerial \in Nat
    /\ restartPending \in BOOLEAN
    /\ ownerTerminal \in 0..(MaxEpoch * MaxRequest)
    /\ workerTerminal \in 0..(MaxEpoch * MaxRequest)

LastGoodIsCoherent == reducerSerial = layoutSerial

FailedSettlementPreservesLayout ==
    restartPending => /\ reducerSerial = abortReducer
                      /\ layoutSerial = abortLayout

RestartHasNoOldOwner ==
    /\ (ownerTerminal # 0 => ownerTerminalEpoch = epoch)
    /\ (workerTerminal # 0 => workerTerminalEpoch = epoch)

TerminalAtMostOnce == Len(delivered) = Cardinality(DeliveredItems)

=============================================================================
