# Direct Scanout Eligibility Modeling Brief

## 1. System Overview

Every client DMA-BUF reaching a Sophia screen today is composed. The Present
path stages the client's buffer as a layer, the renderer draws it into a
compositor-owned GBM buffer, and that buffer gets the KMS framebuffer
(`production_visual_runtime/present.rs:114-181`,
`gbm_platform/scanout/context/render_once.rs:256+`). The `MixedPresent`
content tag names which Present transaction a flip belongs to; it is
attribution, not a direct path
(`persistent_native_scanout/renderer_images.rs:512-548`).

Milestone 14 asks for the complement: when the exact frame is one opaque
DMA-BUF layer and nothing else, put the client's buffer on the plane and skip
composition entirely. Engine must prove the frame needs no composition, the
backend must validate the exact format and modifier through an atomic test,
and any refusal returns to mixed composition without losing committed visual
state (`docs/architecture.md:784-787`). An active overlay or effect that
samples the scene, uses an offscreen group, or otherwise changes the composed
image makes that exact frame ineligible; removing it may restore eligibility
only on a later independently validated frame
(`docs/compositor-graphics.md:193-198`).

The pieces the backend needs exist. `AddFB2` with modifiers, PRIME import,
format and plane-count rules, and affine resource bundles destroyed only after
flip retirement are all in `drm/native_primary_plane/`. `TEST_ONLY` flags
exist but have only ever validated topology
(`drm/native_atomic/request.rs:49-62`). What does not exist is any fallback:
a rejected commit is terminal for the frame and, on the mirror path, for the
session (`native_scanout.rs:1226-1249`, `:2116-2162`).

Three prior briefs deferred this row by name, and the M14 preamble requires
the model before direct-scanout semantics change.

## 2. Scenarios

### Scenario 1: The Client's Buffer Is On Glass

**Mechanism**: a direct flip displays memory the client still owns. Releasing
it at submission or at the flip lets the client draw into pixels the screen is
scanning. Release is lawful only when a successor flip retires it.

**Evidence**:

- Code analysis: the composited path releases the client source *at* the flip
  precisely because a compositor copy is what reaches glass
  (`production_visual_runtime/native.rs:834-938`,
  `PresentCopyOwnership.tla`). The direct path inverts that.
- Code analysis: resource bundles are destroyed only after `Accepted` and
  `Presented` (`drm/native_scanout/retire.rs:3-38`), which is the mechanism
  the direct path must reuse rather than replace.

**Affected code paths**: Present settlement, submission ownership, resource
cleanup.

**Suggested modeling approach**: a displayed phase in which release is
forbidden, and a successor retirement as the only action that releases.

### Scenario 2: An Overlay Activates While A Direct Frame Is Displayed

**Mechanism**: the descriptor overlay opens, or a future effect activates. The
next frame requires composition. The direct frame is still on glass and must
stay there until the composed successor retires it.

**Evidence**:

- Doc: "An active overlay or effect that samples existing scene pixels, uses
  an offscreen group, or otherwise changes the final composed image makes that
  exact frame ineligible for direct scanout"
  (`docs/compositor-graphics.md:193-195`).
- Code analysis: the descriptor overlay and indicator strip lower to ordinary
  `Rect`/`Text`/`IndicatorStrip` commands
  (`production_visual_runtime/compositor_graphics.rs:449-475`), so they change
  the composed image without sampling it -- disqualifying either way.

**Affected code paths**: eligibility verdict, fallback ladder, Present
settlement across a composition boundary.

**Suggested modeling approach**: a composed successor that retires the direct
frame, distinct from a direct successor, so that returning to composition is
modelled as a successor rather than an eviction.

### Scenario 3: Eligibility Returns And Must Be Re-Proved

**Mechanism**: the overlay closes. The scene is structurally eligible again,
but the earlier proof and the earlier atomic test described a different
episode. The row requires a fresh proof and a fresh backend test.

**Evidence**:

- Doc: "Removing the effect may restore eligibility on a later independently
  validated frame" (`docs/compositor-graphics.md:196-197`).
- Code analysis: Engine computes a plan per frame, while the backend's atomic
  test belongs on the composition-to-direct edge -- the two are stamped
  independently and can go stale independently.

**Affected code paths**: the eligibility episode, the atomic test's placement.

**Suggested modeling approach**: an episode counter advanced by activation,
with proof and test stamps compared against it; and an action that re-proves
without re-testing, which is the asymmetry the implementation actually has.

### Scenario 4: The Driver Refuses The Exact Framebuffer

