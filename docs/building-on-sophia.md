# Building on Sophia

This document is the map for anyone building a desktop on the Sophia display
server: a minimal tiling window manager in the dwm or niri tradition, a
Noctalia-class shell, or a full desktop environment in the XFCE or COSMIC
tradition. It explains which component owns what, which protocol each piece
speaks, and how the pieces compose. Every section links to the document that
owns the detail; this one owns the shape.

## The One Rule

Sophia does not divide the desktop by feature. It divides it by **who may see
pixels**.

- **Engine** composites the scene, owns every pixel, and is the only process
  that reads them.
- **Policy clients** — the window manager and the shell — decide what happens,
  and draw nothing or draw blind.
- **Portals** move data between confinement domains, one brokered, identified,
  consent-carrying transfer at a time.

Every design question below resolves against that rule. If a feature needs to
read the screen, it belongs to Engine or behind a portal decision. If it needs
application metadata, it is either refused or it becomes a portal an
application opts into. There are no exceptions granted for convenience,
because each exception is exactly what the confinement exists to prevent.

`docs/compositor-graphics.md` states the corollary for rendering, the
Compositing Operator Rule: Engine admits a drawing primitive only when the
client is physically unable to execute it because of pixel blindness. A shell
rasterizes its own widgets; Engine blurs, because blur reads pixels the shell
must never see.

## The Components

A complete desktop is three processes beside Engine, each replaceable
independently:

| Component | Protocol | Reference | May draw? | Sees |
| --- | --- | --- | --- | --- |
| Window manager | `sophia_wm_v1` (r3, frozen) | [Hagia](https://github.com/sophia-org/hagia) | no | geometry, window facts |
| Shell | `sophia_shell_v1` (r1, experimental) | [Narthex](https://github.com/sophia-org/narthex) | not yet; r2 adds blind content | sanitized labels |
| Broker | `sophia_broker_v1` | in-tree | no | redacted descriptors |

The window manager never learns titles, application identities, or pixel
content. The shell never learns surface identities or coordinates — the
conformance evidence records `surface_ids_disclosed=0 coordinates_disclosed=0`
on every run. The broker issues and revokes the opaque action capabilities
that let a shell activate a window it cannot name.

This is the X11 process model — server owns the display, window manager is a
client — with the part X11 never had: the clients are confined. Under X11 any
client can walk the tree; under Wayland any layer-shell client can draw over
your bank window. Here neither is possible, and the macOS comparison is
instructive: macOS also split WindowServer from Dock.app, but left no
sanctioned seam for third-party window management, which is why tiling tools
there must disable system protection. `sophia_wm_v1` is that missing seam.

## Ladder: What You Build For What You Want

**A window manager** (dwm, niri, xmonad tradition). One binary speaking
`sophia_wm_v1`. Negotiate the capability bits you need — the interface carries
eleven, from `bindings` alone up to `launch_placement` — and ignore the rest.
A minimal tiler is a reducer: snapshot in, projection out. You inherit the
session's shell (or none), portals, and compatibility layer for free. Start
from `protocol/archive/sophia-wm-v1-r3/`, which is self-contained: the frozen
spec, a generated C codec, a worked client, and checksums. Hagia is the
full-width reference; `docs/sophia-wm-api.md` and `docs/wm-v1-freeze-surface.md`
own the detail.

**A shell** (bar, launcher, switcher, notifications — the Noctalia class).
One binary speaking `sophia_shell_v1`, launched by the session into its own
protection domain. Revision 1 carries one capability, the descriptor switcher:
you order sanitized entries, Engine renders and captures them, you receive an
opaque activation. Revision 2 is being derived from a real shell's needs
(`docs/sophia-shell-v1-direction.md`) and adds the content path: you rasterize
widgets you own, transfer bounded content-addressed textures, and Engine
composites them. You still cannot read the screen. The bar is not a separate
component — "shell-owned" covers a small status strip and a full panel set
alike (`docs/compositor-graphics.md`).

**A desktop environment** (XFCE, COSMIC, macOS tradition). Not a fourth
protocol. A desktop decomposes into the pieces above plus portals plus
ordinary applications:

| Desktop feature | Where it lives |
| --- | --- |
| Panels, dock, launcher, OSD, lock, tray | shell |
| Workspace/window switcher | shell (descriptor capability) |
| Work-area reservation for panels | shell candidate, session-capped depth |
| Drag and drop, clipboard | portals |
| File handoff, URI open, notifications | portals |
| Screenshot, screen recording | portals |
| Desktop icons | shell surface launching through opaque actions |
| Settings / control centre | ordinary application writing configuration |
| File manager | ordinary application |
| Session save/restore | session authority, not the shell |
| Per-app menu bar | menu-export portal (below) |
| Panel plugin API | the shell's own affair, inside its domain |

The row that is *not* obvious: a third-party plugin ABI is not a Sophia
concern. Plugins run inside the shell's protection domain and share its
authority; a shell that loads plugins is trusting them with everything it has,
and Sophia neither knows nor cares. What Sophia guarantees is that the blast
radius is the shell's capability set, not the desktop.

## The Menu-Export Portal

The per-application menu bar — macOS and Unity style — is the one classic
desktop feature that cannot be decomposed into the table above, because it
requires application metadata to flow to the shell: the menu tree of the
focused window, which is precisely what `docs/sophia-policy-ipc.md` forbids
the shell from having.

Sophia's answer is neither to refuse the feature nor to open the boundary. It
is a portal: an application **opts in** to exporting its menu tree to an
identified shell, through the same brokered, directional, consent-carrying
mechanism as a clipboard paste. An application that does not export simply has
no global menu, and loses nothing else. The shell renders what was exported
and dispatches selections back as opaque actions.

This inverts the usual design. Every existing global-menu implementation has
the shell read the application (DBusMenu announces, the shell consumes, and
anything on the bus can watch). Here the application publishes to an
identified recipient or nobody sees anything. It is the difference between a
directory the world can read and a letter with an addressee — and it is the
same shape as every other portal, which is why it does not need a new
protocol family.

Status: design direction, not yet specified. It would join the portal set in
`docs/namespaces-and-portals.md` as an eighth transfer kind.

## Why Not A Desktop Protocol

Protocol families here are cut along authority boundaries, never product
categories. "Desktop" is a product category: a `sophia_desktop_v1` would need
surfaces, work-area claims, and activation — everything `sophia_shell_v1`
needs — and the two would drift apart while third parties guessed which to
implement. The spanning mechanism is capability negotiation inside one family,
and it is proven: `sophia_wm_v1` carries a trivial tiler and Hagia's full
policy surface on the same frozen wire, eleven bits apart.

The open design question for the shell family is therefore not width but
**who gates the width**: whether the session's desktop profile decides which
capabilities a shell may negotiate, the way it already caps panel depth
("the session and not the shell decides", `docs/configuration.md`). A
locked-down machine could then run a confined descriptor-only shell while a
workstation runs a full content shell, with the difference bound into the
profile digest the evidence chain already records.

## Two Kinds Of Shell, Named Honestly

Descriptor mode and content mode are not feature tiers; they are trust tiers.

A descriptor shell cannot draw a phishing prompt, because it cannot draw. A
content shell chooses what appears in its own surfaces, though it still reads
nothing. Moving up the ladder trades confinement for expressiveness, and the
protocol should keep that trade visible rather than let it blur — which is why
the small confined reference (Narthex) stays maintained even after richer
shells exist. It is the existence proof that the confined tier carries a
useful desktop, not just a demo.

## Verification Culture

Anything claiming conformance can prove it without trust:

- The protocol corpus (golden frames, malformed frames, fixed records) is
  shared: Sophia's generated codecs and every independent client parse the
  same bytes. Three shell clients — Rust, C, Nim — already pass it.
- Reference clients live in separate repositories with no Sophia build
  dependency, so a wire change that breaks them is a compatibility break, not
  a refactor.
- Physical proofs on real hardware bind the exact signed commit and binary
  digest of every component into an archived record. Your desktop can do the
  same or ignore the machinery entirely; the protocols do not care.

`docs/validation.md` owns the detail.

## Where To Start

| You want to build | Read next | Copy from |
| --- | --- | --- |
| A window manager | `docs/sophia-wm-api.md`, `protocol/archive/sophia-wm-v1-r3/README.md` | the archived `client.c`, then Hagia |
| A shell | `docs/sophia-shell-v1-direction.md`, `protocol/sophia-shell-v1.kdl` | Narthex |
| A full desktop | this document, then both of the above | Hagia + Narthex as the split to imitate |
| Portal-using apps | `docs/namespaces-and-portals.md` | — |

The shell interface is the moving part: revision 1 is a switcher, revision 2
is being derived from a working shell's enumerated needs plus a classic
desktop's, so that the interface is neither too narrow to carry a desktop nor
so wide it re-exposes what confinement removed. If you are building in this
space now, you are early enough to shape it.
