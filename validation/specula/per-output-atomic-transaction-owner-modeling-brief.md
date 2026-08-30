# Per-Output Atomic Transaction Owner Modeling Brief

## 1. System Overview

Sophia drives its primary planes through atomic KMS. One request may carry
several heads of a card session, which is what mirroring is
(`drm/native_primary_plane/multi_head_request.rs`), and the property handles
that request needs -- `fb_id`, `crtc_id`, `src_*`, `crtc_*` -- are discovered
once per head (`drm/native_atomic/properties/handles.rs`). Plane selection
already reads plane types and keeps the one that reports
`PlaneType::Primary` (`drm/native_kms/selection.rs:196`, and again at `:359`).

The cursor does not go through any of that. It rides `drmModeSetCursor` and
`drmModeMoveCursor` on its own, per CRTC, through a controller that knows
nothing about frames (`hardware_validation/atomic_scanout_card/cursor.rs`,
`production_session/native_scanout/persistent_native_scanout/cursor.rs`).
Milestone 14 asks for one per-output transaction owner that can carry
primary and cursor-plane state in the same atomic request, and for the
legacy baseline to be retired only once that owner exists
(`todo.md`, Milestone 14).

The constraint that makes this a modeling problem rather than a refactor is
the kernel's: atomic commits serialize per CRTC, and a second commit while
one is pending is refused. The code already lives with that for primary
flips -- `Status::AlreadyInFlight` becomes `submit_deferred`
(`production_session/native_scanout.rs:1286`). The cursor escapes it today
by not being atomic at all.

That escape is visible in one function. `legacy_hardware_cursor_admission`
(`hardware_validation/atomic_scanout_card/cursor.rs:169`) is a pure
two-input truth table, and the only place in the tree that consults whether
a primary flip is in flight:

| initialized | primary in flight | today |
| --- | --- | --- |
| no | yes | defer initialization |
| no | no | initialize, then update |
| yes | either | **update** |

The third row is exactly what a legacy ioctl can do and an atomic commit
cannot. Direct-scanout archive `0004` counted fifteen of them:
`updates_primary_in_flight=15`, fifteen completed cursor ioctl sequences
issued while a page flip was outstanding. Deciding what replaces that row is
the substance of this model.

Archive `0004` also establishes what the replacement has to preserve. Twelve
proof-driven cursor positions plus live pointer motion produced 519 hardware
updates with no failures, the cursor never leaving `legacy_ioctl`, twenty-six
client buffers reaching the plane after the motion stopped, and
motion-to-submit peaking at 9 ms. The legacy cursor genuinely does ride over
directly scanned frames; this row retires something that works.

## 2. Scenarios

### Scenario 1: A Cursor Move Arrives While A Primary Flip Is In Flight

**Mechanism**: the pointer moves; the CRTC has an outstanding commit. The
legacy ioctl proceeds. An atomic commit cannot, and the move must wait for
the CRTC without being lost and without accumulating one commit per pointer
event.

**Evidence**:

- Code analysis: `head.submitted_at` is set on submit accept
  (`native_scanout.rs:1219`, `:2067`) and cleared at page-flip retirement
  (`:1498`, `:2628`); `cursor.rs:46` samples `any()` over it.
- Physical evidence: archive `0004` recorded `updates_primary_in_flight=15`,
  so the case is not hypothetical on this hardware.
- Code analysis: the pending cursor update *accelerates* the owner loop
  rather than throttling it -- `authority_wait_timeout` drops the authority
  receive timeout from 25 ms to 1 ms while a cursor update is pending
  (`live_session/owner_loop.rs:43-56`, passed at
  `owner_loop/authority.rs:65`). Nothing in the runtime rate-limits cursor
  updates.

**Affected code paths**: `legacy_hardware_cursor_admission`, the submit
deferral ladder, the new transaction owner.

**Suggested modeling approach**: a per-CRTC single-outstanding-commit
resource, a pending cursor position that supersedes in place rather than
queueing, and a rule that the pending position is committed when the CRTC
frees. Supersession is the same shape `SharedWorkerService` uses for its
latest-wins pending cell, and for the same reason: a backlog that grows per
input event is unbounded by construction.

### Scenario 2: A Cursor-Only Commit While The Primary Is Idle

**Mechanism**: the pointer moves and no flip is outstanding. This is the
common case -- a client repainting on a cursor blink leaves the CRTC free
almost always -- and it is what preserves the 9 ms baseline. A cursor-only
atomic request must not disturb the primary plane.

**Evidence**:

- Atomic requests are sparse: properties absent from a request keep their
  committed values, so a request naming only cursor properties leaves the
  primary framebuffer bound. This is what allows a directly scanned client
  buffer to stay on the plane across a cursor move.
- Code analysis: `add_primary_plane_properties`
  (`native_primary_plane/request.rs:163`) shows the property set a plane
  contributes; a cursor contribution is the same set against the cursor
  plane object.

