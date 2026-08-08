# Sophia and Wayland: Architectural Comparison

**Role:** informative architecture rationale.

**Status:** current as of 2026-08-08; non-normative.

This document compares boundaries and failure models. It is not a claim that
every Wayland compositor has one design, or that Sophia's target shell contract
is already implemented. When this document conflicts with
[Architecture](architecture.md), [Target-Resolved Input](target-resolved-input.md),
or a protocol specification, those sources win.

Wayland and Sophia start from some of the same lessons: the display authority
must own physical input routing, ordinary applications must not observe other
clients' input, and rendering should be client-side rather than an X11-style
server-side drawing API. They place different responsibilities at their public
boundaries.

## 1. What is being compared

Wayland is a protocol between clients and a compositor. Its core protocol does
not require the compositor, window-management policy, shell, renderer, and
session supervisor to live in one executable. Real implementations vary:
GNOME Shell integrates several roles, wlroots-based systems often use separate
bars and launchers, and other compositors choose other boundaries. “Wayland is
monolithic” is therefore not a protocol fact.

Sophia is a project architecture as well as a set of protocols. Its normative
design assigns separate ownership to:

- Engine, which owns visual truth, presentation, physical input, and security
  transitions;
- the X Server Frontend, which owns X11-local focus, grabs, event masks, XKB,
  XI2, and client delivery for the current application path;
- a metadata-blind WM policy process, which answers reduced spatial-policy
  requests; and
- a future metadata-bearing shell process, whose target-resolved input
  contract is ratified but remains pre-schema and unimplemented.

The meaningful comparison is therefore not “one Wayland process versus three
Sophia processes.” It is the authority that each system makes public, the data
each boundary discloses, and the state transition used when one component
fails.

## 2. Process failure and recovery

A Wayland compositor is the server endpoint for its clients. Losing it normally
disconnects those clients; what the desktop session then terminates, preserves,
or restarts is an implementation and supervisor decision. Core Wayland does not
standardize transparent client migration to a replacement compositor.

Sophia deliberately keeps WM policy outside Engine. The implemented WM
connection is epoch-scoped and bounded, and Engine can retain the last accepted
layout while policy reconnects. This reduces the failure domain of spatial
policy. It does not make Engine or the graphics stack infallible, and the same
claim cannot yet be made for a native Sophia shell because that process and
protocol do not exist.

The intended distinction is:

```text
Wayland protocol loss                 Sophia policy-process loss

client <X> compositor                 applications ── Engine ── pixels remain
        connection ends                               │
                                                      X WM policy
                                                      reconnects by epoch
```

This diagram compares a protocol endpoint failure with a policy-process
failure. It must not be read as claiming that every Wayland plugin or bar runs
inside its compositor, or that an Engine crash preserves a Sophia session.

## 3. Input selection and presentation time

In core Wayland, the compositor selects the receiving surface. Pointer enter
and motion coordinates delivered to an ordinary client are surface-local, not
output-global. Pointer constraints, relative-pointer input, privileged shell
interfaces, and compositor-specific protocols add other semantics under
explicit compositor policy.

Core Wayland does not prescribe that spatial selection be derived from a scene
snapshot retired by the latest physical page flip. A compositor can build a
coherent implementation, but that coupling is not a portable client-visible
Wayland contract.

Sophia makes presentation coupling an architectural rule:

- In the current native-X application path, the installed primary-output
  pointer domain now derives hit-test layers from the immutable output-frame
  snapshot only after an accepted page flip. A committed or submitted move,
  removal, or stacking change cannot become a fresh pointer target early.
- The future shell path resolves stable generational targets against the
  applicable last-presented interaction snapshot.
- Output-local pointer domains and per-output input epochs remain future work;
  the current implementation must not be generalized beyond its primary-output
  coordinate domain.

Presented-state selection reduces visual/input disagreement. It does not prove
perfect transforms, eliminate rounding defects, or guarantee that all input
bugs are impossible.

## 4. Disclosure and privileged desktop roles

Wayland core avoids X11's ambient global input stream. Ordinary clients receive
events selected for their surfaces and do not automatically receive the global
window list. Desktop shells, task switchers, accessibility services, remote
desktop services, and capture tools require additional compositor-authorized
interfaces. For example, `ext-foreign-toplevel-list-v1` exposes toplevel
identity, title, and application ID to clients that the compositor admits to
that interface. Availability and admission policy differ between compositors.

Sophia separates two desktop consumers:

