-------------------- MODULE OutputTopologyLifecycle --------------------
EXTENDS Naturals, Sequences

(***************************************************************************
 * Owner-wide dynamic output-topology replacement. udev emits only a       *
 * replaceable rescan hint. The session owner revokes routed input, retires  *
 * old scanout work, rebuilds a complete native runtime, publishes one      *
 * topology epoch to scanout/pointer/RandR/policy, waits for the current     *
 * policy layout and its first presentation, and only then enables input.   *
 * No-output and bounded rebuild failure remain fail-closed.                *
 *************************************************************************)

CONSTANTS MaxEpoch, MaxEvents, MaxRebuildFailures

ASSUME /\ MaxEpoch \in Nat
       /\ MaxEpoch >= 4
       /\ MaxEvents \in (Nat \ {0})
       /\ MaxRebuildFailures \in Nat

VARIABLES phase, observedEpoch, transitionEpoch, ownedEpoch,
          scanoutEpoch, pointerEpoch, randrEpoch, policyEpoch,
          policyCommittedEpoch, presentedEpoch, inputControlEpoch,
          inputEnabled, noticePending, candidateHasOutput,
          rightLive, rightGeneration, rightEverLost, oldWorkEpoch,
          rebuildFailures, callbackLog, policyLog

vars == <<phase, observedEpoch, transitionEpoch, ownedEpoch,
          scanoutEpoch, pointerEpoch, randrEpoch, policyEpoch,
          policyCommittedEpoch, presentedEpoch, inputControlEpoch,
          inputEnabled, noticePending, candidateHasOutput,
          rightLive, rightGeneration, rightEverLost, oldWorkEpoch,
          rebuildFailures, callbackLog, policyLog>>

TransitionPhases ==
    {"revoked", "quiesced", "rebuilt", "published",
     "policy_committed", "presented"}

RecordBounded(log, entry) ==
    IF Len(log) < MaxEvents THEN Append(log, entry)
    ELSE Append(Tail(log), entry)

Init ==
    /\ phase = "stable"
    /\ observedEpoch = 1
    /\ transitionEpoch = 1
    /\ ownedEpoch = 1
    /\ scanoutEpoch = 1
    /\ pointerEpoch = 1
    /\ randrEpoch = 1
    /\ policyEpoch = 1
    /\ policyCommittedEpoch = 1
    /\ presentedEpoch = 1
    /\ inputControlEpoch = 1
    /\ inputEnabled = TRUE
    /\ noticePending = FALSE
    /\ candidateHasOutput = TRUE
    /\ rightLive = TRUE
    /\ rightGeneration = 1
    /\ rightEverLost = FALSE
    /\ oldWorkEpoch = 0
    /\ rebuildFailures = 0
    /\ callbackLog = <<>>
    /\ policyLog = <<>>

\* LiveDrmTopologyMonitor::enqueue_notice: a capacity-one notice is a rescan
\* hint; later physical facts replace earlier pending facts.
ObserveRightLoss ==
    /\ observedEpoch < MaxEpoch
    /\ rightLive
    /\ observedEpoch' = observedEpoch + 1
    /\ rightLive' = FALSE
    /\ rightEverLost' = TRUE
    /\ candidateHasOutput' = TRUE
    /\ noticePending' = TRUE
    /\ UNCHANGED <<phase, transitionEpoch, ownedEpoch, scanoutEpoch,
                    pointerEpoch, randrEpoch, policyEpoch,
                    policyCommittedEpoch, presentedEpoch, inputControlEpoch,
                    inputEnabled, rightGeneration, oldWorkEpoch,
                    rebuildFailures, callbackLog, policyLog>>

