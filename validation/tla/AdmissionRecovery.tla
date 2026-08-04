-------------------------- MODULE AdmissionRecovery --------------------------
EXTENDS FiniteSets

(***************************************************************************
 * Exact ownership of a PresentedBuffer candidate across admission timeout, *
 * recovery, scheduler release, retirement, and protocol feedback.          *
 ****************************************************************************)

CONSTANTS PresentCandidate, BackingCandidate, NoCandidate

ASSUME /\ PresentCandidate # BackingCandidate
       /\ PresentCandidate # NoCandidate
       /\ BackingCandidate # NoCandidate

Candidates == {PresentCandidate, BackingCandidate}
Owners == {"none", "quarantine", "scheduler", "inflight", "retired"}
Storages == {"dma", "cpu"}
LayoutPhases == {"pending", "recovery", "committed"}
AdmissionPhases == {"pending", "awaiting_retirement", "managed"}
Outcomes == {"none", "complete", "skip"}

VARIABLES selected, storage, owner, layout, admission, outcome, feedback

vars == <<selected, storage, owner, layout, admission, outcome, feedback>>

Init ==
    /\ selected = NoCandidate
    /\ storage = "none"
    /\ owner = "none"
    /\ layout = "pending"
    /\ admission = "pending"
    /\ outcome = "none"
    /\ feedback = {}

ObservePresent ==
    /\ admission # "managed"
    /\ selected # PresentCandidate
    /\ selected' = PresentCandidate
    /\ storage' \in Storages
    /\ owner' = "quarantine"
    /\ UNCHANGED <<layout, admission, outcome, feedback>>

ObserveBacking ==
    /\ admission # "managed"
    /\ IF selected = NoCandidate
          THEN /\ selected' = BackingCandidate
               /\ storage' = "cpu"
               /\ owner' = "quarantine"
          ELSE /\ selected' = selected
               /\ storage' = storage
               /\ owner' = owner
    /\ UNCHANGED <<layout, admission, outcome, feedback>>

Timeout ==
    /\ layout = "pending"
    /\ layout' = "recovery"
    /\ UNCHANGED <<selected, storage, owner, admission, outcome, feedback>>

CommitRecovery ==
    /\ layout = "recovery"
    /\ selected \in Candidates
    /\ layout' = "committed"
    /\ IF selected = PresentCandidate
          THEN admission' = "awaiting_retirement"
          ELSE admission' = "managed"
    /\ UNCHANGED <<selected, storage, owner, outcome, feedback>>

ReleaseSelectedPresent ==
    /\ selected = PresentCandidate
    /\ layout = "committed"
    /\ admission = "awaiting_retirement"
    /\ owner = "quarantine"
    /\ owner' = "scheduler"
    /\ UNCHANGED <<selected, storage, layout, admission, outcome, feedback>>

SubmitPresent ==
    /\ owner = "scheduler"
    /\ owner' = "inflight"
    /\ UNCHANGED <<selected, storage, layout, admission, outcome, feedback>>

RetirePresent ==
    /\ owner = "inflight"
    /\ owner' = "retired"
    /\ admission' = "managed"
    /\ outcome' = "complete"
    /\ feedback' = {"complete", "idle"}
    /\ UNCHANGED <<selected, storage, layout>>

Progress == ReleaseSelectedPresent \/ SubmitPresent \/ RetirePresent

Next ==
    \/ ObservePresent
    \/ ObserveBacking
    \/ Timeout
    \/ CommitRecovery
    \/ Progress

Spec == Init /\ [][Next]_vars
FairSpec == Spec /\ WF_vars(Progress)

TypeOK ==
    /\ selected \in Candidates \cup {NoCandidate}
    /\ storage \in Storages \cup {"none"}
    /\ owner \in Owners
    /\ layout \in LayoutPhases
    /\ admission \in AdmissionPhases
    /\ outcome \in Outcomes
    /\ feedback \subseteq {"complete", "idle"}

BackingCannotReplacePresent ==
    selected = PresentCandidate => owner # "none"

AdmissionUsesExactPresent ==
    admission = "awaiting_retirement" =>
        /\ selected = PresentCandidate
        /\ storage \in Storages
        /\ owner \in {"quarantine", "scheduler", "inflight"}

ManagedPresentRetired ==
    selected = PresentCandidate /\ admission = "managed" => outcome = "complete"

FeedbackMatchesRetirement ==
    (feedback = {"complete", "idle"}) <=> (outcome = "complete")

ReleasedCandidateIsPresent ==
    owner \in {"scheduler", "inflight", "retired"} =>
        /\ selected = PresentCandidate
        /\ storage \in Storages

SelectedPresentEventuallySettles ==
    (selected = PresentCandidate /\ layout = "committed") ~> (outcome = "complete")

=============================================================================
