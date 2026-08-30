# Desktop comparison inputs

These files are the repository-owned inputs for the Milestone 14 diagnostic
comparison. `sophia-conformance` hashes them into each prepared run. A result is
invalid if the files change after preparation.

The matrix is diagnostic, not a release race. Relative speed or memory use does
not pass or fail Sophia. Verification fails only for an incomplete matrix,
identity/topology mismatch, crash, sample loss, non-native backend, or corrupt
raw evidence.

The pinned stacks are:

- the clean, signed Sophia candidate with native Hagia WM and shell policy;
- XLibre source commit `56be9f4320ef` with xmonad `0.18.1.9`; and
- `/usr/bin/niri` `26.04`.

Every stack uses Kitty `0.48.2`, Firefox `154`, and the same extended topology:
DP-1 at 2560×1440@60 followed by DP-2 at 1920×1080@60. The hardware manifest
also records the kernel, Mesa, and GPU identities supplied by the operator.

Four short workloads run three times each in rotated stack order: a 60-second
Kitty stream, the local Firefox fixture, an interactive resize sequence, and a
16-Kitty launch burst. The two-hour steady-state soak runs once per stack. Each
native stack adapter must emit one `desktop_comparison_sample schema=1` record;
xtask binds the raw log and its checksum to the prepared schedule.

```sh
cargo xtask conformance desktop-comparison prepare RUN KERNEL MESA GPU
cargo xtask conformance desktop-comparison run RUN SAMPLE_LOG
cargo xtask conformance desktop-comparison verify RUN
cargo xtask conformance desktop-comparison report RUN
```

`run` ingests a completed native-stack adapter log. TTY/display-manager takeover
remains outside Rust because it needs local privilege and recovery traps; sample
identity, scheduling, completeness, checksums, and reduction remain typed Rust.
Every ingestion rechecks previously bound checksums before creating its own
immutable sample path, so a damaged early capture stops the matrix immediately
rather than surviving until the final verification.
