-------------------------- MODULE PolicyConnection --------------------------
EXTENDS Naturals, FiniteSets

(***************************************************************************
 * Connection ownership for one exclusive Sophia policy role. The model    *
 * covers negotiation, bounded proposal transfer, disconnect, replacement, *
 * and settlement of work already queued by the transport worker. It omits *
 * byte offsets and socket syscalls.                                        *
 *************************************************************************)

CONSTANTS
    Clients,
    OldClient,
    NewClient,
    Revisions,
    Capabilities,
    ServerMaxRevision,
    ServerCapabilities,
    NoClient,
    Transactions,
    NoTransaction,
    MaxEpoch,
    MaxChunks

ASSUME /\ Clients # {}
       /\ Clients = {OldClient, NewClient}
       /\ OldClient # NewClient
       /\ Revisions # {}
       /\ Revisions \subseteq (Nat \ {0})
       /\ {1, 2} \subseteq Revisions
       /\ ServerMaxRevision \in Revisions
       /\ ServerCapabilities \subseteq Capabilities
       /\ NoClient \notin Clients
       /\ Transactions # {}
       /\ NoTransaction \notin Transactions
       /\ MaxEpoch \in (Nat \ {0})
       /\ MaxChunks \in (Nat \ {0})

ClientMaxRevision(client) == IF client = OldClient THEN 1 ELSE 2

ClientCapabilities(client) ==
    IF client = OldClient THEN {"bindings"} ELSE Capabilities

SelectedRevision(client) ==
    IF ClientMaxRevision(client) < ServerMaxRevision
        THEN ClientMaxRevision(client)
        ELSE ServerMaxRevision

TransferKeys ==
    {<<client, connectionEpoch, transaction>> :
        client \in Clients,
        connectionEpoch \in 1..MaxEpoch,
        transaction \in Transactions}

VARIABLES
    active,
    epoch,
    negotiated,
    selectedRevision,
    selectedCapabilities,
    transfer,
    expectedChunks,
    receivedChunks,
    transferTransaction,
    proposalQueued,
    queuedClient,
    queuedEpoch,
    queuedTransaction,
    queuedComplete,
    usedTransactions,
    completed,
    admitted,
    discarded

vars == <<
    active,
    epoch,
    negotiated,
    selectedRevision,
    selectedCapabilities,
    transfer,
    expectedChunks,
    receivedChunks,
    transferTransaction,
    proposalQueued,
    queuedClient,
    queuedEpoch,
    queuedTransaction,
    queuedComplete,
    usedTransactions,
    completed,
    admitted,
    discarded
>>

Init ==
    /\ active = NoClient
    /\ epoch = 0
    /\ negotiated = FALSE
    /\ selectedRevision = 0
    /\ selectedCapabilities = {}
    /\ transfer = "none"
    /\ expectedChunks = 0
    /\ receivedChunks = 0
    /\ transferTransaction = NoTransaction
    /\ proposalQueued = FALSE
    /\ queuedClient = NoClient
    /\ queuedEpoch = 0
    /\ queuedTransaction = NoTransaction
    /\ queuedComplete = FALSE
    /\ usedTransactions = {}
    /\ completed = {}
    /\ admitted = {}
    /\ discarded = {}

Connect(client) ==
    /\ active = NoClient
    /\ client \in Clients
    /\ epoch < MaxEpoch
    /\ active' = client
    /\ epoch' = epoch + 1
    /\ negotiated' = FALSE
    /\ selectedRevision' = 0
    /\ selectedCapabilities' = {}
    /\ UNCHANGED <<
        transfer, expectedChunks, receivedChunks, transferTransaction,
        proposalQueued, queuedClient, queuedEpoch, queuedTransaction,
        queuedComplete, usedTransactions, completed, admitted, discarded
        >>

Negotiate ==
    /\ active \in Clients
    /\ ~negotiated
    /\ negotiated' = TRUE
    /\ selectedRevision' = SelectedRevision(active)
    /\ selectedCapabilities' =
        ClientCapabilities(active) \cap ServerCapabilities
    /\ UNCHANGED <<
        active, epoch, transfer, expectedChunks, receivedChunks,
        transferTransaction, proposalQueued, queuedClient, queuedEpoch,
        queuedTransaction, queuedComplete, usedTransactions, completed,
        admitted, discarded
        >>

BeginProposal(transaction, chunkCount) ==
    /\ negotiated
    /\ transfer = "none"
    /\ ~proposalQueued
    /\ transaction \in Transactions
    /\ <<active, epoch, transaction>> \notin usedTransactions
    /\ chunkCount \in 1..MaxChunks
    /\ transfer' = "proposal"
    /\ expectedChunks' = chunkCount
    /\ receivedChunks' = 0
    /\ transferTransaction' = transaction
    /\ usedTransactions' =
        usedTransactions \cup {<<active, epoch, transaction>>}
    /\ UNCHANGED <<
        active, epoch, negotiated, selectedRevision, selectedCapabilities,
        proposalQueued, queuedClient, queuedEpoch, queuedTransaction,
        queuedComplete, completed, admitted, discarded
        >>

AppendProposalChunk ==
    /\ transfer = "proposal"
    /\ receivedChunks < expectedChunks
    /\ receivedChunks' = receivedChunks + 1
    /\ UNCHANGED <<
        active, epoch, negotiated, selectedRevision, selectedCapabilities,
        transfer, expectedChunks, transferTransaction, proposalQueued,
        queuedClient, queuedEpoch, queuedTransaction, queuedComplete,
        usedTransactions, completed, admitted, discarded
        >>

