-------------------------- MODULE AdmissionRecovery --------------------------
EXTENDS FiniteSets, Naturals

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
          targetCommitted, relayouts, outcome, feedback, primed, timedOut,
          planningEligible, manageOutstanding, manageRequests

vars == <<selected, storage, fallbackOwner, targetOwner, layout, admission,
          recovery, standing, targetObserved, targetConfigured,
          targetCommitted, relayouts, outcome, feedback, primed, timedOut,
          planningEligible, manageOutstanding, manageRequests>>

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
    /\ planningEligible = TRUE
    /\ manageOutstanding = FALSE
    /\ manageRequests = 0

IssueManage ==
    /\ planningEligible
    /\ ~manageOutstanding
    /\ layout \in {"pending", "fallback"}
    /\ manageRequests < 2
    /\ manageOutstanding' = TRUE
    /\ manageRequests' = manageRequests + 1
    /\ UNCHANGED <<selected, storage, fallbackOwner, targetOwner, layout,
                    admission, recovery, standing, targetObserved,
                    targetConfigured, targetCommitted, relayouts, outcome,
                    feedback, primed, timedOut, planningEligible>>

ObservePresent ==
    /\ admission # "managed"
    /\ selected # PresentCandidate
    /\ selected' = PresentCandidate
    /\ storage' \in Storages
    /\ fallbackOwner' = "quarantine"
    /\ UNCHANGED <<targetOwner, layout, admission, recovery, standing,
                    targetObserved, targetConfigured, targetCommitted,
                    relayouts, outcome, feedback, primed, timedOut,
                    planningEligible, manageOutstanding, manageRequests>>

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
                    relayouts, outcome, feedback, primed, timedOut,
                    planningEligible, manageOutstanding, manageRequests>>

PrimeAdmission ==
    /\ layout = "pending"
    /\ selected \in Candidates
    /\ manageOutstanding
    /\ layout' = "recovery"
    /\ recovery' = TRUE
    /\ standing' = TRUE
    /\ primed' = TRUE
    /\ UNCHANGED <<selected, storage, fallbackOwner, targetOwner, admission,
                    targetObserved, targetConfigured, targetCommitted,
                    relayouts, outcome, feedback, timedOut,
                    planningEligible, manageOutstanding, manageRequests>>

Timeout ==
    /\ layout = "pending"
    /\ selected = NoCandidate
    /\ manageOutstanding
    /\ layout' = "recovery"
    /\ recovery' = TRUE
    /\ standing' = TRUE
    /\ timedOut' = TRUE
    /\ UNCHANGED <<selected, storage, fallbackOwner, targetOwner, admission,
                    targetObserved, targetConfigured, targetCommitted,
                    relayouts, outcome, feedback, primed,
                    planningEligible, manageOutstanding, manageRequests>>

CommitRecovery ==
    /\ layout = "recovery"
    /\ selected \in Candidates
    /\ manageOutstanding
    /\ IF selected = PresentCandidate
          THEN /\ layout' = "fallback"
               /\ admission' = "awaiting_retirement"
               /\ recovery' = recovery
               /\ relayouts' = relayouts
          ELSE /\ layout' = "relayout"
               /\ admission' = "managed"
               /\ recovery' = FALSE
               /\ relayouts' = 1
    /\ planningEligible' = FALSE
    /\ manageOutstanding' = FALSE
    /\ UNCHANGED <<selected, storage, fallbackOwner, targetOwner, standing,
                    targetObserved, targetConfigured, targetCommitted,
                    outcome, feedback, primed, timedOut, manageRequests>>

ObserveTarget ==
    /\ standing
    /\ ~targetObserved
    /\ targetObserved' = TRUE
    /\ targetOwner' = "quarantine"
    /\ UNCHANGED <<selected, storage, fallbackOwner, layout, admission,
                    recovery, standing, targetConfigured, targetCommitted,
                    relayouts, outcome, feedback, primed, timedOut,
                    planningEligible, manageOutstanding, manageRequests>>

ReleaseFallback ==
    /\ selected = PresentCandidate
    /\ layout = "fallback"
    /\ admission = "awaiting_retirement"
    /\ fallbackOwner = "quarantine"
    /\ fallbackOwner' = "scheduler"
    /\ UNCHANGED <<selected, storage, targetOwner, layout, admission,
                    recovery, standing, targetObserved, targetConfigured,
                    targetCommitted, relayouts, outcome, feedback,
                    primed, timedOut, planningEligible, manageOutstanding,
                    manageRequests>>

SubmitFallback ==
    /\ fallbackOwner = "scheduler"
    /\ fallbackOwner' = "inflight"
    /\ UNCHANGED <<selected, storage, targetOwner, layout, admission,
                    recovery, standing, targetObserved, targetConfigured,
                    targetCommitted, relayouts, outcome, feedback,
                    primed, timedOut, planningEligible, manageOutstanding,
                    manageRequests>>

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
                    targetConfigured, targetCommitted, primed, timedOut,
                    planningEligible, manageOutstanding, manageRequests>>

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
                    relayouts, outcome, feedback, primed, timedOut,
                    planningEligible, manageOutstanding, manageRequests>>

ReleaseTarget ==
    /\ targetObserved
    /\ targetConfigured
    /\ admission = "managed"
    /\ targetOwner = "quarantine"
    /\ targetOwner' = "scheduler"
    /\ UNCHANGED <<selected, storage, fallbackOwner, layout, admission,
                    recovery, standing, targetObserved, targetConfigured,
                    targetCommitted, relayouts, outcome, feedback,
                    primed, timedOut, planningEligible, manageOutstanding,
                    manageRequests>>

SubmitTarget ==
    /\ targetOwner = "scheduler"
    /\ targetOwner' = "inflight"
    /\ UNCHANGED <<selected, storage, fallbackOwner, layout, admission,
                    recovery, standing, targetObserved, targetConfigured,
                    targetCommitted, relayouts, outcome, feedback,
                    primed, timedOut, planningEligible, manageOutstanding,
                    manageRequests>>

RetireTarget ==
    /\ targetOwner = "inflight"
    /\ targetOwner' = "retired"
    /\ targetCommitted' = TRUE
    /\ standing' = FALSE
    /\ layout' = "committed"
    /\ UNCHANGED <<selected, storage, fallbackOwner, admission, recovery,
                    targetObserved, targetConfigured, relayouts,
                    outcome, feedback, primed, timedOut, planningEligible,
                    manageOutstanding, manageRequests>>

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
    \/ IssueManage
    \/ ObservePresent
    \/ ObserveBacking
    \/ PrimeAdmission
    \/ Timeout
    \/ CommitRecovery
    \/ Progress

Spec == Init /\ [][Next]_vars
BeginRecovery == PrimeAdmission \/ Timeout
FairSpec == Spec
            /\ WF_vars(IssueManage)
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
    /\ planningEligible \in BOOLEAN
    /\ manageOutstanding \in BOOLEAN
    /\ manageRequests \in 0..2

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

ManagePlanningIsBounded ==
    manageRequests <= 1

CommittedManageConsumesPlanning ==
    layout \in {"fallback", "relayout", "target", "committed"} =>
        /\ ~planningEligible
        /\ ~manageOutstanding
        /\ manageRequests = 1

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
