------------------------ MODULE TargetResolvedInput ------------------------
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************
 * Pre-schema model for shell target resolution. Scenes are immutable once  *
 * committed, presentation advances independently per output, and emitted   *
 * records retain the facts needed to check their authorization later.      *
 * The shell runtime does not exist yet, so these actions model the ratified *
 * contract in docs/target-resolved-input.md rather than production code.    *
 *************************************************************************)

CONSTANTS Seats, Devices, Outputs, Authorities, Slots, Grants,
          MaxScene, MaxGeneration, MaxAction, MaxPoint, MaxScope,
          MaxContact, MaxGrantVersion, MaxPrecision, MaxRate, MaxEvents

ASSUME /\ Seats # {} /\ IsFiniteSet(Seats)
       /\ Devices # {} /\ IsFiniteSet(Devices)
       /\ Outputs # {} /\ IsFiniteSet(Outputs)
       /\ Authorities # {} /\ IsFiniteSet(Authorities)
       /\ Slots # {} /\ IsFiniteSet(Slots)
       /\ Grants # {} /\ IsFiniteSet(Grants)
       /\ MaxScene \in (Nat \ {0}) /\ MaxGeneration \in (Nat \ {0})
       /\ MaxAction \in (Nat \ {0}) /\ MaxPoint \in (Nat \ {0})
       /\ MaxScope \in (Nat \ {0}) /\ MaxContact \in (Nat \ {0})
       /\ MaxGrantVersion \in (Nat \ {0})
       /\ MaxPrecision \in (Nat \ {0}) /\ MaxRate \in (Nat \ {0})
       /\ MaxEvents \in (Nat \ {0})

DefaultAuthority == CHOOSE authority \in Authorities : TRUE
DefaultOutput == CHOOSE output \in Outputs : TRUE
DefaultSlot == CHOOSE slot \in Slots : TRUE
DefaultGrant == CHOOSE grant \in Grants : TRUE
DefaultSeat == CHOOSE seat \in Seats : TRUE

\* The focused configuration uses one seat and numeric slots. Keeping these
\* relations explicit still lets two devices challenge contact correlation.
DeviceSeat(device) == DefaultSeat
SlotZ(slot) == slot
OwnedPixels == {<<1, 1, 1>>, <<1, 1, 2>>, <<2, 1, 3>>}

EmptyTarget == [live |-> FALSE, authority |-> DefaultAuthority, session |-> 0,
    generation |-> 0, action |-> 0, output |-> DefaultOutput, lo |-> 1,
    hi |-> 1, z |-> 0, scope |-> 1, eligible |-> FALSE, grant |-> DefaultGrant]
EmptyScene == [slot \in Slots |-> EmptyTarget]

EmptyCapture == [live |-> FALSE, token |-> 0, slot |-> DefaultSlot,
    authority |-> DefaultAuthority, session |-> 0, generation |-> 0,
    action |-> 0, output |-> DefaultOutput, scope |-> 0,
    device |-> CHOOSE device \in Devices : TRUE, contact |-> 0,
    point |-> 1, grant |-> DefaultGrant, grantVersion |-> 0]

TargetId(target, slot) ==
    <<target.authority, target.session, slot, target.generation>>

Overlaps(left, right) ==
    /\ left.output = right.output
    /\ left.lo <= right.hi /\ right.lo <= left.hi

TargetOwnsPixels(target) ==
    \A point \in target.lo..target.hi :
        <<target.authority, target.output, point>> \in OwnedPixels

SceneWellFormed(scene) ==
    /\ \A slot \in Slots :
        LET target == scene[slot] IN
        target.live =>
            /\ target.session > 0 /\ target.generation > 0
            /\ target.action \in 1..MaxAction
            /\ target.lo \in 1..MaxPoint /\ target.hi \in target.lo..MaxPoint
            /\ target.z \in 1..Cardinality(Slots)
            /\ target.scope \in 1..MaxScope
            /\ (target.eligible => TargetOwnsPixels(target))
    /\ \A left, right \in Slots :
        left # right /\ scene[left].live /\ scene[right].live
            /\ scene[left].eligible /\ scene[right].eligible
            /\ Overlaps(scene[left], scene[right])
            => scene[left].z # scene[right].z

