# Sophia Stack

Sophia is a modern, atomic X11 desktop system. Its protocol-neutral visual engine keeps X11's flexible application model while replacing ambient trust and implicit presentation with explicit authority boundaries.

One way to describe the design is a constitutional cathedral with bazaar edges. The core is planned as one coherent system, but no component has unchecked power. The Engine controls pixels and physical input, the protocol frontend applies X11 rules, and the window manager chooses layout policy. Their interfaces define and limit those roles.

Around that core, window managers, shells, protocol frontends, and portal policies can be replaced or developed independently. The core enforces safety and presentation rules; components at the edges decide how the desktop behaves.

Sophia's current product path is native X11. It preserves a classic shared-X profile for trusted applications and adds isolated namespaces for clients that should not share authority.

## Architecture

Sophia divides its systems based on what each part is permitted to control, rather than what is easiest to write.

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

For detailed design specifications, architectural guides, security policies, and research logs, see [docs/README.md](docs/README.md).

## Status

Sophia is a research prototype. The native Sophia X Server Frontend is the designated product path, and no other application protocol is currently supported. However, the transaction boundary is protocol-neutral, allowing future translation layers or native interfaces—such as Wayland—to be added without importing another protocol's desktop architecture.

## License

Sophia is licensed under the BSD 3-Clause License. See `LICENSE`.
