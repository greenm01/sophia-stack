--------------------- MODULE ShellDescriptorLifecycle ---------------------
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************
 * First experimental sophia_shell_v1 descriptor-switcher lifecycle. Engine *
 * may retain the last pixels across ordinary shell loss, but targets,       *
 * capture, and activations belong to exact shell and broker epochs.         *
 *************************************************************************)

CONSTANTS Slots, MaxEpoch, MaxGeneration, MaxQueue, MaxActivations, NoSlot

ASSUME /\ Slots \subseteq (Nat \ {0})
       /\ Slots # {}
       /\ MaxEpoch \in (Nat \ {0})
       /\ MaxGeneration \in (Nat \ {0})
       /\ MaxQueue \in (Nat \ {0})
       /\ MaxActivations \in (Nat \ {0})
       /\ NoSlot = 0
       /\ NoSlot \notin Slots

VARIABLES
    connected, shellEpoch, brokerEpoch, revocationEpoch,
    snapshotGeneration, liveSlots,
    candidateGeneration, candidateShellEpoch, candidateBrokerEpoch,
    candidateRevocationEpoch, candidateSnapshot, candidateSlots,
    selectedSlot, candidateReady,
    presentedGeneration, presentedShellEpoch, presentedBrokerEpoch,
    presentedRevocationEpoch, presentedSlots,
    captureSlot, activationQueue, acceptedActivations,
    acknowledgedActivations, nextActivation, revokedEpochs, lastOutcome

vars == <<connected, shellEpoch, brokerEpoch, revocationEpoch,
          snapshotGeneration, liveSlots, candidateGeneration,
          candidateShellEpoch, candidateBrokerEpoch,
          candidateRevocationEpoch, candidateSnapshot, candidateSlots,
          selectedSlot, candidateReady, presentedGeneration,
          presentedShellEpoch, presentedBrokerEpoch,
          presentedRevocationEpoch, presentedSlots, captureSlot,
          activationQueue, acceptedActivations, acknowledgedActivations,
          nextActivation, revokedEpochs, lastOutcome>>

Init ==
    /\ connected = FALSE
    /\ shellEpoch = 0
    /\ brokerEpoch = 1
    /\ revocationEpoch = 1
    /\ snapshotGeneration = 0
    /\ liveSlots = {}
    /\ candidateGeneration = 0
    /\ candidateShellEpoch = 0
    /\ candidateBrokerEpoch = 0
    /\ candidateRevocationEpoch = 0
    /\ candidateSnapshot = 0
    /\ candidateSlots = {}
    /\ selectedSlot = NoSlot
    /\ candidateReady = FALSE
    /\ presentedGeneration = 0
    /\ presentedShellEpoch = 0
    /\ presentedBrokerEpoch = 0
    /\ presentedRevocationEpoch = 0
    /\ presentedSlots = {}
    /\ captureSlot = NoSlot
    /\ activationQueue = <<>>
    /\ acceptedActivations = {}
    /\ acknowledgedActivations = {}
    /\ nextActivation = 1
    /\ revokedEpochs = {}
    /\ lastOutcome = "none"

Connect ==
    /\ ~connected
    /\ shellEpoch < MaxEpoch
    /\ connected' = TRUE
    /\ shellEpoch' = shellEpoch + 1
    /\ candidateGeneration' = 0
    /\ candidateShellEpoch' = 0
    /\ candidateBrokerEpoch' = 0
    /\ candidateRevocationEpoch' = 0
    /\ candidateSnapshot' = 0
    /\ candidateSlots' = {}
    /\ selectedSlot' = NoSlot
    /\ candidateReady' = FALSE
    /\ captureSlot' = NoSlot
    /\ activationQueue' = <<>>
    /\ lastOutcome' = "connected"
    /\ UNCHANGED <<brokerEpoch, revocationEpoch, snapshotGeneration,
                    liveSlots, presentedGeneration, presentedShellEpoch,
                    presentedBrokerEpoch, presentedRevocationEpoch,
                    presentedSlots, acceptedActivations,
                    acknowledgedActivations, nextActivation, revokedEpochs>>

Disconnect ==
    /\ connected
    /\ connected' = FALSE
    /\ revokedEpochs' = revokedEpochs \cup {shellEpoch}
    /\ captureSlot' = NoSlot
    /\ activationQueue' = <<>>
    /\ candidateReady' = FALSE
    /\ lastOutcome' = "disconnected"
    /\ UNCHANGED <<shellEpoch, brokerEpoch, revocationEpoch,
                    snapshotGeneration, liveSlots, candidateGeneration,
                    candidateShellEpoch, candidateBrokerEpoch,
                    candidateRevocationEpoch, candidateSnapshot,
                    candidateSlots, selectedSlot, presentedGeneration,
                    presentedShellEpoch, presentedBrokerEpoch,
                    presentedRevocationEpoch, presentedSlots,
                    acceptedActivations, acknowledgedActivations,
                    nextActivation>>

