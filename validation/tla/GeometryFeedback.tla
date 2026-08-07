--------------------------- MODULE GeometryFeedback ---------------------------
EXTENDS Naturals, Sequences, TLC

CONSTANTS OldGeometry, TargetGeometry

Geometry == {OldGeometry, TargetGeometry}
Kind == {"none", "move", "resize", "noop"}
Phase == {"idle", "active", "committed", "timed_out", "settled"}
Outcome == {"none", "success", "timeout", "noop"}

VARIABLES
    kind,
    phase,
    outcome,
    engineGeometry,
    authorityGeometry,
    controls,
    pixelsReady,
    notifications

vars == <<kind, phase, outcome, engineGeometry, authorityGeometry,
          controls, pixelsReady, notifications>>

Init ==
    /\ kind = "none"
    /\ phase = "idle"
    /\ outcome = "none"
    /\ engineGeometry = OldGeometry
    /\ authorityGeometry = OldGeometry
    /\ controls = <<>>
    /\ pixelsReady = FALSE
    /\ notifications = <<>>

BeginMove ==
    /\ phase = "idle"
    /\ kind' = "move"
    /\ phase' = "active"
    /\ outcome' = "none"
    /\ controls' = <<TargetGeometry>>
    /\ UNCHANGED <<engineGeometry, authorityGeometry, pixelsReady, notifications>>

BeginResize ==
    /\ phase = "idle"
    /\ kind' = "resize"
    /\ phase' = "active"
    /\ outcome' = "none"
    /\ controls' = <<TargetGeometry>>
    /\ UNCHANGED <<engineGeometry, authorityGeometry, pixelsReady, notifications>>

BeginNoOp ==
    /\ phase = "idle"
    /\ kind' = "noop"
    /\ phase' = "settled"
    /\ outcome' = "noop"
    /\ UNCHANGED <<engineGeometry, authorityGeometry, controls,
                    pixelsReady, notifications>>

ObservePixels ==
    /\ phase = "active"
    /\ kind = "resize"
    /\ ~pixelsReady
    /\ pixelsReady' = TRUE
    /\ UNCHANGED <<kind, phase, outcome, engineGeometry, authorityGeometry,
                    controls, notifications>>

CommitMove ==
    /\ phase = "active"
    /\ kind = "move"
    /\ engineGeometry' = TargetGeometry
    /\ phase' = "committed"
    /\ outcome' = "success"
    /\ UNCHANGED <<kind, authorityGeometry, controls, pixelsReady, notifications>>

CommitResize ==
    /\ phase = "active"
    /\ kind = "resize"
    /\ pixelsReady
    /\ engineGeometry' = TargetGeometry
    /\ phase' = "committed"
    /\ outcome' = "success"
    /\ UNCHANGED <<kind, authorityGeometry, controls, pixelsReady, notifications>>

Timeout ==
    /\ phase = "active"
    /\ kind \in {"move", "resize"}
    /\ engineGeometry' = OldGeometry
    /\ controls' = Append(controls, OldGeometry)
    /\ phase' = "timed_out"
    /\ outcome' = "timeout"
    /\ UNCHANGED <<kind, authorityGeometry, pixelsReady, notifications>>

DeliverControl ==
    /\ Len(controls) > 0
    /\ LET geometry == Head(controls)
           IN /\ authorityGeometry' = geometry
              /\ controls' = Tail(controls)
              /\ notifications' =
                    IF geometry = authorityGeometry
                    THEN notifications
                    ELSE Append(notifications, geometry)
    /\ UNCHANGED <<kind, phase, outcome, engineGeometry, pixelsReady>>

Settle ==
    /\ phase \in {"committed", "timed_out"}
    /\ Len(controls) = 0
    /\ authorityGeometry = engineGeometry
    /\ phase' = "settled"
    /\ UNCHANGED <<kind, outcome, engineGeometry, authorityGeometry,
                    controls, pixelsReady, notifications>>

Progress == ObservePixels \/ CommitMove \/ CommitResize \/ Timeout

Next ==
    \/ BeginMove
    \/ BeginResize
    \/ BeginNoOp
    \/ Progress
    \/ DeliverControl
    \/ Settle

Spec ==
    /\ Init
    /\ [][Next]_vars
    /\ WF_vars(Progress)
    /\ WF_vars(DeliverControl)
    /\ WF_vars(Settle)

TypeOK ==
    /\ kind \in Kind
    /\ phase \in Phase
    /\ outcome \in Outcome
    /\ engineGeometry \in Geometry
    /\ authorityGeometry \in Geometry
    /\ controls \in Seq(Geometry)
    /\ pixelsReady \in BOOLEAN
    /\ notifications \in Seq(Geometry)

BoundedControlOrder ==
    controls \in {<<>>, <<TargetGeometry>>, <<OldGeometry>>,
                    <<TargetGeometry, OldGeometry>>}

NoOpIsSilent ==
    kind = "noop" =>
        /\ controls = <<>>
        /\ notifications = <<>>
        /\ engineGeometry = OldGeometry
        /\ authorityGeometry = OldGeometry

ResizeCommitRequiresPixels ==
    (kind = "resize" /\ outcome = "success") => pixelsReady

MoveReusesCommittedPixels ==
    kind = "move" => ~pixelsReady

TimeoutPreservesCommittedGeometry ==
    outcome = "timeout" => engineGeometry = OldGeometry

SettledGeometryConverges ==
    phase = "settled" =>
        /\ Len(controls) = 0
        /\ authorityGeometry = engineGeometry

NotificationsDescribeChanges ==
    \A index \in 1..Len(notifications):
        IF index = 1
        THEN notifications[index] # OldGeometry
        ELSE notifications[index] # notifications[index - 1]

StartedEventuallySettles ==
    (phase # "idle") ~> (phase = "settled")

=============================================================================