ObserveRightReturn ==
    /\ observedEpoch < MaxEpoch
    /\ ~rightLive
    /\ rightGeneration < MaxEpoch
    /\ observedEpoch' = observedEpoch + 1
    /\ rightLive' = TRUE
    /\ rightGeneration' = rightGeneration + 1
    /\ candidateHasOutput' = TRUE
    /\ noticePending' = TRUE
    /\ UNCHANGED <<phase, transitionEpoch, ownedEpoch, scanoutEpoch,
                    pointerEpoch, randrEpoch, policyEpoch,
                    policyCommittedEpoch, presentedEpoch, inputControlEpoch,
                    inputEnabled, rightEverLost, oldWorkEpoch,
                    rebuildFailures, callbackLog, policyLog>>

ObserveNoOutput ==
    /\ observedEpoch < MaxEpoch
    /\ candidateHasOutput
    /\ observedEpoch' = observedEpoch + 1
    /\ candidateHasOutput' = FALSE
    /\ rightLive' = FALSE
    /\ rightEverLost' = TRUE
    /\ noticePending' = TRUE
    /\ UNCHANGED <<phase, transitionEpoch, ownedEpoch, scanoutEpoch,
                    pointerEpoch, randrEpoch, policyEpoch,
                    policyCommittedEpoch, presentedEpoch, inputControlEpoch,
                    inputEnabled, rightGeneration, oldWorkEpoch,
                    rebuildFailures, callbackLog, policyLog>>

ObserveOutputRecovery ==
    /\ observedEpoch < MaxEpoch
    /\ ~candidateHasOutput
    /\ observedEpoch' = observedEpoch + 1
    /\ candidateHasOutput' = TRUE
    /\ noticePending' = TRUE
    /\ UNCHANGED <<phase, transitionEpoch, ownedEpoch, scanoutEpoch,
                    pointerEpoch, randrEpoch, policyEpoch,
                    policyCommittedEpoch, presentedEpoch, inputControlEpoch,
                    inputEnabled, rightLive, rightGeneration, rightEverLost,
                    oldWorkEpoch, rebuildFailures, callbackLog, policyLog>>

\* run_session_loop topology boundary: advance the frontend input epoch before
\* any old KMS or policy ownership is released.
BeginTopologyTransition ==
    /\ phase \in {"stable", "awaiting"}
    /\ noticePending
    /\ \/ inputControlEpoch < MaxEpoch
       \/ phase = "awaiting"
    /\ transitionEpoch' = observedEpoch
    /\ inputControlEpoch' =
           IF phase = "awaiting"
           THEN inputControlEpoch
           ELSE inputControlEpoch + 1
    /\ inputEnabled' = FALSE
    /\ oldWorkEpoch' = scanoutEpoch
    /\ noticePending' = FALSE
    /\ phase' = "revoked"
    /\ UNCHANGED <<observedEpoch, ownedEpoch, scanoutEpoch, pointerEpoch, randrEpoch,
                    policyEpoch, policyCommittedEpoch, presentedEpoch,
                    candidateHasOutput, rightLive, rightGeneration,
                    rightEverLost, rebuildFailures, callbackLog, policyLog>>

\* A newer complete observation supersedes every not-yet-presented candidate.
SupersedeTopologyTransition ==
    /\ phase \in TransitionPhases
    /\ transitionEpoch # observedEpoch
    /\ transitionEpoch' = observedEpoch
    /\ inputEnabled' = FALSE
    /\ oldWorkEpoch' = scanoutEpoch
    /\ noticePending' = FALSE
    /\ phase' = "revoked"
    /\ UNCHANGED <<observedEpoch, ownedEpoch, scanoutEpoch, pointerEpoch, randrEpoch,
                    policyEpoch, policyCommittedEpoch, presentedEpoch,
                    inputControlEpoch, candidateHasOutput, rightLive,
                    rightGeneration, rightEverLost, rebuildFailures,
                    callbackLog, policyLog>>

\* LiveProductionVisualRuntime::suspend_native_scanout.
QuiesceNativeScanout ==
    /\ phase = "revoked"
    /\ oldWorkEpoch' = 0
    /\ phase' = "quiesced"
    /\ UNCHANGED <<observedEpoch, transitionEpoch, ownedEpoch, scanoutEpoch,
                    pointerEpoch, randrEpoch, policyEpoch,
                    policyCommittedEpoch, presentedEpoch, inputControlEpoch,
                    inputEnabled, noticePending, candidateHasOutput,
                    rightLive, rightGeneration, rightEverLost,
                    rebuildFailures, callbackLog, policyLog>>

