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

`AdmissionRecovery.tla` covers exact PresentedBuffer selection, a later
backing observation, layout timeout, fallback admission, temporary-constraint
release, one relayout, and exact standing-target retirement. It explores target
observation both before and after fallback retirement. Its actions map to
`LayoutEpochCoordinator`, the bounded visual-candidate tracker, pre-admission
group ownership, the production Present scheduler, and native retirement. The
model nondeterministically gives the selected fallback Present DMA or CPU
storage and requires the same retirement lifecycle for either choice. Content
identity remains distinct from geometry and storage: an extent or
materialization can choose a rendering path without choosing which candidate
owns admission.
