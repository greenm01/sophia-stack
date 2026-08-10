# Sophia Configuration

**Role:** normative ownership, discovery, validation, and reload contract.

Sophia uses KDL 2 for user configuration. KDL 1 compatibility parsing,
includes, inheritance, and implicit multi-file merging are deliberately
unsupported.

## Files and ownership

The default user files are:

- `${XDG_CONFIG_HOME:-$HOME/.config}/sophia/config.kdl`
- `${XDG_CONFIG_HOME:-$HOME/.config}/sophia/wm.kdl`

`config.kdl` belongs to the session and Engine/compositor mechanism. It owns
the application registry, startup applications, physical input source, XKB
RMLVO, repeat timing, output policy, namespace profile, external-WM launch
specification, diagnostic policy, and fallback compositor chrome plus hard
chrome limits.

`wm.kdl` belongs only to a Sophia-native WM. It owns opaque action behavior,
bindings, workspace policy, native layout selection, timeout policy, and
active compositor-chrome preference. It cannot change input admission,
outputs, namespaces, executable registry entries, renderer or scanout policy,
or Engine hard limits.

Native layout selection currently accepts:

- `layout "columns"` to divide the active work area among visible nodes and
  request matching client sizes;
- `layout "natural"` to preserve each node's current opaque allocation,
  constrain it to the work area, center it, and issue no policy resize.

`natural` is useful for single-purpose and diagnostic sessions but is not tied
to an application identity. Both layouts consume the same metadata-blind node
snapshot and remain subject to Engine constraint reconciliation and atomic
admission.

An external WM does not consume `wm.kdl`. A generic compatibility profile may
name its own native configuration. Sophia's installed xmonad profile is
stricter: the release contains the checked-in policy executable and rejects
mutable `~/.config/xmonad` or home-source discovery. In either case, the
compatibility bridge crosses the same blind, versioned Sophia WM API, and a WM
never overrides or mutates `config.kdl`.

Hagia sessions additionally use the unified desktop profile at
`${XDG_CONFIG_HOME:-$HOME/.config}/hagia/config.kdl`. An explicit
`--desktop-profile=/absolute/path` wins, followed by the XDG file,
`/etc/hagia/config.kdl`, and the compiled profile. Unlike Sophia's two
authority-local files, this source permits bounded top-level includes: depth
10, 64 files, and one MiB in aggregate, with owner/mode checks, cycle
detection, deterministic expansion, and per-value provenance.

