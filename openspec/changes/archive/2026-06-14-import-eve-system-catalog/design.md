# Design — import-eve-system-catalog

## Context

The mapper needs a local catalog of EVE systems and the wormhole-type dictionary so it can resolve ids→names/regions, show system class, and (for the chain map) know what a wormhole system's statics spawn. None of this exists in the schema today. The data is assembled from three public feeds and is near-immutable, so the whole design is shaped by two goals: **(1) readers never see missing or partial data**, and **(2) keep it proportional** — this is corp-scale, near-static reference data, not a high-churn pipeline.

## Sources (verified against live payloads, 2026-06-14)

| Source | Rows | Shape | Provides |
|---|---|---|---|
| eve-scout `GET /v2/public/systems` | 8,037 | array of `{system_id, system_name, region_id, region_name, system_class, security_status, jove_observatory?}` | system spine; **J-codes included with class** (e.g. `J172840` = `c5`, id `31002274`) |
| eve-scout `GET /v2/public/wormholetypes` | 97 | array of `{identifier, type_id, target_system_class, max_jump_mass, max_stable_mass, max_stable_time, mass_regeneration, possible_static, wandering_only, signature_level[], source[]}` | type dictionary |
| anoikis `GET https://anoikis.info/data/wh-statics.json` | 2,604 | object `{ "J172840": {"static": ["H296"]}, … }` | J-code → static codes (the only piece eve-scout lacks) |

Class distribution in `/systems`: ns 3322, hs 1194, ls 687, jove 230, c1 358, c2 537, c3 506, c4 523, c5 531, c6 118, c13 25, and one each of c12/c14/c15/c16/c17/c18.

The merge key is dead simple: an anoikis J-code **is** `eve_system.name`. `max_jump_mass` encodes ship size (5M ≈ frigate-hole, 375M ≈ up to battleship, 1.8B+ ≈ capital). `K162` is the generic "exit" signature (all-zero masses, `target_system_class: "exit"`) and `wandering_only` codes exist in the dictionary but never appear as statics — both are fine, `system_static` simply won't reference them.

## Schema (migration 13)

Normalized: a dedicated `wormhole_type` table and a `system_static` join table (chosen over a `text[]` statics column so SQL can answer "which systems have a static to X").

```
wormhole_type
  identifier           text PK              -- "Q003"
  type_id              int  not null
  target_system_class  text not null        -- "ns" / "c5" / "exit"
  max_jump_mass        bigint not null
  max_stable_mass      bigint not null
  max_stable_time      int  not null        -- minutes
  mass_regeneration    bigint not null
  possible_static      bool not null
  wandering_only       bool not null
  signature_level      int[] not null       -- [] for K162
  source               text[] not null      -- ["c1",..], ["exit"]

eve_system
  system_id            bigint PK            -- 30000142 / 31002274
  name                 text not null        -- "Jita" / "J172840"   (indexed)
  class                text not null        -- "hs" / "c5" / "jove"
  region_id            bigint not null
  region_name          text not null
  security_status      real not null
  jove_observatory     bool not null default false
  index on (name)

system_static
  system_id    bigint not null references eve_system(system_id) on delete cascade
  static_code  text   not null references wormhole_type(identifier)
  primary key (system_id, static_code)
```

Decisions:
- **`class` / `target_system_class` as `text`, not a Postgres enum.** CCP has added classes over time (c12–c18); `text` avoids a migration each time and the import is the only writer. (A CHECK constraint would just be churn for no reader benefit here.)
- **`security_status` as `real`.** Source values are integers and one-decimal (`0.7`, `-0.5`, `-1`; all WH systems are `-1`). Exactness doesn't matter for this catalog; `real` is fine.
- **`signature_level` / `source` as arrays.** They are arrays at the source; store them as-is.

## The sync service (`services/eve_system_sync.rs`)

Modelled on `token_sweep` so it reads as the same kind of thing:

