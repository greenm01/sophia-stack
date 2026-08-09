# Visual Transition Model

**Role:** bounded validation model, not production architecture or a refinement
proof of the Rust implementation.

`VisualRetirement.tla` explores the asynchronous lifetime of immutable visual
candidates across two outputs and two generations. That is the smallest scope
that still explores out-of-order retirement and supersession. It excludes
X11 objects, application metadata, pixel content, renderer handles, and native
KMS objects.

The model checks that:

- a successful commit follows retirement of every output required by that
  candidate;
- a late or superseded generation cannot replace newer committed state;
- committed input generation advances with committed visual generation;
- successful feedback exists only for a committed generation;
- active, submitted, or committed resources cannot be released; and
- admitted work eventually reaches one terminal settlement under the weak
  fairness assumption documented in the module.

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
during a complete two-output policy settlement. It requires the canonical
scene generation to fence the staged candidate, both Engine layout and reducer
projections to promote as one last-good state, and a returned output to carry a
new generation. The bounded configuration explores 86 generated states and 64
distinct states to depth 13. Temporary negative controls independently allowed
a prepared candidate to skip its final topology recheck and allowed an output
to return without advancing generation. TLC violated
`CommittedTopologyWasCurrent` and `ReappearedOutputIsFresh`, respectively.

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
complete pixels have been observed, fallback admission, temporary-constraint
release, one relayout, and exact standing-target retirement. It explores target
observation both before and after fallback retirement. Its actions map to
`LayoutEpochCoordinator`, the bounded visual-candidate tracker, pre-admission
group ownership, the production Present scheduler, and native retirement. The
model nondeterministically gives the selected fallback Present DMA or CPU
storage and requires the same retirement lifecycle for either choice. Content
identity remains distinct from geometry and storage: an extent or
materialization can choose a rendering path without choosing which candidate
owns admission. The bounded configuration explores 160 generated states and
84 distinct states to depth 12.
