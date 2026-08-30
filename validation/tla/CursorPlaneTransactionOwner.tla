------------------- MODULE CursorPlaneTransactionOwner -------------------
EXTENDS Naturals

(***************************************************************************
 * One owner deciding what goes into a CRTC's next atomic commit.           *
 *                                                                          *
 * Sophia drives primary planes atomically and the cursor through the       *
 * legacy ioctl, which is why the two never contend: an ioctl can move a    *
 * cursor while a page flip is outstanding, and archive 0004 counted that   *
 * happening fifteen times. An atomic commit cannot. The kernel serializes  *
 * commits per CRTC, so bringing the cursor into the request means the      *
 * cursor joins the queue the primary already waits in.                     *
 *                                                                          *
 * That is the whole subject here: what happens to a cursor move that       *
 * arrives at a busy CRTC. It must not be lost, must not wait for a client  *
 * that may not draw again for most of a second, and must not accumulate    *
 * one commit per pointer event.                                            *
 *                                                                          *
 * The model also has to keep two things a working system already has. A    *
 * cursor commit must not disturb a directly scanned client buffer -- the   *
 * primary keeps scanning what it was scanning. The eligibility episode     *
 * itself stays with `PresentFlipOwnership`, which owns it; nothing here    *
 * advances one, and an invariant over a variable this model never changes  *
 * would pass without meaning anything. And a refused                       *
 * combined commit must never cost the frame: the retry drops the cursor,   *
 * not the primary.                                                         *
 *                                                                          *
 * Deliberately not modelled: cursor image content, hotspot, formats and    *
 * sizes (a startup capability probe answers those once, the way the atomic *
 * test answers format questions for direct scanout); cursor framebuffer    *
 * allocation, which is the existing resource-bundle discipline rather than *
 * a new temporal property; mirror cohort pacing, which `MirrorHeadPacing`  *
 * owns; and every duration.                                                *
 *************************************************************************)

CONSTANTS MaxMoves, MaxFrames

ASSUME MaxMoves \in Nat /\ MaxMoves >= 1
ASSUME MaxFrames \in Nat /\ MaxFrames >= 1

(***************************************************************************
 * outstanding  : which commit kind the CRTC is currently busy with, if     *
 *                any. "none" means the CRTC is free. The kernel allows one *
 *                at a time and this is that one.                           *
 * pendingCursor: the newest cursor position not yet committed, or 0. A     *
 *                cell rather than a queue: a newer move overwrites an      *
 *                uncommitted one, because a backlog that grows per pointer *
 *                event is unbounded by construction.                       *
 * committed    : the cursor position the planes are showing.               *
 * pendingFrame : a client frame waiting to be committed.                   *
 * scanned      : which client buffer the primary plane is scanning. This   *
 *                is what a cursor commit must leave alone.                 *
 * doubleCommit : set if a commit is ever issued while one is outstanding.  *
 *                The single `outstanding` variable makes that structurally *
 *                impossible, so stating it as an invariant over that       *
 *                variable would pass without meaning anything; each commit *
 *                re-evaluates the guard instead, the way PresentFlipOwner- *
 *                ship records a bad flip.                                  *
 * lostFrame    : set if a frame is ever dropped because of the cursor.     *
 * disturbed    : set if a cursor-only commit ever changes what is scanned. *
 * moves        : pointer motions the environment has produced.             *
 * frames       : client frames the environment has produced.               *
 * commits      : atomic commits issued.                                    *
 * freed        : how many times the CRTC became free.                      *
 *************************************************************************)
VARIABLES outstanding, pendingCursor, committed, pendingFrame, scanned,
          doubleCommit, lostFrame, disturbed, moves, frames, commits, freed

vars == <<outstanding, pendingCursor, committed, pendingFrame, scanned,
          doubleCommit, lostFrame, disturbed, moves, frames, commits, freed>>

Kinds == {"none", "primary", "cursorOnly"}

