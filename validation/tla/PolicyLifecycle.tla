--------------------------- MODULE PolicyLifecycle ---------------------------
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************
 * Ordered policy causes, policy-initiated reprojection, replaceable pointer *
 * geometry, configuration activation, frontend settlement, and last-good   *
 * preservation. An activation identity is distinct from its opaque action  *
 * token, so two presses of the same binding remain two ordered operations.  *
 *************************************************************************)

CONSTANTS Activations, MaxQueue, MaxSceneGeneration, MaxConfigGeneration,
          MaxInteractionRevision, MaxPolicyCommits, NoActivation

ASSUME /\ Activations \subseteq (Nat \ {0})
       /\ Activations # {}
       /\ MaxQueue \in (Nat \ {0})
       /\ MaxSceneGeneration \in (Nat \ {0})
       /\ MaxConfigGeneration \in (Nat \ {0})
       /\ MaxInteractionRevision \in (Nat \ {0})
       /\ MaxPolicyCommits \in (Nat \ {0})
       /\ NoActivation = 0
       /\ NoActivation \notin Activations

VARIABLES
    active,
    acceptedActions,
    terminalActions,
    actionQueue,
    currentAction,
    dirtyPending,
    interactionPending,
    currentInteraction,
    configGeneration,
    sceneGeneration,
    phase,
    candidateBase,
    committedSerial,
    lastGoodSerial,
    lastOutcome

vars == <<
    active,
    acceptedActions,
    terminalActions,
    actionQueue,
    currentAction,
    dirtyPending,
    interactionPending,
    currentInteraction,
    configGeneration,
    sceneGeneration,
    phase,
    candidateBase,
    committedSerial,
    lastGoodSerial,
    lastOutcome
>>

ActionSequence == UNION {[1..n -> Activations] : n \in 0..Cardinality(Activations)}

CurrentSequence ==
    IF currentAction = NoActivation THEN <<>> ELSE <<currentAction>>

Items(sequence) == {sequence[index] : index \in 1..Len(sequence)}

FirstAction(sequence) ==
    IF sequence = <<>> THEN NoActivation ELSE Head(sequence)

RemainingActions(sequence) ==
    IF sequence = <<>> THEN <<>> ELSE Tail(sequence)

Init ==
    /\ active = FALSE
    /\ acceptedActions = <<>>
    /\ terminalActions = <<>>
    /\ actionQueue = <<>>
    /\ currentAction = NoActivation
    /\ dirtyPending = FALSE
    /\ interactionPending = 0
    /\ currentInteraction = 0
    /\ configGeneration = 0
    /\ sceneGeneration = 1
    /\ phase = "idle"
    /\ candidateBase = 0
    /\ committedSerial = 0
    /\ lastGoodSerial = 0
    /\ lastOutcome = "none"

Connect ==
    /\ ~active
    /\ active' = TRUE
    /\ UNCHANGED <<acceptedActions, terminalActions, actionQueue,
        currentAction, dirtyPending, interactionPending, currentInteraction,
        configGeneration, sceneGeneration, phase, candidateBase,
        committedSerial, lastGoodSerial, lastOutcome>>

Disconnect ==
    /\ active
    /\ active' = FALSE
    /\ terminalActions' = terminalActions \o CurrentSequence \o actionQueue
    /\ actionQueue' = <<>>
    /\ currentAction' = NoActivation
    /\ dirtyPending' = FALSE
    /\ interactionPending' = 0
    /\ currentInteraction' = 0
    /\ phase' = "idle"
    /\ candidateBase' = 0
    /\ lastOutcome' = "disconnected"
    /\ UNCHANGED <<acceptedActions, configGeneration, sceneGeneration,
        committedSerial, lastGoodSerial>>

