# Sophia Shell Interface Direction

**Role:** design note recording how `sophia_shell_v1` should be specified.
**Status:** design note for future work; non-normative.

This note does not specify the shell interface. It records a method for
specifying it, the external evidence that method draws on, and where the work
sits in the roadmap. `docs/architecture.md`, `docs/sophia-policy-ipc.md`, and
`docs/compositor-graphics.md` remain authoritative wherever this note appears
to disagree with them.

## The Decision

Derive `sophia_shell_v1` from a complete, working shell rather than from first
principles. Use Noctalia as the driving client.

`docs/sophia-policy-ipc.md` already fixes the gating condition:
`sophia_shell_v1` will be modeled and specified "only when retained shell
workflows establish its smallest useful display-list, hit-target,
presentation-data, and action vocabulary." This note proposes Noctalia as the
retained workflow that establishes it, and records what that workflow already
demonstrates.

## Why A Driving Client

`sophia_wm_v1` was specified against two demanding clients: supervised xmonad
through the private compatibility bridge, and Hagia as native policy. That
pairing kept the interface honest. It had to be wide enough to carry a real
window manager and narrow enough to stay metadata-blind, and each client
falsified a different kind of design error.

The shell interface has no comparable client. Specified in isolation it fails
in one of two directions:

- **Too narrow.** It cannot carry a real desktop, every shell falls back to the
  X11 compatibility path, and the native interface becomes decorative.
- **Too wide.** It re-exposes the titles, classes, PIDs, and view identities
  that `docs/sophia-policy-ipc.md` forbids, and the authority separation
  becomes nominal.

xmobar is the only shell-like client with retained evidence today
(`docs/x11-compatibility-matrix.md`). It is a useful presentation and
work-area probe, but it is deliberately minimal: static text, no hit targets,
no popups, no animation, no desktop surfaces. Specifying a shell interface
against xmobar would produce the too-narrow failure.

Noctalia is a suitable driver for three reasons. It is feature-complete across
the shell surface area, so it exercises the full vocabulary rather than a
corner of it. It is externally developed and not Sophia-aware, so it cannot
quietly assume Engine internals the way an in-tree client would. And its
existing protocol usage is a finite, enumerable list, which converts "what does
a shell need?" from a design question into a reading exercise.

## What Noctalia Is

An independently developed Wayland desktop shell in C++, built with Meson,
surveyed at `~/src/noctalia`. It provides bars, panels, dock, launcher,
notifications, lock screen, idle behavior, OSDs, theming, wallpapers, desktop
widgets, and multi-monitor shell surfaces.

Measured scale, for judging what a port or an extraction would cost:

| Subsystem | LOC | Relationship to the display system |
| --- | --- | --- |
| `src/shell/` | 120,631 | shell logic; ~109 files name Wayland types |
| `src/ui/` | 28,333 | widget layer; display-system neutral |
| `src/render/` | 17,778 | scene graph and GLES2 backend |
| `src/dbus/` | 15,544 | neutral |
| `src/config/` | 14,433 | neutral |
| `src/system/` | 13,607 | neutral |
| `src/compositors/` | 12,435 | per-compositor IPC behind virtual backends |
| `src/theme/` | 11,932 | neutral |
| `src/scripting/` | 11,520 | neutral |
| `src/wayland/` | 10,808 | the display-system binding |
| remainder | ~34,000 | mostly neutral services |

Total is roughly 291,000 lines across 1,197 files. The display-system binding
proper is under four percent of that. Roughly 84 files outside `src/wayland/`
name core surface types directly, and about 197 name any generated Wayland
type. Those call sites are the mechanical cost of introducing a platform seam.

Noctalia renders with GLES2 over EGL. Its only Wayland coupling in the render
path is `wl_egl_window` creation, resize, and teardown in
`src/render/backend/gles_render_backend.cpp`. Cairo and Pango appear only to
rasterize glyphs into A8 textures, not to draw the interface.

