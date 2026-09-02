# Development Tooling

**Role:** normative repository-tooling and production-boundary contract.

This document defines which layer owns developer convenience, deterministic
checks, conformance logic, production session behavior, and presentation. It
applies the architecture, style-guide, DRY, and data-oriented-design rules to
the repository itself.

## Dependency Direction

```text
human ──► just ──► cargo xtask ──► sophia-conformance / repository checks
CI ─────────────────► cargo xtask ──► sophia-conformance / repository checks

installed launcher ──► sophia CLI ──► sophia-session ──► runtime / Engine / backends
```

The arrows do not reverse:

- production crates, installed launchers, and repository scripts do not depend
  on `just`;
- production crates do not depend on `xtask` or `sophia-conformance`;
- `just` recipes contain aliases, defaults, and short human guidance, not
  workflow logic;
- shell scripts may remain as installed compatibility adapters or hardware
  takeover boundaries, but new deterministic orchestration belongs in Rust.

## Owners

| Layer | Owns | Must not own |
| --- | --- | --- |
| `justfile` | Optional memorable human aliases | Validation, parsing, archive schemas, production behavior |
| `xtask` | Canonical developer/CI command parsing, process orchestration, and presentation | Production session lifecycle or protocol authority |
| `sophia-conformance` | Typed profiles, evidence parsing, archive identity, and passive gate results | Installed runtime behavior or stdout/stderr |
| `sophia-session` | Production session lifecycle, supervision, recovery, and adapters around Engine | CLI presentation or development-only conformance policy |
| `sophia-cli` | Installed command selection and concrete stdout/stderr ownership | Session state machines or duplicate domain helpers |
| shell adapters | Necessary OS/TTY/installed-format compatibility | A second implementation of typed workflow logic |

`sophia-session` reports exact evidence through host-installed line callbacks.
The library never prints directly. The `sophia` binary installs stdout and
stderr callbacks, preserving the existing evidence schema while keeping
presentation at the binary boundary.

## Canonical Commands

Use these from documentation, CI, and new scripts:

```sh
cargo xtask check
cargo xtask check layout
cargo xtask profile check
cargo xtask profile args --profile=standalone
cargo xtask conformance verify direct-scanout-standalone LOG
cargo xtask conformance verify direct-scanout-overlay LOG
cargo xtask conformance verify direct-scanout-cost LOG
cargo xtask conformance verify direct-scanout-cursor LOG
cargo xtask conformance verify direct-scanout-archive [RUN]
cargo xtask conformance run direct-scanout WIDTH HEIGHT HOLD WORKLOAD [PROOF]
cargo xtask conformance gate direct-scanout [PROOF]
cargo xtask conformance desktop-comparison install-reference XLIBRE_SOURCE PREFIX
cargo xtask conformance desktop-comparison prepare RUN
cargo xtask conformance desktop-comparison status RUN
cargo xtask conformance desktop-comparison attest RUN SUPERVISOR_PID CRTC
cargo xtask conformance desktop-comparison preflight RUN
cargo xtask conformance desktop-comparison capture RUN
cargo xtask conformance desktop-comparison verify RUN
cargo xtask conformance desktop-comparison report RUN
sophia session run [OPTIONS]
sophia session input-guard [OPTIONS]
```

`session-args`, `check-profiles`, `verify direct-scanout`,
`sophia-live-session`, and `sophia-session-input-guard` remain compatibility
aliases. They are not the spelling for new code.

`PROOF` selects what a probe run exercises beyond ordinary direct scanout:
`--overlay-proof` opens an overlay over a directly scanned frame and proves the
return to composition, `--cost` measures direct against composed frames in one
session, `--cursor` sweeps the hardware cursor, and `--atomic-cursor` asserts
the default atomic path rather than selecting it. Each has a matching
`verify` spelling above.

The desktop comparison is a diagnostic 39-sample matrix, not a relative
release gate. Its typed conformance owner requires a clean signed candidate,
pins and hashes configuration plus hardware/software identities, rotates stack
order across three 60-second repetitions, owns workload/process/resource
lifetime, replays kernel-DRM and workload populations, binds every sealed raw
attempt by checksum, and requires one two-hour soak per stack. The operator
selects the named local greetd session explicitly. The code neither switches
display managers nor contacts another host; tracefs privilege is isolated in
one fixed-name, user-directory-validating shell adapter.

`just --list` exposes the small human-facing subset. CI and scripts invoke
`cargo xtask` directly so correctness never depends on a convenience runner.
Installed sessions invoke `sophia` directly.

## Check Contract

`cargo xtask check` is the canonical offline, non-hardware repository gate. It
runs formatting, diff hygiene, offline metadata, workspace tests, workspace
Clippy, typed profile validation, the exact source-layout debt check, the
evidence-reader schema guard, promoted-archive re-verification, and the active
verifier mutation suites.

`tools/check_live_record_schema_readers.sh` refuses a reader that can match only
schemas older than the one its emitter writes. A record that gains a field and
leaves its readers behind fails nothing on its own: the reader finds no line and
skips the rule it owned, so the run passes with fewer assertions than it appears
to. The guard names its records explicitly, because a record name does not
identify a message -- `sophia_live_wm` writes one schema for `status=ready` and
another for `status=session_action_committed` -- so guarding a record means
having checked that its emitters agree.

One step in the graph needs real hardware and is reported rather than skipped.
`tools/check_buffer_age_equivalence.sh` proves a damage-limited render
byte-identical to a full one on this host's GPU, through a render node only. It
exits 2 where no render node is writable, which the gate reports by name: a
question that was never asked is neither a pass nor a failure, and treating it
as either is how an unreferenced proof rots.

`cargo xtask check layout` compares normalized audit identities with
`docs/source-layout-debt.txt`. That file is not an exception list: every entry
still fails `tools/audit_source_layout.sh`. Exact identities prevent a new
violation from hiding behind an unchanged numeric count and make retirement
visible as a reviewed path change.

Hardware gates remain explicit because they require a real TTY, DRM ownership,
and operator authorization. Their argument parsing, evidence verification, and
archive logic belong in `sophia-conformance`; the minimal TTY takeover adapter
remains transitional shell until production session startup owns that boundary.

## Definition Of Done

A tooling or infrastructure change is complete only when:

- there is one canonical implementation of each parser, schema, verifier, and
  archive operation;
- reusable logic returns typed data or errors and does not print;
- the binary layer owns presentation;
- tests live with the crate that owns the behavior and outside production
  source where visibility permits;
- installed artifacts do not acquire development-only dependencies;
- compatibility aliases delegate to the canonical path;
- the offline check graph and relevant mutation suites pass;
- architecture, the active roadmap, and the dated research log agree.

Current admitted debt is enumerated exactly in `docs/source-layout-debt.txt`.
The next infrastructure retirement slice moves the remaining session test
modules out of `src`, splits the named oversized cohesive units without
changing authority, and replaces the transitional TTY launcher with a minimal
OS adapter around the production session entry point.
