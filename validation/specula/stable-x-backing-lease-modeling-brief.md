# Stable X Backing Lease Modeling Brief

## 1. System Overview

A software-rendered X toplevel reaches the screen as CPU pixels. X Authority
owns the canonical raster per drawable and a composed presentation raster per
toplevel; the session lowers updates into Engine; the renderer keeps a registry
copy addressed by handle; head composition resolves a handle into the bytes a
frame is built from.

This brief supersedes the exclusion recorded in
`buffer-age-damage-history-modeling-brief.md` section 3.2, "The bounded CPU
buffer cache beneath target-slot lifetime". That model deliberately stopped at
the target slot. This one is the same shape one level down: what the renderer's
copy of one stable toplevel's backing contains, given updates applied while
presentations hold it.

The optimization is copy-on-write. Today an update either replaces the whole
buffer or patches it in place, and correctness is bought by copying: the
authority clones its snapshot into the transport, the session clones it again
into the registry, the scene clones it per variant, and the backend clones it
again per head. A stable 1080p toplevel with a few kilobytes of real damage
therefore moves tens of megabytes per second per head. Under copy-on-write the
bytes are shared and a mutation splits them only when someone else still holds
the old ones.

The correctness question is not whether fewer bytes move. It is whether a
presentation that was handed a buffer still sees the pixels it was handed, and
whether a registry that has only ever been patched holds exactly what a full
replacement would have produced.

Two failures are in scope, and neither is caught by anything already modelled:

- A patch applied in place while a queued or in-flight presentation still
  references the same allocation. The presentation composes pixels from a
  generation it was not planned against. `VisualRetirement` owns cohort
  retirement and `PresentFlipOwnership` owns which buffer a flip releases;
  neither says anything about the bytes underneath a handle changing while the
  handle stays valid.
- A coalesced or truncated damage set that covers less than the update owed.
  The registry copy is then stale in one region while remaining
  self-consistent, presentable, and correctly generation-ordered. This is the
  same defect `VisualDamageHistory` makes unreachable for a slot, restated for
  a client buffer.

## 2. Why The Existing Models Do Not Cover It

`VisualDamageHistory` states explicitly that it has "no lease incarnation",
because a slot's buffer keeps its content across release and reacquisition and
that persistence is what makes buffer age worth anything. A client raster under
copy-on-write does not have that property: the whole point is that a live lease
changes what a mutation is allowed to do to the allocation. The exclusion is
load-bearing there and false here, which is why this is a separate model rather
than an extension.

`VisualRetirementSlots` owns lease token and incarnation, and the ABA-shaped
stale return. This model borrows its discipline -- a lease is held or it is not,
and what it captured must remain true for as long as it is held -- without
borrowing its slot vocabulary.

`SurfaceContentStream` owns commit ordering and the property that a generation
matches the content visible for it. A copy-on-write split makes one handle's
bytes exist at two addresses at once, which is exactly the situation that
property assumes away.

## 3. Modeling Boundary

### 3.1 Model

- One stable toplevel's backing, as an abstract region partition. Content is a
  generation mark per region, as in `VisualDamageHistory`.
- The authority's presentation truth: which generation each region was last
  written by, and what damage each generation carried.
- The renderer registry's copy: which allocation it currently reads, at which
  generation.
- Allocations as identities, so sharing and splitting are observable.
- Presentation leases: acquire captures an allocation and the content it held;
  hold means that content must not change; retire frees it when nothing else
  refers to it.
- A handle epoch, so a resize can retire a handle whose bytes a lease still
  holds.

### 3.2 Do Not Model

- Pixel values, colour, formats, strides, or byte layout.
- Rectangle arithmetic, packing, and how a coalescer expresses a cover. The
  model governs what an update must cover, not how the encoder says it. A
  coalesced batch is modelled as a superset cover and a full replacement as the
  whole region set.
- The 32-rect protocol bound, which is a transport capacity rather than a
  correctness property.
- Reference-count mechanics. `Arc::strong_count` is the implementation of
  "somebody else still holds this"; the model states the condition, not the
  counter.
- Child-window composition geometry and admission-extent growth. Those are
  unchanged by this row and are pinned by existing deterministic regressions.
- Renderer image sources, DMA-BUF, and direct scanout, which do not read the
  CPU registry.

## 4. Properties

- `LeasedContentStable` -- for every held lease, the allocation it captured
  still holds what it captured. This is the exit criterion "historical handles
  stay immutable until their last presentation lease retires", stated so that a
  violation names the lease.
- `RegistryMatchesStore` -- the registry's copy equals the reconstruction a full
  replacement would produce. The `DamageCoversDivergence` analog, restated over
  the result.
- `AllocationsBounded` -- live allocations never exceed one plus the number of
  held leases. This is the exit criterion "bound backing storage": copy-on-write
  may split, but only per holder, and a split that is never reclaimed shows up
  here rather than as growth on a physical run.
- `RegistryNeverRegresses` -- the registry's generation is monotone, which is
  the stale-update refusal stated as a property.

## 5. Negative Controls

Each removes one guard and must produce a counterexample. Each maps to a
deterministic Rust regression retained under the rule in
`validation/tla/README.md`: preserve the trace before correcting model or code.

1. Allow an in-place apply while a lease is held. Violates
   `LeasedContentStable`. Regression: a leased source keeps its bytes across a
   later patch, and the split is observable as a changed allocation identity.
2. Emit a cover smaller than the generation's damage. Violates
   `RegistryMatchesStore`. Regression: an over-capacity damage list coalesced to
   the transport bound materializes byte-for-byte identically to a full
   replacement.
3. Remove the stale-generation refusal. Violates `RegistryMatchesStore`.
   Regression: a stale coalesced batch does not mutate a newer base.
4. Let retirement stop checking whether a sibling lease still holds the bytes.
   Violates `LeasedAllocationsLive`. Regression: a patch after the last lease
   drops mutates in place, allocating nothing; a patch while a second holder
   remains still splits.
5. Assert that no split ever occurs. Must be violated. This is the reachability
   control, in the shape of `PartialWriteIsReachable`: a model where the
   optimization can never apply satisfies every safety property above and
   describes a system that saved nothing. Model-only.
6. Let a resize free the old epoch's allocation while a lease holds it.
   Violates `LeasedAllocationsLive`. Regression: a presentation queued against a
   pre-resize handle still composes the bytes it was planned against.

Checked results are recorded in `validation/tla/README.md`. `AllocationsBounded`
is not the invariant either lifetime control trips first: reclaiming a
still-read allocation is caught by the liveness of that allocation before the
budget is exceeded, which is the more specific failure and the better
counterexample. The bound remains checked, and is what a leaked split would
violate.

## 6. Implementation-Only Checks

These are consequences the model cannot see and the deterministic suite must:

- The transport's 32-rect capacity is refused identically on both sides.
- Admission extents and child composition are unchanged, which existing
  regressions already pin.
- Equality on a shared snapshot still compares contents. Pointer equality would
  be cheaper and wrong: two allocations may hold identical bytes.
- Residency roots name both allocations across a split, so neither is evicted
  while a candidate refers to it.
- Warmed steady state allocates nothing: after the first pass, repeated patch
  cycles neither grow the registry nor change allocation identity.