**Affected code paths**: request construction, the owner's decision about
what a commit contains.

**Suggested modeling approach**: two commit kinds -- one carrying the
primary (with or without a cursor) and one carrying only the cursor -- both
drawing on the same per-CRTC resource. The invariant worth stating is that a
cursor-only commit never changes what the primary plane is scanning.

### Scenario 3: A Cursor Commit Over A Directly Scanned Frame

**Mechanism**: the primary plane holds the client's own buffer. A cursor
commit lands on the same CRTC. It must not end the eligibility episode, must
not retire the client's buffer, and must not make the next frame ineligible.

**Evidence**:

- Doc and model: `PresentFlipOwnership.tla` ends an eligibility episode on
  *activation*, and releases a displayed buffer only through a successor's
  retirement. A cursor commit is neither.
- Code analysis: the cursor module references nothing about direct scanout,
  eligibility, or composition. It reads only head selection, mapping, output
  size, group, and `submitted_at`.
- Physical evidence: archive `0004` has twenty-six flips after the cursor
  motion finished, so on the legacy path the independence holds.

**Affected code paths**: the eligibility episode, displayed-buffer
retirement.

**Suggested modeling approach**: model the cursor commit as an action that
takes the CRTC resource without touching the primary's displayed buffer or
the episode. The negative control is the version that retires on any commit,
which should violate `ReleasedOnlyBySuccessor`'s analogue here.

**A correction this brief must carry.** `DirectScanoutVerdict::ComposedCursor`
exists (`sophia-engine/src/composition_plan.rs:229`) and is returned when
`plan.cursor.is_some()` (`:385`), but nothing outside tests ever populates
`plan.cursor`. `composed_cursor=0` in every archive is therefore not
evidence that the hardware cursor avoids composition -- it is evidence that
the software-cursor path does not exist. Any invariant phrased as "the
hardware cursor keeps `composed_cursor` at zero" would be vacuous. The
verdict is correct and should stay; what it is not is a check that this row
can lean on.

### Scenario 4: A Refused Combined Commit

**Mechanism**: the driver rejects a request carrying both planes. Because
they share one request, a cursor-side problem takes the primary flip with
it -- a class of failure that cannot exist today.

**Evidence**:

- Code analysis: commit refusal is classified only as `Rejected` with no
  errno inspection (`drm/native_scanout/prepare.rs`), so a cursor-specific
  refusal is indistinguishable from any other.
- Prior art: direct scanout meets the same problem and answers it with a
  fallback ladder -- a refused test or rejected commit re-queues the frame
  composed rather than failing the session.

**Affected code paths**: the fallback ladder, the owner's retry.

**Suggested modeling approach**: rejection reachable as an environment fact,
with a retry that drops the cursor contribution and commits the primary
alone. The invariant is that a frame is never lost to a cursor: every
rejected combined commit is followed by a primary commit carrying the same
frame.

### Scenario 5: Mirror Groups

**Mechanism**: one request already spans every head of a card session. With
cursors, it carries each head's cursor contribution too, and the cursor may
be outside some heads entirely.

**Evidence**:

- Code analysis: the legacy path already projects per head through
  `project_mirror_coordinates` and groups targets by card session
  (`persistent_native_scanout/cursor.rs:100-115`).
- Code analysis: a head whose projection yields nothing is skipped, and a
  group with no targets hides the cursor on every CRTC it had it on
  (`atomic_scanout_card/cursor.rs:113-118`) -- a hide is an ioctl, not a
  no-op.

**Affected code paths**: `LibdrmNativeAtomicHead`, group request assembly.

**Suggested modeling approach**: heads within one group sharing one request
and one outstanding-commit resource. The property to check is that a cursor
leaving one head's viewport does not force that head's primary to commit,
and does not prevent the group's request from going out.

### Scenario 6: Bounded Cursor-Only Work

**Mechanism**: pointer motion arrives far faster than frames. The row
requires bounded cursor-only idle work, which under atomic means bounded
commits rather than bounded ioctls.

**Evidence**:

- Code analysis: `moves_coalesced`
  (`owner_loop/physical_input_phase.rs:433`) counts only cross-iteration
  backlog -- it increments when a motion batch arrives while a previous
  update is still pending, which the same-iteration flush makes rare. Six of
  the seven places that mark a cursor update dirty never touch it. Archive
  `0004` reports 0 beside 519 updates, and both numbers are correct.
- Physical evidence: those 519 updates are one per motion batch, and the
  1 ms authority wait keeps batches near one motion each.

**Affected code paths**: the owner's coalescing, cursor metrics.

**Suggested modeling approach**: bound the number of commits by the number
of times the CRTC becomes free, not by the number of pointer events. The
negative control is the model that commits per event, which should exceed
the bound immediately.

### Scenario 7: Cursor Framebuffer Lifetime

**Mechanism**: a cursor framebuffer is scanned out until a successor commit
replaces it, exactly as a primary framebuffer is.

