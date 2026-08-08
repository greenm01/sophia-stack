--------------------------- MODULE ShellObservation ---------------------------
EXTENDS Naturals

(***************************************************************************
 * Status: proposed boundary, not an implemented one. This model belongs to *
 * the shell interface direction in docs/sophia-shell-v1-direction.md and   *
 * checks a design before ratification. No shipped code implements it.      *
 *                                                                          *
 * Shell observation of policy-private desktop state. Engine owns committed *
 * layout and holds no tag, view, or workspace concept. A spatial-policy    *
 * process owns tags privately and releases a redacted status projection a  *
 * shell renders. Engine therefore cannot be the publisher, and the feed    *
 * crosses an authority the WM interface never crosses.                     *
 *                                                                          *
 * The model fixes two rules and checks they are sufficient. A status       *
 * projection is released only with a committed layout, so a rejected or    *
 * timed-out proposal never reaches a bar. Policy loss clears the feed and  *
 * the shell's held state in the same step, so a replacement policy cannot  *
 * inherit its predecessor's published tag through the shell.               *
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
    proposedTag,      \* tag whose layout proposal is staged with Engine
    committedTag,     \* tag matching Engine's last committed projection
    committedTags,    \* tags committed during the current epoch
    committedSerial,  \* Engine commit counter
    publishedTag,     \* status projection released to the shell
    publishedEpoch,   \* epoch the released projection belongs to
    shellTag,         \* tag the shell currently displays
    shellEpoch        \* epoch of the shell's held state

vars == <<policyAlive, policyEpoch, policyTag, proposedTag, committedTag,
          committedTags, committedSerial, publishedTag, publishedEpoch,
          shellTag, shellEpoch>>

Init ==
    /\ policyAlive = FALSE
    /\ policyEpoch = 0
    /\ policyTag = NoTag
    /\ proposedTag = NoTag
    /\ committedTag = NoTag
    /\ committedTags = {}
    /\ committedSerial = 0
    /\ publishedTag = NoTag
    /\ publishedEpoch = 0
    /\ shellTag = NoTag
    /\ shellEpoch = 0

(***************************************************************************
 * The session runtime admits one policy peer under a fresh epoch. Engine's *
 * last committed layout survives, but its tag label does not: no live      *
 * policy can vouch for what the on-screen projection means.                *
 *************************************************************************)
PolicyConnect ==
    /\ ~policyAlive
    /\ policyEpoch < MaxEpoch
    /\ policyAlive' = TRUE
    /\ policyEpoch' = policyEpoch + 1
    /\ policyTag' = NoTag
    /\ proposedTag' = NoTag
    /\ committedTag' = NoTag
    /\ committedTags' = {}
    /\ UNCHANGED <<committedSerial, publishedTag, publishedEpoch,
                   shellTag, shellEpoch>>

(***************************************************************************
 * Policy loss is the interesting case. Clearing the feed and the shell's   *
 * held state in the same step is a requirement on the session runtime, not *
 * an observation about it. If the shell had to notice loss on its own,     *
 * ShellStateBelongsToLivePolicy would admit a window in which a bar shows  *
 * a dead policy's tag as current.                                          *
 *************************************************************************)
PolicyDisconnect ==
    /\ policyAlive
    /\ policyAlive' = FALSE
    /\ policyTag' = NoTag
    /\ proposedTag' = NoTag
    /\ committedTag' = NoTag
    /\ committedTags' = {}
    /\ publishedTag' = NoTag
    /\ publishedEpoch' = 0
    /\ shellTag' = NoTag
    /\ shellEpoch' = 0
    /\ UNCHANGED <<policyEpoch, committedSerial>>

SelectTag(tag) ==
    /\ policyAlive
    /\ proposedTag = NoTag
    /\ tag # policyTag
    /\ policyTag' = tag
    /\ proposedTag' = tag
    /\ UNCHANGED <<policyAlive, policyEpoch, committedTag, committedTags,
                   committedSerial, publishedTag, publishedEpoch,
                   shellTag, shellEpoch>>

(***************************************************************************
 * Release is bound to the commit. The status projection and the committed  *
 * layout advance in one step, so no observer can read a tag the screen     *
 * does not show.                                                           *
 *************************************************************************)
CommitLayout ==
    /\ policyAlive
    /\ proposedTag # NoTag
    /\ committedSerial < MaxCommits
    /\ committedTag' = proposedTag
    /\ committedTags' = committedTags \cup {proposedTag}
    /\ committedSerial' = committedSerial + 1
    /\ publishedTag' = proposedTag
    /\ publishedEpoch' = policyEpoch
    /\ proposedTag' = NoTag
    /\ UNCHANGED <<policyAlive, policyEpoch, policyTag, shellTag, shellEpoch>>

(***************************************************************************
 * A rejected or timed-out proposal leaves both the committed layout and    *
 * the released projection untouched. The policy-private tag stays ahead of *
 * both; that divergence is private and must not be observable.             *
 *************************************************************************)
RejectLayout ==
    /\ policyAlive
    /\ proposedTag # NoTag
    /\ proposedTag' = NoTag
    /\ UNCHANGED <<policyAlive, policyEpoch, policyTag, committedTag,
                   committedTags, committedSerial, publishedTag,
                   publishedEpoch, shellTag, shellEpoch>>

ShellObserve ==
    /\ policyAlive
    /\ publishedTag # NoTag
    /\ shellTag # publishedTag
    /\ shellTag' = publishedTag
    /\ shellEpoch' = publishedEpoch
    /\ UNCHANGED <<policyAlive, policyEpoch, policyTag, proposedTag,
                   committedTag, committedTags, committedSerial,
                   publishedTag, publishedEpoch>>

Next ==
    \/ PolicyConnect
    \/ PolicyDisconnect
    \/ \E tag \in Tags : SelectTag(tag)
    \/ CommitLayout
    \/ RejectLayout
    \/ ShellObserve

Spec == Init /\ [][Next]_vars

FairSpec == Spec /\ WF_vars(ShellObserve)

TypeOK ==
    /\ policyAlive \in BOOLEAN
    /\ policyEpoch \in 0..MaxEpoch
    /\ policyTag \in Tags \cup {NoTag}
    /\ proposedTag \in Tags \cup {NoTag}
    /\ committedTag \in Tags \cup {NoTag}
    /\ committedTags \subseteq Tags
    /\ committedSerial \in 0..MaxCommits
    /\ publishedTag \in Tags \cup {NoTag}
    /\ publishedEpoch \in 0..MaxEpoch
    /\ shellTag \in Tags \cup {NoTag}
    /\ shellEpoch \in 0..MaxEpoch

(***************************************************************************
 * The released projection is exactly the committed one. This is the rule   *
 * that fails if a policy publishes when it decides rather than when Engine *
 * commits.                                                                 *
 *************************************************************************)
PublishedStateIsCommitted ==
    publishedTag # NoTag => publishedTag = committedTag

(***************************************************************************
 * A shell may lag the commit, but it may never display a tag that was      *
 * never committed in the current epoch. Lag is acceptable; phantoms are    *
 * not.                                                                     *
 *************************************************************************)
ShellNeverShowsUncommittedState ==
    shellTag # NoTag => shellTag \in committedTags

(***************************************************************************
 * Shell-held policy state always belongs to the live policy. This is the   *
 * guarantee `sophia_wm_v1` gives the WM path extended to the shell path:   *
 * a replacement policy never inherits its predecessor's private state, by  *
 * any route.                                                               *
 *************************************************************************)
ShellStateBelongsToLivePolicy ==
    shellTag # NoTag => /\ policyAlive
                        /\ shellEpoch = policyEpoch
                        /\ publishedEpoch = policyEpoch

(***************************************************************************
 * The policy-private model may run ahead of the screen. That lead is not   *
 * separately stated: it is unobservable because the released projection is *
 * always the committed one and the shell only ever holds a released value. *
 * An invariant demanding shellTag = committedTag would instead forbid the  *
 * observation lag any asynchronous feed has, and TLC refutes it in eight   *
 * steps.                                                                   *
 *************************************************************************)
ShellEventuallyMatchesCommitted ==
    (publishedTag # NoTag /\ shellTag # publishedTag)
        ~> (shellTag = publishedTag \/ publishedTag = NoTag)

=============================================================================