EnqueueAction(activation) ==
    /\ active
    /\ activation \in Activations
    /\ activation \notin Items(acceptedActions)
    /\ IF Len(acceptedActions) = 0
          THEN TRUE
          ELSE activation > acceptedActions[Len(acceptedActions)]
    /\ Len(actionQueue) < MaxQueue
    /\ acceptedActions' = Append(acceptedActions, activation)
    /\ actionQueue' = Append(actionQueue, activation)
    /\ UNCHANGED <<active, terminalActions, currentAction, dirtyPending,
        interactionPending, currentInteraction, configGeneration,
        sceneGeneration, phase, candidateBase, committedSerial,
        lastGoodSerial, lastOutcome>>

RequestDirty ==
    /\ active
    /\ dirtyPending' = TRUE
    /\ UNCHANGED <<active, acceptedActions, terminalActions, actionQueue,
        currentAction, interactionPending, currentInteraction,
        configGeneration, sceneGeneration, phase, candidateBase,
        committedSerial, lastGoodSerial, lastOutcome>>

InstallConfig(generation) ==
    /\ active
    /\ phase = "idle"
    /\ generation \in 1..MaxConfigGeneration
    /\ generation > configGeneration
    /\ configGeneration' = generation
    /\ dirtyPending' = TRUE
    /\ UNCHANGED <<active, acceptedActions, terminalActions, actionQueue,
        currentAction, interactionPending, currentInteraction,
        sceneGeneration, phase, candidateBase, committedSerial,
        lastGoodSerial, lastOutcome>>

UpdateInteraction(revision) ==
    /\ active
    /\ revision \in 1..MaxInteractionRevision
    /\ revision > interactionPending
    /\ interactionPending' = revision
    /\ UNCHANGED <<active, acceptedActions, terminalActions, actionQueue,
        currentAction, dirtyPending, currentInteraction, configGeneration,
        sceneGeneration, phase, candidateBase, committedSerial,
        lastGoodSerial, lastOutcome>>

IssueAction ==
    /\ active
    /\ phase = "idle"
    /\ actionQueue # <<>>
    /\ currentAction' = FirstAction(actionQueue)
    /\ actionQueue' = RemainingActions(actionQueue)
    /\ currentInteraction' = 0
    /\ phase' = "requested"
    /\ lastOutcome' = "none"
    /\ UNCHANGED <<active, acceptedActions, terminalActions, dirtyPending,
        interactionPending, configGeneration, sceneGeneration, candidateBase,
        committedSerial, lastGoodSerial>>

IssueDirty ==
    /\ active
    /\ phase = "idle"
    /\ actionQueue = <<>>
    /\ dirtyPending
    /\ currentAction' = NoActivation
    /\ dirtyPending' = FALSE
    /\ currentInteraction' = 0
    /\ phase' = "requested"
    /\ lastOutcome' = "none"
    /\ UNCHANGED <<active, acceptedActions, terminalActions, actionQueue,
        interactionPending, configGeneration, sceneGeneration, candidateBase,
        committedSerial, lastGoodSerial>>

IssueInteraction ==
    /\ active
    /\ phase = "idle"
    /\ actionQueue = <<>>
    /\ ~dirtyPending
    /\ interactionPending > 0
    /\ currentAction' = NoActivation
    /\ currentInteraction' = interactionPending
    /\ interactionPending' = 0
    /\ phase' = "requested"
    /\ lastOutcome' = "none"
    /\ UNCHANGED <<active, acceptedActions, terminalActions, actionQueue,
        dirtyPending, configGeneration, sceneGeneration, candidateBase,
        committedSerial, lastGoodSerial>>

StageProposal ==
    /\ active
    /\ phase = "requested"
    /\ phase' = "configuring"
    /\ candidateBase' = sceneGeneration
    /\ UNCHANGED <<active, acceptedActions, terminalActions, actionQueue,
        currentAction, dirtyPending, interactionPending, currentInteraction,
        configGeneration, sceneGeneration, committedSerial, lastGoodSerial,
        lastOutcome>>

FrontendSettlesChanged ==
    /\ active
    /\ phase = "configuring"
    /\ sceneGeneration < MaxSceneGeneration
    /\ sceneGeneration' = sceneGeneration + 1
    /\ phase' = "requested"
    /\ candidateBase' = 0
    /\ UNCHANGED <<active, acceptedActions, terminalActions, actionQueue,
        currentAction, dirtyPending, interactionPending, currentInteraction,
        configGeneration, committedSerial, lastGoodSerial, lastOutcome>>

