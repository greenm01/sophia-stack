-------------------------- MODULE PolicyProjection --------------------------
EXTENDS Naturals, FiniteSets

(***************************************************************************
 * Atomic logical output projection across policy snapshots, scene changes, *
 * stale replies, timeout, disconnect, and process replacement. Rendering   *
 * preparation and physical output retirement remain in the visual models. *
 *************************************************************************)

CONSTANTS
    Outputs,
    OutputOne,
    OutputTwo,
    Surfaces,
    SurfaceOne,
    SurfaceTwo,
    SurfaceThree,
    Focusable,
    Generations,
    Epochs,
    MaxPolicyCommits,
    NoSurface,
    NoGeneration

ASSUME /\ Outputs # {}
       /\ Outputs = {OutputOne, OutputTwo}
       /\ OutputOne # OutputTwo
       /\ Surfaces = {SurfaceOne, SurfaceTwo, SurfaceThree}
       /\ SurfaceOne # SurfaceTwo
       /\ SurfaceOne # SurfaceThree
       /\ SurfaceTwo # SurfaceThree
       /\ Focusable \subseteq Surfaces
       /\ Generations # {}
       /\ Generations \subseteq (Nat \ {0})
       /\ Epochs # {}
       /\ Epochs \subseteq (Nat \ {0})
       /\ MaxPolicyCommits \in (Nat \ {0})
       /\ NoSurface \notin Surfaces
       /\ NoGeneration = 0

OutputProjection == [Outputs -> SUBSET Surfaces]
OutputFocus == [Outputs -> Surfaces \cup {NoSurface}]

VARIABLES
    active,
    connectionEpoch,
    live,
    snapshotGeneration,
    committedProjection,
    committedFocus,
    policyCommitSerial,
    requestPending,
    requestGeneration,
    proposalPhase,
    proposalEpoch,
    proposalBase,
    proposalIssuedGeneration,
    proposalAffected,
    proposedProjection,
    proposedFocus,
    proposalStartProjection,
    proposalStartFocus,
    proposalStartPolicySerial,
    lastOutcome,
    disconnectedProjection,
    disconnectedFocus,
    disconnectedGeneration

vars == <<
    active,
    connectionEpoch,
    live,
    snapshotGeneration,
    committedProjection,
    committedFocus,
    policyCommitSerial,
    requestPending,
    requestGeneration,
    proposalPhase,
    proposalEpoch,
    proposalBase,
    proposalIssuedGeneration,
    proposalAffected,
    proposedProjection,
    proposedFocus,
    proposalStartProjection,
    proposalStartFocus,
    proposalStartPolicySerial,
    lastOutcome,
    disconnectedProjection,
    disconnectedFocus,
    disconnectedGeneration
>>

EmptyProjection == [output \in Outputs |-> {}]
EmptyFocus == [output \in Outputs |-> NoSurface]

(***************************************************************************
 * Representative projection/focus pairs retain the interleavings under    *
 * study without multiplying every projection by every focus function. The *
 * pairs cover empty, single-output, two-output, stacked, duplicate-surface, *
 * invisible-focus, and nonfocusable-focus candidates.                      *
 *************************************************************************)
CandidateIds == 1..7

ProjectionFor(candidate) ==
    CASE candidate = 1 -> EmptyProjection
      [] candidate = 2 -> [output \in Outputs |->
            IF output = OutputOne THEN {SurfaceOne} ELSE {}]
      [] candidate = 3 -> [output \in Outputs |->
            IF output = OutputOne THEN {SurfaceOne, SurfaceTwo} ELSE {}]
      [] candidate = 4 -> [output \in Outputs |->
            IF output = OutputOne THEN {SurfaceOne} ELSE {SurfaceTwo}]
      [] candidate = 5 -> [output \in Outputs |-> {SurfaceOne}]
      [] candidate = 6 -> [output \in Outputs |->
            IF output = OutputTwo THEN {SurfaceThree} ELSE {}]
      [] OTHER -> EmptyProjection

FocusFor(candidate) ==
    CASE candidate \in {1, 5} -> EmptyFocus
      [] candidate \in {2, 3} -> [output \in Outputs |->
            IF output = OutputOne THEN SurfaceOne ELSE NoSurface]
      [] candidate = 4 -> [output \in Outputs |->
            IF output = OutputTwo THEN SurfaceTwo ELSE NoSurface]
      [] candidate = 6 -> [output \in Outputs |->
            IF output = OutputTwo THEN SurfaceThree ELSE NoSurface]
      [] OTHER -> [output \in Outputs |->
            IF output = OutputOne THEN SurfaceOne ELSE NoSurface]