\* LiveProductionNativeScanout replacement plus runtime resume. The complete
\* output set is still private to the owner at this boundary.
RebuildNativeRuntime ==
    /\ phase = "quiesced"
    /\ candidateHasOutput
    /\ transitionEpoch = observedEpoch
    /\ scanoutEpoch' = transitionEpoch
    /\ phase' = "rebuilt"
    /\ UNCHANGED <<observedEpoch, transitionEpoch, ownedEpoch, pointerEpoch,
                    randrEpoch, policyEpoch, policyCommittedEpoch,
                    presentedEpoch, inputControlEpoch, inputEnabled,
                    noticePending, candidateHasOutput, rightLive,
                    rightGeneration, rightEverLost, oldWorkEpoch,
                    rebuildFailures, callbackLog, policyLog>>

\* Bounded construction failure retains state and re-arms one rescan without
\* re-enabling application input.
FailNativeRuntimeRebuild ==
    /\ phase = "quiesced"
    /\ candidateHasOutput
    /\ rebuildFailures < MaxRebuildFailures
    /\ rebuildFailures' = rebuildFailures + 1
    /\ scanoutEpoch' = 0
    /\ noticePending' = TRUE
    /\ phase' = "awaiting"
    /\ UNCHANGED <<observedEpoch, transitionEpoch, ownedEpoch, pointerEpoch,
                    randrEpoch, policyEpoch, policyCommittedEpoch,
                    presentedEpoch, inputControlEpoch, inputEnabled,
                    candidateHasOutput, rightLive, rightGeneration,
                    rightEverLost, oldWorkEpoch, callbackLog, policyLog>>

EnterNoOutputRecovery ==
    /\ phase = "quiesced"
    /\ ~candidateHasOutput
    /\ scanoutEpoch' = 0
    /\ phase' = "awaiting"
    /\ UNCHANGED <<observedEpoch, transitionEpoch, ownedEpoch, pointerEpoch,
                    randrEpoch, policyEpoch, policyCommittedEpoch,
                    presentedEpoch, inputControlEpoch, inputEnabled,
                    noticePending, candidateHasOutput, rightLive,
                    rightGeneration, rightEverLost, oldWorkEpoch,
                    rebuildFailures, callbackLog, policyLog>>

\* One logical owner completion point records that every consumer received the
\* validated complete epoch. Production IPC settles sequentially while input
\* remains quarantined; this action does not claim simultaneous IPC visibility.
PublishCompleteTopology ==
    /\ phase = "rebuilt"
    /\ transitionEpoch = observedEpoch
    /\ oldWorkEpoch = 0
    /\ ownedEpoch' = transitionEpoch
    /\ pointerEpoch' = transitionEpoch
    /\ randrEpoch' = transitionEpoch
    /\ policyEpoch' = transitionEpoch
    /\ phase' = "published"
    /\ UNCHANGED <<observedEpoch, transitionEpoch, scanoutEpoch,
                    policyCommittedEpoch, presentedEpoch, inputControlEpoch,
                    inputEnabled, noticePending, candidateHasOutput,
                    rightLive, rightGeneration, rightEverLost, oldWorkEpoch,
                    rebuildFailures, callbackLog, policyLog>>

CommitCurrentPolicyLayout ==
    /\ phase = "published"
    /\ transitionEpoch = observedEpoch
    /\ policyEpoch = ownedEpoch
    /\ policyCommittedEpoch' = ownedEpoch
    /\ policyLog' = RecordBounded(policyLog,
           [outcome |-> "committed", base |-> ownedEpoch,
            current |-> observedEpoch])
    /\ phase' = "policy_committed"
    /\ UNCHANGED <<observedEpoch, transitionEpoch, ownedEpoch, scanoutEpoch,
                    pointerEpoch, randrEpoch, policyEpoch, presentedEpoch,
                    inputControlEpoch, inputEnabled, noticePending,
                    candidateHasOutput, rightLive, rightGeneration,
                    rightEverLost, oldWorkEpoch, rebuildFailures, callbackLog>>

