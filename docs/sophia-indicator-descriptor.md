# Sophia Indicator Descriptor

**Role:** target contract for policy-authored desktop status.
**Status:** wire contract and canonical Engine reducer path landed in
`sophia_wm_v1` revision 1; the production chrome renderer is unwritten. The
records, capability bit, and count
fields exist in `protocol/sophia-wm-v1.kdl` with generated Rust and C99 codecs
and golden vectors. Nothing assembles or renders indicators yet.
`docs/architecture.md` and `docs/sophia-policy-ipc.md` remain authoritative
where this document appears to disagree.

A spatial-policy process owns tags, views, groups, or columns privately. Engine
owns none of them: `sophia_wm_v1` snapshots carry no workspace, tag, view, or
layout-tree state, and Milestone 13.3 replaces workspaces with output
projections. Engine therefore cannot publish desktop status, because it does not
have it.

The policy process is the only party that knows. Left unsolved, every window
manager grows a shell-facing socket and every shell grows a backend per window
manager. Noctalia carries nine such backends today.

The descriptor closes that gap without a second endpoint. Policy attaches
indicators to its layout proposal. Engine commits them with the geometry and
republishes them verbatim. Engine never interprets them.

## An Indicator Vocabulary, Not A Workspace One

The obvious design is a workspace record: identity, name, coordinates,
occupancy, urgency. That is the shape `ext-workspace-v1` chose and the shape
Noctalia independently derived across nine compositors.

Sophia must not adopt it. `docs/architecture.md` protects policy processes that
"try a model for which the usual window-manager vocabulary is a poor fit." A
scrolling policy has columns. A single-application session has nothing. Force
either into a workspace schema and it will either lie or open a side channel,
and the fragmentation returns through that channel.

An indicator is weaker and therefore more general. It is an ordered, labelled
slot with state flags and an optional action token. A tag policy emits one slot
per tag. A scrolling policy emits one per column. A kiosk emits none.

Nothing is downsampled, because policy authors the presentation directly instead
of translating a private model into someone else's vocabulary. The shell renders
slots and submits tokens. It never learns what a workspace is.

Noctalia's derived struct fits without becoming the schema. Its `id`, `name`,
`active`, `urgent`, and `occupied` map onto a slot; its `coordinates` and
`index` are the slot ordering, which is free.

## Ownership

| Concern | Owner |
| --- | --- |
| Slot content, labels, ordering, action tokens | spatial-policy process |
| Validation, atomic commit, retention, clearing | Engine |
| Transfer assembly and peer admission | session runtime |
| Rendering | Engine chrome, or a later authorized shell |
| Titles, icons, trust badges | metadata broker, not this interface |

Identity stays out. A taskbar needs window titles and icons; those come from the
metadata broker under separate authorization. Bundling them here would hand
every status bar the right to read client identity.

## Wire Shape

Two records in the existing `projection` transfer, and two count fields.

`ProjectionIndicator`, record kind 3:

| Field | Type | Meaning |
| --- | --- | --- |
| `output` | `u64` | owning output |
| `slot` | `u32` | display order within the output |
| `indicator` | `u64` | policy-private opaque identity |
| `action` | `u64` | advertised opaque action token, 0 for none |
| `state_bits` | `u16` | see below |
| `label_len` | `u16` | bytes used in `label` |
| `label` | bounded UTF-8, max 32 | short display text |

`state_bits`: bit 0 active, bit 1 urgent, bit 2 occupied, bit 3 visible
elsewhere. Unknown bits fail closed.

`ProjectionOutputStatus`, record kind 4:

| Field | Type | Meaning |
| --- | --- | --- |
| `output` | `u64` | owning output |
| `focus_bits` | `u16` | bit 0: output has a focused surface |
| `layout_len` | `u16` | bytes used in `layout` |
| `reserved` | `u32` | zero |
| `layout` | bounded UTF-8, max 32 | policy-approved layout name |

`ProjectionBegin` gains `indicator_count` and `status_count`. The begin record
must declare every category count, so both are required rather than optional.
Each new record kind needs its own count even though `ProjectionOutputStatus`
is at most one per output: a policy without the capability declares zero and
pays no per-output cost.

**This is why the change cannot wait.** Adding a record kind is an additive
revision. Adding a field to an existing message layout is not — after
`sophia_wm_v1` freezes at 13.4, it would require a new interface family. The
descriptor must land in revision 1.

## Permanent Bounds

These cannot be widened later without a new interface family. They are chosen
once.

| Bound | Where | Value | Rationale |
| --- | --- | --- | --- |
| indicator records | `ProjectionIndicator max` | 256 | matches `max-bindings`; 16 KiB on the wire, well inside the 64 KiB frame |
| status records | `ProjectionOutputStatus max` | 16 | one per output at most |
| `label` | record field | 32 bytes UTF-8 | "web", "3", "code" — generous for a slot label |
| `layout` | record field | 32 bytes UTF-8 | "ThreeColMid", "Mirror Tall" fit with room |
| indicators per output | Engine validation | 32 | a bar showing more is unreadable |

