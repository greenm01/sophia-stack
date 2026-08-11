# `sophia_wm_v1` Freeze Surface

**Role:** pre-freeze decision record for wire-layout risk.

Freezing `sophia_wm_v1` makes its message and record layouts permanent. This
file enumerates, for every retained row in Hagia's port ledger, whether closing
that row can require a wire change — and if so, which kind. Its purpose is to
convert an unbounded "we might need to change the wire later" risk into a bounded
list of decisions that can be made while changes are still cheap.

The gate this serves is `todo.md`'s freeze item, whose first condition is that
the retained Triad behavior port completes. The canonical ledger is
`docs/triad-port-ledger.md` in the Hagia repository, at Triad baseline
`fb8fb27ec294e0fe2361375de0b2fa8c08be0ca9`. See
`docs/triad-port-ledger-pointer.md`.

---

## What Costs What

Four expansion moves, cheapest first. Only the last is impossible after the
freeze, but the middle two are impossible *without* a new interface family, which
amounts to the same thing.

### 1. A new message kind — cheap, survives the freeze

Message kinds 32 through 52 are assigned; 53 and above are free. A new message
does not touch any existing layout, and a client that never negotiates the
capability never receives it. **This is the correct home for genuinely new
behavior.** The watched-reload visibility protocol is the known candidate.

### 2. Reserved-field consumption — cheap, but asymmetric and finite

Fourteen reserved fields exist and are validated as zero on decode, so they are
genuinely held for future use rather than ignored padding. Note that the
generated Rust structs **omit** reserved fields while the wire still carries and
checks them — `WmV1ProjectionOutputRecord` has four fields but
`PROJECTION_OUTPUT_RECORD_SIZE` is 24 bytes, and
`crates/sophia-protocol/src/ipc/wm_v1.rs:419` pushes the zero that
`:448` reads back and rejects if non-zero. Do not conclude from a struct
definition that there is no reserved space.

Reserved space is distributed unevenly:

| Record | Kind | Size | Reserved |
| --- | --- | --- | --- |
| `SnapshotOutput` | snapshot 1 | 56 | **none** |
| `SnapshotSurface` | snapshot 2 | 80 | **none** |
| `SnapshotAction` | snapshot 3 | 140 | **none** |
| `SnapshotSessionOperation` | snapshot 4 | 12 | **none** |
| `ProjectionOutput` | projection 1 | 24 | `u32` |
| `ProjectionPlacement` | projection 2 | 60 | **none** |
| `ProjectionIndicator` | projection 3 | 64 | **none** |
| `ProjectionOutputStatus` | projection 4 | 48 | `u32` |

Twelve of the twenty-one messages carry a reserved `u16`. `ProjectionRequest`
carries two — `reserved_cause` and `reserved` — which matters for the pointer row
below. `capabilities` is a `u64` with bits 0 through 9 assigned, leaving 54.

**Six of the eight record kinds have no reserved space at all.** Any new
per-output, per-surface, per-placement, or per-indicator fact is therefore either
a bitfield/enum extension or a layout change.

### 3. An existing enum or bitfield gains a value — pre-freeze only

Unknown discriminants are rejected, not ignored, so a pre-freeze client cannot
tolerate a post-freeze value. The extensible fields are `SnapshotSurface.kind`,
`SnapshotSurface.capability_bits`, `request_state_bits`, `current_state_bits`,
`ProjectionPlacement.transform`, `presentation_bits`,
`ProjectionIndicator.state_bits`, `ProjectionOutputStatus.focus_bits`,
`SnapshotSessionOperation.target_bits`, and `ProjectionRequest.cause_kind` /
`interaction_phase` / `interaction_kind`.

Three of these are near-empty today and are the standing one-way doors:

- `PolicyInteractionKind` has only `Move = 1` and `Resize = 2`
  (`crates/sophia-protocol/src/packets/policy.rs:135-141`). Drag and scrolling
  need values here.