RejectStalePolicyOutcome(epoch) ==
    /\ epoch \in 1..MaxEpoch
    /\ epoch < observedEpoch
    /\ policyLog' = RecordBounded(policyLog,
           [outcome |-> "stale", base |-> epoch, current |-> observedEpoch])
    /\ UNCHANGED <<phase, observedEpoch, transitionEpoch, ownedEpoch,
                    scanoutEpoch, pointerEpoch, randrEpoch, policyEpoch,
                    policyCommittedEpoch, presentedEpoch, inputControlEpoch,
                    inputEnabled, noticePending, candidateHasOutput,
                    rightLive, rightGeneration, rightEverLost, oldWorkEpoch,
                    rebuildFailures, callbackLog>>

PresentReplacementLayout ==
    /\ phase = "policy_committed"
    /\ transitionEpoch = observedEpoch
    /\ policyCommittedEpoch = ownedEpoch
    /\ presentedEpoch' = ownedEpoch
    /\ phase' = "presented"
    /\ UNCHANGED <<observedEpoch, transitionEpoch, ownedEpoch, scanoutEpoch,
                    pointerEpoch, randrEpoch, policyEpoch,
                    policyCommittedEpoch, inputControlEpoch, inputEnabled,
                    noticePending, candidateHasOutput, rightLive,
                    rightGeneration, rightEverLost, oldWorkEpoch,
                    rebuildFailures, callbackLog, policyLog>>

EnablePresentedInput ==
    /\ phase = "presented"
    /\ transitionEpoch = observedEpoch
    /\ scanoutEpoch = ownedEpoch
    /\ pointerEpoch = ownedEpoch
    /\ randrEpoch = ownedEpoch
    /\ policyEpoch = ownedEpoch
    /\ policyCommittedEpoch = ownedEpoch
    /\ presentedEpoch = ownedEpoch
    /\ inputEnabled' = TRUE
    /\ phase' = "stable"
    /\ UNCHANGED <<observedEpoch, transitionEpoch, ownedEpoch, scanoutEpoch,
                    pointerEpoch, randrEpoch, policyEpoch,
                    policyCommittedEpoch, presentedEpoch, inputControlEpoch,
                    noticePending, candidateHasOutput, rightLive,
                    rightGeneration, rightEverLost, oldWorkEpoch,
                    rebuildFailures, callbackLog, policyLog>>

AcceptCurrentPageFlip(epoch) ==
    /\ epoch \in 1..MaxEpoch
    /\ epoch = scanoutEpoch
    /\ scanoutEpoch # 0
    /\ callbackLog' = RecordBounded(callbackLog,
           [callback |-> epoch, owner |-> scanoutEpoch])
    /\ UNCHANGED <<phase, observedEpoch, transitionEpoch, ownedEpoch,
                    scanoutEpoch, pointerEpoch, randrEpoch, policyEpoch,
                    policyCommittedEpoch, presentedEpoch, inputControlEpoch,
                    inputEnabled, noticePending, candidateHasOutput,
                    rightLive, rightGeneration, rightEverLost, oldWorkEpoch,
                    rebuildFailures, policyLog>>

RejectStalePageFlip(epoch) ==
    /\ epoch \in 1..MaxEpoch
    /\ epoch < scanoutEpoch
    /\ UNCHANGED vars