## The Rendering Fork

Noctalia draws its own pixels. `docs/sophia-policy-ipc.md` states that for the
shell interface "Engine retains rendering and hit-testing" and the shell "emits
bounded shell proposals or opaque actions." `docs/compositor-graphics.md`
describes the mechanism: a bounded immutable display list of visual intent,
lowered by Engine into its own primitives and cached textures.

Those two facts are incompatible, and the incompatibility defines two distinct
paths that must not be conflated.

**Path A — X11 compatibility client.** Noctalia keeps its renderer and runs
through the Sophia X Server Frontend, obtaining pixels through EGL on X11 and
the Mesa DRI3/Present path. This requires no new interface. It is reachable
with today's architecture, and the Kitty entry in
`docs/x11-compatibility-matrix.md` establishes precedent for the GPU transport:
ARGB and sRGB FBConfig selection, direct contexts, depth-32 DRI3 import, and
Present submission. But Path A grants no redacted presentation feed, has no
tray or XEmbed, approximates layer placement with override-redirect windows and
`_NET_WM_STRUT_PARTIAL`, and is precisely the shell-as-X11-application model
the native interface exists to replace.

**Path B — native shell client.** Noctalia targets `sophia_shell_v1`, emits a
display list, and Engine renders. Its GLES2 backend, shader programs, and
texture management become dead weight; `src/render/` is rewritten from a
renderer into a display-list emitter. This is architecturally correct and
substantially more work.

The two paths are not sequential, and Path A is not a stepping stone to Path B.
They diverge at the renderer. Committing to Path A as an interim measure means
building throwaway integration and accreting compatibility expectations that
make Path B harder to justify later.

**Noctalia is a useful specification driver for Path B regardless of whether
Noctalia itself is ever ported.** Its value here is evidentiary, not
operational. The scene graph enumerates what a real shell draws; the protocol
bindings enumerate what a real shell needs to know. Both are needed to specify
the interface, and neither requires shipping the port.

## Evidence: The Display-List Delta

`docs/compositor-graphics.md` proposes an initial display-list vocabulary and
requires that "new primitives require demonstrated compositor use." Noctalia's
scene graph is demonstrated use. Comparing the two:

| Proposed Sophia primitive | Noctalia equivalent | Status |
| --- | --- | --- |
| Solid rectangles | `RectNode` | covered |
| Rounded rectangles | `RectNode` | covered |
| Rounded or gradient borders | `RectNode`, linear-gradient program | covered |
| Analytic rounded-rect shadows | rect program | covered |
| Image textures | `ImageNode` | covered |
| Cached text textures | `TextNode`, `GlyphNode` | covered |
| Clips, opacity, placement, stacking | `Node` | covered |
| Offscreen effect groups | `EffectNode` | covered |

Noctalia additionally carries nodes with no counterpart in the proposed
vocabulary:

| Noctalia node | Purpose | Proposed disposition |
| --- | --- | --- |
| `InputArea` | hit targets | **admit** — the hit-target vocabulary the interface already reserves |
| `WallpaperNode` | desktop background | **admit** — session-owned surface class, not novelty |
| `ScreenCornerNode` | screen-corner masking | **evaluate** — cheap analytic primitive, plausibly general |
| `SpinnerNode` | indeterminate progress | **evaluate** — general UI need, but expressible as animated texture |
| `GraphNode` | sparklines and history graphs | **refuse as primitive** — client-rasterized texture |
| `CountdownRingNode` | timer arcs | **refuse as primitive** — client-rasterized texture |
| `AudioSpectrumNode` | audio visualizer | **refuse as primitive** — client-rasterized texture |
| `FancyAudioVisualizerNode` | audio visualizer | **refuse as primitive** — client-rasterized texture |
| blur program | backdrop blur | **defer** — Engine-owned effect, not a shell primitive |

