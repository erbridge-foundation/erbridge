//! Daily EVE system-catalog sync.
//!
//! Once a day this refreshes the local reference catalog — the system spine, the
//! wormhole-type dictionary, and per-system statics — by fetching three public
//! feeds (eve-scout `/systems` + `/wormholetypes`, anoikis `wh-statics`) and
//! merging them on the J-code (`anoikis key == eve_system.name`).
//!
//! The data is near-immutable, so the design optimises for gap-free reads, not
//! speed: fetch ALL three sources first (a failure of any aborts before the DB is
//! touched), apply a sanity floor (an HTTP-200-but-empty payload must not wipe
//! good data), then upsert all three tables in ONE transaction. Under Postgres
//! MVCC a concurrent reader sees the complete prior snapshot until the commit
//! flips to the complete new one. Modelled on `token_sweep`.

use std::time::Duration;

use reqwest_middleware::ClientWithMiddleware;
use sqlx::PgPool;
use tokio::time::interval;
use tracing::{error, info};

use crate::db::eve_system::{self, StaticsMap, SystemRow, WormholeTypeRow};
use crate::esi::eve_scout;

/// Sanity-floor thresholds. Well below the observed counts (8,037 systems, ~150
/// types, 2,602 statics) and well above any plausible partial outage, so a
/// valid-but-tiny payload is rejected before it can overwrite good data.
const MIN_SYSTEMS: usize = 5000;
const MIN_WORMHOLE_TYPES: usize = 50;
const MIN_STATICS: usize = 1000;

/// The sync's dependencies, bundled so `spawn`/`run_once` take one argument.
/// Owns its handles (the spawned task outlives the `AppState` they were cloned
/// from).
pub struct SyncContext {
    pub pool: PgPool,
    pub http: ClientWithMiddleware,
    pub systems_url: String,
    pub wormhole_types_url: String,
    pub statics_url: String,
    pub user_agent: String,
}

/// Rejects an implausibly small payload before any write. Pure, unit-tested. A
/// source that parses but returns far too few rows (e.g. anoikis `{}`) must not
/// be allowed to wipe the catalog through the statics rebuild.
fn sanity_check(
    systems: &[SystemRow],
    types: &[WormholeTypeRow],
    statics: &StaticsMap,
) -> anyhow::Result<()> {
    if systems.len() < MIN_SYSTEMS {
        anyhow::bail!(
            "systems payload below sanity floor: {} < {MIN_SYSTEMS}",
            systems.len()
        );
    }
    if types.len() < MIN_WORMHOLE_TYPES {
        anyhow::bail!(
            "wormhole-types payload below sanity floor: {} < {MIN_WORMHOLE_TYPES}",
            types.len()
        );
    }
    if statics.len() < MIN_STATICS {
        anyhow::bail!(
            "statics payload below sanity floor: {} < {MIN_STATICS}",
            statics.len()
        );
    }
    Ok(())
}

/// Spawns the sync on a ~24h interval. The first tick fires immediately at
/// startup so the catalog populates on boot. A single run's failure is logged
/// and never kills the loop. Identical contract to `token_sweep::spawn`.
pub fn spawn(ctx: SyncContext) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(24 * 60 * 60));
        loop {
            ticker.tick().await;
            if let Err(e) = run_once(&ctx).await {
                error!("eve-system catalog sync run failed: {e:#}");
            }
        }
    });
}

/// One full refresh: fetch all three sources, sanity-check, then upsert all three
/// tables in a single transaction. Returns an error (logged by `spawn`) without
/// touching the DB if any fetch fails or the sanity floor rejects the payload.
pub async fn run_once(ctx: &SyncContext) -> anyhow::Result<()> {
    // PHASE 1 — fetch ALL THREE first; the DB is untouched. Any `?` bails the run.
    let systems = eve_scout::fetch_systems(&ctx.http, &ctx.systems_url).await?;
    let types = eve_scout::fetch_wormhole_types(&ctx.http, &ctx.wormhole_types_url).await?;
    let statics = eve_scout::fetch_statics(&ctx.http, &ctx.statics_url, &ctx.user_agent).await?;

    // SANITY FLOOR — refuse to overwrite good data with an implausible payload.
    sanity_check(&systems, &types, &statics)?;

    // PHASE 2 — one transaction; FK-ordered (types before statics); atomic commit.
    let mut tx = ctx.pool.begin().await?;
    eve_system::upsert_wormhole_types(&mut tx, &types).await?;
    eve_system::upsert_systems(&mut tx, &systems).await?;
    let statics_result = eve_system::rebuild_statics(&mut tx, &statics).await?;
    tx.commit().await?;

    info!(
        systems = systems.len(),
        types = types.len(),
        statics_inserted = statics_result.inserted,
        statics_skipped = statics_result.skipped,
        "eve-system catalog synced"
    );
    Ok(())
}

