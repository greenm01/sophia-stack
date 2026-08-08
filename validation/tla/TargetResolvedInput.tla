------------------------ MODULE TargetResolvedInput ------------------------
EXTENDS Naturals, FiniteSets

(***************************************************************************
 * Target-resolved shell input is paired with presented pixels. The model   *
 * deliberately separates commit, submit, and present, and uses one target  *
 * per scene to keep the state space focused on identity and capture.        *
 *************************************************************************)

CONSTANTS Seats, MaxScene, MaxHandle, MaxAction, MaxScope, MaxCancels

ASSUME /\ Seats # {} /\ IsFiniteSet(Seats)
       /\ MaxScene \in (Nat \ {0})
       /\ MaxHandle \in (Nat \ {0})
       /\ MaxAction \in (Nat \ {0})
       /\ MaxScope \in (Nat \ {0})
       /\ MaxCancels \in (Nat \ {0})

VARIABLES
    committedScene, submittedScene, presentedScene,
    committedHandle, submittedHandle, presentedHandle,
    committedAction, submittedAction, presentedAction,
    committedGrant, submittedGrant, presentedGrant,
    committedScope, submittedScope, presentedScope,
    authorityLive, seatLive, activeScope,
    capture, captureAction, captureScope, cancelled,
    activations, badActivation, coordinateEvents, coordinateLeak

vars == <<committedScene, submittedScene, presentedScene,
          committedHandle, submittedHandle, presentedHandle,
          committedAction, submittedAction, presentedAction,
          committedGrant, submittedGrant, presentedGrant,
          committedScope, submittedScope, presentedScope,
          authorityLive, seatLive, activeScope,
          capture, captureAction, captureScope, cancelled,
          activations, badActivation, coordinateEvents, coordinateLeak>>

NoCapture == [seat \in Seats |-> 0]

Init ==
    /\ committedScene = 0 /\ submittedScene = 0 /\ presentedScene = 0
    /\ committedHandle = 0 /\ submittedHandle = 0 /\ presentedHandle = 0
    /\ committedAction = 0 /\ submittedAction = 0 /\ presentedAction = 0
    /\ committedGrant = FALSE /\ submittedGrant = FALSE /\ presentedGrant = FALSE
    /\ committedScope = 1 /\ submittedScope = 1 /\ presentedScope = 1
    /\ authorityLive = TRUE
    /\ seatLive = [seat \in Seats |-> TRUE]
    /\ activeScope = 1
    /\ capture = NoCapture /\ captureAction = NoCapture /\ captureScope = NoCapture
    /\ cancelled = NoCapture /\ activations = 0
    /\ badActivation = FALSE /\ coordinateEvents = 0 /\ coordinateLeak = FALSE

CommitTarget(handle, action, grant, scope) ==
    /\ committedScene < MaxScene
    /\ handle \in 1..MaxHandle /\ action \in 1..MaxAction
    /\ (committedHandle = 0 \/ handle # committedHandle)
    /\ scope \in 1..MaxScope
    /\ committedScene' = committedScene + 1
    /\ committedHandle' = handle /\ committedAction' = action
    /\ committedGrant' = grant /\ committedScope' = scope
    /\ UNCHANGED <<submittedScene, presentedScene, submittedHandle,
         presentedHandle, submittedAction, presentedAction, submittedGrant,
         presentedGrant, submittedScope, presentedScope, authorityLive,
         seatLive, activeScope, capture, captureAction, captureScope,
         cancelled, activations, badActivation, coordinateEvents, coordinateLeak>>

(***************************************************************************
 * A visual-only commit retains the exact handle, action, scope, and grant.  *
 *************************************************************************)
CommitVisualOnly ==
    /\ committedScene < MaxScene /\ committedHandle # 0
    /\ committedScene' = committedScene + 1
    /\ UNCHANGED <<committedHandle, committedAction, committedGrant,
         committedScope, submittedScene, presentedScene, submittedHandle,
         presentedHandle, submittedAction, presentedAction, submittedGrant,
         presentedGrant, submittedScope, presentedScope, authorityLive,
         seatLive, activeScope, capture, captureAction, captureScope,
         cancelled, activations, badActivation, coordinateEvents, coordinateLeak>>

CommitRemoval ==
    /\ committedScene < MaxScene /\ committedHandle # 0
    /\ committedScene' = committedScene + 1
    /\ committedHandle' = 0 /\ committedAction' = 0
    /\ committedGrant' = FALSE /\ committedScope' = activeScope
    /\ UNCHANGED <<submittedScene, presentedScene, submittedHandle,
         presentedHandle, submittedAction, presentedAction, submittedGrant,
         presentedGrant, submittedScope, presentedScope, authorityLive,
         seatLive, activeScope, capture, captureAction, captureScope,
         cancelled, activations, badActivation, coordinateEvents, coordinateLeak>>

Submit ==
    /\ committedScene > submittedScene
    /\ submittedScene' = committedScene
    /\ submittedHandle' = committedHandle /\ submittedAction' = committedAction
    /\ submittedGrant' = committedGrant /\ submittedScope' = committedScope
    /\ UNCHANGED <<committedScene, presentedScene, committedHandle,
         presentedHandle, committedAction, presentedAction, committedGrant,
         presentedGrant, committedScope, presentedScope, authorityLive,
         seatLive, activeScope, capture, captureAction, captureScope,
         cancelled, activations, badActivation, coordinateEvents, coordinateLeak>>

Present ==
    /\ submittedScene > presentedScene
    /\ \A seat \in Seats : capture[seat] # 0 /\ capture[seat] # submittedHandle
          => cancelled[seat] < MaxCancels
    /\ presentedScene' = submittedScene
    /\ presentedHandle' = submittedHandle /\ presentedAction' = submittedAction
    /\ presentedGrant' = submittedGrant /\ presentedScope' = submittedScope
    /\ capture' = [seat \in Seats |->
          IF capture[seat] # 0 /\ capture[seat] # submittedHandle
             THEN 0 ELSE capture[seat]]
    /\ captureAction' = [seat \in Seats |->
          IF capture[seat] # 0 /\ capture[seat] # submittedHandle
             THEN 0 ELSE captureAction[seat]]
    /\ captureScope' = [seat \in Seats |->
          IF capture[seat] # 0 /\ capture[seat] # submittedHandle
             THEN 0 ELSE captureScope[seat]]
    /\ cancelled' = [seat \in Seats |->
          IF capture[seat] # 0 /\ capture[seat] # submittedHandle
             THEN cancelled[seat] + 1 ELSE cancelled[seat]]
    /\ UNCHANGED <<committedScene, submittedScene, committedHandle,
         submittedHandle, committedAction, submittedAction, committedGrant,
         submittedGrant, committedScope, submittedScope, authorityLive,
         seatLive, activeScope, activations, badActivation, coordinateEvents,
         coordinateLeak>>