- `PolicyTransform` has only `Identity = 1` (`:185-188`).
- `PolicyInteractionPhase` has all four of `Begin`/`Update`/`End`/`Cancel`
  (`:125-132`), but only `End` is ever constructed, so the other three are
  wire-reachable and implementation-absent. No wire change needed to use them.

### 4. A new record kind — pre-freeze only, and the expensive one

Unknown record kinds are rejected outright:
`crates/sophia-protocol/src/ipc/wm_v1_records.rs:762` for snapshots and `:997`
for projections both end in
`other => return Err(invalid("…_record_kind", u32::from(other)))`.

A new record kind also needs a declared count, and both `*Begin` messages are
fixed-layout with a strict `cursor.finish()?`
(`crates/sophia-protocol/src/ipc/wm_v1.rs:886-896`). `WmV1SnapshotBegin` carries
`chunk_count, output_count, surface_count, action_count,
session_operation_count`; `WmV1ProjectionBegin` carries `chunk_count,
output_count, placement_count, indicator_count, status_count`. Adding a sixth
count changes an existing message layout, which per
`docs/sophia-indicator-descriptor.md` requires a new interface family.

The indicator descriptor is the worked precedent: it landed pre-freeze precisely
so `indicator_count` and `status_count` could be added to `ProjectionBegin`.
That door closes at the freeze.

---

## Enumeration

Twenty-seven retained rows across four authority tables. Wire-impact classes:

- **None** — closes inside an authority with no `sophia_wm_v1` change.
- **Enum** — needs a value in an existing enum or bitfield. Pre-freeze only.
- **Record** — needs a new record kind, hence a `*Begin` layout change.
  Pre-freeze only, and the expensive class.
- **Message** — needs a new message kind. Survives the freeze.
- **Separate** — belongs to `sophia_shell_v1`, a broker, or a portal family, not
  to `sophia_wm_v1`.

### Spatial Policy — Hagia

| # | Row | State | Wire impact |
| --- | --- | --- | --- |
| 1 | Stable logical windows, outputs, tags, views, columns, reconciliation | Complete | None |
| 2 | Scrolling columns and fixed-point geometry | Partial | None — proportions, centering, gaps, and constraints all reduce to `ProjectionPlacement` geometry already on the wire |
| 3 | Tags, workspaces, names, dynamic creation/pruning, occupancy navigation, output affinity | Partial | **Decision 1** — everything but configured *names* is None. Names need a home |
| 4 | Focus, movement, exchange, grouping, histories, cross-output behavior | Partial | None — opaque `SnapshotAction` tokens, 256 available |
| 5 | Floating, fullscreen, maximize, minimize, restore, client-visible state | Partial | None for the four proven states; **Enum** if a fifth presentation state appears in `presentation_bits` |
| 6 | Dialogs, transients, popups, scratchpads | Complete | None — `transient_index`/`transient_generation` already carry reduced parent facts |
| 7 | Additional native layouts, frames, tabs, BSP/split trees, grid, layout switching | Partial | **Separate** — the row itself says shell chrome stays outside WM policy. Layout switching is an opaque action: None |
| 8 | Declarative policy configuration | Complete | None — `PolicyConfiguration` and the profile messages exist |
| 9 | Janet commands and layouts | Open | None — sandboxing, determinism, and memory bounds are Hagia-internal; the output is an ordinary projection |
| 10 | Placement, sticky behavior, swallowing, size policy, window rules | Open | **Decision 2** — the highest wire risk in the ledger |
| 11 | Completed and continuous pointer policy interactions | Partial | **Enum**, and see Decision 3 |
| 12 | Checkpoint, crash, reconnect, last-layout preservation | Partial | None — corpus, host, and driver work; Engine already supports the epoch advance |

### Visible Desktop — Hagia Shell

