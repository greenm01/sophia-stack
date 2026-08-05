----------------------- MODULE PresentFrameOwnership -----------------------
EXTENDS FiniteSets

(***************************************************************************
 * A software Present owns one immutable output frame. An unrelated frame   *
 * may submit and retire first, but it cannot advance Present feedback.      *
 ***************************************************************************
*)

CONSTANTS PresentFrame, UnrelatedFrame, NoFrame

ASSUME /\ PresentFrame # UnrelatedFrame
       /\ PresentFrame # NoFrame
       /\ UnrelatedFrame # NoFrame

Frames == {PresentFrame, UnrelatedFrame}
Phases == {"waiting", "pending", "submitted", "retired"}

VARIABLES phase, nativeFrame, unrelatedRetired, retired, feedback

vars == <<phase, nativeFrame, unrelatedRetired, retired, feedback>>

Init ==
    /\ phase = "waiting"
    /\ nativeFrame = NoFrame
    /\ unrelatedRetired = FALSE
    /\ retired = {}
    /\ feedback = FALSE

SubmitUnrelated ==
    /\ phase = "waiting"
    /\ nativeFrame = NoFrame
    /\ ~unrelatedRetired
    /\ nativeFrame' = UnrelatedFrame
    /\ UNCHANGED <<phase, unrelatedRetired, retired, feedback>>

RetireUnrelated ==
    /\ nativeFrame = UnrelatedFrame
    /\ nativeFrame' = NoFrame
    /\ unrelatedRetired' = TRUE
    /\ retired' = retired \cup {UnrelatedFrame}
    /\ UNCHANGED <<phase, feedback>>

QueuePresent ==
    /\ phase = "waiting"
    /\ nativeFrame = NoFrame
    /\ phase' = "pending"
    /\ nativeFrame' = PresentFrame
    /\ UNCHANGED <<unrelatedRetired, retired, feedback>>

SubmitPresent ==
    /\ phase = "pending"
    /\ nativeFrame = PresentFrame
    /\ phase' = "submitted"
    /\ UNCHANGED <<nativeFrame, unrelatedRetired, retired, feedback>>

RetirePresent ==
    /\ phase = "submitted"
    /\ nativeFrame = PresentFrame
    /\ phase' = "retired"
    /\ nativeFrame' = NoFrame
    /\ retired' = retired \cup {PresentFrame}
    /\ feedback' = TRUE
    /\ UNCHANGED unrelatedRetired

PresentProgress == QueuePresent \/ SubmitPresent \/ RetirePresent

Next ==
    \/ SubmitUnrelated
    \/ RetireUnrelated
    \/ PresentProgress

Spec == Init /\ [][Next]_vars
FairSpec == Spec /\ WF_vars(RetireUnrelated) /\ WF_vars(PresentProgress)

TypeOK ==
    /\ phase \in Phases
    /\ nativeFrame \in Frames \cup {NoFrame}
    /\ unrelatedRetired \in BOOLEAN
    /\ retired \subseteq Frames
    /\ feedback \in BOOLEAN

FeedbackRequiresExactFrame ==
    feedback <=> (phase = "retired" /\ PresentFrame \in retired)

UnrelatedRetirementCannotAdvancePresent ==
    UnrelatedFrame \in retired /\ PresentFrame \notin retired => ~feedback

SubmittedFrameOwnsPresent ==
    phase \in {"pending", "submitted"} => nativeFrame = PresentFrame

PresentEventuallyRetires == phase = "waiting" ~> phase = "retired"

=============================================================================