Press(seat) ==
    /\ seatLive[seat] /\ authorityLive /\ presentedHandle # 0
    /\ presentedScope = activeScope /\ capture[seat] = 0
    /\ capture' = [capture EXCEPT ![seat] = presentedHandle]
    /\ captureAction' = [captureAction EXCEPT ![seat] = presentedAction]
    /\ captureScope' = [captureScope EXCEPT ![seat] = presentedScope]
    /\ UNCHANGED <<committedScene, submittedScene, presentedScene,
         committedHandle, submittedHandle, presentedHandle, committedAction,
         submittedAction, presentedAction, committedGrant, submittedGrant,
         presentedGrant, committedScope, submittedScope, presentedScope,
         authorityLive, seatLive, activeScope, cancelled, activations,
         badActivation, coordinateEvents, coordinateLeak>>

Release(seat) ==
    /\ capture[seat] # 0
    /\ activations < MaxCancels
    /\ seatLive[seat] /\ authorityLive
    /\ capture[seat] = presentedHandle
    /\ captureAction[seat] = presentedAction
    /\ captureScope[seat] = presentedScope
    /\ presentedScope = activeScope
    /\ activations' = activations + 1
    /\ badActivation' = badActivation
    /\ cancelled' = cancelled
    /\ capture' = [capture EXCEPT ![seat] = 0]
    /\ captureAction' = [captureAction EXCEPT ![seat] = 0]
    /\ captureScope' = [captureScope EXCEPT ![seat] = 0]
    /\ UNCHANGED <<committedScene, submittedScene, presentedScene,
         committedHandle, submittedHandle, presentedHandle, committedAction,
         submittedAction, presentedAction, committedGrant, submittedGrant,
         presentedGrant, committedScope, submittedScope, presentedScope,
         authorityLive, seatLive, activeScope, coordinateEvents, coordinateLeak>>

LoseAuthority ==
    /\ authorityLive
    /\ \A seat \in Seats : capture[seat] # 0 => cancelled[seat] < MaxCancels
    /\ authorityLive' = FALSE
    /\ cancelled' = [seat \in Seats |-> cancelled[seat] +
          IF capture[seat] # 0 THEN 1 ELSE 0]
    /\ capture' = NoCapture /\ captureAction' = NoCapture /\ captureScope' = NoCapture
    /\ UNCHANGED <<committedScene, submittedScene, presentedScene,
         committedHandle, submittedHandle, presentedHandle, committedAction,
         submittedAction, presentedAction, committedGrant, submittedGrant,
         presentedGrant, committedScope, submittedScope, presentedScope,
         seatLive, activeScope, activations, badActivation, coordinateEvents,
         coordinateLeak>>

