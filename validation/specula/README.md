# Specula Development Audit

**Role:** optional, commit-pinned development analysis; never a runtime or
build dependency.

Specula complements Sophia's hand-maintained TLA+ models by deriving a bounded
model from a deliberately narrow source slice, validating implementation
traces, and hunting for counterexamples. Generated models are review input,
not product specifications or refinement proofs. Sophia retains only corrected
project-sized models and deterministic regressions.

The 2026-08-07 audit used Specula commit
`3946f892cc078d5cfea3629f46bd826c246bf2a9` against Sophia commit
`ef918108da6a97eb73be5cb207ae77c492e9c029`. Its scope was complete legacy-WM
workspace projection, delayed Configure/Focus responses, restart and reseed,
and safe-pixel admission. Eleven configurations found four implementation
defects:

- a reply from a failed request could be attributed to its successor;
- a hard deadline could return success without a final quiet boundary;
- direct workspace assignment left cached membership and mapping stale; and
- a first pixel-silent admission timeout terminated the layout owner loop.

The corresponding Rust regressions and the `LegacyWmProjection`,
`LegacyWmResponseBoundary`, and `PixelSilentAdmission` models are the durable
artifacts. The restart/reseed scenario remained clean in exhaustive searches
of 302,541,189, 4,159, 595, 90,181, and 90,181 distinct states across its five
focused configurations. The generated audit also corrected two model
assumptions before accepting those results: retained fallback pixels need not
equal a later exact successor, and only expected Configure/Focus replies carry
a current-request obligation. Two final depth-100 simulations then ran for the
full 30-minute watchdog without a violation: candidate identity checked
701,155,271 states across 44,868,415 traces, and ownership exclusivity checked
707,143,607 states across 50,238,084 traces.

The optional post-validation agent confirmation is not cited as evidence. Its
first local X11 reproducer was provider-policy blocked five times despite
revised prompts, so that redundant phase was stopped after validation and bug
hunting had completed. The generated candidate remains local; the checked-in
Rust regressions provide the implementation proof.

Install or verify the pinned development tool on Void Linux with:

```sh
tools/install_specula_dev.sh
tools/install_specula_dev.sh --verify-only
```

Run the same bounded audit from a clean Sophia commit with:

```sh
tools/run_specula_x11_wm_bridge.sh
```

The runner makes a temporary clean clone so build products and prior evidence
cannot inflate the private artifact copy. It writes the isolated result under
`$SOPHIA_SPECULA_ROOT/runs` (default `~/src/Specula/runs`) and accepts an
optional run ID. `SOPHIA_SPECULA_TLC_MEMORY_LIMIT` and
`SOPHIA_SPECULA_TLC_WORKER_LIMIT` override the conservative local defaults.

Do not commit `.specula-output`, copied source trees, agent transcripts, TLC
state databases, generated patches, or raw traces. Record confirmed behavior
as the smallest deterministic Rust regression and update the owning TLA+
model, boundary map, research log, and milestone instead.

The desktop-profile startup activation protocol is modeled before wire
implementation under `profile-activation-protocol/`. Its checked design model
separates Sophia's local policy proxy from Hagia's actual authority state,
binds every completion to epoch/transaction/generation/digest, and keeps the
graphical launch gate closed through rejection, timeout, disconnect, and
rollback. The included trace is explicitly a design trace; it is not runtime
conformance evidence. Run the offline base search, focused hunts, and trace with:

```sh
SOPHIA_SPECULA_TLA2TOOLS_JAR=/absolute/path/to/tla2tools.jar \
    tools/check_profile_activation_protocol_model.sh
```