Init ==
    /\ active = FALSE
    /\ connectionEpoch = NoGeneration
    /\ live = Surfaces
    /\ snapshotGeneration = 1
    /\ committedProjection = EmptyProjection
    /\ committedFocus = EmptyFocus
    /\ policyCommitSerial = 0
    /\ requestPending = FALSE
    /\ requestGeneration = NoGeneration
    /\ proposalPhase = "none"
    /\ proposalEpoch = NoGeneration
    /\ proposalBase = NoGeneration
    /\ proposalIssuedGeneration = NoGeneration
    /\ proposalAffected = {}
    /\ proposedProjection = EmptyProjection
    /\ proposedFocus = EmptyFocus
    /\ proposalStartProjection = EmptyProjection
    /\ proposalStartFocus = EmptyFocus
    /\ proposalStartPolicySerial = 0
    /\ lastOutcome = "none"
    /\ disconnectedProjection = EmptyProjection
    /\ disconnectedFocus = EmptyFocus
    /\ disconnectedGeneration = 1

Connect(epoch) ==
    /\ ~active
    /\ epoch \in Epochs
    /\ epoch > connectionEpoch
    /\ active' = TRUE
    /\ connectionEpoch' = epoch
    /\ UNCHANGED <<
        live, snapshotGeneration, committedProjection, committedFocus,
        policyCommitSerial, requestPending, requestGeneration, proposalPhase,
        proposalEpoch, proposalBase, proposalIssuedGeneration,
        proposalAffected, proposedProjection, proposedFocus,
        proposalStartProjection, proposalStartFocus,
        proposalStartPolicySerial, lastOutcome, disconnectedProjection,
        disconnectedFocus, disconnectedGeneration
        >>

Disconnect ==
    /\ active
    /\ active' = FALSE
    /\ requestPending' = FALSE
    /\ requestGeneration' = NoGeneration
    /\ disconnectedProjection' = committedProjection
    /\ disconnectedFocus' = committedFocus
    /\ disconnectedGeneration' = snapshotGeneration
    /\ UNCHANGED <<
        connectionEpoch, live, snapshotGeneration, committedProjection,
        committedFocus, policyCommitSerial, proposalPhase, proposalEpoch,
        proposalBase, proposalIssuedGeneration, proposalAffected,
        proposedProjection, proposedFocus,
        proposalStartProjection, proposalStartFocus,
        proposalStartPolicySerial, lastOutcome
        >>

IssueSnapshotRequest ==
    /\ active
    /\ proposalPhase = "none"
    /\ ~requestPending
    /\ requestPending' = TRUE
    /\ requestGeneration' = snapshotGeneration
    /\ UNCHANGED <<
        active, connectionEpoch, live, snapshotGeneration,
        committedProjection, committedFocus, policyCommitSerial,
        proposalPhase, proposalEpoch, proposalBase,
        proposalIssuedGeneration, proposalAffected, proposedProjection,
        proposedFocus, proposalStartProjection, proposalStartFocus,
        proposalStartPolicySerial, lastOutcome, disconnectedProjection,
        disconnectedFocus, disconnectedGeneration
        >>

BeginProposal(epoch, base, affected, projection, focus) ==
    /\ active
    /\ proposalPhase = "none"
    /\ requestPending
    /\ epoch \in Epochs
    /\ base \in Generations
    /\ affected \in SUBSET Outputs
    /\ affected # {}
    /\ projection \in OutputProjection
    /\ focus \in OutputFocus
    /\ proposalPhase' = "complete"
    /\ proposalEpoch' = epoch
    /\ proposalBase' = base
    /\ proposalIssuedGeneration' = requestGeneration
    /\ proposalAffected' = affected
    /\ proposedProjection' = projection
    /\ proposedFocus' = focus
    /\ proposalStartProjection' = committedProjection
    /\ proposalStartFocus' = committedFocus
    /\ proposalStartPolicySerial' = policyCommitSerial
    /\ lastOutcome' = "none"
    /\ requestPending' = FALSE
    /\ requestGeneration' = NoGeneration
    /\ UNCHANGED <<
        active, connectionEpoch, live, snapshotGeneration,
        committedProjection, committedFocus, policyCommitSerial,
        disconnectedProjection, disconnectedFocus, disconnectedGeneration
        >>

CandidateProjection(output) ==
    IF output \in proposalAffected
        THEN proposedProjection[output]
        ELSE committedProjection[output]