PublishSnapshot(slots) ==
    /\ connected
    /\ slots \subseteq Slots
    /\ snapshotGeneration < MaxGeneration
    /\ snapshotGeneration' = snapshotGeneration + 1
    /\ liveSlots' = slots
    /\ candidateReady' = FALSE
    /\ captureSlot' = IF captureSlot \in slots THEN captureSlot ELSE NoSlot
    /\ lastOutcome' = "snapshot"
    /\ UNCHANGED <<connected, shellEpoch, brokerEpoch, revocationEpoch,
                    candidateGeneration, candidateShellEpoch,
                    candidateBrokerEpoch, candidateRevocationEpoch,
                    candidateSnapshot, candidateSlots, selectedSlot,
                    presentedGeneration, presentedShellEpoch,
                    presentedBrokerEpoch, presentedRevocationEpoch,
                    presentedSlots, activationQueue, acceptedActivations,
                    acknowledgedActivations, nextActivation, revokedEpochs>>

SubmitCandidate(generation, epoch, broker, revocation, snapshot, slots, selected) ==
    /\ connected
    /\ generation \in 1..MaxGeneration
    /\ epoch \in 1..MaxEpoch
    /\ broker \in 1..MaxEpoch
    /\ revocation \in 1..MaxEpoch
    /\ snapshot \in 0..MaxGeneration
    /\ slots \subseteq Slots
    /\ selected \in slots \cup {NoSlot}
    /\ candidateGeneration' = generation
    /\ candidateShellEpoch' = epoch
    /\ candidateBrokerEpoch' = broker
    /\ candidateRevocationEpoch' = revocation
    /\ candidateSnapshot' = snapshot
    /\ candidateSlots' = slots
    /\ selectedSlot' = selected
    /\ candidateReady' = FALSE
    /\ lastOutcome' = "candidate"
    /\ UNCHANGED <<connected, shellEpoch, brokerEpoch, revocationEpoch,
                    snapshotGeneration, liveSlots, presentedGeneration,
                    presentedShellEpoch, presentedBrokerEpoch,
                    presentedRevocationEpoch, presentedSlots, captureSlot,
                    activationQueue, acceptedActivations,
                    acknowledgedActivations, nextActivation, revokedEpochs>>