```rust
pub struct SyncContext { pool: PgPool, http: ClientWithMiddleware,
                         systems_url, wormhole_types_url, statics_url, user_agent }

pub fn spawn(ctx: SyncContext) {            // 24h interval, first tick immediate,
    tokio::spawn(async move {               // per-run error logged, loop survives
        let mut t = interval(Duration::from_secs(24*60*60));
        loop { t.tick().await;
               if let Err(e) = run_once(&ctx).await { error!("eve-system sync failed: {e:#}"); } }
    });
}

pub async fn run_once(ctx) -> anyhow::Result<()> {
    // PHASE 1 — fetch ALL THREE first, DB untouched. Any ? bails the whole run.
    let systems  = fetch_systems(ctx).await?;
    let types    = fetch_wormhole_types(ctx).await?;
    let statics  = fetch_statics(ctx).await?;   // sends ctx.user_agent (identifying courtesy header)

    // SANITY FLOOR — refuse to overwrite good data with an implausible payload.
    sanity_check(&systems, &types, &statics)?;  // pure, unit-tested

    // PHASE 2 — one transaction; FK-ordered; atomic commit.
    let mut tx = ctx.pool.begin().await?;
    db::eve_system::upsert_wormhole_types(&mut tx, &types).await?;   // parents first
    db::eve_system::upsert_systems(&mut tx, &systems).await?;
    db::eve_system::rebuild_statics(&mut tx, &statics).await?;       // skip+log unmatched codes
    tx.commit().await?;
    info!(systems=systems.len(), types=types.len(), statics=..., "eve-system catalog synced");
    Ok(())
}
```

Wired in `main.rs` right after the `token_sweep::spawn`, with cloned `state.db` and `state.http_client` plus the source URLs from config (defaults baked in; overridable via env to point at fixtures/mirrors).

## Why this guarantees gap-free reads

The concern was "no missing data when someone needs it." That is a **transaction** property, not a speed one:

- **Fetch-all-then-write.** Nothing is written until all three sources are in hand. "One source failed" is therefore indistinguishable from "the run failed" — both abort before the transaction. You never get new systems with stale statics.
- **Single transaction across all three tables.** Under Postgres MVCC, concurrent `SELECT`s see the complete *old* snapshot for the entire write window and atomically flip to the complete *new* one at `COMMIT`. A reader can never observe an empty or half-written table.
- **Upsert, never `TRUNCATE`.** `TRUNCATE` takes an `ACCESS EXCLUSIVE` lock that would block readers; `INSERT … ON CONFLICT DO UPDATE` takes only row locks, so plain reads are never blocked. `system_static` "rebuild" = delete the affected rows + reinsert **inside the same transaction**, so old statics remain visible until commit.

Measured cost: ~13k tiny rows across three tables; the write is sub-second, the run is ~1–3s dominated by the HTTP fetch. No build-aside/rename machinery is needed — the single-txn upsert already gives the atomic flip at this volume.

The one unavoidable gap: the **first run on an empty DB** has no prior snapshot to show, so the catalog is empty for the ~1–3s until that first commit. Harmless (nothing reads it yet) and self-resolving.

## Failure contract

| Scenario | Behaviour |
|---|---|
| Any of the 3 fetches errors (timeout/5xx/403) | Abort before the transaction; log; prior catalog fully intact; retry in 24h |
| All 3 fetch OK | Single transaction, atomic swap, gap-free |
| Fetch OK but implausibly small (e.g. `{}`) | **Sanity floor** rejects before the transaction; prior data untouched |
| Transaction fails mid-write | Postgres rolls the whole txn back; prior data intact |
| anoikis static code with no matching `wormhole_type` | That one `system_static` row is skipped and logged; the run continues (not fatal) |
| First run ever (empty DB) | No prior data to protect; empty until first success commits |

**Sanity floor thresholds** (well below observed counts, well above any plausible partial outage): systems ≥ 5000, wormhole_types ≥ 50, statics ≥ 1000. A sub-floor payload returns an error from `run_once` → logged, prior data kept. These are deliberate, documented constants — the only place this design adds rigor beyond "abort on error", because a valid-but-empty response is the one failure that could silently wipe good data through the `system_static` rebuild.

## Proportionality notes

- **Full per-run upsert, no diffing.** 13k rows is trivial; diffing would be machinery for no benefit at this scale and cadence.
- **No job registry.** This is job #2 (after `token_sweep`); per the deferred-jobs decision the registry waits until #3. The sync is a standalone spawned task like the sweep.
- **No admin manual-trigger endpoint.** Deferred to the future job-interface change (which is also where a re-run-now button naturally lives, e.g. after a CCP static rebalance).
- **No retry/backoff/alerting on a failed daily run.** Data is near-immutable; last-good persists; the next tick retries. The shared client already carries ESI rate-limit middleware.

## Out of scope (explicit)

- **Read API** for systems/wormhole types — this change only populates the tables.
- **Live Thera/Turnur connections** (`/v2/public/signatures`) — transient, expiring *connection* data for a separate path-routing service, not this static catalog.
- **Job registry** and **manual admin import** — deferred to the job-interface change.