CommitProposal ==
    /\ active
    /\ phase = "configuring"
    /\ candidateBase = sceneGeneration
    /\ committedSerial < MaxPolicyCommits
    /\ phase' = "terminal"
    /\ terminalActions' = terminalActions \o CurrentSequence
    /\ currentAction' = NoActivation
    /\ currentInteraction' = 0
    /\ committedSerial' = committedSerial + 1
    /\ lastGoodSerial' = committedSerial + 1
    /\ lastOutcome' = "committed"
    /\ UNCHANGED <<active, acceptedActions, actionQueue, dirtyPending,
        interactionPending, configGeneration, sceneGeneration, candidateBase>>

TimeoutProposal ==
    /\ phase \in {"requested", "configuring"}
    /\ phase' = "terminal"
    /\ terminalActions' = terminalActions \o CurrentSequence
    /\ currentAction' = NoActivation
    /\ currentInteraction' = 0
    /\ lastOutcome' = "timed_out"
    /\ UNCHANGED <<active, acceptedActions, actionQueue, dirtyPending,
        interactionPending, configGeneration, sceneGeneration, candidateBase,
        committedSerial, lastGoodSerial>>

ClearTerminal ==
    /\ phase = "terminal"
    /\ phase' = "idle"
    /\ candidateBase' = 0
    /\ lastOutcome' = "none"
    /\ UNCHANGED <<active, acceptedActions, terminalActions, actionQueue,
        currentAction, dirtyPending, interactionPending, currentInteraction,
        configGeneration, sceneGeneration, committedSerial, lastGoodSerial>>

IssueRequest == IssueAction \/ IssueDirty \/ IssueInteraction
Settlement == FrontendSettlesChanged \/ CommitProposal \/ TimeoutProposal

Next ==
    \/ Connect
    \/ Disconnect
    \/ \E activation \in Activations : EnqueueAction(activation)
    \/ RequestDirty
    \/ \E generation \in 1..MaxConfigGeneration : InstallConfig(generation)
    \/ \E revision \in 1..MaxInteractionRevision : UpdateInteraction(revision)
    \/ IssueRequest
    \/ StageProposal
    \/ Settlement
    \/ ClearTerminal

Spec == Init /\ [][Next]_vars

FairSpec ==
    Spec
    /\ WF_vars(IssueRequest)
    /\ WF_vars(StageProposal)
    /\ WF_vars(Settlement)
    /\ WF_vars(ClearTerminal)

TypeOK ==
    /\ active \in BOOLEAN
    /\ acceptedActions \in ActionSequence
    /\ terminalActions \in ActionSequence
    /\ actionQueue \in ActionSequence
    /\ currentAction \in Activations \cup {NoActivation}
    /\ dirtyPending \in BOOLEAN
    /\ interactionPending \in 0..MaxInteractionRevision
    /\ currentInteraction \in 0..MaxInteractionRevision
    /\ configGeneration \in 0..MaxConfigGeneration
    /\ sceneGeneration \in 1..MaxSceneGeneration
    /\ phase \in {"idle", "requested", "configuring", "terminal"}
    /\ candidateBase \in 0..MaxSceneGeneration
    /\ committedSerial \in 0..MaxPolicyCommits
    /\ lastGoodSerial \in 0..MaxPolicyCommits
    /\ lastOutcome \in {"none", "committed", "timed_out", "disconnected"}

ActionsRemainOrdered ==
    acceptedActions = terminalActions \o CurrentSequence \o actionQueue

LastGoodChangesOnlyOnCommit == lastGoodSerial = committedSerial

NoCommitBeforeFrontendSettlement ==
    lastOutcome = "committed" => phase = "terminal"

PendingCandidateDoesNotReplaceLastGood ==
    phase \in {"requested", "configuring"} =>
        lastGoodSerial = committedSerial

ActionEventuallyTerminates ==
    currentAction # NoActivation ~> currentAction = NoActivation

=============================================================================