LoseSeat(seat) ==
    /\ seatLive[seat]
    /\ (capture[seat] # 0 => cancelled[seat] < MaxCancels)
    /\ seatLive' = [seatLive EXCEPT ![seat] = FALSE]
    /\ cancelled' = IF capture[seat] # 0
          THEN [cancelled EXCEPT ![seat] = @ + 1] ELSE cancelled
    /\ capture' = [capture EXCEPT ![seat] = 0]
    /\ captureAction' = [captureAction EXCEPT ![seat] = 0]
    /\ captureScope' = [captureScope EXCEPT ![seat] = 0]
    /\ UNCHANGED <<committedScene, submittedScene, presentedScene,
         committedHandle, submittedHandle, presentedHandle, committedAction,
         submittedAction, presentedAction, committedGrant, submittedGrant,
         presentedGrant, committedScope, submittedScope, presentedScope,
         authorityLive, activeScope, activations, badActivation,
         coordinateEvents, coordinateLeak>>

ChangeScope(scope) ==
    /\ scope \in 1..MaxScope /\ scope # activeScope
    /\ \A seat \in Seats : capture[seat] # 0 /\ captureScope[seat] # scope
          => cancelled[seat] < MaxCancels
    /\ activeScope' = scope
    /\ cancelled' = [seat \in Seats |-> cancelled[seat] +
          IF capture[seat] # 0 /\ captureScope[seat] # scope THEN 1 ELSE 0]
    /\ capture' = [seat \in Seats |->
          IF capture[seat] # 0 /\ captureScope[seat] # scope THEN 0 ELSE capture[seat]]
    /\ captureAction' = [seat \in Seats |->
          IF capture[seat] # 0 /\ captureScope[seat] # scope THEN 0 ELSE captureAction[seat]]
    /\ captureScope' = [seat \in Seats |->
          IF capture[seat] # 0 /\ captureScope[seat] # scope THEN 0 ELSE captureScope[seat]]
    /\ UNCHANGED <<committedScene, submittedScene, presentedScene,
         committedHandle, submittedHandle, presentedHandle, committedAction,
         submittedAction, presentedAction, committedGrant, submittedGrant,
         presentedGrant, committedScope, submittedScope, presentedScope,
         authorityLive, seatLive, activations, badActivation, coordinateEvents,
         coordinateLeak>>

DiscloseLocalCoordinate ==
    /\ presentedHandle # 0 /\ authorityLive /\ presentedGrant
    /\ coordinateEvents < MaxCancels
    /\ coordinateEvents' = coordinateEvents + 1
    /\ coordinateLeak' = IF presentedGrant THEN coordinateLeak ELSE TRUE
    /\ UNCHANGED <<committedScene, submittedScene, presentedScene,
         committedHandle, submittedHandle, presentedHandle, committedAction,
         submittedAction, presentedAction, committedGrant, submittedGrant,
         presentedGrant, committedScope, submittedScope, presentedScope,
         authorityLive, seatLive, activeScope, capture, captureAction,
         captureScope, cancelled, activations, badActivation>>

Next ==
    \/ \E handle \in 1..MaxHandle, action \in 1..MaxAction,
          grant \in BOOLEAN, scope \in 1..MaxScope :
          CommitTarget(handle, action, grant, scope)
    \/ CommitVisualOnly \/ CommitRemoval \/ Submit \/ Present
    \/ \E seat \in Seats : Press(seat) \/ Release(seat) \/ LoseSeat(seat)
    \/ LoseAuthority
    \/ \E scope \in 1..MaxScope : ChangeScope(scope)
    \/ DiscloseLocalCoordinate

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ committedScene \in 0..MaxScene
    /\ submittedScene \in 0..MaxScene /\ presentedScene \in 0..MaxScene
    /\ committedHandle \in 0..MaxHandle /\ submittedHandle \in 0..MaxHandle
    /\ presentedHandle \in 0..MaxHandle
    /\ committedAction \in 0..MaxAction /\ submittedAction \in 0..MaxAction
    /\ presentedAction \in 0..MaxAction
    /\ committedScope \in 1..MaxScope /\ submittedScope \in 1..MaxScope
    /\ presentedScope \in 1..MaxScope /\ activeScope \in 1..MaxScope
    /\ seatLive \in [Seats -> BOOLEAN]
    /\ capture \in [Seats -> 0..MaxHandle]
    /\ captureAction \in [Seats -> 0..MaxAction]
    /\ captureScope \in [Seats -> 0..MaxScope]
    /\ cancelled \in [Seats -> 0..MaxCancels]
    /\ activations \in 0..MaxCancels /\ coordinateEvents \in 0..MaxCancels

SubmittedWasCommitted == submittedScene <= committedScene
PresentedWasSubmitted == presentedScene <= submittedScene
CaptureIsBounded == \A seat \in Seats : capture[seat] \in 0..MaxHandle
CapturedTargetIsPresented == \A seat \in Seats : capture[seat] # 0 =>
    /\ capture[seat] = presentedHandle
    /\ captureAction[seat] = presentedAction
    /\ captureScope[seat] = presentedScope
ActivationIsNeverStale == ~badActivation
CoordinatesRequireLocalGrant == ~coordinateLeak
EmptyCaptureIsFullyCleared == \A seat \in Seats :
    capture[seat] = 0 => /\ captureAction[seat] = 0 /\ captureScope[seat] = 0

=============================================================================