Init ==
    /\ outstanding = "none"
    /\ pendingCursor = 0
    /\ committed = 0
    /\ pendingFrame = 0
    /\ scanned = 0
    /\ doubleCommit = FALSE
    /\ lostFrame = FALSE
    /\ disturbed = FALSE
    /\ moves = 0
    /\ frames = 0
    /\ commits = 0
    /\ freed = 0

(***************************************************************************
 * Environment: the pointer moves. Unfair and bounded -- nothing obliges a  *
 * hand to move a mouse. A move while one is already pending supersedes it  *
 * in place, which is the latest-wins cell the implementation needs.        *
 *************************************************************************)
PointerMoves ==
    /\ moves < MaxMoves
    /\ moves' = moves + 1
    /\ pendingCursor' = moves + 1
    /\ UNCHANGED <<outstanding, committed, pendingFrame, scanned,
         doubleCommit, lostFrame, disturbed, frames, commits, freed>>

(***************************************************************************
 * Environment: the client draws. Also unfair: a client repainting on a     *
 * cursor blink may not draw again for most of a second, which is exactly   *
 * why a cursor cannot be made to wait for the next frame.                  *
 *************************************************************************)
ClientDraws ==
    /\ frames < MaxFrames
    /\ pendingFrame = 0
    /\ frames' = frames + 1
    /\ pendingFrame' = frames + 1
    /\ UNCHANGED <<outstanding, pendingCursor, committed, scanned,
         doubleCommit, lostFrame, disturbed, moves, commits, freed>>

(***************************************************************************
 * A commit carrying the primary, and the cursor too when one is pending.   *
 * This is the cheap case: the cursor rides a request that was going out    *
 * anyway.                                                                  *
 *************************************************************************)
CommitPrimary ==
    /\ outstanding = "none"
    /\ pendingFrame # 0
    /\ outstanding' = "primary"
    /\ scanned' = pendingFrame
    /\ pendingFrame' = 0
    /\ committed' = IF pendingCursor # 0 THEN pendingCursor ELSE committed
    /\ pendingCursor' = 0
    /\ commits' = commits + 1
    /\ doubleCommit' = (doubleCommit \/ outstanding # "none")
    /\ UNCHANGED <<lostFrame, disturbed, moves, frames, freed>>

(***************************************************************************
 * A commit carrying only the cursor. Atomic requests are sparse, so a      *
 * request naming only cursor properties leaves the primary's framebuffer   *
 * bound -- which is what lets a directly scanned client buffer stay on the *
 * plane while the pointer moves over it. `scanned` is therefore carried    *
 * forward explicitly rather than by UNCHANGED, and `disturbed` compares    *
 * the new value against the old, so a weakened version shows up as a       *
 * recorded violation instead of a quietly different model.                 *
 *************************************************************************)
CommitCursorOnly ==
    /\ outstanding = "none"
    /\ pendingCursor # 0
    /\ pendingFrame = 0
    /\ outstanding' = "cursorOnly"
    /\ committed' = pendingCursor
    /\ pendingCursor' = 0
    /\ commits' = commits + 1
    /\ scanned' = scanned
    /\ disturbed' = (disturbed \/ scanned' # scanned)
    /\ doubleCommit' = (doubleCommit \/ outstanding # "none")
    /\ UNCHANGED <<pendingFrame, lostFrame, moves, frames, freed>>

(***************************************************************************
 * The commit completes and the CRTC frees. This is the page-flip event for *
 * a primary commit; a cursor-only commit settles the same way as far as    *
 * the CRTC is concerned, which is the point -- it occupies the same slot.  *
 *************************************************************************)
CommitCompletes ==
    /\ outstanding # "none"
    /\ outstanding' = "none"
    /\ freed' = freed + 1
    /\ UNCHANGED <<pendingCursor, committed, pendingFrame, scanned,
         doubleCommit, lostFrame, disturbed, moves, frames, commits>>

