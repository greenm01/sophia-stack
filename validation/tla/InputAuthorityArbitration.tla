--------------------- MODULE InputAuthorityArbitration ---------------------
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************
 * Intended coexistence contract for application route leases and future    *
 * shell capture. Every ordinary selection uses the last-presented choice.  *
 * A frontend grab is reduced to an Engine-visible profile-scoped lease;     *
 * secure transitions revoke all delivery without waiting for an ack.        *
 *************************************************************************)

CONSTANTS Seats, Authorities, Profiles, Ids, Kinds, MaxScene, MaxEpoch,
          MaxEvents

ASSUME /\ Seats # {} /\ IsFiniteSet(Seats)
       /\ Authorities # {} /\ IsFiniteSet(Authorities)
       /\ Profiles = {"confined", "classic-shared"}
       /\ Ids # {} /\ IsFiniteSet(Ids)
       /\ Kinds = {"none", "app", "shell"}
       /\ MaxScene >= 2 /\ MaxEpoch >= 2 /\ MaxEvents >= 1

DefaultAuthority == CHOOSE authority \in Authorities : TRUE
DefaultProfile == CHOOSE profile \in Profiles : TRUE
DefaultId == CHOOSE id \in Ids : TRUE

NoChoice == [kind |-> "none", authority |-> DefaultAuthority,
    profile |-> DefaultProfile, id |-> DefaultId, generation |-> 0]

NoLease == [state |-> "none", authority |-> DefaultAuthority,
    profile |-> DefaultProfile, id |-> DefaultId, generation |-> 0,
    presented |-> 0]

NoShellCapture == [live |-> FALSE, authority |-> DefaultAuthority,
    id |-> DefaultId, generation |-> 0, presented |-> 0]

VARIABLES scenes, committedScene, submittedScene, presentedScene,
          authorityLive, authoritySession, secureActive, controlEpoch,
          lease, shellCapture, reservedShortcut, routed, queue, delivered,
          nextSerial, releaseRequests, releaseAcks, securityTransitions

vars == <<scenes, committedScene, submittedScene, presentedScene,
          authorityLive, authoritySession, secureActive, controlEpoch,
          lease, shellCapture, reservedShortcut, routed, queue, delivered,
          nextSerial, releaseRequests, releaseAcks, securityTransitions>>

Init ==
    /\ scenes = [scene \in 0..MaxScene |-> NoChoice]
    /\ committedScene = 0 /\ submittedScene = 0 /\ presentedScene = 0
    /\ authorityLive = [authority \in Authorities |-> TRUE]
    /\ authoritySession = [authority \in Authorities |-> 1]
    /\ secureActive = FALSE /\ controlEpoch = 1
    /\ lease = [seat \in Seats |-> NoLease]
    /\ shellCapture = [seat \in Seats |-> NoShellCapture]
    /\ reservedShortcut = [seat \in Seats |-> FALSE]
    /\ routed = <<>> /\ queue = <<>> /\ delivered = <<>>
    /\ nextSerial = 1 /\ releaseRequests = 0 /\ releaseAcks = 0
    /\ securityTransitions = 0

ChoiceTemplate(template, generation) ==
    [kind |-> IF template = 4 THEN "shell"
              ELSE IF template = 5 THEN "none" ELSE "app",
     authority |-> IF template \in {2, 3} THEN 2 ELSE 1,
     profile |-> IF template = 3 THEN "classic-shared" ELSE "confined",
     id |-> DefaultId, generation |-> generation]

CommitChoice(template) ==
    LET old == scenes[committedScene] IN
    LET choice == ChoiceTemplate(template, old.generation + 1) IN
    /\ committedScene < MaxScene /\ template \in 1..5
    /\ authorityLive[choice.authority]
    /\ scenes' = [scenes EXCEPT ![committedScene + 1] =
         choice]
    /\ committedScene' = committedScene + 1
    /\ UNCHANGED <<submittedScene, presentedScene, authorityLive,
         authoritySession, secureActive, controlEpoch, lease, shellCapture,
         reservedShortcut, routed, queue, delivered, nextSerial,
         releaseRequests, releaseAcks, securityTransitions>>

Submit ==
    /\ committedScene > submittedScene
    /\ submittedScene' = committedScene
    /\ UNCHANGED <<scenes, committedScene, presentedScene, authorityLive,
         authoritySession, secureActive, controlEpoch, lease, shellCapture,
         reservedShortcut, routed, queue, delivered, nextSerial,
         releaseRequests, releaseAcks, securityTransitions>>