The trusted session coordinator validates and partitions all seven desktop
authorities before constructing the graphical session. It stages owner-only
fragments with one generation and digest in the private policy runtime
directory and gives Hagia only the policy-fragment path. Hagia cannot read or
replace the shell, shortcut, session, input, output, or broker candidates.
The trusted side now also has a pure seven-authority activation reducer ported
from Hagia's exhaustively checked lifecycle model. One generation/digest must
prepare everywhere before activation effects are emitted, and promotion waits
for every matching activation completion. A prepare or activation failure
emits generation-wide rollback while retaining the active profile. Attempted
generations advance monotonically even after rejection, so delayed completion
cannot alias a retry.
An injected startup executor boundary converts each typed effect into its
matching authority call and returns the exact typed completion message. Its
handlers are not populated by production authority transports yet; the seam
exists so filesystem or process work cannot re-enter the reducer implicitly.
A synchronous startup driver now drains that boundary through the complete
prepare and activate barriers. The first failed operation cancels the
remaining phase, dispatches rollback to every authority, and retains the prior
active identity. Even when one rollback fails, every remaining rollback is
attempted and the exact unresolved recovery set is returned as an error.
Each authority now has one shared pure participant model behind that contract.
It admits only a strictly newer generation and a different digest, makes exact
prepare, activate, and rollback retries idempotent, restores the exact previous
identity on rollback, and rejects a same-generation digest mismatch. The last
admitted full key remains bounded participant state after cleanup so an exact
retry cannot be confused with an identity collision. Because the synchronous
driver stops preparation at the first failure but rolls every authority back,
an idle authority consumes an exact unseen rollback key as a no-state
tombstone. That prevents local reuse of a generation it was never asked to
prepare while preserving its active identity.
Shortcut candidates are prepared into bounded typed chords before staging:
at most 256 key or pointer bindings, normalized modifier/key identities, no
duplicate chord, and no reserved Ctrl-Alt-Backspace override. Every target is
explicitly authority-qualified, for example `policy:switch-layout` or
`session:close-window`; pointer bindings cannot invoke session capabilities.
This preparation does not activate the candidate or grant an unavailable
capability.
The desktop-profile form of `config check` runs the same typed shortcut,
session, input, and output preparation used by graphical startup; it performs
no device discovery or activation.
That preparation boundary first verifies that all seven authority candidates
match the profile's exact generation and digest, including authorities whose
payload is consumed by another process. Live-session configuration retains the
resulting immutable typed bundle rather than splitting it into unrelated
copies; the public policy setup reuses its prepared shortcut candidate instead
of reparsing the source profile. The startup loader returns the raw
provenance-bearing profile, exact activation key, and derived bundle together,
so it does not validate and then immediately prepare the same values again.
This is candidate selection only, not authority activation.
An authority-local fragment loader now admits the staged format through the
same absolute-path, regular-file, one-MiB, owner, and mode checks as other
trusted configuration. It rejects symlinks, malformed or duplicate markers,
multiple/crossed authority sections, duplicate settings, and any generation or
digest that differs from the coordinator's exact activation key. The result is
the existing provenance-bearing raw candidate DTO; loading still performs no
activation.
One generic authority-local candidate slot now couples that DTO (or a prepared
typed candidate) to exactly one participant model. The slot is the sole owner
of active, previous-active, and candidate payloads; its pure prepare, activate,
and rollback functions delegate identity changes to the participant reducer.
Exact retries compare semantic payload while ignoring staging-path provenance,
but changed settings at the same key fail closed. Fragment admission or payload
rejection returns without mutating either identity or payload state.
The trusted session authority now has one deterministic preparation seam for
its effective application configuration. It parses explicit CLI application,
argument, startup, and action selections into a bounded immutable overlay,
then applies that overlay and the canonical typed session candidate to a clone
of the trusted application registry. The ordering preserves CLI superiority,
and any unknown, duplicate, ambiguous, or over-limit reference rejects the
clone without changing accepted state. This preparation is retained admission
data; it does not activate the session participant or enable desktop-profile
reload.
Startup now places the canonical typed session payload in the generic
session-owned slot before deriving that effective configuration. The retained
slot is exactly `Prepared`, advertises the profile's activation key, and is
checked against the canonical bundle in tests. It is intentionally not
activated by configuration assembly.
When native scanout is requested, startup additionally projects the already
owned DRM capabilities and reconciles the output candidate before launching a
graphical client. A second pure boundary converts the validated reconciliation
into a stable-`OutputId` activation plan carrying exact requested and rollback
states, the shared generation/digest, and optional startup focus. It rejects
capability drift between projection and planning and exposes no connector,
CRTC, property, or file-descriptor handle. This remains admission only: it
performs no atomic test, output apply, or candidate activation.
The next authority-local boundary is also present as a pure coordinator. It
emits typed test, apply, and rollback effects and accepts only completions for
the exact profile generation and digest in the expected phase. Test rejection
discards the candidate without rollback; an apply failure cannot settle until
rollback succeeds or reports a terminal recovery failure. No executor is wired
to these effects yet, so startup behavior remains unchanged.
The production session does not yet invoke this driver or the participant
transitions. At startup, a future executor may expose authority-local activated
state only because graphical launch remains gated until every authority has
activated the same key; failure rolls all participants back before launch.
That synchronous visibility rule is not a live-reload protocol. Watched
desktop-profile reload remains disabled until cross-authority transports,
durable recovery, and an explicit global visibility barrier populate the
executor handlers; Sophia's existing core and native WM reload behavior is
unchanged.

## Discovery

Each domain resolves exactly one source at startup:

1. an explicit absolute `--config=PATH` or `--wm-config=PATH`;
2. the corresponding XDG user file;
3. `/etc/sophia/config.kdl` or `/etc/sophia/wm.kdl`;
4. the compiled default.

Sophia does not rediscover a different source during reload. Deleting,
renaming, or making the resolved file unreadable retains the last-known-good
snapshot. Recreating that same path makes it eligible again.

File-backed configuration must be an absolute, regular file no larger than
one MiB. On Unix it must be owned by the effective user or root and must not be
group- or world-writable.

## Validation and transactions

