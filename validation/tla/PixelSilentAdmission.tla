------------------------ MODULE PixelSilentAdmission ------------------------
EXTENDS Naturals

(***************************************************************************
 * Presentation intent may reach blind-WM layout before complete pixels.    *
 * The first pixel-silent timeout preserves one retry and the standing      *
 * target. Persistent silence withdraws the admission; neither path kills  *
 * the session owner.                                                       *
 *************************************************************************)

Phases == {"intent", "layout", "retry", "managed", "withdrawn"}

VARIABLES phase, safePixels, standingTarget, attempts, ownerLoopAlive

vars == <<phase, safePixels, standingTarget, attempts, ownerLoopAlive>>

Init ==
    /\ phase = "intent"
    /\ safePixels = FALSE
    /\ standingTarget = FALSE
    /\ attempts = 0
    /\ ownerLoopAlive = TRUE

BeginLayout ==
    /\ phase = "intent"
    /\ phase' = "layout"
    /\ standingTarget' = TRUE
    /\ UNCHANGED <<safePixels, attempts, ownerLoopAlive>>

ObservePixels ==
    /\ phase \in {"intent", "layout", "retry"}
    /\ ~safePixels
    /\ safePixels' = TRUE
    /\ UNCHANGED <<phase, standingTarget, attempts, ownerLoopAlive>>

FirstSilentTimeout ==
    /\ phase = "layout"
    /\ ~safePixels
    /\ attempts = 0
    /\ phase' = "retry"
    /\ attempts' = 1
    /\ UNCHANGED <<safePixels, standingTarget, ownerLoopAlive>>

RestartAndReseed ==
    /\ phase = "retry"
    /\ phase' = "layout"
    /\ UNCHANGED <<safePixels, standingTarget, attempts, ownerLoopAlive>>

WithdrawSilentAdmission ==
    /\ phase = "layout"
    /\ ~safePixels
    /\ attempts = 1
    /\ phase' = "withdrawn"
    /\ standingTarget' = FALSE
    /\ UNCHANGED <<safePixels, attempts, ownerLoopAlive>>

CommitPixels ==
    /\ phase \in {"layout", "retry"}
    /\ safePixels
    /\ phase' = "managed"
    /\ standingTarget' = FALSE
    /\ UNCHANGED <<safePixels, attempts, ownerLoopAlive>>

Progress ==
    \/ BeginLayout
    \/ FirstSilentTimeout
    \/ RestartAndReseed
    \/ WithdrawSilentAdmission
    \/ CommitPixels

Next == Progress \/ ObservePixels

Spec == Init /\ [][Next]_vars
FairSpec == Spec /\ WF_vars(Progress)

TypeOK ==
    /\ phase \in Phases
    /\ safePixels \in BOOLEAN
    /\ standingTarget \in BOOLEAN
    /\ attempts \in 0..1
    /\ ownerLoopAlive \in BOOLEAN

OwnerLoopSurvivesAdmission == ownerLoopAlive

PixelSilentAdmissionIsNotManaged ==
    phase = "managed" => safePixels

TerminalAdmissionHasNoStandingTarget ==
    phase \in {"managed", "withdrawn"} => ~standingTarget

FirstTimeoutPreservesTarget ==
    phase = "retry" => /\ attempts = 1
                        /\ standingTarget

PixelSilentLayoutEventuallyAdvances ==
    (phase = "layout" /\ ~safePixels)
        ~> (safePixels \/ phase \in {"retry", "withdrawn"})

RetryEventuallySettlesOrGetsPixels ==
    (phase = "retry" /\ ~safePixels)
        ~> (safePixels \/ phase \in {"managed", "withdrawn"})

=============================================================================
