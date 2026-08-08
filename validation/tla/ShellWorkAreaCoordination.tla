---------------------- MODULE ShellWorkAreaCoordination ----------------------
EXTENDS Naturals

(***************************************************************************
 * Status: target pre-schema architecture. No production shell runtime or   *
 * sophia_shell_v1 schema implements this state machine.                    *
 *                                                                          *
 * A shell candidate owns both visuals and an output reservation. Engine    *
 * derives a work-area snapshot from that exact candidate, WM answers that  *
 * exact snapshot, and Engine may present only the coherent three-part      *
 * bundle. Candidate preparation may overlap policy work, but presentation  *
 * cannot splice together generations or connection epochs.                *
 *                                                                          *
 * Normal shell or WM failure preserves the last presented bundle. The      *
 * model intentionally proves safety only: a failed or absent policy may    *
 * prevent future presentation indefinitely. Security surfaces and input    *
 * preemption belong to InputAuthorityArbitration, not this model.           *
 *************************************************************************)

CONSTANTS Reservations, NoReservation, MaxGeneration, MaxEpoch

ASSUME /\ NoReservation \in Reservations
       /\ MaxGeneration \in (Nat \ {0})
       /\ MaxEpoch \in (Nat \ {0})

BundleRecords ==
    [generation: 1..MaxGeneration,
     reservation: Reservations,
     wmBasis: Reservations,
     candidateShellEpoch: 1..MaxEpoch,
     requestShellEpoch: 1..MaxEpoch,
     proposalShellEpoch: 1..MaxEpoch,
     requestWmEpoch: 1..MaxEpoch,
     proposalWmEpoch: 1..MaxEpoch,
     ready: BOOLEAN]

RejectedRecords ==
    [beforeGeneration: 0..MaxGeneration,
     afterGeneration: 0..MaxGeneration,
     beforeReservation: Reservations,
     afterReservation: Reservations,
     beforeWmBasis: Reservations,
     afterWmBasis: Reservations]

VARIABLES
    shellAlive,
    shellEpoch,
    wmAlive,
    wmEpoch,
    candidateGeneration,
    candidateReservation,
    candidateShellEpoch,
    candidateReady,
    requestGeneration,
    requestReservation,
    requestShellEpoch,
    requestWmEpoch,
    proposalGeneration,
    proposalWmBasis,
    proposalShellEpoch,
    proposalWmEpoch,
    presentedGeneration,
    presentedReservation,
    presentedWmBasis,
    presentedShellEpoch,
    presentedWmEpoch,
    presentedHistory,
    rejectedAttempts,
    lastOutcome

vars ==
    <<shellAlive, shellEpoch, wmAlive, wmEpoch,
      candidateGeneration, candidateReservation, candidateShellEpoch,
      candidateReady, requestGeneration, requestReservation,
      requestShellEpoch, requestWmEpoch, proposalGeneration,
      proposalWmBasis, proposalShellEpoch, proposalWmEpoch,
      presentedGeneration, presentedReservation, presentedWmBasis,
      presentedShellEpoch, presentedWmEpoch, presentedHistory,
      rejectedAttempts, lastOutcome>>

Init ==
    /\ shellAlive = TRUE
    /\ shellEpoch = 1
    /\ wmAlive = TRUE
    /\ wmEpoch = 1
    /\ candidateGeneration = 0
    /\ candidateReservation = NoReservation
    /\ candidateShellEpoch = 0
    /\ candidateReady = FALSE
    /\ requestGeneration = 0
    /\ requestReservation = NoReservation
    /\ requestShellEpoch = 0
    /\ requestWmEpoch = 0
    /\ proposalGeneration = 0
    /\ proposalWmBasis = NoReservation
    /\ proposalShellEpoch = 0
    /\ proposalWmEpoch = 0
    /\ presentedGeneration = 0
    /\ presentedReservation = NoReservation
    /\ presentedWmBasis = NoReservation
    /\ presentedShellEpoch = 0
    /\ presentedWmEpoch = 0
    /\ presentedHistory = {}
    /\ rejectedAttempts = {}
    /\ lastOutcome = "none"