**Mechanism**: the frame is structurally eligible but its format, modifier, or
plane layout is not scannable on this plane. The atomic test refuses, or a
real commit is rejected after a passing test.

**Evidence**:

- Code analysis: commit refusal is classified only as `Rejected` with no errno
  inspection (`drm/native_scanout/prepare.rs:454-460`), so a format refusal is
  indistinguishable from a permission error and cannot be retried by kind.
- Code analysis: accepted formats and plane-count rules already exist
  (`resource_create.rs:621-638`) but are enforced at FB creation, not as a
  scanout precondition.

**Affected code paths**: the fallback ladder, direct-path counters.

**Suggested modeling approach**: both test verdicts reachable as environment
facts, and a refusal that settles the frame as an ordinary copy rather than
failing the session.

### Scenario 5: Formats, Modifiers, And Slots

**Mechanism**: which fourcc is opaque, which modifier the plane accepts, and
how renderer target slots are recycled.

**Evidence**:

- Code analysis: opacity is derived from the DRM fourcc inside the renderer
  (`gbm_platform/scanout/render.rs:538-646`); Engine's types carry no pixel
  format at all.
- Code analysis: a direct frame acquires no renderer slot, so
  `VisualRetirementSlots` is untouched by this row.

**Suggested modeling approach**: out of scope. Format is the atomic test's
verdict, which the model takes as an environment fact; slots belong to a model
that already exists.

## 3. Modeling Recommendations

### 3.1 Model

- A displayed phase in which the client buffer may not be released.
- Successor retirement as the only release, by either a direct or a composed
  successor.
- An eligibility episode ended by activation, with independently staleable
  proof and test stamps.
- Both atomic-test verdicts, and a rejected real commit after a passing test.
- The `Flip` Present settlement, reachable only from a real direct flip.

### 3.2 Do Not Model

- Pixel formats, modifiers, and plane capability: the atomic test's verdict
  stands for all of it.
- Renderer frame slots and damage history: a direct frame takes no slot, and
  `VisualRetirementSlots` and `VisualDamageHistory` own those.
- The effect provider registry, capability negotiation, and offscreen
  allocation: `todo.md:2353-2368` queue those; here an effect is a boolean
  that ends an episode.
- Mirror groups: eligibility requires a single-head plan shape, so a mirror
  output composes by construction.
- Timing, cadence, and any deadline.

## 4. Proposed Extensions

| Extension | Variables | Purpose | Scenario |
| --- | --- | --- | --- |
| Displayed client buffer | `phase`, `released` | Release only by successor retirement | 1, 2 |
| Eligibility episode | `effectActive`, `episode`, `proofEpisode`, `testEpisode` | Fresh proof and test per episode | 2, 3 |
| Atomic test verdict | `testVerdict` | Driver refusal as an environment fact | 4 |
| Settlement | `settlement`, `directFlipped` | `Flip` only from a real flip | 1, 4 |

## 5. Proposed Invariants

| Invariant | Type | Description | Targets |
| --- | --- | --- | --- |
| `DisplayedClientBufferIsNeverReleased` | Safety | The buffer on glass is not released | 1 |
| `ReleasedOnlyBySuccessor` | Safety | Release happens only after a successor retires | 1, 2 |
| `EveryFlipWasEligible` | Safety | Every flip had a proof and test fresh in its episode | 2, 3 |
| `FlipFeedbackRequiresRealFlip` | Safety | The `Flip` disposition implies a direct flip | 4 |
| `DisplayedFrameSettles` | Liveness | A displayed direct frame is eventually retired and settled | 1, 2 |

## 6. Findings Pending Verification

### 6.1 Model-Checkable

- Whether the proof and test stamps are independently load-bearing. (Checked:
  only once an action expresses re-proving without re-testing; without it the
  test stamp's control cannot fail.)
- Whether forbidding a flip during an active effect is load-bearing. (Checked:
  it is provably unreachable given the episode stamps, and is kept as a stated
  consequence rather than deleted.)

### 6.2 Testable Only

- That the eligibility verdict classifies every current compositor command,
  and that an unrecognized future command disqualifies rather than passes.
- That a `TEST_ONLY` commit carries no page-flip event, which the flag type
  already makes unrepresentable.
- That a refused direct attempt destroys its imported buffer handles through
  the existing cleanup-retry path rather than leaking them.
- That the legacy hardware cursor continues to update over a directly scanned
  frame, since it rides its own ioctl on the same CRTC.

### 6.3 Out Of Scope

- The hardware cursor plane and the per-output atomic transaction owner that
  must combine primary and cursor state.
- Effect providers, capability admission, and offscreen groups.
- Scanout cloning across heads of one device group.
