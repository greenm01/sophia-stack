# Installed Sophia Operations

This is the operator runbook for an immutable Sophia release installed below
`/opt/sophia`. It describes the current Milestone 11 candidate, not the
checkout-based development launchers.

## Support Boundary

The retained physical reference is a Void Linux x86-64 host with an AMD Radeon
RX 7900 GRE (`amdgpu`), two connected DisplayPort outputs at 2560 by 1440 and
1920 by 1080, and keyboard and pointer devices on `seat0`. The latest retained
runtime identity when this runbook was written used Linux 6.18.40, Mesa 26.1.5,
Kitty 0.48.0, and Firefox 153.0.1. Those versions identify the proven
combination; they are not general compatibility promises or version floors.

An installed candidate requires:

- A local Linux virtual terminal supplied by greetd. The validated tuigreet
  configuration reads `/usr/share/wayland-sessions`.
- An absolute, existing, user-owned `XDG_RUNTIME_DIR`.
- A working libseat provider for `seat0`; the validated Void host uses
  elogind. Sophia obtains DRM and input device leases through libseat.
- A primary DRM card that admits universal planes and atomic modesetting, plus
  two connected outputs for the current installed-cycle promotion gate.
- Readable udev input discovery with at least one keyboard. The complete
  daily-driver proof also requires a pointer.
- The runtime libraries used by the packaged binary: libdrm, GBM/Mesa,
  libseat, libudev, libinput, and libxkbcommon.
- Bash, Python 3, GNU core utilities, procps, and Kitty on `PATH`. Firefox is
  optional for a basic login but required for the daily-driver and soak gates.
  Xmonad is frozen inside the release. Xmobar is optional.

The QEMU virtio-gpu gates prove deterministic protocol and lifecycle semantics;
they do not extend the physical hardware support claim.

## Installation And Session Entries

From a clean checkout at the commit to promote, package and install once:

```sh
tools/install_live_session.sh
```

The command builds before requesting privilege, verifies every artifact
digest, installs a new immutable directory below `/opt/sophia/releases`, and
atomically updates `current` while retaining the former release as `previous`.
It installs four greetd entries:

- `Sophia xmonad (Experimental)` is the ordinary candidate.
- `Sophia Kitty (Baseline)` is the known-good reduced fallback.
- `Sophia Firefox Proof` runs the integrated browser evidence workflow.
- `Sophia Recovery Proof` adds a process-external 45-second watchdog. It is an
  evidence gate, not the ordinary desktop.

## Status And Logs

From the session, an independent text VT, or SSH as the same user, run:

```sh
sophia-status
```

Status verifies the current release checksums and prints the current and
previous targets, relevant processes, the latest lifecycle outcome, the
runtime identity, and the newest installed-cycle attempt. An `OK` line for
every packaged file is expected. Investigate any checksum failure before
launching or rolling back.

The durable user evidence is stored below `${XDG_STATE_HOME:-$HOME/.local/state}`:

| Evidence | Path |
| --- | --- |
| xmonad session, guard, recovery, lifecycle | `sophia/xmonad-session/` |
| fallback session, guard, recovery, lifecycle | `sophia/kitty-session/` |
| installed launch and runtime identity | `sophia/installed-session/` |
| automatic normal-cycle attempts | `sophia/promotion/runs/` |
| emergency recovery archives | `sophia/promotion/emergency-runs/` |
| watchdog recovery archives | `sophia/promotion/watchdog-runs/` |
| Firefox proof archives | `sophia/promotion/firefox-runs/` |

The active files have a `.previous` sibling after rotation where applicable.
The logs contain reduced state and identity evidence; they must not contain
typed text, clipboard data, window titles, or application content.

## Normal Stop

Use `Super+Shift+Q` for an ordinary logout. It commits the WM logout action,
drains presentation and application ownership, restores the VT, and returns to
greetd.

If the graphical session cannot accept the shortcut, log in as the same user
on an independent text VT or through SSH and run:

```sh
sophia-stop
```

The command signals the outer session wrapper, which performs bounded child
cleanup and restores terminal state. Do not kill Sophia, xmonad, Kitty, or
Firefox individually; that bypasses the owner responsible for cleanup and
weakens the retained evidence.

## Emergency Recovery And Fallback

The independent input guard is armed before graphics takeover. If both Sophia
rendering and routed input are unusable, press and release
`Ctrl+Alt+Backspace`. The guard does not depend on the X server or WM. It ends
the supervised process group, restores keyboard, KD, and termios state, and
returns control to greetd. Use `sophia-stop` from another VT when the chord is
unavailable.

After greetd returns:

1. Select `Sophia Kitty (Baseline)` to distinguish an integrated xmonad or
   Firefox failure from the core display, input, and recovery path.
2. Exit Kitty normally to return to greetd.
3. Run `sophia-status` from a text VT and inspect the xmonad or Kitty lifecycle
   and recovery lines before retrying.

For a release recovery gate, select `Sophia Recovery Proof`, arm the local
guard when prompted, and leave the session running. The external watchdog must
return to greetd after 45 seconds. Archive that completed proof with:

```sh
sophia-record-watchdog-run
```

An ordinary `Ctrl+Alt+Backspace` recovery is separate and is archived with:

```sh
sophia-record-emergency-run
```

## Rollback

Rollback requires both `/opt/sophia/current` and `/opt/sophia/previous`. From
an independent text VT after the current session has ended, run:

```sh
sophia-status
sudo sophia-rollback
sophia-status
```

Rollback atomically swaps the two symlinks; it does not edit or delete either
immutable release. Confirm the reported commit after the swap, then select the
ordinary session or `Sophia Kitty (Baseline)` in greetd. Do not repeat rollback
blindly: a second invocation swaps the same releases back.

## Installed Evidence

Every ordinary xmonad launch automatically reserves a numbered attempt before
graphics takeover. Normal handoff finalizes it as passed or failed; a wrapper
crash leaves it pending. Failed and pending attempts intentionally interrupt
the consecutive-cycle gate. No recording command is needed after an ordinary
login.

Verify the latest three consecutive attempts with:

```sh
sophia-verify-cycles 3
```

The gate rechecks release and evidence digests, launch uniqueness, runtime
identity, two-output startup, page-flip retirement, normal logout, protocol and
session health, application cleanup, guard state, VT restoration, and the
display-manager handoff. The longer Milestone 12 gates use the same ledger with
`sophia-verify-cycles 10` and the documented soak verifier in `validation.md`.

## Known Limitations

- Sophia is a native X11 research candidate, not a full Xorg replacement.
  Compatibility is limited to the admitted clients and operations in the X11
  compatibility matrix.
- Wayland application protocol support is intentionally absent.
- Only the AMD two-output reference above has retained physical daily-driver
  evidence. Other drivers, output topologies, and architectures are unproven.
- The installed xmonad profile disables the session D-Bus address and desktop
  portal activation. Applications that require a desktop session bus, portal,
  notification service, or accessibility bus are outside the current support
  boundary. PipeWire is not required by Sophia's display path.
- Direct scanout, hardware cursor-plane composition, shared multi-output GPU
  workers, buffer-age optimization, and VRR activation remain Milestone 13
  work. The validated outputs reported no usable VRR capability.
- The fallback proves the reduced Kitty display/input path; it does not prove
  xmonad, Firefox, clipboard, or portal behavior.
- Only one previous release is retained by the rollback interface. Keep the
  immutable release directories until the promoted build has passed its login,
  recovery, fallback, and soak gates.