SubmitShellCandidate(generation, reservation) ==
    /\ shellAlive
    /\ generation \in 1..MaxGeneration
    /\ generation > candidateGeneration
    /\ generation > presentedGeneration
    /\ candidateGeneration' = generation
    /\ candidateReservation' = reservation
    /\ candidateShellEpoch' = shellEpoch
    /\ candidateReady' = FALSE
    /\ requestGeneration' = 0
    /\ requestReservation' = NoReservation
    /\ requestShellEpoch' = 0
    /\ requestWmEpoch' = 0
    /\ proposalGeneration' = 0
    /\ proposalWmBasis' = NoReservation
    /\ proposalShellEpoch' = 0
    /\ proposalWmEpoch' = 0
    /\ lastOutcome' = "candidate"
    /\ UNCHANGED <<shellAlive, shellEpoch, wmAlive, wmEpoch,
                    presentedGeneration, presentedReservation,
                    presentedWmBasis, presentedShellEpoch, presentedWmEpoch,
                    presentedHistory, rejectedAttempts>>

CandidateReady ==
    /\ shellAlive
    /\ candidateGeneration # 0
    /\ candidateShellEpoch = shellEpoch
    /\ ~candidateReady
    /\ candidateReady' = TRUE
    /\ lastOutcome' = "ready"
    /\ UNCHANGED <<shellAlive, shellEpoch, wmAlive, wmEpoch,
                    candidateGeneration, candidateReservation,
                    candidateShellEpoch, requestGeneration,
                    requestReservation, requestShellEpoch, requestWmEpoch,
                    proposalGeneration, proposalWmBasis,
                    proposalShellEpoch, proposalWmEpoch,
                    presentedGeneration, presentedReservation,
                    presentedWmBasis, presentedShellEpoch, presentedWmEpoch,
                    presentedHistory, rejectedAttempts>>

IssueWorkAreaSnapshot ==
    /\ shellAlive
    /\ wmAlive
    /\ candidateGeneration # 0
    /\ candidateShellEpoch = shellEpoch
    /\ requestGeneration' = candidateGeneration
    /\ requestReservation' = candidateReservation
    /\ requestShellEpoch' = candidateShellEpoch
    /\ requestWmEpoch' = wmEpoch
    /\ proposalGeneration' = 0
    /\ proposalWmBasis' = NoReservation
    /\ proposalShellEpoch' = 0
    /\ proposalWmEpoch' = 0
    /\ lastOutcome' = "requested"
    /\ UNCHANGED <<shellAlive, shellEpoch, wmAlive, wmEpoch,
                    candidateGeneration, candidateReservation,
                    candidateShellEpoch, candidateReady,
                    presentedGeneration, presentedReservation,
                    presentedWmBasis, presentedShellEpoch, presentedWmEpoch,
                    presentedHistory, rejectedAttempts>>

(***************************************************************************
 * The transport can deliver a stale or malformed proposal. Its fields are  *
 * therefore nondeterministic here; AttemptPresent owns exact validation.    *
 *************************************************************************)
SubmitWmProposal(generation, reservation, proposalShell, proposalWm) ==
    /\ wmAlive
    /\ requestGeneration # 0
    /\ generation \in 1..MaxGeneration
    /\ reservation \in Reservations
    /\ proposalShell \in 1..MaxEpoch
    /\ proposalWm \in 1..MaxEpoch
    /\ proposalGeneration' = generation
    /\ proposalWmBasis' = reservation
    /\ proposalShellEpoch' = proposalShell
    /\ proposalWmEpoch' = proposalWm
    /\ lastOutcome' = "proposed"
    /\ UNCHANGED <<shellAlive, shellEpoch, wmAlive, wmEpoch,
                    candidateGeneration, candidateReservation,
                    candidateShellEpoch, candidateReady, requestGeneration,
                    requestReservation, requestShellEpoch, requestWmEpoch,
                    presentedGeneration, presentedReservation,
                    presentedWmBasis, presentedShellEpoch, presentedWmEpoch,
                    presentedHistory, rejectedAttempts>>

ExactBundle ==
    /\ shellAlive
    /\ wmAlive
    /\ candidateReady
    /\ candidateGeneration # 0
    /\ candidateShellEpoch = shellEpoch
    /\ requestGeneration = candidateGeneration
    /\ requestReservation = candidateReservation
    /\ requestShellEpoch = candidateShellEpoch
    /\ requestWmEpoch = wmEpoch
    /\ proposalGeneration = requestGeneration
    /\ proposalWmBasis = requestReservation
    /\ proposalShellEpoch = requestShellEpoch
    /\ proposalWmEpoch = requestWmEpoch