The first four are wire bounds and are frozen with the interface. The
per-output limit is an Engine validation rule rather than a wire constant,
because the wire carries a flat record array and the owning output is a field;
it is therefore the one bound that could be revised without a new family.

A proposal exceeding any bound is rejected whole. Truncation is never silent.
Labels are zero padded to their full width and `label_len` gives the used
prefix; Engine validates that the prefix is well-formed UTF-8 and that the
padding is zero.

## Commit Semantics

The descriptor rides the proposal and commits with it. That is the entire
mechanism, and two properties fall out of it rather than being enforced on top:

- **A rejected proposal never reaches a shell.** Its indicators go into the bin
  with its geometry. No observer can read a tag the screen does not show.
- **Policy loss cannot leave stale status.** Engine holds the descriptor, so
  Engine clears it when the connection epoch changes. A replacement policy
  cannot inherit its predecessor's published state through a shell.

`validation/tla/ShellObservation.tla` records both properties. An earlier design
in which policy published on its own clock required two explicit rules to
achieve the same result, and TLC refuted that design in five steps when either
rule was removed.

After policy loss and before the replacement's first commit, Engine's last
committed layout survives but its indicators do not. No live policy can vouch
for what the on-screen projection means. The feed reports no indicators until
the replacement commits. This is the fail-closed reading and it is deliberate.

**An epoch transition is a publication point, not private bookkeeping.** Engine
must expose the cleared descriptor under the new epoch at the moment the epoch
advances. TLC refuted a version that advanced the epoch silently: a tier-1
observer could then read a state Engine had never announced, which is exactly
the torn read the design is supposed to make impossible.

A tier-1 observer holding its own copy may lag, and no rule prevents that. What
the design does guarantee is that every triple it holds — serial, indicators,
epoch — is one Engine actually exposed, so the observer can always tell whether
it is current. Being behind is recoverable. Being silently wrong is not. Tier 0
avoids the question entirely, because Engine chrome renders the committed
descriptor with no second copy to go stale.

## Redaction

The descriptor is policy declassifying its own private model, which is the
opposite direction from the rest of `sophia_wm_v1`.

No filtering is required. A blind policy cannot leak client identity into a
label because it never received any: snapshots carry opaque `SurfaceId` values
and no titles, classes, PIDs, or paths. The blindness that protects clients from
policy also sanitizes policy's output.

Engine validates shape, bounds, and UTF-8. It does not inspect meaning, because
there is no meaning it could recognise.

Labels and layout names must be nonempty UTF-8 without control characters.
Indicator identities are nonzero and unique per output; slots are unique per
output but need not be contiguous. Records name only affected outputs. A
proposal completely replaces the descriptor for every affected output, while
unaffected outputs retain their last committed descriptor.

## Capability Negotiation

`capability "indicators" bit=8`. A policy that does not advertise it sends no
indicator records, and Engine renders no indicator chrome.

Optionality here does not repeat Wayland's mistake, because there is no
alternative channel to fall back to. A policy either uses this interface or
publishes nothing at all.

## Rendering Tiers

The descriptor is the data. How it reaches a screen is separate, and three
answers coexist:

- **Tier 0 — Engine chrome.** Engine draws an indicator strip from the committed
  descriptor. No client, no new interface. This covers a status bar's entire job
  and reuses the chrome path that already draws focus rings and frames under
  `capability "chrome"`. The strip's bounded geometry is session/Engine chrome
  configuration established before WM work rectangles are produced; descriptor
  commits change contents, not the reservation. Policy loss therefore clears
  slots while retaining the already-projected work area.
- **Tier 1 — `sophia_shell_v1`.** A separately authorized display-list client
  for rich shells. Deferred; see `docs/sophia-shell-v1-direction.md`.
- **Tier 2 — X11 compatibility.** Ordinary X clients under the frontend.

Tier 0 ships first. It also removes the unresolved 64 KiB texture question from
the critical path, since that constraint binds Tier 1 alone.

The existing `reserve_indicator_strip` and layout reducer are pure helpers;
nothing in the production session currently assembles or renders this tier.
The reservation rule above is a required production wiring invariant, not a
claim that the strip is already active.

## Non-Goals

- Not a workspace protocol. Slots have no semantics Engine can name.
- Not a metadata channel. Titles, icons, and badges belong to the broker.
- Not a placement or focus authority. A shell submits an opaque token; policy
  and Engine decide what happens.
- Not a display list. Tier 0 may add a private semantic strip command, but it
  lowers through existing renderer layers and admits no protocol primitive.
