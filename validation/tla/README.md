# Visual Transition Model

**Role:** bounded validation model, not production architecture or a refinement
proof of the Rust implementation.

`VisualRetirement.tla` explores the asynchronous lifetime of immutable visual
candidates across two outputs and two generations. Each logical output is backed
by one or more heads, and one output is a two-head mirror group, so a single run
exercises per-head preparation, partial physical submission, joint retirement
within a group, and independent retirement between groups. A transaction may
name both outputs, but its feedback waits for both output retirements while each
output retains its own committed generation. This is the smallest scope that
still explores out-of-order retirement, supersession, and mirror-group member
loss. It excludes X11 objects, application metadata, pixel content, renderer
handles, and native KMS objects.
The join is stated over heads and flips rather than over buffers, which is what
lets the scanout architecture change without the model changing. It held when a
mirror group shared one framebuffer and holds unchanged when each head owns a
buffer at its own mode: what makes a group presented is that every screen shows
the frame, not that they read it from the same memory.

The model checks that:

- every submitted head belongs to a completely prepared output cohort;
- no physical head or partially submitted output cohort is in flight for two
  generations, and overlapping output cohorts cannot both be active;
- a successful commit follows retirement of every output required by that
  candidate, and a logical output retires only when every one of its heads has
  flipped;
- distinct logical outputs never share a head, so one group's flip cannot retire
  another group's output;
- a candidate whose mirror group lost a head settles as `head_lost`, never
  commits, and drops the lost lease without counting it as a flip;
- no candidate reaches a terminal outcome while a required output remains
  logically pending;
- a late generation loses an output to the newer one that already committed it,
  and loses it before the kernel rather than after: supersession reaches only
  outputs with no submitted head, a superseded output never becomes that
  output's committed or input generation, and a superseded candidate never
  publishes feedback;
- input never leads physical visual state, and a successful transaction publishes
  matching input only after every required output retires;
- successful feedback exists only for a committed generation;
- active, submitted, or committed resources cannot be released, which now also
  means no generation is released while any head still scans it out; and
- admitted work eventually reaches one terminal settlement without arbitrary
  failure being available to satisfy it.

The checked configuration explores 2,432,103 generated states and 968,679
distinct states to depth 28.

Fairness is per action rather than over the whole progress disjunction, and
`Settle` is outside it. That distinction is the difference between a settlement
property and a tautology: `Settle` is enabled in every non-terminal state, so a
single lumped assumption is discharged by failing, and the property holds even
with no productive action assumed fair at all. A temporary control restoring the
lumped assumption confirms this -- it passes while assuming nothing about
preparation, submission, retirement, or completion. Head loss stays outside
fairness because nothing guarantees a connector disappears; page-flip callbacks
stay inside it because a kernel that accepted a flip does report it.

Temporary negative controls independently remove the prepare-all guard,
whole-cohort ownership guard, transaction feedback join, release guard,
submit-time staleness guard, and supersession guard, and weaken joint
retirement to accept one flipped head; each violates its corresponding enabled
invariant, reaching `SubmittedOutputsAreNeverSuperseded`,
`OutputGenerationDominatesHistory`, and `OutputCommitAfterHeadRetirement`
respectively for the last three.
Removing supersession from the fairness assumption violates the settlement
property, which is what shows that property is now carried by work that
advances.

`PresentFrameOwnership.tla` isolates the output-frame association needed by
software Present. It allows an unrelated frame to submit and retire before the
Present frame, but requires feedback to remain false until the exact bound
frame retires. It also permits the backend to submit a successor DMA frame
after observing the Present retirement but before reducing its feedback. That
successor cannot steal or block the captured retirement. Under weak fairness,
waiting Present work eventually reaches settlement.

`PresentCopyOwnership.tla` isolates composited DMA-BUF ownership. Capture
creates a staged compositor image without releasing the client source. Exact
page-flip retirement promotes that image, makes the source idle, and only then
permits Copy completion. A failure before retirement instead removes the
staged image and settles Skip. The model forbids client-owned content from
becoming the displayed owner and requires every staged Present to reach Copy
or rollback under weak fairness.