#[cfg(test)]
mod sanity_tests {
    use super::*;
    use std::collections::HashMap;

    use crate::db::eve_system::StaticsEntry;

    fn systems(n: usize) -> Vec<SystemRow> {
        (0..n)
            .map(|i| SystemRow {
                system_id: i as i64,
                system_name: format!("S{i}"),
                system_class: "hs".into(),
                region_id: 1,
                region_name: "R".into(),
                security_status: 0.5,
                jove_observatory: false,
            })
            .collect()
    }

    fn types(n: usize) -> Vec<WormholeTypeRow> {
        (0..n)
            .map(|i| WormholeTypeRow {
                identifier: format!("T{i}"),
                type_id: i as i32,
                target_system_class: "ns".into(),
                max_jump_mass: 1,
                max_stable_mass: 1,
                max_stable_time: 1,
                mass_regeneration: 0,
                possible_static: true,
                wandering_only: false,
                signature_level: vec![],
                source: vec![],
            })
            .collect()
    }

    fn statics(n: usize) -> StaticsMap {
        let mut m = HashMap::new();
        for i in 0..n {
            m.insert(
                format!("J{i:06}"),
                StaticsEntry {
                    statics: vec!["T0".into()],
                },
            );
        }
        m
    }

    #[test]
    fn passes_at_the_floors() {
        assert!(
            sanity_check(
                &systems(MIN_SYSTEMS),
                &types(MIN_WORMHOLE_TYPES),
                &statics(MIN_STATICS)
            )
            .is_ok()
        );
    }

    #[test]
    fn fails_just_below_systems_floor() {
        assert!(
            sanity_check(
                &systems(MIN_SYSTEMS - 1),
                &types(MIN_WORMHOLE_TYPES),
                &statics(MIN_STATICS)
            )
            .is_err()
        );
    }

    #[test]
    fn fails_just_below_types_floor() {
        assert!(
            sanity_check(
                &systems(MIN_SYSTEMS),
                &types(MIN_WORMHOLE_TYPES - 1),
                &statics(MIN_STATICS)
            )
            .is_err()
        );
    }

