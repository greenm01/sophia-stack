# Sophia Active Roadmap

Sophia is a research prototype moving toward a usable native-X daily driver.
This file contains active work, ordering constraints, and promotion gates.
Completed milestones belong in `docs/roadmap-history.md`; detailed decisions,
diagnoses, and retained evidence belong in `docs/research-log.md`.
The normative multi-monitor design lives in
[`docs/multi-monitor-composition.md`](docs/multi-monitor-composition.md); its
"Current And Target State" section and this roadmap's multi-monitor critical
path must be updated together.

Roadmap rules:

- Keep exit criteria measurable and fail closed.
- Expand X11 behavior only from retained real-client evidence.
- Use QEMU for repeatable protocol, policy, transaction, and application
  semantics. Do not substitute it for physical DRM, input-device, VT,
  display-manager, or visible-pixel requirements.
- Keep Engine protocol-neutral and free of application-specific policy.
- Keep the WM blind to XIDs, namespace IDs, titles, classes, PIDs, and client
  payloads.
- Rebuild and re-prove the installed candidate whenever its executable,
  packaged policy, or supervised application set changes.
- Archive a milestone when its complete exit gate passes.

---

## Current Position

Sophia's product path is its native **Sophia X Server Frontend**. Engine owns
physical input, focus authority, scene state, rendering, presentation, and
scanout. X Authority owns X11 protocol semantics and private client resources.
One versioned, protocol-neutral WM API accepts native Sophia policy clients or
legacy-X11 policy translated through a private compatibility bridge. Xmonad is
the first mature bridge profile and current promotion vehicle; it is not
Sophia's architectural WM. XLibre and Wayland prototypes remain under
`research/` as architectural evidence.

The currently retained installed candidate provides:

- guarded two-output startup and exact TTY restoration;
- automatic Kitty, supervised xmonad, and optional unmodified xmobar;
- physical keyboard, pointer, focus, workspace, resize, clipboard, Firefox,
  floating-dialog, and normal-logout workflows;
- Engine-owned KMS presentation, protocol-neutral cursor and input policy,
  native chrome, and retained-frame recovery across VT release; and
- commit-pinned normal, fallback, watchdog, emergency, native-chrome, and
  switch-away/switch-back evidence with exact runtime identity.

Milestones 9 through 12 are complete historical evidence for the xmonad
compatibility profile and are archived in `docs/roadmap-history.md`. Their
bounded lifecycle, recovery, color, work-area, and soak artifacts remain
reproducible regressions, but elapsed wall time is not a current promotion
criterion. Milestone 13's installed product path is complete. Hagia is the
ordinary remembered installed session, records every real session
automatically, and leaves Kitty, xmonad, and the previous immutable release
available for recovery. The retained Triad behavior port is complete;
`sophia_wm_v1` interface major 1, wire revision 3 is frozen; and API v7 plus its
Engine-owned workspace policy are removed. Schema-5 packaged-promotion archive
`0002` binds signed Sophia source `66792329d90d64e26af839dfe494c74d94323c6a`
to Hagia's signed generic default, proves the repaired GLX first-pixel admission
through sustained final-extent presentation, and ends with normal Logout and
clean health. Mutable XDG policy remains confined to the ordinary dogfooding
entry. Milestone 14's three-slot boundary is promoted on signed native archive
`0001`, which also made Hagia the first proof client of the Sophia WM and shell
protocols with no compatibility bridge in the session. Bounded buffer-age damage
history is promoted on signed native archive `0002`. The one-in-flight and
refresh-relative latency row is proved on physical run `20260828T231430Z`
(source `96b00d0d`): full chain p99 24 ms against the two-refresh budget over
two hundred forty-five independent presses with clean stage percentiles.
Native archive `0003` promotes one shared renderer worker per DRM device
group, with both heads of one card on one thread and no result reaching an
output that did not ask for it. Direct-scanout archive `0001` promotes
atomic-test-gated direct scanout for one compatible opaque DMA-BUF layer:
thirty-eight client buffers reached the plane from one validating commit, with
no test rejections, proof disagreements, or fallbacks. Returning a directly
scanned output to composition on effect activation is the active product step,
with the hardware cursor plane behind it.

The current Void host has the required xmonad-configuration build and runtime
dependencies installed. Dependency installation is complete and is not an
active roadmap item.

### Active Critical Path

Work proceeds in this order. Later rows do not justify bypassing an earlier
promotion gate.

1. **Complete.** Make the mixed mirror-plus-extended evidence truthful:
   classify sampling from realized source and target extents, finish the
   public-policy chrome transaction, audit the post-commit topology path, and
   prove head loss and return on a signed candidate. Signed mixed-output source
   `3d19e2e67cfe2e43eb643d219be11a3251fe7176` and signed head-loss/return
   source `66bc0dd71a40e249eb00cd98f6080cf0f6aa9c54` passed their physical gates.
2. **Complete.** Let mirror heads pace independently while preserving
   primary-owned Present completion and last-head buffer retirement, then
   re-run the affected physical gates. Signed source
   `e946cc725bf731515a477c86e9a575554965418c` produced and independently
   re-verified mirror archive `0007` and mixed archive `0001`. Both mirror heads
   paced independently, the primary owned Present completion, the last scanning
   head owned release, and the mixed topology finished with clean health and
   output-local Present submission ownership. Signed successor
   `eeef531a33564391765c1ec9fecddf6d06dcd4cc` independently re-proved the
   complete display slice as mirror archive `0008` and mixed archive `0002`.
3. **Complete.** Host the metadata-reduction chain
   through broker interface revision 1 and enforce Bubblewrap protection
   domains before admitting a metadata-bearing role. The protected transport,
   production Hagia host, executable isolation smoke, and the role socket's
   refusal to admit a metadata-bearing peer without launch evidence all pass
   locally. The physical verifier now requires the real protected broker's
   ready, redacted-descriptor, and clean-stop lifecycle plus signed Sophia and
   Hagia source/binary identities; retain that run after the display gates
   before calling this row promoted. The first run on the row-2 candidate
   stopped before Kitty appeared because its old restart wrapper depended on
   ambient variables and the host `/tmp`, both correctly absent from the
   protected policy domain. The gate now launches Hagia directly. A bounded
   proof flag arms on committed action 66 and asks the session-owned supervisor
   for one replacement only after Hagia atomically replaces its private
   checkpoint; Sophia observes the checkpoint inode, not its contents. The
   replacement candidate reached the real two-output session, then the guide's
   retired `Super+Y` instruction entered the application path because public
   shortcut ownership now binds fullscreen to `Super+Shift+F`. The corrected
   guide also names the public `Super+Shift+B` minimize and `Super+Alt+B`
   restore chords, and a local matcher ties all three to the compiled profile.
   The next run on signed source
   `de0daad96cac0285e56602c0254642f7ba0ed84e` completed the restart, all ten
   requested actions, exact text, 13 ms input-to-page-flip proof, clean broker,
   and clean teardown. Its final native aggregate correctly refused DP-2's
   `nonzero_exports=0`: focus-output actions changed the active output but never
   moved Kitty there. The guide now adds the public move-to-output actions and
   waits for a nonzero DP-2 submission before moving Kitty back. Signed source
   `9ca384a9ffb2e392b584092e64054c2d1f9fc833` and signed Hagia source
   `074e374c537b316b6bdf196ac8f3727004ba6549` produced independently verified
   Hagia archive `0004`. It retained the protected broker, redacted descriptor,
   causal epoch-two restart, all twelve requested action commits, nonzero
   presentation on both outputs, exact 34-event text, clean health, and clean
   teardown. Its Sophia digest matches display archives `0008` and `0002`.
4. **Completed.** Signed Hagia archive `0005` proves the Tier-0 display list,
   per-head raster, fixed work-area reservation, fullscreen coexistence, and
   last-presented captured-input path. Pointer activations selected views 2 and
   1 after the causal restart; all fourteen requested actions committed, both
   outputs presented nonzero content, exact text completed, and health and
   teardown were clean.
5. **Complete.** Build the minimum metadata-backed shell rendering substrate.
   Engine now reduces at most sixteen sanitized descriptors into a title-only
   compositor projection with stable generic rectangle and text nodes,
   per-head lowering, renderer-private cached JetBrains Mono rasters, and exact
   last-presented opaque activation targets. The offline reference gate covers
   unequal heads, damage, cache bounds, cancellation, and authority epochs;
   the 16-entry two-head probe measured 110 us p95 against a 16,667 us budget.
   This is a reference boundary, not a shell protocol or live shortcut.
6. **Complete.** Add the protected shell-role transport and use it for the first
   separately authorized `hagia-shell` descriptor switcher. Model the minimum
   candidate, presentation, target, action, reconnect, and revocation lifecycle
   before freezing records. Keep ordering, selection, and lifecycle in the
   shell; keep validation, rendering, hit-testing, and presentation in Engine.
   The no-reservation path is now live behind an enabled shell profile: the
   session launches `hagia-shell` in its own protected domain,
   `session:window-switcher` requests its candidate from current presented
   policy-managed descriptors, Engine presents and captures it, the broker
   checks the exact issuer tuple, and the WM
   adjudicates the resulting focus request. Disconnect burns interaction and
   reconnects at a fresh epoch while retaining old pixels as inert. Packaging
   requires and hashes the separate executable. Core `GrabPointer` and admitted
   XI pointer grabs now cross a bounded frontend/owner handshake into the same
   Engine lease state as ordinary pointer ownership. Provisional explicit grabs
   route nothing, release is ordered, stale epochs fail closed, and application
   ownership still precedes shell capture. The compiled profile now enables the
   shell and binds `Super+P`; signed installed shell evidence remains the row's
   promotion gate. The latest run reached `Super+B`, then exposed an actor/owner
   mix-up when one classic-shared Helium connection changed another connection's
   surface. Frontend batches now carry a separate authoritative owner route;
   input, control, and metadata admission no longer follow the causing client,
   and exact retired identities reject late route or metadata observations.
   The first signed rerun then exposed an overbroad route domain: passive Kitty
   helper windows entered the public WM snapshot, all resizes timed out, and the
   guide never appeared. Engine batches now retain owner routes only for actual
   transaction or presentation-intent surfaces. Focused helper-window and
   two-client regressions pass. The corrected route run reached Helium's first
   CPU-backed frame, then exposed core dispatch using the client's local X
   sequence as an Engine transaction. Dispatch now carries the listener's
   global transaction through every Engine-visible path, while admission keeps
   rejecting real mismatches and reports the exact failed invariant. Direct and
   two-client regressions pass. The next rerun rendered Helium through several
   generations, then a retained repaint failed with `MissingCpuSource(4)`.
   Retained composition had treated an in-flight renderer image and the same
   surface's CPU authority rasters as alternatives, even though the head plan
   may select either content variant. The source set now carries both exact
   realizations, and a mixed-source regression pins that invariant. The next
   signed run repeated the failure and showed the remaining lifetime gap: the
   CPU scene's residency roots omitted queued and in-flight Present candidates,
   so its bounded recent-update fallback eventually evicted handle 4 before the
   retained source set was built. Every scheduled candidate's CPU variants now
   remain rooted through rejection or complete cohort retirement, with a
   scheduler regression beside the source-set regression. The third signed run
   repeated `MissingCpuSource(4)` with handle 4 resident throughout, which
   placed the defect outside residency entirely: a queued Present planned the
   candidate it was released against while sourcing from the CPU layers frozen
   when it entered the queue, so a surface admitted during the wait was named
   by the plan and absent from its sources, and a silent fall-through deferred
   the mismatch to lowering. Present submission now reads its sources from the
   same candidate it plans, the enqueue-time snapshot is gone, and an
   unsourced candidate surface is refused where it is found. A differential
   regression pins both directions. The signed rerun on `0505cb19` confirms it
   on hardware: the guide ran to completion, with Helium composing through
   committed generation 265 on the same CPU handle whose fifth generation ended
   the previous run, and the proof phrase accepted at 34 of 34 events. The run
   still failed at the completion check, on 29 X protocol errors whose first is
   a `BadRequest` for XInput1 `ListInputDevices` -- never implemented between
   `GetExtensionVersion` and the XI2 range. The client recovered on its own; a
   normal session tolerates no protocol errors. The enumeration is now
   implemented rather than excused: one passive table of the virtual master pair
   is projected into both the XI1 and XI2 replies, and a test walks both wire
   formats back into records and requires them to agree, since the two encodings
   are too different for drift to show up any other way. That rerun cleared the
   protocol errors and then hard-stalled a page flip: an extra `Super+N` had left
   the active view in a layout that places one window, so policy answered the
   browser's `Manage` request by placing nothing and the owner re-asked it 1,438
   times in five seconds. A committed answer now settles its cause until
   snapshot-visible facts change; engine focus reports an unchanged seat focus as
   unchanged; indicator chrome is published by its content rather than by a commit
   count, which also stops a policy commit cancelling an in-flight indicator
   click; and the retained queue skips a scene the output already holds. The next
   run passed the entire guide, proof phrase included, and failed only on 24 X
   protocol errors: XFixes `SelectSelectionInput` refused the root window, which
   is the argument every toolkit passes to watch a selection. The root is now
   admitted wherever a request scopes to a window rather than acts on one, and
   protocol errors are tallied per opcode so a failed run names every cause at
   once instead of costing a run each. Selection watching itself is still
   unimplemented -- the error is gone, the events were never there. The next run
   named three causes at once rather than one, which is what the tally was for:
   GLX `CreatePbuffer` and `DestroyPbuffer` refused six times each, and six core
   `GetGeometry` refusals beside them, one sequence from a GL client bootstrapping
   an offscreen surface. Sophia advertised GLX 1.4 while implementing five of the
   twelve requests GLX 1.3 introduced; the pbuffer half is now implemented, core
   `GetGeometry` answers for a GLX drawable, and `GLX_PIXMAP_BIT` is withdrawn so
   the advertised drawable types are the implemented ones. That is proven at the
   wire boundary only: the offline probe meant to exercise the real Mesa path does
   not run, so the physical rerun was the first real-client evidence -- and it
   found that creating a pbuffer nothing may use moves the failure rather than
   removing it. The browser's GL now initialised, reached DRI3, named the pbuffer
   and took `BadWindow`, so its GPU process crash-looped where it had previously
   fallen back to software. Seven request sites across three validators now admit
   a drawable whose buffers the client allocated, while core drawing keeps
   refusing one. The probe harness that should have caught this first is fixed:
   its accept had no deadline and it joined a server thread no client had reached,
   so it hung silently; it now reproduces the physical failure offline in seconds.
   The GLX 1.3 surface is complete apart from the withdrawn pixmap requests, and
   both identified causes are now closed. DRI3 minor 8 `BuffersFromPixmap`
   answers from retained plane descriptors, and its server-owned half originates
   a buffer for whatever drawable a GL client names, since a client asking the
   server to own its storage names a GLX drawable rather than a core pixmap. GLX
   advertises the ES profiles a translating client's fallback asks for. The
   offline pbuffer smoke drives real Mesa through a GLX pbuffer and exits zero.
   It requires `--features native-session`: without it the allocator that
   originates the buffer is compiled out, the server-owned half never runs, and
   the recovery refuses a pbuffer it should have backed. The browser stall is
   reproduced offline by `x-authority-browser-smoke`, but only at partial
   fidelity -- it stops after negotiating the GLX and DRI3 versions, where the
   rig brings a GL context up before going quiet, so it does not yet stand in
   for the rig. Signed Hagia archive `0006` closed the row on signed source
   `f97e8e807e3e15716fde50b25b4b9aaaf07806f1` and signed Hagia source
   `a76528fcf6e227e5c0a58772da655f44b85d0821`. It retained zero protocol errors,
   the complete `Super+P` proof -- three shortcut admissions, a restart at
   `visible_presentation=2 retained_pixels=true`, a fresh `connection_epoch=2`,
   an inert click on the retained pixels at `activation=false`, and two admitted
   activations -- nonzero presentation on both outputs, exact text, and clean
   health and teardown. The first attempt on `fd44d748` was a correct session
   refused by a totals check that no run could satisfy; see the research log.
   Reservations are deliberately still absent.
   The authoritative retained-behavior ledger has 28 classified rows. The
   frozen profile is 21 complete, 0 partial, 0 open, and 7 excluded with written
   product rationales. The checked-in Hagia
   daily-driver profile, not all 137 historical Triad bindings, defines the
   retained surface.
7. **Complete.** The complete cross-client reconnect/restart corpus, public
   xmonad projection migration, digest-pinned revision-3 client, and signed
   frame-fed output archive pass. Revision 3 is stable. API v7 and Engine-owned
   workspace policy are removed.

Cross-drawable `CopyArea` replay, alternative upscale kernels, linear-light
fixed-function blending, mirror re-moding, scanout cloning, and Milestone 14
efficiency work stay off this path unless a named gate produces evidence that
promotes one of them.

### Immediate Next

Ordering for the next few sessions. Each row points at where its detail already
lives rather than restating it; this is a priority index, not a second roadmap.

1. **Complete.** The work-area reservation coordinator is in production.
   Signed Hagia archive `0007` closed it on signed source
   `4e54dfc886d0a96737f3a1fcd3d0bbe4a8ca9edd` and signed Hagia source
   `8a38033aae2bd4470f66e474dcf4447f482df87f`: three claims admitted and
   presented, two reductions and two releases, no refusal, the claim retained
   across the shell's death and re-made at connection epoch 2, zero protocol
   errors, and clean teardown. Previews, icons, MRU policy, and generic
   textures remain out of scope.
   The substrate is complete and proven offline. `sophia_shell_v1` candidates
   carry an optional edge claim in bytes the pre-reservation encoder already
   wrote as reserved zeroes, so older frames decode unchanged; Rust, C, and Nim
   move together and a Nim test pins its encoder to the Rust-generated golden
   byte for byte. `ShellWorkAreaCoordinator` admits a claim against the realized
   topology, names every refusal, and reduces nothing until its bundle commits;
   `reduce_output_work_areas` now subtracts shell bands and X-side struts in one
   reduction. `check_shell_protocol.sh` drives the real Nim shell through the
   real coordinator and reports `reservations=1`, taking a 1440-tall output to
   1412 only after commit and restoring it on withdrawal.
   The live wiring is in place. The session's shell owner holds the
   coordinator, admits each candidate's claim against the realized topology
   before the candidate is prepared, commits it in the same step its pixels
   present, and retains it across disconnect; the WM session mirrors the
   committed bands and reprojects only when they change.
   The depth remains a configuration decision, validated against the wire's
   own reservation maximum and refused outright when no shell is enabled. The
   ordinary compiled profile no longer asks the window switcher to reserve a
   panel: a transient switcher stream is not a persistent panel. The signed
   archive remains the regression for an explicitly configured reservation.
   An earlier entry here named an
   environment variable instead; that variable existed only in the offline
   conformance host, so no session could set it and no session could raise a
   claim. The profile key is what makes the coordinator reachable.
   Two corrections to what this row previously claimed. The claim is not a
   persistent bar: one shell connection carries one candidate stream and one
   visible state, so the strip lives exactly as long as the switcher is
   visible. A panel that persists independently needs a second shell role and
   is separate work. And losing the shell does not restore the work area --
   the presented claim is retained beside the retained pixels, because growing
   the area while nothing can present into the strip is the incoherent desktop
   `ShellWorkAreaCoordination.tla` forbids. The guide and verifier now prove
   the retention rather than a restore.
   The guide, matcher fixture, and verifier carry that evidence, with negative
   cases proving each check fires. One fidelity note from the archived run: a
   public-policy session emits no `sophia_live_work_area status=applied` lines
   at all, so the reduction is evidenced by the shell's own
   `reservation_reduced` records rather than by a work-area line the fixture
   had briefly invented.
