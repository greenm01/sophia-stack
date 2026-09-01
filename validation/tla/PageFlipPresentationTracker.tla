---------------- MODULE PageFlipPresentationTracker ----------------
EXTENDS Naturals, FiniteSets

(***************************************************************************
 * The physical KMS owner and Engine's logical presentation tracker must    *
 * name the same outstanding frame. A page-flip event and an out-fence are  *
 * alternate completion proofs for that one owner; changing proof source    *
 * must not change the clock domain supplied to OutputPresentationRegistry. *
 *                                                                         *
 * Rust correspondence:                                                    *
 * - production_session.rs::LiveProductionPageFlipTracker;                 *
 * - frame/presentation.rs::OutputPresentationRegistry; and                 *
 * - production_session/native_scanout.rs::observe_callbacks_with_source.  *
 *************************************************************************)

CONSTANTS MaxFrame, MonotonicBase

ASSUME MaxFrame \in Nat \ {0} /\ MonotonicBase \in Nat \ {0}

NoFrame == 0
Frames == 1..MaxFrame
Sources == {"none", "page_flip", "out_fence"}

VARIABLES
    nextFrame,
    physicalInFlight,
    trackerPending,
    physicalRetired,
    trackerRetired,
    lastSequence,
    lastPresentationMsec,
    monotonicMsec,
    lastSource,
    overlapRejections,
    phaseRejections

vars == <<
    nextFrame,
    physicalInFlight,
    trackerPending,
    physicalRetired,
    trackerRetired,
    lastSequence,
    lastPresentationMsec,
    monotonicMsec,
    lastSource,
    overlapRejections,
    phaseRejections
>>

Init ==
    /\ nextFrame = 1
    /\ physicalInFlight = NoFrame
    /\ trackerPending = NoFrame
    /\ physicalRetired = {}
    /\ trackerRetired = {}
    /\ lastSequence = 0
    /\ lastPresentationMsec = 0
    /\ monotonicMsec = MonotonicBase
    /\ lastSource = "none"
    /\ overlapRejections = 0
    /\ phaseRejections = 0

(***************************************************************************
 * A physical submission is accepted whenever KMS is free. The logical      *
 * tracker must accept the same frame; a surviving tracker owner would turn  *
 * this otherwise valid physical submission into an overlap rejection.      *
 *************************************************************************)
Submit ==
    /\ nextFrame <= MaxFrame
    /\ physicalInFlight = NoFrame
    /\ physicalInFlight' = nextFrame
    /\ IF trackerPending = NoFrame
       THEN /\ trackerPending' = nextFrame
            /\ overlapRejections' = overlapRejections
       ELSE /\ trackerPending' = trackerPending
            /\ overlapRejections' = overlapRejections + 1
    /\ nextFrame' = nextFrame + 1
    /\ monotonicMsec' = monotonicMsec + 1
    /\ UNCHANGED <<physicalRetired, trackerRetired, lastSequence,
                    lastPresentationMsec, lastSource, phaseRejections>>

Complete(source) ==
    /\ source \in Sources \ {"none"}
    /\ physicalInFlight # NoFrame
    /\ trackerPending = physicalInFlight
    /\ LET frame == physicalInFlight
           sequence == lastSequence + 1
           presentationMsec == monotonicMsec + 1
       IN /\ physicalInFlight' = NoFrame
          /\ trackerPending' = NoFrame
          /\ physicalRetired' = physicalRetired \cup {frame}
          /\ trackerRetired' = trackerRetired \cup {frame}
          /\ lastSequence' = sequence
          /\ lastPresentationMsec' = presentationMsec
          /\ monotonicMsec' = presentationMsec
          /\ lastSource' = source
    /\ UNCHANGED <<nextFrame, overlapRejections, phaseRejections>>

CompletePageFlip == Complete("page_flip")
CompleteOutFence == Complete("out_fence")

(***************************************************************************
 * Negative control for the observed 2026-09-01 failure. After an absolute   *
 * monotonic page-flip sample, the out-fence path supplied session-relative  *
 * elapsed time. Engine rejected that timestamp but the physical owner still *
 * retired, orphaning the tracker owner and rejecting every successor.       *
 * The fix settles the exact owner before returning a cadence error. This    *
 * action deliberately omits that settlement.                                *
 *************************************************************************)
CompleteOutFenceWithRelativeClock ==
    /\ physicalInFlight # NoFrame
    /\ trackerPending = physicalInFlight
    /\ lastPresentationMsec >= MonotonicBase
    /\ monotonicMsec - MonotonicBase < lastPresentationMsec
    /\ LET frame == physicalInFlight
       IN /\ physicalInFlight' = NoFrame
          /\ physicalRetired' = physicalRetired \cup {frame}
    /\ phaseRejections' = phaseRejections + 1
    /\ lastSource' = "out_fence"
    /\ UNCHANGED <<nextFrame, trackerPending, trackerRetired, lastSequence,
                    lastPresentationMsec, monotonicMsec, overlapRejections>>

Quiesce == UNCHANGED vars

Next == Submit \/ CompletePageFlip \/ CompleteOutFence \/ Quiesce

Spec == Init /\ [][Next]_vars

MixedClockNext ==
    Submit \/ CompletePageFlip \/ CompleteOutFenceWithRelativeClock \/ Quiesce

MixedClockSpec == Init /\ [][MixedClockNext]_vars

TypeOK ==
    /\ nextFrame \in 1..(MaxFrame + 1)
    /\ physicalInFlight \in 0..MaxFrame
    /\ trackerPending \in 0..MaxFrame
    /\ physicalRetired \subseteq Frames
    /\ trackerRetired \subseteq Frames
    /\ lastSequence \in 0..MaxFrame
    /\ lastPresentationMsec \in Nat
    /\ monotonicMsec \in Nat
    /\ lastSource \in Sources
    /\ overlapRejections \in Nat
    /\ phaseRejections \in Nat

(***************************************************************************
 * This is the cross-layer ownership invariant absent from                  *
 * PageFlipCompletionPump. Physical completion and logical observation are  *
 * one owner transition, even though either kernel proof may license it.     *
 *************************************************************************)
PhysicalTrackerOwnerAgreement ==
    /\ trackerPending = physicalInFlight
    /\ trackerRetired = physicalRetired

AcceptedCompletionsUseOneClock ==
    /\ overlapRejections = 0
    /\ phaseRejections = 0

ExactRetirementAccounting ==
    /\ Cardinality(physicalRetired) = lastSequence
    /\ trackerRetired \subseteq 1..(nextFrame - 1)

=============================================================================
