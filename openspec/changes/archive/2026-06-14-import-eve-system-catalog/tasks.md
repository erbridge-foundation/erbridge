# Tasks — import-eve-system-catalog

> Backend-only change (schema + background service). No frontend. Follow the
> `rust-rest-api` skill: db layer (queries) below service layer (the sync).

## 1. Schema (migration 13)

- [x] 1.1 Add `backend/migrations/00000000000013_create_eve_system_catalog.sql` creating, in FK order:
  - `wormhole_type` (PK `identifier text`; `type_id int`, `target_system_class text`, `max_jump_mass bigint`, `max_stable_mass bigint`, `max_stable_time int`, `mass_regeneration bigint`, `possible_static bool`, `wandering_only bool`, `signature_level int[]`, `source text[]` — all NOT NULL)
  - `eve_system` (PK `system_id bigint`; `name text NOT NULL`, `class text NOT NULL`, `region_id bigint NOT NULL`, `region_name text NOT NULL`, `security_status real NOT NULL`, `jove_observatory bool NOT NULL DEFAULT false`) + index on `name`
  - `system_static` (`system_id bigint REFERENCES eve_system ON DELETE CASCADE`, `static_code text REFERENCES wormhole_type(identifier)`, `PRIMARY KEY (system_id, static_code)`)
- [x] 1.2 `class` / `target_system_class` stored as plain `text` (no enum, no CHECK) per design.

## 2. DB layer (`backend/src/db/eve_system.rs`)

- [x] 2.1 Typed source structs for the three feeds (deserialize targets): a system row, a wormhole-type row, and the anoikis statics map (`HashMap<String, {static: Vec<String>}>` shape).
- [x] 2.2 `upsert_wormhole_types(&mut tx, &[..])` — `INSERT … ON CONFLICT (identifier) DO UPDATE`.
- [x] 2.3 `upsert_systems(&mut tx, &[..])` — `INSERT … ON CONFLICT (system_id) DO UPDATE`.
- [x] 2.4 `rebuild_statics(&mut tx, &anoikis_map)` — resolve each J-code to its `system_id` and each code to a known `wormhole_type`; delete the affected systems' existing `system_static` rows and reinsert, **all inside the passed `tx`**. Skip + log (count) any `(j-code, code)` with no matching system or type; never abort the run.
- [x] 2.5 Register the module in `db/mod.rs`.

## 3. Source fetch

- [x] 3.1 Add typed fetch functions for the three sources using the shared `ClientWithMiddleware` (so they go through the existing rate-limit/tracing chain). Place per the `rust-rest-api` skill (a small fetch module under `esi/` or the sync service — match the layout the skill dictates; correct any task path that conflicts).
- [x] 3.2 The anoikis fetch sends an explicit, identifying `User-Agent` header (good-citizen courtesy; not currently enforced by the host). Make the UA value config-driven with a sane default.
- [x] 3.3 Source URLs come from config with baked-in defaults (overridable via env to point at fixtures/mirrors in tests).

## 4. Sync service (`backend/src/services/eve_system_sync.rs`, mirror `token_sweep`)

- [x] 4.1 `SyncContext { pool, http, systems_url, wormhole_types_url, statics_url, user_agent }`.
- [x] 4.2 `pub fn spawn(ctx)` — 24h `interval`, first tick immediate, per-run error logged via `error!`, loop never dies (identical contract to `token_sweep::spawn`).
- [x] 4.3 `pub async fn run_once(&ctx) -> anyhow::Result<()>`:
  - Phase 1: fetch all three sources first; any `?` bails before touching the DB.
  - `sanity_check` (pure fn): systems ≥ 5000, wormhole_types ≥ 50, statics ≥ 1000 → else `Err`.
  - Phase 2: one `tx`; `upsert_wormhole_types` → `upsert_systems` → `rebuild_statics`; `commit`.
  - `info!` summary counts (systems / types / statics inserted / skipped).
- [x] 4.4 Register in `services/mod.rs`.
- [x] 4.5 Wire `eve_system_sync::spawn(...)` in `backend/src/main.rs` immediately after the `token_sweep::spawn`, with cloned `state.db` + `state.http_client` and the configured URLs/UA.

## 5. Tests

- [x] 5.1 Unit: `sanity_check` — passes at/above floors, fails just below each of the three floors.
- [x] 5.2 `#[sqlx::test]` `run_once` against wiremock-served payloads: asserts all three tables populated, FK-ordered (a static's type exists), and statics joined to the right system.
- [x] 5.3 `#[sqlx::test]`: an anoikis code with no matching `wormhole_type` (and a J-code with no `eve_system`) is skipped+logged, run still succeeds, other rows present.
- [x] 5.4 `#[sqlx::test]`: a sub-floor payload (e.g. anoikis `{}`) aborts before writing — pre-seed catalog rows and assert they are unchanged after the run errors.
- [x] 5.5 `#[sqlx::test]`: a fetch failure (one mock returns 500) leaves a pre-seeded catalog intact (no transaction opened).
- [x] 5.6 Regenerate and commit the sqlx offline cache (`.sqlx/`) for the new queries.

## 6. Docs

- [x] 6.1 Update `openspec/AGENTS.md`: add `eve_system`, `wormhole_type`, `system_static` to the DB (`db/`) list; add `eve_system_sync` to the Services list (daily catalog refresh from eve-scout + anoikis). Keep it a map, not a changelog.

## 7. Verification

> Backend-only change — the three frontend commands (`pnpm test` / `pnpm run check`
> / `pnpm run test:e2e`) do not apply; no frontend files are touched.

- [x] 7.1 `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` clean.
- [x] 7.2 `cargo test` (unit + integration, incl. the OpenAPI strict + layering tests) green.
- [x] 7.3 Live smoke: run `run_once` (or the booted service) against the real eve-scout + anoikis endpoints once; confirm `eve_system` ≈ 8k rows, `wormhole_type` ≈ 150, `system_static` populated, and a spot-check (`J172840` → `c5` with its statics joined) resolves. Confirm the anoikis fetch succeeds with the custom UA (not 403). *(`#[ignore]`d `live_smoke_populates_real_catalog` test; anoikis statics host is `anoikis.info`, not `anoik.is`; live `wormhole_type` count is 97, comfortably above the 50 floor.)*
