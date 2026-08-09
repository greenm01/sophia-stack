--------------------------- MODULE PolicyLifecycle ---------------------------
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************
 * Ordered policy causes, policy-initiated reprojection, replaceable pointer *
 * geometry, configuration activation, frontend settlement (including       *
 * protocol-visible presentation-state acknowledgement), and last-good      *
 * preservation. An activation identity is distinct from its opaque action  *
 * token, so two presses of the same binding remain two ordered operations.  *
 *************************************************************************)

CONSTANTS Activations, Operations, MaxQueue, MaxSceneGeneration, MaxConfigGeneration,
          MaxInteractionRevision, MaxPolicyCommits, NoActivation, NoOperation

ASSUME /\ Activations \subseteq (Nat \ {0})
       /\ Activations # {}
       /\ MaxQueue \in (Nat \ {0})
       /\ MaxSceneGeneration \in (Nat \ {0})
       /\ MaxConfigGeneration \in (Nat \ {0})
       /\ MaxInteractionRevision \in (Nat \ {0})
       /\ MaxPolicyCommits \in (Nat \ {0})
       /\ NoActivation = 0
       /\ NoActivation \notin Activations
       /\ Operations \subseteq (Nat \ {0})
       /\ Operations # {}
       /\ NoOperation = 0
       /\ NoOperation \notin Operations

VARIABLES
    active,
    acceptedActions,
    consumedActions,
    lastActivation,
    terminalActions,
    actionQueue,
    currentAction,
    dirtyPending,
    interactionPending,
    currentInteraction,
    acceptedOperations,
    terminalOperations,
    currentOperation,
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
    consumedActions,
    lastActivation,
    terminalActions,
    actionQueue,
    currentAction,
    dirtyPending,
    interactionPending,
    currentInteraction,
    acceptedOperations,
    terminalOperations,
    currentOperation,
    configGeneration,
    sceneGeneration,
    phase,
    candidateBase,
    committedSerial,
    lastGoodSerial,
    lastOutcome
>>

ActionSequence == UNION {[1..n -> Activations] : n \in 0..Cardinality(Activations)}
OperationSequence == UNION {[1..n -> Operations] : n \in 0..Cardinality(Operations)}

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
    /\ consumedActions = <<>>
    /\ lastActivation = NoActivation
    /\ terminalActions = <<>>
    /\ actionQueue = <<>>
    /\ currentAction = NoActivation
    /\ dirtyPending = FALSE
    /\ interactionPending = 0
    /\ currentInteraction = 0
    /\ acceptedOperations = <<>>
    /\ terminalOperations = <<>>
    /\ currentOperation = NoOperation
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
    /\ UNCHANGED <<acceptedActions, consumedActions, lastActivation,
        terminalActions, actionQueue,
        currentAction, dirtyPending, interactionPending, currentInteraction,
        acceptedOperations, terminalOperations, currentOperation,
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
    /\ terminalOperations' =
        IF currentOperation = NoOperation
        THEN terminalOperations
        ELSE Append(terminalOperations, currentOperation)
    /\ currentOperation' = NoOperation
    /\ phase' = "idle"
    /\ candidateBase' = 0
    /\ lastOutcome' = "disconnected"
    /\ UNCHANGED <<acceptedActions, consumedActions, lastActivation,
        acceptedOperations, configGeneration, sceneGeneration,
        committedSerial, lastGoodSerial>>

EnqueueAction(activation) ==
    /\ active
    /\ activation \in Activations
    /\ activation \notin Items(acceptedActions)
    /\ activation > lastActivation
    /\ Len(actionQueue) < MaxQueue
    /\ acceptedActions' = Append(acceptedActions, activation)
    /\ lastActivation' = activation
    /\ actionQueue' = Append(actionQueue, activation)
    /\ UNCHANGED <<active, consumedActions, terminalActions, currentAction, dirtyPending,
        interactionPending, currentInteraction, acceptedOperations,
        terminalOperations, currentOperation, configGeneration,
        sceneGeneration, phase, candidateBase, committedSerial,
        lastGoodSerial, lastOutcome>>

RejectSaturatedAction(activation) ==
    /\ active
    /\ activation \in Activations
    /\ activation > lastActivation
    /\ Len(actionQueue) = MaxQueue
    /\ consumedActions' = Append(consumedActions, activation)
    /\ lastActivation' = activation
    /\ UNCHANGED <<active, acceptedActions, terminalActions, actionQueue,
        currentAction, dirtyPending, interactionPending, currentInteraction,
        acceptedOperations, terminalOperations, currentOperation, configGeneration,
        sceneGeneration, phase, candidateBase, committedSerial, lastGoodSerial,
        lastOutcome>>

