-------------------------- MODULE AdmissionRecovery --------------------------
EXTENDS FiniteSets

(***************************************************************************
 * Two-phase admission recovery. An observed safe extent primes recovery;   *
 * timeout is reserved for admission without complete pixels. The fallback *
 * becomes managed and releases its temporary constraint. One ordinary     *
 * relayout then drives the standing target, whose exact native retirement  *
 * alone commits its extent.                                                 *
 ***************************************************************************)

CONSTANTS PresentCandidate, BackingCandidate, NoCandidate

ASSUME /\ PresentCandidate # BackingCandidate
       /\ PresentCandidate # NoCandidate
       /\ BackingCandidate # NoCandidate

Candidates == {PresentCandidate, BackingCandidate}
Owners == {"none", "quarantine", "scheduler", "inflight", "retired"}
Storages == {"dma", "cpu"}
LayoutPhases == {"pending", "recovery", "fallback", "relayout", "target", "committed"}
AdmissionPhases == {"pending", "awaiting_retirement", "managed"}
Outcomes == {"none", "complete", "skip"}

VARIABLES selected, storage, fallbackOwner, targetOwner, layout, admission,
          recovery, standing, targetObserved, targetConfigured,
          targetCommitted, relayouts, outcome, feedback, primed, timedOut

vars == <<selected, storage, fallbackOwner, targetOwner, layout, admission,
          recovery, standing, targetObserved, targetConfigured,
          targetCommitted, relayouts, outcome, feedback, primed, timedOut>>

Init ==
    /\ selected = NoCandidate
    /\ storage = "none"
    /\ fallbackOwner = "none"
    /\ targetOwner = "none"
    /\ layout = "pending"
    /\ admission = "pending"
    /\ recovery = FALSE
    /\ standing = FALSE
    /\ targetObserved = FALSE
    /\ targetConfigured = FALSE
    /\ targetCommitted = FALSE
    /\ relayouts = 0
    /\ outcome = "none"
    /\ feedback = {}
    /\ primed = FALSE
    /\ timedOut = FALSE

ObservePresent ==
    /\ admission # "managed"
    /\ selected # PresentCandidate
    /\ selected' = PresentCandidate
    /\ storage' \in Storages
    /\ fallbackOwner' = "quarantine"
    /\ UNCHANGED <<targetOwner, layout, admission, recovery, standing,
                    targetObserved, targetConfigured, targetCommitted,
                    relayouts, outcome, feedback, primed, timedOut>>

ObserveBacking ==
    /\ admission # "managed"
    /\ IF selected = NoCandidate
          THEN /\ selected' = BackingCandidate
               /\ storage' = "cpu"
               /\ fallbackOwner' = "quarantine"
          ELSE /\ selected' = selected
               /\ storage' = storage
               /\ fallbackOwner' = fallbackOwner
    /\ UNCHANGED <<targetOwner, layout, admission, recovery, standing,
                    targetObserved, targetConfigured, targetCommitted,
                    relayouts, outcome, feedback, primed, timedOut>>

PrimeAdmission ==
    /\ layout = "pending"
    /\ selected \in Candidates
    /\ layout' = "recovery"
    /\ recovery' = TRUE
    /\ standing' = TRUE
    /\ primed' = TRUE
    /\ UNCHANGED <<selected, storage, fallbackOwner, targetOwner, admission,
                    targetObserved, targetConfigured, targetCommitted,
                    relayouts, outcome, feedback, timedOut>>

Timeout ==
    /\ layout = "pending"
    /\ selected = NoCandidate
    /\ layout' = "recovery"
    /\ recovery' = TRUE
    /\ standing' = TRUE
    /\ timedOut' = TRUE
    /\ UNCHANGED <<selected, storage, fallbackOwner, targetOwner, admission,
                    targetObserved, targetConfigured, targetCommitted,
                    relayouts, outcome, feedback, primed>>

CommitRecovery ==
    /\ layout = "recovery"
    /\ selected \in Candidates
    /\ IF selected = PresentCandidate
          THEN /\ layout' = "fallback"
               /\ admission' = "awaiting_retirement"
               /\ recovery' = recovery
               /\ relayouts' = relayouts
          ELSE /\ layout' = "relayout"
               /\ admission' = "managed"
               /\ recovery' = FALSE
               /\ relayouts' = 1
    /\ UNCHANGED <<selected, storage, fallbackOwner, targetOwner, standing,
                    targetObserved, targetConfigured, targetCommitted,
                    outcome, feedback, primed, timedOut>>

ObserveTarget ==
    /\ standing
    /\ ~targetObserved
    /\ targetObserved' = TRUE
    /\ targetOwner' = "quarantine"
    /\ UNCHANGED <<selected, storage, fallbackOwner, layout, admission,
                    recovery, standing, targetConfigured, targetCommitted,
                    relayouts, outcome, feedback, primed, timedOut>>

ReleaseFallback ==
    /\ selected = PresentCandidate
    /\ layout = "fallback"
    /\ admission = "awaiting_retirement"
    /\ fallbackOwner = "quarantine"
    /\ fallbackOwner' = "scheduler"
    /\ UNCHANGED <<selected, storage, targetOwner, layout, admission,
                    recovery, standing, targetObserved, targetConfigured,
                    targetCommitted, relayouts, outcome, feedback,
                    primed, timedOut>>

