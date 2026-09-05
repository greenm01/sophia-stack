# Building on Sophia

This is the map for anyone who wants to build a desktop on the Sophia display
server — a lean tiling window manager in the dwm or niri tradition, a shell in
the Noctalia class, or a full desktop environment along the lines of XFCE or
COSMIC. It tells you which component owns what, which protocol each piece
speaks, and how the pieces fit together. Each section links to the document
that owns the details. This one owns the shape.

## The One Rule

Sophia doesn't divide the desktop by feature. It divides it by who may see
pixels.

- **Engine** composites the scene, owns every pixel, and is the only process
  that reads them.
- **Policy clients** — the window manager and the shell — decide what happens.
  They draw nothing, or they draw blind.
- **Portals** move data between confinement domains: one transfer at a time,
  brokered, with an identified recipient and the user's consent.

Every design question in this document resolves against that rule. A feature
that needs to read the screen belongs to Engine, or behind a portal decision.
A feature that needs application metadata is either refused or becomes a
portal the application opts into. There are no exceptions for convenience,
because every exception is exactly what the confinement exists to prevent.

Rendering has a corollary, the Compositing Operator Rule from
`docs/compositor-graphics.md`: Engine admits a drawing primitive only when the
client physically cannot perform the operation itself. A future content shell
rasterizes its own widgets. Engine does the blur, because blur reads pixels the
shell must never see. The confined descriptor tier cannot rasterize widgets;
Engine renders its fixed chrome from sanitized descriptors.

## Bring Your Own Language

Sophia's protocols are byte-level wire contracts over Unix sockets: a fixed
frame header, fixed offsets, explicit widths, reserved fields that must be
zero. There is no required SDK, no blessed binding, and no library you must
link. If your language can open a socket and read bytes, you can build on
Sophia.

This isn't an aspiration; it's how the existing clients work. Hagia and
Narthex are written in Nim and depend on nothing from this repository. The
archived window-manager client is 438 lines of plain C99, compiled directly
with no binding — the compatibility gate builds those exact sources and runs
them against the live server, so a wire change that breaks them is rejected
as a break, not absorbed as a refactor. The shell has its own independent C
client at 367 lines. Sophia's own codecs are Rust. Three languages already
speak the same bytes, and yours would be the fourth.

The protocol specifications live in `protocol/*.kdl` as language-neutral
descriptions: every message, field, width, and bound. The shared corpus of
golden frames, malformed frames, and fixed records gives you conformance
testing from the first day — your decoder either parses the same bytes the
Rust, C, and Nim decoders parse, or it doesn't, and no one has to take your
word for it either way.

## The Components

A complete desktop is three processes beside Engine, each one independently
replaceable:

| Component | Protocol | Reference | May draw? | Sees |
| --- | --- | --- | --- | --- |
| Window manager | `sophia_wm_v1` (r3, frozen) | [Hagia](https://github.com/sophia-org/hagia) | no | geometry, window facts |
| Shell | `sophia_shell_v1` (r2, experimental; r1 supported) | [Narthex](https://github.com/sophia-org/narthex) | no; descriptor chrome is Engine-rendered | sanitized labels |
| Broker | `sophia_broker_v1` | in-tree | no | redacted descriptors |

The window manager never learns titles, application identities, or pixel
content. The shell never learns surface identities or coordinates — the
conformance evidence records `surface_ids_disclosed=0 coordinates_disclosed=0`
on every run. The broker issues and revokes the opaque action capabilities
that let a shell activate a window it can't name.

This is the X11 process model — the server owns the display, the window
manager is just a client — plus the one thing X11 never had: the clients are
confined. Under X11, any client can walk the window tree. Under Wayland, any
layer-shell client can draw over your bank window. Here, neither is possible.
The macOS comparison is instructive too: macOS split WindowServer from
Dock.app but left no sanctioned seam for third-party window management, which
is why tiling tools there have to disable system protection to work at all.
`sophia_wm_v1` is that missing seam.

## Scripting And Live Control

[Scripting Sophia](scripting.md) defines the proposed `sophia msg` interface
for any conforming WM or shell. The session admits and authorizes callers,
routes commands to their responsible owner, and reports correlated outcomes.
WM action semantics remain in the WM; shell behavior stays within the shell
role. Neither client serves a scripting socket.

The CLI and public control endpoint are unimplemented. Existing named WM
actions and session reload/restart operations provide the first intended
command scope; generic shell commands require a separately negotiated
extension. Namespace admission does not grant desktop-control authority,
and command invocation does not grant application-data access. The scripting
contract distinguishes host-user administration, confined callers, and future
namespace-scoped automation. The experimental [control v1 wire](sophia-control-v1.md)
is specified for independent clients: disabled by default, explicit host-control
opt-in, one outstanding request per stream, and owner-settled results. Confined
delegation and shell commands require future extensions. Offline wire checks
do not establish the still-unimplemented endpoint's security or live behavior.

## The Ladder

**A window manager**, in the dwm, niri, or xmonad tradition. One binary
speaking `sophia_wm_v1`. The interface carries twelve capability bits, from
`bindings` alone up to `tab_groups`; negotiate the ones you need and
ignore the rest. A minimal tiler is a reducer — snapshot in, projection out —
and you inherit the session's shell (or none), the portals, and the
compatibility layer for free. Start from `protocol/archive/sophia-wm-v1-r3/`,
which is self-contained: the frozen spec, a generated C codec, a worked
client, and checksums. Hagia is the full-width reference.
`docs/sophia-wm-api.md` and `docs/wm-v1-freeze-surface.md` own the details.

Don't mistake this rung for a lesser desktop. What makes niri daily-drivable
isn't that the compositor does everything; it's that the environment around
the window manager is complete — clipboard, screenshots, screen capture,
notifications all work. Sophia gives the WM rung the same completeness
through portals and the session, and the window manager reaches them without
gaining an inch of authority: its keybindings map to opaque session-operation
slots. It asks for slot N; the session decides what slot N does; anything
that moves data gets a portal decision behind it. Hagia already spawns a
terminal, launches a browser, closes a window, and logs out this way. Lock,
screenshot, wallpaper, and audio ride the same pattern and are queued. The
target for Hagia plus Narthex is exactly this product: a complete,
daily-drivable, niri-class desktop where the WM owns policy, portals own
data, and nothing owns more than its job requires.

**A shell** — bar, launcher, switcher, notifications; the Noctalia class. One
binary speaking `sophia_shell_v1`, launched by the session into its own
protection domain. Revision 1 carries the descriptor switcher and bounded
work-area reservations: you order sanitized entries, Engine renders and captures
them, and you receive an opaque activation. Revision 2 adds persistent tab-group
descriptors for WM layouts. Narthex remains confined to descriptors; Engine owns
the bars' geometry, GPU rendering, and hit testing. A future content capability,
derived from a real shell's enumerated needs (`docs/sophia-shell-v1-direction.md`),
would let you rasterize your own widgets and hand over bounded content-addressed
textures for Engine to composite. It would still grant no screen reads.
The bar isn't a separate component — "shell-owned" covers a
small status strip and a full panel set alike. How your users configure it is
covered below in Two Configs, Two Owners — the short version is that your app's
settings are yours, and only the operator's envelope goes through Sophia.

**A desktop environment**, in the XFCE, COSMIC, or macOS tradition. This is
not a fourth protocol. A desktop decomposes into the pieces above, plus
portals, plus ordinary applications:

| Desktop feature | Where it lives |
| --- | --- |
| Panels, dock, launcher, OSD, lock, tray | shell |
| Workspace and window switcher | shell (descriptor capability) |
| Work-area reservation for panels | shell candidate, session-capped depth |
| Drag and drop, clipboard | portals |
| File handoff, URI open, notifications | portals |
| Screenshot, screen recording | portals |
| Desktop icons | shell surface launching through opaque actions |
| Settings and control centre | edit KDL; core config hot-reloads live, the desktop profile applies at session start. A GUI is an optional third-party editor |
| File manager | an ordinary application |
| Session save and restore | session authority, not the shell |
| Per-app menu bar | menu-export portal (below) |
| Panel plugin API | the shell's own affair, inside its domain |

One row deserves a word: a third-party plugin ABI isn't a Sophia concern.
Plugins run inside the shell's protection domain and share its authority. A
shell that loads plugins is trusting them with everything it has, and Sophia
neither knows nor cares. What Sophia guarantees is the blast radius — a rogue
plugin gets the shell's capability set, not the desktop.

### A Desktop Is a Composition, Not a Second Platform

On traditional Linux, the distance between a niri-class environment and a
KDE-class one is architectural. The desktop environment brings its own
compositor, its own session manager, its own config daemon, its own portal
backends — a parallel platform, not a window manager with additions. And the
real difference between the two was never the feature list; it's who does the
integrating. In a WM environment, the user assembles the parts and wires them
together. In a desktop environment, the project ships a tested-together whole:
one settings system every component reads, a GUI that writes it, changes that
propagate live, sessions that restore your applications and not just your
windows.

On Sophia, the architecture stops varying. Engine, session, portals, and
broker are fixed, and both a WM environment and a full desktop are
compositions of the same parts over the same wires. The integration glue that
makes a desktop feel whole — traditionally a soup of session bus daemons — is
the platform itself: the profile for configuration, session-operation slots
for actions, portals for data, the broker for metadata. Even the WM rung
inherits it, which is why that rung is complete rather than spartan.

The unified settings system is usually the deepest thing separating the two,
and Sophia already has the parts that matter. The desktop profile carries
seven typed authority sections — policy, shell, shortcut, session, input,
output, broker — with digests, validation, and a full prepare–activate–rollback
activation machine, model-checked in TLA+. Core configuration is live: the
session watches its file and reloads on change, waiting for input to fall idle,
revalidating, and applying atomically or keeping what runs. The desktop
profile's seven sections currently apply once, at session start. Its activation
reducer is built to accept a newer generation, so re-running it is not the
obstacle; the missing pieces are a watcher on the profile path and a live
re-handoff of the Policy authority to the already-running window manager, since
that authority is reached over the wire rather than settled inside the session.
That is a genuine increment, not a toggle — but every hard part, the
seven-authority prepare–activate–rollback machine, is done.

Either way the interface is the KDL file, not an application. Edit it by hand,
with `sed`, or with an editor someone writes. A settings GUI is therefore not
core work — it is an optional third-party application over a file format that
is already the interface, and it belongs outside this repository the same way
a shell backend does.

So the honest distance from the WM rung to a full desktop is a short list, in
rough dependency order: a future content capability in `sophia_shell_v1` (the
gating item), a bounded status feed so rich panels have something to show,
application-session restore in the session authority, and a live
reload path for the full profile, which needs a profile-file watcher plus a
re-handoff of the Policy authority to the running window manager over the wire.
A theming story across applications is the one genuinely unsolved item, since
applications are protocol clients and their toolkits theme themselves, but
that is a hard problem every desktop shares rather than a Sophia gap. A
settings GUI is not on the list at all: the config file is the interface.

Which yields a sentence no other platform gets to write: on Sophia, a desktop
environment is a superset composition, not a second platform. Moving from
niri-class to KDE-class changes what you ship — never how it's wired — and
the climb is also the trust gradient: a complete confined desktop at the
bottom rung, more expressiveness for more granted trust above it.

## Two Configs, Two Owners

A developer building on Sophia will want their users to configure the thing they
built. They can — and the config the user edits is the developer's own, not
Sophia's. There are two layers, owned by two parties, and keeping them straight
is the difference between an afternoon and a bad week.

**Your app's config is yours.** A shell's bar colors, widget choices, fonts,
module layout, behavior — none of it crosses into Sophia's authority, so Sophia
neither sees it nor imposes anything on it. Pick your own format, read your own
file, watch it and hot-reload it however you like. This is bring-your-own-config
to match bring-your-own-language: a shell written in Zig can read TOML its own
way. Sophia mandates a format only at the authority boundary.

**The profile is the operator's envelope.** The desktop profile — the seven
authority sections Sophia owns — is not where your users tune your app. It's
where the operator grants what your app is *allowed* to do: shell enabled, may
reserve up to N pixels of work area, these keybindings map to these
session-operation slots, input repeats this fast. Sophia validates it because it
crosses the whole session. Your app configures freely inside the envelope; the
envelope itself is granted, not claimed. Your config file may ask for a 40-pixel
panel, but the shell *requests* 40 through the reservation mechanism and Sophia
caps it at whatever the profile allowed. A user cannot, through your app's
settings, quietly grant your app more of the screen than the operator permitted
— which is the same property that stops your app drawing a phishing prompt.

The existing unified profile also carries a `policy` section as a transport for
WM-owned settings. Sophia preserves those ordered KDL records within the checked
envelope; it does not maintain the WM's setting vocabulary or interpret layouts,
workspace names, gaps, or scratchpad dimensions. The selected WM validates that
fragment before acknowledging activation. Adding a spatial-policy setting in
Hagia therefore needs no Sophia parser update. See [configuration](configuration.md)
for the distinction between envelope checks and WM semantic validation.

**An action is a request, not a thing your app does.** This is the seam every
developer arriving from X11 or Wayland gets wrong. When a user binds a key to
"launch a terminal," your app does not spawn the terminal. It asks Sophia
through an opaque session-operation slot, and the session decides what that slot
does. Your config expresses the intent — this key, that action — but the effect
routes through the authority that owns it. Same for anything that moves data: a
portal decision sits behind it. You express what the user wants; Sophia decides
whether and how it happens.

## The Menu-Export Portal

The per-application menu bar, macOS and Unity style, is the one classic
desktop feature that refuses to decompose into the table above. It needs
application metadata to flow to the shell — the menu tree of the focused
window — and that's precisely what `docs/sophia-policy-ipc.md` forbids the
shell from having.

Sophia's answer is neither to refuse the feature nor to open the boundary.
It's a portal. An application opts in to exporting its menu tree to an
identified shell, through the same brokered, consent-carrying mechanism as a
clipboard paste. An application that doesn't export simply has no global menu
and loses nothing else. The shell renders what was exported and dispatches
selections back as opaque actions.

This inverts the usual design. Every existing global-menu implementation has
the shell read the application: DBusMenu announces, the shell consumes, and
anything on the bus can watch. Here the application publishes to an identified
recipient, or nobody sees anything. It's the difference between a directory
the world can read and a letter with an addressee. And because it's the same
shape as every other portal, it needs no new protocol family.

Status: design direction, not yet specified. It would join the portal set in
`docs/namespaces-and-portals.md` as an eighth transfer kind.

## Why There's No Desktop Protocol

Protocol families here are cut along authority boundaries, never product
categories. "Desktop" is a product category. A `sophia_desktop_v1` would need
surfaces, work-area claims, and activation — everything `sophia_shell_v1`
needs — and the two would drift apart while third parties guessed which one
to implement. The spanning mechanism is capability negotiation inside a
single family, and it's proven: `sophia_wm_v1` carries a trivial tiler and
Hagia's full policy surface on the same frozen wire through optional capabilities.

The open question for the shell family isn't width. It's who gates the width:
whether the session's desktop profile decides which capabilities a shell may
negotiate, the way it already caps panel depth — "the session and not the
shell decides," as `docs/configuration.md` puts it. A locked-down machine
could then run a confined, descriptor-only shell while a workstation runs a
full content shell, with the difference bound into the profile digest the
evidence chain already records.

## Two Kinds of Shell, Named Honestly

Descriptor mode and content mode aren't feature tiers. They're trust tiers.

A descriptor shell can't draw a phishing prompt, because it can't draw. A
content shell chooses what appears in its own surfaces, though it still reads
nothing. Moving up the ladder trades confinement for expressiveness, and the
protocol should keep that trade visible rather than let it blur. That's why
the small confined reference, Narthex, stays maintained even after richer
shells exist: it's the standing proof that the confined tier carries a useful
desktop, not just a demo.

## Verification Culture

Anything that claims conformance can prove it, and nobody has to trust
anybody:

- The protocol corpus — golden frames, malformed frames, fixed records — is
  shared. Sophia's generated codecs and every independent client parse the
  same bytes. Rust, C, and Nim clients already pass it.
- Reference clients live in separate repositories with no Sophia build
  dependency, so a wire change that breaks them is a compatibility break, not
  a refactor.
- Physical proofs on real hardware bind the exact signed commit and binary
  digest of every component into an archived record. Your desktop can adopt
  that machinery or ignore it; the protocols don't care.

`docs/validation.md` owns the details.

## Where to Start

| You want to build | Read next | Copy from |
| --- | --- | --- |
| A window manager | `docs/sophia-wm-api.md`, `protocol/archive/sophia-wm-v1-r3/README.md` | the archived `client.c`, then Hagia |
| A shell | `docs/sophia-shell-v1-direction.md`, `protocol/sophia-shell-v1.kdl` | Narthex |
| A full desktop | this document, then both of the above | Hagia and Narthex, as the split to imitate |
| Portal-using apps | `docs/namespaces-and-portals.md` | — |

The shell interface is the moving part. Revision 1 provides a switcher and
bounded reservations; revision 2 adds persistent WM tab descriptors. Broader
content support is still being derived from a working shell's enumerated needs
plus a classic desktop's. If you're building in this space now, you're early
enough to shape that capability.

### Tabbed WM layouts

The [tabbed-layout protocol](tabbed-layouts.md) is an example of the descriptor
shell tier. A WM commits opaque group membership and bar geometry alongside its
layout projection. Sophia remaps those facts into sanitized, recipient-local
shell descriptors. The shell confirms its candidate through `sophia_shell_v1`;
Sophia renders and presents the bars through its normal GPU composition path.
Neither a private WM–shell channel nor application metadata in the WM is needed.
Richer raster content still requires a separate, explicitly negotiated capability.