`SurfaceContentStream.tla` models a Present followed by the three representative
software operations in the X11 content stream: SHM, clear, and core drawing.
It requires those operations to remain bounded and FIFO until the exact Present
retires, while unrelated work may progress. Retirement publishes the Present
generation before any deferred operation can advance the visible generation;
weak fairness then requires the complete backlog to drain.

`GeometryFeedback.tla` separates a surface's full X geometry from pixel
readiness. It explores move-only and resize commits, an unchanged proposal,
control delivery before or after logical commit, and a timeout whose FIFO
rollback follows a late target control. It requires move-only work to reuse
committed pixels, resize commits to wait for pixels, unchanged geometry to
remain silent, and every settled Engine/X Authority pair to converge. This is
the protocol invariant behind `ConfigureNotify`: position feedback is not a
resize side effect.

`PolicyConnection.tla` models one exclusive public policy role across
negotiation, bounded begin/chunk/end proposal assembly, disconnect, replacement,
and settlement of work already queued by the transport worker. It checks that
selected revisions and capabilities are mutually supported, no transfer begins
before negotiation, only complete work is admitted, stale queued work is
discarded, and responsive negotiation and transfer work eventually settle.
The bounded configuration explores 3,321 generated states and 2,177 distinct
states to depth 23.

`PolicyProjection.tla` models the first public WM interface's complete scene
snapshot and affected-output replacement semantics. It explores stale snapshot
generations, invalid focus or visibility, surface removal, timeout, disconnect,
and a replacement policy epoch. It requires live unique projected surfaces,
visible focus, one logical multi-output commit, no policy commit on rejection,
and last-projection preservation while policy is absent.
The bounded configuration explores 1,342,325 generated states and 524,396
distinct states to depth 18.

`PolicyLifecycle.tla` models the causes around that projection boundary. It
keeps distinct activation identities in FIFO order, prioritizes them over
replaceable policy-dirty and pointer updates, applies configuration only at an
idle boundary, consumes a saturated shortcut as an explicit bounded rejection,
settles opaque session operations exactly once, cancels pending interactions,
retries against a fresh scene when frontend facts change, and preserves the
last-good serial until a settled proposal commits. Repeated
opaque action tokens use distinct activation identities and therefore cannot
be collapsed into one focus, movement, view, or layout operation. The bounded
configuration explores 348,608 generated states and 65,467 distinct states to
depth 26.

`PolicySettlementRecovery.tla` isolates the public Hagia owner-loop boundary
after proposal validation. It separates non-mutating staged revalidation,
frontend settlement, one logical reducer/layout promotion, capacity-one
terminal delivery, transport loss, forced abort, and connection replacement.
It requires the last-good reducer and layout serials to remain coherent, a
failed settlement to preserve both, old-epoch terminal ownership to disappear
before restart, and each delivered terminal identity to occur at most once.
The bounded configuration explores 314 generated states and 224 distinct
states to depth 36. A temporary prepare-time reducer promotion immediately
violates `LastGoodIsCoherent`, preserving the implementation defect found by
the modeling pass as a negative control.

`PolicyOutputSettlement.tla` isolates output loss and exact-identity return
during a complete two-output policy settlement. Each output carries the head
set backing it, so a mirror group that loses one connector while keeping its
identity is a scene change like any other. It requires the canonical scene
generation to fence the staged candidate against both the output set and the
head sets, both Engine layout and reducer projections to promote as one
last-good state, and a returned output to carry a new generation. The bounded
configuration explores 2,070 generated states and 1,369 distinct states to
depth 13. Temporary negative controls independently allowed a prepared
candidate to skip its final topology recheck, allowed an output to return
without advancing generation, and allowed a head loss not to advance the scene.
TLC violated `CommittedTopologyWasCurrent`, `ReappearedOutputIsFresh`, and
`StagedHeadsCannotGoStaleSilently`, respectively.