EligibleAt(scene, slot, output, point, activeScope, authorityLive,
           authoritySession, outputLive) ==
    LET target == scene[slot] IN
    /\ target.live /\ target.eligible
    /\ target.output = output /\ outputLive[output]
    /\ target.lo <= point /\ point <= target.hi
    /\ target.scope = activeScope
    /\ authorityLive[target.authority]
    /\ target.session = authoritySession[target.authority]

TopTarget(scene, slot, output, point, activeScope, authorityLive,
          authoritySession, outputLive) ==
    /\ EligibleAt(scene, slot, output, point, activeScope, authorityLive,
                  authoritySession, outputLive)
    /\ \A other \in Slots :
        EligibleAt(scene, other, output, point, activeScope, authorityLive,
                   authoritySession, outputLive)
            => scene[other].z <= scene[slot].z

CaptureMatchesScene(cap, scene, activeScope, authorityLive,
                    authoritySession, seatLive, deviceLive, outputLive) ==
    LET target == scene[cap.slot] IN
    /\ cap.live /\ seatLive[DeviceSeat(cap.device)] /\ deviceLive[cap.device]
    /\ outputLive[cap.output]
    /\ TopTarget(scene, cap.slot, cap.output, cap.point, activeScope,
                  authorityLive, authoritySession, outputLive)
    /\ TargetId(target, cap.slot) =
        <<cap.authority, cap.session, cap.slot, cap.generation>>
    /\ target.action = cap.action /\ target.output = cap.output
    /\ target.scope = cap.scope /\ cap.scope = activeScope
    /\ authorityLive[cap.authority]
    /\ authoritySession[cap.authority] = cap.session

VARIABLES scenes, committedScene, submittedScene, presentedScene,
          authorityLive, authoritySession, seatLive, deviceLive, outputLive,
          activeScope, capture, nextToken, cancelledTokens, cancelCount,
          activationEvents,
          releaseAttempts, grantLive, grantVersion, grantAuthority,
          grantSession, grantSlot, grantGeneration, grantOutput, grantLo,
          grantHi, grantPrecision, grantRemaining, coordinateEvents

vars == <<scenes, committedScene, submittedScene, presentedScene,
          authorityLive, authoritySession, seatLive, deviceLive, outputLive,
          activeScope, capture, nextToken, cancelledTokens, cancelCount,
          activationEvents,
          releaseAttempts, grantLive, grantVersion, grantAuthority,
          grantSession, grantSlot, grantGeneration, grantOutput, grantLo,
          grantHi, grantPrecision, grantRemaining, coordinateEvents>>

Init ==
    /\ scenes = [scene \in 0..MaxScene |-> EmptyScene]
    /\ committedScene = 0 /\ submittedScene = 0
    /\ presentedScene = [output \in Outputs |-> 0]
    /\ authorityLive = [authority \in Authorities |-> TRUE]
    /\ authoritySession = [authority \in Authorities |-> 1]
    /\ seatLive = [seat \in Seats |-> TRUE]
    /\ deviceLive = [device \in Devices |-> TRUE]
    /\ outputLive = [output \in Outputs |-> TRUE]
    /\ activeScope = 1
    /\ capture = [seat \in Seats |-> EmptyCapture]
    /\ nextToken = 1 /\ cancelledTokens = {}
    /\ cancelCount = [token \in 1..MaxEvents |-> 0]
    /\ activationEvents = <<>> /\ releaseAttempts = 0
    /\ grantLive = [grant \in Grants |-> FALSE]
    /\ grantVersion = [grant \in Grants |-> 0]
    /\ grantAuthority = [grant \in Grants |-> DefaultAuthority]
    /\ grantSession = [grant \in Grants |-> 0]
    /\ grantSlot = [grant \in Grants |-> DefaultSlot]
    /\ grantGeneration = [grant \in Grants |-> 0]
    /\ grantOutput = [grant \in Grants |-> DefaultOutput]
    /\ grantLo = [grant \in Grants |-> 1]
    /\ grantHi = [grant \in Grants |-> 1]
    /\ grantPrecision = [grant \in Grants |-> 1]
    /\ grantRemaining = [grant \in Grants |-> 0]
    /\ coordinateEvents = <<>>

(***************************************************************************
 * Four small templates cover owned targets, a second authority, an          *
 * ineligible foreign overlay, and a modal-scope replacement without         *
 * multiplying equivalent geometry states. A replacement always allocates    *
 * the next generation for its slot, including across A->B->A changes.        *
 *************************************************************************)