ExactCandidate ==
    /\ connected
    /\ candidateGeneration > presentedGeneration
    /\ candidateShellEpoch = shellEpoch
    /\ candidateBrokerEpoch = brokerEpoch
    /\ candidateRevocationEpoch = revocationEpoch
    /\ candidateSnapshot = snapshotGeneration
    /\ candidateSlots \subseteq liveSlots
    /\ (candidateSlots = {} => selectedSlot = NoSlot)
    /\ (candidateSlots # {} => selectedSlot \in candidateSlots)

PrepareCandidate ==
    /\ candidateGeneration # 0
    /\ ~candidateReady
    /\ ExactCandidate
    /\ candidateReady' = TRUE
    /\ lastOutcome' = "prepared"
    /\ UNCHANGED <<connected, shellEpoch, brokerEpoch, revocationEpoch,
                    snapshotGeneration, liveSlots, candidateGeneration,
                    candidateShellEpoch, candidateBrokerEpoch,
                    candidateRevocationEpoch, candidateSnapshot,
                    candidateSlots, selectedSlot, presentedGeneration,
                    presentedShellEpoch, presentedBrokerEpoch,
                    presentedRevocationEpoch, presentedSlots, captureSlot,
                    activationQueue, acceptedActivations,
                    acknowledgedActivations, nextActivation, revokedEpochs>>

RejectCandidate ==
    /\ candidateGeneration # 0
    /\ ~candidateReady
    /\ ~ExactCandidate
    /\ candidateGeneration' = 0
    /\ candidateSlots' = {}
    /\ selectedSlot' = NoSlot
    /\ lastOutcome' = "rejected"
    /\ UNCHANGED <<connected, shellEpoch, brokerEpoch, revocationEpoch,
                    snapshotGeneration, liveSlots, candidateShellEpoch,
                    candidateBrokerEpoch, candidateRevocationEpoch,
                    candidateSnapshot, candidateReady, presentedGeneration,
                    presentedShellEpoch, presentedBrokerEpoch,
                    presentedRevocationEpoch, presentedSlots, captureSlot,
                    activationQueue, acceptedActivations,
                    acknowledgedActivations, nextActivation, revokedEpochs>>

PresentCandidate ==
    /\ candidateReady
    /\ ExactCandidate
    /\ presentedGeneration' = candidateGeneration
    /\ presentedShellEpoch' = candidateShellEpoch
    /\ presentedBrokerEpoch' = candidateBrokerEpoch
    /\ presentedRevocationEpoch' = candidateRevocationEpoch
    /\ presentedSlots' = candidateSlots
    /\ candidateGeneration' = 0
    /\ candidateSlots' = {}
    /\ selectedSlot' = NoSlot
    /\ candidateReady' = FALSE
    /\ captureSlot' = NoSlot
    /\ lastOutcome' = "presented"
    /\ UNCHANGED <<connected, shellEpoch, brokerEpoch, revocationEpoch,
                    snapshotGeneration, liveSlots, candidateShellEpoch,
                    candidateBrokerEpoch, candidateRevocationEpoch,
                    candidateSnapshot, activationQueue, acceptedActivations,
                    acknowledgedActivations, nextActivation, revokedEpochs>>

BeginCapture(slot) ==
    /\ connected
    /\ shellEpoch \notin revokedEpochs
    /\ presentedShellEpoch = shellEpoch
    /\ presentedBrokerEpoch = brokerEpoch
    /\ presentedRevocationEpoch = revocationEpoch
    /\ slot \in presentedSlots \cap liveSlots
    /\ captureSlot = NoSlot
    /\ captureSlot' = slot
    /\ lastOutcome' = "captured"
    /\ UNCHANGED <<connected, shellEpoch, brokerEpoch, revocationEpoch,
                    snapshotGeneration, liveSlots, candidateGeneration,
                    candidateShellEpoch, candidateBrokerEpoch,
                    candidateRevocationEpoch, candidateSnapshot,
                    candidateSlots, selectedSlot, candidateReady,
                    presentedGeneration, presentedShellEpoch,
                    presentedBrokerEpoch, presentedRevocationEpoch,
                    presentedSlots, activationQueue, acceptedActivations,
                    acknowledgedActivations, nextActivation, revokedEpochs>>

ReleaseCapture ==
    /\ captureSlot # NoSlot
    /\ connected
    /\ presentedShellEpoch = shellEpoch
    /\ presentedBrokerEpoch = brokerEpoch
    /\ presentedRevocationEpoch = revocationEpoch
    /\ captureSlot \in presentedSlots \cap liveSlots
    /\ Len(activationQueue) < MaxQueue
    /\ nextActivation <= MaxActivations
    /\ activationQueue' = Append(activationQueue, nextActivation)
    /\ acceptedActivations' = acceptedActivations \cup {nextActivation}
    /\ nextActivation' = nextActivation + 1
    /\ captureSlot' = NoSlot
    /\ lastOutcome' = "activated"
    /\ UNCHANGED <<connected, shellEpoch, brokerEpoch, revocationEpoch,
                    snapshotGeneration, liveSlots, candidateGeneration,
                    candidateShellEpoch, candidateBrokerEpoch,
                    candidateRevocationEpoch, candidateSnapshot,
                    candidateSlots, selectedSlot, candidateReady,
                    presentedGeneration, presentedShellEpoch,
                    presentedBrokerEpoch, presentedRevocationEpoch,
                    presentedSlots, acknowledgedActivations, revokedEpochs>>

Saturate ==
    /\ connected
    /\ captureSlot # NoSlot
    /\ Len(activationQueue) = MaxQueue
    /\ connected' = FALSE
    /\ revokedEpochs' = revokedEpochs \cup {shellEpoch}
    /\ captureSlot' = NoSlot
    /\ activationQueue' = <<>>
    /\ candidateReady' = FALSE
    /\ lastOutcome' = "saturated"
    /\ UNCHANGED <<shellEpoch, brokerEpoch, revocationEpoch,
                    snapshotGeneration, liveSlots, candidateGeneration,
                    candidateShellEpoch, candidateBrokerEpoch,
                    candidateRevocationEpoch, candidateSnapshot,
                    candidateSlots, selectedSlot,
                    presentedGeneration, presentedShellEpoch,
                    presentedBrokerEpoch, presentedRevocationEpoch,
                    presentedSlots, acceptedActivations,
                    acknowledgedActivations, nextActivation>>

Acknowledge ==
    /\ connected
    /\ activationQueue # <<>>
    /\ acknowledgedActivations' =
           acknowledgedActivations \cup {Head(activationQueue)}
    /\ activationQueue' = Tail(activationQueue)
    /\ lastOutcome' = "acknowledged"
    /\ UNCHANGED <<connected, shellEpoch, brokerEpoch, revocationEpoch,
                    snapshotGeneration, liveSlots, candidateGeneration,
                    candidateShellEpoch, candidateBrokerEpoch,
                    candidateRevocationEpoch, candidateSnapshot,
                    candidateSlots, selectedSlot, candidateReady,
                    presentedGeneration, presentedShellEpoch,
                    presentedBrokerEpoch, presentedRevocationEpoch,
                    presentedSlots, captureSlot, acceptedActivations,
                    nextActivation, revokedEpochs>>

RevokeBroker ==
    /\ brokerEpoch < MaxEpoch
    /\ revocationEpoch < MaxEpoch
    /\ brokerEpoch' = brokerEpoch + 1
    /\ revocationEpoch' = revocationEpoch + 1
    /\ liveSlots' = {}
    /\ snapshotGeneration' = 0
    /\ candidateReady' = FALSE
    /\ captureSlot' = NoSlot
    /\ activationQueue' = <<>>
    /\ lastOutcome' = "revoked"
    /\ UNCHANGED <<connected, shellEpoch, candidateGeneration,
                    candidateShellEpoch, candidateBrokerEpoch,
                    candidateRevocationEpoch, candidateSnapshot,
                    candidateSlots, selectedSlot, presentedGeneration,
                    presentedShellEpoch, presentedBrokerEpoch,
                    presentedRevocationEpoch, presentedSlots,
                    acceptedActivations, acknowledgedActivations,
                    nextActivation, revokedEpochs>>

Next ==
    \/ Connect
    \/ Disconnect
    \/ \E slots \in SUBSET Slots : PublishSnapshot(slots)
    \/ \E generation \in 1..MaxGeneration,
          epoch \in 1..MaxEpoch,
          broker \in 1..MaxEpoch,
          revocation \in 1..MaxEpoch,
          snapshot \in 0..MaxGeneration,
          slots \in SUBSET Slots :
             \E selected \in slots \cup {NoSlot} :
                SubmitCandidate(generation, epoch, broker, revocation,
                                snapshot, slots, selected)
    \/ PrepareCandidate
    \/ RejectCandidate
    \/ PresentCandidate
    \/ \E slot \in Slots : BeginCapture(slot)
    \/ ReleaseCapture
    \/ Saturate
    \/ Acknowledge
    \/ RevokeBroker

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ connected \in BOOLEAN
    /\ shellEpoch \in 0..MaxEpoch
    /\ brokerEpoch \in 1..MaxEpoch
    /\ revocationEpoch \in 1..MaxEpoch
    /\ snapshotGeneration \in 0..MaxGeneration
    /\ liveSlots \subseteq Slots
    /\ candidateGeneration \in 0..MaxGeneration
    /\ candidateShellEpoch \in 0..MaxEpoch
    /\ candidateBrokerEpoch \in 0..MaxEpoch
    /\ candidateRevocationEpoch \in 0..MaxEpoch
    /\ candidateSnapshot \in 0..MaxGeneration
    /\ candidateSlots \subseteq Slots
    /\ selectedSlot \in Slots \cup {NoSlot}
    /\ candidateReady \in BOOLEAN
    /\ presentedGeneration \in 0..MaxGeneration
    /\ presentedShellEpoch \in 0..MaxEpoch
    /\ presentedBrokerEpoch \in 0..MaxEpoch
    /\ presentedRevocationEpoch \in 0..MaxEpoch
    /\ presentedSlots \subseteq Slots
    /\ captureSlot \in Slots \cup {NoSlot}
    /\ Len(activationQueue) <= MaxQueue
    /\ acceptedActivations \subseteq 1..MaxActivations
    /\ acknowledgedActivations \subseteq acceptedActivations
    /\ nextActivation \in 1..(MaxActivations + 1)
    /\ revokedEpochs \subseteq 0..MaxEpoch

PreparedCandidatesAreExact == candidateReady => ExactCandidate

CaptureBelongsToCurrentPresentedAuthority ==
    captureSlot # NoSlot =>
        /\ connected
        /\ shellEpoch \notin revokedEpochs
        /\ presentedShellEpoch = shellEpoch
        /\ presentedBrokerEpoch = brokerEpoch
        /\ presentedRevocationEpoch = revocationEpoch
        /\ captureSlot \in presentedSlots \cap liveSlots

QueuedActivationsWereAccepted ==
    {activationQueue[index] : index \in 1..Len(activationQueue)}
        \subseteq acceptedActivations

AcknowledgementsAreAccepted ==
    acknowledgedActivations \subseteq acceptedActivations

RevokedShellEpochIsInert ==
    shellEpoch \in revokedEpochs =>
        /\ ~connected
        /\ captureSlot = NoSlot
        /\ activationQueue = <<>>

=============================================================================