Present ==
    /\ submittedScene > presentedScene
    /\ presentedScene' = submittedScene
    /\ shellCapture' = [seat \in Seats |->
         LET capture == shellCapture[seat] IN
         LET choice == scenes[submittedScene] IN
         IF capture.live
              /\ (choice.kind # "shell" \/ choice.authority # capture.authority
                  \/ choice.id # capture.id
                  \/ choice.generation # capture.generation)
         THEN NoShellCapture ELSE capture]
    /\ UNCHANGED <<scenes, committedScene, submittedScene, authorityLive,
         authoritySession, secureActive, controlEpoch, lease,
         reservedShortcut, routed, queue, delivered, nextSerial,
         releaseRequests, releaseAcks, securityTransitions>>

RouteRecord(source, seat, choice, routeAuthority, routeProfile) ==
    [serial |-> nextSerial, epoch |-> controlEpoch, source |-> source,
     seat |-> seat, kind |-> choice.kind, authority |-> choice.authority,
     profile |-> choice.profile, id |-> choice.id,
     generation |-> choice.generation, presented |-> presentedScene,
     routeAuthority |-> routeAuthority, routeProfile |-> routeProfile,
     leaseState |-> lease[seat].state, shortcut |-> reservedShortcut[seat],
     secure |-> secureActive]

Emit(source, seat, choice, routeAuthority, routeProfile) ==
    LET event == RouteRecord(source, seat, choice, routeAuthority, routeProfile) IN
    /\ nextSerial <= MaxEvents
    /\ routed' = Append(routed, event)
    /\ queue' = Append(queue, event)
    /\ nextSerial' = nextSerial + 1

LeaseCovers(current, held) ==
    /\ current.kind = "app" /\ current.profile = held.profile
    /\ (held.profile = "classic-shared"
        \/ current.authority = held.authority)

RouteExistingLease(seat) ==
    LET held == lease[seat] IN
    LET current == scenes[presentedScene] IN
    /\ ~secureActive /\ ~reservedShortcut[seat]
    /\ held.state = "active" /\ LeaseCovers(current, held)
    /\ authorityLive[held.authority]
    /\ Emit("lease", seat, current, held.authority, held.profile)
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch, lease,
         shellCapture, reservedShortcut, delivered, releaseRequests,
         releaseAcks, securityTransitions>>

RequestLeaseRelease(seat) ==
    LET held == lease[seat] IN
    LET current == scenes[presentedScene] IN
    /\ ~secureActive /\ held.state = "active"
    /\ ~LeaseCovers(current, held)
    /\ lease' = [lease EXCEPT ![seat].state = "releasing"]
    /\ releaseRequests' = releaseRequests + 1
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch,
         shellCapture, reservedShortcut, routed, queue, delivered,
         nextSerial, releaseAcks, securityTransitions>>

FrontendReleaseAck(seat) ==
    /\ lease[seat].state = "releasing"
    /\ lease' = [lease EXCEPT ![seat] = NoLease]
    /\ releaseAcks' = releaseAcks + 1
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch,
         shellCapture, reservedShortcut, routed, queue, delivered,
         nextSerial, releaseRequests, securityTransitions>>

ResolveFresh(seat) ==
    LET choice == scenes[presentedScene] IN
    /\ ~secureActive /\ ~reservedShortcut[seat]
    /\ lease[seat].state = "none" /\ ~shellCapture[seat].live
    /\ choice.kind \in {"app", "shell"}
    /\ authorityLive[choice.authority]
    /\ Emit("fresh", seat, choice, choice.authority, choice.profile)
    /\ shellCapture' = IF choice.kind = "shell"
         THEN [shellCapture EXCEPT ![seat] =
              [live |-> TRUE, authority |-> choice.authority,
               id |-> choice.id, generation |-> choice.generation,
               presented |-> presentedScene]]
         ELSE shellCapture
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch, lease,
         reservedShortcut, delivered, releaseRequests, releaseAcks,
         securityTransitions>>

RouteShellCapture(seat) ==
    LET capture == shellCapture[seat] IN
    LET choice == scenes[presentedScene] IN
    /\ ~secureActive /\ ~reservedShortcut[seat]
    /\ lease[seat].state = "none" /\ capture.live
    /\ choice.kind = "shell" /\ choice.authority = capture.authority
    /\ choice.id = capture.id /\ choice.generation = capture.generation
    /\ Emit("shell-capture", seat, choice,
            choice.authority, choice.profile)
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch, lease,
         shellCapture, reservedShortcut, delivered, releaseRequests,
         releaseAcks, securityTransitions>>

