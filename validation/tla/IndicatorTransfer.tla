-------------------------- MODULE IndicatorTransfer --------------------------
EXTENDS Naturals, FiniteSets

(***************************************************************************
 * Status: proposed boundary, not an implemented one. Companion to          *
 * ShellObservation; both belong to docs/sophia-indicator-descriptor.md.    *
 *                                                                          *
 * Indicator records travel in the existing projection transfer, so they    *
 * inherit its discipline: the begin record declares every category count,  *
 * chunks carry exact ordinals, and the end record is accepted only after   *
 * counts and ordinals match. This model checks that indicators cannot      *
 * defeat that discipline, and that the permanent bounds hold on committed  *
 * state rather than only on well-behaved input.                            *
 *                                                                          *
 * Over-declaration fails at begin. Under-delivery, over-delivery, ordinal  *
 * gaps, repeats, timeouts, and per-output overflow all fail at or before   *
 * end. Every failure preserves the last committed descriptor whole; there  *
 * is no partial application and no silent truncation.                      *
 *************************************************************************)

CONSTANTS Outputs, MaxIndicators, MaxPerOutput, MaxDeclared, MaxCommits

ASSUME /\ Outputs # {}
       /\ IsFiniteSet(Outputs)
       /\ MaxIndicators \in (Nat \ {0})
       /\ MaxPerOutput \in (Nat \ {0})
       /\ MaxDeclared \in (Nat \ {0})
       /\ MaxDeclared >= MaxIndicators
       /\ MaxCommits \in (Nat \ {0})

VARIABLES
    phase,          \* "idle" | "begun" | "terminal"
    declared,       \* indicator_count declared by the begin record
    delivered,      \* indicator records accepted so far
    staged,         \* per-output counts assembled but not committed
    committed,      \* last committed per-output counts
    commitSerial,   \* increments only on an accepted transfer
    beginSerial,    \* commitSerial observed when this transfer began
    outcome

vars == <<phase, declared, delivered, staged, committed, commitSerial,
          beginSerial, outcome>>

ZeroCounts == [output \in Outputs |-> 0]

TotalOf(counts) ==
    LET Sum[remaining \in SUBSET Outputs] ==
            IF remaining = {} THEN 0
            ELSE LET pick == CHOOSE output \in remaining : TRUE
                 IN counts[pick] + Sum[remaining \ {pick}]
    IN Sum[Outputs]

WithinBounds(counts) ==
    /\ TotalOf(counts) <= MaxIndicators
    /\ \A output \in Outputs : counts[output] <= MaxPerOutput

Init ==
    /\ phase = "idle"
    /\ declared = 0
    /\ delivered = 0
    /\ staged = ZeroCounts
    /\ committed = ZeroCounts
    /\ commitSerial = 0
    /\ beginSerial = 0
    /\ outcome = "none"

(***************************************************************************
 * A begin record declaring more indicators than the permanent bound is     *
 * refused before any record is assembled. The bound is not a rendering     *
 * convenience; it is checked at admission.                                 *
 *************************************************************************)
BeginTransfer(count) ==
    /\ phase = "idle"
    /\ count \in 0..MaxDeclared
    /\ beginSerial' = commitSerial
    /\ declared' = count
    /\ delivered' = 0
    /\ staged' = ZeroCounts
    /\ IF count > MaxIndicators
          THEN /\ phase' = "terminal"
               /\ outcome' = "rejected"
          ELSE /\ phase' = "begun"
               /\ outcome' = "none"
    /\ UNCHANGED <<committed, commitSerial>>

DeliverRecord(ordinal, output) ==
    /\ phase = "begun"
    /\ delivered < declared
    /\ ordinal = delivered + 1
    /\ delivered' = delivered + 1
    /\ staged' = [staged EXCEPT ![output] = @ + 1]
    /\ UNCHANGED <<phase, declared, committed, commitSerial, beginSerial,
                   outcome>>

