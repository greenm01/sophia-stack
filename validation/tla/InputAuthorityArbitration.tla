--------------------- MODULE InputAuthorityArbitration ---------------------
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************
 * Intended coexistence contract for application route leases and future    *
 * shell capture. Every ordinary selection uses the last-presented choice.  *
 * A frontend grab is reduced to an Engine-visible profile-scoped lease;     *
 * secure transitions revoke all delivery without waiting for an ack.        *
 *************************************************************************)

CONSTANTS Seats, Authorities, Profiles, Ids, Kinds, MaxScene, MaxEpoch,
          MaxEvents, MaxLeases

ASSUME /\ Seats # {} /\ IsFiniteSet(Seats)
       /\ Authorities # {} /\ IsFiniteSet(Authorities)
       /\ Profiles = {"confined", "classic-shared"}
       /\ Ids # {} /\ IsFiniteSet(Ids)
       /\ Kinds = {"none", "app", "shell"}
       /\ MaxScene >= 2 /\ MaxEpoch >= 2 /\ MaxEvents >= 1
       /\ MaxLeases >= 1

DefaultAuthority == CHOOSE authority \in Authorities : TRUE
DefaultProfile == CHOOSE profile \in Profiles : TRUE
DefaultId == CHOOSE id \in Ids : TRUE

NoChoice == [kind |-> "none", authority |-> DefaultAuthority,
    profile |-> DefaultProfile, id |-> DefaultId, generation |-> 0]

NoLease == [state |-> "none", leaseId |-> 0,
    authority |-> DefaultAuthority, authoritySession |-> 0,
    profile |-> DefaultProfile, id |-> DefaultId, generation |-> 0,
    presented |-> 0, controlEpoch |-> 0, frontendSequence |-> 0]

NoShellCapture == [live |-> FALSE, authority |-> DefaultAuthority,
    id |-> DefaultId, generation |-> 0, presented |-> 0]

VARIABLES scenes, committedScene, submittedScene, presentedScene,
          authorityLive, authoritySession, secureActive, controlEpoch,
          lease, shellCapture, reservedShortcut, routed, queue, delivered,
          nextSerial, nextLeaseId, frontendSequence, releaseRequests,
          releaseAcks, rejectedGrabs, securityTransitions

vars == <<scenes, committedScene, submittedScene, presentedScene,
          authorityLive, authoritySession, secureActive, controlEpoch,
          lease, shellCapture, reservedShortcut, routed, queue, delivered,
          nextSerial, nextLeaseId, frontendSequence, releaseRequests,
          releaseAcks, rejectedGrabs, securityTransitions>>

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
    /\ nextSerial = 1 /\ nextLeaseId = 1
    /\ frontendSequence = [seat \in Seats |-> 0]
    /\ releaseRequests = 0 /\ releaseAcks = 0 /\ rejectedGrabs = 0
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
         nextLeaseId, frontendSequence, releaseRequests, releaseAcks,
         rejectedGrabs, securityTransitions>>

Submit ==
    /\ committedScene > submittedScene
    /\ submittedScene' = committedScene
    /\ UNCHANGED <<scenes, committedScene, presentedScene, authorityLive,
         authoritySession, secureActive, controlEpoch, lease, shellCapture,
         reservedShortcut, routed, queue, delivered, nextSerial,
         nextLeaseId, frontendSequence, releaseRequests, releaseAcks,
         rejectedGrabs, securityTransitions>>

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
         nextLeaseId, frontendSequence, releaseRequests, releaseAcks,
         rejectedGrabs, securityTransitions>>

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
    /\ held.state \in {"provisional", "active"}
    /\ held.controlEpoch = controlEpoch
    /\ held.authoritySession = authoritySession[held.authority]
    /\ LeaseCovers(current, held)
    /\ authorityLive[held.authority]
    /\ Emit("lease", seat, current, held.authority, held.profile)
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch, lease,
         shellCapture, reservedShortcut, delivered, releaseRequests,
         releaseAcks, rejectedGrabs, securityTransitions,
         nextLeaseId, frontendSequence>>