TemplateTarget(old, slot, template) ==
    LET authority == IF template = 2 THEN 2 ELSE 1 IN
    LET lo == IF template = 2 \/ template = 3 THEN 3 ELSE 1 IN
    LET hi == IF template = 2 \/ template = 3 THEN 3 ELSE 2 IN
    [live |-> TRUE, authority |-> authority,
     session |-> authoritySession[authority],
     generation |-> old.generation + 1, action |-> 1,
     output |-> 1, lo |-> lo, hi |-> hi, z |-> SlotZ(slot),
     scope |-> IF template = 4 THEN 2 ELSE 1,
     eligible |-> template # 3, grant |-> DefaultGrant]

CommitTarget(slot, template) ==
    LET old == scenes[committedScene][slot] IN
    LET fresh == TemplateTarget(old, slot, template) IN
    LET candidate == [scenes[committedScene] EXCEPT ![slot] = fresh] IN
    /\ committedScene < MaxScene /\ old.generation < MaxGeneration
    /\ slot = 1 /\ template \in {1, 3, 4}
    /\ authorityLive[fresh.authority]
    /\ SceneWellFormed(candidate)
    /\ scenes' = [scenes EXCEPT ![committedScene + 1] = candidate]
    /\ committedScene' = committedScene + 1
    /\ UNCHANGED <<submittedScene, presentedScene, authorityLive,
         authoritySession, seatLive, deviceLive, outputLive, activeScope,
         capture, nextToken, cancelledTokens, cancelCount, activationEvents,
         releaseAttempts, grantLive, grantVersion, grantAuthority,
         grantSession, grantSlot, grantGeneration, grantOutput, grantLo,
         grantHi, grantPrecision, grantRemaining, coordinateEvents>>

CommitOverlap ==
    LET first == TemplateTarget(scenes[0][1], 1, 1) IN
    LET second == [TemplateTarget(scenes[0][2], 2, 1) EXCEPT
         !.lo = 2, !.hi = 2] IN
    LET candidate == [scenes[0] EXCEPT ![1] = first, ![2] = second] IN
    /\ committedScene = 0 /\ SceneWellFormed(candidate)
    /\ scenes' = [scenes EXCEPT ![1] = candidate]
    /\ committedScene' = 1
    /\ UNCHANGED <<submittedScene, presentedScene, authorityLive,
         authoritySession, seatLive, deviceLive, outputLive, activeScope,
         capture, nextToken, cancelledTokens, cancelCount, activationEvents,
         releaseAttempts, grantLive, grantVersion, grantAuthority,
         grantSession, grantSlot, grantGeneration, grantOutput, grantLo,
         grantHi, grantPrecision, grantRemaining, coordinateEvents>>

CommitVisualOnly ==
    /\ committedScene < MaxScene
    /\ scenes' = [scenes EXCEPT
         ![committedScene + 1] = scenes[committedScene]]
    /\ committedScene' = committedScene + 1
    /\ UNCHANGED <<submittedScene, presentedScene, authorityLive,
         authoritySession, seatLive, deviceLive, outputLive, activeScope,
         capture, nextToken, cancelledTokens, cancelCount, activationEvents,
         releaseAttempts, grantLive, grantVersion, grantAuthority,
         grantSession, grantSlot, grantGeneration, grantOutput, grantLo,
         grantHi, grantPrecision, grantRemaining, coordinateEvents>>

CommitRemoval(slot) ==
    LET old == scenes[committedScene][slot] IN
    LET removed == [old EXCEPT !.live = FALSE, !.eligible = FALSE] IN
    LET candidate == [scenes[committedScene] EXCEPT ![slot] = removed] IN
    /\ committedScene < MaxScene /\ old.live /\ slot = 1
    /\ scenes' = [scenes EXCEPT ![committedScene + 1] = candidate]
    /\ committedScene' = committedScene + 1
    /\ UNCHANGED <<submittedScene, presentedScene, authorityLive,
         authoritySession, seatLive, deviceLive, outputLive, activeScope,
         capture, nextToken, cancelledTokens, cancelCount, activationEvents,
         releaseAttempts, grantLive, grantVersion, grantAuthority,
         grantSession, grantSlot, grantGeneration, grantOutput, grantLo,
         grantHi, grantPrecision, grantRemaining, coordinateEvents>>