This delta is the direction's first concrete output. It is the kind of finding
that first-principles design does not produce, because nobody guesses "audio
spectrum" while enumerating a compositor primitive set.

## The Novelty Risk

The driving-client method has a governance failure mode that must be named
before the work starts. `docs/compositor-graphics.md` states that "one-off
visual novelty is not sufficient reason to expand the stable Engine boundary."
A real shell will demand exactly that novelty. Four of Noctalia's nodes above
are visual novelty by that standard.

Letting a driving client expand the primitive set on demand turns Engine into a
general-purpose UI toolkit, which the architecture explicitly refuses. The
method therefore requires a pressure-release valve, and the natural one already
exists in the vocabulary: **client-rasterized image textures.**

A shell that wants an audio visualizer rasterizes it and uploads a texture.
Engine composites the texture without learning what it depicts. Novelty stays
client-side, the primitive set stays small, and the boundary holds. This should
be an explicit stated rule of the specification process, not an outcome
negotiated per primitive.

The open cost is that texture upload has different damage, bandwidth, and
power characteristics than an analytic primitive, and an animated visualizer
uploading every frame is the worst case. Quantifying that cost is a
prerequisite to relying on the valve.

## Derived Capability Requirements

Noctalia binds 25 Wayland protocols. Each is a requirement statement about what
a complete shell needs. Grouped by the Sophia authority that would own the
capability:

### Surface placement and stacking

| Noctalia dependency | Shell need | Owner |
| --- | --- | --- |
| `wlr-layer-shell-unstable-v1` | anchors, exclusive zones, four-layer ordering, keyboard-interactivity modes | `sophia_shell_v1` |
| `xdg-shell` | popups, positioners, grabs | `sophia_shell_v1` |
| `hyprland-focus-grab-v1` | dismiss-on-click-outside for menus | `sophia_shell_v1` |

Exclusive-zone negotiation is the load-bearing piece and has no X11 analog
beyond strut approximation. It is the single largest gap.

### Output geometry and scaling

| Noctalia dependency | Shell need | Owner |
| --- | --- | --- |
| `xdg-output-unstable-v1` | logical output geometry and names | Engine projection |
| `fractional-scale-v1` | per-output fractional scale | Engine projection |
| `viewporter` | buffer-to-surface scaling | Engine (display list) |
| `wlr-output-management-unstable-v1` | output configuration UI | session/config domain |

Engine already owns outputs and work areas. Most of this is projection, not new
authority.

### Presentation data

| Noctalia dependency | Shell need | Owner |
| --- | --- | --- |
| `ext-workspace-v1` | workspace list, occupancy, focus | **indicator descriptor** |
| `ext-foreign-toplevel-list-v1` | taskbar and dock entries | metadata broker |
| `wlr-foreign-toplevel-management-unstable-v1` | activate, close, minimize | broker plus opaque actions |

This group is redaction-critical and is where the interface most easily goes
wrong. It corresponds to the existing roadmap item for a bounded redacted
status feed.

**Superseded in part.** The first row is now answered by
`docs/sophia-indicator-descriptor.md`, and not by a broker. Workspace state
originates in the policy process, not in Engine and not in any metadata the
broker can see, so a broker has no upstream source for it. Policy attaches
indicators to its layout proposal and Engine republishes them after commit.

The correction matters beyond one row. This table originally assigned all
presentation data to one owner. There are two, with different trust properties:
policy-authored structure is blind-safe by construction, while broker-supplied
identity needs real sanitization. Keeping them separate means a status bar never
requests identity at all.

### Session services

| Noctalia dependency | Shell need | Owner |
| --- | --- | --- |
| `ext-session-lock-v1` | lock surface with input isolation | session service |
| `ext-idle-notify-v1` | idle timers | session service |
| `idle-inhibit-unstable-v1` | inhibit during media playback | session service |
| `xdg-activation-v1` | launch and focus handoff | session capability |

### Capture and transfer