    #[test]
    fn fails_just_below_statics_floor() {
        assert!(
            sanity_check(
                &systems(MIN_SYSTEMS),
                &types(MIN_WORMHOLE_TYPES),
                &statics(MIN_STATICS - 1)
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod run_once_tests {
    use super::*;
    use reqwest_middleware::ClientBuilder;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn http() -> ClientWithMiddleware {
        ClientBuilder::new(reqwest::Client::new()).build()
    }

    /// A full set of plausible payloads (above the sanity floors) so `run_once`
    /// reaches the write phase. `J000000` is a real system with a `D845` static;
    /// the rest are filler to clear the floors.
    fn systems_body() -> serde_json::Value {
        let mut v: Vec<serde_json::Value> = (0..MIN_SYSTEMS)
            .map(|i| {
                json!({
                    "system_id": 30_000_000_i64 + i as i64,
                    "system_name": format!("Filler{i}"),
                    "system_class": "hs",
                    "region_id": 10_000_001,
                    "region_name": "TheForge",
                    "security_status": 0.9,
                })
            })
            .collect();
        // The system under test: a J-code with a known static.
        v.push(json!({
            "system_id": 31_002_274_i64,
            "system_name": "J172840",
            "system_class": "c5",
            "region_id": 11_000_028,
            "region_name": "D-R00021",
            "security_status": -1.0,
            "jove_observatory": false,
        }));
        json!(v)
    }

    fn types_body() -> serde_json::Value {
        let mut v: Vec<serde_json::Value> = (0..MIN_WORMHOLE_TYPES)
            .map(|i| {
                json!({
                    "identifier": format!("F{i:03}"),
                    "type_id": 30000 + i as i32,
                    "target_system_class": "ns",
                    "max_jump_mass": 375_000_000_i64,
                    "max_stable_mass": 3_000_000_000_i64,
                    "max_stable_time": 1440,
                    "mass_regeneration": 0,
                    "possible_static": true,
                    "wandering_only": false,
                    "signature_level": [1],
                    "source": ["c5"],
                })
            })
            .collect();
        v.push(json!({
            "identifier": "D845",
            "type_id": 30124,
            "target_system_class": "hs",
            "max_jump_mass": 375_000_000_i64,
            "max_stable_mass": 5_000_000_000_i64,
            "max_stable_time": 1440,
            "mass_regeneration": 0,
            "possible_static": true,
            "wandering_only": false,
            "signature_level": [1, 2],
            "source": ["c5"],
        }));
        json!(v)
    }

    fn statics_body() -> serde_json::Value {
        let mut m = serde_json::Map::new();
        for i in 0..MIN_STATICS {
            m.insert(format!("X{i:06}"), json!({ "static": ["D845"] }));
        }
        // The real entry under test; its J-code matches the seeded system.
        m.insert("J172840".into(), json!({ "static": ["D845"] }));
        serde_json::Value::Object(m)
    }

    /// Mounts all three sources on one mock server and returns a `SyncContext`
    /// pointed at it. `bodies` overrides let individual tests degrade a source.
    async fn mock_ctx(
        pool: &PgPool,
        systems: ResponseTemplate,
        types: ResponseTemplate,
        statics: ResponseTemplate,
    ) -> (MockServer, SyncContext) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/systems"))
            .respond_with(systems)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/wormholetypes"))
            .respond_with(types)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/wh-statics.json"))
            .respond_with(statics)
            .mount(&server)
            .await;
        let ctx = SyncContext {
            pool: pool.clone(),
            http: http(),
            systems_url: format!("{}/systems", server.uri()),
            wormhole_types_url: format!("{}/wormholetypes", server.uri()),
            statics_url: format!("{}/wh-statics.json", server.uri()),
            user_agent: "test-agent".into(),
        };
        (server, ctx)
    }

    fn ok(body: serde_json::Value) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(body)
    }

    /// Seed one type, one system and one static so abort-paths can assert the
    /// prior catalog is left intact.
    async fn seed_existing(pool: &PgPool) {
        sqlx::query!(
            r#"INSERT INTO wormhole_type (identifier, type_id, target_system_class,
                 max_jump_mass, max_stable_mass, max_stable_time, mass_regeneration,
                 possible_static, wandering_only, signature_level, source)
               VALUES ('OLD', 1, 'ns', 1, 1, 1, 0, true, false, '{}', '{}')"#
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query!(
            r#"INSERT INTO eve_system (system_id, name, class, region_id, region_name,
                 security_status, jove_observatory)
               VALUES (99, 'OldSystem', 'hs', 1, 'R', 0.5, false)"#
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query!("INSERT INTO system_static (system_id, static_code) VALUES (99, 'OLD')")
            .execute(pool)
            .await
            .unwrap();
    }

    async fn count(pool: &PgPool, table: &str) -> i64 {
        // table names are literals from the tests, not user input.
        let sql = format!("SELECT COUNT(*) AS c FROM {table}");
        sqlx::query_scalar::<_, i64>(&sql)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[sqlx::test]
    async fn populates_all_three_tables_fk_ordered(pool: PgPool) {
        let (_s, ctx) = mock_ctx(
            &pool,
            ok(systems_body()),
            ok(types_body()),
            ok(statics_body()),
        )
        .await;

        run_once(&ctx).await.unwrap();

        assert_eq!(count(&pool, "eve_system").await, MIN_SYSTEMS as i64 + 1);
        assert_eq!(
            count(&pool, "wormhole_type").await,
            MIN_WORMHOLE_TYPES as i64 + 1
        );
        // Every static row's J-code resolved to the right system, and its type
        // exists (FK satisfied) — assert the J172840 → D845 join specifically.
        let row = sqlx::query!(
            r#"SELECT e.name, e.class, w.identifier AS code
               FROM system_static s
               JOIN eve_system e ON e.system_id = s.system_id
               JOIN wormhole_type w ON w.identifier = s.static_code
               WHERE e.name = 'J172840'"#
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.class, "c5");
        assert_eq!(row.code, "D845");
    }

    #[sqlx::test]
    async fn unmatched_static_is_skipped_not_fatal(pool: PgPool) {
        // statics references a code with no type AND a J-code with no system; the
        // run must still succeed and the valid rows must land.
        let mut m = serde_json::Map::new();
        for i in 0..MIN_STATICS {
            m.insert(format!("X{i:06}"), json!({ "static": ["D845"] }));
        }
        // Unknown type code for a real system.
        m.insert("J172840".into(), json!({ "static": ["D845", "ZZZZ"] }));
        // Unknown J-code (no such system).
        m.insert("J999999".into(), json!({ "static": ["D845"] }));
        let statics = serde_json::Value::Object(m);

        let (_s, ctx) = mock_ctx(&pool, ok(systems_body()), ok(types_body()), ok(statics)).await;
        run_once(&ctx).await.unwrap();

        // The valid D845 static for J172840 landed; ZZZZ and J999999 were skipped.
        let n = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM system_static s JOIN eve_system e
               ON e.system_id = s.system_id WHERE e.name = 'J172840'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(n, 1);
        // No row references the unknown code.
        assert_eq!(
            count(&pool, "system_static WHERE static_code = 'ZZZZ'").await,
            0
        );
    }

    #[sqlx::test]
    async fn sub_floor_payload_aborts_without_writing(pool: PgPool) {
        seed_existing(&pool).await;
        // anoikis returns `{}` — valid JSON, but below the statics floor.
        let (_s, ctx) = mock_ctx(&pool, ok(systems_body()), ok(types_body()), ok(json!({}))).await;

        let err = run_once(&ctx).await;
        assert!(err.is_err());

        // Prior catalog untouched — no system/type overwrite, the old static stays.
        assert_eq!(count(&pool, "eve_system").await, 1);
        assert_eq!(count(&pool, "wormhole_type").await, 1);
        assert_eq!(count(&pool, "system_static").await, 1);
    }

    #[sqlx::test]
    async fn fetch_failure_leaves_catalog_intact(pool: PgPool) {
        seed_existing(&pool).await;
        // The wormholetypes source 500s; no transaction should open.
        let (_s, ctx) = mock_ctx(
            &pool,
            ok(systems_body()),
            ResponseTemplate::new(500),
            ok(statics_body()),
        )
        .await;

        assert!(run_once(&ctx).await.is_err());

        assert_eq!(count(&pool, "eve_system").await, 1);
        assert_eq!(count(&pool, "wormhole_type").await, 1);
        assert_eq!(count(&pool, "system_static").await, 1);
    }

    /// Live smoke against the real eve-scout + anoikis endpoints. `#[ignore]`d so
    /// the normal suite stays offline; run with
    /// `cargo test live_smoke -- --ignored --nocapture`. Confirms the catalog
    /// populates at scale, the anoikis fetch succeeds with the custom UA, and the
    /// J172840 → c5 + statics join resolves.
    #[sqlx::test]
    #[ignore = "hits live eve-scout + anoikis; run with --ignored"]
    async fn live_smoke_populates_real_catalog(pool: PgPool) {
        use crate::config::CatalogConfig;
        let cfg = CatalogConfig::default();
        let ctx = SyncContext {
            pool: pool.clone(),
            http: http(),
            systems_url: cfg.systems_url,
            wormhole_types_url: cfg.wormhole_types_url,
            statics_url: cfg.statics_url,
            user_agent: cfg.user_agent,
        };

        run_once(&ctx).await.unwrap();

        assert!(count(&pool, "eve_system").await > 7000);
        assert!(count(&pool, "wormhole_type").await > 80);
        assert!(count(&pool, "system_static").await > 1000);

        // Spot-check: J172840 is a c5 with its statics joined to known types.
        let row = sqlx::query!(
            r#"SELECT e.class, COUNT(s.static_code) AS "statics!"
               FROM eve_system e
               LEFT JOIN system_static s ON s.system_id = e.system_id
               WHERE e.name = 'J172840'
               GROUP BY e.class"#
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.class, "c5");
        assert!(row.statics >= 1);
    }
}