All five rows are Open, and all five are **Separate** by construction: they need a
least-authority shell endpoint and target-resolved input, not WM messages. The
shell reservation to work-area to WM projection chain rides `SnapshotOutput`'s
existing `work_x/work_y/work_width/work_height`, so coordinating it costs no
`sophia_wm_v1` change.

One caveat that is *not* a wire risk but is an implementation-coupling risk: the
shell texture-transport question (64 KiB frame cap against a bar frame several
times that size) would touch the shared transport in `sophia-runtime`. The
envelope is role-neutral and each role negotiates its own family, so a shell-role
descriptor extension stays additive. Resolve it during the experimental port
anyway, because it is the only shell question with a back-edge into shared code.

### Session And Dedicated Sophia Authorities

| # | Row | State | Wire impact |
| --- | --- | --- | --- |
| 1 | Physical key, pointer, axis, gesture, switch, shortcut matching | Partial | **Enum** if axis, gesture, and switch bindings need a binding-kind discriminant Hagia can see; None if they resolve entirely in Engine and surface as ordinary opaque actions |
| 2 | Input-device and XKB configuration | Partial | None — session-owned, never crosses policy IPC |
| 3 | Output mode, scale, position, transform, VRR, enablement, power, reservations | Partial | None, **conditional on Decision 4** |
| 4 | Launch, startup environment, configured processes, shell supervision | Partial | None — opaque session-operation tokens |
| 5 | Lock, logout, session exit, idle inhibition, shortcut inhibition | Open | None — `connection_epoch` is already on every message, which is the barrier these transitions advance |
| 6 | Configuration discovery, validation, activation, reload, rollback | Open | **Message** — the separate visibility and recovery protocol required before watched reload should take new message kinds at 53 and above, not new fields |

### Brokers And Portals

| # | Row | State | Wire impact |
| --- | --- | --- | --- |
| 1 | Application classification and launch placement | Open | **Decision 2** — same boundary as Spatial row 10; these close together |
| 2 | Window lists and shell-facing descriptors | Open | **Separate** — shell endpoint; must stay out of WM policy |
| 3 | Screenshots and capture sessions | Open | **Separate** — portal |
| 4 | Clipboard, drag-and-drop, files, notifications | Open | **Separate** — portal |

### Result

Twenty-three of twenty-seven rows need no `sophia_wm_v1` change. The residue is
four decisions, below. That is the useful output of this pass: the wire risk is
bounded and small, but it is not zero, and three of the four must be settled
before the freeze rather than discovered after it.

---

## The Four Decisions

### Decision 1 — Where configured workspace and view names live

Hagia implements dynamic workspace creation and pruning, occupancy navigation,
and scratchpads. `setWorkspaceName` exists in its policy state with length
validation but no bound action, and the ledger row still lists configured names
as open.

`ProjectionIndicator` already carries a 32-byte UTF-8 label plus `state_bits` and
an action token, and `ProjectionOutputStatus` carries a 32-byte layout name. A
workspace name is the same shape of fact, and routing it through the indicator
descriptor is blind-safe by construction: policy authors the label, so no broker
sanitization is required.

**Recommendation:** project names as indicator labels; add no field and no record.
This costs nothing and closes the naming half of the row. The only cost is the
32-byte label ceiling, which is already permanent for indicators.

### Decision 2 — How broker-issued classifications reach Hagia

This is the one row that can genuinely force a `*Begin` layout change, and it
governs two ledger rows at once: Spatial 10 and Broker 1.

Hagia must consume opaque broker classifications and never receive title, app ID,
PID, path, or regex input. The question is the shape of what crosses:

- If a classification is **a small closed set of policy classes** — "dialog-like",
  "prefers-floating", "wants-workspace-N" — it fits `SnapshotSurface.kind` or
  spare bits in `capability_bits`. That is an **Enum** change: cheap, pre-freeze,
  no layout impact.