CandidateFocus(output) ==
    IF output \in proposalAffected
        THEN proposedFocus[output]
        ELSE committedFocus[output]

SurfaceIsUnique(surface) ==
    Cardinality({output \in Outputs : surface \in CandidateProjection(output)})
        <= 1

CandidateIsValid ==
    /\ active
    /\ proposalEpoch = connectionEpoch
    /\ proposalBase = proposalIssuedGeneration
    /\ proposalIssuedGeneration = snapshotGeneration
    /\ \A output \in Outputs : CandidateProjection(output) \subseteq live
    /\ \A surface \in Surfaces : SurfaceIsUnique(surface)
    /\ \A output \in Outputs :
        \/ CandidateFocus(output) = NoSurface
        \/ CandidateFocus(output) \in
            (CandidateProjection(output) \cap Focusable)

CommitProposal ==
    /\ proposalPhase = "complete"
    /\ CandidateIsValid
    /\ policyCommitSerial < MaxPolicyCommits
    /\ proposalPhase' = "settled"
    /\ committedProjection' =
        [output \in Outputs |-> CandidateProjection(output)]
    /\ committedFocus' = [output \in Outputs |-> CandidateFocus(output)]
    /\ policyCommitSerial' = policyCommitSerial + 1
    /\ lastOutcome' = "committed"
    /\ UNCHANGED <<
        active, connectionEpoch, live, snapshotGeneration, proposalEpoch,
        proposalBase, proposalIssuedGeneration, proposalAffected,
        proposedProjection, proposedFocus, requestPending, requestGeneration,
        proposalStartProjection, proposalStartFocus,
        proposalStartPolicySerial, disconnectedProjection, disconnectedFocus,
        disconnectedGeneration
        >>

RejectProposal ==
    /\ proposalPhase = "complete"
    /\ ~CandidateIsValid
    /\ proposalPhase' = "settled"
    /\ IF ~active \/ proposalEpoch # connectionEpoch
          THEN lastOutcome' = "disconnected"
          ELSE IF proposalBase # proposalIssuedGeneration
                  \/ proposalIssuedGeneration # snapshotGeneration
              THEN lastOutcome' = "stale"
              ELSE lastOutcome' = "invalid"
    /\ UNCHANGED <<
        active, connectionEpoch, live, snapshotGeneration,
        committedProjection, committedFocus, policyCommitSerial,
        requestPending, requestGeneration, proposalEpoch, proposalBase,
        proposalIssuedGeneration, proposalAffected, proposedProjection,
        proposedFocus, proposalStartProjection, proposalStartFocus,
        proposalStartPolicySerial, disconnectedProjection, disconnectedFocus,
        disconnectedGeneration
        >>

TimeoutProposal ==
    /\ proposalPhase = "complete"
    /\ proposalPhase' = "settled"
    /\ lastOutcome' = "timed_out"
    /\ UNCHANGED <<
        active, connectionEpoch, live, snapshotGeneration,
        committedProjection, committedFocus, policyCommitSerial,
        requestPending, requestGeneration, proposalEpoch, proposalBase,
        proposalIssuedGeneration, proposalAffected, proposedProjection,
        proposedFocus, proposalStartProjection, proposalStartFocus,
        proposalStartPolicySerial, disconnectedProjection, disconnectedFocus,
        disconnectedGeneration
        >>

ClearSettlement ==
    /\ proposalPhase = "settled"
    /\ proposalPhase' = "none"
    /\ proposalEpoch' = NoGeneration
    /\ proposalBase' = NoGeneration
    /\ proposalIssuedGeneration' = NoGeneration
    /\ proposalAffected' = {}
    /\ proposedProjection' = EmptyProjection
    /\ proposedFocus' = EmptyFocus
    /\ proposalStartProjection' = committedProjection
    /\ proposalStartFocus' = committedFocus
    /\ proposalStartPolicySerial' = policyCommitSerial
    /\ lastOutcome' = "none"
    /\ UNCHANGED <<
        active, connectionEpoch, live, snapshotGeneration,
        committedProjection, committedFocus, policyCommitSerial,
        requestPending, requestGeneration,
        disconnectedProjection, disconnectedFocus, disconnectedGeneration
        >>