SuccessfulRecord ==
    [generation |-> candidateGeneration,
     reservation |-> candidateReservation,
     wmBasis |-> proposalWmBasis,
     candidateShellEpoch |-> candidateShellEpoch,
     requestShellEpoch |-> requestShellEpoch,
     proposalShellEpoch |-> proposalShellEpoch,
     requestWmEpoch |-> requestWmEpoch,
     proposalWmEpoch |-> proposalWmEpoch,
     ready |-> candidateReady]

RejectedRecord ==
    [beforeGeneration |-> presentedGeneration,
     afterGeneration |-> presentedGeneration,
     beforeReservation |-> presentedReservation,
     afterReservation |-> presentedReservation,
     beforeWmBasis |-> presentedWmBasis,
     afterWmBasis |-> presentedWmBasis]

AttemptPresent ==
    /\ proposalGeneration # 0
    /\ IF ExactBundle
          THEN /\ presentedGeneration' = candidateGeneration
               /\ presentedReservation' = candidateReservation
               /\ presentedWmBasis' = proposalWmBasis
               /\ presentedShellEpoch' = candidateShellEpoch
               /\ presentedWmEpoch' = proposalWmEpoch
               /\ presentedHistory' = presentedHistory \cup
                       {SuccessfulRecord}
               /\ rejectedAttempts' = rejectedAttempts
               /\ candidateGeneration' = 0
               /\ candidateReservation' = NoReservation
               /\ candidateShellEpoch' = 0
               /\ candidateReady' = FALSE
               /\ lastOutcome' = "presented"
          ELSE /\ UNCHANGED <<presentedGeneration, presentedReservation,
                                presentedWmBasis, presentedShellEpoch,
                                presentedWmEpoch, presentedHistory,
                                candidateGeneration, candidateReservation,
                                candidateShellEpoch, candidateReady>>
               /\ rejectedAttempts' = rejectedAttempts \cup
                       {RejectedRecord}
               /\ lastOutcome' = "rejected"
    /\ requestGeneration' = 0
    /\ requestReservation' = NoReservation
    /\ requestShellEpoch' = 0
    /\ requestWmEpoch' = 0
    /\ proposalGeneration' = 0
    /\ proposalWmBasis' = NoReservation
    /\ proposalShellEpoch' = 0
    /\ proposalWmEpoch' = 0
    /\ UNCHANGED <<shellAlive, shellEpoch, wmAlive, wmEpoch>>

ShellDisconnect ==
    /\ shellAlive
    /\ shellAlive' = FALSE
    /\ candidateGeneration' = 0
    /\ candidateReservation' = NoReservation
    /\ candidateShellEpoch' = 0
    /\ candidateReady' = FALSE
    /\ requestGeneration' = 0
    /\ requestReservation' = NoReservation
    /\ requestShellEpoch' = 0
    /\ requestWmEpoch' = 0
    /\ proposalGeneration' = 0
    /\ proposalWmBasis' = NoReservation
    /\ proposalShellEpoch' = 0
    /\ proposalWmEpoch' = 0
    /\ lastOutcome' = "shell_disconnected"
    /\ UNCHANGED <<shellEpoch, wmAlive, wmEpoch, presentedGeneration,
                    presentedReservation, presentedWmBasis,
                    presentedShellEpoch, presentedWmEpoch,
                    presentedHistory, rejectedAttempts>>

ShellReconnect ==
    /\ ~shellAlive
    /\ shellEpoch < MaxEpoch
    /\ shellAlive' = TRUE
    /\ shellEpoch' = shellEpoch + 1
    /\ lastOutcome' = "shell_reconnected"
    /\ UNCHANGED <<wmAlive, wmEpoch, candidateGeneration,
                    candidateReservation, candidateShellEpoch, candidateReady,
                    requestGeneration, requestReservation, requestShellEpoch,
                    requestWmEpoch, proposalGeneration, proposalWmBasis,
                    proposalShellEpoch, proposalWmEpoch, presentedGeneration,
                    presentedReservation, presentedWmBasis,
                    presentedShellEpoch, presentedWmEpoch,
                    presentedHistory, rejectedAttempts>>

WmDisconnect ==
    /\ wmAlive
    /\ wmAlive' = FALSE
    /\ requestGeneration' = 0
    /\ requestReservation' = NoReservation
    /\ requestShellEpoch' = 0
    /\ requestWmEpoch' = 0
    /\ proposalGeneration' = 0
    /\ proposalWmBasis' = NoReservation
    /\ proposalShellEpoch' = 0
    /\ proposalWmEpoch' = 0
    /\ lastOutcome' = "wm_disconnected"
    /\ UNCHANGED <<shellAlive, shellEpoch, wmEpoch, candidateGeneration,
                    candidateReservation, candidateShellEpoch, candidateReady,
                    presentedGeneration, presentedReservation,
                    presentedWmBasis, presentedShellEpoch, presentedWmEpoch,
                    presentedHistory, rejectedAttempts>>

