------------------------- MODULE TargetInputPacing -------------------------
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************
 * One active shell stream exists per seat. Intermediate values occupy one  *
 * replaceable slot and may flush at most once per pacing tick. Admission    *
 * reserves room for the final value and normal completion/cancellation.    *
 * Security revocation clears local state and sends no post-revoke value.    *
 *************************************************************************)

CONSTANTS Seats, Targets, Values, NoTarget, MaxQueue, MaxEvents, MaxTicks,
          MaxEpoch

ASSUME /\ Seats # {} /\ IsFiniteSet(Seats)
       /\ Targets # {} /\ IsFiniteSet(Targets) /\ NoTarget \notin Targets
       /\ Values # {} /\ IsFiniteSet(Values) /\ 0 \in Values
       /\ MaxQueue >= 3 /\ MaxEvents >= MaxQueue
       /\ MaxTicks \in (Nat \ {0}) /\ MaxEpoch >= 2

Kinds == {"begin", "discrete", "value", "complete", "cancel"}
NoneValue == [some |-> FALSE, value |-> 0]

EmptyRecord == [serial |-> 0, epoch |-> 0, kind |-> "begin", seat |->
    CHOOSE seat \in Seats : TRUE, target |-> NoTarget, token |-> 0,
    value |-> 0, expected |-> 0]

VARIABLES activeTarget, activeToken, currentValue, pending, lastFlushTick,
          tick, queue, emitted, delivered, nextSerial, nextToken,
          endpointLive, authorityEpoch, failures, securityCancellations

vars == <<activeTarget, activeToken, currentValue, pending, lastFlushTick,
          tick, queue, emitted, delivered, nextSerial, nextToken,
          endpointLive, authorityEpoch, failures, securityCancellations>>