2. **Complete.** The checked-in Hagia
   command and pointer surface is closed by executable catalog coverage plus
   its deterministic and physical gates. Trusted one-shot launch placement and
   frame-fed atomic output activation are implemented and offline-proven, and
   the shared restart corpus is complete. The signed-harness candidate now has
   a one-shot proof control at the exact boundary after every KMS card accepts
   the startup candidate but before installation or publication. Its TTY4
   runner, paired evidence/archive verifiers, reference profile, and mutation
   fixtures pass offline. The first explicitly authorized run passed the atomic
   hardware preflight, then Sophia exited before either phase could be archived;
   the runner's error trap incorrectly erased that phase log. Its signed
   successor retained the retry and proved the whole success-side KMS apply,
   first presentation, frontend publication, and local commit. The session then
   rejected Hagia's configuration because the output-only runner omitted browser
   operation slot 2 while Hagia advertises all four session-operation slots. The
   runner now aliases that unused slot to its existing terminal application and
   has a regression requiring slots 1--4. The next signed run passed the complete
   success phase and reached the exact forced rollback, which restored and then
   presented DP-2 correctly. Later scene churn nevertheless submitted two more
   byte-identical DP-2 frames because retained-scene suppression ignored
   rendering, submitted, and displayed ownership; the last redundant flip lost
   its callback before the text proof. The reducer now suppresses only when the
   newest owned logical scene matches, with pending/rendering/submitted/presented
   and newer-different precedence regressions. Signed source
   `870ba46ae231081220b982ecc3a5a95517df7a90` then passed both phases, and
   frame-fed archive `0001` independently verifies the bound success/rollback
   pair, physical input, and clean teardown.
   Generic switcher archive `0006`, Tier-0 indicator archive `0005`, and
   reservation archive `0007` already close their retained slices. MRU policy,
   filters, previews, icons, persistent panels, Janet, broad portals, watched
   reload, and other excluded behavior remain explicit post-freeze work.
3. **Complete.** API v7 and Engine-owned workspace policy are removed. The
   public Hagia, xmonad adapter, archived-client, and protocol gates remain
   green. Row 4 promotes their schema-5 installed successor.
4. **Complete.** Promote the next schema-5 successor through `Sophia Hagia
   Promotion (Packaged Default)`. The first intended login instead archived
   attempt `0006` from old schema-3 release `0.1.0-66a279286bdd`; its emergency
   exit is recovery evidence, not a promotion result. Installation recovery now
   verifies and re-activates an already-present immutable release, repairs
   operator links and greetd entries, and preserves rollback. That repair
   activated `0.1.0-50c7cb2d2d54`, and its packaged-default attempt `0001`
   completed one normal Logout with clean lifecycle health, but the operator's
   `glxgears` window did not present. Its 300-by-300 first DMA-BUF arrived after
   policy had staged a 1278-by-1424 launch layout; the pixel-silent surface was
   correctly deferred from that epoch, but candidate selection neither primed
   its admission extent nor queued a successor while any epoch was pending.
   Candidate selection now primes its measured safe extent and queues recovery
   unless the current epoch owns that exact surface and size. Signed successor
   `66792329d90d64e26af839dfe494c74d94323c6a` produced independently verified
   packaged-promotion archive `0002`: the 300-by-300 first GLX candidate primed,
   armed, committed, and retired, its 1278-by-1424 standing target committed,
   772 GLX presentations retired, and normal Logout finished with zero protocol
   errors and clean lifecycle health.
5. **Complete.** Milestone 14's bounded visual-retirement model and the
   retirement-fed three-slot implementation are complete, and the three-slot
   boundary is promoted. Public snapshots source surface state from
   X-authority lifecycle facts rather than raster generations; signed source
   `c681f762` confirmed that ordinary repaint no longer causes the
   stale/rebuild storm.
   Signed native archive `0001` closed the row on signed Sophia source
   `05d98e44981f5086fc8d2bd3ee4580944029a952` and signed Hagia source
   `9c9a59061fd0d8e88310b764f7dd240e729fb035`, against Hagia's tracked default
   profile. Hagia was the first proof client of the Sophia WM and shell
   protocols with no xmonad bridge in the session. The bounded workflow ran in
   order: three terminal launches, each committing its layout before the next
   was requested and reaching an admitted surface; one visible `Super+J`
   committing focus to a surface; one close; and a normal logout. Committed
   layouts moved 0 to 1 to 4 surfaces and back to 3 after the close.
   The three-slot evidence is exact. Renderer-worker requests settled as
   `worker_requests=263 == worker_completions=263 + frame_slot_deferrals=0`,
   with `frame_slot_stale_releases=0` and `frame_slots_leased=0` at
   completion: no stale release and no leaked lease. The aggregate watermark
   of 6 is three slots on each of two presented heads, both reaching full
   occupancy.
   Everything else the row required is in the archive: separate protected
   broker and shell admissions with clean ready-to-stopped lifecycles, the
   34-event physical text proof, 2 ms session-control queue dwell and 1 ms
   ack against a 100 ms budget, drained native scanout with no abandoned
   scanouts, clean session/topology/cleanup health, zero unexpected protocol
   errors, and exact TTY recovery with `emergency=false`. Four stale policy
   responses were ordinary scene races; every one re-armed and committed.
   Bounded buffer-age damage history is now the active step.