EndShellCapture(seat) ==
    /\ shellCapture[seat].live
    /\ shellCapture' = [shellCapture EXCEPT ![seat] = NoShellCapture]
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch, lease,
         reservedShortcut, routed, queue, delivered, nextSerial,
         releaseRequests, releaseAcks, securityTransitions>>

RequestFrontendGrab(seat) ==
    LET choice == scenes[presentedScene] IN
    /\ ~secureActive /\ lease[seat].state = "none"
    /\ ~shellCapture[seat].live /\ choice.kind = "app"
    /\ authorityLive[choice.authority]
    /\ lease' = [lease EXCEPT ![seat] =
         [state |-> "requested", authority |-> choice.authority,
          profile |-> choice.profile, id |-> choice.id,
          generation |-> choice.generation, presented |-> presentedScene]]
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch,
         shellCapture, reservedShortcut, routed, queue, delivered,
         nextSerial, releaseRequests, releaseAcks, securityTransitions>>

AcknowledgeFrontendGrab(seat) ==
    /\ lease[seat].state = "requested"
    /\ authorityLive[lease[seat].authority]
    /\ lease' = [lease EXCEPT ![seat].state = "active"]
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch,
         shellCapture, reservedShortcut, routed, queue, delivered,
         nextSerial, releaseRequests, releaseAcks, securityTransitions>>

ArmShortcut(seat) ==
    /\ ~reservedShortcut[seat]
    /\ reservedShortcut' = [reservedShortcut EXCEPT ![seat] = TRUE]
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch, lease,
         shellCapture, routed, queue, delivered, nextSerial,
         releaseRequests, releaseAcks, securityTransitions>>

ConsumeShortcut(seat) ==
    /\ reservedShortcut[seat]
    /\ reservedShortcut' = [reservedShortcut EXCEPT ![seat] = FALSE]
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch, lease,
         shellCapture, routed, queue, delivered, nextSerial,
         releaseRequests, releaseAcks, securityTransitions>>

SecurityTransition ==
    /\ ~secureActive /\ controlEpoch < MaxEpoch
    /\ secureActive' = TRUE /\ controlEpoch' = controlEpoch + 1
    /\ lease' = [seat \in Seats |-> NoLease]
    /\ shellCapture' = [seat \in Seats |-> NoShellCapture]
    /\ queue' = <<>>
    /\ securityTransitions' = securityTransitions + 1
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, reservedShortcut, routed, delivered,
         nextSerial, releaseRequests, releaseAcks>>

EndSecurityTransition ==
    /\ secureActive /\ secureActive' = FALSE
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, controlEpoch, lease, shellCapture,
         reservedShortcut, routed, queue, delivered, nextSerial,
         releaseRequests, releaseAcks, securityTransitions>>

RevokeAuthority(authority) ==
    /\ authorityLive[authority] /\ authoritySession[authority] < MaxEpoch
    /\ authorityLive' = [authorityLive EXCEPT ![authority] = FALSE]
    /\ authoritySession' = [authoritySession EXCEPT ![authority] = @ + 1]
    /\ lease' = [seat \in Seats |->
         IF lease[seat].authority = authority THEN NoLease ELSE lease[seat]]
    /\ shellCapture' = [seat \in Seats |->
         IF shellCapture[seat].authority = authority
         THEN NoShellCapture ELSE shellCapture[seat]]
    /\ queue' = SelectSeq(queue, LAMBDA event : event.authority # authority)
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         secureActive, controlEpoch, reservedShortcut, routed, delivered,
         nextSerial, releaseRequests, releaseAcks, securityTransitions>>

Drain ==
    /\ Len(queue) > 0 /\ queue[1].epoch = controlEpoch
    /\ delivered' = Append(delivered, Head(queue))
    /\ queue' = Tail(queue)
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch, lease,
         shellCapture, reservedShortcut, routed, nextSerial,
         releaseRequests, releaseAcks, securityTransitions>>