- If a classification is **an expiring grant with its own identity** — a token
  Hagia echoes back, with a generation or expiry — it does not fit.
  `SnapshotSurface` has no reserved space, so it needs either a new snapshot
  record kind (a `*Begin` layout change) or a widened surface record (also a
  layout change).

**This decision must be made before the freeze even though the broker does not
exist yet**, because the freeze forecloses the second option. Deciding the
*shape* now does not require building the broker.

**Recommendation:** commit to the closed-set form and record it as a constraint on
the broker's eventual design. Expiring per-surface grants are the more powerful
model, but they buy capability the blind-WM contract does not obviously need, and
they are the only thing in the ledger that forces a layout change.

### Decision 3 — The continuous-pointer payload

`PolicyInteractionKind` needs `Drag` and `Scroll` values, and
`PolicyInteractionPhase`'s existing `Begin`/`Update`/`Cancel` need
implementations. Both are pre-freeze **Enum** work.

The payload question is better than feared. `ProjectionRequest` already carries
`interaction_x`, `interaction_y`, `interaction_width`, `interaction_height` as
four `i32`s, plus two reserved `u16`s including `reserved_cause`. A scroll axis
plus delta fits the existing integer fields, and an axis discriminant fits
`reserved_cause`. **No layout change is required** provided the continuous
payload is expressed within that budget.

**Recommendation:** bind scroll and drag into the existing interaction fields and
spend `reserved_cause` on the axis discriminant. Do not add a field to
`PolicyRequestCause::Interaction`, and do not introduce a parallel cause message.
Record the field reuse in the schema so the meaning is not rediscovered later.

Two coupling notes that constrain *when*, not *what*:

- `Cancel` is a lease-revocation contract. Security transitions advance epochs
  and revoke leases and captures, so specify `Cancel` alongside the lock and
  security-authority epoch barrier or it will be specified twice.
- Per-motion `Update` requests are currently dropped: the queue deduplicates by
  pointer-gesture source and returns `Duplicate` for a second request on the same
  surface and mode. Continuous updates need a coalescing rule — replaceable
  latest-value, matching the reduced-continuous-value discipline already ratified
  for target-resolved input — not a queue-capacity increase.

### Decision 4 — The logical-space contract for outputs

`SnapshotOutput` carries `output, generation, focus, bounds, work_area` and no
scale, transform, mode, or enabled flag. This looks like a missing pre-freeze
field addition and is not:

- Hagia's `Scale` is a fixed-point column-width ratio, not display scaling.
- Policy operates purely in the logical space it is handed via `bounds` and
  `work_area`.
- Rotation can be absorbed by Engine presenting pre-rotated logical bounds.
- Disablement is expressed by omission from the complete snapshot, bounded by the
  permanent 16-output maximum.

**Recommendation:** write this down as a contract rather than leaving it implicit.
The output-authority tranche is the largest remaining chunk of work and the most
likely place to reach for a wider `SnapshotOutput`. Note also that the row demands
reservations and a separate power authority beyond test/apply/rollback, and that
reservations couple to the shell work-area coordinator — so the temptation will be
real. `SnapshotOutput` has no reserved space; widening it is a layout change.

---

## Two Open Items This Pass Does Not Decide

Both belong to whoever owns the product call, not to this enumeration.

1. **The forward-compatibility rule.** Either admit unknown record kinds by
   skipping them and add a generic extension chunk, requiring the archived
   revision-1 client to ignore what it does not know; or declare that revision 3
   is final for WM-side records and that every future authority gets its own
   interface family. The second appears to be the existing intent — the envelope
   is deliberately role-neutral and each role negotiates independently — but it
   is nowhere written as a constraint on `sophia_wm_v1`. Writing it down removes
   most of the residual risk at no engineering cost.

2. **Native output mirroring.** The ledger requires it to be implemented with
   evidence or explicitly rejected with a written architectural or product
   rationale before the port gate closes; "not yet implemented" is not an
   exclusion. This is an output-service product decision. Resolving it either way
   removes a row from the freeze critical path.
