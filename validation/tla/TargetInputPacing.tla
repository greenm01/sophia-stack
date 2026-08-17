------------------------- MODULE TargetInputPacing -------------------------
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************
 * One active shell stream exists per seat. Intermediate values occupy one  *
 * replaceable slot and may flush at most once per pacing tick. Admission    *
 * reserves room for the final value and normal completion/cancellation.    *
 * Security revocation clears local state and sends no post-revoke value.    *
 *                                                                           *
 * Device acquisition is the producer the compositor cannot decline to have  *
 * happen: the packet already exists when capacity is examined, so a full    *
 * queue is a disposition choice rather than an absence of work. The three   *
 * choices modelled are bounded deferral (refuse to read, hold nothing),     *
 * scoped endpoint closure (drop the arrival but flush the terminating       *
 * boundary), and terminal failure. Which one a resource takes is data       *
 * attached to the resource, not control flow at the acquisition site.       *
 *************************************************************************)

CONSTANTS Seats, Targets, Values, NoTarget, MaxQueue, MaxEvents, MaxTicks,
          MaxEpoch, MaxDeferral

ASSUME /\ Seats # {} /\ IsFiniteSet(Seats)
       /\ Targets # {} /\ IsFiniteSet(Targets) /\ NoTarget \notin Targets
       /\ Values # {} /\ IsFiniteSet(Values) /\ 0 \in Values
       /\ MaxQueue >= 3 /\ MaxEvents >= MaxQueue
       /\ MaxTicks \in (Nat \ {0}) /\ MaxEpoch >= 2
       /\ MaxDeferral \in (Nat \ {0})

Kinds == {"begin", "discrete", "value", "complete", "cancel", "acquire"}
NoneValue == [some |-> FALSE, value |-> 0]

EmptyRecord == [serial |-> 0, epoch |-> 0, kind |-> "begin", seat |->
    CHOOSE seat \in Seats : TRUE, target |-> NoTarget, token |-> 0,
    value |-> 0, expected |-> 0]

VARIABLES activeTarget, activeToken, currentValue, pending, lastFlushTick,
          tick, queue, emitted, delivered, nextSerial, nextToken,
          endpointLive, authorityEpoch, failures, securityCancellations,
          produced, arrivals, deferralTicks, discarded, reported,
          capacityClosures

capacityVars == <<produced, arrivals, deferralTicks, discarded, reported,
                  capacityClosures>>

vars == <<activeTarget, activeToken, currentValue, pending, lastFlushTick,
          tick, queue, emitted, delivered, nextSerial, nextToken,
          endpointLive, authorityEpoch, failures, securityCancellations,
          produced, arrivals, deferralTicks, discarded, reported,
          capacityClosures>>

