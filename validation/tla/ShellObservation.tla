--------------------------- MODULE ShellObservation ---------------------------
EXTENDS Naturals

(***************************************************************************
 * Status: ratified boundary implemented by the canonical Engine reducer.   *
 * docs/sophia-indicator-descriptor.md and checks a design before           *
 * ratification. No shipped code implements it.                             *
 *                                                                          *
 * Policy-private desktop status carried on the layout commit. Engine holds *
 * no tag, view, or workspace concept, so it cannot author this state; a    *
 * spatial-policy process owns it privately. The descriptor rides the       *
 * layout proposal, and Engine commits and retains it with the geometry.    *
 *                                                                          *
 * There is deliberately no publish action. Engine's committed descriptor   *
 * is the published value, so a rejected or timed-out proposal cannot reach *
 * an observer and no ordering rule is needed to prevent it.                *
 *                                                                          *
 * Tier 0 needs no observer at all: Engine chrome renders `engineTag`       *
 * directly, so the rendered state is the committed state by construction.  *
 * The observer variables model a later tier-1 client holding its own copy, *
 * which is where the remaining guarantee has to be stated.                 *
 *************************************************************************)

CONSTANTS Tags, MaxEpoch, MaxCommits, NoTag

ASSUME /\ Tags \subseteq (Nat \ {0})
       /\ Tags # {}
       /\ MaxEpoch \in (Nat \ {0})
       /\ MaxCommits \in (Nat \ {0})
       /\ NoTag = 0
       /\ NoTag \notin Tags

VARIABLES
    policyAlive,      \* spatial-policy peer is admitted
    policyEpoch,      \* session-assigned connection epoch
    policyTag,        \* policy-private active tag
    proposedTag,      \* descriptor riding the staged layout proposal
    engineTag,        \* Engine's committed descriptor; also the published value
    engineEpoch,      \* epoch that committed it
    engineSerial,     \* Engine commit counter
    committedTags,    \* tags committed during the current epoch
    published,        \* every (serial, tag, epoch) triple Engine ever exposed
    observerTag,      \* tier-1 observer's held copy
    observerEpoch,
    observerSerial

vars == <<policyAlive, policyEpoch, policyTag, proposedTag, engineTag,
          engineEpoch, engineSerial, committedTags, published, observerTag,
          observerEpoch, observerSerial>>

Init ==
    /\ policyAlive = FALSE
    /\ policyEpoch = 0
    /\ policyTag = NoTag
    /\ proposedTag = NoTag
    /\ engineTag = NoTag
    /\ engineEpoch = 0
    /\ engineSerial = 0
    /\ committedTags = {}
    /\ published = {}
    /\ observerTag = NoTag
    /\ observerEpoch = 0
    /\ observerSerial = 0

(***************************************************************************
 * A fresh policy epoch clears Engine's descriptor. The committed layout    *
 * survives, but no live policy can say what it means until the replacement *
 * commits, so the status is empty rather than inherited.                   *
 *                                                                          *
 * The epoch transition is itself a publication point. TLC refuted an       *
 * earlier version that advanced the epoch without recording the cleared    *
 * triple: an observer could then read a state Engine had never exposed.    *
 * Implementations must treat epoch change as an announced transition, not  *
 * as private bookkeeping.                                                  *
 *************************************************************************)
PolicyConnect ==
    /\ ~policyAlive
    /\ policyEpoch < MaxEpoch
    /\ policyAlive' = TRUE
    /\ policyEpoch' = policyEpoch + 1
    /\ policyTag' = NoTag
    /\ proposedTag' = NoTag
    /\ engineTag' = NoTag
    /\ engineEpoch' = policyEpoch + 1
    /\ committedTags' = {}
    /\ published' = published \cup
           {<<engineSerial, NoTag, policyEpoch + 1>>}
    /\ UNCHANGED <<engineSerial, observerTag, observerEpoch, observerSerial>>

(***************************************************************************
 * Engine clears its own state on peer loss. Nothing reaches into the       *
 * observer, and nothing needs to: the observer cannot be silently wrong,   *
 * only behind, and it converges by reading again.                          *
 *************************************************************************)
PolicyDisconnect ==
    /\ policyAlive
    /\ policyAlive' = FALSE
    /\ policyTag' = NoTag
    /\ proposedTag' = NoTag
    /\ engineTag' = NoTag
    /\ committedTags' = {}
    /\ published' = published \cup {<<engineSerial, NoTag, engineEpoch>>}
    /\ UNCHANGED <<policyEpoch, engineEpoch, engineSerial, observerTag,
                   observerEpoch, observerSerial>>