WmReconnect ==
    /\ ~wmAlive
    /\ wmEpoch < MaxEpoch
    /\ wmAlive' = TRUE
    /\ wmEpoch' = wmEpoch + 1
    /\ lastOutcome' = "wm_reconnected"
    /\ UNCHANGED <<shellAlive, shellEpoch, candidateGeneration,
                    candidateReservation, candidateShellEpoch, candidateReady,
                    requestGeneration, requestReservation, requestShellEpoch,
                    requestWmEpoch, proposalGeneration, proposalWmBasis,
                    proposalShellEpoch, proposalWmEpoch, presentedGeneration,
                    presentedReservation, presentedWmBasis,
                    presentedShellEpoch, presentedWmEpoch,
                    presentedHistory, rejectedAttempts>>

Next ==
    \/ \E generation \in 1..MaxGeneration,
          reservation \in Reservations :
          SubmitShellCandidate(generation, reservation)
    \/ CandidateReady
    \/ IssueWorkAreaSnapshot
    \/ \E generation \in 1..MaxGeneration,
          reservation \in Reservations,
          proposalShell \in 1..MaxEpoch,
          proposalWm \in 1..MaxEpoch :
          SubmitWmProposal(generation, reservation, proposalShell, proposalWm)
    \/ AttemptPresent
    \/ ShellDisconnect
    \/ ShellReconnect
    \/ WmDisconnect
    \/ WmReconnect

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ shellAlive \in BOOLEAN
    /\ shellEpoch \in 1..MaxEpoch
    /\ wmAlive \in BOOLEAN
    /\ wmEpoch \in 1..MaxEpoch
    /\ candidateGeneration \in 0..MaxGeneration
    /\ candidateReservation \in Reservations
    /\ candidateShellEpoch \in 0..MaxEpoch
    /\ candidateReady \in BOOLEAN
    /\ requestGeneration \in 0..MaxGeneration
    /\ requestReservation \in Reservations
    /\ requestShellEpoch \in 0..MaxEpoch
    /\ requestWmEpoch \in 0..MaxEpoch
    /\ proposalGeneration \in 0..MaxGeneration
    /\ proposalWmBasis \in Reservations
    /\ proposalShellEpoch \in 0..MaxEpoch
    /\ proposalWmEpoch \in 0..MaxEpoch
    /\ presentedGeneration \in 0..MaxGeneration
    /\ presentedReservation \in Reservations
    /\ presentedWmBasis \in Reservations
    /\ presentedShellEpoch \in 0..MaxEpoch
    /\ presentedWmEpoch \in 0..MaxEpoch
    /\ presentedHistory \subseteq BundleRecords
    /\ rejectedAttempts \subseteq RejectedRecords
    /\ lastOutcome \in
          {"none", "candidate", "ready", "requested", "proposed",
           "presented", "rejected", "shell_disconnected",
           "shell_reconnected", "wm_disconnected", "wm_reconnected"}

PresentedBundlesAreCoherent ==
    /\ presentedReservation = presentedWmBasis
    /\ \A record \in presentedHistory :
          record.reservation = record.wmBasis

PresentedBundlesWereReady ==
    \A record \in presentedHistory : record.ready

PresentedBundlesMatchExactGenerationAndEpoch ==
    \A record \in presentedHistory :
        /\ record.candidateShellEpoch = record.requestShellEpoch
        /\ record.requestShellEpoch = record.proposalShellEpoch
        /\ record.requestWmEpoch = record.proposalWmEpoch

PresentedBundleIsRecorded ==
    presentedGeneration # 0 =>
      \E record \in presentedHistory :
        /\ record.generation = presentedGeneration
        /\ record.reservation = presentedReservation
        /\ record.wmBasis = presentedWmBasis
        /\ record.candidateShellEpoch = presentedShellEpoch
        /\ record.proposalWmEpoch = presentedWmEpoch

RejectedAttemptsPreservePresentedBundle ==
    \A record \in rejectedAttempts :
        /\ record.beforeGeneration = record.afterGeneration
        /\ record.beforeReservation = record.afterReservation
        /\ record.beforeWmBasis = record.afterWmBasis

=============================================================================