ActiveSeats == {seat \in Seats : activeTarget[seat] # NoTarget}
ReservedSlots == 2 * Cardinality(ActiveSeats)

AdmittedCount ==
    Cardinality({index \in 1..Len(emitted) : emitted[index].kind = "acquire"})

(***************************************************************************
 * Saturated means one more ordered arrival would consume boundary reserve.  *
 *************************************************************************)
AcquisitionSaturated == Len(queue) + ReservedSlots + 1 > MaxQueue

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
    /\ produced = 0 /\ arrivals = 0 /\ deferralTicks = 0
    /\ discarded = 0 /\ reported = 0 /\ capacityClosures = {}

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
    /\ UNCHANGED capacityVars

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
    /\ UNCHANGED capacityVars

AdvanceTick ==
    /\ tick < MaxTicks /\ tick' = tick + 1
    /\ UNCHANGED <<activeTarget, activeToken, currentValue, pending,
         lastFlushTick, queue, emitted, delivered, nextSerial, nextToken,
         endpointLive, authorityEpoch, failures, securityCancellations>>
    /\ UNCHANGED capacityVars

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
    /\ UNCHANGED capacityVars

Discrete(seat, target) ==
    LET record == Record(nextSerial, "discrete", seat, target, 0, 0) IN
    /\ endpointLive /\ Len(queue) + ReservedSlots + 1 <= MaxQueue
    /\ nextSerial <= MaxEvents
    /\ AppendOne(record)
    /\ UNCHANGED <<activeTarget, activeToken, currentValue, pending,
         lastFlushTick, tick, delivered, nextToken, endpointLive,
         authorityEpoch, failures, securityCancellations>>
    /\ UNCHANGED capacityVars

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
    /\ UNCHANGED capacityVars

Complete(seat) == Finish(seat, "complete")
Cancel(seat) == Finish(seat, "cancel")

Drain ==
    /\ endpointLive /\ Len(queue) > 0
    /\ delivered' = Append(delivered, Head(queue))
    /\ queue' = Tail(queue)
    /\ UNCHANGED <<activeTarget, activeToken, currentValue, pending,
         lastFlushTick, tick, emitted, nextSerial, nextToken, endpointLive,
         authorityEpoch, failures, securityCancellations>>
    /\ UNCHANGED capacityVars

(***************************************************************************
 * A full ordered queue is endpoint failure, not permission to coalesce a    *
 * discrete action or boundary. Security cancellation is local; no final     *
 * value is enqueued to an endpoint whose epoch has been revoked.             *
 *************************************************************************)
FailTerminal ==
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
    /\ reported' = reported + 1
    /\ UNCHANGED <<currentValue, lastFlushTick, tick, emitted, delivered,
         nextSerial, nextToken>>
    /\ UNCHANGED <<produced, arrivals, deferralTicks, discarded,
         capacityClosures>>

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
    /\ UNCHANGED capacityVars

RestoreEndpoint ==
    /\ ~endpointLive
    /\ endpointLive' = TRUE
    /\ UNCHANGED <<activeTarget, activeToken, currentValue, pending,
         lastFlushTick, tick, queue, emitted, delivered, nextSerial,
         nextToken, authorityEpoch, failures, securityCancellations>>
    /\ UNCHANGED capacityVars

(***************************************************************************
 * The device produced a packet. No amount of compositor capacity undoes     *
 * that: the bytes already exist when capacity is examined, which is what    *
 * makes a full queue a choice of disposition rather than an absence of      *
 * work. Producing is outside fairness because nothing obliges a user to     *
 * touch the keyboard.                                                       *
 *************************************************************************)
DeviceProduces ==
    /\ produced < MaxEvents
    /\ produced' = produced + 1 /\ arrivals' = arrivals + 1
    /\ UNCHANGED <<activeTarget, activeToken, currentValue, pending,
         lastFlushTick, tick, queue, emitted, delivered, nextSerial,
         nextToken, endpointLive, authorityEpoch, failures,
         securityCancellations>>
    /\ UNCHANGED <<deferralTicks, discarded, reported, capacityClosures>>

(***************************************************************************
 * Admission. Ordered arrivals take their own slot rather than replacing an  *
 * earlier one, so admitting ends the stall outright.                        *
 *************************************************************************)
Acquire(seat) ==
    LET record == Record(nextSerial, "acquire", seat, NoTarget, 0, 0) IN
    /\ endpointLive /\ arrivals > 0 /\ ~AcquisitionSaturated
    /\ nextSerial <= MaxEvents
    /\ arrivals' = arrivals - 1 /\ deferralTicks' = 0
    /\ AppendOne(record)
    /\ UNCHANGED <<activeTarget, activeToken, currentValue, pending,
         lastFlushTick, tick, delivered, nextToken, endpointLive,
         authorityEpoch, failures, securityCancellations>>
    /\ UNCHANGED <<produced, discarded, reported, capacityClosures>>

(***************************************************************************
 * Bounded deferral. Declining to read is backpressure rather than loss --   *
 * the packet stays where the device left it -- and what separates this      *
 * from the unbounded retry it replaces is that the wait has a ceiling.      *
 * Deferral is outside fairness: it is permitted, never obliged.             *
 *************************************************************************)
Defer ==
    /\ endpointLive /\ arrivals > 0 /\ AcquisitionSaturated
    /\ deferralTicks < MaxDeferral
    /\ deferralTicks' = deferralTicks + 1
    /\ UNCHANGED <<activeTarget, activeToken, currentValue, pending,
         lastFlushTick, tick, queue, emitted, delivered, nextSerial,
         nextToken, endpointLive, authorityEpoch, failures,
         securityCancellations>>
    /\ UNCHANGED <<produced, arrivals, discarded, reported, capacityClosures>>

(***************************************************************************
 * Escalation once deferral is spent. The waiting arrival is dropped and      *
 * counted, but the terminating boundary is emitted out of reserved capacity  *
 * rather than dropped with it: a discarded release is a key held down        *
 * forever. Closure is scoped to one seat, so a saturated endpoint does not   *
 * fence the others, and it names its tokens separately from security         *
 * revocation because only the latter is entitled to leave no boundary.       *
 *************************************************************************)
EscalateEndpoint(seat) ==
    LET final == currentValue[seat] IN
    LET valueRecord == [Record(nextSerial, "value", seat, activeTarget[seat],
                               activeToken[seat], final) EXCEPT
                              !.expected = currentValue[seat]] IN
    LET boundary == [Record(nextSerial + 1, "cancel", seat, activeTarget[seat],
                            activeToken[seat], final) EXCEPT
                           !.expected = currentValue[seat]] IN
    /\ endpointLive /\ arrivals > 0 /\ AcquisitionSaturated
    /\ deferralTicks = MaxDeferral /\ activeTarget[seat] # NoTarget
    /\ nextSerial + 1 <= MaxEvents /\ Len(queue) + 2 <= MaxQueue
    /\ activeTarget' = [activeTarget EXCEPT ![seat] = NoTarget]
    /\ activeToken' = [activeToken EXCEPT ![seat] = 0]
    /\ pending' = [pending EXCEPT ![seat] = NoneValue]
    /\ capacityClosures' = capacityClosures \cup {activeToken[seat]}
    /\ discarded' = discarded + arrivals /\ arrivals' = 0
    /\ deferralTicks' = 0 /\ reported' = reported + 1
    /\ AppendTwo(valueRecord, boundary)
    /\ UNCHANGED <<currentValue, lastFlushTick, tick, delivered, nextToken,
         endpointLive, authorityEpoch, failures, securityCancellations,
         produced>>

Next ==
    \/ \E seat \in Seats, target \in Targets, value \in Values :
          Begin(seat, target, value)
    \/ \E seat \in Seats, value \in Values : Update(seat, value)
    \/ AdvanceTick \/ \E seat \in Seats : Flush(seat)
    \/ \E seat \in Seats, target \in Targets : Discrete(seat, target)
    \/ \E seat \in Seats : Complete(seat) \/ Cancel(seat)
    \/ Drain \/ FailTerminal \/ SecurityRevoke \/ RestoreEndpoint
    \/ DeviceProduces \/ \E seat \in Seats : Acquire(seat)
    \/ Defer \/ \E seat \in Seats : EscalateEndpoint(seat)

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
    /\ produced \in 0..MaxEvents /\ arrivals \in 0..MaxEvents
    /\ deferralTicks \in Nat
    /\ discarded \in 0..MaxEvents /\ reported \in Nat
    /\ capacityClosures \subseteq 1..MaxEvents

OneCapturePerSeat ==
    \A seat \in Seats :
        activeTarget[seat] = NoTarget <=> activeToken[seat] = 0

OneReplaceableSlot ==
    \A seat \in Seats : ~pending[seat].some => pending[seat] = NoneValue

(***************************************************************************
 * The reserve is load-bearing rather than decorative. Two slots per active   *
 * seat is exactly what EscalateEndpoint spends on a boundary, so this bound  *
 * is what makes escalation always possible: room for every active seat to    *
 * terminate is present in every reachable state, and escalation can never be *
 * the thing that blocks itself.                                              *
 *************************************************************************)
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

(***************************************************************************
 * A ceiling on waiting is the whole difference between bounded deferral and  *
 * the unbounded retry loop it replaces.                                      *
 *************************************************************************)
DeferralIsBounded == deferralTicks <= MaxDeferral

(***************************************************************************
 * Nothing the device produced vanishes unaccounted at the acquisition        *
 * boundary: every packet is still waiting, admitted to the ordered stream,   *
 * or counted as discarded. This is what forbids two arrivals collapsing into *
 * one record, so it is the executable form of the ordered capacity class.    *
 * Work lost later to an epoch close is a separate concern, governed by       *
 * NoPostRevokeQueuedData.                                                    *
 *************************************************************************)
AcquisitionIsConserved == produced = AdmittedCount + discarded + arrivals

(***************************************************************************
 * Ordered work is also never reordered: draining and epoch clearing both     *
 * preserve emission order, so delivered serials strictly increase.           *
 *************************************************************************)
DeliveryPreservesOrder ==
    \A left, right \in 1..Len(delivered) :
        left < right => delivered[left].serial < delivered[right].serial

(***************************************************************************
 * A degradation that says nothing is indistinguishable from working code.    *
 * Reports are also bounded by what actually happened, so a coalescer cannot  *
 * satisfy this by fabricating them.                                          *
 *************************************************************************)
SaturationIsRecorded ==
    /\ discarded > 0 => reported > 0
    /\ reported <= discarded + failures

(***************************************************************************
 * Capacity closure and security revocation are different powers. Only        *
 * revocation may leave a stream with no boundary; a capacity closure owes    *
 * every token it closed a cancellation, which is what keeps a modifier from  *
 * latching down when its endpoint saturates.                                 *
 *************************************************************************)
EndpointCloseIsScoped ==
    /\ capacityClosures \cap securityCancellations = {}
    /\ \A token \in capacityClosures :
          \E index \in 1..Len(emitted) :
              /\ emitted[index].token = token
              /\ emitted[index].kind = "cancel"

QueueEventuallyEmpties == []<>(Len(queue) = 0)

=============================================================================