Next ==
    \/ ObserveRightLoss
    \/ ObserveRightReturn
    \/ ObserveNoOutput
    \/ ObserveOutputRecovery
    \/ BeginTopologyTransition
    \/ SupersedeTopologyTransition
    \/ QuiesceNativeScanout
    \/ RebuildNativeRuntime
    \/ FailNativeRuntimeRebuild
    \/ EnterNoOutputRecovery
    \/ PublishCompleteTopology
    \/ CommitCurrentPolicyLayout
    \/ PresentReplacementLayout
    \/ EnablePresentedInput
    \/ \E epoch \in 1..MaxEpoch : RejectStalePolicyOutcome(epoch)
    \/ \E epoch \in 1..MaxEpoch : AcceptCurrentPageFlip(epoch)
    \/ \E epoch \in 1..MaxEpoch : RejectStalePageFlip(epoch)

Spec ==
    /\ Init
    /\ [][Next]_vars
    /\ WF_vars(BeginTopologyTransition)
    /\ WF_vars(SupersedeTopologyTransition)
    /\ WF_vars(QuiesceNativeScanout)
    /\ WF_vars(RebuildNativeRuntime)
    /\ WF_vars(EnterNoOutputRecovery)
    /\ WF_vars(PublishCompleteTopology)
    /\ WF_vars(CommitCurrentPolicyLayout)
    /\ WF_vars(PresentReplacementLayout)
    /\ WF_vars(EnablePresentedInput)

TypeOK ==
    /\ phase \in {"stable", "awaiting"} \union TransitionPhases
    /\ observedEpoch \in 1..MaxEpoch
    /\ transitionEpoch \in 1..MaxEpoch
    /\ ownedEpoch \in 1..MaxEpoch
    /\ scanoutEpoch \in 0..MaxEpoch
    /\ pointerEpoch \in 1..MaxEpoch
    /\ randrEpoch \in 1..MaxEpoch
    /\ policyEpoch \in 1..MaxEpoch
    /\ policyCommittedEpoch \in 1..MaxEpoch
    /\ presentedEpoch \in 1..MaxEpoch
    /\ inputControlEpoch \in 1..MaxEpoch
    /\ inputEnabled \in BOOLEAN
    /\ noticePending \in BOOLEAN
    /\ candidateHasOutput \in BOOLEAN
    /\ rightLive \in BOOLEAN
    /\ rightGeneration \in 1..MaxEpoch
    /\ rightEverLost \in BOOLEAN
    /\ oldWorkEpoch \in 0..MaxEpoch
    /\ rebuildFailures \in 0..MaxRebuildFailures
    /\ Len(callbackLog) <= MaxEvents
    /\ Len(policyLog) <= MaxEvents

InputRequiresPresentedPolicy ==
    inputEnabled =>
        /\ phase = "stable"
        /\ ownedEpoch = scanoutEpoch
        /\ ownedEpoch = pointerEpoch
        /\ ownedEpoch = randrEpoch
        /\ ownedEpoch = policyEpoch
        /\ ownedEpoch = policyCommittedEpoch
        /\ ownedEpoch = presentedEpoch

PublishedTopologyIsAtomic ==
    phase \in {"published", "policy_committed", "presented", "stable"} =>
        /\ ownedEpoch = scanoutEpoch
        /\ ownedEpoch = pointerEpoch
        /\ ownedEpoch = randrEpoch
        /\ ownedEpoch = policyEpoch

OldWorkCannotCrossEpoch ==
    /\ \A index \in 1..Len(callbackLog) :
          callbackLog[index].callback = callbackLog[index].owner
    /\ \A index \in 1..Len(policyLog) :
          policyLog[index].outcome = "committed" =>
              policyLog[index].base = policyLog[index].current

ReturnedOutputGenerationAdvances ==
    rightLive /\ rightEverLost => rightGeneration > 1

AwaitingRecoveryIsQuarantined ==
    phase = "awaiting" => ~inputEnabled

RecoveryReady ==
    /\ phase = "awaiting"
    /\ candidateHasOutput
    /\ noticePending
    /\ observedEpoch = MaxEpoch

ValidReplacementCanRecover ==
    RecoveryReady ~> (phase = "stable" /\ inputEnabled)

==========================================================================