`OutputTopologyLifecycle.tla` carries that policy-level topology rule through
the native session owner's split ownership-transfer boundary. It separates a
replaceable rescan hint, routed-input epoch revocation, old scanout retirement,
complete runtime rebuild, one logical scanout/pointer/RandR/policy publication
barrier, current policy settlement, and exact first presentation. The logical
barrier abstracts sequential cross-process settlement while input is
quarantined; it does not assert simultaneous IPC visibility. No-output and one
bounded rebuild failure remain input-quarantined and can recover after a later
valid rescan. A logical output carries a live head count, so losing one
connector of a mirror group republishes the full epoch exactly as losing an
output does; what consumers hold is checked against the head set that is
actually live. Scenario correspondence and exclusions are recorded in
`validation/specula/output-topology-lifecycle-modeling-brief.md`.
The bounded configuration explores 5,109,131 generated and 784,338 distinct
states to depth 32. A temporary negative control let a head loss leave the
observed epoch alone; TLC violated `PublishedHeadsAreCurrent`.

`PolicyRefreshLifecycle.tla` isolates the revision-1 refresh contract added for
native Hagia policy. It admits only strictly increasing private generations,
coalesces idle dirty output scopes, retains a newer dirty generation that
arrives during an in-flight relayout, requires active-output switches to cover
both old and new outputs, and promotes reducer state plus visible layout as one
settlement. The bounded configuration explores 175 generated states and 71
distinct states to depth 8. A temporary candidate that promoted the active
output without the corresponding layout immediately violated
`ActiveOutputSettlesAtomically`.

`ShellWorkAreaCoordination.tla` is a target pre-schema model for future native
shell reservations. It binds a ready shell candidate and reservation to the
exact derived work-area request, WM connection epoch, and answering projection
before one logical presentation. Stale, torn, or unready attempts are rejected,
and normal shell or WM loss preserves the previous complete bundle. The model
does not promise progress while either optional desktop component is absent and
does not correspond to a production shell runtime. Its bounded configuration
explores 189,816 generated states and 12,278 distinct states to depth 23.

`LegacyWmProjection.tla` models exact complete-snapshot replacement, direct
workspace assignment, workspace activation, and delayed private Configure or
Focus requests. It requires cached membership to remain unique, mapping to
equal the authoritative active-workspace projection, and translation to expose
only currently mapped surfaces. Weak fairness also requires the finite request
backlog to settle. The bounded configuration explores 32,563 generated states
and 2,106 distinct states to depth 11.

`LegacyWmResponseBoundary.tla` models the compatibility limit imposed by an
unmodified legacy WM: its X requests carry no Sophia transaction identity.
Successful collection therefore requires a validated registered grab and a
final quiet boundary. A hard deadline fails and quarantines the process;
restart clears every private reply stage before later work begins. The model
requires every emitted reply to belong to its collecting request. Its bounded
configuration explores 27,667 generated states and 9,489 distinct states to
depth 42.

`PixelSilentAdmission.tla` distinguishes presentation intent from complete
pixels. A first timeout without a safe extent preserves the standing target,
owner loop, and one bounded retry. Later pixels may complete admission;
persistent silence withdraws it without killing the owner. The bounded
configuration explores 14 generated states and 11 distinct states to depth 5,
including both liveness properties.

`TargetResolvedInput.tla` models output-local presented interaction snapshots,
non-reusable target identities, deterministic ownership and occlusion,
per-seat/device/contact capture, modal membership, independently issued local
coordinate grants, and lifecycle cancellation. `TargetInputPacing.tla` isolates
the replaceable continuous slot from ordered begin, discrete, completion,
cancellation, endpoint-revocation, and security barriers.

`TargetInputPacing.tla` also models device acquisition, which is the producer
the compositor cannot decline to have happen: the packet already exists when
capacity is examined, so a full queue is a choice of disposition rather than an
absence of work. Three choices are modelled -- bounded deferral, which declines
to read without dropping anything and has a ceiling; scoped endpoint closure,
which drops the arrival but spends reserved capacity on the terminating
boundary; and terminal failure. Closure is per seat and names its tokens
separately from security revocation, because only revocation is entitled to
leave a stream with no boundary. The bounded configuration explores 1,969,760
generated states and 567,966 distinct states to depth 21.

