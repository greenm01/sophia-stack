# Installed Sophia Operations

This is the operator runbook for an immutable Sophia release installed below
`/opt/sophia`. The ordinary desktop is the native Hagia WM plus Narthex shell.
Sophia does not package or run an unmodified legacy X11 WM as policy.

## Support Boundary

The retained physical reference is a Void Linux x86-64 host with an AMD Radeon
RX 7900 GRE (`amdgpu`), two DisplayPort outputs, and keyboard and pointer
devices on `seat0`. That identifies the proven combination; it is not a general
hardware promise.

An installed candidate requires:

- a local Linux virtual terminal supplied by greetd;
- an absolute, user-owned `XDG_RUNTIME_DIR`;
- a working libseat provider and readable udev input discovery;
- atomic KMS plus GBM/Mesa, libdrm, libudev, libinput, and libxkbcommon;
- Bubblewrap 0.11.2 or newer at `/usr/bin/bwrap` for Hagia/Narthex;
- Bash, Python 3, GNU core utilities, procps, Kitty, Helium, and xterm.

X11 applications connect to Sophia's X Authority. Hagia connects directly to
`sophia_wm_v1`, and Narthex connects directly to `sophia_shell_v1`. These are
separate supervised protection domains. X Authority is not an X11 policy
environment and does not expose root-window WM authority.

## Installation And Session Entries

From a clean checkout at the commit to promote:

```sh
tools/install_live_session.sh
```

The command builds before requesting privilege, verifies every artifact digest,
installs a new immutable directory below `/opt/sophia/releases`, and atomically
updates `current` while retaining the former release as `previous`. Activation
and rollback validate the complete target surface before changing command links
or greetd entries.

A native-only schema-6 artifact records the Sophia commit and whether Hagia is
included. A Hagia artifact additionally records its signed source commit, the
canonical default-profile digest, and Hagia and Narthex executable digests.
Installation rejects missing, non-executable, or mismatched artifacts. Legacy
WM executables, compatibility configuration, and bridge fields are forbidden.

Every release installs these base entries:

- `Sophia Kitty (Baseline)` — one application, no WM or shell;
- `Sophia Native Chrome Proof` — a bounded Engine-chrome diagnostic.

A release built with explicit `SOPHIA_HAGIA_BIN` and
`SOPHIA_HAGIA_SHELL_BIN` paths also installs:

- `Sophia Hagia (Native Policy)` — the ordinary user-profile session;
- `Sophia Hagia Promotion (Packaged Default)` — immutable release evidence;
- `Sophia Firefox Proof` — the integrated browser workflow;
- `Sophia Recovery Proof` — a bounded watchdog/recovery gate.

The generic `sophia-session` command is an internal launcher and requires an
explicit native profile. It has no legacy-WM profile and no compatibility
fallback. Activating a native-only release removes stale Sophia-managed legacy
entries, but preserves unrelated files or links at the same paths.

## Status And Logs

From the session or an independent text VT:

```sh
sophia-status
```

Status verifies the current release checksums and reports current/previous
targets, relevant processes, lifecycle outcomes, runtime identity, and the
newest native proof attempts.

Durable user evidence lives below
`${XDG_STATE_HOME:-$HOME/.local/state}/sophia/`:

| Evidence | Path |
| --- | --- |
| Hagia session, guard, recovery, lifecycle | `hagia-session/` |
| Kitty fallback session | `kitty-session/` |
| native diagnostics | `native-session/` |
| installed launch and runtime identity | `installed-session/` |
| Hagia attempts and coverage | `promotion/hagia-runs/` |
| packaged-default Hagia attempts | `promotion/hagia-promotion-runs/` |
| Firefox, xterm, and TrueColor attempts | `promotion/{firefox,xterm,truecolor}-runs/` |
| fallback, emergency, watchdog, and native-chrome attempts | `promotion/{fallback,emergency,watchdog,native-chrome}-runs/` |

Active logs retain at most one `.previous` generation. Immutable attempts bind
the Sophia executable, relevant native policy executable, selected profile
identity, lifecycle, recovery, and reduced session evidence. Personal
configuration contents are never copied into an archive. Logs must not contain
typed text, clipboard data, window titles, or application content.

## Normal Stop

Use `Ctrl+Alt+Delete` for ordinary Hagia logout. A customized profile may choose
another nonreserved binding. Normal logout commits the policy action, drains
presentation and application ownership, restores the VT, and returns to greetd.

If the graphical session cannot accept the shortcut, use an independent text
VT as the same user:

```sh
sophia-stop
```

The command signals the outer session wrapper. Do not kill Sophia, Hagia,
Narthex, Kitty, or Firefox individually; that bypasses the owner responsible
for bounded cleanup. `sophia-stop hagia` remains available when an explicit
profile is useful.

## Emergency Recovery And Fallback

The independent input guard is armed before graphics takeover. If both
rendering and routed input are unusable, press and release
`Ctrl+Alt+Backspace`. The guard does not depend on Sophia, Hagia, or Narthex. It
ends the supervised process group, restores keyboard/KD/termios state, and
returns control to greetd.

After greetd returns:

1. Select `Sophia Kitty (Baseline)` to isolate the core display/input path.
2. Exit Kitty normally.
3. Run `sophia-verify-fallback` from a text VT.
4. Inspect `sophia-status` before retrying Hagia.

`Sophia Recovery Proof` exercises the process-external watchdog. Verify its
latest archive with `sophia-verify-watchdog`. An independent emergency chord
from a Hagia session is archived separately and verified with
`sophia-verify-emergency`.

## Rollback

From an independent text VT after the current session has ended:

```sh
sophia-status
sudo sophia-rollback
sophia-status
```

Rollback swaps `current` and `previous`; it does not edit either immutable
release. A second invocation swaps the same pair back.

## Native Evidence Workflows

The ordinary Hagia session automatically reserves an attempt before graphics
takeover. Normal logout records `status=passed`, emergency recovery records
`status=recovered`, unexpected exits record `status=failed`, and interruption
before finalization leaves `status=pending`.

Verify ordinary and packaged-default sessions with:

```sh
sophia-verify-hagia
sophia-verify-hagia-promotion
```

Focused X11-application proofs remain commands rather than desktop policy
profiles:

```sh
sophia-xterm-proof
sophia-verify-xterm-runs 1

sophia-truecolor-proof
sophia-verify-truecolor-runs 1
```

Both run under Hagia/Narthex. The xterm proof covers CPU-backed placement,
work-area reservation, VT switch/resume, page-flip retirement, and clean
logout. The TrueColor proof covers core color requests, CPU and DMA-BUF
composition, independent output readiness, retirement, and exact recovery.

`Sophia Native Chrome Proof` records the ordered ring/frame sequence and is
verified with `sophia-verify-native-chrome`. Firefox proof attempts are checked
with `sophia-verify-firefox-runs`.

There is no installed cycle or two-hour soak gate. Long durability runs are
optional overnight diagnostics and do not block product work. Historical
bridge-era evidence remains in the Git history and roadmap archives; it is not
installed, re-executed, or accepted as current native-policy evidence.

## Known Limitations

- Sophia is a native X11 application-server candidate, not a full Xorg
  replacement. Compatibility is limited to the admitted operations in the X11
  matrix.
- Wayland application protocol support is intentionally absent.
- Only the retained AMD reference has physical daily-driver evidence; other
  drivers, topologies, and architectures remain unproven.
- Applications that require a desktop portal, notification service, or
  accessibility bus are outside the current support boundary.
- Only one previous release is addressable through rollback. Preserve older
  immutable release directories until a successor has clean Hagia/Narthex and
  recovery evidence.
