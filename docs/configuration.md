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

An external WM does not consume `wm.kdl`. It keeps its native configuration,
such as `xmonad.hs`. Its compatibility bridge still crosses the same blind,
versioned Sophia WM API. A WM never overrides or mutates `config.kdl`.

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

Both files require `schema 1`. Parsers reject unknown nodes and properties,
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
still validates its geometry and renders/damages the chrome. The
`config.kdl` style is the fallback when the WM is absent, invalid, or
degraded.

WM API version 5 carries a generation and bounded chrome policy during
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

The launcher uses a private runtime `wm.kdl`; it does not modify the user's
default configuration. It atomically advances focused-border thickness from
2 to 6, submits an invalid edit, deletes the file, and recreates it at
thickness 4. The border must retain its last-known-good value during the
invalid and missing-file phases. Open an interactive Kitty with `Super+Enter`
during each phase and use `Super+Shift+Q` for normal logout after the final
4-pixel border appears. The launcher prints the exact session and phase-log
paths before takeover. This runner covers the native-WM live-policy portion;
core pending-restart and external-WM isolation still require their separate
physical evidence before the roadmap gate closes.
