# Desktop comparison

This directory owns the immutable inputs and isolated profiles for the
Milestone 14 same-hardware diagnostic. Preparation hashes every descriptor,
profile, workload input, and privileged adapter into the run manifest. Changing
any of them after preparation invalidates the run.

The comparison is not a release race. Reference speed or memory use never
passes or fails Sophia. Verification fails for incomplete or reordered rows,
identity/topology mismatch, crash, sample loss, premature process exit,
truncated resource or kernel timing populations, dirty teardown, partial
capture, or corrupt evidence.

## Acquisition status

Run `cp14` remains preserved but invalid: its first 15 rows predate the
terminal-free visibility contract. The replacement acquisition is implemented
as one typed TTY3 gate. It starts every stack without an operator application,
runs the controller outside the measured supervisor tree, and requires passive
workload-owned focus and visibility on DP-1 before and throughout measurement.
Physical validation and a fresh 36-row run remain outstanding. Do not continue
or cite `cp14`; schema-4 admission rejects it explicitly.

## Pinned matrix

The stacks are:

- the clean, signed Sophia candidate with native Hagia WM and shell policy;
- XLibre source commit `56be9f4320ef121dc5d4bc40a6365d995512d3bc`
  with xmonad `0.18.1` and xmonad-contrib `0.18.2`; and
- `/usr/bin/niri` `26.04`.

Every row uses Kitty `0.48.2`, Firefox `154`, DP-1 at
2560×1440@60, and DP-2 at 1920×1080@60, with the workload on DP-1.
The repository-owned profiles under `profiles/` replace personal Hagia, niri,
and xmonad configuration for the matrix. Animations are disabled and comparison
windows are floating where the reference stack supports a rule.

Four short workloads run three times per stack in rotated order: a visibly
changing Kitty stream, the loopback-only animated Firefox fixture with a fresh
profile and readiness beacon, 120 Kitty resize requests, and a 16-Kitty launch
burst. This required interactive lane is 36 rows. A single Sophia two-hour
Kitty soak is a separate optional durability lane; it neither extends nor
blocks verification of the comparison matrix.

## Local row workflow

Prepare once from the clean signed candidate:

```sh
cargo xtask conformance desktop-comparison prepare RUN
cargo xtask conformance desktop-comparison status RUN
```

An overnight soak uses its own run and the same one-row gate:

```sh
cargo xtask conformance desktop-comparison prepare-soak SOAK_RUN
cargo xtask conformance desktop-comparison gate SOAK_RUN
```

For each row, switch to TTY3, log in, and run one command:

```sh
just desktop-comparison-row RUN
# equivalent:
cargo xtask conformance desktop-comparison gate RUN
```

`gate` revalidates the exact clean prepared commit and builds the release
Sophia candidate before graphical takeover, then verifies the six prepared
stack/policy/shell executable digests. It reads only the next typed row,
launches the matching local terminal-free Sophia, XLibre+xmonad, or niri
session with the repository profile, checks the fixed topology, attests the
actual supervisor, resolves DP-1's active CRTC through DRM, captures and seals
one row, and tears the session down. It never uses SSH. Sophia's adapter may
stop and restore the local display manager because Sophia owns DRM directly.
It activates greetd only after verifying the configured manager VT's input
state; otherwise it returns to the independently verified originating TTY3.
Captured manager state is restored and read back before greetd starts. If exact
kernel round-tripping diverges, recovery records each field and establishes a
verified safe text-console baseline instead. After startup, readiness permits
tuigreet's intentional termios transition but requires stable text display
mode, a non-disabled keyboard mode, readable termios, and a live greeter on the
configured VT before activation.

`attest` derives the stack and version from the exact next schedule row. It
refuses a foreign-UID process, a supervisor executable that cannot implement
that stack, PID reuse, and XLibre without the pinned-prefix identity file. It
publishes one mode-0600 record below
`$XDG_RUNTIME_DIR/sophia-desktop-comparison/`. Automatic attestation accepts
only `RUN SUPERVISOR_PID`; the explicit CRTC form remains diagnostic.
`capture` accepts no caller stack, version, topology, workload, duration, or
output identity.