Completed since this ordering was written. The Pnut Landlock empty-allowlist fix
was submitted upstream as [mikedanese/pnut#3](https://github.com/mikedanese/pnut/pull/3)
from signed commit `70c8ea8a9fb419ca9808caae4615d6bbeb5dd973`; see
[`docs/pnut-evaluation.md`](docs/pnut-evaluation.md). Protection domains are now
required where roles are admitted rather than only where domains are
constructed: the metadata-bearing role sockets refuse a supervised PID, refuse
an expected peer identity at bind time, and admit only the launch evidence
carrying their domain role. A caller that builds no domain no longer gets
admission without a boundary and without a complaint. See the ratified entry
below.

Not yet: making the protection domain the default rather than opt-in. It reads
as the obvious next step now that the metadata-bearing roles require one, and it
still owes a deliberate decision about hosts that have no `bwrap` rather than
arriving as a side effect. The blind spatial-policy and output roles are what
that decision governs; they still admit on a supervised PID today.

### Production Readiness Infrastructure

This supporting tranche does not reorder the product critical path above.

- [x] Extract production session lifecycle and domain integration tests from
  `sophia-cli` into `sophia-session`. The installed binary now selects commands
  and owns concrete stdout/stderr presentation; the passive session library
  reports exact evidence through a host-installed callback.
- [x] Extract typed profile, direct-scanout evidence, archive, and gate logic
  from `xtask` into development-only `sophia-conformance`. Production crates
  and installed artifacts have no dependency on it.
- [x] Make `cargo xtask` the canonical offline developer/CI entry point and
  reduce `just` to optional one-line human aliases. Direct-scanout verifier and
  archive shell entry points are compatibility shims into the typed Rust path;
  no repository workflow calls `just`.
- [x] Add canonical `sophia session run` and `sophia session input-guard`
  commands. The old flat spellings remain delegating compatibility aliases.
- [x] Replace the numeric source-layout baseline with an exact identity ledger.
  A moved, added, or retired violation now requires review; every recorded row
  remains debt rather than becoming an exception.
- [ ] Move the remaining session-private test modules out of production source
  as visibility boundaries are made testable, and split the oversized cohesive
  units named in `docs/source-layout-debt.txt`. Do not weaken privacy or create
  test-only production APIs merely to move a file.
- [ ] Reduce `tools/start_sophia_tty3.sh` to the smallest necessary TTY/display-
  manager adapter around the production session entry point. Typed profile
  parsing, verification, archive handling, and gate orchestration already live
  in Rust and must not return to shell.

The ownership and command contract is in
[`docs/development-tooling.md`](docs/development-tooling.md). The next product
row remains direct-scanout return-to-composition on overlay/effect activation.

## Installed Hagia Promotion Contract

Sophia/Hagia becomes the ordinary physical session when one packaged candidate
passes bounded deterministic preflight and preserves all recovery routes. Live
use then produces immutable evidence rather than serving a fixed-duration gate:

1. Normal login, automatic Kitty startup, and normal logout through greetd.
2. Exact Sophia and Hagia executable identities in every archived attempt.
3. Clean application, policy, frontend, renderer, KMS, input, and VT teardown.
4. Ctrl-Alt-Backspace returns safely and is classified as `recovered`, never
   as a clean session.
5. Unexpected termination and invalid final health remain failed evidence.
6. Installed release artifacts: no source build, mutable home-directory policy,
   manual service repair, or ad hoc process cleanup during ordinary login.
7. No minimum elapsed time, launch count, or action count. Scenario coverage is
   cumulative and informational; a named scenario becomes a gate only when its
   affected architectural change requires that bounded proof.

---

## Boundary And Capability Ledger

This ledger records the current product limits so later feature work goes to
the correct authority.

### Boundaries To Preserve

- **Engine** owns physical input, outputs, work areas, scene geometry, focus,
  chrome, transactions, rendering, presentation, and scanout. It must not learn
  X11 resource identities or application metadata.
- **X Authority** owns visuals, colormaps, X resources, ICCCM/EWMH reduction,
  X11 events, client drawing, and protocol feedback. It lowers pixels and
  opaque policy facts into Engine; it does not own physical layout or scanout.
- **Blind WM policy** consumes opaque surfaces, workspaces or views, geometry,
  constraints, and permitted role facts. A native Sophia WM speaks this API
  directly. A classical X11 WM speaks to a private synthetic X server whose
  bounded profile translates its policy into the same API. Neither path may
  receive XIDs, titles, classes, PIDs, namespace IDs, or payloads.
- **Session shell and configuration** own trusted launch provenance, key-bound
  applications, status presentation, wallpaper, lock, screenshots, audio, and
  process supervision. These are not X Authority shortcuts.
- **Portals** own cross-namespace clipboard, drag-and-drop, file, URI, capture,
  and notification decisions. Only the small-text `CLIPBOARD` and `PRIMARY`
  execution path is complete.

### Current Limitations

- Release `0.1.0-4c3121421f12` remains installed. Automatic Firefox attempt
  `0002` passes the dedicated immutable gate, including exact renderer-image VT
  capture/restore, the browser and floating-dialog workflow, clean normal
  logout, zero unexpected protocol errors, and no retained profile. The
  remaining promotion work is outside this focused browser boundary.
- The xmonad bridge has one flattened `active_workspace` policy view even
  though the session descriptor can express output/workspace mappings. True
  independent per-output workspaces require output-scoped active-workspace
  state throughout the bridge and Engine transaction path.
- The xmonad compatibility profile now exposes opaque focus-master,
  swap-master/up/down, shrink/expand, master-count, reset-layout,
  toggle-floating, and sink actions without expanding the WM wire format.
  Focus-output, move-to-output, output-scoped layout state, and supervised WM
  restart remain compatibility work.
- `ThreeColMid`, `Tall`, `Mirror Tall`, `Full`, and `Spiral` have exact
  configured-bridge geometry coverage. Xmonad's `Tabbed` layout depends on
  title-aware, WM-drawn decorations and therefore does not fit the blind-WM
  contract. If tabs are admitted later, Engine must draw metadata-free native
  tabs.
- Xmobar can render, reserve a work area, update, and retire cleanly, but it has
  no private workspace/layout/focus feed. Such a feed must be emitted by
  Engine or a trusted shell broker and contain only workspace number, approved
  layout name, and focus state—never window titles or client identity.
- Application placement cannot use xmonad class/title rules. Requested launch
  placement, such as Firefox on workspace 2, must come from trusted launch
  provenance or explicit user action.
- The X setup catalog, passive colormap ownership, RGB16 allocation,
  named-color lookup, color query, and error paths now agree on fixed 24-bit
  XRGB and 32-bit ARGB TrueColor semantics. The remaining color gate is a
  physical captured-pixel proof on the successor installed candidate. The
  proof command and fail-closed archive verifier are implemented but do not
  count as physical evidence until a new installed run passes.
- The daily-driver session still uses the `classic-shared` X namespace. The
  confined-group architecture and most portal executors are not yet promoted
  into the normal Firefox session.
- Tray/XEmbed, lock, screenshots, wallpaper, audio control, and general prompt
  UI are shell or portal work. `xcompmgr` must never run under Sophia because
  Sophia is the compositor.
- Full classical-desktop parity remains explicitly deferred and ownership
  split. A trusted shell/session broker must own arbitrary launch, lock,
  screenshots, wallpaper, audio/media/eject, and launch-placement provenance.
  Engine chrome must own metadata-free tabs, decorations, and fullscreen
  presentation. A redacted shell feed must own workspace/layout/focus labels.
  The X compatibility layer still needs tray/XEmbed, output focus/move,
  optional input aliases such as Super+Tab and button-2 swap-master, and
  evidence-backed per-WM profiles. None of these may introduce titles,
  classes, XIDs, PIDs, namespace identity, or executable commands into the
  blind WM boundary.
- The compatibility bridge currently has a complete xmonad profile, not broad
  classical-WM compatibility. Other WMs such as i3, dwm, and qtile require
  separate evidence-backed profiles against the same synthetic-X and Sophia
  WM boundaries; no profile may grow into a proxy for the real X Authority.
- The small bundled native WM proves the direct API and native chrome path, but
  it is not the intended full desktop policy. Hagia is the planned first
  demanding Sophia-native WM and shell family: a blind spatial-policy process,
  an optional separately authorized shell, and ordinary Sophia session and
  portal services.

---

## Milestone 13: Public Policy Protocol And Hagia

This is the active promotion milestone. Production follows the public native
policy path directly; the frozen xmonad baseline remains a regression and
future compatibility target, not a prerequisite.

### 13.1 Ratify And Model The Boundary

- [x] Reconcile the architecture, WM API, Hagia design, specification draft,
  and research log around one language-neutral policy protocol. Mark the
  current workspace-oriented Rust API v7 experimental and reserve
  `sophia_wm_v1` for the first stable public projection interface.
- [x] Add bounded `PolicyConnection` and `PolicyProjection` TLA+ models before
  changing production IPC or Engine policy state. Check negotiation,
  capabilities, transfer assembly, connection epochs, stale proposals,
  multi-output atomicity, focus, timeout, disconnect, restart, and
  last-committed projection preservation.
- [x] Map every model action to its owning Rust boundary. Preserve each
  implementation-relevant TLC counterexample as a deterministic Rust
  regression before correcting the implementation or model.
- [x] Audit retained Triad capabilities against Sophia, Hagia, River, and Niri.
  Keep spatial policy in Hagia; keep input, client settlement, rendering, and
  scanout in Engine; reserve separate session, shell, broker, and portal roles.
- [x] Extend the policy models for ordered action causes, policy-initiated
  reprojection, configuration generations, frontend settlement, reduced
  pointer interactions, and opaque session-operation outcomes before adding
  those transitions to the draft wire.

### 13.2 Publish A Dependency-Free Wire Contract

- [x] Keep the bounded 24-byte little-endian Sophia envelope and owner-only
  Unix transport. Make the session host role-specific sockets beneath its
  private runtime directory and admit exactly the expected supervised peer.
- [x] Define stable layouts in a narrow checked-in KDL schema. Generate and
  retain dependency-free Rust and C99 codecs, normative byte tables, and
  golden vectors; normal builds and third-party clients must not run or link
  the generator.
- [x] Add strict begin/chunk/end transfers for complete snapshots and
  projections above the 64-KiB frame limit. Bound the first WM interface to 16
  outputs, 1,024 manageable surfaces, and 256 registered bindings.
- [x] Compile an independent C client and run it against the same golden and
  malformed-frame corpus as the Rust codec. Reject unknown, excessive,
  partial, duplicate, reordered, stale, and trailing data without mutation.
- [x] Complete the draft revision line before stability (currently revision 3):
  add output work rectangles;
  reduced surface kind, presentation request/current state, and exact-size
  constraints; projection presentation decisions; request causes; policy
  configuration; Engine chrome; session-operation tokens; reduced
  interactions; and a bounded policy-dirty request.
- [x] Preserve non-idempotent activation order with the existing bounded
  sixteen-request owner queue. Coalesce only replaceable scene refreshes and
  continuous interaction geometry; saturation consumes the shortcut, fails
  closed, and emits a bounded diagnostic.
- [x] Regenerate and re-run the Rust/C golden and malformed corpora, then update
  Hagia's independent Nim codec without adding a Sophia build dependency.
- [x] Add the indicator descriptor before the 13.4 freeze.
  `capability "indicators" bit=8`, record kinds `ProjectionIndicator` (max 256)
  and `ProjectionOutputStatus` (max 16), and `indicator_count`/`status_count`
  fields in `ProjectionBegin` are in the schema with generated Rust and C99
  codecs and golden vectors. The generator gained a fixed-octet field type so
  records could carry bounded labels while staying fixed width. Wire bounds are
  permanent: 256 indicators, 16 status records, 32-byte UTF-8 labels and layout
  names. The 32-per-output limit is Engine validation, not a wire constant.
  See `docs/sophia-indicator-descriptor.md`.
- [x] Model the descriptor before changing the schema. Revise
  `validation/tla/ShellObservation.tla` so the descriptor rides the proposal and
  its invariants hold with no explicit publish or invalidate step, and add
  `validation/tla/IndicatorTransfer.tla` for declared-count, ordinal, and
  bounds integrity across begin/chunk/end.
- [x] Regenerate the Rust and C99 codecs, wire tables, and golden corpora for
  the new records. `tools/check_policy_protocol.sh` passes end to end. Closing
  it also repaired a pre-existing gap: the C conformance harness had never been
  taught `snapshot_session_operation` or the five policy/session messages, so
  its valid-frame and record gates had been failing before this work began.
- [x] Update Hagia's independent Nim codec for the new records so the
  cross-repository conformance gate stays green, without adding a Sophia build
  dependency. Hagia decodes both records, rejects an over-long label length and
  non-zero padding, and declares zero indicators until it advertises the
  capability. `SOPHIA_STACK_ROOT=… nimble test` passes.
- [x] Defer the tier-1 texture question rather than blocking on it. Whether the
  shared transport can carry shell texture traffic under the 64-KiB frame limit,
  single in-flight transfer, and bytes-only wire binds `sophia_shell_v1` only.
  Tier-0 Engine chrome renders the descriptor with no client interface, which
  removes that question from the freeze path; see
  `docs/sophia-shell-v1-direction.md` open question 2.

### 13.3 Replace Workspaces With Output Projections

- [x] Introduce one canonical Engine reducer for complete scene snapshots and
  complete affected-output projection proposals. Validate generations,
  capability, constraints, geometry, uniqueness, one-output-per-surface, and
  visible focus before one logical commit.
- [x] Keep full snapshots and complete projections as stable semantics. Permit
  only model-equivalent chunking, coalescing, caching, or later delta encoding;
  no transport optimization may expose partial policy state.
- [x] Add the API v7-to-projection adapter and prove the dormant Rust reference
  WM and generic X11 WM bridge against the canonical reducer.
- [x] Add an explicitly selected Hagia live profile through the public
  transport and canonical reducer, with no silent API-v7 fallback.
- [x] Promote that profile to the installed native default while retaining
  Kitty, xmonad, and the previous immutable release as recovery routes.
- [x] Remove v7 and Engine-owned workspace state. Xmonad runs through the public
  compatibility adapter, the complete restart/last-layout corpus passes, and
  signed frame-fed archive `0001` has closed the final retained-ledger gate.
- [x] Preserve registered physical actions and session operations as opaque,
  capability-gated tokens. Keep raw input, executable commands, client
  metadata, protocol objects, namespaces, pixels, and renderer handles out of
  policy IPC.
- [x] Add a two-stage canonical reducer: validate a complete proposal against a
  cloned successor, preserve last-good authority, and reject promotion if its
  connection, request, scene generation, or earlier commit was superseded.
- [x] Wire staged projections through production frontend configure and
  renderable-content settlement. Emit `committed` only when authoritative
  state matches; otherwise request a fresh snapshot without silently changing
  policy geometry.
- [x] Bind the owner-only endpoint before a supervised peer starts, authorize
  its exact UID/PID afterward, and prove that ownership order through the
  independent C and Hagia conformance host.
- [x] Host the production endpoint in the Sophia session, supervise exactly one
  admitted peer, preserve the committed scene across replacement, and keep
  policy checkpoints private to that peer.

### 13.4 Prove The Boundary And Port Triad

Interface major 1, wire revision 3 is stable. The fixed nine-view scroller is
the frozen profile, not the feature ceiling. Hagia's retained-behavior port
ledger closed across spatial policy, Hagia Shell, Sophia session/dedicated
authorities, and the required brokers/portals. River/Wayland and Niri
compatibility machinery is excluded; retained product behavior is not.

- [ ] Implement the minimum experimental display-list, target-resolved input,
  redacted broker, and shell-role transport needed to port retained Triad shell
  workflows before the WM freeze. Keep this endpoint distinct from
  `sophia_wm_v1`; this item does not itself stabilize `sophia_shell_v1` or pull
  general rendering-efficiency work forward from Milestone 14.
  **Start with the broker, not the transport.**
  `docs/sophia-shell-v1-direction.md` is explicit that the metadata broker is the
  larger prerequisite: the redacted presentation feed has no implementation, and
  without it the shell interface would be specified against a data source that does
  not exist. Specifying a transport first would produce a wire for nothing to send.
  The feed has exactly two sources with different trust properties, and conflating
  them is the failure this row is most likely to have. Policy-authored structure —
  workspace list, occupancy, focus — is blind-safe by construction and already
  answered by `docs/sophia-indicator-descriptor.md`, because workspace state
  originates in the policy process where no broker can see it. Only toplevel
  identity for taskbars and docks needs real sanitization, and keeping the two apart
  is what lets a status bar never request identity at all.
  The first buildable piece is therefore the toplevel descriptor and its reduction:
  a record a shell can render, constructible only by reducing client metadata, so
  that leaking a title, app ID, PID, or path is a compile error rather than a review
  finding. Activation, close, and minimize ride the existing opaque action tokens,
  which already exist and already carry issuer scoping — the broker mints no new
  authority for them. Decision 2 settled the neighbouring question for the WM side:
  classifications reach policy through a capability-gated extension chunk, so the
  broker's design is no longer constrained by what fits in `SnapshotSurface`.
  The crate question is settled: `sophia-broker`, its own crate and its own
  `PolicyRole::Broker` socket. Not `sophia-portal`, whose broker is a single-use
  transfer grant lifecycle with nothing in common beyond the word, and whose own
  ownership row forbids the client-global visibility a metadata broker needs.
  **Built so far**, all with the authority reducing and the broker never holding raw
  identity: the disclosure vocabulary in `sophia-protocol`; authority-side reduction
  in `sophia-x-authority`; the `sophia-broker` crate owning trust, icon tokens,
  disclosure rules, and descriptor emission; `PolicyRole::Broker` with its own socket
  and env var; and the metadata broker health smoke reporting the real broker.
  The chain is proven to compose end to end in `crates/sophia-session/tests/metadata_chain.rs`
  — authority reduction, broker, `ChromeDescriptorTable` — including that a title
  never reaches Engine under a `ClassOnly` rule, and that the Engine ingress needed
  no widening to accept broker output.
  **The broker tranche is now hosted.** `sophia_broker_v1` has its own bounded
  schema, strict codec, revision negotiation, owner-only role socket, and exact
  protected-peer admission. A production Hagia session starts `MetadataBroker`
  in a distinct Bubblewrap domain. The broker publishes a per-surface rule to the
  X Authority; only the authority's reduced candidate returns; and the sanitized
  descriptor commits to the session owner's real Engine
  `ChromeDescriptorTable`. The default production rule is `ClassOnly`, and a
  protected executable smoke proves no title escalation, ambient display/session
  bus, inherited descriptor, host-home/temporary-file visibility, or outbound
  network. Signed archive `0004` supplies its ordered real-session proof.
  **The reference rendering tranche is now complete.** A pure Engine reducer
  turns at most sixteen exact-generation descriptors plus issuer- and
  recipient-epoch-scoped actions into a bounded title-only rectangle/text list
  and presented opaque targets. The same immutable list lowers independently
  for unequal heads; a 128-entry, 16-MiB renderer-private JetBrains Mono cache
  retains in-flight rasters safely. Exact press/release capture returns only
  the opaque capability reference and activation identity. Offline tests and a
  16-entry two-head timing probe cover this tranche. The umbrella remains open:
  the protected wire, external Hagia Shell, and shell-enabled live dispatch now
  exist, and explicit X-grab arbitration plus compiled-profile enablement are
  complete. Signed installed shell evidence remains open.

- [x] Create Hagia as a standalone Nim repository with no Triad history,
  River/Wayland dependency, inherited binary, or shared build scaffolding. Its
  independent envelope and record decoder pass Sophia's retained corpus.
- [x] Complete Hagia's first socket proof: strict snapshot assembly, exact
  affected-output request, projection encoding, committed outcome, and
  canonical Engine reduction without generated Sophia or Triad protocol types.
- [x] Keep Hagia tags, stable `ViewId` values, ordered per-output views, and
  reconnect affinity private. Project the fixed nine-view, fixed-point
  scrolling layout into one-output-per-surface Sophia geometry
  with no Hagia back door.
- [ ] Add deterministic Hagia reducer messages for view/tag changes, focus,
  movement, grouping, layout adjustment, output focus/moves, presentation
  state, floating, scratchpads, and opaque session operations.
  The fixed nine-view scroller profile now covers ordered view moves, output
  focus and movement, column consume/expel, width/height adjustment, floating,
  fullscreen, maximize, minimize/restore, and operation-slot-bound opaque
  session requests. Hagia now retains nonempty multi-tag view and
  focused-window membership through eighteen additional opaque actions;
  dynamic workspace lifecycle, occupancy navigation, and scratchpads are now
  implemented in Hagia's action catalog; configured workspace naming remains
  partial because `setWorkspaceName` has no bound action. The transitions
  remain unbound until configuration migration preserves Triad's existing chord
  meanings. Interface revision 3 admits 256 binding registrations, so capacity is not the
  constraint: the bootstrap emits 39 key plus 2 pointer bindings from Triad's
  baseline 132 key plus 5 pointer bindings, and the other 96 classify into
  shell, broker, portal, and session authorities that do not exist yet. That
  authority split, not a slot count, is what keeps binding classification on
  the pre-freeze port path.
- [ ] Add Engine-owned pointer interactions, bounded focus history, private
  checkpoint/reconciliation, and crash/restart proof while applications and
  the last committed scene remain alive.
  One completed Engine-captured move/resize now crosses as a reduced final
  interaction, and Hagia validates its target, capability, output bounds, and
  exact floating geometry. Focus and minimized histories are bounded, and an
  owner-only, size-bounded checkpoint uses a same-directory fsynced atomic
  replacement and is revalidated before complete-snapshot reconciliation. The
  owner-side recovery matrix now kills the public
  path at proposal-staged, frontend-pending, prepared, and terminal-outcome
  boundaries through the normal supervisor; all four preserve layout, restart
  at a fresh epoch, and drain cleanly. Continuous updates, a physical
  checkpoint restore and operation-phase faults remain open. The separate
  client lifecycle gate now defines post-negotiation and complete-snapshot
  crash/replacement checks; it remains unclaimed until an authorized live run.
  Hagia now emits exactly one bounded `PolicyDirty` after a restored checkpoint
  first reconciles and commits; the independent socket test proves generation
  advance and a fresh complete cycle. Both installed Hagia gates require its
  diagnostic after restart, but no physical evidence has been claimed.
  `tools/hagia_policy_physical_gate.sh` now encodes the opt-in two-output
  restore/presentation/active-output procedure. Its first authorized run
  proved checkpoint save, supervised epoch-2 restart, checkpoint load and
  reconciliation, retained fullscreen geometry, and post-restart page flips,
  then failed the exact text boundary because seven keys arrived while Engine
  and frontend focus differed. Engine now retains only those client-bound keys
  in a bounded exact-target handoff while continuing to resolve Hagia's
  reserved chords. A replacement run then proved the exact text and clean
  bounded shutdown, but the operator entered the final phrase before the
  post-restart actions. That run also showed that checkpoint occurrence 4 was
  reached during startup, before the intended physical trigger. The gate now
  arms occurrence 6, after the two pre-restart actions, labels every missing
  phase explicitly, and uses the phrase only as the final exit signal. A third
  run delivered exactly the 34 press/release events for the 16-character final
  phrase plus Enter, with no preceding physical chord events, no ordered
  actions, and no restart. It was a clean negative attempt rather than routing
  evidence. The gate now keeps an event-driven procedure visible inside Kitty,
  advances only when each committed action appears in the evidence stream, and
  withholds the final phrase until the restart and complete action sequence are
  proven. Its first guided run exposed a harness-ordering bug: the leading
  Super transition reached the application path before the bound action key
  was consumed and incorrectly entered the exact text matcher. The text proof
  now excludes non-text modifier transitions while leaving their ordinary
  client delivery intact; an unmatched non-modifier key still fails exactly.
  The next guided run proved the complete pre-restart sequence, epoch-2
  checkpoint load/reconciliation and refresh, post-restart fullscreen, and two
  maximize transitions. It then minimized its own guide before displaying the
  restore instruction and timed out with no text events. Minimize and restore
  are now one visible paired instruction. A follow-up still stopped after
  minimize with exactly the twelve routed modifier transitions belonging to
  the six committed chords. The guide now leads with an explicit three-line
  warning to press and release `Super+R` while the screen is blank, and evidence
  distinguishes immediate physical action admission from later policy commit.
  A later attempt reached the second maximize prompt but exhausted the old
  deadline while the operator was reading. Timestamp review corrected that
  diagnosis: a separate 15-second physical-sequence timeout fired, while the
  global deadline had not. Physical proofs now retain that fail-fast default
  but may request an explicit bounded override; this human-guided gate uses ten
  minutes inside an eleven-minute global ceiling and still exits immediately
  on success. The following run proved that `Super+R` was admitted and
  committed but still left a blank scene: minimize had removed the surface
  from Sophia's supposedly complete policy snapshot, so Hagia correctly
  reconciled it as destroyed before applying restore. Sophia now retains
  authority-observed facts separately from visible layers. A follow-up run
  proved that the X `mapped` observation also cannot carry policy lifetime:
  Engine admission is not a second client `MapWindow`, so an admitted surface
  may retain its pre-admission `mapped=false` observation. The snapshot now
  follows explicit request/withdraw ownership until withdrawal or removal. The
  gate additionally requires a nonempty checkpoint after restore, rejecting
  the observed committed no-op. The first request/withdraw-lifetime run then
  failed closed at startup because authority presentation observations admitted
  three transient surfaces while one lacked an X11 client route. Snapshot
  admission now intersects retained policy ownership with a live authority
  route, which survives minimize but excludes unrouted hierarchy observations;
  the regression covers that race. The nonexclusive real-Hagia restart smoke
  passes this boundary with one routed surface, nonempty epoch-2 checkpoint
  reconciliation, and clean shutdown. The following physical run proved the
  complete action sequence and, critically, retained one nonempty surface
  through minimize and restore. It then failed after accepting the final text
  because Kitty closed while Hagia's last one-surface projection was in flight.
  Public response handling now advances current Engine scene facts before
  materializing placements and retires such a response as `RejectedStale`.
  The guide now remains alive after accepting the phrase until Sophia completes
  the proof, rather than voluntarily closing during evidence settlement.
  The next attempt admitted and committed both `Super+Right` chords but never
  restarted: legitimate extra settlement checkpoints had shifted the fixed
  occurrence used for fault injection. The physical gate now correlates its
  one restart to the committed fullscreen action, the first committed
  active-output action, and the next nonempty checkpoint. Its watcher runs
  beside an `exec`-replaced Hagia, preserving the supervisor-authorized PID,
  and the marker prevents a second epoch from being killed.
  The resulting run proved restart timing, checkpoint recovery, and the entire
  physical action sequence, then flushed all 52 action-plus-text key
  transitions. It failed only because the stock semantic-result writer was
  appended after Kitty's custom guide command and therefore became unused
  guide arguments. Sophia now passes the private result path explicitly and
  the guide records the exact line it reads; an isolated completed-log replay
  proves the witness.
  The final physical run passed: one causal restart preserved the nonempty
  checkpoint and fullscreen state; every required post-restart action
  committed; all 52 routed action-plus-text transitions flushed; exact text
  changed pixels and reached a kernel page flip in 24 ms; and health, topology,
  native ownership, namespace, Xauthority, and process cleanup were clean.
  Continuous pointer updates and operation-phase fault coverage remain open
  within this broader item.
- [x] Carry committed public-policy fullscreen, maximize, minimize, and restore
  state through the X frontend's protocol-visible state transition and verify
  exact configure/state feedback. Engine geometry, focus exclusion, semantic
  minimized placement, and render-layer omission are implemented. The offline
  path now installs protected `_NET_WM_STATE` and ICCCM `WM_STATE`, waits for a
  flushed frontend acknowledgement before policy promotion, and restores the
  previous state on rejection. Exact socket tests cover property values,
  notifications, and denied client overwrite/deletion. The installed Hagia
  physical gate now proves all four transitions with real Kitty across one
  supervised checkpoint restart and clean shutdown.
- [ ] Prove the retained scrolling layout plus actions, constraints,
  focus, hidden surfaces, multi-output moves, output loss/return, crash,
  restart, and hot-swap. Port Janet after candidate validation and fallback
  behavior have their own model and deterministic tests; both remain on the
  revision-1 freeze path.
  `PolicyOutputSettlement.tla` now proves the topology core for an atomic
  two-output candidate, output loss, and generation-advancing return. Dynamic
  output ingress now uses a capacity-one udev rescan hint, an owner-wide
  quiescence/rebuild barrier, one routed-input epoch advance, complete
  scanout/pointer/RandR/policy publication, and policy-plus-presentation fence
  before input resumes. `OutputTopologyLifecycle.tla`, Alloy, Z3, and focused
  Rust tests cover the offline boundary. Its guarded physical multi-output
  disconnect/reconnect harness remains unrun, so installed evidence is open. The public
  owner now admits only complete, valid output snapshots atomically, advances
  generations after disappearance, selects a surviving active output, and
  recognizes same-ID descriptor changes without partially mutating state.
  Unified desktop output admission now also constructs a pure stable-ID plan
  with exact candidate and rollback states after revalidating the complete
  reconciliation against the owned capability snapshot. Startup still issues
  no configuration KMS mutation; atomic test/apply execution and backend
  settlement remain the next dedicated output-authority tranche. A pure
  coordinator now models those typed test/apply/rollback effects and exact
  generation/digest completions, rejects stale or phase-invalid results,
  discards test-rejected candidates, and requires terminal rollback settlement
  after apply failure. That coordinator is no longer dead code: a typed effect
  executor trait and a bounded driver now carry one prepared candidate from test
  through apply or rollback to a terminal settlement, and startup drives the real
  phase machine. Deterministic
  tests cover activation, declined test, rollback after apply failure, and failed
  recovery, and prove the declined path never reaches apply.
  **Startup's test phase now reaches the kernel.** The candidate is resolved into
  topology heads and submitted as one `TEST_ONLY` request, so the kernel judges the
  complete desktop rather than nothing at all. Startup still issues no configuration
  KMS mutation, and not because a flag says so:
  `NativeOutputTopologyValidationExecutor` has no apply to gate. Its heads carry no
  plane state, so there is nothing to scan out and applying them would activate
  CRTCs showing nothing; mutation needs `NativeOutputCommitExecutor` and real
  framebuffers.
  A validated topology still settles as rejected, because apply then refuses, and it
  refuses with the same `WouldBlock` a busy device reports. The settlement alone
  therefore cannot distinguish an accepted desktop from a busy card, so the executor
  retains what the kernel said and startup logs it separately as `validation=`.
  A topology spanning two DRM devices is declined rather than validated: one atomic
  request reaches one device, and validating a fragment would answer a question
  nobody asked.
  `tools/run_native_output_gate_tty4.sh` passed on tty4 at commit `eab7922e`,
  which is the first identity-pinned evidence for this row:
  `result=passed`, probe accepted with and without plane state, and
  `validation=accepted outputs=2 heads=2`. Its transcript carries the commit, the
  binary's checksum, the sysfs connector facts, and a digest over the body, so the
  claim can be rechecked rather than believed — the checksum matches what that
  commit builds and the digest matches the body.
  `tools/native_topology_validate.sh` runs that chain read-only against real
  hardware — capabilities, projection, reconciliation, plan, heads, phase machine —
  differing from startup only in opening the cards directly rather than through a
  seat controller. **Its first run passed on the AMD two-output reference:**
  `validation=accepted settlement=not_applied outputs=2 heads=2`. The kernel accepts
  the configured two-output desktop as one atomic request.
  What remains unproven offline is the executor adapter itself. Covering its three
  arms needs a fake atomic-commit device, which means the `drm` crate in
  `sophia-cli`, and `docs/live-backend-dependency-policy.md` keeps device-facing
  types in `sophia-backend-live`. The submission beneath it and the resolution above
  it are both covered; the seam is covered by that hardware run.
  Apply now exists behind two gates and remains **unrun**. `native-topology-apply`
  refuses without `SOPHIA_NATIVE_OUTPUT_APPLY=1`, and
  `tools/native_topology_apply.sh` refuses again, validates first, and refuses to
  apply a topology `TEST_ONLY` would not accept. What apply can reach is bounded by
  construction rather than by care: apply heads reuse the framebuffer each CRTC
  already scans out, so the only topology expressible is one whose scanout size
  matches what is displayed, and anything else declines as `NeedsFramebuffer` before
  a commit is submitted. A mode change needs a buffer allocated at the new size,
  which is renderer work and is not in this tranche.
  **That bound turns out to exclude the reference host, which was not the
  expectation.** The plan was that re-applying the topology already on screen would
  be the smallest real mutation available. It is not available: the first authorized
  run refused, and the refusal names why. `DP-2` is a 1920x1080 panel scanning out a
  2560x1440 framebuffer, because the console gives both CRTCs one buffer sized for
  the larger monitor, while the candidate asks each output for its own preferred
  mode. There is no correctly sized buffer to reuse, so no apply is expressible here
  without allocating one. Notably the shared buffer is the mirror-group shape, and
  the two outputs disagree on size, so reusing it for both heads would fail
  `MismatchedMirrorSize` as well — the same constraint reached from the other side.
  Apply therefore stays unrun, and its hardware evidence waits on the renderer
  rather than on anything in the apply path. Nothing was submitted and no output
  state changed; the resolver failed closed before the first commit, which is the
  behavior the gate exists for.
  **Apply is frame-fed, not scratch-fed.** The obvious unblock — allocate a buffer
  at the new mode's size so apply can run whenever it likes — is foreclosed by
  `docs/renderer-import-boundary.md`: native KMS initialization waits for the first
  committed-state frame rather than requiring a speculative or blank visual
  bootstrap. A scratch buffer is that bootstrap, and it would put a frame on screen
  that no committed state produced. The sequence is therefore resize the frame
  target to the new mode, compose one frame at that size from committed state, then
  apply the topology naming that frame. `LiveGbmEglFrameTargetRecord` and its
  created/retained/resized/invalidated/retired lifecycle already model the resize
  step; what was missing was the precondition tying it to activation.
  `native_output_apply_admission` is that precondition. It answers whether every
  enabled output has a valid frame target at its requested mode, and names the
  output and both sizes when one does not, so a mode change reports where the
  session is in the transition rather than reporting a hardware defect. Disabled
  targets owe no frame, for the same reason they contribute no head. Apply consults
  it before composing anything, which turns a missing framebuffer from a discovery
  mid-resolution into a refusal before submission: the reference host now reports
  `native output 2 has a 2560x1440 frame but the requested mode is 1920x1080`.
  Both the precondition and head composition read the currently scanned-out
  framebuffer through one reader, `read_native_current_framebuffer`, so they cannot
  disagree about what "currently displayed" means.
  The renderer half is now wired into normal session startup. An accepted desktop
  output plan is projected into the resource-free authority candidate, retained as
  a private startup transaction, and dispatched only after the visual runtime can
  resize its frame targets and compose the candidate frame from committed scene
  state. It then follows the ordinary atomic apply/rollback, first-presentation,
  and publication sequence; no output-policy peer owns or settles this private
  candidate, and peer reconnect cannot cancel it. The projection and complete
  `sophia-cli` all-features suite pass. The bounded
  `--output-proof-rollback-after-apply` control now forces only that private
  startup transaction into the existing reverse-card rollback path after final
  KMS acceptance and before candidate installation. It requires normal public
  Hagia, native scanout, a bounded runtime, a prepared startup candidate, and
  explicit hardware arming. `tools/run_frame_fed_output_gate_tty4.sh` binds one
  signed Sophia/Hagia build and exact DP-1/DP-2 profile to separate successful
  apply and forced-rollback logs; its verifier rejects publication in the
  rollback phase and its archive binds source, binaries, configurations, and
  connector facts. Synthetic mutation coverage passes. What remains is running
  this gate with explicit authorization on the reference DRM hardware. The
  three-slot recycling pool in Milestone 14 stays gated on its own measurement and
  is not a prerequisite here.
  Rollback heads are resolved beside apply heads, before anything is submitted,
  from the topology still on screen. Sourcing them afterwards would source them from
  a desktop that is already wrong. An output that cannot be restored fails the whole
  plan closed, so an apply never begins without a way back.
  Applying blocks and carries no page-flip event: a modeset must complete before a
  caller may believe it did, and there is no flip to wait on.
  The DRM primitives that executor needs already exist:
  `LibdrmNativeAtomicCommitRequest` exposes `modeset`, `allow_modeset`, and
  `test_only`, and property discovery already finds connector `CRTC_ID` and CRTC
  `MODE_ID`/`ACTIVE`. Composing one request across N heads now exists too:
  `build_native_multi_head_atomic_request` folds every head into one
  `AtomicModeReq` so the kernel accepts or rejects the complete topology and a
  partially applied desktop is never observable. It validates before adding any
  property, so a rejected build never yields a half-populated request, and it
  rejects an empty head set, a shared connector, CRTC, or plane, an invalid size,
  and a missing mode blob on a modeset. Heads sharing one framebuffer are a mirror
  group and must agree on scanout size, which is where the same-mode rule is
  enforced. The returned request carries the previously dead `test_only` path, so
  a caller can validate a topology without touching hardware.
  A planned timing now resolves to a mode too: `resolve_native_output_mode_index`
  matches a requested timing against a connector's reported modes and returns the
  first match, which is the same choice the capability reader makes when it dedupes
  reduced timings, so advertisement and commit cannot disagree about which mode a
  timing names. `create_mode_blob` takes that resolved mode, and
  `create_mode_blob_for_selection` delegates to it.
  `submit_native_multi_head_topology` submits one topology as one request, setting
  `TEST_ONLY` for a validation intent and reporting an unbuildable head set apart
  from a kernel refusal, since one is a mistake in what was asked for and the other
  is hardware declining something well-formed. `NativeOutputCommitExecutor` adapts
  that to the activation reducer and gates apply, so a caller can validate against
  real hardware and then decline.
  What remains is the piece that cannot be settled offline: resolving a candidate
  into heads. That means naming connectors, CRTCs, and planes for outputs that may
  not be active yet, sourcing correctly sized framebuffers for a mode that is not
  running, and sourcing the previous topology's heads for rollback.
  `native-topology-probe` exists to answer the one question that decides how much
  of that is needed: whether a `TEST_ONLY` modeset requires plane state and a valid
  `FB_ID`, which is driver-dependent. It is read-only — every commit carries
  `TEST_ONLY` and the only framebuffer it names is one the CRTC already scans out,
  so nothing is allocated and no output state changes. It submits the same modeset
  twice, once with connector and CRTC state only and once with plane state added,
  and reports the two outcomes separately. `tools/native_topology_probe.sh` runs it
  as one step: it refuses to start while a display server holds the card, because a
  `MasterUnavailable` run proves nothing, then builds, captures the report, and
  states the conclusion.
  Atomic commits need DRM master even to validate, so the probe reports
  `MasterUnavailable` rather than a rejection when a compositor holds the card, and
  refuses to draw a conclusion.
  **The framebuffer question is answered.** On the AMD two-output reference, from a
  bare TTY holding DRM master, both probes are accepted: 2 connected connectors, 36
  modes, `without_plane_state=accepted`, `with_current_framebuffer=accepted`. A
  `TEST_ONLY` modeset validates with connector `CRTC_ID` and CRTC `MODE_ID`/`ACTIVE`
  alone, so resolving a candidate into heads does **not** need a framebuffer
  allocated at the new mode's size before anything can be checked. Validation can
  precede allocation.
  Getting there required fixing a defect the probe existed to expose. Its first run
  reported both probes rejected with `EINVAL` at matching mode and framebuffer sizes,
  which was not the hardware refusing anything: `LibdrmNativeAtomicCommitRequest`
  defaulted `page_flip_event` true and `test_only()` did not clear it, and the kernel
  rejects `TEST_ONLY` together with `PAGE_FLIP_EVENT` with `EINVAL` before inspecting
  a single property. Every validation-only commit in the tree returned `EINVAL`
  unconditionally, including `submit_native_multi_head_topology`'s `Validate` intent,
  so `NativeOutputCommitExecutor::validating` would have declined every topology and
  looked like hardware saying no. The flag is now derived rather than stored, so the
  combination is unrepresentable, and a deterministic test holds it.
  That is also the reason the report now carries errno and both sizes: the first run
  recorded `Rejected` with nothing to diagnose it by, and a rejection that cannot say
  why is indistinguishable from a bug in the asker.
  A separate power authority now exists: `crates/sophia-engine/src/output_power.rs`
  holds per-output levels for the outputs the desktop currently has. Power is kept
  apart from enablement because blanking a screen and removing a monitor are
  different facts — a dark output keeps its bounds, work area, and surfaces, and
  policy must not see the transition, while a disabled output leaves the complete
  snapshot and forces a relayout. The distinction is easy to lose at the KMS layer,
  where atomic modesetting powers a head down by clearing the same CRTC `ACTIVE`
  that disables one; that is a property of the commit, not a licence to merge the
  two above it. A topology change preserves the level of every surviving output and
  keeps none for a departed one, so a mode change cannot relight what was powered
  down and a reconnected monitor cannot inherit a stale level. Power transitions do
  not travel through topology activation: they alter no geometry, invalidate no
  candidate, and need no rollback beyond the previous level. The KMS write waits on
  the same framebuffer allocation apply does.
  Reservations turned out to be largely present rather than missing. Work areas are
  already re-projected from the new output rects as part of topology publication
  (`owner_loop/topology_phase.rs`), inside the same publication that swaps the
  outputs, so geometry and work area do commit together on the hotplug path. What
  was missing was any test pinning it; a mode change that shrinks an output under a
  live reservation is now covered.
  One fail-open edge is now documented and pinned rather than fixed. Reservations
  are root-relative, so a shrink can leave one outside the new root, and such a
  reservation is filtered *before* reduction — reduction then succeeds and reports
  the full output as available, silently releasing the reservation. An out-of-root
  reservation that arrives malformed should indeed be ignored, and the pure reducer
  cannot tell that case from one that was valid until the mode changed, because it
  holds no previous state. The fail-closed path exists next door: a reduction that
  returns `None` makes callers preserve the previous work area. Closing the gap
  means deciding at the layer that does hold previous state
  (`SurfaceOutputReservationState`), and it changes behavior for every bar, so it is
  called out here rather than settled quietly.
  That decision is now taken, and it rules out both shortcuts. Failing closed by
  preserving the previous work area is wrong after a shrink: the preserved rectangle
  belongs to the larger output and would put policy beyond the screen. Releasing the
  reservation is suboptimal; preserving a stale one is incoherent. Clamping the span
  inside the pure reducer is also wrong, because the reducer cannot tell a span
  clamped by a shrink from one that arrived malformed, and admitting the latter to
  fix the former trades a fail-open edge for weaker rejection. The fix belongs in
  `SurfaceOutputReservationState`: a reservation already admitted against a larger
  root is re-projected onto the smaller one, while a reservation arriving for the
  first time is validated against the current root and rejected if it does not fit.
  Same geometry, different provenance, different answer. Implementation is open.
  Evidence follows that.
  `PolicyRefreshLifecycle.tla` additionally proves that newer dirty
  generations survive an older in-flight refresh and that active output
  settles atomically with the frontend layout. Alloy and Z3 retain operation
  binding and presentation-geometry attacks alongside their protected checks.

#### Multi-Monitor Per-Head Composition Critical Path

The normative design, ownership boundaries, and acceptance rules live in
[Multi-Monitor Per-Head Composition](docs/multi-monitor-composition.md). This
roadmap tracks only the remaining executable slices. Completed mirror,
per-head-planning, topology-transaction, output-IPC, and authority-raster
foundation is summarized in [Roadmap
History](docs/roadmap-history.md#2026-08-16-multi-monitor-per-head-composition-foundation).
Detailed physical-run diagnoses remain in
[Research Log](docs/research-log.md).

- [x] Make accepted core X11 `PutImage` replayable in authority-owned density
  stores. Decode and retain bounded owned pixels, destination geometry, format,
  depth, byte order, GC semantics, and source generation inside X Authority;
  never expose X requests or XIDs to Engine. Raster each requested density
  independently with the document's deterministic rational-edge/area-coverage
  rules. A full opaque replacement may establish a new replay baseline and
  discard older journal commands only when doing so preserves the canonical
  protocol-visible drawable.
  Core and MIT-SHM uploads now retain bounded owned pixels behind a fail-closed
  subset gate: tight ZPixmap depth-24/32 rows, no left padding, unconditional
  GXcopy, full visible plane mask, no clip rectangles. Replay projects those
  retained 1x pixels per density as a per-channel rational area average, so a
  fully covered destination pixel keeps its source color exactly. A full-window
  qualifying upload replaces the journal as a new baseline; a partial one does
  not.
- [ ] Extend replay to cross-drawable `CopyArea` with explicit source drawable
  and generation dependencies. Preserve clipping, overlap, and GC semantics;
  reject stale, destroyed, cyclic, cross-namespace, or over-budget dependencies
  without poisoning unrelated surfaces. Until this is implemented, publish the
  canonical raster with an explicit sampled-fallback reason.
  The explicit reason is in place: a cross-drawable copy now poisons its journal
  with the `unsupported_cross_drawable_copy` cause rather than an unnamed
  fallback. Replay itself remains open, and the re-run gate's causes decide
  whether it must precede a passing unequal-density run.
- [ ] Keep authority raster storage bounded and fail visible. Cover payload,
  command-count, variant-count, and canonical-plus-derived byte limits; late
  density demand; fractional targets; baseline replacement; source destruction;
  and allocation failure. Classify fallback telemetry by cause (including
  unsupported `PutImage`, unsupported cross-drawable copy, stale dependency,
  journal capacity, backing capacity, and transform mismatch) and coalesce
  repeated warnings without hiding counts. Requirement-admission staleness is
  reported as two distinct causes, stale content generation and logical extent
  mismatch, because collapsing them hid which check a physical run had hit.
  Cause classification and coalescing are implemented: an authority-private
  cause accompanies every sampled-fallback outcome, and a bounded per-surface
  coalescer emits the first occurrence and each subsequent power of two with a
  cumulative count. Deterministic coverage exists for unsupported `PutImage`,
  unsupported cross-drawable copy, stale dependency, journal capacity, backing
  capacity, stale content generation, logical extent mismatch, absent
  canonical raster, and transform
  mismatch. Source destruction and allocation failure remain open.
- [x] Add deterministic authority regressions for the real xterm sequence:
  startup `PutImage`, later ImageText8/PolyText8 and line drawing, same-drawable
  scrolling, late 750-density demand, canonical plus derived publication, and
  generation races. Require exact-density output to differ in pixel identity
  where density differs while retaining the same logical content generation.
  Add negative controls proving an unsupported or over-budget command cannot be
  mislabeled as exact.
  The wire sequence drives opcodes 72, 76, 74, 65, and 62 in the traced order,
  then requires 750 and 1000 to publish distinct native-size authority rasters
  with zero sampled fallback. Pixel identity is proven by a source split whose
  boundary does not align with a 0.75 pixel edge, so replay produces boundary
  values absent from the uploaded palette. Negative controls cover XYPixmap at
  the wire, non-copy function, partial plane mask, clipping, absent semantics,
  journal capacity, transform mismatch, and a generation race.
- [x] Re-run the signed unequal-mode mirror gate after the replay slice lands.
  Require DP-1 to select its exact 1000-density variant and DP-2 to select a
  distinct exact 750-density variant for one common logical generation; require
  zero sampled fallback, causal plan/queue/submit/callback/retire records,
  clean suspend, zero abandoned ownership, and an archived verifier-approved
  result. Do not accept visual similarity produced by downsampling the
  canonical head.
  Attempt `0025` satisfies every telemetry condition: both heads select their
  own exact variant for one logical generation, with zero sampled fallback and
  zero stale responses. The original "positive native-size text evidence on
  both heads" clause is withdrawn as unreachable for this workload rather than
  left open, because a fixed 6x13 cell becomes 4.5 pixels at 0.75 density: no
  stem can occupy a whole pixel, so the result is soft however it is produced,
  and thresholding it crisp yields the blocky rendering the same clause
  rejects. A deterministic comparison retains the reasoning — replay keeps a
  one-pixel line fully lit where resampling the canonical raster cannot, while
  replayed and resampled bitmap glyphs land within a few levels of each other.
  Visual acceptance of native-density rendering moves to the extended
  topology below, where a window is rendered at its own head's density and
  nothing is resampled. See
  [Research Log](docs/research-log.md) for the ink-density evidence.
- [x] Prove the same architecture for a mixed mirror-plus-extended topology
  driven through `sophia_output_v1`. This now also carries the visual
  acceptance withdrawn from the mirror gate: a window resident on the
  lower-density head must be rendered at that head's density with no
  resampling, and must read as sharp rather than soft. Unlike a mirror, this
  case is reachable, because the surface is composed for one head only.
  Prefer content with resolution-independent form for that judgement; a
  fixed-cell bitmap font cannot be crisp at a fractional ratio. The privileged output role hosted by the
  shell or selected WM process must independently select each opaque head's
  mode, scale, transform, position, and mirror membership. Ordinary
  `sophia_wm_v1` policy remains logical-output-only and receives no head or
  connector identity. Require spanning-surface coverage, complete candidate and
  rollback ownership, first-presentation publication, head-loss recovery, and
  clean teardown on the physical target.
  The Rust reference policy now hosts the exclusive output role for this gate.
  It negotiates the owner-only socket, accepts only an exact three-connected-head
  snapshot, preserves every current mode, and submits one complete candidate
  containing a two-head mirror group beside one extended group. Extra connected
  heads fail closed instead of being disabled implicitly. In the same supervised
  process, the public-policy client completes exact profile activation and
  configuration, then partitions two proof surfaces across the resulting logical
  outputs using only policy-visible geometry; connector labels never cross into
  blind policy. `tools/run_mixed_output_gate_tty4.sh` retains signed source and
  binary identity and arms the real modeset only from a recovery-safe TTY. One
  client proposal now waits behind an owner-local two-second frame-quiescence
  barrier; ordinary authority and policy intake remain queued while the native
  owner drains existing frames. The evidence verifier accepts an empty first
  DP-2 topology frame, then requires the later exact active DP-2 frame after
  blind policy partitions the two surfaces, correlated through queue, submit,
  callback, and retirement. The next physical run reached that quiescence
  barrier, then exposed a candidate-composition mismatch: provisional topology
  frames used the CPU-only lowerer even though both committed Kitty surfaces
  were retained renderer images. Candidate and rollback planning now reuse the
  ordinary mixed source set, preserving Engine membership and renderer-image
  ownership without a second DMA-BUF import. The following run rendered both
  proof surfaces on the initial large output and left the two initial secondary
  outputs black, but never submitted an output proposal: the proof client waited
  for a redundant scene echo after its committed two-surface proposal and timed
  out. It now starts directly from that committed proposal. Restart acceptance
  is also paused across spawn-to-PID reauthorization, closing the secondary
  unauthorized-peer race exposed by the timeout. The next run reached candidate
  renderer preparation and showed that the new mirror member did not own the
  retained renderer-image IDs created on the original large head. Topology
  preparation now realizes retained images per physical head by restoring a
  compositor-owned snapshot from a live donor before it queues candidate or
  rollback work; a missing donor rejects before KMS. Unchanged preparation
  progress is no longer logged on every owner turn. The following run committed
  the physical candidate and both first presentations, then exposed a stale
  private raster journal when post-publication density demand arrived. Standard
  pixmap and DRI3 Present now invalidate semantic replay, so stale extent or
  unsupported-command demand becomes a bounded sampled fallback instead of an
  X Authority process failure or falsely exact pixels. Signed attempt
  `3d19e2e67cfe2e43eb643d219be11a3251fe7176` then passed the physical runtime,
  archive verifier, and visible-pixel acceptance: two logical outputs settled,
  the extended head's exact draw retired, topology input quarantine released,
  health and cleanup were clean, and the operator confirmed matching mirror
  content plus sharp extended text. Signed head-loss/return source
  `66bc0dd71a40e249eb00cd98f6080cf0f6aa9c54` then passed the physical
  `3 -> 2 -> 3` cable gate: both kernel notices produced changed,
  generation-advancing publications, Hagia policy commitments, later
  presentations, released input quarantine, and clean topology and session
  teardown. That closes the combined item.
- [x] Run one black-box conformance corpus against the Rust reference WM,
  Hagia, the X11 bridge, and the independent C client. This is draft boundary
  evidence while the Triad port is incomplete; it does not publish or freeze
  `sophia_wm_v1`.
  The authenticated behavior host now runs the Rust reference, independent C,
  immutable revision-3 C snapshot, Hagia, and configured public xmonad bridge
  through the same sequential eleven-scenario corpus:
  constrained single output, two-output partition, output loss/migration, and
  generational return, followed by an ordered focus action, timeout discard,
  and successful post-timeout recovery. Stale-scene and invalid-candidate
  outcomes are also discarded before later successful cycles. Rust, C, and
  Hagia additionally run the corpus across two supervised processes. The real
  configured xmonad bridge negotiates profile activation and its action catalog
  over the public wire, then passes the same scenes across five epochs covering
  normal replacement and each noncommitted recovery. The candidate archive
  retains its own C codec, client, schema, and fixed digests; its permanent
  compatibility status begins only when the remaining physical ledger row
  closes and revision 3 freezes.

### 13.5 Migrate And Promote The Native Policy Path

- [ ] Install a bounded Hagia profile through the session-hosted public
  transport and canonical reducer, using only the retained column and
  scrolling layouts. Prove Kitty, Firefox, floating dialogs, work areas,
  ordered repeated actions, pointer move/resize, multi-output views,
  `glxgears`, `vkcube`, policy restart, and clean logout.
  Packaging and installation accept an explicitly supplied Hagia binary and
  publish `Sophia Hagia (Native Policy)` beside Kitty and xmonad recovery
  entries. Promotion means selecting that entry as the remembered ordinary
  session after deterministic preflight; it has no workday-duration
  prerequisite. Every exit must enter the checksummed Hagia ledger as passed,
  recovered, failed, or pending, with cumulative scenario coverage.
  Installed attempt `0001` reached both physical outputs and admitted Kitty,
  then failed closed at `layout_pending`: Hagia proposed `2560x1440` while the
  Engine retained a coherent `1323x1424` recovery extent. The immutable failed
  attempt and exact TTY recovery are preserved. Public proposals now undergo
  Engine constraint reconciliation before reducer staging, the bounded live
  restart regression passes, and a replacement physical attempt remains the
  acceptance proof.
  Installed attempt `0002` then exposed the public admission-ownership half of
  the same boundary: the corrected Manage committed, but remained eligible for
  replanning until visual retirement, producing 3,121 reconciliations before
  startup failed closed. A committed public Manage now consumes only its exact
  planning ownership while retaining the independent visual-retirement fence;
  deterministic tests cap the resulting policy and checkpoint traffic.
  Installed attempt `0003` then completed the physical session but exposed an
  evidence-only manifest collision: the release and attempt recorder emitted
  the same Hagia digest field. The immutable attempt remains failed closed.
  Signed successor `66a279286bddd0354b6022102c4dac5254e34481`
  canonicalizes that field without weakening duplicate rejection. Installed
  attempt `0004` passed exact Sophia/Hagia identity, two-output startup,
  physical actions, presentation, clean normal logout, lifecycle, coverage,
  and archive verification. Hagia is now the ordinary remembered session; the
  item remains open for the named restart, state-transition, application, and
  output-topology scenarios above.
- [x] Remove API v7 and Engine-owned workspace policy only after both freeze
  conditions below hold. The freeze conditions closed first. Configuration now
  accepts only `sophia_wm_v1`; the v7 frame codec, client-hosted socket,
  transport worker, policy reload exchange, Engine IPC error/restart branch,
  demo server/process modes, and obsolete tests are deleted. Shortcut matching
  remains protocol-neutral in Engine. The workspace reducer moved to
  `sophia-x11-wm-bridge`, where it is private compatibility state translated
  into complete public projections. The deterministic xmonad and public-policy
  regressions remain.
- [x] Freeze `sophia_wm_v1` and retain an archived revision-3 client only after
  the retained Triad behavior port is complete and the Rust reference, Hagia,
  X11 bridge, and C client pass the complete black-box reconnect/restart
  corpus. Do not remove API v7, declare stability, or create the permanent
  archived compatibility client before both conditions hold. A digest-pinned
  candidate snapshot may run beforehand, but it is not designated stable until
  this item closes.
  The first condition is defined by Hagia's `docs/triad-port-ledger.md` at Triad
  baseline `fb8fb27e`; `docs/triad-port-ledger-pointer.md` locates it and
  summarizes its 28 classified rows. Twenty-one are complete, none are partial
  or open, and seven are excluded with written product rationales. Signed
  frame-fed archive `0001` closes the physical output gate. Shared
  reconnect/restart, public xmonad migration, and the immutable archived client
  also pass.
  Before it lands, settle the wire decisions enumerated in
  `docs/wm-v1-freeze-surface.md`. Twenty-four of the 28 rows need no wire change;
  the residue was workspace-name projection, broker classification shape, the
  continuous-pointer payload, and the output logical-space contract. All four
  are now settled and normative in `docs/sophia-policy-ipc.md`. The output
  logical-space contract landed ahead of the output-authority tranche that would
  otherwise be the first thing tempted to widen `SnapshotOutput`.
  Broker classification shape is also settled, and it removed a pre-freeze
  obligation rather than satisfying one. Reading Triad's `WindowRule` at baseline
  `fb8fb27e` showed the rules are mostly parametric — a default workspace, a column
  proportion, a named scratchpad, a floating position — which no bitfield or enum
  carries, so the "small closed set in spare `capability_bits`" shape was never
  going to fit. Classifications instead ride a capability-gated extension chunk in
  the reserved `0xFF00`–`0xFFFF` range, which is uncounted and therefore costs no
  `*Begin` layout change. Nothing needs reserving in `SnapshotSurface`, and the
  classification vocabulary is no longer frozen with the revision. This option only
  became available when outbound gating landed and made clause 2 sound; the original
  analysis predates it.
  The generator now rejects any ordinary record declaring a kind in the reserved
  range, so what was a review-time rule about a number is checked.
  The last two are now settled too, both as recommended, so all four wire decisions
  are closed and none of them cost a layout change.
  Workspace names project as `ProjectionIndicator` labels: no field, no record, a
  hard 32-byte UTF-8 ceiling truncated on a character boundary, and a name is never
  an identity — activation stays on the action token so a label cannot become a
  namespace. The retained command surface closes the rest of that row.
  The continuous-pointer payload fixes its vocabulary now and its behavior later.
  `PolicyInteractionKind` gains `Drag` and `Scroll`, and four values is the whole
  vocabulary; the payload rides the existing `interaction_*` fields with the axis
  discriminant in the former `reserved_cause` slot, now named `interaction_axis`,
  scroll using the coordinate pair as its delta and leaving the size fields zero.
  `PolicyInteractionPhase` needs nothing — all
  four phases are already wire-reachable and only `End` is ever constructed. The
  coalescing rule and `Cancel`'s revocation semantics are behavior rather than
  layout, and both stay gated on the lock and security-authority epoch barrier: a
  guessed revocation contract would be worse than a late one.
  The pre-freeze implementation from this analysis is complete. The binding
  constraint remains server-to-client enum vocabularies, not record kinds: an uncounted
  extension chunk in reserved kinds `0xFF00`–`0xFFFF` stays available after the
  freeze, but enum values sit at fixed offsets inside fixed-width records where no
  side channel reaches them.
- [x] Decide the `sophia_wm_v1` forward-compatibility rule. The three-clause form
  is recorded normatively in `docs/sophia-policy-ipc.md` under Versioning: the
  frozen revision is final for record layouts and enum vocabularies; new WM-side
  facts arrive as capability-gated extension chunks in the reserved kind range;
  new authorities take new interface families. Receivers keep rejecting unknown
  kinds because gating guarantees they are never sent one they did not negotiate.
- [x] Build outbound capability gating before the freeze. It is a prerequisite of
  the rule above, not an optimization: without it a frozen client can be sent
  content it must reject. `encode_wm_v1_policy_snapshot` now takes the selected
  capability set and omits governed record kinds along with their declared counts,
  so the transfer stays self-consistent; scene outputs and surfaces stay ungated as
  core semantics. The production caller passes what negotiation actually selected.
  Two tests hold the line, both in `crates/sophia-protocol/tests/policy_semantics.rs`:
  a producer pin asserting that enabling a capability leaves ungated chunk ordinals,
  kinds, item counts, and bytes byte-identical, and one binding the corpora to the
  default set plus each capability in isolation. Clause 2 of the forward-compatibility
  rule is sound as a result: an extension chunk in the reserved kind range reaches
  only a client that negotiated it. Enum widening remains now-or-never, because no
  gate reaches a value at a fixed offset inside a record already being sent.
- [x] Give bounded resources one saturation vocabulary instead of thirty ad-hoc
  choices. `crates/sophia-protocol/src/capacity/` holds the passive types, the
  pure admission arithmetic, the report coalescer, and a driver that owns no
  clock and no thread, so a resource's behaviour at its bound is data beside its
  capacity rather than control flow at each call site. `dod.md` now scopes its
  terminal rule to the pre-admission group FIFO it was always about, and
  enumerates the five dispositions under Saturation Dispositions; the FIFO stays
  terminal, which is what keeps this a classification rather than a general
  softening. `style-guide.md` records the telemetry-schema rule for the first
  time. The X authority backpressure path reuses the shared stall ledger with its
  four existing tests unchanged, which is what proves the step is behaviour-
  neutral.
- [x] Extend `validation/tla/TargetInputPacing.tla` with the producer it never
  had. Device acquisition, bounded deferral, scoped endpoint closure that flushes
  its terminating boundary, and terminal failure are all modelled; 1,969,760
  generated and 567,966 distinct states to depth 21. Seven negative controls each
  violate their own invariant, including the one on `Drain` fairness, so the four
  new actions sitting outside fairness did not turn progress into a tautology.
  Two invariants were removed for failing their own controls rather than kept as
  decoration. Four counterexamples are pinned as deterministic Rust regressions
  in `crates/sophia-protocol/tests/capacity.rs`.
- [x] Apply the dispositions to the session-killing input and presentation
  sites, one revertible commit each. Libinput acquisition defers 50ms and then
  abandons a batch it counts, rather than ending the worker. The seven
  routed-input ingress sites close the endpoint epoch, which `architecture.md`
  already required and which needs no queue reserve because the epoch advance is
  an atomic rather than a queued record; releases still get a bounded wait that
  ordinary input does not. The XKB worker returns `XkbWorkerSaturated` and
  `XkbWorkerUnavailable` instead of panicking in its thread and blocking without
  a deadline. The present-cadence sampler slides a 1024-interval window instead
  of latching an overflow flag that killed the measurement for the rest of the
  session. A lost key-timing sidecar is consumed; a *mismatched* one stays fatal,
  because that means the serial-to-timing association is wrong. A full
  pressed-key ledger closes the epoch and flushes what it holds, which is what
  lets it drain. `input_capacity.rs` and `client_keys.rs` hold the constants and
  the flush group, and `input.rs` came down from 1444 to 1304 lines.
- [x] Drop the session observation batch from that list. It proposed
  reject-and-consume, but `session/reducer.rs` shows the batch is not telemetry:
  every observation drives a phase transition and emits a command, so dropping
  one would skip a frame render or a scanout submission while reporting success.
  The terminal error is correct and the producer-side bound already prevents the
  overflow. See `docs/research-log.md`.
- [x] Fix the frame-service arbitration deadlock that stalled every topology
  candidate. The reducer admitted a present the handler then silently refused,
  while withholding the only effect that drains what the handler refused over;
  neither could advance and no submission reached the kernel. The reducer now
  owns the ordering and each handler refusal is unreachable by construction,
  which also closes a latent session fatal in software staging.
  `validation/tla/FrameServiceArbitration.tla` models the reducer/handler split
  and violates `PresentSettles` when the old reservation is restored. An
  existing test had pinned the defect as correct; it is rewritten as the
  regression. Bound software presents orphaned by topology installation now
  settle, and the quiescence wait escalates once -- skipping runnable and
  waiting presents with `Skipped` feedback, keeping the displayed topology --
  before rejecting a candidate. See `docs/research-log.md`.
- [x] Stop a queued policy cause from being rejected for naming outputs a
  topology change replaced. Holding ordinary cycles for the whole of a candidate
  made queuing across a transition normal rather than rare, so a cause raised
  before the commit reached the projection reducer after it and failed the
  session with `UnknownAffectedOutput`. A cause's outputs are a hint about where
  work is owed, not an identity, so they resolve against the scene the request
  will actually carry; a cause that outlived every output it named still needs
  servicing, because the topology moving is itself a reason to lay out again.
- [x] Keep a policy socket timeout apart from a dead transport. The socket
  carries `SO_RCVTIMEO`, Linux reports expiry as `WouldBlock`, and folding that
  into an I/O error restarted a window manager that was merely slow -- which is
  what a client is after a topology change hands it a whole new layout. Also
  fixed alongside: the non-blocking read path restored blocking mode on only two
  of its four exits, so any other error left the socket non-blocking and made
  the next blocking read fail instantly for the same reason.

Eight consecutive gate failures on this path shared one shape, and it is worth
naming as architecture rather than as eight bugs: **the same fact is derived in
two places from different data, and nothing forces the two to agree.** The
hotplug and policy writers shared one `Quarantined` field; the wait's
precondition was stricter than what installation required; scanout quiescence
and runtime quiescence were separate definitions of the same question; the frame
reducer emitted on `native_phase` while its handler refused on `pending_frame`;
a queued cause's captured outputs outlived the scene they named. The durable
fixes were the ones that made the disagreement unrepresentable -- naming the
quarantine's holder in its type, and having both callers read one predicate --
rather than the ones that merely detected it. These duplications are semantic
rather than lexical, so they cannot be found by searching for a name; that is
why they survived review and only surfaced under execution, and it is what makes
a bounded model asserting `Gate => HandlerPrecondition` the right instrument
where the types cannot be made to carry the agreement.

- [x] Audit the post-commit topology path statically before running it again.
  `mark_policy_committed` -> `observe_presentation` -> `Stable` has never
  executed, so every defect on it is latent, and the physical gate surfaces
  exactly one per manual thirty-second run. Enumerate each guard and every
  fallible step between the commit log and `Stable`, and judge which are
  satisfiable in a normal commit, rather than discovering them one run at a
  time. The audit closed four latent failures before another physical run:
  policy commitment now forces a full repaint after taking its retirement
  baseline; parked hardware snapshots from an equal or older authority epoch
  are dropped rather than ending the session; a second policy topology effect
  cannot begin until the owner is `Stable`; and startup submission requirements
  are keyed and re-derived by opaque head identity when scanout is replaced.
  Deterministic regressions cover the stale-publication decision, back-to-back
  owner transition, and same-count head reordering. The signed head-loss/return
  run remains the promotion proof, not this audit.
- [x] Do *not* collapse `output_topology_preparation_quiescent` (scanout) and
  `topology_rebind_quiescent` (runtime), which an audit showed would break the
  working path. They read as two definitions of one question but answer
  different ones: the scanout asks whether a preparation may *begin*, and it is
  false for the whole of an installed candidate, because
  `install_applied_output_topology` restores the preparation with phase
  `CandidateInstalled` -- which is exactly when the runtime's rebind runs. An
  AND enforced at the rebind would reject every candidate installation. The
  owner's pre-apply wait ANDs them correctly, because it precedes both. Both
  now carry doc comments saying so, since the similar names are what invited
  the mistake. The audit also found the escalation log mixing a blocker
  observed before the skip with a runtime report taken after it, describing a
  state that never existed; both are now read after.
- [ ] Cover `output_topology_preparation_quiescent`, its blocker, and the head
  report with tests. They have none anywhere, and no test calls
  `begin_output_topology_preparation` either, so the scanout half of the
  pre-apply wait is unpinned. `native.rs` has roughly eighty lines of headroom
  before it becomes a twenty-seventh audit error, so new code belongs beside the
  tests rather than in it.
- [x] Drop a queued policy cause whose surface was withdrawn under it. The
  outputs fix was half of the instance: `Focus` and `Interaction` causes also
  name a surface, the projection reducer refuses one that is gone, and that
  refusal ends the session. A cause is queued long enough for this to matter
  only because ordinary cycles are held for the whole of a candidate, which is
  exactly the window in which a surface can vanish.
- [x] Bound the post-commit presentation wait. Confirmed statically that a
  topology commit does *not* force a flip: `reconfigure_output_size` returns
  false when the primary's own size is unchanged, which a three-to-two output
  change need not alter, and the commit recomposes only when it returns true.
  What the wait actually protects is narrower than it first appears -- first
  presentation already put the new *topology* on screen, so the wait is for the
  new *layout*, which is what keeps input from resuming onto stale window
  positions. That makes the stall case the safe case: a relayout that moves
  nothing produces no damage and so no flip, and in exactly that case the
  displayed layout is already the committed one. Since that is
  indistinguishable from a slow client, the wait now expires after two seconds,
  says what was missing, and restores input rather than holding a desktop at
  shortcuts-only forever.
- [x] Wait at the runtime deadline for policy requests already *issued*, and
  drop causes still queued locally. The first attempt folded the whole pending
  count into the key drain, which was wrong twice over: the queue never
  converges, because a moving pointer keeps raising fresh causes for as long as
  the drain waits, and the timeout message printed only the three key counters,
  so a policy-driven expiry reported `pressed=0 pending_deliveries=0
  release_barrier_pending=0` and named nothing. That is the same
  two-facts-one-field mistake this roadmap already describes, committed while
  documenting it. A queued cause was promised to nobody and is dropped when the
  session stops; an issued request is owed an answer. The completion check now
  reads the same count, so a cause raised during the drain cannot fail a session
  the drain just declared clean.
- [x] Drain an outstanding policy request at the runtime deadline instead of
  reporting it as pending work. A deadline lands at an arbitrary instant, so the
  last pointer motion before it can raise a focus request that cannot settle in
  the same tick; ending there discarded the user's final intent and failed a
  session on work that was never stuck. The existing bounded key drain already
  had the right shape, so policy requests are counted alongside held keys rather
  than given a second mechanism, and one that genuinely cannot settle still
  times out and is still reported.
- [x] Let an oversized recovery extent yield to the client's declared
  constraints instead of failing the session, and drop the topology-commit
  release entirely. The extent becomes both minimum *and* maximum, pinning the
  surface to exactly the pixels it had already produced, so on an output that
  cannot hold it no proposal is satisfiable and reconciliation errors out. Two
  attempts to fix that from the commit site were both wrong, for opposite
  reasons: releasing every extent stranded a surface mid-admission, and
  releasing only those no output could hold kept one that fitted the mirrored
  output and was then placed on the extended one. The commit site cannot know
  where the relayout will put a surface; `constrained_size` does, because it
  receives the target bounds. The extent is a courtesy rather than a client
  requirement, so it gives way there and the surface keeps its declared limits.
- [x] Release only the recovery extents no output in the new topology can hold.
  The first attempt released every extent, which stranded a surface still
  mid-admission: its extent was the only size evidence it had, re-priming reads
  `safe_size`, and that comes from a committed size such a surface has not got
  yet, so it never committed and never regained one. The gate showed it
  directly -- `recovery_extent_cleared reason=output_topology_changed` followed
  by a surface that configured eight times and visually committed never. Both
  keeping and clearing are wrong in different cases, so the rule is now the
  narrow one: drop an extent when no remaining output could satisfy it, keep it
  otherwise.
- [x] Release recovery extents at a topology commit. An extent records the
  pixels a client had already produced on the output it was then on, held so
  admission can show real content before the blind WM drives final geometry,
  and it lives until that admission commits. A surface still mid-admission when
  the topology changed kept a 1280x1440 extent -- half of the mirror group --
  and the relayout placed it on the 1920x1080 extended output, where constraint
  reconciliation fails the session outright rather than shrinking the surface.
  The clamp never consults output bounds, so no proposal could have satisfied
  it. Re-priming reads the client's current size and costs one pass, so
  dropping a still valid extent is nearly free; keeping a stale one is not.
- [x] Let the reference policy client tolerate a quiet owner at any time. The
  owner issues a cycle only when policy work exists, so silence is the ordinary
  state, but the client forgave a read timeout only while a topology
  transaction was preparing. Once that settled, the next silent window killed
  the client, and the request the owner sent afterwards had nobody left to
  answer it -- which the session then reported as an unsettled in-flight
  request at its deadline. A closed peer reports itself differently and still
  ends the loop.
- [x] Give clients a realistic window to acknowledge a layout. The reference
  policy asked the owner to hold one open for 300ms, which is a property of the
  clients being tiled rather than of the compositor: a topology change can
  double a surface's width -- 1280x1440 to 2560x1440 when two heads merge into
  one mirrored output -- and a terminal answering that reallocates its buffers
  and re-renders its whole grid while the compositor drives three heads. At
  300ms the same code converged with no timeouts on one run and timed out seven
  times on the next, rolling the layout back each round and never settling.
  This is tuning, not a defect fix: the compositor delivered every configure
  correctly on both runs.
- [x] Find why no present reaches "stable", which is what startup readiness
  waits on. Neither of the two candidate causes was it: starvation was ruled
  out by the eleven flips being spread across the run, and no present was
  overtaken before being displayed. The predicate asked for the wrong thing.
  Besides requiring that this transaction be displayed with real pixels -- the
  only part that is about the flip -- it required that nothing newer be
  submitted and that no head be busy, which is a claim about the instant of
  judgement rather than about the frame. Judgement happens after a whole
  `service_native` pass, and that pass submits the successor before it returns;
  a mirror group's retirement promotes the coalesced successor into every
  head's exporter slot while the retirement is still being reported. Those
  conjuncts were written when the frame reducer reserved the primary, and that
  reservation was the arbitration deadlock, not a source of quiet.
  `PresentFrameOwnership.tla` already permits a successor submitted after a
  retirement is observed, so the code had been stricter than its own model. A
  present is now stable when its page flip retired with it displayed and
  carrying nonzero pixels. Telemetry moved to `schema=2`, dropping a
  `pending_primary` field that was `!stable` under a name suggesting an
  independent fact, in favour of the pixel count.
- [x] Spend the composition pixel proof where light could actually appear. With
  the predicate narrowed, the next run reported `nonzero_rgb_pixels=0` on every
  retirement while naming the right transaction, which located the last term.
  That count is not this frame's: a context reads its composition back at most
  three times and keeps the last result, because a full-framebuffer readback
  cannot run per frame. All three attempts were spent in the first hundred
  milliseconds on compositions with zero layers -- clears to black, which prove
  nothing -- so the zero they latched rode every later present and readiness was
  unreachable by construction. An attempt now requires a composition with at
  least one layer, the budget is a named constant instead of a literal in three
  places, and the stamping site says what the value is: the head's proof that it
  has shown light, not a measurement of the frame carrying it.
- [x] Stop ending the session when it revokes its own input. Closing the input
  security epoch for an output policy change strands whatever was in flight, and
  both paths that do so -- draining frozen input, refusing an event stamped with
  the closed epoch -- reported `RouteRejected`, which the owner loop treats as
  fatal. A live run died four seconds in because the pointer moved during the
  topology change. This is the timeout-is-not-a-fault distinction again:
  `EpochRevoked` now names the session's own boundary and retires like a
  departed target, while `RouteRejected` keeps meaning a fault and keeps ending
  the session.
- [x] Compose the scene for every output, not for the primary one. The gate
  reached its telemetry stage -- readiness at 256ms, topology committed and
  settled, health clean, presents stable with real pixel counts -- and failed
  because the extended head never showed a client pixel. The presentation
  layout filtered every layer through `surface_visible_on_output(layer,
  output.id)` against the primary output alone, which is harmless with one
  logical output and wrong with two: the surface the policy placed on the
  extended output left the scene. A staged Present is released only when its
  surface is in the projection, the surface enters the projection only when it
  is composed, and the resize that places it commits only when that Present
  retires, so nothing moved again. The head stayed blank and its client, still
  waiting on that Present, stopped drawing -- which is why the terminal looked
  dead to the keyboard even though the keys were routed and flushed.
Mirror work is sequenced deliberately, because every item below changes either
the logical size of an output or the pacing of its heads -- the inputs whose
churn produced the last several gate failures. Take the physical gate to
`status=passed` first, then the optimized-head choice (contained, no new model
surface), then per-head pacing (model first), then mode matching (its own gate
run). Correct pacing outranks sharper mirroring: the first is wrong on ordinary
hardware, the second only decides which screen looks best.

- [x] Let a mirror group's heads flip at their own refresh rates. One generation
  is held until every required head has flipped
  (`LiveProductionMirrorGroupLifecycle` completes on the `flipped` set covering
  `required`), so mirroring 144Hz beside 60Hz would run both at 60. Mixed
  refresh across separate logical outputs is already independent -- each head
  owns its CRTC and its own vblank -- so this is a mirror-group question only.
  Each head should take the newest ready generation on its own vblank and
  coalesce what it missed, with the group's primary head owning present
  completion; throttling a client's frame callbacks to the slowest panel on the
  desk is worse than a mirror that briefly lags. Three things must move
  together: buffer lifetime becomes per-head (a slow head may still be scanning
  a generation the fast head has left, so release waits for every head that
  scanned it), which is frame-slot semantics and so is modelled before any code
  -- `validation/tla/MirrorHeadPacing.tla` now states the successor rule, and
  restoring today's joint advance in it violates `PrimarySubmitNeverBlocked`,
  which is what the fast head being throttled looks like in the model;
  `stable_present` stops
  quantifying over every head of the output and names the primary; and the
  mirror gate's matching-content criterion must permit the bounded lag or it
  will fail correct runs. The Rust slice is now implemented: topology carries
  an explicit primary per logical output, the prepare-all cohort completes
  logical presentation on that primary, each head independently submits the
  newest complete generation and coalesces stale unsubmitted work, and native
  owners release only after the last scanning head moves on. The mirror and
  mixed verifiers require ordered `primary_presented -> released` evidence, and
  their visual prompts judge convergence only after motion settles. Engine,
  backend feature, verifier-fixture, and TLA checks are local prerequisites;
  signed source `e946cc725bf731515a477c86e9a575554965418c` passed and
  independently re-verified mirror archive `0007` and mixed archive `0001`,
  including ordered primary-presentation/last-head-release evidence on the real
  mismatched topology.
- [x] Make the optimized head a property of a mirror group, as macOS does. The
  choice turned out to be expressible already: a group proposal carries one
  logical rect and a mapping per member, so optimizing for a head is sizing the
  group to that head's mode and marking it `Exact` while the others `Fit`. The
  compositor needed nothing -- it already plans each head against the group's
  logical size. The reference policy now takes the choice as a parameter and
  the gate exposes it as `SOPHIA_MIXED_OPTIMIZE_FOR_LABEL`, defaulting to the
  primary, which is what every run so far did implicitly. Note this also makes
  the group's size follow the optimized head's mode rather than whatever rect
  the group already had; those coincide on this rig and the former is the
  meaningful one.
- [x] Give the pixel evidence a population that reads intensity. The plan to
  filter in linear light rested on a claim that turned out to be false: that the
  composition-region pixel populations already reported would show the change.
  They could not. Those buckets key on which channels are lit and never on how
  brightly, deliberately, so that a palette check survives an intensity
  conversion and still exposes a channel swap -- the property that makes them
  good at their job makes them blind to this one. Gamma moves intensity and
  nothing else, so every one of them would have held still while the pixels
  underneath changed, leaving only `checksum`, which proves a difference without
  saying which way it went. `luminance_sum` and `luminance_buckets` were added
  for it, with integer weights summing to 256 so the shift never rounds and the
  numbers stay as reproducible as the checksum beside them. Judge a filtering
  change on the histogram, not the sum: a mean holds still while the population
  behind it splits, which is exactly the shape gamma-space filtering makes.
- [x] Filter in linear light. Every filter weight was applied to gamma-encoded
  bytes as though they were light, which is the ordinary cause of muddy edges on
  resampled text: averaging the encoded bytes 0 and 255 gives 127, about a fifth
  of the light of white rather than half of it. The reconstruction shader now
  decodes each tap before weighting it and re-encodes the sum once at the end,
  under gamma 2.0, matching the transfer function `software/raster_replay.rs`
  already chose for the CPU path and for its stated reason.

  Both directions, from one program. Catmull-Rom is an interpolating kernel, so
  it is the textbook bicubic upsample as well as a reduction filter; the upscale
  path stopped falling through to a hardware bilinear and no new kernel had to be
  chosen. The sampler is `NEAREST` for every reconstructed draw, which is the
  part that is easy to get wrong invisibly: a hardware `LINEAR` filter would
  blend the texels in gamma-encoded space before the shader ran, and the evidence
  would still read `sharp_downscale status=active`. The filter and the program
  are now one decision taken in one function rather than two that could disagree.

  Premultiplied sources are unpremultiplied across the decode -- `v*v/a` in and
  `sqrt(L*a)` out -- and alpha is never transformed, being coverage rather than
  light. The clamp precedes the encode because Catmull-Rom rings negative and
  `sqrt` of a negative reaches the screen as a hole. Three negative controls
  were run against the shader and the filter policy; each fails the intended
  test when reverted.
- [ ] Give the upscale direction a real kernel. Superseded in part: Catmull-Rom
  is itself the textbook bicubic upsample, so the linear-light change serves
  both directions from the one program that already exists and the upscale path
  stops being a hardware bilinear of encoded bytes. What remains is the question
  of whether bicubic upscale is sharp enough -- Lanczos-2 first, small and
  predictable on glyphs with no vendored source, with FSR 1 EASU or NIS as the
  alternative if photographic content proves to matter more than text. Both are
  MIT and self-contained, but either would be the first third-party shader in
  this tree, which is a provenance decision rather than a detail. A sharpen-only
  post-pass is separable and optional; contrast-adaptive sharpening rings on
  glyphs that were already crisp. None of this may change the reported sampling
  class.
- [x] Keep the GLSL in its own files and compile it before a GPU sees it. The
  shaders were string literals in Rust, which is workable until you notice that
  a shader failing to compile is not fatal by design: the pipeline logs
  `status=unavailable`, falls back to the direct program, and the session runs on
  with its filtering silently uncorrected. Right at runtime, wrong as the first
  place a typo is discovered. `tools/check_shaders.sh` compiles every
  `.vert`/`.frag` under `crates/*/src` with a real GLSL front end; it refuses to
  run without a validator and refuses a run that matched no sources, since a
  search over nothing would otherwise report success. Front-end only -- it does
  not know a driver's limits or whether a uniform was bound.

- [x] Bound head-native content by the scene, not by the framebuffer. Every
  mirror policy until centre-unscaled projected the scene across the whole head,
  so "the framebuffer" and "the region the scene occupies on it" were one rect
  and clipping to either gave the same answer. A scene placed inside a border
  separates them, and content bounded by the framebuffer paints into the margin
  that is meant to hold background alone. Borders showed it first because they
  are bright lines and because they were not clipped at all -- surfaces and the
  cursor were clipped, just to the wrong rect.

  Border bands are clipped one at a time rather than by clipping the `outer` and
  `inner` rects they are derived from. Those are not the same operation: where
  the clip leaves the two degenerate their difference is still positive, so a
  window lying entirely outside the scene keeps a band at its original
  off-screen coordinates. The first version of this note claimed the wrong
  failure mode -- an invented band at the clip edge -- and enumerating the cases
  showed it does not happen; the surviving off-screen band does.

- [x] Relay out for a public policy when the work area moves. The check sat
  below the early return into the public path, so it ran only for a private
  policy: three places set `work_area_relayout_required` and one read it, and
  that reader was unreachable in every session running a public policy client --
  which is every session running the reference WM. `enqueue_relayout` opens by
  handling the public case, so the capability was always present and only the
  ordering kept it from being reached.

  What it cost was window chrome. Chrome clearance going from zero to two raised
  the flag when a focus ring first appeared; nothing consumed it, so windows
  stayed placed against the old clearance while the ring was drawn against the
  new one. A ring is drawn outside the window geometry, and the window filled its
  output exactly, so the ring landed wholly outside that output. The only part
  visible anywhere was the sliver crossing into a neighbouring output in root
  space -- one monitor's window border appearing on another monitor, and none on
  its own.

- [x] Convert a public policy's placement into content geometry without losing
  the outer allocation that blind policy owns.

  The defect is real and located. A public policy's placement is an outer
  allocation, chrome included -- the code says so in a comment directly above the
  line that assigns it into the layer as content geometry. The private path
  converts through `apply_surface_chrome_clearance`; the public path assigns it
  raw, so every surface it places fills its whole allocation and the chrome drawn
  around it has nowhere to go. A focused window allocated an entire output puts
  its focus ring wholly outside that output, visible only where it crosses into a
  neighbouring one, which is exactly what the rig shows.

  The conversion reached the geometry -- a surface was observed at 1916x1076 --
  and the session then failed. Layout timeouts went from one per run to seven,
  and the session ended with `wm_layout=1` pending. Adding a content-size request
  alongside the converted extent did not help, which is the fact that matters:
  the second attempt assumed the client was never told to resize, and it made no
  difference, so that is not the whole reason either.

  The missing boundary was transaction ownership. Reconciliation now carries
  both projections: the outer geometry remains the policy/reducer value, while
  chrome clearance is applied before layout-epoch reconciliation and produces
  the content geometry plus any client configure request. The public proposal
  materializes that content projection, but the reducer stages and commits only
  the outer projection after layout settlement. An omitted policy configure may
  therefore still generate the content resize that chrome requires without
  making an unacknowledged geometry current. Schema-2 chrome records expose the
  reconciled, acknowledged, and settled stages, and regressions cover the
  outer/content split, generated configure, and post-settlement materialization.

  Signed mixed attempt from `7ff94e20` proved the projection arithmetic and
  exposed one remaining ownership race. A second public relayout arrived after
  the first resize epoch had installed its content geometry and armed exact
  pixels, but before one surface's native retirement committed its size. The
  staging filter compared only with the last retired size, treated the identical
  target as new, and sent a second configure that displaced the armed candidate.
  An installed target already owned by `ResizeVisualCommitTracker` now removes
  only that duplicate resize request. A standing recovery target whose geometry
  is not installed still configures normally. Both sides have deterministic
  regressions. The signed `3d19e2e6` rerun passed runtime, telemetry, and visual
  confirmation; this resize race is physically closed.

- [x] Give the composition-region trace a head. The line carried a rect and no
  output identity, and on the three-head rig two heads compose a `1920x1080_0_0`
  rect -- DP-2 extended at `mapping=exact` and the DP-3 mirror member at
  `mapping=fit`. Their records are indistinguishable, so a before-and-after keyed
  on the rect compares whichever the collector happened to keep, and the first
  read of the linear-light verification was byte-identical for exactly that
  reason: it was comparing the unfiltered head with itself. The correction was
  confirmed on the 2560x1440 regions instead, whose geometry only one head has,
  which is luck rather than method. `trace_final_composition_region` takes the
  pipeline, stage, layer index and rect; the output identity has to reach it.
  Same class as the raster/presentation split -- a record that does not carry the
  fact needed to read it. `LiveCompositionTrace` now carries output, opaque head,
  and scene generation through lowering into the native renderer. Sampling
  evidence is schema 3 and keyed by all three, so the verifier correlates the
  exact draw between that head's queue and submit records instead of selecting
  an arbitrary equal-sized region.

- [ ] Filter the blend in linear light too, and the opacity multiply with it.
  The fixed-function ROP mixes `dst*(1-src.a)` on gamma-encoded destination
  bytes, and no shader can reach it: it needs `GL_FRAMEBUFFER_SRGB` and an
  sRGB-capable EGL surface, neither of which has been probed on this hardware,
  and imported client textures take their format from EGLImage rather than from
  us. The shader's `rgb * opacity` and its `min(rgb, a)` ringing clamp stay in
  encoded space alongside it deliberately -- moving one without the other would
  leave them inconsistent, so they travel together as one later change.
- [x] Offer centre-unscaled as a third mirror sizing policy. Both optimized
  policies make one panel pixel-exact by making the other stretch; this one gives
  that up so neither does, at the cost of an unused border on any head with room
  left over. It is the only policy under which both panels are exact at once.

  The compositor needed nothing. `OutputHeadMapping::Exact` already takes the
  logical size verbatim and the projection already centres it, so "this head owns
  the size" and "this head shows the image unscaled in a border" are the same
  placement; what differs is only whether there is a remainder. The policy
  chooses the size and the mapping follows, which is why this landed as a value
  of the reference client's sizing choice rather than as a compositor change.

  Two things it forced into the open. The size is the per-axis minimum across the
  group, not whichever member is smaller: two heads need not be ordered, and
  taking either mode whole would run the other's image past its edge where
  clipping crops rather than borders it -- a policy promising nothing resamples
  would have silently lost pixels instead. And the applied-topology predicate now
  reads the logical size, not the member mappings alone, because two exact
  members sized to the larger head are exactly that cropping configuration and
  wear the same pair of mappings. The type was renamed with it: a value that
  optimizes for neither head cannot live in a type called "which head is
  optimized" without one of its values meaning something else.

- [ ] Decide whether mirror members should be re-moded rather than resampled.
  Windows Duplicate and X both refuse to scale: they restrict the desktop to a
  mode every member supports, so each panel scans out natively and the larger
  one runs below its own resolution. That is a different knob from the
  optimized head -- one chooses which size the desktop is rasterized at, the
  other chooses whether members change mode to match it -- and it touches the
  topology candidate and rollback machinery, so it wants its own gate run.
- [ ] Watch whether the public policy transport restarts on quiet rather than
  on failure. A mixed run restarted the reference WM with
  `reason=public_transport_failed error=TimedOut` after twelve seconds of
  silence from a policy client that was alive. Restarting a wedged policy is the
  designed recovery and it worked -- layout preserved, session continued -- so
  this is not being loosened without evidence that the peer was healthy. It is
  worth distinguishing if restarts recur: what the owner was waiting for is not
  in the line, only that it waited.
- [ ] Demand rasters at the extent that was presented into, not at the raster's
  own extent. With a raster now stating its own size, `raster_requirements`
  briefly asks the authority to re-raster at a stale client's extent instead of
  the extent it was asked to fill. It self-corrects on the client's next frame,
  and fixing it properly means carrying the presentation extent on
  `CommittedSurfaceState` as well.
- [x] Classify sampling by extent, not by density alone. `sampling_class`
  compares `density_millis` against the projected density and never looks at
  whether the raster spans the geometry it is placed into, so a stale frame
  scaled up still reports `Exact`. The mixed-output gate's sharpness criterion
  reads that field, which means it is currently provable by a frame that was
  visibly resampled.

  The full shape of it, found while planning the linear-light work. The engine
  and the renderer each decide independently what a draw is: the plan compares
  two `density_millis` scalars, while `native_composition_sampling` compares the
  real source and target rectangles. Nothing connects them and nothing forces
  them to agree -- the parameter the renderer calls `requested_sampling` is its
  own geometry-derived value, a homonym of the engine's field rather than a copy
  of it. They diverge in four separate ways: content whose `logical_extent`
  differs from its placed geometry, which is the ordinary condition of any
  surface mid-resize; plain rounding, because the projected density truncates
  while the expected pixel size ceilings and each projected edge truncates
  independently, so any scale that is not dyadic-clean lands a pixel off; mixed-
  axis projections, where one scalar cannot describe an upscale in x and a
  downscale in y; and retained renderer images, which are exempt from the
  source-size check by design and so are classified from a number that is not
  the one drawn. A plan line reading `mapping=exact exact=1 downsampled=0
  upsampled=0` -- the exact string the gate greps for -- is therefore satisfied
  by a frame the GPU resampled, and the one renderer-side check in that script
  rejects `status=fallback|unavailable` while passing
  `requested=sharp_downscale status=active`. Secondary consequence: the plan's
  one-pixel repaint dilation for filter footprint is gated on the plan's class,
  so it is skipped exactly when the renderer is in fact filtering. It is the
  same defect class as the raster/presentation extent split -- one fact derived
  in two places with nothing forcing agreement -- and the fix is to make one
  derivation the source and the other read it.

  Engine now classifies each axis from realized source pixels and native target
  pixels, including an explicit mixed-axis class. Lowering recomputes that class
  after the actual retained image extent is known, passes it unchanged to the
  native renderer, and records the realized source extent in damage evidence.
  The mixed-output verifier requires a schema-2 plan with zero mixed draws and a
  keyed schema-3 renderer `exact` draw between queue and submit; legacy or
  fallback-only sampling lines fail closed. Exact, upscale, downscale, mixed,
  stale-retained-image, and verifier-mutation regressions pass. A signed physical
  candidate is still required before the mixed gate is promotion evidence.
- [ ] Apply the timeout-is-not-a-fault distinction to the two remaining socket
  transports. Four places set `SO_RCVTIMEO`/`SO_SNDTIMEO`; the session's policy
  transport and the reference policy client are now fixed, while
  `sophia-runtime/src/output_transport.rs` and
  `sophia-wm-demo/src/output_v1.rs` still treat an expired window as an I/O
  failure. Neither has bitten yet because the output role completes early, but
  the class has now cost two separate sessions.
- [x] Force a topology commit to repaint rather than
  relying on the relayout to generate one. The first full run settled naturally
  -- presentation baseline 26, retirements 27 -- so the relayout did produce the
  flip, and the bounded wait added alongside never fired. Nothing guarantees
  that, though. With `policy_required` set, `mark_policy_committed`
  replaces the presentation baseline with the live retirement counter, so
  `observe_presentation` then needs a strictly *new* flip -- and neither of the
  two things the commit touches produces one. `scene.reconfigure_output_size`
  is conditional on the primary's own size changing, which a three-to-two output
  change need not do, and `cursor_updates.dirty` drives the cursor plane. If the
  relayout's flip lands before the pass that snapshots the baseline, the owner
  parks in `AwaitingPresentation` with no watchdog, deadline, or retry, and
  `input_quarantined()` holds routing at `ShortcutsOnly` for the duration:
  pointer motion dropped, cursor frozen, every non-shortcut key discarded. The
  baseline overwrite is deliberate and the invariant behind it is right -- a
  presentation that retired before the projection committed cannot release the
  quarantine -- so the fix must make the flip happen rather than weaken the
  requirement. `mark_policy_committed` now clears the CPU damage baseline and
  queues one full repaint after taking that baseline, making the required newer
  presentation causal instead of timing-dependent.
- [x] Give `pending_hardware_output_publication` the same treatment as the
  policy cause. It is parked across a candidate by design and its topology-epoch
  check is a hard error (`StalePublishedSnapshot`), so a snapshot parked before
  a commit that advances the epoch ends the session rather than being resolved
  or dropped. Publication now compares against the current authority topology
  epoch and drops an equal or older parked snapshot with a named record.
- [x] Guard `begin_policy_change` at its call site on the owner being `Stable`.
  There is no phase check there, so a second `sophia_output_v1` proposal
  promoted while the owner is `Published` or `AwaitingPresentation` fails with
  "live policy topology change overlaps another transition". Promotion now
  waits for `Stable`; a regression proves a second candidate cannot begin until
  post-commit presentation settles.
- [x] Re-derive `startup_required_submissions` when the scanout is replaced. It
  was positional over head indices with absolute submission counters, and only a
  length check guarded it, so a same-count reordering compared against the wrong
  head and a replacement scanout made the requirement permanently
  unsatisfiable. Requirements are now a map keyed by `RenderHeadId`, validated
  against exact head coverage, and rebuilt from the replacement scanout's head
  counters and focused geometry.
- [ ] Stamp work queued across a topology transition with the epoch it was built
  in, and revalidate or rebuild it on mismatch, rather than fixing instances.
  An inventory found the class runs wider than the two already fixed: absolute
  geometry in `session_controls` configure packets and in `Interaction` causes,
  pre-transition pointer coordinates released from `pointer_focus_handoff`, and
  the legacy WM's `queued_requests` carrying enqueue-time output and bounds --
  all of which produce a wrong result silently rather than an error. Follow the
  precedent already in the tree: `ApplicationRouteLease` stamps output,
  presentation epoch, authority session epoch, and control epoch, and validates
  all four at use with typed refusals instead of a session error.
- [ ] Bound the page-flip callback *read* rather than its write, the last
  session-killing site. The event is already consumed from the DRM fd when the
  queue is examined, so a write-side degrade would lose a retirement outright.
  Land it alone and last: it restructures the most physically coupled code here,
  and `fake_source.rs` should give deterministic coverage first.
- [ ] Close the remaining silent drops once the fatal sites are converted: the
  three `broker.rs` swallow points, the unbounded input-delivery channel, the
  deadline-free route-lease send, and config-time bound publication. Add
  `discarded = 0` assertions to the gate verifiers so a degraded run still fails
  rather than passing quietly, which is the one risk converting fatals to
  degradations actually creates.

Milestone 13 exits only when the public wire is independently implementable,
the retained Triad behavior port is complete across the correct authorities,
the formal and deterministic gates pass, installed Hagia uses the Engine
projection path, and a policy crash or replacement preserves the last coherent
desktop.

---

## Milestone 14: Native Graphics Efficiency

This milestone starts after installed Hagia is the usable ordinary session and
the revision-1 compatibility gate passes. It does not wait on elapsed dogfood
time. It optimizes the same native-X product; XLibre, Xorg, niri, river, and
other mature compositors are references rather than Sophia runtime components.

- [x] Extend the bounded visual-retirement model before changing frame-slot,
  coalescing, multi-output, shared-worker, direct-scanout, or buffer-lifetime
  semantics. Check out-of-order output retirement, supersession, fallback, and
  release safety. Retain a deterministic Rust regression for every
  implementation-relevant counterexample.
- [x] Recycle three generational frame-surface slots per output through
  explicit page-flip retirement, with bounded deferral when all slots are
  leased. Promoted on signed native archive `0001`: requests settled as
  completions plus bounded deferrals with no stale release and no slot leased
  at completion, and both presented heads reached full three-slot occupancy.
- [x] Carry bounded buffer-age damage history per slot and repaint only
  accumulated damage. Fall back to a full repaint whenever history is
  incomplete. Promoted on signed native archive `0002`: 129 partial repaints
  beside 627 full fallbacks, 201 history records with zero invalidations, and
  the slot ledger still exact at 201 requests settling as 201 completions with
  no stale release and no slot leased at completion. The model boundary came first
  (`VisualDamageHistory.tla`, negative controls for under-computed damage and
  a rebuilt bundle keeping its recorded generation). The content age comes
  from `EGL_BUFFER_AGE_EXT` per acquired back buffer, answering the brief's
  open question; a monotonic bundle generation catches every rebuild path; and
  every path that cannot prove an age falls back to a full repaint with a
  named reason. `tools/check_buffer_age_equivalence.sh` proves on this host's
  GPU, through a render node only, that a damage-limited render is
  byte-identical to a full one across a twelve-frame sequence with real
  partial repaints, and that a lying damage table is caught by the same
  comparison. The feature is opt-in via `SOPHIA_ENABLE_BUFFER_AGE_DAMAGE=1`;
  the native gate exports it as the promotion step, and its verifier requires
  schema-8 evidence in which at least one frame rendered partially. Tick this
  box when that gate passes.
- [x] Keep one latest pending frame and one KMS submission in flight per head,
  and prove the 2026-07-31 stage contract at p99 against the measured refresh:
  full chain below two refresh periods, queue dwell within 1 ms,
  dwell-to-submit within one refresh, and submit-to-page-flip within one
  refresh plus one millisecond of commit-and-completion jitter -- a press
  arriving just after a vblank waits the full period, and the atomic commit
  and completion event add time no pipeline can remove, so the flip stage's
  bound carries the allowance explicitly rather than pretending the period
  alone is achievable. Stage percentiles come from the pooled in-session
  press distributions, never from the one-shot correlation, whose stage
  split measures gap phase once presses are spaced.
  This row originally said "half a refresh period at p99" for the full chain.
  That is stricter than the one-refresh bound the 2026-07-31 entry already
  rejected on physical evidence -- a randomly phased input can spend nearly a
  whole refresh waiting for the next synchronized flip -- so the sentence is
  superseded rather than reinstated. It also said per output; a mirror output
  holds one submission per head by design, which is what `MirrorHeadPacing`
  authorizes.
  The mechanism was already enforced and model-checked
  (`FrameServiceArbitration`'s `OneSubmissionInFlight`); what was missing was
  evidence, now `max_in_flight_per_output` and `pending_frame_supersessions`
  in `sophia_live_native_resources` schema 9. Input-to-photon sampling no
  longer latches after one correlation, so a session carries its own
  distribution, and the reporter takes p99 over that population against the
  refresh the session recorded, refusing below two hundred samples.
  Proved on source `96b00d0d`, physical run `20260828T231430Z`:
  thirty-five sessions, two hundred forty-five independent presses, zero
  page-flip stalls, `status=passed failed_gates=none` -- full chain p99
  24 ms against the 34 ms two-refresh budget, queue dwell 1 ms,
  dwell-to-submit p99 7 ms against one refresh, submit-to-page-flip p99
  18 ms against one refresh plus the named jitter millisecond. Getting the
  measurement honest was the row: the correlation was accepting a flip
  carrying a composition older than the press, readiness was accepting a
  pre-content picture on glass, and the reporter was gating stages on one
  press's vsync phase. Each fix is pinned by regressions that reproduce the
  recorded defect shape. Evidence under
  `rendering-benchmarks/96b00d0d*/input-latency/20260828T231430Z`.
- [x] Coalesce all outputs in the same DRM/render-device group onto one shared
  renderer worker. Preserve one latest pending request per output, bounded
  response demultiplexing, explicit per-output retirement tokens, and bounded
  inter-output service skew under concurrent producers.
  Promoted on signed native archive `0003`, Sophia source
  `8806046462bdd2f8c23c2702427e0d8b9fd7cd1b` against Hagia
  `9c9a59061fd0d8e88310b764f7dd240e729fb035`: two heads of one card on one
  renderer thread, `renderer_workers=1` with `worker_result_misroutes=0`,
  `worker_max_service_skew=1` inside the one-per-sibling bound,
  `max_in_flight_per_output=2` for two presented heads, 207 worker requests
  settling as 207 completions with no deferral, no stale release, and no slot
  leased at completion.
  The model came first (`SharedWorkerService.tla`, three load-bearing negative
  controls), and it refused two drafts before it held: skew compared service
  counts directly, which fires on an idle head that simply has nothing to
  draw, and the environment forbade composing during an in-flight render,
  which is what the code does and whose absence made the submission gate's
  control unable to fail.
  The renderer context was the load-bearing change. One three-slot array
  serving two outputs rebuilds a bundle on every alternation between their
  sizes, so slots and the pixel proof taken from them are keyed per output;
  results are routed on a channel per output rather than correlated by
  position, and the misroute check is kept anyway. Renderer images are now
  imported once per device rather than once per head.
  Two instrumentation defects surfaced only under real evidence.
  `max_in_flight_per_output` read a field only the mirror path sets, so it had
  been structurally zero for every non-mirror session since schema 9 landed --
  this row's central claim had never actually been measured on an extended
  desktop. And the output key was unique only by an argument about scope,
  which a two-card guest reporting `head=1` twice disproved; it composes group
  with head now, and duplicate registration is refused rather than trusted.
- [x] Add atomic-test-gated direct scanout for one compatible opaque DMA-BUF
  layer. Engine proves the exact frame has no overlay or active effect that
  samples the scene, uses an offscreen group, or otherwise requires
  composition; the backend re-derives the same structure from the lowered
  layers and refuses on any disagreement. `PresentFlipOwnership.tla` owns the
  ownership half: the displayed buffer is released only by a successor's
  retirement, and the successor may be a composed frame. Mixed composition is
  the fail-closed fallback, and a refused validating commit, a rejected real
  commit, or any prepare failure re-queues the same content composed rather
  than reaching the terminal submit-failure path. ARGB8888 is offered where the
  layer covers the whole head, because nothing is behind it there and the
  atomic test still decides.
- [x] Promote the direct-scanout run under the archive discipline.
  Direct-scanout archive `0001` binds a signed commit with the Sophia binary,
  the client whose buffer reached the plane, and both configurations the
  session loaded: thirty-eight client buffers on the plane from one validating
  commit, no test rejections, no proof disagreements, no unsupported formats,
  no fallbacks, and a clean retirement and session end.
- [x] Prove that effect or overlay activation returns a directly scanned output
  to composition without a lost or stale frame, and that later removal restores
  eligibility only through a fresh Engine proof and a fresh backend atomic
  test. Direct-scanout archive `0002` promotes it: ten direct flips, an
  overlay opened by the session's own proof control through the same
  `set_descriptor_overlay` entry the shell uses, a composed successor built
  from the client's still-held planes and retired inside the window with the
  displaced buffer's snapshot promoted before its release, withdrawal, a
  second validating commit, and flips resumed to a clean bounded completion.
  Getting there fixed three boundary defects the offline suite could not
  reach: the retained requeue sourced a never-imported renderer image for a
  direct frame, the lowering carried an eligibility verdict that stopped
  being true when it substituted the snapshot, and the conformance readers
  anchored records to the line start and so had never actually run the
  episode-order rules against decorated hardware evidence.
- [x] Measure whether a direct frame costs less than a composed one. It does,
  by roughly half at the median and by three orders of magnitude in the tail.
  Direct-scanout archive `0003` measures both populations on one head in one
  session -- direct flips outside the overlay window, composed frames inside
  it -- with the client's buffer offered to the plane in 17 microseconds at
  the median against 35 for a composed frame, and a p99 of 22 microseconds
  against 12,883. The tail is the whole finding: a direct frame's cost is
  nearly constant because nothing is drawn, while a composed frame waits on a
  renderer that occasionally takes twelve milliseconds. Submit-to-flip is not
  the point but is measured beside it to check the assumption that the
  display engine does not care how the buffer arrived: 7,972 microseconds
  against 15,099 at the median, both dominated by this host's chronic DCN32
  flip stalls rather than by anything Sophia does, and neither population is
  penalised for arriving directly.
  The stated blocker turned out to be about the wrong harness. Input latency
  needs an input proof, whose session runs xterm, which never presents a
  DMA-BUF -- but this question was never about input, and the standalone
  probe whose eligibility archives `0001` and `0002` already proved answers
  it directly. No threshold gates the values: this host stalls modesets, and
  the row asks what a frame costs, not that it cost less than a number.
  One instrumentation defect surfaced only under real evidence. The
  classifier asked the flip counter whether the export it had timed was
  direct, but a flip happens later at submit, so every export answered no and
  the direct population was left with no offer samples at all. The emitter
  then dropped the half-measured population entirely rather than reporting
  it, so the only symptom was a comparison that said no direct frames
  existed.
- [ ] Add the hardware cursor plane, with the per-output KMS transaction owner
  the next row introduces. **Partly proven.** Direct-scanout archive `0005`
  runs the same twelve-position sweep as `0004` with the cursor on an atomic
  plane: the card accepted the plane, twelve moves reached it, no hardware
  failures, and direct scanout was undisturbed -- 36 flips with 26 after the
  motion stopped. `updates_primary_in_flight=0` is the kernel's per-CRTC
  serialization observed rather than assumed, where the legacy path counted
  fifteen.
  What it costs is worst-case latency: motion-to-submit peaked at 21 ms
  against `0004`'s 9 ms, which is about one frame of waiting for a busy CRTC
  and is the expected price of joining the queue the primary waits in.
  What is **not** proven, and blocks the next row: the owner never combines
  primary and cursor state in one request. Every atomic cursor update is a
  standalone commit. The machinery exists end to end -- the submit policy
  carries a cursor, the request builder applies it -- but nothing populates
  it, so `plan_cursor_commit`'s `RideNextPrimary` branch is unreachable.
  Connecting it is what would remove most of that 21 ms, since a cursor
  riding a frame that is going out waits for nothing.
  The legacy cursor continues over directly scanned frames on its own ioctl,
  which archive `0004` establishes rather than assumes: twelve cursor positions driven through the same `Pointer::place`
  entry physical input uses, 519 hardware updates with no failures, the
  cursor never leaving `legacy_ioctl`, `composed_cursor` still zero, and
  twenty-six client buffers reaching the plane after the motion stopped.
  Motion-to-submit peaked at 9 milliseconds, which is the baseline the atomic
  cursor plane has to match.
  The three earlier archives had asserted this and tested none of it: their
  cursor records read `moves_coalesced=0 max_motion_to_submit_msec=0
  hardware_updates=1`, a cursor initialized once and never moved.
- [ ] Replace the bounded legacy cursor baseline only after one per-output KMS
  transaction owner can combine primary and cursor-plane state in the same
  atomic request. Retain bounded cursor-only idle work and the pointer-motion
  GLX cadence gate.
- [ ] Replace full immutable CPU presentation replacement for stable
  software-rendered X toplevels with lease-safe damage generations or
  copy-on-write backing. Preserve child composition, bounded storage,
  historical-handle immutability, and exact admission extents.
- [ ] Compare identical Kitty, Firefox, resize, launch-burst, and soak
  workloads against separate XLibre+xmonad and mature Wayland-compositor
  sessions on the same hardware. Comparative results are diagnostic; Sophia's
  absolute correctness and latency gates remain authoritative.

Milestone 14 exits with bounded warmed resource counts, no steady-state
allocation growth, refresh-relative latency evidence, and no change to
Sophia's native-X authority model.

---

## Post-Promotion Capability Roadmap

These are ordered product capabilities. They do not block Milestone 13's freeze
unless the retained Triad port ledger names the same behavior as a retained row,
or a named failure promotes one. Check the ledger before assuming an item here
is post-freeze work: several rows under Native Sophia Follow-Ups and Status,
Launcher, And Shell Integration are pre-freeze port requirements.

### Blind WM And Multi-Output Policy

- [ ] Add opaque actions for focus master, swap master/up/down, shrink/expand,
  master count, reset layout, focus output, move surface to output, and
  supervised WM restart.
- [ ] Replace the bridge's singular active-workspace view with output-scoped
  active workspace and focus state. Prove independent workspace changes,
  surface moves, output removal, output return, and bridge restart without
  exposing application identity.
- [ ] Add trusted launch-placement provenance for configured applications.
  Keep class/title matching out of the WM and Engine.
- [ ] If tabs are justified, design metadata-free Engine-owned native tab
  chrome and opaque tab actions. Do not emulate title-aware xmonad decorations.

### Classical X11 WM Compatibility

- [ ] After native promotion, reinstall the practical xmonad profile and pass
  its bounded physical scenario corpus on one immutable candidate. Require
  exact action and pointer commits, correct Kitty, Firefox,
  xmobar, chrome, and TrueColor behavior, zero lifecycle debt, redacted health
  summaries, and checksummed artifacts.
- [ ] Migrate that profile through the public projection transport without
  changing retained behavior; it must use the same Engine reducer as Hagia but
  may keep its profile translation behind the compatibility adapter.
- [ ] Separate profile-independent synthetic-X lifecycle, layout translation,
  validation, supervision, and recovery from xmonad-specific bindings and
  request patterns. Keep one shared conformance suite for every compatibility
  profile.
- [ ] Define profile admission criteria: a named upstream WM and version,
  frozen configuration, minimal captured synthetic-X request surface,
  complete opaque-action map, deterministic layout/focus/workspace/restart
  tests, and one real installed-session proof.
- [ ] Add classical WMs incrementally from retained user workflows. Likely
  candidates include i3, dwm, and qtile, but ordering follows user demand and
  evidence rather than nominal X11 compatibility.
- [ ] Consider a conventional GTK3 desktop profile such as Xfce as the driver
  for X11 compatibility completeness: EWMH coverage, `_NET_WM_STRUT_PARTIAL`
  work-area reservation, and tray/XEmbed admission. Such a profile draws its
  own pixels and can never exercise a display-list interface, so it is
  compatibility evidence only and must not be cited as `sophia_shell_v1`
  evidence; see `docs/sophia-shell-v1-direction.md`.
- [ ] Reject profiles that require real client metadata, global X server
  ownership, drawing through the fake server, raw input, arbitrary command
  execution, or protocol-specific authority below Engine. Supply missing
  metadata, shell, and session behavior through their proper bounded brokers.

### Native Sophia Follow-Ups

- [x] Ratify target-resolved input as a pre-schema prerequisite. The contract
  resolves against per-output presented snapshots, admits targets only inside
  their owner's visible allocation, gives non-recyclable authority/session
  identity and device/contact-bound per-seat capture, and paces normalized
  continuous values. Exceptional coordinates require an independently issued,
  revocable region/precision/rate capability. The target, pacing, and
  cross-authority arbitration TLA+ models precede any `sophia_shell_v1` schema
  or runtime work; see `docs/target-resolved-input.md`.
- [x] Admit the first complementary architecture-model gate. Alloy checks
  bounded role/namespace/portal and presented-target topology; Z3 checks target
  geometry/disclosure arithmetic and schema-generated `sophia_wm_v1` wire
  bounds. Every promoted rule retains a satisfiable negative control. Keep
  temporal ownership in TLA+ and keep Spin, dependency policy, and fuzzing
  deferred until they have concrete retained artifacts; see
  `docs/architectural-alignment.md`.
- [x] Ratify the WM/shell hardening prerequisites. Blind WM policy cannot share
  a protection domain with metadata-bearing shell, broker/portal, or frontend
  roles; opaque actions bind issuer/recipient epochs, operation class, and
  target generation; and a tier-1 shell reservation, derived work area, and WM
  projection promote as one exact presented bundle. Alloy and
  `ShellWorkAreaCoordination` retain focused negative controls. This is target
  architecture, not a shipped shell or sandbox.
- [x] Enforce protection domains in session supervision before admitting a
  metadata-bearing shell. The Bubblewrap backend rejects blind-policy domains
  composed with shell, broker/portal, or frontend roles; launches public Hagia
  and the production metadata broker with cleared environments, stdio-only
  inherited descriptors, private PID/user/network/IPC/UTS/cgroup namespaces,
  no ambient home or temporary tree, and only explicit read-only socket/profile
  paths plus Hagia's private writable checkpoint directory. Exact host peer PID
  is recovered for role-socket admission, and the protected broker smoke is the
  executable negative control. Bubblewrap 0.11.2 is now a checked Hagia install
  prerequisite. The live Hagia Shell now receives that separate supervised
  role/domain. Compiled-profile enablement is now complete; its physical proof
  remains a later gate, and this checkbox alone does not claim it.
- [x] Require that domain where roles are admitted, not only where domains are
  constructed. The shell and metadata-broker sockets refuse a supervised PID,
  refuse an expected peer identity at bind time, and admit only the launch
  evidence their supervisor produced, which must carry that role's
  protection-domain role. The metadata broker transport publishes no PID-only
  call, so a caller that spawns it unprotected fails to compile rather than
  admitting quietly. The blind spatial-policy and output roles are unchanged and
  still admit on a supervised PID; requiring a domain everywhere remains the
  separate `bwrap`-availability decision. Evidence is a passive record whose
  fields any caller can write, so this closes silent omission rather than
  deliberate misreporting.
- [ ] Implement issuer-scoped action-capability validation and the atomic
  shell-reservation/work-area/WM coordinator with the eventual shell schema.
  Preserve the prior complete presented bundle on ordinary failure and keep
  lock/session security takeover independent of shell or WM acknowledgement.
  Experimental `sophia_shell_v1` revision 1 now carries nonzero, nonreused
  broker action grants through exact broker, revocation, recipient, target, and
  descriptor generations; its model and transport revoke interaction on shell
  loss while retaining prior pixels. Issuer-side live dispatch now revalidates
  the broker grant before submitting an ordinary WM focus request. The atomic
  reservation/work-area/WM coordinator remains open.
- [ ] Repair native application input before shell coexistence.
  - [x] In the installed primary-output pointer domain, derive hit-test layers
    from the immutable output-frame snapshot only after accepted page-flip
    retirement. Committed/submitted moves, removals, and stacking changes no
    longer become selectable before their pixels.
  - [x] Introduce output-local pointer coordinate domains and per-output
    presented interaction epochs rather than merging independently retired
    heads into one global projection.
  - [x] Advance Sophia `SurfaceId` generations when a client reuses an XID and
    retire the exact frontend route on successful surface removal. Frozen or
    deferred generation-N input cannot resolve to generation N+1.
  - [x] Isolate a non-reading X client's private input queue. Saturation now
    removes that endpoint's sender set, rejects tracked delivery, and leaves
    the shared frontend broker available to healthy clients.
  - [x] Revalidate deferred pointer-focus handoffs before release. Every exact
    generational target must remain in the last-presented input projection and
    frontend route table; otherwise the complete buffered sequence is dropped.
  - [x] Preserve ordered client keyboard input across asynchronous focus
    acknowledgement without retargeting it. Reserved controls resolve first;
    the remaining keys retain their exact seat, generational surface, order,
    and libinput timing in a capacity- and timeout-bounded handoff. Focus,
    target, topology, or security invalidation drops the complete sequence.
  - [x] Turn ordinary and passive-grab pointer sequences into exact
    Engine-visible profile-scoped leases with ordered release acknowledgement.
    VT and seat transitions advance a shared epoch, clear frontend active
    ownership, and reject queued or frozen old-epoch input without waiting.
  - [x] Reduce client-initiated explicit `GrabPointer` and admitted XI grab
    requests into the same Engine lease handshake. The passive bounded control
    path prepares before frontend mutation, activates before success is
    exposed, orders release, rejects saturation, and shares the existing
    disconnect, presentation, admission, and control-epoch revocation path.
  - [ ] Bind lock and future security-authority takeover to the established
    epoch barrier.
- [ ] Add bounded policy interactions for move, resize, drag, and scrolling.
  Engine owns hit-testing, grabs, raw physical input, cursor state, and
  animation; Hagia receives only opaque targets and reduced geometry updates.
  Revision 3 now permanently fixes `Drag = 3`, `Scroll = 4`, and the in-place
  `interaction_axis` values (`None = 0`, `Horizontal = 1`, `Vertical = 2`) in
  Rust, generated C, and Hagia's independent Nim decoder. The codecs reject
  ambiguous geometry/axis combinations. Engine-captured move/resize now send
  ordered Begin/Update/End values; only the latest matching queued Update is
  retained behind an in-flight request. Output-topology, VT, and seat security
  transitions clear capture, purge its queued values, and prioritize Cancel;
  policy restart clears capture before the next physical input drain. Hagia
  applies continuous geometry and treats Cancel as a spatial no-op. Live
  drag/scroll production remains open, so this row is intentionally not complete.
- [x] Make exact desktop-profile activation mandatory for every public
  `sophia_wm_v1` launch. The pre-graphical gate prepares all seven authorities,
  activates the six Sophia-local owner slots, admits Hagia against the exact
  Policy fragment, and promotes only after its matching completion. Timeout,
  disconnect, rejection, or any local failure rolls the generation back before
  graphical construction. The old opt-in switch remains only as a harmless
  compatibility argument. This closes startup visibility; watched reload still
  requires its separate message-53+ visibility and durable-recovery protocol.
- [ ] Model and publish `sophia_shell_v1` through the same formal, schema, C
  client, and permanent-compatibility process. Keep its endpoint and
  capabilities separate from `sophia_wm_v1`. Begin experimental modeling and
  the Hagia Shell port before the WM freeze so retained workflows can falsify
  both contracts. Derive the vocabulary from a driving client with a retained
  scene graph rather than from first principles; see
  `docs/sophia-shell-v1-direction.md`. Revision 1 is now published as an
  explicitly experimental descriptor-switcher schema with strict Rust codecs,
  a generated golden/malformed corpus, an independent C proof client, and a
  23,582,243-state lifecycle check. Permanent compatibility, broader display
  lists, reservation coordination, and signed installed evidence remain open.
- [x] Ratify one developer-facing Sophia native protocol family rather than a
  collection of role-specific transports or language tooling. The common
  contract now owns the envelope, negotiation, capabilities, epochs, bounded
  complete transfers, explicit outcomes, recovery, extension discipline,
  source hierarchy, and independent-implementation requirement. WM, shell, and
  output retain separate endpoints, disclosures, schemas, and stability status;
  see `docs/sophia-policy-ipc.md`.
- [ ] Audit `sophia_wm_v1`, `sophia_shell_v1`, and `sophia_output_v1` against
  the common family lifecycle. Align hello/welcome negotiation, effective
  bounds, capabilities, epochs, transaction identity, complete-transfer
  behavior, outcomes, recovery, and extension handling. Every intentional
  difference must be role-specific and documented rather than an accidental
  transport fork. This audit does not block the active Milestone 14 direct-
  scanout proof, but it gates broad shell vocabulary and shell stabilization.
- [ ] Add one family-level conformance entry point that invokes each role's
  retained valid, malformed, codec, and lifecycle corpus. Every stable role
  must retain a complete independently implemented non-Rust lifecycle proof;
  shell stabilization specifically requires the existing C proof plus Hagia's
  independent Nim client. The proof must be implementable from normative prose
  and the checked-in schema without Sophia Rust crates, generated bindings,
  generators, or implementation-source inspection.
- [x] Ratify visual styling and compositor-effect extensibility without moving
  compositor authority out of Engine. WM and shell authors own role-appropriate
  visual policy; public protocols carry bounded semantic intent rather than
  shader code; Engine owns validation, animation timing, damage, rendering, and
  presentation. The first provider boundary is a private version-coupled Rust
  trait for separately maintained modules linked into the trusted renderer
  build and selected by the immutable packaged profile. Runtime shared-library
  loading and a sandboxed effect host remain evidence-gated later designs; see
  `docs/compositor-graphics.md` and `docs/sophia-shell-v1-direction.md`.
- [ ] Before broadening the shell schema, model effect capability admission,
  bounded parameters, generation supersession, Engine-clock cancellation,
  provider absence or failure, deterministic fallback, and atomic multi-head
  presentation. Retain negative controls that expose unnegotiated capabilities,
  stale animation, missing fallback, unequal-head semantics, and an effect that
  incorrectly survives provider or authority revocation.
- [ ] Implement one protocol-neutral Engine effect registry and the private
  build-linked provider interface, then prove backdrop blur as a scene-sampling
  effect and a focus transition as Engine-clocked animation. Keep provider code
  outside the core renderer module, bind it to the installed release identity,
  add deterministic reference lowering and native shader/pixel/damage gates,
  and prove that effect activation disables direct scanout without stale pixels.
  Only then admit capability-gated semantic effect intents to the experimental
  shell contract or a future outbound-gated WM extension; do not reopen frozen
  `sophia_wm_v1` revision 3. Require the Rust and independent C/Hagia corpora and
  one independently packaged provider proof before considering a stable
  provider interface.
- [ ] Settle the remaining display-list vocabulary before schema work. Admit
  generic target regions and a desktop-background surface class, evaluate analytic
  screen-corner and indeterminate-progress primitives, and refuse per-widget
  visual novelty in favor of client-rasterized textures. Record the damage,
  bandwidth, and power cost of that texture path before relying on it.
- [ ] Build `hagia-shell` as one ordinary separately authorized shell client
  for tabs, overview, switchers, previews, and other visible furniture. Shell
  metadata must never leak into Hagia's blind spatial-policy projection. The
  standalone Nim executable and reducer now pass the shared corpus and a
  protected title-only cross-process proof without linking Hagia policy.
  Shell-enabled live launch, shortcut admission, exact activation, issuer-side
  dispatch, withdrawal, reconnect, explicit X-grab arbitration, and compiled
  profile enablement are implemented. Installed evidence and richer furniture
  remain open.
- [ ] Add trusted classification, launch, lock, capture, output, and transfer
  services through brokers, session capabilities, and portals. Hagia may
  request opaque actions but may not receive executable paths, client
  metadata, portal payloads, or compositor authority.

### Status, Launcher, And Shell Integration

- [ ] Define a bounded redacted status feed for workspace number, approved
  layout name, focus state, output health, and supervised-component health.
  Workspace number, layout name, and focus state are settled by the indicator
  descriptor and arrive on the layout commit, not through a broker: policy owns
  them and no broker has an upstream source. Output and supervised-component
  health remain session-owned and still need a path. See
  `docs/sophia-indicator-descriptor.md`.
- [x] Render tier-0 indicator chrome in Engine from the committed descriptor,
  reusing the existing `capability "chrome"` path and the renderer-neutral
  display list. The private semantic strip lowers through ordinary CPU layers,
  uses one bundled font, reserves 14 logical pixels before the first public WM
  snapshot, and publishes exact last-presented hit targets. The existing
  `tools/hagia-proof` one-shot now requires two pointer activations and their
  committed policy actions. Signed archive `0005` verifies both activations,
  all fourteen ordered action commits, nonzero presentation on both outputs,
  exact physical text, and clean teardown.
- [x] Emit indicators from Hagia's private tags, keeping tags private and
  crossing only labels, state bits, and action tokens. Hagia's independent Nim
  codec and the cross-repository conformance gate cover the records.
- [ ] Register a new bounded opaque launcher action and decide whether the
  compatibility UI is dmenu or native Engine/shell chrome. Do not reuse the
  established xmonad layout-action IDs.
- [ ] Implement lock, screenshot, wallpaper, and audio actions through their
  owning shell or portal boundaries.
- [ ] Admit tray/XEmbed only from a retained application workflow and keep it
  outside blind WM policy.

### Portals And Namespace Promotion

- [ ] Promote a confined daily-driver application group only after Firefox and
  Kitty pass the same workflow under explicit grants.
- [ ] Implement large X11 `INCR` clipboard transfers from retained evidence.
- [ ] Implement Xdnd and URI/file launching through portal grants.
- [ ] Implement prompt UI, notification actions, and capture/FD handoff through
  the existing reducers and bounded executors.

### Rendering And Compatibility Follow-Ups

- [ ] Fix stable-relayout silence in the xmonad compatibility bridge. Signed
  source `c681f762` reached one through four retained synthetic windows, but
  unchanged `SceneChanged` cycles waited for ConfigureRequest replies that real
  xmonad did not emit and exhausted the outer supervisor. Add a quiet-boundary
  regression for stable retained geometry while preserving strict manage and
  resize response fences. This is retained compatibility work, not a native
  Sophia WM protocol, shell protocol, or Milestone 14 promotion blocker.
- [ ] Retain the bounded physical `glxgears` proof with visible animation,
  advancing Present/KMS cadence, matching reference provider, clean retirement,
  and zero protocol or renderer debt.
- [ ] Obtain an unredirected Xorg/XLibre `Flip` reference only if end-to-end
  presentation-latency parity is needed. Keep composited `Copy` results labeled
  as client-cadence evidence.
- [ ] Complete the two-output concurrent-producer workload after the shared
  renderer-worker prerequisite in Milestone 13. Require bounded inter-output
  service skew and no producer starvation.
- [ ] Replace per-frame CPU GBM allocation with an output-scoped,
  retirement-fed three-slot pool only if measured software fallback remains
  outside its parity gate.
- [ ] Run the deterministic Firefox pointer/keyboard/wheel fixture in Chromium
  as an independent native-X consumer after Chromium is installed.
- [ ] Add client-selected classic X11 cursor images or further toolkit,
  extension, font, color, and WM behavior only when a retained workflow exposes
  the missing protocol fact.
- [ ] Add opportunistic scanout cloning for equal-mode mirror heads after the
  per-head composition path is promotion-proven. Eligibility is plan-record
  equivalence (geometry, mapping, density, generations; never the content
  checksum) per the normative design in
  [Multi-Monitor Per-Head Composition](docs/multi-monitor-composition.md#target);
  the decision stays backend-private and switches only through topology
  transactions. Exit gate: an equal-mode physical mirror run whose evidence
  shows one composition per frame and one framebuffer identity behind both
  opaque heads' plane assignments with joined retirement; a dual-render audit
  phase proving the cloned and per-head compositions byte-identical for one
  committed scene; a forced mid-run divergence through `sophia_output_v1` that
  demotes to per-head with no visual discontinuity or leaked framebuffer; and
  re-promotion only after a passing atomic `TEST_ONLY` probe. Cross-card
  mirrors and unequal modes remain render copy permanently.

### Hardware Diagnostics And Hotplug

- [ ] Retain the exhaustive pc105 US shifted-punctuation and Ctrl-Alt-F1
  through Ctrl-Alt-F12 physical runner as a focused diagnostic. Repeat it after
  input/seat changes or for release burn-in; ordinary candidate promotion
  requires one real VT round-trip plus the deterministic XKB suite.
- [ ] After work-area, output, or seat changes, re-run the exhaustive xmobar
  reservation lifecycle and require no stale gap, overlap, resize timeout, or
  focus change. Pair dynamic output-topology behavior with the later physical
  multi-output hotplug gate. `tools/output_topology_physical_gate.sh` now arms
  that exact multi-output loss/return procedure and requires two input-epoch
  barriers, generation-advancing complete publications, policy settlement,
  later page flips, client survival, and clean final topology health. The
  recovery-safe one-command entry point is
  `tools/run_output_topology_gate_tty4.sh`; it supplies the routine arm, seat,
  matching signed Hagia and Sophia builds, and timestamped evidence defaults so
  the operator carries no shell state between TTY sessions. Failed attempts
  `/tmp/sophia-output-topology-20260821-231655.log` and
  `/tmp/sophia-output-topology-20260821-232830.log` exposed the missing-`udevd`
  rebroadcast and stale connector-cache dependencies respectively. Signed run
  `/tmp/sophia-output-topology-20260821-233802.log` passed the exact changed
  `3 -> 2 -> 3` publication, policy, presentation, completion, and clean-health
  predicates. The dynamic-output half is promoted; this checkbox remains open
  for the exhaustive xmobar reservation lifecycle after a relevant work-area,
  output-policy, or seat change, not for head-loss/return.

---

## Secondary Development Tooling

Interactive QEMU is useful for reproduction but is not a physical daily-driver
blocker. Work on it only when it shortens an active milestone.

- [ ] Fix the load-sensitive flake in `sophia-x-authority`'s `x11_wire` suite.
  Diagnosed, not fixed. It is **not** a timeout: under a parallel build,
  `routed_service_confines_input_and_control_to_two_workers_and_drains` fails at
  `socket_observation.rs:710` with `BadWindow` (3) where `BadAccess` (10) is
  expected, and `routed_lifecycle_events_follow_structure_and_substructure_masks`
  and `configured_present_child_receives_xlibre_ordered_geometry_notification`
  fail the same way intermittently. The shape is a cross-client race: the first
  client writes four requests and never reads, so nothing establishes that the
  server processed its `CreateWindow` before a second client refers to that window.
  The obvious fix is wrong. Adding a round-trip barrier on the first client — a
  request against an absent resource, whose error reply proves everything earlier
  was processed — makes the test fail **deterministically** rather than fixing it.
  So per-connection request ordering is not the whole mechanism, and the routed
  two-worker path or the confined-namespace boundary between the two clients is
  involved. That is where the next attempt should start, and it is worth more than
  the failed patch, which was reverted.
  A second mechanism is now recorded, and it rules out the tempting fix.
  `configured_present_child_receives_xlibre_ordered_geometry_notification` fails
  under full-workspace load inside `read_x_reply`: the reply's 32-byte prefix
  arrives and the body never does, until the **10-second** `SOCKET_IO_TIMEOUT`
  expires. Raising timeouts is therefore not the answer — ten seconds is already
  generous, and a reply that is half-sent for ten seconds is a server withholding a
  body, not a machine that was briefly busy. Both mechanisms point at the same
  place: what the routed workers do when more than one client is live.
  Note also that a failure here truncates the workspace run, because cargo stops
  before the remaining test binaries. A full-suite total that drops by roughly
  thirty-six tests is this flake, not a missing suite.
  A third attempt narrowed the mechanism to `read_x_reply`
  (`tests/x11_wire/support_extensions.rs`) and then failed too, which is the most
  useful thing recorded here. That helper reads 32 bytes and derives a body length
  from bytes 4..8 whatever the record is. Only a reply has a body: an error's bytes
  4..8 are its offending resource id, and an event has none at all. So a non-reply
  record yields a nonsense length and a read that blocks for the full timeout.
  What makes it stubborn is that the two failing tests **depend on that mis-parse**.
  Instrumenting the helper to reject non-replies showed both reading Sophia Present
  **event type 35** through `read_x_reply` on *every* run, not only under load — one
  call site even names the result `present`. The mis-parse is load-bearing: those
  events carry zero in bytes 4..8, so the bogus length is zero and the helper
  happens to return the event intact. Returning non-reply records whole, which is
  what the wire actually says, also broke both tests deterministically, so they rely
  on more than the zero-length coincidence.
  Two conclusions. The fix is **not local to the helper** — those two tests must be
  rewritten alongside it, which needs someone to work out what they intend to assert
  about Present events versus replies. And raising timeouts remains wrong: the
  records arrive promptly, they are simply parsed as the wrong kind.
  Both attempted fixes were reverted. Baseline is 178 passing.
  A suite that fails for non-reasons erodes every other claim in this file, so this
  is worth closing even though it is not on the critical path.
- [ ] Complete one human-visible `xmonad-interactive` capture proving pointer
  movement, terminal launch, typed text, focus change, application close, and
  clean manual shutdown. The fail-closed verifier, mutations, and RFB capture
  already pass.

## Deferred

- XLibre provider integration until measured native-X gaps justify its
  authority and maintenance cost.
- Any new application protocol or compatibility frontend without a
  specification amendment backed by named product evidence.
- VRR activation until physical hardware reports `vrr_capable=1`.
- General X11 conformance work not required by a retained daily-driver client.