| Noctalia dependency | Shell need | Owner |
| --- | --- | --- |
| `wlr-screencopy-unstable-v1` | screenshots, region capture | portal |
| `ext-data-control-v1`, `wlr-data-control-unstable-v1` | clipboard history | portal |

Sophia's portal model differs structurally from data-control. A clipboard
manager under a portal grant is not a drop-in translation and needs its own
design pass.

### Input

| Noctalia dependency | Shell need | Owner |
| --- | --- | --- |
| `text-input-unstable-v3` | on-screen keyboard input method | Engine plus broker |
| `virtual-keyboard-unstable-v1` | on-screen keyboard injection | Engine, gated capability |
| `cursor-shape-v1` | cursor over shell surfaces | Engine (already protocol-neutral) |

Virtual keyboard is synthetic input injection and needs an explicit capability
grant. It should not be reachable from an ordinary shell authorization.

### Display control and compatibility

| Noctalia dependency | Shell need | Owner |
| --- | --- | --- |
| `wlr-gamma-control-unstable-v1` | night light, color temperature | Engine |
| `ext-background-effect-v1` | backdrop blur behind surfaces | Engine effect |
| XEmbed (not Wayland) | system tray | retained-workflow admission only |
| `dwl-ipc-unstable-v2`, `org-kde-plasma-virtual-desktop`, `hyprland-toplevel-mapping-v1` | per-compositor workspace and window IPC | not applicable |

The last row is Noctalia's compositor-specific backend layer. Under Sophia it
is replaced wholesale by the redacted presentation feed, which is a
simplification rather than a port.

## Authority Constraints The Driver Must Not Relax

Recorded here because a driving client creates continuous pressure to relax
them:

- The shell receives only broker-approved presentation facts. Titles, classes,
  PIDs, paths, XIDs, namespace identities, and portal payloads stay out
  regardless of what the driving client's widgets would like to display.
- Shell authority cannot set application placement or focus. Dock and taskbar
  activation is an opaque action submitted for adjudication, not a focus call.
- WM authority cannot acquire shell metadata. Hagia's blind spatial-policy
  projection stays blind; `hagia-shell` is a separately authorized process.
- Engine retains rendering, hit-testing, physical input, grabs, cursor state,
  and animation. The shell describes visual intent and receives resolved hit
  events.
- The shell endpoint is distinct from `sophia_wm_v1`. Sharing an executable,
  repository, or language never combines capabilities.

A shell that cannot be built within these constraints is evidence about the
constraints, and that evidence belongs in the research log before any boundary
moves.

## Open Questions

1. Does a display-list shell interface carry a full desktop at acceptable cost,
   or does per-frame list submission for animated content force a buffer path?
   This is the question that decides whether Path B is viable at all.
2. **Can the shared transport carry shell texture traffic at all?** This is the
   one open question that reaches back into `sophia_wm_v1` and is therefore
   sequencing-critical. `docs/sophia-policy-ipc.md` limits one frame to 64 KiB,
   uses begin/chunk/end transfers for anything larger, permits one transfer in
   flight per direction, and describes a bytes-only wire with no file-descriptor
   passing. A 1920x40 bar at ARGB8888 is roughly 307 KiB, or five chunks for a
   single full upload. A continuously animating widget is not expressible.
   Three candidate resolutions, in order of preference:
   - **Content-addressed cached textures.** Upload once, reference by handle
     thereafter. `docs/compositor-graphics.md` already implies this with its
     cached-text strategy. Preserves the bytes-only wire. Does not help
     genuinely per-frame content.
   - **A descriptor-passing side channel for the shell role only.** Language
     neutral over a unix socket, but it weakens the independently-implementable
     property and touches shared transport code in `sophia-runtime`.
   - **Strictly analytic display lists, no client rasterization.** Preserves the
     transport untouched but removes the novelty valve above, forcing primitive
     expansion and the toolkit outcome the architecture refuses.
