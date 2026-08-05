------------------------- MODULE SurfaceContentStream -------------------------
EXTENDS Naturals, Sequences

CONSTANTS OpCount, Capacity

VARIABLES owner, nextOp, deferred, ready, applied, generation, feedback,
          unrelatedProgress

vars == <<owner, nextOp, deferred, ready, applied, generation, feedback,
          unrelatedProgress>>

Content == 1..OpCount
ContentSequence == [index \in Content |-> index]

Init ==
    /\ owner = "present"
    /\ nextOp = 1
    /\ deferred = <<>>
    /\ ready = <<>>
    /\ applied = <<>>
    /\ generation = 0
    /\ feedback = FALSE
    /\ unrelatedProgress = FALSE

QueueNext ==
    /\ nextOp <= OpCount
    /\ IF owner = "present"
          THEN /\ deferred' = Append(deferred, nextOp)
               /\ UNCHANGED ready
          ELSE /\ ready' = Append(ready, nextOp)
               /\ UNCHANGED deferred
    /\ nextOp' = nextOp + 1
    /\ UNCHANGED <<owner, applied, generation, feedback, unrelatedProgress>>

RetirePresent ==
    /\ owner = "present"
    /\ owner' = "none"
    /\ ready' = ready \o deferred
    /\ deferred' = <<>>
    /\ generation' = 1
    /\ feedback' = TRUE
    /\ UNCHANGED <<nextOp, applied, unrelatedProgress>>

ApplyReady ==
    /\ owner = "none"
    /\ Len(ready) > 0
    /\ applied' = Append(applied, Head(ready))
    /\ ready' = Tail(ready)
    /\ generation' = generation + 1
    /\ UNCHANGED <<owner, nextOp, deferred, feedback, unrelatedProgress>>

AdvanceUnrelated ==
    /\ ~unrelatedProgress
    /\ unrelatedProgress' = TRUE
    /\ UNCHANGED <<owner, nextOp, deferred, ready, applied, generation, feedback>>

Next == QueueNext \/ RetirePresent \/ ApplyReady \/ AdvanceUnrelated

TypeOK ==
    /\ owner \in {"present", "none"}
    /\ nextOp \in 1..(OpCount + 1)
    /\ deferred \in Seq(Content)
    /\ ready \in Seq(Content)
    /\ applied \in Seq(Content)
    /\ generation \in Nat
    /\ feedback \in BOOLEAN
    /\ unrelatedProgress \in BOOLEAN

StreamBound == Len(deferred) <= Capacity

NoContentOvertakesPresent ==
    owner = "present" => /\ Len(applied) = 0
                         /\ Len(ready) = 0
                         /\ generation = 0
                         /\ ~feedback

ContentRemainsOrdered ==
    applied \o ready \o deferred = SubSeq(ContentSequence, 1, nextOp - 1)

GenerationMatchesVisibleContent ==
    generation = IF owner = "present" THEN 0 ELSE 1 + Len(applied)

AllContentEventuallyApplies == <>(Len(applied) = OpCount)

Spec ==
    /\ Init
    /\ [][Next]_vars
    /\ WF_vars(QueueNext)
    /\ WF_vars(RetirePresent)
    /\ WF_vars(ApplyReady)

=============================================================================
