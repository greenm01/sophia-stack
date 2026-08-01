# Sophia Stack

Sophia is a modern, atomic X11 desktop system. It uses a protocol-neutral, authority-separated visual engine to retain X11's highly flexible application model while replacing its implicit presentation and ambient trust with strict visual authority.

Linux graphics stacks have historically required choosing between X11's open, scriptable environment and Wayland's secure, tear-free rendering. X11 offers a shared property tree where any script can manipulate the desktop, but it suffers from tearing and assumes all clients are trustworthy. Wayland secures the desktop and enforces atomic buffer swaps, but its restrictive protocol often stifles customization and rapid development.

Sophia provides an alternative. It preserves the inspectable, shared-X profile that power users value while adopting a modern rendering architecture underneath.

## Architecture

Sophia is separated by authority, not by convenience. 

- **Sophia Engine:** The absolute visual authority. It manages physical input, visual state, frame scheduling, transaction commits, rendering, and display output.
- **Sophia X Server Frontend:** A clean, modern X11 frontend. It presents the established X11 API, translates protocol state into Sophia surface transactions, and performs X11 delivery rules. It does not control layout or scanout.
- **Sophia WM (Window Manager):** A dedicated policy process handling layout, focus, keybindings, workspaces, and launch decisions. It operates entirely on opaque layout nodes and `SurfaceId` handles.
- **Sophia Portals:** Mechanisms for deliberate cross-namespace transfers, such as clipboard sharing, drag-and-drop, and screen capture.
- **Metadata Broker and Chrome:** Translates protocol metadata into redacted compositor UI without exposing namespaces to the window manager.

```text
================================================================================
                         HARDWARE AND KERNEL
================================================================================
 [ physical input devices ]                                  [ display output ]
            │                                                        ▲
            │ raw input via libinput                                 │ DRM/KMS
            ▼                                                        │

================================================================================
                    SOPHIA ENGINE: COMPOSITOR AUTHORITY
================================================================================
 ┌────────────────────────────────────────────────────────────────────────────┐
 │ Scene graph | spatial hit-testing | damage tracking | frame scheduling     │
 │ Atomic visual commits | rendering | scanout                                │
 └───────────────┬───────────────────┬────────────────────┬───────────────────┘
          ▲      │                   │                    │      ▲
          │      │ opaque snapshots  │ portal events      │      │ chrome data
          │      ▼                   ▼                    ▼      │
 ┌───────────────┐        ┌────────────────┐       ┌─────────────────────────┐
 │  SOPHIA WM    │        │ SOPHIA PORTALS │       │ METADATA BROKER/CHROME  │
 │ blind policy  │        │ allow/deny     │       │ redacted UI only        │
 │ layout/focus  │        │ handoff/revoke │       │ labels/icons/badges     │
 └───────┬───────┘        └────────┬───────┘       └────────────┬────────────┘
         │                         │                            ▲
         │ layout proposals        │ portal commands            │ sanitized
         ▼                         ▼                            │ metadata

================================================================================
                         PROTOCOL AUTHORITY LAYER
================================================================================
 ┌────────────────────────────────────────────────────────────────────────────┐
 │ Sophia X Server Frontend: X11 resources, selections, grabs, protocol checks │
 └────────────────────────────────┬───────────────────────────────────────────┘
                                  │
                                  │ namespace-checked surface transactions
                                  │ routed input / configure / lifecycle
                                  ▲

================================================================================
                         SANDBOXED CLIENT NAMESPACES
================================================================================
 ┌────────────────────────────────────┐     ┌─────────────────────────────────┐
 │ Namespace A: trusted               │     │ Namespace B: untrusted          │
 │ X terminal | trusted local tools   │  X  │ X browser | untrusted X app     │
 └────────────────────────────────────┘     └─────────────────────────────────┘
```

## Core Principles

### Visual Authority

The Sophia Engine dictates the pixels. It enforces a simple rule: no new geometry appears on the screen without matching, committed pixels. If an application hangs during a resize, Sophia maintains the last successfully rendered layout. Slow or misbehaving clients will not tear the desktop or expose black borders.

### Opaque Window Management

Layout policy belongs in an external process. Because the window manager sits outside the rendering hot path, it can crash, restart, or be rewritten without taking down the session. To maintain security, the window manager remains intentionally blind to client identity. It receives opaque layout nodes and never sees an XID, a window title, a namespace, or a clipboard payload. 

### Secure Namespaces

Sophia assumes clients are untrusted and places them in isolated namespaces. A classic shared-X profile can be used to run trusted applications in a single namespace, preserving the traditional X11 object model. However, an untrusted application cannot inspect or send events to a trusted namespace without an explicit, user-granted portal handoff. Cross-namespace lookups fail closed by default.

## Documentation

- `docs/README.md` — Maps normative contracts, subsystem status, evidence, and historical material.
- `docs/specification.md` — The proposed project constitution, including invariants, hard non-goals, and amendment rules.
- `docs/architecture.md` — Maps processes and load-bearing boundaries.
- `docs/namespaces-and-portals.md` — Defines admission, isolation profiles, capabilities, grants, and cross-namespace failure behavior.
- `docs/dod.md` — Defines Sophia's data-oriented design rules.
- `docs/sophia-x-authority.md` — Defines the long-term Sophia X Server Frontend and its X11 compatibility boundary.
- `docs/x11-compatibility-matrix.md` — Records the real-client evidence that admits each native X11 compatibility slice.
- `docs/style-guide.md` — Records implementation discipline.
- `docs/research-log.md` — Captures active decisions and research questions.
- `docs/research-log-archive.md` — Preserves completed research and validation evidence.
- `todo.md` — Tracks only active milestones and measurable exits.

## Status

Sophia is a research prototype. The native Sophia X Server Frontend is the designated product path; no other application protocol is currently supported. 

Completed milestones feature paired classic/confined namespace evidence, Engine-owned CPU and DMA-BUF composition, physical input, multi-output KMS, portal-mediated selections, and an unattended xmonad daily-driver session.

Standard DRI3 1.2 now carries FD-bearing `Open`, modifier-bearing multi-plane pixmaps, xshmfences, and Present transactions through the native frontend. The persistent renderer imports those typed resources, gates acquire fences, composes DMA-BUF and CPU layers, and applies the prepared Engine state only after matching native page-flip feedback. The offline suite covers complete-before-Idle delivery, idle-fence triggering, rejection preservation, and exact teardown. The paired software and CPU-plus-`vkcube` X13 gate now passes with controlled acquire delay, rejection recovery, mixed page flips, idle-fence delivery, and exact teardown.

The namespace-keyed X resource model, explicit classic/confined live launch profiles, per-client revocation, portal request/grant lifecycle, and owner-only broker IPC are implemented. The bounded cross-namespace enforcement matrix and authority-private native clipboard flows are proven for targeted text transfers.

The former Smithay-backed Wayland frontend is retired under `research/wayland`. It proved that the Engine boundary is not inherently X-shaped, but it is not a production dependency or promise of future compatibility. A future translator must justify itself from product evidence and reduce to Sophia's existing authority model.

Similarly, XLibre is not a production dependency. Its frozen source and prototype evidence reside under `research/xlibre`. Sophia may reconsider an optional provider only if measured native-X gaps justify the authority and maintenance cost.

## License

Sophia is licensed under the BSD 3-Clause License. See `LICENSE`.
