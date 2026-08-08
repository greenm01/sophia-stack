------------------------- MODULE TargetInputPacing -------------------------
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************
 * Continuous input has one replaceable value slot per seat/target. Ordered *
 * records carry capture and discrete boundaries. Completion and cancellation*
 * materialize the final value immediately before their boundary.            *
 *************************************************************************)

CONSTANTS Seats, Targets, MaxValue, MaxQueue, MaxBoundaries

ASSUME /\ Seats # {} /\ IsFiniteSet(Seats)
       /\ Targets # {} /\ IsFiniteSet(Targets)
       /\ MaxValue \in (Nat \ {0})
       /\ MaxQueue \in (Nat \ {0})
       /\ MaxBoundaries \in (Nat \ {0})

Pairs == Seats \X Targets
Records == {"begin", "discrete", "value", "complete", "cancel"}

VARIABLES active, pending, ordered, boundaries, finals, badFinal

vars == <<active, pending, ordered, boundaries, finals, badFinal>>

Init ==
    /\ active = [pair \in Pairs |-> FALSE]
    /\ pending = [pair \in Pairs |-> 0]
    /\ ordered = <<>>
    /\ boundaries = 0 /\ finals = 0 /\ badFinal = FALSE

Begin(pair) ==
    /\ ~active[pair] /\ Len(ordered) < MaxQueue /\ boundaries < MaxBoundaries
    /\ active' = [active EXCEPT ![pair] = TRUE]
    /\ pending' = [pending EXCEPT ![pair] = 0]
    /\ ordered' = Append(ordered, "begin")
    /\ boundaries' = boundaries + 1
    /\ UNCHANGED <<finals, badFinal>>

(***************************************************************************
 * Repeated motion replaces the same slot; it never grows a motion queue.   *
 *************************************************************************)
Update(pair, value) ==
    /\ active[pair] /\ value \in 1..MaxValue
    /\ pending' = [pending EXCEPT ![pair] = value]
    /\ UNCHANGED <<active, ordered, boundaries, finals, badFinal>>

Discrete ==
    /\ Len(ordered) < MaxQueue /\ boundaries < MaxBoundaries
    /\ ordered' = Append(ordered, "discrete")
    /\ boundaries' = boundaries + 1
    /\ UNCHANGED <<active, pending, finals, badFinal>>

Complete(pair) ==
    /\ active[pair] /\ pending[pair] # 0
    /\ Len(ordered) + 2 <= MaxQueue /\ boundaries < MaxBoundaries
    /\ ordered' = Append(Append(ordered, "value"), "complete")
    /\ active' = [active EXCEPT ![pair] = FALSE]
    /\ pending' = [pending EXCEPT ![pair] = 0]
    /\ boundaries' = boundaries + 1 /\ finals' = finals + 1
    /\ badFinal' = badFinal \/ pending[pair] = 0

Cancel(pair) ==
    /\ active[pair] /\ pending[pair] # 0
    /\ Len(ordered) + 2 <= MaxQueue /\ boundaries < MaxBoundaries
    /\ ordered' = Append(Append(ordered, "value"), "cancel")
    /\ active' = [active EXCEPT ![pair] = FALSE]
    /\ pending' = [pending EXCEPT ![pair] = 0]
    /\ boundaries' = boundaries + 1 /\ finals' = finals + 1
    /\ badFinal' = badFinal \/ pending[pair] = 0

Drain ==
    /\ Len(ordered) > 0
    /\ ordered' = Tail(ordered)
    /\ UNCHANGED <<active, pending, boundaries, finals, badFinal>>

Next ==
    \/ \E pair \in Pairs : Begin(pair)
    \/ \E pair \in Pairs, value \in 1..MaxValue : Update(pair, value)
    \/ Discrete
    \/ \E pair \in Pairs : Complete(pair) \/ Cancel(pair)
    \/ Drain

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ active \in [Pairs -> BOOLEAN]
    /\ pending \in [Pairs -> 0..MaxValue]
    /\ ordered \in Seq(Records) /\ Len(ordered) <= MaxQueue
    /\ boundaries \in 0..MaxBoundaries /\ finals \in 0..MaxBoundaries
    /\ badFinal \in BOOLEAN

OneReplaceableSlot == \A pair \in Pairs : pending[pair] \in 0..MaxValue
InactiveHasNoPendingValue == \A pair \in Pairs : ~active[pair] => pending[pair] = 0
FinalValueCannotBeCoalescedAway == ~badFinal
FinalsAreBoundaries == finals <= boundaries

=============================================================================