SceneChange(nextLive) ==
    /\ proposalPhase # "settled"
    /\ snapshotGeneration + 1 \in Generations
    /\ nextLive \in SUBSET Surfaces
    /\ live' = nextLive
    /\ snapshotGeneration' = snapshotGeneration + 1
    /\ committedProjection' =
        [output \in Outputs |-> committedProjection[output] \cap nextLive]
    /\ committedFocus' =
        [output \in Outputs |->
            IF committedFocus[output] \in
                (committedProjection[output] \cap nextLive \cap Focusable)
                THEN committedFocus[output]
                ELSE NoSurface]
    /\ IF ~active
          THEN /\ disconnectedProjection' = committedProjection'
               /\ disconnectedFocus' = committedFocus'
               /\ disconnectedGeneration' = snapshotGeneration'
          ELSE UNCHANGED <<
              disconnectedProjection, disconnectedFocus,
              disconnectedGeneration
              >>
    /\ UNCHANGED <<
        active, connectionEpoch, policyCommitSerial, proposalPhase,
        requestPending, requestGeneration, proposalEpoch, proposalBase,
        proposalIssuedGeneration, proposalAffected, proposedProjection,
        proposedFocus, proposalStartProjection, proposalStartFocus,
        proposalStartPolicySerial, lastOutcome
        >>

ProposalProgress == CommitProposal \/ RejectProposal \/ TimeoutProposal

Next ==
    \/ \E epoch \in Epochs : Connect(epoch)
    \/ Disconnect
    \/ IssueSnapshotRequest
    \/ \E epoch \in Epochs :
        \E base \in Generations :
            \E affected \in (SUBSET Outputs) \ {{}} :
                \E candidate \in CandidateIds :
                    BeginProposal(
                        epoch,
                        base,
                        affected,
                        ProjectionFor(candidate),
                        FocusFor(candidate))
    \/ ProposalProgress
    \/ ClearSettlement
    \/ \E nextLive \in SUBSET Surfaces : SceneChange(nextLive)

Spec == Init /\ [][Next]_vars

FairSpec == Spec /\ WF_vars(ProposalProgress)

TypeOK ==
    /\ active \in BOOLEAN
    /\ connectionEpoch \in Epochs \cup {NoGeneration}
    /\ live \subseteq Surfaces
    /\ snapshotGeneration \in Generations
    /\ committedProjection \in OutputProjection
    /\ committedFocus \in OutputFocus
    /\ policyCommitSerial \in 0..MaxPolicyCommits
    /\ requestPending \in BOOLEAN
    /\ requestGeneration \in Generations \cup {NoGeneration}
    /\ proposalPhase \in {"none", "complete", "settled"}
    /\ proposalEpoch \in Epochs \cup {NoGeneration}
    /\ proposalBase \in Generations \cup {NoGeneration}
    /\ proposalIssuedGeneration \in Generations \cup {NoGeneration}
    /\ proposalAffected \subseteq Outputs
    /\ proposedProjection \in OutputProjection
    /\ proposedFocus \in OutputFocus
    /\ proposalStartProjection \in OutputProjection
    /\ proposalStartFocus \in OutputFocus
    /\ proposalStartPolicySerial \in Nat
    /\ lastOutcome \in
        {"none", "committed", "stale", "invalid", "timed_out", "disconnected"}
    /\ disconnectedProjection \in OutputProjection
    /\ disconnectedFocus \in OutputFocus
    /\ disconnectedGeneration \in Generations

CommittedSurfacesAreLive ==
    \A output \in Outputs : committedProjection[output] \subseteq live

CommittedSurfaceHasOneOutput ==
    \A surface \in Surfaces :
        Cardinality({output \in Outputs :
            surface \in committedProjection[output]}) <= 1

CommittedFocusIsVisible ==
    \A output \in Outputs :
        \/ committedFocus[output] = NoSurface
        \/ committedFocus[output] \in
            (committedProjection[output] \cap Focusable)

RejectedProposalDidNotPolicyCommit ==
    lastOutcome \in {"stale", "invalid", "timed_out", "disconnected"} =>
        policyCommitSerial = proposalStartPolicySerial

CommittedProposalIsAtomic ==
    lastOutcome = "committed" =>
        /\ policyCommitSerial = proposalStartPolicySerial + 1
        /\ \A output \in proposalAffected :
            /\ committedProjection[output] = proposedProjection[output]
            /\ committedFocus[output] = proposedFocus[output]
        /\ \A output \in Outputs \ proposalAffected :
            /\ committedProjection[output] = proposalStartProjection[output]
            /\ committedFocus[output] = proposalStartFocus[output]

DisconnectedPreservesProjection ==
    ~active /\ snapshotGeneration = disconnectedGeneration =>
        /\ committedProjection = disconnectedProjection
        /\ committedFocus = disconnectedFocus

ProposalEventuallySettles ==
    proposalPhase = "complete" ~> proposalPhase = "settled"

=============================================================================