Submit ==
    /\ committedScene > submittedScene
    /\ submittedScene' = committedScene
    /\ UNCHANGED <<scenes, committedScene, presentedScene, authorityLive,
         authoritySession, seatLive, deviceLive, outputLive, activeScope,
         capture, nextToken, cancelledTokens, cancelCount, activationEvents,
         releaseAttempts, grantLive, grantVersion, grantAuthority,
         grantSession, grantSlot, grantGeneration, grantOutput, grantLo,
         grantHi, grantPrecision, grantRemaining, coordinateEvents>>

InvalidatedOn(output, scene) ==
    {seat \in Seats : capture[seat].live /\ capture[seat].output = output
        /\ ~CaptureMatchesScene(capture[seat], scenes[scene], activeScope,
              authorityLive, authoritySession, seatLive, deviceLive, outputLive)}

Present(output) ==
    LET invalid == InvalidatedOn(output, submittedScene) IN
    /\ outputLive[output] /\ submittedScene > presentedScene[output]
    /\ presentedScene' = [presentedScene EXCEPT ![output] = submittedScene]
    /\ capture' = [seat \in Seats |->
         IF seat \in invalid THEN EmptyCapture ELSE capture[seat]]
    /\ cancelledTokens' = cancelledTokens
         \cup {capture[seat].token : seat \in invalid}
    /\ cancelCount' = [token \in 1..MaxEvents |-> cancelCount[token]
         + Cardinality({seat \in invalid : capture[seat].token = token})]
    /\ UNCHANGED <<scenes, committedScene, submittedScene, authorityLive,
         authoritySession, seatLive, deviceLive, outputLive, activeScope,
         nextToken, activationEvents, releaseAttempts, grantLive,
         grantVersion, grantAuthority, grantSession, grantSlot,
         grantGeneration, grantOutput, grantLo, grantHi, grantPrecision,
         grantRemaining, coordinateEvents>>

Press(seat, device, contact, slot, output, point) ==
    LET scene == scenes[presentedScene[output]] IN
    LET target == scene[slot] IN
    LET grantVer == IF grantLive[target.grant] THEN grantVersion[target.grant] ELSE 0 IN
    /\ DeviceSeat(device) = seat /\ seatLive[seat] /\ deviceLive[device]
    /\ contact \in 1..MaxContact /\ capture[seat].live = FALSE
    /\ TopTarget(scene, slot, output, point, activeScope, authorityLive,
                 authoritySession, outputLive)
    /\ nextToken <= MaxEvents
    /\ capture' = [capture EXCEPT ![seat] =
         [live |-> TRUE, token |-> nextToken, slot |-> slot,
          authority |-> target.authority, session |-> target.session,
          generation |-> target.generation, action |-> target.action,
          output |-> output, scope |-> target.scope, device |-> device,
          contact |-> contact, point |-> point, grant |-> target.grant,
          grantVersion |-> grantVer]]
    /\ nextToken' = nextToken + 1
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, seatLive, deviceLive, outputLive,
         activeScope, cancelledTokens, cancelCount, activationEvents,
         releaseAttempts,
         grantLive, grantVersion, grantAuthority, grantSession, grantSlot,
         grantGeneration, grantOutput, grantLo, grantHi, grantPrecision,
         grantRemaining, coordinateEvents>>

CanActivate(seat, device, contact) ==
    LET cap == capture[seat] IN
    /\ cap.live /\ cap.device = device /\ cap.contact = contact
    /\ DeviceSeat(device) = seat
    /\ CaptureMatchesScene(cap, scenes[presentedScene[cap.output]],
         activeScope, authorityLive, authoritySession, seatLive,
         deviceLive, outputLive)