Next ==
    \/ \E template \in 1..5 : CommitChoice(template)
    \/ Submit \/ Present
    \/ \E seat \in Seats :
          RouteExistingLease(seat) \/ RequestLeaseRelease(seat)
          \/ FrontendReleaseAck(seat) \/ ResolveFresh(seat)
          \/ RouteShellCapture(seat) \/ EndShellCapture(seat)
          \/ RequestFrontendGrab(seat) \/ AcknowledgeFrontendGrab(seat)
          \/ ArmShortcut(seat) \/ ConsumeShortcut(seat)
    \/ SecurityTransition \/ EndSecurityTransition
    \/ \E authority \in Authorities : RevokeAuthority(authority)
    \/ Drain

Spec == Init /\ [][Next]_vars

EventType(event) ==
    /\ event.serial \in 1..MaxEvents /\ event.epoch \in 1..MaxEpoch
    /\ event.source \in {"fresh", "lease", "shell-capture"}
    /\ event.seat \in Seats /\ event.kind \in Kinds
    /\ event.authority \in Authorities /\ event.profile \in Profiles
    /\ event.routeAuthority \in Authorities /\ event.routeProfile \in Profiles
    /\ event.leaseState \in {"none", "requested", "active", "releasing"}
    /\ event.shortcut \in BOOLEAN
    /\ event.id \in Ids /\ event.generation \in 0..MaxScene
    /\ event.presented \in 0..MaxScene /\ event.secure \in BOOLEAN

TypeOK ==
    /\ scenes \in [0..MaxScene ->
         [kind : Kinds, authority : Authorities, profile : Profiles,
          id : Ids, generation : 0..MaxScene]]
    /\ committedScene \in 0..MaxScene /\ submittedScene \in 0..MaxScene
    /\ presentedScene \in 0..MaxScene
    /\ authorityLive \in [Authorities -> BOOLEAN]
    /\ authoritySession \in [Authorities -> 1..MaxEpoch]
    /\ secureActive \in BOOLEAN /\ controlEpoch \in 1..MaxEpoch
    /\ lease \in [Seats ->
         [state : {"none", "requested", "active", "releasing"},
          authority : Authorities, profile : Profiles, id : Ids,
          generation : 0..MaxScene, presented : 0..MaxScene]]
    /\ shellCapture \in [Seats ->
         [live : BOOLEAN, authority : Authorities, id : Ids,
          generation : 0..MaxScene, presented : 0..MaxScene]]
    /\ reservedShortcut \in [Seats -> BOOLEAN]
    /\ Len(routed) <= MaxEvents /\ Len(queue) <= MaxEvents
    /\ Len(delivered) <= MaxEvents
    /\ \A index \in 1..Len(routed) : EventType(routed[index])
    /\ \A index \in 1..Len(queue) : EventType(queue[index])
    /\ \A index \in 1..Len(delivered) : EventType(delivered[index])
    /\ nextSerial \in 1..(MaxEvents + 1)
    /\ releaseRequests \in Nat /\ releaseAcks \in Nat
    /\ securityTransitions \in Nat

SceneLedgerOrdered ==
    presentedScene <= submittedScene /\ submittedScene <= committedScene

RoutesUsePresentedChoice ==
    \A index \in 1..Len(routed) :
        LET event == routed[index] IN
        LET choice == scenes[event.presented] IN
        /\ event.kind = choice.kind /\ event.authority = choice.authority
        /\ event.profile = choice.profile /\ event.id = choice.id
        /\ event.generation = choice.generation

ApplicationLeasesAreProfileScoped ==
    \A index \in 1..Len(routed) : routed[index].source = "lease" =>
        /\ routed[index].kind = "app"
        /\ routed[index].leaseState = "active"
        /\ routed[index].profile = routed[index].routeProfile
        /\ (routed[index].routeProfile = "classic-shared"
            \/ routed[index].authority = routed[index].routeAuthority)

ShellAndApplicationCaptureAreExclusive ==
    \A seat \in Seats :
        shellCapture[seat].live => lease[seat].state = "none"

SecurityStateHasNoCaptureOrQueuedInput ==
    secureActive =>
        /\ \A seat \in Seats :
             lease[seat].state = "none" /\ ~shellCapture[seat].live
        /\ Len(queue) = 0

NoRoutedShortcutOrSecureEvent ==
    \A index \in 1..Len(routed) :
        /\ ~routed[index].secure /\ ~routed[index].shortcut
        /\ (routed[index].kind = "shell" =>
             routed[index].leaseState = "none")

QueuedInputUsesCurrentControlEpoch ==
    \A index \in 1..Len(queue) : queue[index].epoch = controlEpoch

ShellWaitsForFrontendRelease == releaseAcks <= releaseRequests

=============================================================================
