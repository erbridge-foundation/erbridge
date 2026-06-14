## Why

The mapper has no knowledge of EVE's system topology. To resolve a system id to a name and region, to show a system's class (highsec / C5 / Jove / …), and — for the chain map — to know what wormhole connections a system's **statics** will spawn (target class, max ship size, lifetime), the backend needs a local reference catalog of every system plus the wormhole-type dictionary. No such table exists today; migration `…12` is the latest and there is no system/wormhole schema at all.

Three public sources together provide this, and crucially the **J-codes (wormhole systems) carry their class and ids in eve-scout's `/systems` feed**, so the catalog is buildable without the EVE SDE:

- **eve-scout `GET /v2/public/systems`** — the system spine: 8,037 rows covering every class (hs/ls/ns/jove and c1–c6, c13, plus the c12/14–18 specials), each with `system_id`, `system_name`, `region_id`, `region_name`, `system_class`, `security_status`.
- **eve-scout `GET /v2/public/wormholetypes`** — the wormhole-type dictionary (97 rows): per type code (`Q003`, `A239`, `K162`, …) the `target_system_class`, `max_jump_mass` (→ ship size), `max_stable_mass`, `max_stable_time` (lifetime, minutes), `mass_regeneration`, `possible_static`, `wandering_only`, `signature_level`, `source`.
- **anoikis `GET https://anoikis.info/data/wh-statics.json`** — J-code → list of static type codes (2,604 entries), e.g. `"J172840": {"static": ["H296"]}`. This is the only piece eve-scout does not provide; the J-code keys match `eve_system.name` exactly.

This data is **near-immutable** — system ids/classes never change and statics change only on a CCP rebalance (years apart) — so a single daily refresh is generous, and a failed refresh is a non-event.

## What Changes

- **New schema (migration 13), three reference tables:** `wormhole_type` (the dictionary, PK = type-code `identifier`), `eve_system` (PK = `system_id`, name indexed), and `system_static` (join table: `(system_id, static_code)` with FKs to both, so "which systems have a static to nullsec" is answerable in SQL).
- **New daily sync service** `services/eve_system_sync.rs`, **modelled exactly on the existing `token_sweep`**: a `spawn(ctx)` that loops on a 24h `interval` (first tick fires at startup, so the catalog populates on boot), a `run_once(&ctx) -> anyhow::Result<()>` doing the fetch-and-merge, and the same failure contract — a single run's error is logged and never kills the loop. Wired in `main.rs` after `AppState` with cloned handles.
- **Fetch-all-then-write, single transaction.** `run_once` fetches all three sources **first**; only if all three succeed does it open one transaction and upsert all three tables, committing atomically. Postgres MVCC means readers always see a complete prior snapshot until the commit flips to the complete new one — there is **no window of missing or partial data** (except the unavoidable first-ever run on an empty DB).
- **Sanity floor** before the transaction: a fetch that succeeds at the HTTP level but returns an implausibly small payload (e.g. anoikis `200 OK` with `{}`) is rejected, so a valid-but-empty response can never wipe good data via the `system_static` rebuild.
- **anoikis User-Agent:** the anoikis fetch sends an explicit, identifying `User-Agent` header (good-citizen courtesy; the host does not currently reject the default UA, but identifying the client is polite and future-proofs against UA gating).
- **Not doing:** no read API in this change — it only *populates* the tables; system/wormhole read endpoints are a follow-up. No live eve-scout Thera/Turnur connections (`/v2/public/signatures`) — those are transient, expiring *connection* data belonging to a separate path-routing service, not this static catalog. No job registry and no admin-triggered manual import — both deferred to the future job-interface change (this is only job #2; the registry waits until #3 per the deferred-jobs decision). No manual SDE import.

## Capabilities

### New Capabilities
- `eve-system-catalog`: The local reference catalog of EVE systems, the wormhole-type dictionary, and per-system statics — the three tables, the three sources and how they merge (J-code = `eve_system.name`), the daily single-transaction upsert with its sanity floor and failure contract, and the explicit non-goals (no read API, no live connections, no job registry here).

## Impact

- **Schema:** new migration `backend/migrations/00000000000013_create_eve_system_catalog.sql` (three tables + the name index + the two `system_static` FKs). sqlx offline cache (`.sqlx/`) regenerated and committed.
- **Code:** new `backend/src/services/eve_system_sync.rs` (mirrors `token_sweep`); new `backend/src/db/eve_system.rs` (upsert/rebuild functions for the three tables); new `backend/src/esi/` (or a small fetch module) for the three typed fetches incl. the anoikis UA; `backend/src/main.rs` (spawn the sync alongside the token sweep, cloned `pool` + `http_client`). `services/mod.rs` and `db/mod.rs` gain the new modules.
- **Tests:** unit tests for the merge/sanity-floor decision logic (pure, like `token_sweep::decide`); `#[sqlx::test]` `run_once` tests against wiremock-served source payloads asserting the atomic upsert, the FK-ordered writes, that an unmatched anoikis static code is skipped-and-logged (not fatal), that a sub-floor payload aborts without touching existing rows, and that a fetch failure leaves the prior catalog intact.
- **Docs:** `openspec/AGENTS.md` updated in this change — `eve_system`/`wormhole_type`/`system_static` added to the DB list, `eve_system_sync` added to the Services list (daily catalog refresh).
- **Behaviour:** on first boot the catalog is empty until the first sync commits (~1–3s, network-bound); thereafter every refresh is gap-free. The sync adds three outbound fetches/day (two eve-scout, one anoikis) through the rate-limited client.
