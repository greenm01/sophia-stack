------------------------ MODULE PresentFlipOwnership ------------------------
EXTENDS Naturals

(***************************************************************************
 * A direct-scanout Present puts the client's own DMA-BUF on the plane.     *
 * `PresentCopyOwnership` owns the composited path, where a compositor     *
 * snapshot goes to glass and the client source is idle at the flip; this   *
 * module owns its complement, where nothing is copied and the client       *
 * buffer itself is displayed.                                              *
 *                                                                          *
 * Three things follow from that and are what this module forbids getting   *
 * wrong. The displayed buffer cannot be released at submission or at the   *
 * flip; the client would draw into pixels the screen is scanning. It is    *
 * released only when a successor flip retires it -- and the successor may  *
 * be a composed frame, because an effect or overlay activating makes the   *
 * next frame ineligible while the direct frame is still on glass. And a    *
 * flip is lawful only under a proof and a backend atomic test that are     *
 * fresh in the current eligibility episode: an activation ends the         *
 * episode, so a proof or test taken before it proves nothing about the     *
 * frame after it.                                                          *
 *                                                                          *
 * Deliberately not modelled: render slots (a direct frame acquires none;   *
 * `VisualRetirementSlots` owns slot lifetime and is untouched by this      *
 * row), pixel formats and modifiers (the atomic test's verdict is an       *
 * environment fact here), and all timing.                                  *
 *************************************************************************)

CONSTANTS MaxEpisodes

ASSUME MaxEpisodes \in Nat /\ MaxEpisodes >= 1

Phases == {"waiting", "proven", "tested", "flipped", "retired", "completed"}
Settlements == {"none", "flip", "copy"}

VARIABLES phase, effectActive, episode, proofEpisode, testEpisode, testVerdict,
          released, directFlipped, badFlip, settlement

vars == <<phase, effectActive, episode, proofEpisode, testEpisode, testVerdict,
          released, directFlipped, badFlip, settlement>>

NoEpisode == MaxEpisodes + 1

Init ==
    /\ phase = "waiting"
    /\ effectActive = FALSE
    /\ episode = 0
    /\ proofEpisode = NoEpisode
    /\ testEpisode = NoEpisode
    /\ testVerdict = "none"
    /\ released = FALSE
    /\ directFlipped = FALSE
    /\ badFlip = FALSE
    /\ settlement = "none"

(***************************************************************************
 * Environment: an effect or overlay activates or clears. Activation ends   *
 * the eligibility episode; whatever was proven or tested before it proves  *
 * nothing about frames after it. Neither action is fair -- policy may hold *
 * an overlay open forever -- and the episode counter bounds the space.     *
 *************************************************************************)
EffectActivate ==
    /\ ~effectActive
    /\ episode < MaxEpisodes
    /\ effectActive' = TRUE
    /\ episode' = episode + 1
    /\ UNCHANGED <<phase, proofEpisode, testEpisode, testVerdict, released,
         directFlipped, badFlip, settlement>>

EffectDeactivate ==
    /\ effectActive
    /\ effectActive' = FALSE
    /\ UNCHANGED <<phase, episode, proofEpisode, testEpisode, testVerdict,
         released, directFlipped, badFlip, settlement>>

(***************************************************************************
 * Engine proves the exact frame needs no composition. Only meaningful      *
 * while no effect is active, and stamped with the episode it was taken in. *
 *************************************************************************)
ProveEligibility ==
    /\ phase = "waiting"
    /\ ~effectActive
    /\ phase' = "proven"
    /\ proofEpisode' = episode
    /\ UNCHANGED <<effectActive, episode, testEpisode, testVerdict, released,
         directFlipped, badFlip, settlement>>

(***************************************************************************
 * The backend's atomic test of the exact framebuffer. Its verdict is an    *
 * environment fact -- the driver answers, not this model -- so both        *
 * answers are reachable.                                                   *
 *************************************************************************)
TestPass ==
    /\ phase \in {"proven", "tested"}
    /\ ~effectActive
    /\ phase' = "tested"
    /\ testEpisode' = episode
    /\ testVerdict' = "passed"
    /\ UNCHANGED <<effectActive, episode, proofEpisode, released,
         directFlipped, badFlip, settlement>>

TestRefuse ==
    /\ phase = "proven"
    /\ testVerdict' = "refused"
    /\ phase' = "waiting"
    /\ proofEpisode' = NoEpisode
    /\ UNCHANGED <<effectActive, episode, testEpisode, released,
         directFlipped, badFlip, settlement>>

(***************************************************************************
 * Engine proves every frame; the backend tests only on the eligibility     *
 * edge. So a frame can carry a proof taken in this episode over a test     *
 * taken in the last one, and that asymmetry is the whole reason the two    *
 * stamps are separate. Without this action the test stamp is unfalsifiable *
 * -- its negative control cannot fail, because the proof stamp blocks      *
 * every path that would have exercised it.                                 *
 *************************************************************************)
ReProveAfterEpisodeChange ==
    /\ phase = "tested"
    /\ ~effectActive
    /\ proofEpisode # episode
    /\ proofEpisode' = episode
    /\ UNCHANGED <<phase, effectActive, episode, testEpisode, testVerdict,
         released, directFlipped, badFlip, settlement>>