FinishProposal ==
    /\ transfer = "proposal"
    /\ receivedChunks = expectedChunks
    /\ transfer' = "none"
    /\ expectedChunks' = 0
    /\ receivedChunks' = 0
    /\ transferTransaction' = NoTransaction
    /\ proposalQueued' = TRUE
    /\ queuedClient' = active
    /\ queuedEpoch' = epoch
    /\ queuedTransaction' = transferTransaction
    /\ queuedComplete' = TRUE
    /\ completed' = completed \cup {<<active, epoch, transferTransaction>>}
    /\ UNCHANGED <<
        active, epoch, negotiated, selectedRevision, selectedCapabilities,
        usedTransactions, admitted, discarded
        >>

Disconnect ==
    /\ active \in Clients
    /\ active' = NoClient
    /\ negotiated' = FALSE
    /\ selectedRevision' = 0
    /\ selectedCapabilities' = {}
    /\ transfer' = "none"
    /\ expectedChunks' = 0
    /\ receivedChunks' = 0
    /\ transferTransaction' = NoTransaction
    /\ UNCHANGED <<
        epoch, proposalQueued, queuedClient, queuedEpoch, queuedTransaction,
        queuedComplete, usedTransactions, completed, admitted, discarded
        >>

SettleQueued ==
    /\ proposalQueued
    /\ proposalQueued' = FALSE
    /\ queuedClient' = NoClient
    /\ queuedEpoch' = 0
    /\ queuedTransaction' = NoTransaction
    /\ queuedComplete' = FALSE
    /\ IF /\ active = queuedClient
           /\ epoch = queuedEpoch
           /\ negotiated
           /\ queuedComplete
          THEN /\ admitted' = admitted \cup
                   {<<queuedClient, queuedEpoch, queuedTransaction>>}
               /\ UNCHANGED discarded
          ELSE /\ discarded' = discarded \cup
                   {<<queuedClient, queuedEpoch, queuedTransaction>>}
               /\ UNCHANGED admitted
    /\ UNCHANGED <<
        active, epoch, negotiated, selectedRevision, selectedCapabilities,
        transfer, expectedChunks, receivedChunks, transferTransaction,
        usedTransactions, completed
        >>

NegotiationProgress == Negotiate \/ Disconnect

TransferProgress == AppendProposalChunk \/ FinishProposal \/ Disconnect

Next ==
    \/ \E client \in Clients : Connect(client)
    \/ Negotiate
    \/ \E transaction \in Transactions :
        \E chunkCount \in 1..MaxChunks :
            BeginProposal(transaction, chunkCount)
    \/ AppendProposalChunk
    \/ FinishProposal
    \/ Disconnect
    \/ SettleQueued

Spec == Init /\ [][Next]_vars

FairSpec ==
    /\ Spec
    /\ WF_vars(NegotiationProgress)
    /\ WF_vars(TransferProgress)
    /\ WF_vars(SettleQueued)

TypeOK ==
    /\ active \in Clients \cup {NoClient}
    /\ epoch \in 0..MaxEpoch
    /\ negotiated \in BOOLEAN
    /\ selectedRevision \in Revisions \cup {0}
    /\ selectedCapabilities \subseteq Capabilities
    /\ transfer \in {"none", "proposal"}
    /\ expectedChunks \in 0..MaxChunks
    /\ receivedChunks \in 0..MaxChunks
    /\ transferTransaction \in Transactions \cup {NoTransaction}
    /\ proposalQueued \in BOOLEAN
    /\ queuedClient \in Clients \cup {NoClient}
    /\ queuedEpoch \in 0..MaxEpoch
    /\ queuedTransaction \in Transactions \cup {NoTransaction}
    /\ queuedComplete \in BOOLEAN
    /\ usedTransactions \subseteq TransferKeys
    /\ completed \subseteq TransferKeys
    /\ admitted \subseteq TransferKeys
    /\ discarded \subseteq TransferKeys

NegotiationIsCompatible ==
    negotiated =>
        /\ active \in Clients
        /\ selectedRevision = SelectedRevision(active)
        /\ selectedRevision <= ClientMaxRevision(active)
        /\ selectedRevision <= ServerMaxRevision
        /\ selectedCapabilities =
            ClientCapabilities(active) \cap ServerCapabilities

NoUseBeforeNegotiation ==
    transfer = "proposal" => negotiated

TransferIsBounded ==
    /\ receivedChunks <= expectedChunks
    /\ (transfer = "none") =>
        /\ expectedChunks = 0
        /\ receivedChunks = 0
        /\ transferTransaction = NoTransaction

QueuedTransferIsComplete ==
    proposalQueued =>
        /\ queuedClient \in Clients
        /\ queuedEpoch \in 1..MaxEpoch
        /\ queuedTransaction \in Transactions
        /\ queuedComplete
        /\ <<queuedClient, queuedEpoch, queuedTransaction>> \in completed

IdleQueueIsEmpty ==
    ~proposalQueued =>
        /\ queuedClient = NoClient
        /\ queuedEpoch = 0
        /\ queuedTransaction = NoTransaction
        /\ ~queuedComplete

OnlyCompletedWorkIsAdmitted == admitted \subseteq completed

StaleWorkIsNotAdmitted == admitted \cap discarded = {}

QueuedWorkEventuallySettles == proposalQueued ~> ~proposalQueued

PartialTransferEventuallyEnds ==
    (transfer = "proposal") ~> (transfer = "none")

ConnectionAttemptEventuallyResolves ==
    (active \in Clients /\ ~negotiated) ~>
        (negotiated \/ active = NoClient)

=============================================================================