**Evidence**:

- Prior art: `PresentFlipOwnership` proved this shape for the primary plane;
  the cursor plane is a second instance of it, with the difference that the
  compositor owns the buffer rather than a client.

**Suggested modeling approach**: out of scope for the first model, and worth
saying why. The buffer is compositor-owned and its contents do not change
per move -- only its position does -- so the lifetime question reduces to
"do not free the cursor FB while a commit referencing it is outstanding",
which is the existing resource-bundle discipline
(`drm/native_scanout/retire.rs`) rather than a new temporal property.

## 3. Modeling Recommendations

### 3.1 Model

- One outstanding atomic commit per CRTC, taken by either commit kind.
- A latest-wins pending cursor position per output, superseding in place.
- Commit kinds: primary (optionally carrying a cursor) and cursor-only.
- Rejection of a combined commit, and the primary-only retry that follows.
- Mirror groups: several heads sharing one request and one resource.
- A bound on commits stated against CRTC availability, not pointer events.

### 3.2 Do Not Model

- Cursor image content, hotspot semantics, formats, and sizes. A startup
  capability probe answers those once, the way the atomic test answers
  format questions for direct scanout.
- Cursor framebuffer allocation and free, per Scenario 7.
- Slot incarnations and renderer lifetime: `VisualRetirementSlots` owns
  them, and a cursor plane acquires no render slot.
- Mirror cohort pacing: `MirrorHeadPacing` owns when heads of a group
  advance; this model owns what one request contains.
- Any duration, deadline, or refresh-relative bound. The physical gates own
  that half, and the harness holds that no model expresses timing.

## 4. Proposed Extensions

| Extension | Variables | Purpose | Scenario |
| --- | --- | --- | --- |
| Per-CRTC commit resource | `outstanding`, `kind` | One commit at a time, either kind | 1, 2 |
| Pending cursor cell | `pendingCursor`, `committedCursor` | Latest-wins motion that cannot backlog | 1, 6 |
| Commit rejection | `rejected`, `retriedPrimaryOnly` | A cursor never costs a frame | 4 |
| Group membership | `heads`, `groupOf` | One request across a mirror group | 5 |
| Commit accounting | `commits`, `crtcFreed` | Work bounded by availability | 6 |

## 5. Proposed Invariants

| Invariant | Type | Description | Targets |
| --- | --- | --- | --- |
| `OneOutstandingCommitPerCrtc` | Safety | No second commit while one is pending | 1, 2 |
| `CursorOnlyCommitPreservesPrimary` | Safety | A cursor-only commit never changes the scanned framebuffer | 2, 3 |
| `DirectFrameSurvivesCursorCommit` | Safety | A cursor commit neither retires the displayed client buffer nor ends the episode | 3 |
| `NoFrameLostToCursor` | Safety | Every rejected combined commit is followed by a primary commit carrying the same frame | 4 |
| `CursorWorkBoundedByAvailability` | Safety | Commits are bounded by CRTC availability, not pointer events | 6 |
| `PendingCursorEventuallyCommits` | Liveness | A moved cursor eventually reaches a plane | 1, 5 |

## 6. Findings Pending Verification

### 6.1 Model-Checkable

- Whether the cursor-only commit kind is load-bearing, or whether riding the
  next primary commit suffices. It should not suffice: with a client
  repainting twice a second, the next primary commit is most of a second
  away, and `PendingCursorEventuallyCommits` under a fair environment should
  fail without it.
- Whether latest-wins supersession is required for the work bound, or
  implied by the single-outstanding-commit rule alone.
- Whether the primary-only retry can lose a cursor position -- the retry
  drops the cursor contribution, and the pending cell must survive it.

### 6.2 Testable Only

- That a cursor-only atomic request leaves the primary plane's framebuffer
  bound, which is a kernel property rather than a model one.
- That the new admission answer round-trips through
  `legacy_hardware_cursor_admission`'s truth table, whose regressions live
  at `tests/libdrm_events_feature/legacy_cursor.rs:215`.
- That a cursor plane exists and accepts the compositor's format and size on
  this card, via a startup capability probe.
- That motion-to-submit on the atomic path is comparable to archive
  `0004`'s 9 ms baseline.

### 6.3 Out Of Scope

- Replacing the legacy path, which is the row after this one.
- The GLX cadence gate's rework. `tools/report_sophia_glxgears_performance.sh`
  requires `path=legacy_ioctl` and a strictly positive
  `updates_primary_in_flight`; both are legacy-specific by construction and
  cannot hold on an atomic path, where updates during an in-flight flip are
  precisely what becomes impossible. The cadence half of that gate -- 55 FPS,
  p95 within 25 ms, bounded update cost, no failures -- is what the row means
  by retaining it, and reworking the two shape assertions belongs with the
  implementation rather than with this model.
- Cursor plane composition with overlay planes, which no row asks for.