(***************************************************************************
 * The driver refuses a combined commit. The retry drops the cursor and     *
 * commits the primary alone: the frame survives, and the cursor stays      *
 * pending for a later commit rather than being discarded with the request  *
 * that carried it. A cursor must never cost a frame.                       *
 *                                                                          *
 * `lostFrame` re-evaluates whether the frame reached the plane, so a retry *
 * that dropped the primary instead would be a recorded violation.          *
 *************************************************************************)
CombinedCommitRefused ==
    /\ outstanding = "none"
    /\ pendingFrame # 0
    /\ pendingCursor # 0
    /\ outstanding' = "primary"
    /\ scanned' = pendingFrame
    /\ pendingFrame' = 0
    /\ lostFrame' = (lostFrame \/ scanned' # pendingFrame)
    /\ commits' = commits + 1
    /\ doubleCommit' = (doubleCommit \/ outstanding # "none")
    /\ UNCHANGED <<pendingCursor, committed, disturbed, moves,
         frames, freed>>

Owner ==
    \/ CommitPrimary
    \/ CommitCursorOnly
    \/ CombinedCommitRefused
    \/ CommitCompletes

Next ==
    \/ PointerMoves
    \/ ClientDraws
    \/ Owner

Spec == Init /\ [][Next]_vars
FairSpec == Spec /\ WF_vars(Owner)

TypeOK ==
    /\ outstanding \in Kinds
    /\ pendingCursor \in 0..MaxMoves
    /\ committed \in 0..MaxMoves
    /\ pendingFrame \in 0..MaxFrames
    /\ scanned \in 0..MaxFrames
    /\ doubleCommit \in BOOLEAN
    /\ lostFrame \in BOOLEAN
    /\ disturbed \in BOOLEAN
    /\ commits \in 0..(MaxMoves + 2 * MaxFrames + 1)

(***************************************************************************
 * The kernel's rule, which is the reason this model exists. Two commits    *
 * outstanding on one CRTC is not a race to be tuned; it is a request the   *
 * driver refuses.                                                          *
 *************************************************************************)
OneOutstandingCommitPerCrtc == ~doubleCommit

(***************************************************************************
 * A cursor-only commit leaves the primary scanning what it was scanning.   *
 * Without this a cursor move over a directly scanned frame could evict the *
 * client's buffer, which is the interaction the whole row has to preserve. *
 *************************************************************************)
CursorOnlyCommitPreservesPrimary == ~disturbed

(***************************************************************************
 * A cursor never costs a frame. A refused combined commit retries with the *
 * primary alone; the cursor waits, which is invisible next to a dropped    *
 * frame.                                                                   *
 *************************************************************************)
NoFrameLostToCursor == ~lostFrame

(***************************************************************************
 * Work is bounded by how often the CRTC becomes free, not by how often the *
 * pointer moves. This is what the roadmap means by bounded cursor-only     *
 * idle work: an implementation that queued a commit per pointer event      *
 * would break it, because a hand moving a mouse produces motion far faster *
 * than a display retires frames.                                           *
 *                                                                          *
 * In this model it is implied rather than independent: exceeding the bound  *
 * requires committing while a commit is outstanding, which sets            *
 * `doubleCommit` first. Its control fires on that invariant instead, and   *
 * this is kept as a stated consequence rather than deleted -- the same     *
 * reasoning `PresentFlipOwnership` gives for keeping a conjunct its        *
 * neighbours already make unreachable. An implementation is free to drift  *
 * the two apart; the property should be here when it does.                 *
 *************************************************************************)
CursorWorkBoundedByAvailability == commits <= freed + 1

(***************************************************************************
 * A moved cursor eventually reaches a plane. Under service fairness only:  *
 * nothing here says when, and nothing obliges the client to draw -- which  *
 * is the point, since a cursor that waited for the next client frame would *
 * freeze on an idle desktop.                                               *
 *************************************************************************)
PendingCursorEventuallyCommits ==
    (pendingCursor # 0) ~> (pendingCursor = 0)

=============================================================================
