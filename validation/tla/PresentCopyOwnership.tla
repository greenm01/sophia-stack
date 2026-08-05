------------------------ MODULE PresentCopyOwnership ------------------------
EXTENDS Naturals

(***************************************************************************
 * A composited DMA-BUF Present is copied into compositor-owned storage.    *
 * The client source becomes idle at exact page-flip retirement; Copy       *
 * completion follows, while failed staged work rolls its snapshot back.    *
 ***************************************************************************
*)

Phases == {"waiting", "captured", "submitted", "displayed", "completed",
           "rolled_back", "evicted"}
SnapshotStates == {"absent", "staged", "promoted"}
Settlements == {"none", "copy", "skip"}

VARIABLES phase, sourceOwned, snapshot, outputOwner, idle, complete, settlement

vars == <<phase, sourceOwned, snapshot, outputOwner, idle, complete, settlement>>

Init ==
    /\ phase = "waiting"
    /\ sourceOwned = TRUE
    /\ snapshot = "absent"
    /\ outputOwner = "none"
    /\ idle = FALSE
    /\ complete = FALSE
    /\ settlement = "none"

Capture ==
    /\ phase = "waiting"
    /\ phase' = "captured"
    /\ snapshot' = "staged"
    /\ UNCHANGED <<sourceOwned, outputOwner, idle, complete, settlement>>

Submit ==
    /\ phase = "captured"
    /\ phase' = "submitted"
    /\ UNCHANGED <<sourceOwned, snapshot, outputOwner, idle, complete, settlement>>

PageFlip ==
    /\ phase = "submitted"
    /\ snapshot = "staged"
    /\ phase' = "displayed"
    /\ sourceOwned' = FALSE
    /\ snapshot' = "promoted"
    /\ outputOwner' = "snapshot"
    /\ idle' = TRUE
    /\ UNCHANGED <<complete, settlement>>

CompleteCopy ==
    /\ phase = "displayed"
    /\ idle
    /\ phase' = "completed"
    /\ complete' = TRUE
    /\ settlement' = "copy"
    /\ UNCHANGED <<sourceOwned, snapshot, outputOwner, idle>>

Rollback ==
    /\ phase \in {"captured", "submitted"}
    /\ snapshot = "staged"
    /\ phase' = "rolled_back"
    /\ sourceOwned' = FALSE
    /\ snapshot' = "absent"
    /\ outputOwner' = "none"
    /\ idle' = TRUE
    /\ complete' = TRUE
    /\ settlement' = "skip"

Evict ==
    /\ phase = "completed"
    /\ snapshot = "promoted"
    /\ phase' = "evicted"
    /\ snapshot' = "absent"
    /\ outputOwner' = "none"
    /\ UNCHANGED <<sourceOwned, idle, complete, settlement>>

PresentProgress == Capture \/ Submit \/ PageFlip \/ CompleteCopy \/ Rollback \/ Evict
Next == PresentProgress
Spec == Init /\ [][Next]_vars
FairSpec == Spec /\ WF_vars(PresentProgress)

TypeOK ==
    /\ phase \in Phases
    /\ sourceOwned \in BOOLEAN
    /\ snapshot \in SnapshotStates
    /\ outputOwner \in {"none", "snapshot"}
    /\ idle \in BOOLEAN
    /\ complete \in BOOLEAN
    /\ settlement \in Settlements

DisplayedContentIsCompositorOwned ==
    outputOwner = "snapshot" => snapshot = "promoted" /\ ~sourceOwned

CopyRequiresPromotedSnapshot ==
    settlement = "copy" => idle /\ complete /\ ~sourceOwned

CompletedCopyRetainsSnapshot ==
    phase = "completed" => snapshot = "promoted" /\ outputOwner = "snapshot"

CopyIdlesBeforeCompletion ==
    phase = "displayed" => idle /\ ~complete

RollbackReleasesEverything ==
    phase = "rolled_back" => snapshot = "absent" /\ ~sourceOwned /\ settlement = "skip"

StagedSnapshotsAreNotCommitted ==
    snapshot = "staged" => outputOwner = "none" /\ settlement = "none"

EventuallyTerminal == phase = "waiting" ~> phase \in {"completed", "rolled_back"}

=============================================================================