SelectTag(tag) ==
    /\ policyAlive
    /\ proposedTag = NoTag
    /\ tag # policyTag
    /\ policyTag' = tag
    /\ proposedTag' = tag
    /\ UNCHANGED <<policyAlive, policyEpoch, engineTag, engineEpoch,
                   engineSerial, committedTags, published, observerTag,
                   observerEpoch, observerSerial>>

(***************************************************************************
 * Geometry and descriptor commit in one step. This is the whole mechanism. *
 *************************************************************************)
CommitLayout ==
    /\ policyAlive
    /\ proposedTag # NoTag
    /\ engineSerial < MaxCommits
    /\ engineTag' = proposedTag
    /\ engineEpoch' = policyEpoch
    /\ engineSerial' = engineSerial + 1
    /\ committedTags' = committedTags \cup {proposedTag}
    /\ published' = published \cup
           {<<engineSerial + 1, proposedTag, policyEpoch>>}
    /\ proposedTag' = NoTag
    /\ UNCHANGED <<policyAlive, policyEpoch, policyTag, observerTag,
                   observerEpoch, observerSerial>>

(***************************************************************************
 * A rejected proposal discards its descriptor with its geometry. No        *
 * separate suppression step exists because no separate publish step does.  *
 *************************************************************************)
RejectLayout ==
    /\ policyAlive
    /\ proposedTag # NoTag
    /\ proposedTag' = NoTag
    /\ UNCHANGED <<policyAlive, policyEpoch, policyTag, engineTag,
                   engineEpoch, engineSerial, committedTags, published,
                   observerTag, observerEpoch, observerSerial>>

ObserverRead ==
    /\ <<observerSerial, observerTag, observerEpoch>>
           # <<engineSerial, engineTag, engineEpoch>>
    /\ observerTag' = engineTag
    /\ observerEpoch' = engineEpoch
    /\ observerSerial' = engineSerial
    /\ UNCHANGED <<policyAlive, policyEpoch, policyTag, proposedTag,
                   engineTag, engineEpoch, engineSerial, committedTags,
                   published>>

Next ==
    \/ PolicyConnect
    \/ PolicyDisconnect
    \/ \E tag \in Tags : SelectTag(tag)
    \/ CommitLayout
    \/ RejectLayout
    \/ ObserverRead

Spec == Init /\ [][Next]_vars

FairSpec == Spec /\ WF_vars(ObserverRead)

TypeOK ==
    /\ policyAlive \in BOOLEAN
    /\ policyEpoch \in 0..MaxEpoch
    /\ policyTag \in Tags \cup {NoTag}
    /\ proposedTag \in Tags \cup {NoTag}
    /\ engineTag \in Tags \cup {NoTag}
    /\ engineEpoch \in 0..MaxEpoch
    /\ engineSerial \in 0..MaxCommits
    /\ committedTags \subseteq Tags
    /\ published \subseteq ((0..MaxCommits) \X (Tags \cup {NoTag})
                              \X (0..MaxEpoch))
    /\ observerTag \in Tags \cup {NoTag}
    /\ observerEpoch \in 0..MaxEpoch
    /\ observerSerial \in 0..MaxCommits

(***************************************************************************
 * Engine never holds a descriptor that was not committed in the current    *
 * epoch. With no publish action, this is what makes a rejected proposal    *
 * unobservable.                                                            *
 *************************************************************************)
EnginePublishesOnlyCommitted ==
    engineTag # NoTag => engineTag \in committedTags

(***************************************************************************
 * Engine's descriptor always belongs to the live policy. A replacement     *
 * cannot inherit its predecessor's status, because Engine clears its own   *
 * state rather than relying on anyone else to do it.                       *
 *************************************************************************)
EngineStateBelongsToLivePolicy ==
    engineTag # NoTag => /\ policyAlive
                         /\ engineEpoch = policyEpoch

(***************************************************************************
 * The tier-1 guarantee. An observer may lag, but every triple it holds is  *
 * one Engine actually exposed. It is never torn and never invents a        *
 * combination, so it can always tell whether it is current by comparing    *
 * the serial. Being behind is recoverable; being silently wrong is not.    *
 *************************************************************************)
ObserverHoldsAPublishedTriple ==
    observerSerial # 0 =>
        <<observerSerial, observerTag, observerEpoch>> \in published

ObserverConverges ==
    <<observerSerial, observerTag, observerEpoch>>
        # <<engineSerial, engineTag, engineEpoch>>
    ~> <<observerSerial, observerTag, observerEpoch>>
        = <<engineSerial, engineTag, engineEpoch>>

=============================================================================