3. What is the damage, bandwidth, and power cost of the client-rasterized
   texture valve under a continuously animating widget?
4. Can exclusive zones, layer ordering, and keyboard-interactivity modes be
   expressed without giving the shell placement authority over applications?
5. *Partly answered.* Workspace and layout structure is settled by
   `docs/sophia-indicator-descriptor.md`: policy-authored slots, blind-safe by
   construction, carried on the commit. What remains open is the dock and
   taskbar half, which needs per-window identity from the broker. Icons are the
   hard case: an application icon is close to an identity disclosure, and no
   structural property makes it safe the way policy blindness makes labels safe.
6. Does the shell need its own animation clock, or does Engine own animation
   and the shell only declare transitions?
7. Where does the launcher live? It needs a text input, a result list, and
   arbitrary launch, and it straddles shell, broker, and session capability.

## Roadmap Placement

This direction sits in `todo.md` under **Post-Promotion Capability Roadmap**
and is gated behind existing work. It cannot start earlier than its
dependencies allow:

- **Milestone 12** must complete. The installed daily-driver candidate is the
  current promotion vehicle and takes precedence.
- **Milestone 13** must complete, specifically 13.4, which proves Hagia and
  freezes `sophia_wm_v1`. The shell interface reuses that interface's process
  and must not be specified against a moving WM contract.

### Sequencing: After `sophia_wm_v1`, Not In Parallel

The two interfaces are sequential, and the ordering is `sophia_wm_v1` proven by
Hagia first. Four reasons:

- **The specification process is the risky part, not the interface.** Milestone
  13 exercises a full pipeline: ratified boundary, formal model, declarative
  schema, generated Rust and C99 codecs, checked-in golden vectors, and an
  independently implemented client that passes a cross-repository conformance
  gate. Hagia has already shown that pipeline works end to end in a third
  language. Debugging that machinery once, on the simpler and better-understood
  interface, is much cheaper than debugging it twice concurrently.
- **Freezing requires quiet.** 13.4 exists to freeze `sophia_wm_v1`. Concurrent
  shell design generates continuous pressure to reopen shared framing,
  transport, and versioning decisions at exactly the moment they need to stop
  moving.
- **Engine is not ready to render a shell.** Path B requires Engine-side
  display-list lowering, cached-texture strategy, and damage handling. That is
  Milestone 14 and `docs/compositor-graphics.md` work. The protocol cannot be
  specified ahead of the rendering model it describes.
- **The metadata broker is a larger prerequisite than the protocol.** The
  redacted presentation feed has no implementation. Without it, most of the
  presentation-data table above has nothing to connect to, and the shell
  interface would be specified against a data source that does not exist.

**One item should run in parallel, and only one.** Open question 2 asks whether
the shared transport can carry shell texture traffic given the 64 KiB frame
limit, single in-flight transfer, and bytes-only wire. That question reaches
back into shared code in `sophia-runtime`, so the answer is worth having
*before* 13.4 freezes rather than after.

The risk is bounded, not fatal: the 24-byte envelope is deliberately
role-neutral and each role negotiates its own family and revision, so a
shell-role descriptor extension is additive and does not rewrite the WM
contract. The exposure is implementation coupling in the shared transport, not
wire-format lock-in. That justifies a cheap analysis pass during Milestone 13,
not a parallel specification effort.

Requirements capture — this document and any refinement of it — is also
free-running. It consumes no Engine or protocol work and blocks nothing.

### Choosing The Driving Client

`docs/specification.md` states Sophia must serve "a lean policy client such as
xmonad or qtile, a conventional environment such as Xfce, and designs that do
not look much like today's window managers." Xfce is therefore an architectural
target, and it is a strong candidate for a driving client — but for a different
interface than this one.