Both files require `schema 2`. Schema 1 chrome syntax is rejected with a
targeted migration diagnostic. Parsers reject unknown nodes and properties,
duplicate singleton nodes and properties, type annotations, invalid IDs,
unbounded strings or vectors, invalid cross-references, unsafe paths, and the
reserved Ctrl-Alt-Backspace emergency chord. A SHA-256 digest identifies the
exact accepted bytes.

Reload watches the resolved parent directory, so atomic editor replacement is
supported. Events are quiet-debounced for 100 ms and forced after one second
of continuous change. Parsing and validation produce an immutable candidate
before any live state changes.

Core reload is whole-file atomic:

- application registry changes affect later launches;
- repeat timing applies only after the shortcut/key ledger is idle;
- fallback chrome and diagnostics apply live;
- input source, XKB, outputs, namespace, and external-WM launch changes mark
  the entire candidate `pending_restart`;
- a pending-restart candidate does not partially apply its otherwise-live
  fields.

The active native WM chrome preference wins while that WM is healthy. Engine
still validates its geometry and renders/damages the chrome. An external WM
does not advertise chrome-policy ownership, so it uses `config.kdl`. A failed
or restarting WM retains the last complete visual chrome and geometry until a
replacement policy can relayout atomically.

Chrome has two explicit roles. A focus ring is painted only for the focused
managed surface. A frame may be painted for every managed surface with
focused and unfocused colors. Engine reserves one stable clearance equal to
the maximum enabled width, then derives client content geometry by insetting
the WM's outer allocation. Focus and color changes repaint without resizing.
A clearance change is prepared through the ordinary atomic resize path; the
old complete style and client geometry remain visible until every matching
client buffer is ready.

WM API version 6 carries a generation and bounded focus-ring/frame policy during
negotiation. Versioned policy-update and acknowledgement frames reject stale
generations; Engine's policy reducer defers replacement until shortcut state
is idle. The supervised transport delivers updates without requiring an
existing binding or layout request. Its worker performs socket I/O only; the
Engine owner validates and swaps the immutable policy, then returns the exact
generation acknowledgement. The WM does not service action or layout requests
under the new snapshot until that acknowledgement arrives. A request already
in flight is held across the exchange and answered afterward, so bindings,
action behavior, workspace/layout policy, and active chrome cross one
generation boundary.

## Commands

Validate the discovered files without starting a graphical session:

```sh
cargo run --offline -q -p sophia-cli -- config check
cargo run --offline -q -p sophia-cli -- config check --wm
cargo run --offline -q -p sophia-cli -- config check \
    --desktop-profile=/absolute/path/to/hagia/config.kdl
```

Inspect the parsed, default-expanded snapshots:

```sh
cargo run --offline -q -p sophia-cli -- config print-effective
cargo run --offline -q -p sophia-cli -- config print-effective --wm
```

Use `--config=/absolute/path` for the core domain and
`--wm-config=/absolute/path --wm` for the native-WM domain. The example files
are [config.kdl](../examples/config.kdl) and [wm.kdl](../examples/wm.kdl).

## Guarded native hot-reload proof

From a logged-in TTY 3, run:

```sh
tools/start_sophia_native_hot_reload_tty3.sh
```

For an installed release, select `Sophia Native Chrome Proof` in greetd. It
uses the same sequence without a checkout or build and automatically reserves,
finalizes, and checksums a commit-pinned archive. After normal logout, run
`sophia-verify-native-chrome` from a text session.

The launcher uses a private runtime `wm.kdl`; it does not modify the user's
default configuration. After two Kitty surfaces are visible, it advances a
2-pixel ring to 6 pixels, rejects an invalid edit, rejects deletion while
retaining the last-known-good state, applies a 4-pixel frame-only policy, then
applies a 2-pixel ring with a 6-pixel focused/unfocused frame. Each width
change must cross a matching two-surface resize epoch before the reduced
chrome-set observation can advance. Intermediate retired frames remain visible
for three seconds so the physical proof can be inspected instead of merely
logged.

The external-WM half uses the core domain:

```sh
tools/start_sophia_xmonad_config_reload_tty3.sh
```

It proves that the chrome-blind xmonad bridge uses core fallback chrome, a
live-safe width edit applies atomically, a namespace edit remains wholly
pending restart, an invalid edit retains the active value, and no native-WM
reload record appears. Both launchers retain a commit-bearing ordered sequence
log. Validate their verifier logic without physical hardware with:

```sh
tools/check_sophia_native_chrome_verifier.sh
tools/check_sophia_xmonad_config_reload_verifier.sh
```
