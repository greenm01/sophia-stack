-------------------- MODULE TabDescriptorPresentation --------------------
EXTENDS Naturals, TLC
CONSTANTS MaxGeneration, CheckGeneration, RevokeOnLoss
VARIABLES scene, shell, connected, candidate, presented, capture, accepted
vars == <<scene, shell, connected, candidate, presented, capture, accepted>>
Identity == [scene : 0..MaxGeneration, shell : 0..MaxGeneration]
None == [scene |-> 0, shell |-> 0]
Current == [scene |-> scene, shell |-> shell]
Init == /\ scene = 1 /\ shell = 1 /\ connected = TRUE
        /\ candidate = None /\ presented = None /\ capture = None
        /\ accepted = None
Layout == /\ scene < MaxGeneration /\ scene' = scene + 1
          /\ presented' = None /\ capture' = None /\ accepted' = None
          /\ UNCHANGED <<shell, connected, candidate>>
Prepare == /\ connected /\ candidate' = Current
           /\ UNCHANGED <<scene, shell, connected, presented, capture, accepted>>
Present == /\ connected /\ candidate # None
           /\ (~CheckGeneration \/ candidate = Current)
           /\ presented' = candidate /\ capture' = None /\ accepted' = None
           /\ UNCHANGED <<scene, shell, connected, candidate>>
Press == /\ connected /\ presented = Current /\ capture' = presented
         /\ UNCHANGED <<scene, shell, connected, candidate, presented, accepted>>
Release == /\ capture # None /\ capture = presented
           /\ accepted' = capture /\ capture' = None
           /\ UNCHANGED <<scene, shell, connected, candidate, presented>>
LoseShell == /\ connected /\ connected' = FALSE
             /\ capture' = IF RevokeOnLoss THEN None ELSE capture
             /\ accepted' = None
             /\ UNCHANGED <<scene, shell, candidate, presented>>
Reconnect == /\ ~connected /\ shell < MaxGeneration
             /\ shell' = shell + 1 /\ connected' = TRUE
             /\ candidate' = None /\ presented' = None /\ capture' = None
             /\ accepted' = None /\ UNCHANGED scene
Next == Layout \/ Prepare \/ Present \/ Press \/ Release \/ LoseShell \/ Reconnect
TypeOK == /\ scene \in 1..MaxGeneration /\ shell \in 1..MaxGeneration
          /\ connected \in BOOLEAN
          /\ candidate \in Identity /\ presented \in Identity
          /\ capture \in Identity /\ accepted \in Identity
CoherentPresentation == presented = None \/ presented.scene = scene
ExactActivation == accepted = None \/ (connected /\ accepted = Current)
Spec == Init /\ [][Next]_vars
=============================================================================