(***************************************************************************
 * Any ordinal that is not the exact successor kills the transfer. This      *
 * covers gaps, repeats, and reordering in one rule.                        *
 *************************************************************************)
DeliverOutOfOrder(ordinal) ==
    /\ phase = "begun"
    /\ ordinal # delivered + 1
    /\ phase' = "terminal"
    /\ outcome' = "rejected"
    /\ UNCHANGED <<declared, delivered, staged, committed, commitSerial,
                   beginSerial>>

DeliverExcess ==
    /\ phase = "begun"
    /\ delivered = declared
    /\ phase' = "terminal"
    /\ outcome' = "rejected"
    /\ UNCHANGED <<declared, delivered, staged, committed, commitSerial,
                   beginSerial>>

EndTransfer ==
    /\ phase = "begun"
    /\ phase' = "terminal"
    /\ IF /\ delivered = declared
          /\ WithinBounds(staged)
          /\ commitSerial < MaxCommits
          THEN /\ committed' = staged
               /\ commitSerial' = commitSerial + 1
               /\ outcome' = "committed"
          ELSE /\ UNCHANGED <<committed, commitSerial>>
               /\ outcome' = "rejected"
    /\ UNCHANGED <<declared, delivered, staged, beginSerial>>

TimeoutTransfer ==
    /\ phase = "begun"
    /\ phase' = "terminal"
    /\ outcome' = "rejected"
    /\ UNCHANGED <<declared, delivered, staged, committed, commitSerial,
                   beginSerial>>

ClearTerminal ==
    /\ phase = "terminal"
    /\ phase' = "idle"
    /\ declared' = 0
    /\ delivered' = 0
    /\ staged' = ZeroCounts
    /\ outcome' = "none"
    /\ UNCHANGED <<committed, commitSerial, beginSerial>>

Settle == EndTransfer \/ TimeoutTransfer \/ DeliverExcess

Next ==
    \/ \E count \in 0..MaxDeclared : BeginTransfer(count)
    \/ \E ordinal \in 1..(MaxDeclared + 1), output \in Outputs :
           DeliverRecord(ordinal, output)
    \/ \E ordinal \in 1..(MaxDeclared + 1) : DeliverOutOfOrder(ordinal)
    \/ Settle
    \/ ClearTerminal

Spec == Init /\ [][Next]_vars

FairSpec == Spec /\ WF_vars(Settle) /\ WF_vars(ClearTerminal)

TypeOK ==
    /\ phase \in {"idle", "begun", "terminal"}
    /\ declared \in 0..MaxDeclared
    /\ delivered \in 0..MaxDeclared
    /\ staged \in [Outputs -> 0..MaxDeclared]
    /\ committed \in [Outputs -> 0..MaxDeclared]
    /\ commitSerial \in 0..MaxCommits
    /\ beginSerial \in 0..MaxCommits
    /\ outcome \in {"none", "committed", "rejected"}

(***************************************************************************
 * The permanent bounds hold on committed state, not merely on input that   *
 * happened to respect them.                                                *
 *************************************************************************)
CommittedRespectsBounds == WithinBounds(committed)

(***************************************************************************
 * No partial application. A commit means every declared record arrived.    *
 *************************************************************************)
CommitImpliesExactDelivery ==
    outcome = "committed" => delivered = declared

(***************************************************************************
 * Every rejection preserves the last committed descriptor whole. Malformed *
 * input never mutates committed state, so a shell keeps showing the last   *
 * good status rather than a half-applied one.                              *
 *************************************************************************)
RejectionPreservesLastGood ==
    outcome = "rejected" => commitSerial = beginSerial

(***************************************************************************
 * A transfer in flight has not yet touched committed state.                *
 *************************************************************************)
InFlightHasNotCommitted ==
    phase = "begun" => commitSerial = beginSerial

TransferEventuallySettles == phase = "begun" ~> phase # "begun"

=============================================================================