Release(seat, device, contact) ==
    LET cap == capture[seat] IN
    LET valid == CanActivate(seat, device, contact) IN
    /\ cap.live /\ releaseAttempts < MaxEvents
    /\ releaseAttempts' = releaseAttempts + 1
    /\ activationEvents' = IF valid
         THEN Append(activationEvents,
              [token |-> cap.token, seat |-> seat, device |-> device,
               contact |-> contact, slot |-> cap.slot,
               captureDevice |-> cap.device, captureContact |-> cap.contact,
               authority |-> cap.authority, session |-> cap.session,
               generation |-> cap.generation, action |-> cap.action,
               output |-> cap.output, point |-> cap.point,
               presented |-> presentedScene[cap.output],
               authorityLive |-> authorityLive[cap.authority],
               seatLive |-> seatLive[seat], deviceLive |-> deviceLive[device],
               top |-> TopTarget(scenes[presentedScene[cap.output]], cap.slot,
                         cap.output, cap.point, activeScope, authorityLive,
                         authoritySession, outputLive)])
         ELSE activationEvents
    /\ capture' = IF valid
         THEN [capture EXCEPT ![seat] = EmptyCapture] ELSE capture
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, seatLive, deviceLive, outputLive,
         activeScope, nextToken, cancelledTokens, cancelCount, grantLive,
         grantVersion,
         grantAuthority, grantSession, grantSlot, grantGeneration,
         grantOutput, grantLo, grantHi, grantPrecision, grantRemaining,
         coordinateEvents>>

CancelSeats(seats) ==
    /\ capture' = [seat \in Seats |->
         IF seat \in seats THEN EmptyCapture ELSE capture[seat]]
    /\ cancelledTokens' = cancelledTokens
         \cup {capture[seat].token :
              seat \in {candidate \in seats : capture[candidate].live}}
    /\ cancelCount' = [token \in 1..MaxEvents |-> cancelCount[token]
         + Cardinality({seat \in seats :
              capture[seat].live /\ capture[seat].token = token})]

LoseSeat(seat) ==
    LET affected == {seat} IN
    /\ seatLive[seat]
    /\ seatLive' = [seatLive EXCEPT ![seat] = FALSE]
    /\ CancelSeats(affected)
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, deviceLive, outputLive, activeScope,
         nextToken, activationEvents, releaseAttempts, grantLive,
         grantVersion, grantAuthority, grantSession, grantSlot,
         grantGeneration, grantOutput, grantLo, grantHi, grantPrecision,
         grantRemaining, coordinateEvents>>

LoseDevice(device) ==
    LET affected == {seat \in Seats : capture[seat].live
        /\ capture[seat].device = device} IN
    /\ deviceLive[device]
    /\ deviceLive' = [deviceLive EXCEPT ![device] = FALSE]
    /\ CancelSeats(affected)
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, seatLive, outputLive, activeScope,
         nextToken, activationEvents, releaseAttempts, grantLive,
         grantVersion, grantAuthority, grantSession, grantSlot,
         grantGeneration, grantOutput, grantLo, grantHi, grantPrecision,
         grantRemaining, coordinateEvents>>

LoseOutput(output) ==
    LET affected == {seat \in Seats : capture[seat].live
        /\ capture[seat].output = output} IN
    /\ outputLive[output]
    /\ outputLive' = [outputLive EXCEPT ![output] = FALSE]
    /\ CancelSeats(affected)
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, seatLive, deviceLive, activeScope,
         nextToken, activationEvents, releaseAttempts, grantLive,
         grantVersion, grantAuthority, grantSession, grantSlot,
         grantGeneration, grantOutput, grantLo, grantHi, grantPrecision,
         grantRemaining, coordinateEvents>>

LoseAuthority(authority) ==
    LET affected == {seat \in Seats : capture[seat].live
        /\ capture[seat].authority = authority} IN
    /\ authorityLive[authority] /\ authoritySession[authority] < MaxGeneration
    /\ authorityLive' = [authorityLive EXCEPT ![authority] = FALSE]
    /\ authoritySession' = [authoritySession EXCEPT ![authority] = @ + 1]
    /\ grantLive' = [grant \in Grants |->
         IF grantAuthority[grant] = authority THEN FALSE ELSE grantLive[grant]]
    /\ CancelSeats(affected)
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         seatLive, deviceLive, outputLive, activeScope, nextToken,
         activationEvents, releaseAttempts, grantVersion, grantAuthority,
         grantSession, grantSlot, grantGeneration, grantOutput, grantLo,
         grantHi, grantPrecision, grantRemaining, coordinateEvents>>

RestoreAuthority(authority) ==
    /\ ~authorityLive[authority]
    /\ authorityLive' = [authorityLive EXCEPT ![authority] = TRUE]
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authoritySession, seatLive, deviceLive, outputLive, activeScope,
         capture, nextToken, cancelledTokens, cancelCount, activationEvents,
         releaseAttempts, grantLive, grantVersion, grantAuthority,
         grantSession, grantSlot, grantGeneration, grantOutput, grantLo,
         grantHi, grantPrecision, grantRemaining, coordinateEvents>>