| Consumer | Public data contract | Current status |
| --- | --- | --- |
| Application-facing X frontend | `RoutedInputRequest` with global and surface-local coordinates; namespace/profile isolation and X11-local delivery rules | implemented, with admitted grab and queue hardening debt |
| WM spatial policy | opaque surface identity and reduced geometry/focus operations; no application title/class metadata | implemented `sophia_wm_v1` direction and production transport slices |
| Native shell | coordinate-free discrete actions by default, paced normalized continuous values, independently authorized region-local coordinates | ratified pre-schema contract; unimplemented |

Target-resolved input is data minimization, not “absolute privacy.” An action
reveals the selected target, and a malicious authorized shell could encode
location through dense target partitions unless quotas, geometry limits, and
disclosure budgets are enforced. Those concrete schema bounds remain open.

## 5. Grabs, leases, and security transitions

Wayland compositors retain routing authority while implementing core implicit
grab behavior and any admitted pointer-constraint or relative-pointer
protocols. The exact scope of privileged or compositor-specific mechanisms is
not universal. It is inaccurate to describe core Wayland as giving every grab
an output-global coordinate stream.

Sophia's current application route is different because it serves X11 clients:
`RoutedInputRequest` carries global and local coordinates to the X Server
Frontend, which applies X11/XI2 rules. Frontend-local grabs are not yet fully
visible to Engine. Ordinary motion and release can therefore be re-hit-tested
before the frontend finds the old namespace's grab.

The target architecture replaces that split ownership with Engine-visible,
profile-scoped route leases and ordered frontend release acknowledgement.
Security/session epoch transitions preempt locally and quarantine old queued
input; normal scope exit does not silently transfer ownership to a shell
target. This hierarchy is documented and modeled, but the application lease
control path is not implemented. It is a release blocker for shell coexistence,
not a current advantage over Wayland.

## 6. Protocol evolution and portability

Wayland intentionally keeps its core small. The wider `wayland-protocols`
project distinguishes stable, staging, and unstable protocols, while desktop
projects also maintain private or implementation-specific extensions. This
supports evolution but means a shell or management client is portable only
across compositors that implement and admit the interfaces it needs. Calling
all Wayland extensions unstable, or the ecosystem wholly unstandardized, would
be false.

Sophia instead intends a small set of project-owned, versioned interfaces with
permanent compatibility fixtures. Its Compositing Operator Rule admits an
Engine primitive only when it represents a general composition operation that
a pixel-blind client cannot reproduce at its own boundary. Other visual novelty
remains client-rasterized content.

That policy constrains growth; it does not make a protocol literally immutable
or guarantee a century of compatibility. The shell display-list schema is not
ratified, and DMA-BUF transfer for shell content remains an unproven candidate.
CPU buffers or other future transports cannot be ruled out by this comparison.

## 7. Comparative ledger

| Dimension | Wayland | Sophia |
| --- | --- | --- |
| System definition | protocol plus compositor-specific architecture | normative multi-process project architecture plus protocols |
| Application input disclosure | surface-selected, surface-local core pointer coordinates; optional protocols add capabilities | current X path intentionally carries global and local coordinates inside an isolated profile |
| Spatial policy | compositor implementation choice; no core split mandated | separately supervised, metadata-blind WM contract |
| Shell metadata | privileged interfaces chosen and admitted by compositor | future separately authorized shell/broker roles; schema incomplete |
| Presentation-coupled selection | not prescribed by core protocol | primary native application domain implemented; future shell contract requires applicable presented snapshot |
| Grab ownership | compositor implements core and extension semantics | frontend-local today; Engine-visible route leases are modeled target architecture |
| Extension evolution | core plus stable/staging/unstable and private protocols | project-owned versioned schemas with compatibility gates; shell schema not yet ratified |
| Failure behavior | compositor loss usually disconnects clients; session policy varies | WM policy loss is isolated; Engine/graphics failure and future shell recovery have separate limits |

## 8. Sources and scope

Wayland claims in this document should be checked against the
[Wayland architecture documentation](https://wayland.freedesktop.org/docs/html/ch03.html),
the [core protocol XML](https://gitlab.freedesktop.org/wayland/wayland/-/blob/main/protocol/wayland.xml),
and the
[`wayland-protocols` repository](https://gitlab.freedesktop.org/wayland/wayland-protocols).
Sophia claims are grounded in this repository's normative documents, dated
research log, implementation, and verification gates.

The useful conclusion is narrower than “Sophia beats Wayland.” Sophia is
testing whether explicit authority separation, presentation-coupled input, and
measured disclosure budgets produce a system whose failure and security
properties are easier to state and verify. Wayland remains essential prior art
and a diverse implementation ecosystem. Sophia's comparison is credible only
when current implementation, target architecture, and unproven hypotheses stay
visibly separate.