SubmitFallback ==
    /\ fallbackOwner = "scheduler"
    /\ fallbackOwner' = "inflight"
    /\ UNCHANGED <<selected, storage, targetOwner, layout, admission,
                    recovery, standing, targetObserved, targetConfigured,
                    targetCommitted, relayouts, outcome, feedback,
                    primed, timedOut>>

RetireFallback ==
    /\ fallbackOwner = "inflight"
    /\ fallbackOwner' = "retired"
    /\ admission' = "managed"
    /\ recovery' = FALSE
    /\ relayouts' = 1
    /\ layout' = "relayout"
    /\ outcome' = "complete"
    /\ feedback' = {"complete", "idle"}
    /\ UNCHANGED <<selected, storage, targetOwner, standing, targetObserved,
                    targetConfigured, targetCommitted, primed, timedOut>>

ConfigureTarget ==
    /\ admission = "managed"
    /\ ~recovery
    /\ standing
    /\ layout = "relayout"
    /\ ~targetConfigured
    /\ targetConfigured' = TRUE
    /\ layout' = "target"
    /\ UNCHANGED <<selected, storage, fallbackOwner, targetOwner, admission,
                    recovery, standing, targetObserved, targetCommitted,
                    relayouts, outcome, feedback, primed, timedOut>>

ReleaseTarget ==
    /\ targetObserved
    /\ targetConfigured
    /\ admission = "managed"
    /\ targetOwner = "quarantine"
    /\ targetOwner' = "scheduler"
    /\ UNCHANGED <<selected, storage, fallbackOwner, layout, admission,
                    recovery, standing, targetObserved, targetConfigured,
                    targetCommitted, relayouts, outcome, feedback,
                    primed, timedOut>>

SubmitTarget ==
    /\ targetOwner = "scheduler"
    /\ targetOwner' = "inflight"
    /\ UNCHANGED <<selected, storage, fallbackOwner, layout, admission,
                    recovery, standing, targetObserved, targetConfigured,
                    targetCommitted, relayouts, outcome, feedback,
                    primed, timedOut>>

RetireTarget ==
    /\ targetOwner = "inflight"
    /\ targetOwner' = "retired"
    /\ targetCommitted' = TRUE
    /\ standing' = FALSE
    /\ layout' = "committed"
    /\ UNCHANGED <<selected, storage, fallbackOwner, admission, recovery,
                    targetObserved, targetConfigured, relayouts,
                    outcome, feedback, primed, timedOut>>

Progress ==
    \/ ObserveTarget
    \/ ReleaseFallback
    \/ SubmitFallback
    \/ RetireFallback
    \/ ConfigureTarget
    \/ ReleaseTarget
    \/ SubmitTarget
    \/ RetireTarget

Next ==
    \/ ObservePresent
    \/ ObserveBacking
    \/ PrimeAdmission
    \/ Timeout
    \/ CommitRecovery
    \/ Progress

Spec == Init /\ [][Next]_vars
BeginRecovery == PrimeAdmission \/ Timeout
FairSpec == Spec
            /\ WF_vars(BeginRecovery)
            /\ WF_vars(CommitRecovery)
            /\ WF_vars(Progress)

TypeOK ==
    /\ selected \in Candidates \cup {NoCandidate}
    /\ storage \in Storages \cup {"none"}
    /\ fallbackOwner \in Owners
    /\ targetOwner \in Owners
    /\ layout \in LayoutPhases
    /\ admission \in AdmissionPhases
    /\ recovery \in BOOLEAN
    /\ standing \in BOOLEAN
    /\ targetObserved \in BOOLEAN
    /\ targetConfigured \in BOOLEAN
    /\ targetCommitted \in BOOLEAN
    /\ relayouts \in {0, 1}
    /\ outcome \in Outcomes
    /\ feedback \subseteq {"complete", "idle"}
    /\ primed \in BOOLEAN
    /\ timedOut \in BOOLEAN

BackingCannotReplacePresent ==
    selected = PresentCandidate => fallbackOwner # "none"

AdmissionUsesExactPresent ==
    admission = "awaiting_retirement" =>
        /\ selected = PresentCandidate
        /\ storage \in Storages
        /\ fallbackOwner \in {"quarantine", "scheduler", "inflight"}

ManagedPresentRetired ==
    selected = PresentCandidate /\ admission = "managed" => outcome = "complete"

FeedbackMatchesRetirement ==
    (feedback = {"complete", "idle"}) <=> (outcome = "complete")

ManagedRecoveryIsReleased ==
    admission = "managed" => /\ ~recovery /\ relayouts = 1

RecoveryHasOneCause ==
    layout # "pending" => primed \/ timedOut

PrimedAdmissionAvoidsTimeout ==
    primed => /\ selected \in Candidates /\ ~timedOut

TargetCommitsOnlyAfterExactRetirement ==
    targetCommitted =>
        /\ targetObserved
        /\ targetConfigured
        /\ targetOwner = "retired"
        /\ admission = "managed"
        /\ ~recovery
        /\ ~standing
        /\ relayouts = 1

SelectedPresentEventuallySettles ==
    (selected = PresentCandidate /\ layout = "fallback") ~> (outcome = "complete")

StandingTargetEventuallyCommits ==
    (standing /\ admission = "managed") ~> targetCommitted

ObservedPendingEventuallyManaged ==
    (selected \in Candidates /\ layout = "pending") ~> (admission = "managed")

=============================================================================