RequestDirty ==
    /\ active
    /\ dirtyPending' = TRUE
    /\ UNCHANGED <<active, acceptedActions, consumedActions, lastActivation,
        terminalActions, actionQueue,
        currentAction, interactionPending, currentInteraction, acceptedOperations,
        terminalOperations, currentOperation,
        configGeneration, sceneGeneration, phase, candidateBase,
        committedSerial, lastGoodSerial, lastOutcome>>

InstallConfig(generation) ==
    /\ active
    /\ phase = "idle"
    /\ generation \in 1..MaxConfigGeneration
    /\ generation > configGeneration
    /\ configGeneration' = generation
    /\ dirtyPending' = TRUE
    /\ UNCHANGED <<active, acceptedActions, consumedActions, lastActivation,
        terminalActions, actionQueue,
        currentAction, interactionPending, currentInteraction, acceptedOperations,
        terminalOperations, currentOperation,
        sceneGeneration, phase, candidateBase, committedSerial,
        lastGoodSerial, lastOutcome>>

UpdateInteraction(revision) ==
    /\ active
    /\ revision \in 1..MaxInteractionRevision
    /\ revision > interactionPending
    /\ interactionPending' = revision
    /\ UNCHANGED <<active, acceptedActions, consumedActions, lastActivation,
        terminalActions, actionQueue,
        currentAction, dirtyPending, currentInteraction, acceptedOperations,
        terminalOperations, currentOperation, configGeneration,
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
    /\ UNCHANGED <<active, acceptedActions, consumedActions, lastActivation,
        terminalActions, dirtyPending,
        interactionPending, acceptedOperations, terminalOperations, currentOperation,
        configGeneration, sceneGeneration, candidateBase,
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
    /\ UNCHANGED <<active, acceptedActions, consumedActions, lastActivation,
        terminalActions, actionQueue,
        interactionPending, acceptedOperations, terminalOperations, currentOperation,
        configGeneration, sceneGeneration, candidateBase,
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
    /\ UNCHANGED <<active, acceptedActions, consumedActions, lastActivation,
        terminalActions, actionQueue,
        dirtyPending, acceptedOperations, terminalOperations, currentOperation,
        configGeneration, sceneGeneration, candidateBase,
        committedSerial, lastGoodSerial>>

StageProposal ==
    /\ active
    /\ phase = "requested"
    /\ phase' = "configuring"
    /\ candidateBase' = sceneGeneration
    /\ UNCHANGED <<active, acceptedActions, consumedActions, lastActivation,
        terminalActions, actionQueue,
        currentAction, dirtyPending, interactionPending, currentInteraction,
        acceptedOperations, terminalOperations, currentOperation,
        configGeneration, sceneGeneration, committedSerial, lastGoodSerial,
        lastOutcome>>

FrontendSettlesChanged ==
    /\ active
    /\ phase = "configuring"
    /\ sceneGeneration < MaxSceneGeneration
    /\ sceneGeneration' = sceneGeneration + 1
    /\ phase' = "requested"
    /\ candidateBase' = 0
    /\ UNCHANGED <<active, acceptedActions, consumedActions, lastActivation,
        terminalActions, actionQueue,
        currentAction, dirtyPending, interactionPending, currentInteraction,
        acceptedOperations, terminalOperations, currentOperation,
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
    /\ UNCHANGED <<active, acceptedActions, consumedActions, lastActivation,
        actionQueue, dirtyPending,
        interactionPending, acceptedOperations, terminalOperations, currentOperation,
        configGeneration, sceneGeneration, candidateBase>>

TimeoutProposal ==
    /\ phase \in {"requested", "configuring"}
    /\ phase' = "terminal"
    /\ terminalActions' = terminalActions \o CurrentSequence
    /\ currentAction' = NoActivation
    /\ currentInteraction' = 0
    /\ lastOutcome' = "timed_out"
    /\ UNCHANGED <<active, acceptedActions, consumedActions, lastActivation,
        actionQueue, dirtyPending,
        interactionPending, acceptedOperations, terminalOperations, currentOperation,
        configGeneration, sceneGeneration, candidateBase,
        committedSerial, lastGoodSerial>>

ClearTerminal ==
    /\ phase = "terminal"
    /\ phase' = "idle"
    /\ candidateBase' = 0
    /\ lastOutcome' = "none"
    /\ UNCHANGED <<active, acceptedActions, consumedActions, lastActivation,
        terminalActions, actionQueue,
        currentAction, dirtyPending, interactionPending, currentInteraction,
        acceptedOperations, terminalOperations, currentOperation,
        configGeneration, sceneGeneration, committedSerial, lastGoodSerial>>