(***************************************************************************
 * The direct flip. Lawful only under a proof and a passed test that are    *
 * fresh in the current episode; the same facts are re-evaluated into       *
 * `badFlip` so that a weakened guard is a recorded violation rather than   *
 * a silently different model.                                              *
 *                                                                          *
 * The `~effectActive` conjunct is provably unreachable given the stamps    *
 * beside it: activation advances the episode, and every action that        *
 * stamps a proof or a test refuses to run while an effect is active, so a  *
 * live effect always implies a stale stamp. It is stated anyway, in the    *
 * same spirit as `EmittedEffectsAreExecutable`, so that a later change     *
 * decoupling the episode from activation cannot quietly make a flip during *
 * an effect lawful.                                                        *
 *                                                                          *
 * The client buffer is NOT released here. It is on glass now.              *
 *************************************************************************)
DirectFlip ==
    /\ phase = "tested"
    /\ ~effectActive
    /\ proofEpisode = episode
    /\ testEpisode = episode
    /\ testVerdict = "passed"
    /\ phase' = "flipped"
    /\ directFlipped' = TRUE
    /\ badFlip' = (badFlip \/ effectActive \/ proofEpisode # episode
                       \/ testEpisode # episode \/ testVerdict # "passed")
    /\ UNCHANGED <<effectActive, episode, proofEpisode, testEpisode,
         testVerdict, released, settlement>>

(***************************************************************************
 * A rejected real commit after a passed test, or any prepare failure on    *
 * the direct path: the frame falls back to mixed composition and settles   *
 * as an ordinary copy. Never a session failure, and never a Flip           *
 * settlement -- no direct flip happened.                                   *
 *************************************************************************)
CommitRefused ==
    /\ phase = "tested"
    /\ phase' = "completed"
    /\ settlement' = "copy"
    /\ UNCHANGED <<effectActive, episode, proofEpisode, testEpisode,
         testVerdict, released, directFlipped, badFlip>>

(***************************************************************************
 * A successor flip retires the direct frame. Two successors exist: the     *
 * next direct frame, lawful only while no effect is active; and a composed *
 * frame, which is how an activation returns to mixed composition while the *
 * direct frame stays on glass until this exact moment. Either way, this is *
 * the first point at which the client buffer may be released.              *
 *************************************************************************)
SuccessorDirectRetires ==
    /\ phase = "flipped"
    /\ ~effectActive
    /\ phase' = "retired"
    /\ released' = TRUE
    /\ UNCHANGED <<effectActive, episode, proofEpisode, testEpisode,
         testVerdict, directFlipped, badFlip, settlement>>

SuccessorComposedRetires ==
    /\ phase = "flipped"
    /\ phase' = "retired"
    /\ released' = TRUE
    /\ UNCHANGED <<effectActive, episode, proofEpisode, testEpisode,
         testVerdict, directFlipped, badFlip, settlement>>

(***************************************************************************
 * Present feedback settles as Flip: the client's own pixels reached glass  *
 * by a real flip, which is what the reserved disposition exists to say.    *
 *************************************************************************)
CompleteFlip ==
    /\ phase = "retired"
    /\ phase' = "completed"
    /\ settlement' = "flip"
    /\ UNCHANGED <<effectActive, episode, proofEpisode, testEpisode,
         testVerdict, released, directFlipped, badFlip>>

ServiceProgress ==
    \/ ProveEligibility
    \/ ReProveAfterEpisodeChange
    \/ TestPass
    \/ TestRefuse
    \/ DirectFlip
    \/ CommitRefused
    \/ SuccessorDirectRetires
    \/ SuccessorComposedRetires
    \/ CompleteFlip

Next ==
    \/ EffectActivate
    \/ EffectDeactivate
    \/ ServiceProgress

Spec == Init /\ [][Next]_vars
FairSpec == Spec /\ WF_vars(ServiceProgress)

TypeOK ==
    /\ phase \in Phases
    /\ effectActive \in BOOLEAN
    /\ episode \in 0..MaxEpisodes
    /\ proofEpisode \in 0..NoEpisode
    /\ testEpisode \in 0..NoEpisode
    /\ testVerdict \in {"none", "passed", "refused"}
    /\ released \in BOOLEAN
    /\ directFlipped \in BOOLEAN
    /\ badFlip \in BOOLEAN
    /\ settlement \in Settlements

(***************************************************************************
 * The screen is scanning the client's buffer; releasing it would let the   *
 * client draw into displayed pixels.                                       *
 *************************************************************************)
DisplayedClientBufferIsNeverReleased ==
    phase = "flipped" => ~released

(***************************************************************************
 * Release happens only through a successor's retirement, never at submit,  *
 * flip, or effect activation.                                              *
 *************************************************************************)
ReleasedOnlyBySuccessor ==
    released => phase \in {"retired", "completed"}

(***************************************************************************
 * Every flip that ever happens was proven and tested in the episode it     *
 * flipped in. A guard weakened in code shows up here as a recorded bad     *
 * flip rather than as a quietly different behaviour.                       *
 *************************************************************************)
EveryFlipWasEligible == ~badFlip

(***************************************************************************
 * The Flip disposition means a real direct flip. The copy path may never   *
 * produce it, and a fallback settles as copy.                              *
 *************************************************************************)
FlipFeedbackRequiresRealFlip ==
    settlement = "flip" => directFlipped

(***************************************************************************
 * A displayed direct frame is eventually retired by some successor, and a  *
 * settled transaction settles exactly once.                                *
 *************************************************************************)
DisplayedFrameSettles == (phase = "flipped") ~> (phase = "completed")

=============================================================================