What the reserve buys is now stated rather than implied. Two slots per active
seat is exactly what escalation spends, so the admission bound is what makes
escalation always possible instead of a decoration on a guard.

Seven temporary negative controls independently remove `Drain` fairness, the
deferral ceiling, the discard accounting, the boundary flush at closure, the
saturation report, the reserve in the acquisition predicate, and in-order
draining. TLC then violates `QueueEventuallyEmpties`, `DeferralIsBounded`,
`AcquisitionIsConserved`, `EndpointCloseIsScoped`, `SaturationIsRecorded`,
`BoundaryCapacityIsReserved`, and `DeliveryPreservesOrder` respectively. The
fairness control is the one that matters most here: the four new actions all sit
outside fairness, and removing the assumption on `Drain` still fails the
property, so admitting a deferral disposition did not quietly convert progress
into a tautology.

An earlier draft checked `EscalationCanAlwaysFlush` and constrained
`deferralTicks` in both `TypeOK` and `DeferralIsBounded`. Neither survived a
control: the first is a strict consequence of `BoundaryCapacityIsReserved` and
no edit can break one without the other, and the second meant `TypeOK` failed
first so the ceiling was never the invariant under test. `TypeOK` now states the
type and `DeferralIsBounded` alone states the bound.

`FrameServiceArbitration.tla` isolates how frame service arbitrates the three
owners of one output: a native pending frame, a queued GPU present, and a
waiting software present. It models the reducer's emit gate and the handler's
own precondition as separate conjuncts, because the defect it was written for
lived in the gap between them. The reducer admitted a present that the handler
then silently refused, while withholding the only effect that would have
drained what the handler was refusing over; neither owner could advance, and
nothing re-deferred the present, so the two waited on each other permanently.
The bounded configuration explores 177 generated states and 96 distinct states
to depth 14.

`EmittedEffectsAreExecutable` is the invariant that keeps the two halves from
drifting apart again: it states each handler precondition as a consequence of
the reducer gate that emits it. A handler guard that merely repeats its gate is
then provably unreachable, which is what makes a silent deferral impossible
rather than merely unlikely. Staging is the case where the mismatch is fatal
instead of silent, so `ServiceNeverCrashes` covers it separately.

Six temporary negative controls independently restore the old primary
suppression, drop the global-idle requirement from the staging gate, drop the
pending-frame requirement from the presentation gate, remove staging from
fairness, and let a present reach flight without occupying the kernel. TLC then
violates `PresentSettles`, `EmittedEffectsAreExecutable` (and
`ServiceNeverCrashes` once the stronger invariant is unlisted),
`EmittedEffectsAreExecutable`, `SoftwareSettles`, and `OneSubmissionInFlight`
respectively. The first is the production deadlock exactly: TLC reaches
`pendingFrame = TRUE` with `presentState = "queued"` and no action remains
enabled. Environment actions stay outside fairness and both producers are
budgeted, so liveness here is a question about arbitration rather than about
outrunning an unbounded producer.

`MirrorHeadPacing.tla` states the successor rule for a mirror group's heads,
and is written before the code that will implement it. `VisualRetirement.tla`
records today's rule -- a logical output retires when every one of its heads has
flipped -- which paces a group at its slowest screen. That is wrong on ordinary
hardware: 144Hz beside 60Hz is a normal desk, and holding the fast panel back
throttles the client's frame callbacks with it. Here each head instead takes the
newest generation it has not shown, skipping what it missed, and the primary
head alone owns present feedback. Screens may disagree for a while; the bounded
configuration explores 745 generated states and 302 distinct states to depth 12.

Independent pacing is what makes the release rule load-bearing.  Under joint
retirement, "no screen is reading this generation" followed from the group
having moved on together; it no longer does, so `NoScannedGenerationIsReleased`
is stated over every head separately and the newest generation is never freed --
a head that has not caught up will take it next.  `PrimarySubmitNeverBlocked` is
provable from the submit guard as written, in the same way and for the same
reason as `EmittedEffectsAreExecutable`: it exists so that a later conjunct
about another head's progress cannot be added quietly.

