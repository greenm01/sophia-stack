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
client physically cannot perform the operation itself. A shell rasterizes its
own widgets. Engine does the blur, because blur reads pixels the shell must
never see.

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
| Shell | `sophia_shell_v1` (r1, experimental) | [Narthex](https://github.com/sophia-org/narthex) | not yet; r2 adds blind content | sanitized labels |
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

## The Ladder

**A window manager**, in the dwm, niri, or xmonad tradition. One binary
speaking `sophia_wm_v1`. The interface carries eleven capability bits, from
`bindings` alone up to `launch_placement`; negotiate the ones you need and
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
protection domain. Revision 1 carries a single capability, the descriptor
switcher: you order sanitized entries, Engine renders and captures them, and
you receive an opaque activation. Revision 2 is being derived from a real
shell's enumerated needs (`docs/sophia-shell-v1-direction.md`) and adds the
content path: you rasterize widgets you own, hand over bounded
content-addressed textures, and Engine composites them. You still can't read
the screen. And the bar isn't a separate component — "shell-owned" covers a
small status strip and a full panel set alike.

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
| Settings and control centre | an ordinary application writing configuration |
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
and Sophia already has one. The desktop profile carries seven typed authority
sections — policy, shell, shortcut, session, input, output, broker — with
digests, validation, and a full prepare–activate–rollback activation
machine, model-checked in TLA+. What's missing is only the front half: a
settings application that writes the profile. The propagation half, the hard
half, exists.

So the honest distance from the WM rung to a full desktop is five items, in
rough dependency order: the content-mode shell (`sophia_shell_v1` r2, the
gating item), a bounded status feed so rich panels have something to show, a
settings application, application-session restore in the session authority,
and a theming story across applications — the one genuinely unsolved item,
since applications are protocol clients and their toolkits theme themselves.

Which yields a sentence no other platform gets to write: on Sophia, a desktop
environment is a superset composition, not a second platform. Moving from
niri-class to KDE-class changes what you ship — never how it's wired — and
the climb is also the trust gradient: a complete confined desktop at the
bottom rung, more expressiveness for more granted trust above it.

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
Hagia's full policy surface on the same frozen wire, eleven bits apart.

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

The shell interface is the moving part. Revision 1 is a switcher. Revision 2
is being derived from a working shell's enumerated needs plus a classic
desktop's, so that it lands neither too narrow to carry a desktop nor so wide
it re-exposes what confinement removed. If you're building in this space now,
you're early enough to shape it.