| | Xfce | Noctalia |
| --- | --- | --- |
| Native display system | X11 | Wayland |
| Rendering model | GTK3 immediate drawing into Cairo | retained scene graph of typed nodes |
| Emits a display list | no, and never will | structurally yes, unserialized |
| Real installed users | many | few |
| Admission model fit | unmodified retained workflow | requires a port |
| Drives | X11 compatibility completeness | `sophia_shell_v1` vocabulary |

The distinction is not quality, it is which interface each one can falsify.

Xfce is a GTK3 application stack that draws its own pixels through Cairo and
consumes EWMH, struts, and XEmbed. Under Sophia it is a Path A client
permanently. It cannot exercise a display-list protocol because it will never
produce a display list, so it cannot falsify a single design decision in
`sophia_shell_v1`. What it *can* falsify is the X11 compatibility surface —
frontend coverage, WM bridge behavior, strut and work-area reservation, and
tray/XEmbed admission — and it fits the existing unmodified-retained-workflow
admission model that xmonad, xmobar, Kitty, and Firefox already use.

Noctalia's value is narrower and more specific: `src/render/scene/` is already a
retained tree of typed primitives with hit-test areas attached. That is
structurally the artifact `sophia_shell_v1` is trying to standardize, minus
serialization. A driving client for a display-list protocol has to *have* a
display list, and among realistic candidates Noctalia does and Xfce does not.

Two honest caveats on Noctalia. Its requirements arrive expressed in Wayland
protocol terms that do not all translate, so the capability tables above are a
translation exercise rather than a direct reading. And its user base is small,
so it carries less weight as evidence of what a desktop *must* have.

**Recommended: both, on different interfaces.** Xfce as a classical
compatibility profile under **Classical X11 WM Compatibility**, alongside the
existing i3, dwm, and qtile candidates. Noctalia as the display-list and
capability driver for `sophia_shell_v1`. Neither substitutes for the other, and
neither should be cited as evidence for the other's interface.

Existing roadmap items this direction feeds, all under **Native Sophia
Follow-Ups** and **Status, Launcher, And Shell Integration**:

- Model and publish `sophia_shell_v1` through the same formal, schema, C
  client, and permanent-compatibility process. This note supplies the input
  evidence that item is waiting on.
- Build `hagia-shell` as one ordinary separately authorized shell client. The
  display-list delta and capability tables above are its requirement set.
- Define a bounded redacted status feed. The presentation-data table narrows
  this from open-ended to a specific list.
- Implement lock, screenshot, wallpaper, and audio actions through their owning
  boundaries. The session-services and capture tables enumerate them.
- Admit tray/XEmbed only from a retained application workflow.

**Proposed process shape.** Milestone 13 is the template, and the shell
interface should follow its five-stage structure rather than inventing a new
one: ratify and model the boundary, publish a dependency-free wire contract,
project the state the interface exposes, prove one demanding client and freeze
the interface, then migrate and promote.

**Proposed first gate, ahead of any specification work.** A bounded rendering
probe answering open question 1, since a negative answer invalidates Path B
before any schema is written. Two independent measurements:

- Emit a representative Noctalia bar frame as a display list against the
  proposed vocabulary, and measure list size, submission cost, and Engine
  lowering cost at a realistic update rate.
- Separately, admit Noctalia as an X11 client through the frontend under Path A
  purely as GPU-transport evidence, proving EGL-on-X11 reaches Engine
  presentation. This is a compatibility-matrix entry, not a product direction,
  and the distinction should be stated in the matrix entry itself so it is not
  later cited as shell support.

Neither probe commits the project to porting Noctalia.

## Non-Goals

- This is not a commitment to port Noctalia. Noctalia is a specification input.
- This is not a commitment to run Noctalia under Sophia in any form.
- This does not promote Path A. Any X11 admission of Noctalia is transport
  evidence for the compatibility matrix only.
- This does not expand the display-list vocabulary. It records a candidate
  delta and a proposed disposition for each entry; expansion follows the normal
  demonstrated-use process.
- This does not alter `sophia_wm_v1`, Hagia's blindness, or any existing
  authority boundary.
