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

Revision 1 remains experimental while **any** retained row is partial or open.
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

Twenty-seven retained rows across four authority tables:

| Table | Rows | Complete | Partial | Open |
| --- | --- | --- | --- | --- |
| Spatial Policy — Hagia | 12 | 3 | 7 | 2 |
| Visible Desktop — Hagia Shell | 5 | 0 | 0 | **5** |
| Session And Dedicated Sophia Authorities | 6 | 0 | 4 | 2 |
| Brokers And Portals | 4 | 0 | 0 | **4** |

The shell and broker/portal tables are entirely open, and they are inside the
gate. A reader who assumes the freeze waits only on WM-side rows will
underestimate it by nine rows. `hagia-shell` does not exist as source.

## Consequences For Work In This Repository

- **API v7 cannot be removed until the freeze conditions hold.** Because the
  freeze is far off, v7 is load-bearing for a long time. Extraction from the v7
  module is *not* gated and happened early: `WmShortcutRegistry` and
  `WmShortcutRouter` now live in `crates/sophia-engine/src/shortcut.rs`, and the
  public path builds its registry from configuration rather than fabricating a
  `WmHello`. `WmSocketTransport` was left in place on purpose — it is v7 frame
  coding reached only by the legacy bridge, so lifting it would move v7 code
  rather than free anything. Engine-owned `WmWorkspaceState` is the remaining
  extraction.
- **Several items filed under `todo.md`'s Post-Promotion Capability Roadmap are
  pre-freeze requirements**, because the ledger names the same behavior as a
  retained row. Check the ledger before treating an item there as post-freeze
  work. The known cases are explicit-grab reduction with the lock and security
  epoch barrier, protection-domain enforcement in session supervision, the
  redacted status feed, and the output authority's atomic multi-output
  test/apply/rollback with reservations and a separate power authority.
- **Wire-layout risk is enumerated separately** in
  `docs/wm-v1-freeze-surface.md`, which classifies every ledger row by whether
  closing it can force a `sophia_wm_v1` layout change.

## Binding Capacity Is Not The Constraint

The ledger records the binding inventory carefully, and it is easy to misread.
Revision 1 admits **256** binding registrations. Hagia's compiled profile
contains 50 Sophia-owned chords resolved against its 66-entry action catalog,
while Triad's baseline default holds 132 key and 137 total physical bindings, of
which the semantic migrator emits 39 key plus 2 pointer bindings today. The
remaining bindings are classified but land in shell, broker, portal, and session
authorities that do not exist yet.

So the open question is the **authority split**, not a slot count. The ledger's
remark that a smaller bound "cannot be frozen without first classifying and
migrating that set" is about a proposed freeze bound, not a current wire limit.
