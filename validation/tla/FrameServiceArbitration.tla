---------------------- MODULE FrameServiceArbitration ----------------------
EXTENDS Naturals

(***************************************************************************
 * Frame service arbitrates three owners of one output: a native pending    *
 * frame, a queued GPU present, and a waiting software present. A reducer    *
 * decides which may proceed; a handler then executes that decision against  *
 * the same state.                                                           *
 *                                                                           *
 * The bug this exists to forbid lived in the gap between those two. The     *
 * reducer admitted a present while the handler refused it, and the reducer  *
 * withheld the only effect that drains what the handler was refusing over.  *
 * Neither owner could advance, and nothing re-deferred the present, so the  *
 * pair waited on each other permanently. Modelling the reducer gate and the *
 * handler precondition as separate conjuncts is the whole point: a handler  *
 * guard that merely repeats its reducer gate is unreachable, and one that   *
 * asks for more than the gate is a stall or a crash waiting to happen.      *
 *************************************************************************)

CONSTANTS MaxFrames, MaxSoftware

ASSUME MaxFrames \in Nat /\ MaxSoftware \in Nat

PresentStates == {"none", "queued", "inflight", "displayed"}

VARIABLES pendingFrame, presentState, kmsInFlight, swWaiting, crashed,
          frameBudget, swBudget

vars == <<pendingFrame, presentState, kmsInFlight, swWaiting, crashed,
          frameBudget, swBudget>>

Init ==
    /\ pendingFrame = FALSE
    /\ presentState = "none"
    /\ kmsInFlight = FALSE
    /\ swWaiting = FALSE
    /\ crashed = FALSE
    /\ frameBudget = MaxFrames
    /\ swBudget = MaxSoftware

(***************************************************************************
 * Environment. Neither is fair: nothing obliges a client to draw, and the   *
 * budgets keep liveness a question about arbitration rather than about      *
 * outrunning an unbounded producer.                                         *
 *************************************************************************)
EnqueueFrame ==
    /\ frameBudget > 0
    /\ ~pendingFrame
    /\ pendingFrame' = TRUE
    /\ frameBudget' = frameBudget - 1
    /\ UNCHANGED <<presentState, kmsInFlight, swWaiting, crashed, swBudget>>

EnqueueSoftware ==
    /\ swBudget > 0
    /\ ~swWaiting
    /\ swWaiting' = TRUE
    /\ swBudget' = swBudget - 1
    /\ UNCHANGED <<pendingFrame, presentState, kmsInFlight, crashed,
         frameBudget>>

QueuePresent ==
    /\ presentState = "none"
    /\ presentState' = "queued"
    /\ UNCHANGED <<pendingFrame, kmsInFlight, swWaiting, crashed, frameBudget,
         swBudget>>

(***************************************************************************
 * Reducer gates. These are the conditions under which each effect is        *
 * emitted at all.                                                           *
 *************************************************************************)
ReducerEmitsPendingFrame == pendingFrame /\ ~kmsInFlight

ReducerEmitsPresent ==
    /\ presentState = "queued"
    /\ ~kmsInFlight
    /\ ~pendingFrame
    /\ ~swWaiting

ReducerEmitsStaging == swWaiting /\ ~pendingFrame /\ ~kmsInFlight

(***************************************************************************
 * Draining the pending frame is what un-wedges a queued present, so it is   *
 * never withheld on account of one. An earlier revision reserved this       *
 * effect away from the primary output whenever a present was queued, which  *
 * is exactly the deadlock: the present waited on the drain and the drain    *
 * waited on the present.                                                    *
 *************************************************************************)
SubmitPendingFrame ==
    /\ ReducerEmitsPendingFrame
    /\ pendingFrame' = FALSE
    /\ kmsInFlight' = TRUE
    /\ UNCHANGED <<presentState, swWaiting, crashed, frameBudget, swBudget>>

(***************************************************************************
 * The second conjunct is the handler's own refusal. It repeats the reducer  *
 * gate deliberately: stated here, it is provably unreachable, which is what *
 * makes the silent deferral it used to perform impossible rather than       *
 * merely unlikely.                                                          *
 *************************************************************************)
SubmitPresent ==
    /\ ReducerEmitsPresent
    /\ ~pendingFrame
    /\ presentState' = "inflight"
    /\ kmsInFlight' = TRUE
    /\ UNCHANGED <<pendingFrame, swWaiting, crashed, frameBudget, swBudget>>

(***************************************************************************
 * Staging lowers a waiting software present into a native pending frame.    *
 * Its handler fails hard rather than deferring, so the reducer must not     *
 * emit it unless the handler's precondition already holds. Asking for less  *
 * here than the handler demands is a session-ending error, not a stall.     *
 *************************************************************************)
StageSoftware ==
    /\ ReducerEmitsStaging
    /\ IF ~pendingFrame /\ ~kmsInFlight
       THEN /\ swWaiting' = FALSE
            /\ pendingFrame' = TRUE
            /\ UNCHANGED <<presentState, kmsInFlight, crashed, frameBudget,
                 swBudget>>
       ELSE /\ crashed' = TRUE
            /\ UNCHANGED <<pendingFrame, presentState, kmsInFlight, swWaiting,
                 frameBudget, swBudget>>

Retire ==
    /\ kmsInFlight
    /\ kmsInFlight' = FALSE
    /\ presentState' = IF presentState = "inflight"
                       THEN "displayed"
                       ELSE presentState
    /\ UNCHANGED <<pendingFrame, swWaiting, crashed, frameBudget, swBudget>>

ServiceProgress ==
    SubmitPendingFrame \/ SubmitPresent \/ StageSoftware \/ Retire

Next ==
    \/ EnqueueFrame
    \/ EnqueueSoftware
    \/ QueuePresent
    \/ ServiceProgress

Spec == Init /\ [][Next]_vars
FairSpec == Spec /\ WF_vars(ServiceProgress)

TypeOK ==
    /\ pendingFrame \in BOOLEAN
    /\ presentState \in PresentStates
    /\ kmsInFlight \in BOOLEAN
    /\ swWaiting \in BOOLEAN
    /\ crashed \in BOOLEAN
    /\ frameBudget \in 0..MaxFrames
    /\ swBudget \in 0..MaxSoftware

(***************************************************************************
 * A reducer that emits an effect whose handler then refuses it is the       *
 * defect class this module exists for. Staging is the case where that       *
 * refusal is fatal rather than silent.                                      *
 *************************************************************************)
ServiceNeverCrashes == ~crashed

(***************************************************************************
 * One owner cannot occupy the kernel on behalf of two effects at once.      *
 *************************************************************************)
OneSubmissionInFlight ==
    presentState = "inflight" => kmsInFlight

(***************************************************************************
 * Every emitted effect is executable. Stating each handler precondition as  *
 * a consequence of its reducer gate is what keeps the two from drifting     *
 * apart again, which is how the deadlock arose.                             *
 *************************************************************************)
EmittedEffectsAreExecutable ==
    /\ ReducerEmitsPresent => ~pendingFrame
    /\ ReducerEmitsStaging => (~pendingFrame /\ ~kmsInFlight)

PresentSettles == (presentState = "queued") ~> (presentState = "displayed")

SoftwareSettles == swWaiting ~> ~swWaiting

=============================================================================