Five temporary negative controls independently restore joint advance, judge
release by the primary head alone, let a head take work it has already shown,
make feedback wait for every screen, and drop the secondary head from fairness.
TLC then violates `PrimarySubmitNeverBlocked`, `NoScannedGenerationIsReleased`,
`HeadsNeverShowOlderWork`, `FeedbackIsItsOwnScreen`, and `AllHeadsConverge`
respectively. The first is today's implementation: restoring the rule the code
currently follows is what the fast head being blocked looks like in the model.

`InputAuthorityArbitration.tla` models the coexistence prerequisite between
application routing and future shell targets. It separates committed,
submitted, and presented choices; creates an exact provisional frontend lease;
requires matching confirmation or rejection; preserves it only inside its
profile/output/session/control epochs; and waits for exact normal-release
acknowledgement. Security transition instead revokes immediately and fences
stale queued input. The bounded configuration explores 352,311 generated
states and 99,680 distinct states to depth 20.

The first connection run found that `(client, connection epoch)` was not a
unique transfer identity when one connection reused it for later work. The
model now includes the transaction in that identity and rejects transaction
reuse within an epoch. The first projection run found that a client could name
a future scene generation and become accidentally current after a later scene
change. A proposal must now answer an outstanding, server-issued request for
the current generation. These are protocol requirements, not model-only
restrictions.

The policy models intentionally omit wire offsets, KDL schema machinery, tags,
Hagia `ViewId` values, rendering, and physical output retirement. Golden-vector
tests own byte layouts. Hagia owns tags and views. The existing visual models
own preparation, submission, and page-flip retirement after a logical
projection is accepted.

Temporary negative controls removed candidate readiness, reservation-basis
equality, and WM-epoch equality separately. TLC then violated
`PresentedBundlesWereReady`, `PresentedBundlesAreCoherent`, and
`PresentedBundlesMatchExactGenerationAndEpoch`, respectively. The clean model
restores all three checks; no unsafe-mode switch remains in the retained spec.

## Rust Boundary Map

The model is deliberately smaller than the implementation. Its actions map to
the current owning Rust boundaries as follows:

| Model action | Current Rust boundary |
| --- | --- |
| `Propose`, `Prepare` | `HeadlessEngine::prepare_surface_transactions` and `ProductionSessionCoordinator::prepare_present_transaction` |
| `Submit` | `OutputPresentationRegistry::schedule` followed by the production presentation adapter's `submit_frame` effect |
| `Retire` | `OutputPresentationRegistry::retire` and `ProductionSessionCoordinator::settle_prepared_retirement` |
| `Settle(..., "rejected")` | `TransactionOutcome::{RejectedStaleSurface, RejectedInvalidSurface}` reducers |
| `Settle(..., "timed_out")` | authority and WM timeout reducers returning `TransactionOutcome::TimedOut` |
| `Settle(..., "disconnected")` | `AuthorityTransactionInbox::drain_ready` reports disconnect; universal visual settlement remains a gap |
| `Settle(..., "removed")` | ordered `AuthorityTransactionIntake::removed_surfaces` handling, currently on the direct-commit path |
| `LoseHead(...)/"head_lost"` | physical output loss while a head owns a native presentation lease |
| `Release` | output retirement plus backend-specific lease teardown; a universal release reducer remains deferred |
| `QueueNext` | `SurfaceContentStream::admit` carrying a complete `LiveProductionAuthorityGroup` |
| `RetirePresent` | `finish_surface_content_owner` after exact DMA or software frame retirement |
| `ApplyReady` | the next production cycle's sequential rebase, CPU update, and authority commit |
| `BeginMove`, `BeginResize` | `LiveWmLayoutManager::stage` deriving full geometry controls separately from pixel resize obligations |
| `DeliverControl` | `XAuthorityRuntime::configure_window_from_engine` followed by Present ConfigureNotify and core ConfigureNotify for a real change |
| `CommitMove`, `CommitResize` | logical layout commit, with only the resize path gated by exact candidate retirement |
| `Timeout` | layout recovery queuing the complete last-committed rectangle behind any late target control |
| `PrimeAdmission` | `PersistentLiveLayout::prime_admission_extent` selecting complete safe pixels before the first blind-WM target |
| `IssueConfigure`, `IssueFocus` | delayed synthetic requests emitted by the legacy WM process |
| `ReplaceProjection` | `X11WmBridgeState::replace_active_workspace_projection` replacing cached membership and the complete mapped-window projection |
| `AssignWorkspace` | `X11WmBridgeState::assign_workspace` updating cached membership before mapping reconciliation |
| `SwitchWorkspace` | `X11WmBridgeState::activate_workspace_into` selecting the exact cached workspace membership |
| `TranslateConfigure`, `TranslateFocus` | `X11WmBridgeState::translate_legacy_requests_for_output` filtering requests through the current mapped-window set |
| `BeginRequest`, `ValidateGrab`, `ObserveQuietBoundary`, `CompleteRequest` | registered private xmonad action validation, `LegacyX11WmBridgeRuntime::handle_request_once`, and `collect_legacy_responses` |
| `BeginFrontendGrab`, `ConfirmFrontendGrab`, `RejectFrontendGrab` | provisional `ApplicationRouteLeaseState`, `XAuthorityRoutedInput::route_lease`, and sanitized `XAuthorityRouteLeaseUpdate` feedback |
| `RequestLeaseRelease`, `AcknowledgeLeaseRelease` | exact Engine release state plus `XAuthorityRouteLeaseRelease` and frontend grab teardown acknowledgement |
| `SecurityTransition`, stale queued route rejection | shared input control epoch, `advance_security_epoch`, and epoch-stamped bounded frontend ingress |
| `ReachHardDeadline` | response collection returning an error before any normal response is encoded |
| `Restart` | live-session supervision replacing the failed bridge process and reseeding committed state |
| `BeginLayout`, `FirstSilentTimeout` | admission staging and `LiveWmLayoutManager::expire_pending` retaining a pixel-silent retry |
| `RestartAndReseed`, `WithdrawSilentAdmission` | live-session WM recovery and bounded admission retry accounting |
| `ObservePixels`, `CommitPixels` | Engine safe-pixel observation and ordinary exact-candidate layout settlement |

The public policy model is intentionally ahead of production implementation.
Its actions map to the following target boundaries:

| Model action | Target Rust boundary |
| --- | --- |
| `Connect`, `Negotiate`, `Disconnect` | session-owned role endpoint and policy transport worker |
| `BeginProposal`, `AppendProposalChunk`, `FinishProposal` | generated frame codec plus bounded transfer assembler |
| `SettleQueued` | connection-epoch intake before Engine proposal validation |
| `BeginProposal` in the projection model | immutable canonical projection candidate construction |
| `CommitProposal`, `RejectProposal`, `TimeoutProposal` | Engine-owned projection reducer and explicit transaction outcome |
| `SceneChange` | Engine surface/output lifecycle reduction and fresh complete snapshot generation |
| `EnqueueAction`, `IssueAction` | Engine shortcut router and bounded session-owned policy request queue |
| `RequestDirty`, `IssueDirty` | policy transport requesting a fresh complete Engine cycle without geometry |
| `UpdateInteraction`, `IssueInteraction` | Engine-owned grab coalescing reduced continuous geometry |
| `InstallConfig` | policy configuration generation activated at a shortcut-idle boundary |
| `FrontendSettlesChanged`, `CommitProposal` | frontend settlement retry and final canonical Engine commit |
| `IssueCrossOutputRequest`, `StageCompleteProposal` | complete affected-output request and cloned canonical reducer successor |
| `ObserveRightOutputLoss`, `ObserveRightOutputReturn` | Engine output lifecycle observation, scene advancement, and non-recyclable output generation |
| `PrepareCurrentCandidate`, `SettlePreparedCandidate` | non-mutating staged revalidation followed by one reducer/layout settlement |
| `RejectSupersededCandidate` | stale topology terminal outcome preserving the prior complete projection |
| `AdmitDirty`, `IssueRefresh`, `StageProposal`, `SettleProposal` in `PolicyRefreshLifecycle.tla` | `PolicyDirty` admission/coalescing, complete refresh issuance, and atomic staged reducer/layout settlement in the public owner |