ChangeScope(scope) ==
    LET affected == {seat \in Seats : capture[seat].live
        /\ capture[seat].scope # scope} IN
    /\ scope \in 1..MaxScope /\ scope # activeScope
    /\ activeScope' = scope
    /\ CancelSeats(affected)
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, seatLive, deviceLive, outputLive,
         nextToken, activationEvents, releaseAttempts, grantLive,
         grantVersion, grantAuthority, grantSession, grantSlot,
         grantGeneration, grantOutput, grantLo, grantHi, grantPrecision,
         grantRemaining, coordinateEvents>>

(***************************************************************************
 * Only the independent session policy issues a grant. The target merely    *
 * names it. The grant is immutable for one version and is narrower than the *
 * target that motivated it.                                                 *
 *************************************************************************)
IssueGrant(grant, slot, lo, hi, precision) ==
    LET target == scenes[committedScene][slot] IN
    /\ target.live /\ slot = 1 /\ authorityLive[target.authority]
    /\ ~grantLive[grant] /\ grantVersion[grant] < MaxGrantVersion
    /\ lo \in target.lo..target.hi /\ hi \in lo..target.hi
    /\ precision \in 1..MaxPrecision
    /\ grantLive' = [grantLive EXCEPT ![grant] = TRUE]
    /\ grantVersion' = [grantVersion EXCEPT ![grant] = @ + 1]
    /\ grantAuthority' = [grantAuthority EXCEPT ![grant] = target.authority]
    /\ grantSession' = [grantSession EXCEPT ![grant] = target.session]
    /\ grantSlot' = [grantSlot EXCEPT ![grant] = slot]
    /\ grantGeneration' = [grantGeneration EXCEPT ![grant] = target.generation]
    /\ grantOutput' = [grantOutput EXCEPT ![grant] = target.output]
    /\ grantLo' = [grantLo EXCEPT ![grant] = lo]
    /\ grantHi' = [grantHi EXCEPT ![grant] = hi]
    /\ grantPrecision' = [grantPrecision EXCEPT ![grant] = precision]
    /\ grantRemaining' = [grantRemaining EXCEPT ![grant] = MaxRate]
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, seatLive, deviceLive, outputLive,
         activeScope, capture, nextToken, cancelledTokens, cancelCount,
         activationEvents,
         releaseAttempts, coordinateEvents>>

RevokeGrant(grant) ==
    LET affected == {seat \in Seats : capture[seat].live
        /\ capture[seat].grant = grant
        /\ capture[seat].grantVersion = grantVersion[grant]} IN
    /\ grantLive[grant]
    /\ grantLive' = [grantLive EXCEPT ![grant] = FALSE]
    /\ CancelSeats(affected)
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, seatLive, deviceLive, outputLive,
         activeScope, nextToken, activationEvents, releaseAttempts,
         grantVersion, grantAuthority, grantSession, grantSlot,
         grantGeneration, grantOutput, grantLo, grantHi, grantPrecision,
         grantRemaining, coordinateEvents>>

GrantValidFor(cap, point) ==
    LET grant == cap.grant IN
    /\ grantLive[grant] /\ cap.grantVersion = grantVersion[grant]
    /\ grantAuthority[grant] = cap.authority
    /\ grantSession[grant] = cap.session /\ grantSlot[grant] = cap.slot
    /\ grantGeneration[grant] = cap.generation
    /\ grantOutput[grant] = cap.output
    /\ point \in grantLo[grant]..grantHi[grant]
    /\ grantRemaining[grant] > 0

AttemptCoordinate(seat, device, contact, point) ==
    LET cap == capture[seat] IN
    LET valid == /\ cap.live /\ cap.device = device /\ cap.contact = contact
                 /\ DeviceSeat(device) = seat
                 /\ CaptureMatchesScene(cap, scenes[presentedScene[cap.output]],
                      activeScope, authorityLive, authoritySession, seatLive,
                      deviceLive, outputLive)
                 /\ GrantValidFor(cap, point) IN
    /\ cap.live /\ point \in 1..MaxPoint
    /\ Len(coordinateEvents) < MaxEvents
    /\ coordinateEvents' = IF valid
         THEN Append(coordinateEvents,
              [token |-> cap.token, seat |-> seat, device |-> device,
               contact |-> contact, slot |-> cap.slot,
               authority |-> cap.authority, session |-> cap.session,
               generation |-> cap.generation, output |-> cap.output,
               grant |-> cap.grant, grantVersion |-> cap.grantVersion,
               currentGrantVersion |-> grantVersion[cap.grant],
               grantLive |-> grantLive[cap.grant],
               grantAuthority |-> grantAuthority[cap.grant],
               grantSession |-> grantSession[cap.grant],
               grantSlot |-> grantSlot[cap.grant],
               grantGeneration |-> grantGeneration[cap.grant],
               grantOutput |-> grantOutput[cap.grant],
               grantLo |-> grantLo[cap.grant],
               grantHi |-> grantHi[cap.grant],
               grantRemaining |-> grantRemaining[cap.grant], point |-> point,
               local |-> (point - grantLo[cap.grant]) \div grantPrecision[cap.grant],
               localMax |-> (grantHi[cap.grant] - grantLo[cap.grant])
                            \div grantPrecision[cap.grant],
               authorized |-> valid])
         ELSE coordinateEvents
    /\ grantRemaining' = IF valid
         THEN [grantRemaining EXCEPT ![cap.grant] = @ - 1]
         ELSE grantRemaining
    /\ UNCHANGED <<scenes, committedScene, submittedScene, presentedScene,
         authorityLive, authoritySession, seatLive, deviceLive, outputLive,
         activeScope, capture, nextToken, cancelledTokens, cancelCount,
         activationEvents,
         releaseAttempts, grantLive, grantVersion, grantAuthority,
         grantSession, grantSlot, grantGeneration, grantOutput, grantLo,
         grantHi, grantPrecision>>

Next ==
    \/ \E slot \in Slots, template \in 1..4 : CommitTarget(slot, template)
    \/ CommitOverlap \/ CommitVisualOnly
    \/ \E slot \in Slots : CommitRemoval(slot)
    \/ Submit \/ \E output \in Outputs : Present(output)
    \/ \E seat \in Seats, device \in Devices, contact \in 1..MaxContact,
          slot \in Slots, output \in Outputs, point \in 1..MaxPoint :
          Press(seat, device, contact, slot, output, point)
    \/ \E seat \in Seats, device \in Devices, contact \in 1..MaxContact :
          Release(seat, device, contact)
    \/ \E seat \in Seats : LoseSeat(seat)
    \/ \E device \in Devices : LoseDevice(device)
    \/ \E output \in Outputs : LoseOutput(output)
    \/ LoseAuthority(1)
    \/ \E scope \in 1..MaxScope : ChangeScope(scope)
    \/ \E grant \in Grants, slot \in Slots, lo \in 1..MaxPoint,
          hi \in 1..MaxPoint, precision \in 1..MaxPrecision :
          IssueGrant(grant, slot, lo, hi, precision)
    \/ \E grant \in Grants : RevokeGrant(grant)
    \/ \E seat \in Seats, device \in Devices,
          contact \in 1..MaxContact, point \in 1..MaxPoint :
          AttemptCoordinate(seat, device, contact, point)

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ scenes \in [0..MaxScene -> [Slots ->
         [live : BOOLEAN, authority : Authorities, session : 0..MaxGeneration,
          generation : 0..MaxGeneration, action : 0..MaxAction,
          output : Outputs, lo : 1..MaxPoint, hi : 1..MaxPoint,
          z : 0..Cardinality(Slots), scope : 1..MaxScope,
          eligible : BOOLEAN, grant : Grants]]]
    /\ committedScene \in 0..MaxScene /\ submittedScene \in 0..MaxScene
    /\ presentedScene \in [Outputs -> 0..MaxScene]
    /\ authorityLive \in [Authorities -> BOOLEAN]
    /\ authoritySession \in [Authorities -> 1..MaxGeneration]
    /\ seatLive \in [Seats -> BOOLEAN] /\ deviceLive \in [Devices -> BOOLEAN]
    /\ outputLive \in [Outputs -> BOOLEAN] /\ activeScope \in 1..MaxScope
    /\ capture \in [Seats -> [live : BOOLEAN, token : 0..MaxEvents,
         slot : Slots, authority : Authorities, session : 0..MaxGeneration,
         generation : 0..MaxGeneration, action : 0..MaxAction,
         output : Outputs, scope : 0..MaxScope, device : Devices,
         contact : 0..MaxContact, point : 1..MaxPoint, grant : Grants,
         grantVersion : 0..MaxGrantVersion]]
    /\ nextToken \in 1..(MaxEvents + 1)
    /\ cancelledTokens \subseteq 1..MaxEvents
    /\ cancelCount \in [1..MaxEvents -> 0..MaxEvents]
    /\ Len(activationEvents) <= MaxEvents /\ releaseAttempts \in 0..MaxEvents
    /\ grantLive \in [Grants -> BOOLEAN]
    /\ grantVersion \in [Grants -> 0..MaxGrantVersion]
    /\ grantAuthority \in [Grants -> Authorities]
    /\ grantSession \in [Grants -> 0..MaxGeneration]
    /\ grantSlot \in [Grants -> Slots]
    /\ grantGeneration \in [Grants -> 0..MaxGeneration]
    /\ grantOutput \in [Grants -> Outputs]
    /\ grantLo \in [Grants -> 1..MaxPoint]
    /\ grantHi \in [Grants -> 1..MaxPoint]
    /\ grantPrecision \in [Grants -> 1..MaxPrecision]
    /\ grantRemaining \in [Grants -> 0..MaxRate]
    /\ Len(coordinateEvents) <= MaxEvents

SceneLedgerOrdered ==
    /\ submittedScene <= committedScene
    /\ \A output \in Outputs : presentedScene[output] <= submittedScene

AllCommittedScenesWellFormed ==
    \A scene \in 0..committedScene : SceneWellFormed(scenes[scene])

GenerationsNeverRecycle ==
    \A scene \in 0..(committedScene - 1), slot \in Slots :
        LET previous == scenes[scene][slot] IN
        LET current == scenes[scene + 1][slot] IN
        /\ current.generation >= previous.generation
        /\ (~previous.live /\ current.live =>
             current.generation > previous.generation)
        /\ (previous.live /\ current.live
             /\ <<previous.authority, previous.session, previous.action,
                  previous.output, previous.lo, previous.hi, previous.scope,
                  previous.grant>>
                # <<current.authority, current.session, current.action,
                     current.output, current.lo, current.hi, current.scope,
                     current.grant>>
            => current.generation > previous.generation)

CapturedTargetsArePresented ==
    \A seat \in Seats : capture[seat].live =>
        CaptureMatchesScene(capture[seat],
            scenes[presentedScene[capture[seat].output]], activeScope,
            authorityLive, authoritySession, seatLive, deviceLive, outputLive)

CancelledCaptureCannotActivate ==
    \A index \in 1..Len(activationEvents) :
        activationEvents[index].token \notin cancelledTokens

ActivationsMatchCapturedPresentedTarget ==
    \A index \in 1..Len(activationEvents) :
        LET event == activationEvents[index] IN
        LET target == scenes[event.presented][event.slot] IN
        /\ event.authorityLive /\ event.seatLive /\ event.deviceLive
        /\ event.top
        /\ TargetId(target, event.slot) =
             <<event.authority, event.session, event.slot, event.generation>>
        /\ target.live /\ target.eligible /\ target.action = event.action
        /\ target.output = event.output
        /\ event.device = event.captureDevice
        /\ event.contact = event.captureContact
        /\ DeviceSeat(event.device) = event.seat

CoordinatesAreAuthorizedAndLocal ==
    \A index \in 1..Len(coordinateEvents) :
        LET event == coordinateEvents[index] IN
        /\ event.authorized
        /\ event.grantLive
        /\ event.grantVersion = event.currentGrantVersion
        /\ event.grantAuthority = event.authority
        /\ event.grantSession = event.session
        /\ event.grantSlot = event.slot
        /\ event.grantGeneration = event.generation
        /\ event.grantOutput = event.output
        /\ event.point \in event.grantLo..event.grantHi
        /\ event.grantRemaining > 0
        /\ event.local \in 0..event.localMax
        /\ event.grantVersion > 0
        /\ DeviceSeat(event.device) = event.seat

CancellationOccursAtMostOnce ==
    /\ Cardinality(cancelledTokens) <= MaxEvents
    /\ \A token \in 1..MaxEvents : cancelCount[token] <= 1

=============================================================================