RequestLeaseRelease(seat) ==
    LET held == lease[seat] IN
    LET current == scenes[presentedScene] IN
    /\ ~secureActive /\ held.state \in {"provisional", "active"}
    /\ ~LeaseCovers(current, held)
    /\ lease' = [lease EXCEPT ![seat].state = "releasing"]
    /\ releaseRequests' = releaseRequests + 1
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch,
         shellCapture, reservedShortcut, routed, queue, delivered,
         nextSerial, nextLeaseId, frontendSequence, releaseAcks,
         rejectedGrabs, securityTransitions>>

FrontendReleaseAck(seat) ==
    LET held == lease[seat] IN
    /\ held.state = "releasing"
    /\ held.controlEpoch = controlEpoch
    /\ held.authoritySession = authoritySession[held.authority]
    /\ held.frontendSequence = frontendSequence[seat]
    /\ lease' = [lease EXCEPT ![seat] = NoLease]
    /\ releaseAcks' = releaseAcks + 1
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch,
         shellCapture, reservedShortcut, routed, queue, delivered,
         nextSerial, nextLeaseId, frontendSequence, releaseRequests,
         rejectedGrabs, securityTransitions>>

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
         rejectedGrabs, securityTransitions, nextLeaseId, frontendSequence>>

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
         releaseAcks, rejectedGrabs, securityTransitions,
         nextLeaseId, frontendSequence>>

EndShellCapture(seat) ==
    /\ shellCapture[seat].live
    /\ shellCapture' = [shellCapture EXCEPT ![seat] = NoShellCapture]
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch, lease,
         reservedShortcut, routed, queue, delivered, nextSerial,
         nextLeaseId, frontendSequence, releaseRequests, releaseAcks,
         rejectedGrabs, securityTransitions>>

BeginFrontendGrab(seat) ==
    LET choice == scenes[presentedScene] IN
    /\ ~secureActive /\ lease[seat].state = "none"
    /\ nextLeaseId <= MaxLeases
    /\ ~shellCapture[seat].live /\ choice.kind = "app"
    /\ authorityLive[choice.authority]
    /\ lease' = [lease EXCEPT ![seat] =
         [state |-> "provisional", leaseId |-> nextLeaseId,
          authority |-> choice.authority,
          authoritySession |-> authoritySession[choice.authority],
          profile |-> choice.profile, id |-> choice.id,
          generation |-> choice.generation, presented |-> presentedScene,
          controlEpoch |-> controlEpoch,
          frontendSequence |-> frontendSequence[seat] + 1]]
    /\ nextLeaseId' = nextLeaseId + 1
    /\ frontendSequence' = [frontendSequence EXCEPT ![seat] = @ + 1]
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch,
         shellCapture, reservedShortcut, routed, queue, delivered,
         nextSerial, releaseRequests, releaseAcks, rejectedGrabs,
         securityTransitions>>

ConfirmFrontendGrab(seat) ==
    LET held == lease[seat] IN
    /\ held.state = "provisional"
    /\ held.controlEpoch = controlEpoch
    /\ held.authoritySession = authoritySession[held.authority]
    /\ held.frontendSequence = frontendSequence[seat]
    /\ authorityLive[held.authority]
    /\ lease' = [lease EXCEPT ![seat].state = "active"]
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch,
         shellCapture, reservedShortcut, routed, queue, delivered,
         nextSerial, nextLeaseId, frontendSequence, releaseRequests,
         releaseAcks, rejectedGrabs, securityTransitions>>

RejectFrontendGrab(seat) ==
    LET held == lease[seat] IN
    /\ held.state = "provisional"
    /\ held.controlEpoch = controlEpoch
    /\ held.authoritySession = authoritySession[held.authority]
    /\ held.frontendSequence = frontendSequence[seat]
    /\ lease' = [lease EXCEPT ![seat] = NoLease]
    /\ rejectedGrabs' = rejectedGrabs + 1
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch,
         shellCapture, reservedShortcut, routed, queue, delivered,
         nextSerial, nextLeaseId, frontendSequence, releaseRequests,
         releaseAcks, securityTransitions>>