This map fixes ownership before the Rust names exist. Update the right column
when implementation establishes the final module names; do not move an action
to another authority merely to match convenient code placement.

The frame-ownership model maps `QueuePresent` to
`queue_software_present_frame` and `stage_software_present_frame`,
`SubmitPresent` to `mark_software_present_frame_submitted`, and
`ObservePresentRetirement` plus `SettlePresent` to
`settle_software_present_frame`. The typed
`LiveProductionNativeFrameId` is the identity shared by those transitions.
`SubmitUnrelated` and `RetireUnrelated` represent ordinary CPU or DMA frames
whose callbacks must leave the software binding unchanged. `SubmitSuccessor`
represents callback-and-submit coalescing within one backend tick.

The copy-ownership model maps `Capture` to
`NativeGbmRenderedScanoutContext::capture_renderer_image`, `PageFlip` to the
exact mixed-frame retirement that promotes the image and retires the client
presentation, `CompleteCopy` to X Present Copy feedback, and `Rollback` to
renderer-image teardown on export, submission, or authority failure.

This table identifies correspondence, not equivalence. The current direct
authority commit and committed-snapshot replacement APIs remain implementation
gaps recorded in `docs/research-log.md`; the model must not be weakened to make
those shortcuts appear retirement-safe.

When TLC finds a counterexample that changes implementation behavior, preserve
the trace as a deterministic Rust regression before correcting the model or
code. Do not replay external effects from a TLC trace.

## Reproducible Command

Sophia pins the command-line TLA+ tools release rather than the unmaintained
graphical Toolbox. Version 1.7.4 includes the stable liveness-checking fix
called out by its release notes. Obtain its official `tla2tools.jar`, then run:

```sh
SOPHIA_TLA2TOOLS_JAR=/absolute/path/to/tla2tools.jar tools/check_tla.sh
```

The checker requires Java 11 or newer and verifies the pinned SHA-256 before
running TLC with one worker in a temporary directory. It performs no network
access and leaves no model-checker state in the repository. The pinned artifact
is:

- release: <https://github.com/tlaplus/tlaplus/releases/tag/v1.7.4>
- jar: <https://github.com/tlaplus/tlaplus/releases/download/v1.7.4/tla2tools.jar>
- SHA-256: `936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88`

TLC's deadlock error is disabled because a fully settled session with no newer
generation is valid quiescence. Safety invariants and the explicit liveness
property remain enabled.

The optional commit-pinned Specula development audit is documented under
`validation/specula`. It generates review evidence outside this repository and
adds no runtime or build dependency.

Complementary bounded relational and arithmetic models live under
`validation/architecture`. Alloy owns static role/namespace/portal and
presented-target topology; Z3 owns region and wire-bound arithmetic. They do
not replace or translate the TLA+ models: presentation epochs, route-lease
revocation, capture cancellation, pacing, and fairness remain temporal here.
The cross-model relationship is a documented correspondence, not a refinement
chain.

`AdmissionRecovery.tla` covers exact PresentedBuffer selection, a later
backing observation, proactive safe-extent admission, timeout only when no
complete pixels have been observed, one outstanding Manage request, committed
public planning-ownership consumption, fallback admission,
temporary-constraint release, one relayout, and exact standing-target
retirement. It explores target observation both before and after fallback
retirement. Its actions map to `LayoutEpochCoordinator`, the bounded
visual-candidate tracker, public Manage ownership, pre-admission group
ownership, the production Present scheduler, and native retirement. The model
nondeterministically gives the selected fallback Present DMA or CPU storage and
requires the same retirement lifecycle for either choice. Content identity
remains distinct from geometry and storage: an extent or materialization can
choose a rendering path without choosing which candidate owns admission. The
bounded configuration explores 172 generated states and 88 distinct states to
depth 13.
