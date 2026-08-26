# Triad Port Ledger (External Gate)

**Role:** pointer to a freeze gate that lives outside this repository.

`todo.md`'s `sophia_wm_v1` freeze item has two conditions. The first — that the
retained Triad behavior port is complete — is defined by a ledger that lives in
the **Hagia** repository, not here:

    docs/triad-port-ledger.md        (in the Hagia checkout)

This file exists so that a reader of `todo.md` can find that gate. It is a
pointer and a summary, not a copy. **The Hagia file is authoritative.** Do not
resolve a disagreement between them by editing the summary below; re-read the
ledger.

## Identity

- Reviewed Triad source baseline: `fb8fb27ec294e0fe2361375de0b2fa8c08be0ca9`.
  A later Triad change is considered separately and does not move this baseline.
- Default checkout location: sibling of this repository, `../hagia`. The
  cross-repository conformance gate resolves it through `SOPHIA_HAGIA_ROOT` and
  runs Hagia's suite with `SOPHIA_STACK_ROOT` pointing back here.

## Completion Rule

Interface revision 3 remains experimental while **any** retained row is partial
or open. Excluded rows stay visible as post-freeze product work and carry a
written rationale; they are not silently treated as implemented.
The gate closes only when every Triad feature family is classified as retained or
excluded; every retained family works through its assigned authority with no
hidden River, Wayland, Triad, or Niri runtime dependency; the retained default
desktop configuration has a validated migration with no accepted command silently
losing behavior; deterministic parity scenarios cover state transitions, failure,
restart, and authority loss; and ordinary installed sessions cover the physical
workflows that cannot be established offline.

An exclusion requires a written architectural or product rationale. "Not yet
implemented" is not an exclusion.

## Row States

Twenty-eight classified rows across four authority tables:

| Table | Rows | Complete | Partial | Open | Excluded |
| --- | --- | --- | --- | --- | --- |
| Spatial Policy — Hagia | 12 | 11 | 0 | 0 | 1 |
| Visible Desktop — Hagia Shell | 5 | 2 | 0 | 0 | 3 |
| Session And Dedicated Sophia Authorities | 7 | 4 | 1 | 0 | 2 |
| Brokers And Portals | 4 | 3 | 0 | 0 | 1 |

Totals: 20 complete, 1 partial, 0 open, and 7 excluded. The checked-in Hagia
daily-driver profile, rather than every binding in Triad's historical default,
defines the freeze surface. Trusted one-shot launch placement and frame-fed
output activation are implemented and offline-proven. The shared
reconnect/restart corpus, public xmonad migration, and immutable archived
revision-3 client now pass. The sole remaining retained gate is physical output
apply/rollback evidence.

`hagia-shell` now exists as source: Hagia commits `216fb87`, `3795dce`,
`c33a1f4`, and `a76528f` add the experimental client, the live shell service,
the protected switcher, and unlabeled descriptor decoding. Signed archive
`0006` proves the retained generic switcher; signed archive `0007` separately
proves coherent work-area reservation and reconnect. MRU policy, previews,
icons, and persistent Tier-1 panels are explicit post-freeze work.

## Consequences For Work In This Repository

- **API v7 cannot be removed until the freeze conditions hold.** The one
  partial retained output row now bounds that work; public xmonad migration,
  shared restart coverage, and the archived-client check are complete.
  Extraction from the v7
  module is *not* gated and happened early: `WmShortcutRegistry` and
  `WmShortcutRouter` now live in `crates/sophia-engine/src/shortcut.rs`, and the
  public path builds its registry from configuration rather than fabricating a
  `WmHello`. `WmSocketTransport` was left in place on purpose — it is v7 frame
  coding reached only by the legacy bridge, so lifting it would move v7 code
  rather than free anything. Engine-owned `WmWorkspaceState` is the remaining
  extraction.
- **Several items filed under `todo.md`'s Post-Promotion Capability Roadmap are
  already on the freeze path.** Protection-domain enforcement, the redacted
  status feed, and explicit-grab reduction are complete. Frame-fed atomic
  multi-output apply/rollback remains retained; lock, watched reload, and a
  separate output-power authority are explicitly post-freeze.
- **Wire-layout risk is enumerated separately** in
  `docs/wm-v1-freeze-surface.md`, which classifies every ledger row by whether
  closing it can force a `sophia_wm_v1` layout change.

## Binding Capacity Is Not The Constraint

The ledger records the binding inventory carefully, and it is easy to misread.
Interface revision 3 admits **256** binding registrations. Hagia's compiled profile
contains 51 Sophia-owned chords resolved against its 66-entry action catalog,
while Triad's baseline default holds 132 key and 137 total physical bindings, of
which the semantic migrator emits 39 key plus 2 pointer bindings today. The
remaining bindings stay classified with explicit retained or excluded
dispositions; exclusion does not consume a WM registration slot.

The constraint is the **authority split**, not a slot count. Every accepted
binding must name its matching owner and behavior owner; every excluded binding
must remain visible in the migration report.
