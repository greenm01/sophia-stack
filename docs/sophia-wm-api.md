# Sophia Window Manager API

**Role:** normative policy-process boundary and compatibility contract.

Sophia Engine has one window-management interface. A Sophia-native window
manager speaks this API directly. A legacy X11 window manager speaks its normal
policy protocol only to the private synthetic X server inside
`sophia-x11-wm-bridge`; the bridge translates that behavior into this same API.
Neither path is an alternate compositor or application authority.

## Ownership

Sophia Engine owns physical input, shortcut matching, committed workspace and
focus state, scene validation, rendering, and scanout. The session owns process
launch, logout, and protocol-specific polite close execution. A WM proposes
bounded policy changes and never receives:

- physical input streams, grabs, or client sockets;
- real XIDs, protocol object IDs, namespaces, titles, classes, PIDs, or paths;
- client pixels, renderer handles, DRM objects, or portal payloads.

The Engine validates every proposal and preserves the last committed layout when
a WM is absent, incompatible, timed out, malformed, or restarting.

## Policy Model

`WM` is the name of the version 5 policy slot. It is not a requirement that a
client behave like a traditional window manager, nor does it make one layout
family part of Sophia's architecture.

The API carries surface capabilities, state, constraints, and geometry. It
does not carry a tiling tree, master-and-stack roles, scrolling columns, or a
global stacking model. A policy client may keep any such structure privately
and return the same bounded Sophia commands. Tiling clients such as xmonad and
qtile, scrolling policies, freeform stacking policies, hybrid layouts, and
single-application sessions therefore share one Engine boundary.

A larger environment uses other boundaries as well. An Xfce-style session, for
example, would not turn its panel, decorations, settings, notifications, and
session services into WM powers. Those parts belong to shell, metadata,
portal, and session interfaces, while spatial policy remains blind.

Version 5's workspace, registered-action, pointer-focus, and chrome-policy models are current,
versioned
contracts. They are not constitutional claims that every later policy model
must have nine workspaces, keyboard-first control, or the same notion of
visibility. A later version may broaden those mechanics without weakening the
ownership rules above.

## Version 5 Session Negotiation

WM API version 5 uses the existing Sophia IPC frame version. It does not change
the framing or the protocol versions of brokers and authorities.

After Engine connects to the supervised WM socket, the WM sends one bounded
`WmHello` containing API version 5, a nonzero policy generation, capability
bits, bounded compositor chrome, and at most 256 binding registrations. Engine
rejects unsupported capabilities, stale or zero generations, invalid chrome,
duplicate chords or
action IDs, invalid modifier masks, zero action IDs, excessive registrations,
and Ctrl-Alt-Backspace. Engine replies with one `WmSessionDescriptor` containing
the configured outputs, nine opaque workspace IDs by default, the active
workspace for every output, and the opaque session actions available to that WM.
No layout or action request is sent before this exchange succeeds.

`WmPolicyUpdate` carries the same bounded binding and chrome policy with a
strictly increasing generation. Engine rejects stale generations and invalid
candidates, and defers an otherwise valid replacement while a shortcut or
modifier remains held. `WmPolicyAck` reports applied, stale, or invalid
outcomes. Engine owns chrome geometry, display-list insertion, damage, render,
and scanout regardless of which accepted style is active.

The supervised socket is multiplexed. Its I/O worker may receive a policy
update while one Engine request is in flight, but it never mutates Engine
state. It forwards the immutable candidate to the owner loop, which applies it
only at a shortcut-idle boundary and returns the acknowledgement through the
worker. The WM may buffer the single bounded in-flight request while awaiting
that acknowledgement and services it only after the generation is applied.
This ordering prevents a response planned with policy that Engine has not yet
accepted. Transport failure requests a supervised WM restart while preserving
the last committed layout.

A restart repeats negotiation, restores the committed workspace/output mapping,
then sends a complete relayout snapshot. Negotiation failure leaves applications
and the last frame alive while supervisor policy decides whether to retry or
remain degraded.

## Registered Actions

A binding contains an opaque action ID, a normalized evdev keycode, and a bounded
modifier mask. Engine matches the physical chord before client routing, emits one
activation on the initial press, ignores repeat presses until release, consumes
the matching release, and exposes only the action ID to the WM.

`WmRequestKind::ActionActivated` carries the action ID plus current output,
workspace, focused surface, and an immutable layout-node snapshot. The WM may
respond with the same transactional layout commands used for manage and relayout
requests.

`WmRequestKind::FocusRequested` carries only the hit-tested opaque surface,
output, and workspace. It is emitted for an unmodified primary press on an
unfocused visible surface. The WM may accept that request with
`FocusSurface`; no raw motion, button payload, protocol handle, namespace, or
application metadata crosses the policy boundary. Engine retains the press
and following pointer events in a bounded ordered handoff until both Engine
focus and frontend protocol focus have committed. Timeout or disappearance
drops the handoff instead of routing it to stale focus.

Session actions are advertised tokens. Application launches carry only a
nonzero `SessionApplicationId`; the session maps that ID to its private
executable registry. A WM may request an advertised token with an optional
opaque target surface. It cannot supply an executable, arguments, environment,
signal, or protocol handle. Initial configured tokens cover applications,
close-focused, and logout. Application names and roles never cross into Engine
or the WM.

## Workspace Model

The initial session creates nine workspaces. Each output displays exactly one
workspace and a workspace is visible on at most one output.

Engine validates and atomically commits:

- activate a workspace on an output;
- swap visible workspaces when the requested workspace is already on another
  output;
- assign or move a surface to a workspace;
- optionally focus a valid visible surface;
- configure and place the visible surfaces.

Activating a hidden workspace replaces the target output's current workspace.
Activating a workspace visible elsewhere swaps the two outputs' workspaces.
Focus follows the target output and falls back to the first focusable visible
surface when the prior focus is no longer visible.

## Legacy X11 WM Profiles

The compatibility bridge is generic, while concrete legacy behavior is selected
by a bounded profile. A profile declares bindings and maps action IDs to either
synthetic policy input or a private Sophia action message.

The bundled xmonad profile preserves familiar focus, layout, workspace,
move-to-workspace, three opaque application slots, close, and logout chords.
Policy-only actions become bounded synthetic events on xmonad's private display.
Workspace and session actions use private bridge messages and emerge as normal
Sophia WM commands. They never execute an application on the synthetic display.
Pointer focus requests become a private synthetic primary-button gesture
against xmonad's opaque synthetic window so its internal stack remains
consistent; the bridge response still contains only a Sophia `SurfaceId`.

The profile supplies generic empty ICCCM/EWMH property data. Metadata-dependent
legacy rules are unsupported by design. Future policy tags require a separate
explicit broker contract and cannot expose raw application metadata.

## Failure Rules

All vectors and strings are bounded before allocation. Unknown action IDs,
unadvertised session tokens, stale surfaces, nonexistent workspaces, duplicate
workspace visibility, invalid geometry, and transaction mismatch reject the
whole proposal. No rejected action falls through to client input. No failed WM
request launches a process, changes focus, or partially mutates workspace state.

## Evidence Levels

Milestone 7 requires identical direct-API and bridge evidence for negotiation,
bindings, focus, layout, workspaces, session actions, restart, and last-layout
preservation. The xmonad QEMU gate uses real client surfaces and virtio input;
internal injection cannot satisfy it.

Milestone 8 adds the normal session launcher, Firefox, the retained X11
application mix, multi-output workspace behavior, and the unattended soak.
Machine-specific DRM/input runs are optional compatibility diagnostics.