ArmShortcut(seat) ==
    /\ ~reservedShortcut[seat]
    /\ reservedShortcut' = [reservedShortcut EXCEPT ![seat] = TRUE]
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch, lease,
         shellCapture, routed, queue, delivered, nextSerial,
         nextLeaseId, frontendSequence, releaseRequests, releaseAcks,
         rejectedGrabs, securityTransitions>>

ConsumeShortcut(seat) ==
    /\ reservedShortcut[seat]
    /\ reservedShortcut' = [reservedShortcut EXCEPT ![seat] = FALSE]
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch, lease,
         shellCapture, routed, queue, delivered, nextSerial,
         nextLeaseId, frontendSequence, releaseRequests, releaseAcks,
         rejectedGrabs, securityTransitions>>

SecurityTransition ==
    /\ ~secureActive /\ controlEpoch < MaxEpoch
    /\ secureActive' = TRUE /\ controlEpoch' = controlEpoch + 1
    /\ lease' = [seat \in Seats |-> NoLease]
    /\ shellCapture' = [seat \in Seats |-> NoShellCapture]
    /\ queue' = <<>>
    /\ securityTransitions' = securityTransitions + 1
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, reservedShortcut, routed, delivered,
         nextSerial, nextLeaseId, frontendSequence, releaseRequests,
         releaseAcks, rejectedGrabs>>

EndSecurityTransition ==
    /\ secureActive /\ secureActive' = FALSE
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, controlEpoch, lease, shellCapture,
         reservedShortcut, routed, queue, delivered, nextSerial,
         nextLeaseId, frontendSequence, releaseRequests, releaseAcks,
         rejectedGrabs, securityTransitions>>

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
         nextSerial, nextLeaseId, frontendSequence, releaseRequests,
         releaseAcks, rejectedGrabs, securityTransitions>>

Drain ==
    /\ Len(queue) > 0 /\ queue[1].epoch = controlEpoch
    /\ delivered' = Append(delivered, Head(queue))
    /\ queue' = Tail(queue)
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, secureActive, controlEpoch, lease,
         shellCapture, reservedShortcut, routed, nextSerial,
         nextLeaseId, frontendSequence, releaseRequests, releaseAcks,
         rejectedGrabs, securityTransitions>>

Next ==
    \/ \E template \in 1..5 : CommitChoice(template)
    \/ Submit \/ Present
    \/ \E seat \in Seats :
          RouteExistingLease(seat) \/ RequestLeaseRelease(seat)
          \/ FrontendReleaseAck(seat) \/ ResolveFresh(seat)
          \/ RouteShellCapture(seat) \/ EndShellCapture(seat)
          \/ BeginFrontendGrab(seat) \/ ConfirmFrontendGrab(seat)
          \/ RejectFrontendGrab(seat)
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
    /\ event.leaseState \in {"none", "provisional", "active", "releasing"}
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
         [state : {"none", "provisional", "active", "releasing"},
          leaseId : Nat, authority : Authorities,
          authoritySession : 0..MaxEpoch, profile : Profiles, id : Ids,
          generation : 0..MaxScene, presented : 0..MaxScene,
          controlEpoch : 0..MaxEpoch, frontendSequence : Nat]]
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
    /\ nextLeaseId \in 1..(MaxLeases + 1)
    /\ frontendSequence \in [Seats -> 0..MaxLeases]
    /\ releaseRequests \in Nat /\ releaseAcks \in Nat
    /\ rejectedGrabs \in 0..MaxLeases
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
        /\ routed[index].leaseState \in {"provisional", "active"}
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

LiveLeasesUseCurrentEpochs ==
    \A seat \in Seats : lease[seat].state # "none" =>
        /\ lease[seat].controlEpoch = controlEpoch
        /\ lease[seat].authoritySession =
             authoritySession[lease[seat].authority]

FrontendConfirmationIsExact ==
    \A seat \in Seats : lease[seat].state = "active" =>
        lease[seat].frontendSequence = frontendSequence[seat]

=============================================================================
