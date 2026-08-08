--------------------- MODULE LegacyWmResponseBoundary ---------------------
EXTENDS Naturals

(***************************************************************************
 * An unmodified legacy WM does not attach Sophia transaction identity to  *
 * X11 requests. A quiet boundary settles successful work. Any request     *
 * failure poisons that private WM process; restart clears every reply     *
 * stage before later Engine work may begin.                               *
 *************************************************************************)

CONSTANTS ReplyOne, ReplyTwo, RequestOne, RequestTwo, NoReply, NoRequest

ASSUME /\ ReplyOne # ReplyTwo
       /\ RequestOne # RequestTwo
       /\ NoReply \notin {ReplyOne, ReplyTwo}
       /\ NoRequest \notin {RequestOne, RequestTwo}

Replies == {ReplyOne, ReplyTwo}
Requests == {RequestOne, RequestTwo}
ReplyRecords ==
    [value : Replies, origin : Requests]
EmittedRecords ==
    [value : Replies, origin : Requests, response : Requests]
Phases == {"ready", "collecting", "failed"}
Outcomes == {"none", "success", "failed"}

VARIABLES processEpoch, request, phase, scheduled, delivered, collected,
          quietBoundary, grabValidated, outcome, emitted

vars == <<processEpoch, request, phase, scheduled, delivered, collected,
          quietBoundary, grabValidated, outcome, emitted>>

NeedsGrab(candidate) == candidate = RequestOne

Init ==
    /\ processEpoch = 0
    /\ request = NoRequest
    /\ phase = "ready"
    /\ scheduled = {}
    /\ delivered = {}
    /\ collected = NoReply
    /\ quietBoundary = FALSE
    /\ grabValidated = FALSE
    /\ outcome = "none"
    /\ emitted = {}

BeginRequest(nextRequest) ==
    /\ phase = "ready"
    /\ nextRequest \in Requests
    /\ request' = nextRequest
    /\ phase' = "collecting"
    /\ collected' = NoReply
    /\ quietBoundary' = FALSE
    /\ grabValidated' = ~NeedsGrab(nextRequest)
    /\ outcome' = "none"
    /\ UNCHANGED <<processEpoch, scheduled, delivered, emitted>>

ValidateGrab ==
    /\ phase = "collecting"
    /\ NeedsGrab(request)
    /\ ~grabValidated
    /\ grabValidated' = TRUE
    /\ UNCHANGED <<processEpoch, request, phase, scheduled, delivered,
                    collected, quietBoundary, outcome, emitted>>

ScheduleReply(value) ==
    /\ phase = "collecting"
    /\ ~quietBoundary
    /\ value \in Replies
    /\ scheduled' = scheduled \cup
        {[value |-> value, origin |-> request]}
    /\ UNCHANGED <<processEpoch, request, phase, delivered, collected,
                    quietBoundary, grabValidated, outcome, emitted>>

DeliverReply(reply) ==
    /\ phase = "collecting"
    /\ reply \in scheduled
    /\ scheduled' = scheduled \ {reply}
    /\ delivered' = delivered \cup {reply}
    /\ UNCHANGED <<processEpoch, request, phase, collected,
                    quietBoundary, grabValidated, outcome, emitted>>

CollectReply(reply) ==
    /\ phase = "collecting"
    /\ reply \in delivered
    /\ delivered' = delivered \ {reply}
    /\ collected' = reply
    /\ UNCHANGED <<processEpoch, request, phase, scheduled,
                    quietBoundary, grabValidated, outcome, emitted>>

ObserveQuietBoundary ==
    /\ phase = "collecting"
    /\ scheduled = {}
    /\ delivered = {}
    /\ grabValidated
    /\ quietBoundary' = TRUE
    /\ UNCHANGED <<processEpoch, request, phase, scheduled, delivered,
                    collected, grabValidated, outcome, emitted>>

CompleteRequest ==
    /\ phase = "collecting"
    /\ quietBoundary
    /\ phase' = "ready"
    /\ request' = NoRequest
    /\ outcome' = "success"
    /\ emitted' =
        IF collected = NoReply
            THEN emitted
            ELSE emitted \cup
                {[value |-> collected.value,
                  origin |-> collected.origin,
                  response |-> request]}
    /\ UNCHANGED <<processEpoch, scheduled, delivered, collected,
                    quietBoundary, grabValidated>>

ReachHardDeadline ==
    /\ phase = "collecting"
    /\ ~quietBoundary
    /\ phase' = "failed"
    /\ request' = NoRequest
    /\ outcome' = "failed"
    /\ UNCHANGED <<processEpoch, scheduled, delivered, collected,
                    quietBoundary, grabValidated, emitted>>

Restart ==
    /\ phase = "failed"
    /\ processEpoch = 0
    /\ processEpoch' = 1
    /\ phase' = "ready"
    /\ request' = NoRequest
    /\ scheduled' = {}
    /\ delivered' = {}
    /\ collected' = NoReply
    /\ quietBoundary' = FALSE
    /\ grabValidated' = FALSE
    /\ outcome' = "none"
    /\ UNCHANGED emitted

Next ==
    \/ \E nextRequest \in Requests : BeginRequest(nextRequest)
    \/ ValidateGrab
    \/ \E value \in Replies : ScheduleReply(value)
    \/ \E reply \in scheduled : DeliverReply(reply)
    \/ \E reply \in delivered : CollectReply(reply)
    \/ ObserveQuietBoundary
    \/ CompleteRequest
    \/ ReachHardDeadline
    \/ Restart

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ processEpoch \in 0..1
    /\ request \in Requests \cup {NoRequest}
    /\ phase \in Phases
    /\ scheduled \subseteq ReplyRecords
    /\ delivered \subseteq ReplyRecords
    /\ collected \in ReplyRecords \cup {NoReply}
    /\ quietBoundary \in BOOLEAN
    /\ grabValidated \in BOOLEAN
    /\ outcome \in Outcomes
    /\ emitted \subseteq EmittedRecords

EmittedReplyMatchesRequest ==
    \A reply \in emitted : reply.origin = reply.response

DeadlineCannotSucceed ==
    outcome = "success" => quietBoundary

GrabbedQuietNoOpIsAuthorized ==
    (outcome = "success" /\ collected = NoReply) => grabValidated

ReadyHasNoLateReply ==
    phase = "ready" => /\ scheduled = {}
                        /\ delivered = {}

FailedRuntimeIsQuarantined ==
    phase = "failed" => /\ request = NoRequest
                         /\ outcome = "failed"

=============================================================================
