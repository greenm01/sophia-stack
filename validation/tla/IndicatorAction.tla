--------------------------- MODULE IndicatorAction ---------------------------
EXTENDS Naturals

(***************************************************************************
 * Tier-0 hit targets carry the descriptor epoch and projection serial that *
 * produced their pixels. A click may route only while both still match.    *
 *************************************************************************)

CONSTANT MaxEpoch, MaxSerial

VARIABLES epoch, serial, targetEpoch, targetSerial, targetAction, routed

vars == <<epoch, serial, targetEpoch, targetSerial, targetAction, routed>>

Init ==
    /\ epoch = 1
    /\ serial = 0
    /\ targetEpoch = 0
    /\ targetSerial = 0
    /\ targetAction = 0
    /\ routed = 0

Commit(action) ==
    /\ serial < MaxSerial
    /\ serial' = serial + 1
    /\ targetEpoch' = epoch
    /\ targetSerial' = serial + 1
    /\ targetAction' = action
    /\ UNCHANGED <<epoch, routed>>

Reconnect ==
    /\ epoch < MaxEpoch
    /\ epoch' = epoch + 1
    /\ UNCHANGED <<serial, targetEpoch, targetSerial, targetAction, routed>>

Click ==
    /\ targetAction # 0
    /\ routed < MaxSerial
    /\ IF /\ targetEpoch = epoch /\ targetSerial = serial
          THEN routed' = routed + 1
          ELSE UNCHANGED routed
    /\ UNCHANGED <<epoch, serial, targetEpoch, targetSerial, targetAction>>

Next == (\E action \in 1..2 : Commit(action)) \/ Reconnect \/ Click
Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ epoch \in 1..MaxEpoch
    /\ serial \in 0..MaxSerial
    /\ targetEpoch \in 0..MaxEpoch
    /\ targetSerial \in 0..MaxSerial
    /\ targetAction \in 0..2
    /\ routed \in 0..MaxSerial

=============================================================================