RequestOperation(operation) ==
    /\ active
    /\ phase = "idle"
    /\ currentOperation = NoOperation
    /\ operation \in Operations
    /\ operation \notin Items(acceptedOperations)
    /\ acceptedOperations' = Append(acceptedOperations, operation)
    /\ currentOperation' = operation
    /\ phase' = "operation"
    /\ UNCHANGED <<active, acceptedActions, consumedActions, lastActivation,
        terminalActions, actionQueue,
        currentAction, dirtyPending, interactionPending, currentInteraction,
        terminalOperations, configGeneration, sceneGeneration, candidateBase,
        committedSerial, lastGoodSerial, lastOutcome>>

SettleOperation ==
    /\ active
    /\ phase = "operation"
    /\ currentOperation # NoOperation
    /\ terminalOperations' = Append(terminalOperations, currentOperation)
    /\ currentOperation' = NoOperation
    /\ phase' = "idle"
    /\ UNCHANGED <<active, acceptedActions, consumedActions, lastActivation,
        terminalActions, actionQueue,
        currentAction, dirtyPending, interactionPending, currentInteraction,
        acceptedOperations, configGeneration, sceneGeneration, candidateBase,
        committedSerial, lastGoodSerial, lastOutcome>>

CancelInteraction ==
    /\ active
    /\ phase = "idle"
    /\ interactionPending > 0
    /\ interactionPending' = 0
    /\ UNCHANGED <<active, acceptedActions, consumedActions, lastActivation,
        terminalActions, actionQueue,
        currentAction, dirtyPending, currentInteraction, acceptedOperations,
        terminalOperations, currentOperation, configGeneration, sceneGeneration,
        phase, candidateBase, committedSerial, lastGoodSerial, lastOutcome>>

IssueRequest == IssueAction \/ IssueDirty \/ IssueInteraction
Settlement == FrontendSettlesChanged \/ CommitProposal \/ TimeoutProposal

Next ==
    \/ Connect
    \/ Disconnect
    \/ \E activation \in Activations : EnqueueAction(activation)
    \/ \E activation \in Activations : RejectSaturatedAction(activation)
    \/ RequestDirty
    \/ \E generation \in 1..MaxConfigGeneration : InstallConfig(generation)
    \/ \E revision \in 1..MaxInteractionRevision : UpdateInteraction(revision)
    \/ CancelInteraction
    \/ \E operation \in Operations : RequestOperation(operation)
    \/ SettleOperation
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
    /\ WF_vars(SettleOperation)

TypeOK ==
    /\ active \in BOOLEAN
    /\ acceptedActions \in ActionSequence
    /\ consumedActions \in ActionSequence
    /\ lastActivation \in Activations \cup {NoActivation}
    /\ terminalActions \in ActionSequence
    /\ actionQueue \in ActionSequence
    /\ currentAction \in Activations \cup {NoActivation}
    /\ dirtyPending \in BOOLEAN
    /\ interactionPending \in 0..MaxInteractionRevision
    /\ currentInteraction \in 0..MaxInteractionRevision
    /\ configGeneration \in 0..MaxConfigGeneration
    /\ sceneGeneration \in 1..MaxSceneGeneration
    /\ acceptedOperations \in OperationSequence
    /\ terminalOperations \in OperationSequence
    /\ currentOperation \in Operations \cup {NoOperation}
    /\ phase \in {"idle", "requested", "configuring", "terminal", "operation"}
    /\ candidateBase \in 0..MaxSceneGeneration
    /\ committedSerial \in 0..MaxPolicyCommits
    /\ lastGoodSerial \in 0..MaxPolicyCommits
    /\ lastOutcome \in {"none", "committed", "timed_out", "disconnected"}

ActionsRemainOrdered ==
    acceptedActions = terminalActions \o CurrentSequence \o actionQueue

SaturatedActionsAreConsumed ==
    /\ Items(consumedActions) \cap Items(acceptedActions) = {}
    /\ Len(consumedActions) = Cardinality(Items(consumedActions))

OperationsSettleExactlyOnce ==
    acceptedOperations = terminalOperations \o
        (IF currentOperation = NoOperation THEN <<>> ELSE <<currentOperation>>)

LastGoodChangesOnlyOnCommit == lastGoodSerial = committedSerial

NoCommitBeforeFrontendSettlement ==
    lastOutcome = "committed" => phase = "terminal"

PendingCandidateDoesNotReplaceLastGood ==
    phase \in {"requested", "configuring"} =>
        lastGoodSerial = committedSerial

ActionEventuallyTerminates ==
    currentAction # NoActivation ~> currentAction = NoActivation

OperationEventuallyTerminates ==
    currentOperation # NoOperation ~> currentOperation = NoOperation

=============================================================================
