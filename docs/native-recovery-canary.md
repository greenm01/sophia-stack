# Native recovery canaries

CP-14.3's recovery implementation has deterministic coverage. Physical acceptance
still requires the two runs below. They use the normal Hagia launcher and do not
create, reopen, or alter a desktop-comparison run.

## Prepare once

Build Sophia with `cargo build --offline --release -p sophia-cli --features native-session`.
Use compatible Hagia and Narthex executables, Hagia's normal desktop profile, and
an explicit Engine configuration. Preserve copies of all three executables and
both profiles in a private diagnostic directory. Record their SHA-256 digests,
repository revisions and dirty state, the Sophia patch when applicable, and the
launcher scripts' digests. A binary digest identifies the executable even when
its source checkout has changed since it was built.

Give each run its own `XDG_STATE_HOME`, Firefox profile, and operator notes. The
launcher writes session, input-guard, lifecycle, and recovery logs beneath
`$XDG_STATE_HOME/sophia/hagia-session/`. Copy the shared
`/tmp/sophia-hagia-tty3-launch.log` immediately after each run, before the next
launch overwrites it. Preserve the matching `sophia/tty-handoff.log` too.

Set these existing launcher inputs to the preserved absolute paths:

```sh
export SOPHIA_TTY_PROFILE=hagia SOPHIA_TTY_NUMBER=3
export SOPHIA_BUILD_SESSION=false SOPHIA_SESSION_STARTUP=terminal
export SOPHIA_BIN=/absolute/canary/bin/sophia
export SOPHIA_HAGIA_BIN=/absolute/canary/bin/hagia
export SOPHIA_HAGIA_SHELL_BIN=/absolute/canary/bin/narthex
export SOPHIA_CORE_CONFIG=/absolute/canary/profiles/core.kdl
export SOPHIA_DESKTOP_PROFILE=/absolute/canary/profiles/hagia.kdl
export SOPHIA_HAGIA_BROWSER_BIN=/absolute/path/to/firefox
```

Before takeover, save work in the fallback desktop and confirm a usable TTY and
the existing recovery path. Run only from the selected TTY. The launcher manages
the display-manager handoff, input guard, and recovery; do not bypass it with a
bare compositor invocation. `Ctrl+Alt+Delete` requests normal logout with the
stock Hagia profile. The launcher's printed emergency escape and existing
`tools/stop_sophia_session.sh` remain the recovery paths.

## 1. Return before the deadline

Use a fresh `resume` state/profile directory, a 90,000 ms runtime deadline and a
120-second watchdog. Launch with the existing adapter:

```sh
export XDG_STATE_HOME=/absolute/canary/resume/state
export SOPHIA_SESSION_WATCHDOG_SECONDS=120
mkdir -p /absolute/canary/resume/firefox-profile
tools/start_sophia_tty3.sh --max-runtime-ms=90000 \
  --session-app-arg=browser=--profile \
  --session-app-arg=browser=/absolute/canary/resume/firefox-profile
```

Launch Firefox with `Super+b`, load a local page, and confirm changing content.
Switch to another VT, return well before 90 seconds, type into a terminal, and
confirm the resulting text and Firefox updates appear. Log out normally. Record
whether both outputs and the fallback desktop remain usable.

Require a settled close for the old owner, a new owner epoch, actual retirements
in the new epoch, and clean session quiescence/cleanup. The final native totals
must include both epochs, even if logout follows resume immediately. A resumed
initial modeset is separate from subsequent kernel page-flip retirement.

## 2. Remain away through the deadline

Repeat with a fresh `suspended` state/profile directory, a 60,000 ms runtime
deadline and a 90-second watchdog. After Firefox displays changing content,
switch away and stay away. Observe the retained log from the other TTY. Do not
return merely to make shutdown progress.

Require `reason=runtime_deadline` quiescence to begin and complete while away,
without another native owner opening, a seat resume, or a watchdog recovery.
The final session record must say `native_presentation=enabled` and retain the
retirements from before suspension, with no in-flight or cleanup obligation.
Confirm that terminal input and the fallback desktop work after teardown.

For both runs retain the command, timestamps of operator actions, binary/profile
identity, all logs, exit status, and observations. Require zero pending authority,
coordinator, CPU, and native work in the completed quiescence record, clean
frontend/application cleanup, and no earlier native failure or unsettled owner.
Failure leaves the physical checkbox open and becomes a focused incident; it
does not reset unrelated workflow evidence or restart the 36-row matrix.

## Evidence lifetimes

`sophia_live_native_owner schema=1` records activation (`status=opened`) and
closure (`status=closed`) with a monotonically increasing session-local `epoch`
and reason. Closure reports owner submissions/retirements, failures, remaining
native work, and cumulative `settlement_failures`. An unsuccessful drain stays a
session failure even if a later owner is healthy. `settled=true` describes the
owner's current work gauges; it does not erase a previous failed drain.

The existing session, resource, cursor, page-flip-clock, direct-scanout totals
and cost summaries retain their schemas. Counters sum disjoint owner lifetimes;
maxima take the maximum; live resource gauges describe the current owner and are
zero when there is none. Per-head content, verdict, cursor-path and capability
records describe the final owner. Direct-scanout verdict totals cover the whole
session. Bounded cost samples merge before percentiles are calculated, preserving
saturation; percentiles themselves are never averaged.

CPU progress retains completed history, releases old native frame bindings, and
restarts presentation baselines at each owner boundary. Unbound CPU updates keep
their scene/lifecycle owner. Input-latency samples retain completed observations
and count pending observations abandoned at replacement. Native frame IDs can
repeat, so evidence readers must not join frames across owner closure. The
Firefox rendering verifier now separates those lifetimes; its pixel readback
proof remains an optional, more expensive diagnostic than these two canaries.

No shell, WM, or output wire format changes are part of this fix. Native rendering
keeps its existing GPU defaults. Suspended shutdown consumes final authority work
with presentation unavailable: it skips Present requests and preserves revoked
input projections rather than fabricating a displayed frame.