`preflight` and `capture` prompt through `sudo` only for
`tools/desktop_comparison_tracefs.sh`. Preflight asks that narrow adapter to
confirm the tracepoint before an attempt directory exists, including on hosts
whose tracefs tree is root-private. Capture validates fixed artifact names and
an owner-only attempt directory, creates a private tracefs instance, enables
DRM vblank delivery tracepoints, and cleans the instance on every exit. Kernel
DRM completion timestamps are authoritative. Native stack timing is retained
only as a diagnostic availability record and never replaces missing kernel
evidence.

Kitty workloads ignore personal configuration and keep their remote-control
sockets in a mode-0700 namespace below `$XDG_RUNTIME_DIR`; the owner refuses a
socket path that exceeds Linux's pathname limit before launching Kitty.

The Rust owner launches and tears down the fixed workload, samples the union of
the attested supervisor tree and owned workload trees once per second, records
PSS/RSS/anonymous/private-dirty memory, CPU and fault deltas, processes,
threads, and descriptors, and derives frame and resize distributions. It seals
the row only after raw replay succeeds. Failed captures remain under
`RUN/incoming/`; that intentionally blocks status, capture, verify, and report
until the partial evidence is diagnosed.

The trusted conformance owner correlates X11 `_NET_WM_PID` or niri IPC PIDs
against start-time-bound workload roots, but persists only normalized counts
and placement/focus booleans. No title, class, PID, or application identity
crosses Sophia's blind WM boundary or enters the evidence. Baseline must contain
zero application toplevels; settled and one-second records must contain an
owned, focused, visible DP-1 toplevel and zero foreign application toplevels.

Replay and final reduction do not need the original desktop session:

```sh
cargo xtask conformance desktop-comparison replay RUN ATTEMPT
cargo xtask conformance desktop-comparison verify RUN
cargo xtask conformance desktop-comparison report RUN
```

Each sealed attempt contains exactly six raw inputs, including
`visibility.log`, the derived schema-3 sample, and an internal checksum ledger.
The run ledger separately binds that sample to its schedule path. Report rows
preserve resource/allocation, launch, settle, resize, and kernel-frame
populations and always end in `verdict=none`.

## Profile admission

Use absolute paths when launching sessions:

- Sophia: set `SOPHIA_DESKTOP_PROFILE` to `profiles/hagia.kdl`; do not allow
  the installed launcher to fall back to `~/.config/hagia/config.kdl`.
- niri: launch with `niri --config profiles/niri.kdl` (or the equivalent
  `NIRI_CONFIG` setting). Validate it with
  `niri validate -c profiles/niri.kdl`.
- XLibre+xmonad: build XLibre from the pinned clean source into a dedicated
  prefix, then register that source/prefix and compile the isolated xmonad:

  ```sh
  cargo xtask conformance desktop-comparison install-reference XLIBRE_SOURCE PREFIX
  ```

  Installation refuses a dirty or wrong source revision, a server that identifies
  as X.Org, mismatched xmonad core/contrib libraries, or an existing
  xmonad/identity artifact. GHC object files remain in a temporary directory
  below the prefix and are removed before installation completes. The installer
  writes the XLibre commit, both runtime-library versions, and xmonad-profile
  digest beside the newly compiled executable. Attestation and preflight require
  those sidecars and exactly one owned xmonad.

  The host launcher reports `0.18.1.9`, but that is its executable-package
  version, not the standalone profile's linked runtime. The compiled comparison
  executable must itself report xmonad `0.18.1`.

The host's current `/usr/bin/Xorg` is not evidence for the XLibre row. The
exact XLibre prefix must exist before preparing the physical matrix.