ActiveSeats == {seat \in Seats : activeTarget[seat] # NoTarget}
ReservedSlots == 2 * Cardinality(ActiveSeats)

Record(serial, kind, seat, target, token, value) ==
    [serial |-> serial, epoch |-> authorityEpoch, kind |-> kind,
     seat |-> seat, target |-> target, token |-> token, value |-> value,
     expected |-> value]

AppendOne(record) ==
    /\ queue' = Append(queue, record)
    /\ emitted' = Append(emitted, record)
    /\ nextSerial' = nextSerial + 1

AppendTwo(first, second) ==
    /\ queue' = Append(Append(queue, first), second)
    /\ emitted' = Append(Append(emitted, first), second)
    /\ nextSerial' = nextSerial + 2

Init ==
    /\ activeTarget = [seat \in Seats |-> NoTarget]
    /\ activeToken = [seat \in Seats |-> 0]
    /\ currentValue = [seat \in Seats |-> 0]
    /\ pending = [seat \in Seats |-> NoneValue]
    /\ lastFlushTick = [seat \in Seats |-> 0]
    /\ tick = 0 /\ queue = <<>> /\ emitted = <<>> /\ delivered = <<>>
    /\ nextSerial = 1 /\ nextToken = 1
    /\ endpointLive = TRUE /\ authorityEpoch = 1
    /\ failures = 0 /\ securityCancellations = {}

Begin(seat, target, value) ==
    LET record == Record(nextSerial, "begin", seat, target, nextToken, value) IN
    /\ endpointLive /\ activeTarget[seat] = NoTarget /\ value \in Values
    /\ Len(queue) + ReservedSlots + 3 <= MaxQueue
    /\ nextSerial <= MaxEvents /\ nextToken <= MaxEvents
    /\ activeTarget' = [activeTarget EXCEPT ![seat] = target]
    /\ activeToken' = [activeToken EXCEPT ![seat] = nextToken]
    /\ currentValue' = [currentValue EXCEPT ![seat] = value]
    /\ pending' = [pending EXCEPT ![seat] = NoneValue]
    /\ lastFlushTick' = [lastFlushTick EXCEPT ![seat] = tick]
    /\ nextToken' = nextToken + 1
    /\ AppendOne(record)
    /\ UNCHANGED <<tick, delivered, endpointLive, authorityEpoch, failures,
         securityCancellations>>

(***************************************************************************
 * Zero is a real value. Repeated updates replace this record in place and   *
 * therefore do not consume queue capacity.                                  *
 *************************************************************************)
Update(seat, value) ==
    /\ endpointLive /\ activeTarget[seat] # NoTarget /\ value \in Values
    /\ pending' = [pending EXCEPT ![seat] = [some |-> TRUE, value |-> value]]
    /\ currentValue' = [currentValue EXCEPT ![seat] = value]
    /\ UNCHANGED <<activeTarget, activeToken, lastFlushTick, tick, queue,
         emitted, delivered, nextSerial, nextToken, endpointLive,
         authorityEpoch, failures, securityCancellations>>

AdvanceTick ==
    /\ tick < MaxTicks /\ tick' = tick + 1
    /\ UNCHANGED <<activeTarget, activeToken, currentValue, pending,
         lastFlushTick, queue, emitted, delivered, nextSerial, nextToken,
         endpointLive, authorityEpoch, failures, securityCancellations>>

Flush(seat) ==
    LET record == Record(nextSerial, "value", seat, activeTarget[seat],
                         activeToken[seat], pending[seat].value) IN
    /\ endpointLive /\ activeTarget[seat] # NoTarget /\ pending[seat].some
    /\ lastFlushTick[seat] < tick
    /\ Len(queue) + ReservedSlots < MaxQueue
    /\ nextSerial <= MaxEvents
    /\ pending' = [pending EXCEPT ![seat] = NoneValue]
    /\ lastFlushTick' = [lastFlushTick EXCEPT ![seat] = tick]
    /\ AppendOne(record)
    /\ UNCHANGED <<activeTarget, activeToken, currentValue, tick, delivered,
         nextToken, endpointLive, authorityEpoch, failures,
         securityCancellations>>

Discrete(seat, target) ==
    LET record == Record(nextSerial, "discrete", seat, target, 0, 0) IN
    /\ endpointLive /\ Len(queue) + ReservedSlots + 1 <= MaxQueue
    /\ nextSerial <= MaxEvents
    /\ AppendOne(record)
    /\ UNCHANGED <<activeTarget, activeToken, currentValue, pending,
         lastFlushTick, tick, delivered, nextToken, endpointLive,
         authorityEpoch, failures, securityCancellations>>

Finish(seat, kind) ==
    LET final == currentValue[seat] IN
    LET valueRecord == [Record(nextSerial, "value", seat, activeTarget[seat],
                               activeToken[seat], final) EXCEPT
                              !.expected = currentValue[seat]] IN
    LET boundary == [Record(nextSerial + 1, kind, seat, activeTarget[seat],
                            activeToken[seat], final) EXCEPT
                           !.expected = currentValue[seat]] IN
    /\ endpointLive /\ activeTarget[seat] # NoTarget
    /\ kind \in {"complete", "cancel"}
    /\ Len(queue) + 2 <= MaxQueue /\ nextSerial + 1 <= MaxEvents
    /\ activeTarget' = [activeTarget EXCEPT ![seat] = NoTarget]
    /\ activeToken' = [activeToken EXCEPT ![seat] = 0]
    /\ pending' = [pending EXCEPT ![seat] = NoneValue]
    /\ AppendTwo(valueRecord, boundary)
    /\ UNCHANGED <<currentValue, lastFlushTick, tick, delivered, nextToken,
         endpointLive, authorityEpoch, failures, securityCancellations>>

Complete(seat) == Finish(seat, "complete")
Cancel(seat) == Finish(seat, "cancel")

Drain ==
    /\ endpointLive /\ Len(queue) > 0
    /\ delivered' = Append(delivered, Head(queue))
    /\ queue' = Tail(queue)
    /\ UNCHANGED <<activeTarget, activeToken, currentValue, pending,
         lastFlushTick, tick, emitted, nextSerial, nextToken, endpointLive,
         authorityEpoch, failures, securityCancellations>>

(***************************************************************************
 * A full ordered queue is endpoint failure, not permission to coalesce a    *
 * discrete action or boundary. Security cancellation is local; no final     *
 * value is enqueued to an endpoint whose epoch has been revoked.             *
 *************************************************************************)
FailClosed ==
    /\ endpointLive /\ authorityEpoch < MaxEpoch
    /\ Len(queue) + ReservedSlots = MaxQueue
    /\ endpointLive' = FALSE /\ authorityEpoch' = authorityEpoch + 1
    /\ failures' = failures + 1
    /\ securityCancellations' = securityCancellations
         \cup {activeToken[seat] : seat \in ActiveSeats}
    /\ activeTarget' = [seat \in Seats |-> NoTarget]
    /\ activeToken' = [seat \in Seats |-> 0]
    /\ pending' = [seat \in Seats |-> NoneValue]
    /\ queue' = <<>>
    /\ UNCHANGED <<currentValue, lastFlushTick, tick, emitted, delivered,
         nextSerial, nextToken>>

SecurityRevoke ==
    /\ endpointLive /\ authorityEpoch < MaxEpoch
    /\ endpointLive' = FALSE /\ authorityEpoch' = authorityEpoch + 1
    /\ securityCancellations' = securityCancellations
         \cup {activeToken[seat] : seat \in ActiveSeats}
    /\ activeTarget' = [seat \in Seats |-> NoTarget]
    /\ activeToken' = [seat \in Seats |-> 0]
    /\ pending' = [seat \in Seats |-> NoneValue]
    /\ queue' = <<>>
    /\ UNCHANGED <<currentValue, lastFlushTick, tick, emitted, delivered,
         nextSerial, nextToken, failures>>

RestoreEndpoint ==
    /\ ~endpointLive
    /\ endpointLive' = TRUE
    /\ UNCHANGED <<activeTarget, activeToken, currentValue, pending,
         lastFlushTick, tick, queue, emitted, delivered, nextSerial,
         nextToken, authorityEpoch, failures, securityCancellations>>

Next ==
    \/ \E seat \in Seats, target \in Targets, value \in Values :
          Begin(seat, target, value)
    \/ \E seat \in Seats, value \in Values : Update(seat, value)
    \/ AdvanceTick \/ \E seat \in Seats : Flush(seat)
    \/ \E seat \in Seats, target \in Targets : Discrete(seat, target)
    \/ \E seat \in Seats : Complete(seat) \/ Cancel(seat)
    \/ Drain \/ FailClosed \/ SecurityRevoke \/ RestoreEndpoint

Spec == Init /\ [][Next]_vars /\ WF_vars(Drain)

RecordType(record) ==
    /\ record.serial \in 1..MaxEvents /\ record.epoch \in 1..MaxEpoch
    /\ record.kind \in Kinds /\ record.seat \in Seats
    /\ record.target \in Targets \cup {NoTarget}
    /\ record.token \in 0..MaxEvents /\ record.value \in Values
    /\ record.expected \in Values

TypeOK ==
    /\ activeTarget \in [Seats -> Targets \cup {NoTarget}]
    /\ activeToken \in [Seats -> 0..MaxEvents]
    /\ currentValue \in [Seats -> Values]
    /\ pending \in [Seats -> [some : BOOLEAN, value : Values]]
    /\ lastFlushTick \in [Seats -> 0..MaxTicks] /\ tick \in 0..MaxTicks
    /\ Len(queue) <= MaxQueue /\ Len(emitted) <= MaxEvents
    /\ Len(delivered) <= MaxEvents
    /\ \A index \in 1..Len(queue) : RecordType(queue[index])
    /\ \A index \in 1..Len(emitted) : RecordType(emitted[index])
    /\ \A index \in 1..Len(delivered) : RecordType(delivered[index])
    /\ nextSerial \in 1..(MaxEvents + 1) /\ nextToken \in 1..(MaxEvents + 1)
    /\ endpointLive \in BOOLEAN /\ authorityEpoch \in 1..MaxEpoch
    /\ failures \in Nat /\ securityCancellations \subseteq 1..MaxEvents

OneCapturePerSeat ==
    \A seat \in Seats :
        activeTarget[seat] = NoTarget <=> activeToken[seat] = 0

OneReplaceableSlot ==
    \A seat \in Seats : ~pending[seat].some => pending[seat] = NoneValue

BoundaryCapacityIsReserved == Len(queue) + ReservedSlots <= MaxQueue

SerialsAreUnique ==
    \A left, right \in 1..Len(emitted) :
        emitted[left].serial = emitted[right].serial => left = right

DeliveredAtMostOnce ==
    \A left, right \in 1..Len(delivered) :
        delivered[left].serial = delivered[right].serial => left = right

FinalValuePrecedesNormalBoundary ==
    \A index \in 1..Len(emitted) :
        emitted[index].kind \in {"complete", "cancel"} =>
            /\ index > 1 /\ emitted[index - 1].kind = "value"
            /\ emitted[index - 1].token = emitted[index].token
            /\ emitted[index - 1].value = emitted[index].value
            /\ emitted[index - 1].value = emitted[index - 1].expected
            /\ emitted[index].value = emitted[index].expected

NoPostRevokeQueuedData ==
    \A index \in 1..Len(queue) : queue[index].epoch = authorityEpoch

SecurityCancellationHasNoBoundary ==
    \A index \in 1..Len(emitted) :
        emitted[index].token \in securityCancellations
            => emitted[index].epoch < authorityEpoch

QueueEventuallyEmpties == []<>(Len(queue) = 0)

=============================================================================
