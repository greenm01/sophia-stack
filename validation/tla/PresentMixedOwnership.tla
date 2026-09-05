----------------------- MODULE PresentMixedOwnership -----------------------
EXTENDS Naturals, FiniteSets

(***************************************************************************
 * A mixed Present is composited and presented at once: the compositor      *
 * draws a frame that carries a client's Present, and that frame goes to    *
 * the plane. `PresentFrameOwnership` owns the software path,               *
 * `PresentCopyOwnership` the composited copy, and `PresentFlipOwnership`   *
 * the direct flip where nothing is copied. This module owns the case none  *
 * of them covers, which is the one that ended a session.                   *
 *                                                                          *
 * The question is who owns a retirement. The scheduler holds one present   *
 * in flight at a time, so a later composition replaces what it names while *
 * the earlier frame is still in the kernel. The kernel then retires that   *
 * earlier frame, because the kernel retires what it scanned out, not what  *
 * the scheduler currently believes.                                        *
 *                                                                          *
 * Two things follow and are what this module forbids getting wrong. A      *
 * retirement must name a frame this session actually submitted -- that is  *
 * the ownership rule, and violating it means retiring a buffer the session *
 * never gave the kernel. And every submitted frame must eventually settle: *
 * judging a superseded frame's retirement unlawful strands it, which is    *
 * the failure this module exists to exclude, because in the running system *
 * that verdict ends the session.                                           *
 *                                                                          *
 * Deliberately not modelled: pixel content, fences and all timing (a       *
 * retirement's arrival order is the only timing that matters here), and    *
 * buffer release, which `PresentFlipOwnership` owns for the direct path    *
 * and which a mixed frame does not perform against a client buffer.        *
 ***************************************************************************)

CONSTANTS
    MaxFrames,
    \* Judge ownership by the frame the scheduler names right now, which is
    \* what the running code does. The negative control sets this TRUE and the
    \* stranding invariant then fails, which is the bug stated as a model.
    SchedulerOnlyOwnership

ASSUME MaxFrames \in Nat /\ MaxFrames >= 2
ASSUME SchedulerOnlyOwnership \in BOOLEAN

Frames == 1 .. MaxFrames
NoFrame == 0

VARIABLES
    next,        \* the next frame identity to hand out
    submitted,   \* every frame this session gave the kernel
    scheduler,   \* the frame the scheduler names for this output, or NoFrame
    inKernel,    \* frames the kernel still owes a retirement for
    settled,     \* frames whose retirement was accepted
    stranded     \* frames whose retirement was judged unowned

vars == <<next, submitted, scheduler, inKernel, settled, stranded>>

TypeOK ==
    /\ next \in 1 .. (MaxFrames + 1)
    /\ submitted \subseteq Frames
    /\ scheduler \in Frames \cup {NoFrame}
    /\ inKernel \subseteq submitted
    /\ settled \subseteq submitted
    /\ stranded \subseteq Frames

Init ==
    /\ next = 1
    /\ submitted = {}
    /\ scheduler = NoFrame
    /\ inKernel = {}
    /\ settled = {}
    /\ stranded = {}

(***************************************************************************
 * Composing and submitting a mixed frame. The scheduler names it, which    *
 * displaces whatever it named before; that earlier frame stays in the      *
 * kernel, which is the whole situation this module is about.               *
 ***************************************************************************)
Submit ==
    /\ next <= MaxFrames
    /\ submitted' = submitted \cup {next}
    /\ inKernel' = inKernel \cup {next}
    /\ scheduler' = next
    /\ next' = next + 1
    /\ UNCHANGED <<settled, stranded>>

(***************************************************************************
 * A scene-driven composition takes the scheduler without submitting a      *
 * present of its own. This is the transition the crashed session showed:   *
 * the scheduler stopped naming a frame that was still in the kernel.       *
 ***************************************************************************)
Supersede ==
    /\ scheduler # NoFrame
    /\ scheduler' = NoFrame
    /\ UNCHANGED <<next, submitted, inKernel, settled, stranded>>

(***************************************************************************
 * Ownership is a question about what this session submitted, not about     *
 * what the scheduler happens to name now. A frame the session submitted    *
 * settles whether or not it was superseded first.                          *
 ***************************************************************************)
OwnsRetirement(f) ==
    IF SchedulerOnlyOwnership THEN f = scheduler ELSE f \in submitted

Retire(f) ==
    /\ f \in inKernel
    /\ inKernel' = inKernel \ {f}
    /\ IF OwnsRetirement(f)
         THEN /\ settled' = settled \cup {f}
              /\ UNCHANGED stranded
         ELSE /\ stranded' = stranded \cup {f}
              /\ UNCHANGED settled
    /\ scheduler' = IF scheduler = f THEN NoFrame ELSE scheduler
    /\ UNCHANGED <<next, submitted>>

Next ==
    \/ Submit
    \/ Supersede
    \/ \E f \in inKernel : Retire(f)

Spec == Init /\ [][Next]_vars

FairSpec ==
    /\ Spec
    /\ WF_vars(\E f \in inKernel : Retire(f))

(***************************************************************************
 * Safety. A retirement is accepted only for a frame this session gave the  *
 * kernel. This is the rule the running check is defending, and it stays.   *
 ***************************************************************************)
SettledWereSubmitted == settled \subseteq submitted

(***************************************************************************
 * Safety. Nothing this session submitted is ever judged unowned. A         *
 * superseded frame is still ours; the scheduler forgetting it is not       *
 * evidence that the kernel retired someone else's buffer.                  *
 *                                                                          *
 * The negative control weakens `OwnsRetirement` to the scheduler's current *
 * frame, which is what the code does today, and this is the invariant that *
 * then fails.                                                              *
 ***************************************************************************)
NoSubmittedFrameIsStranded == stranded \cap submitted = {}

(***************************************************************************
 * Progress. Every frame the kernel holds eventually settles. A stranded    *
 * retirement ends the session in the running system, so a model where one  *
 * can persist is a model of a desktop that dies.                           *
 ***************************************************************************)
EveryKernelFrameSettles == [](inKernel # {} => <>(inKernel = {}))

=============================================================================
