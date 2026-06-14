//! The EVE system reference catalog: upserts for the three tables populated by
//! the daily `eve_system_sync` service. Pure persistence — the merge orchestration
//! and sanity floor live in the service layer.

use std::collections::HashMap;

use serde::Deserialize;
use sqlx::{Postgres, Transaction};
use tracing::warn;

/// One system from eve-scout `GET /v2/public/systems`. The J-code systems carry
/// their class here, so the catalog needs no SDE. Extra fields in the feed are
/// ignored.
#[derive(Debug, Clone, Deserialize)]
pub struct SystemRow {
    pub system_id: i64,
    pub system_name: String,
    pub system_class: String,
    pub region_id: i64,
    pub region_name: String,
    pub security_status: f32,
    #[serde(default)]
    pub jove_observatory: bool,
}

/// One wormhole type from eve-scout `GET /v2/public/wormholetypes`.
#[derive(Debug, Clone, Deserialize)]
pub struct WormholeTypeRow {
    pub identifier: String,
    pub type_id: i32,
    pub target_system_class: String,
    pub max_jump_mass: i64,
    pub max_stable_mass: i64,
    pub max_stable_time: i32,
    pub mass_regeneration: i64,
    pub possible_static: bool,
    pub wandering_only: bool,
    pub signature_level: Vec<i32>,
    pub source: Vec<String>,
}

/// One anoikis entry value: `{"static": ["D845","N062"]}`. The map key is the
/// J-code (== `eve_system.name`).
#[derive(Debug, Clone, Deserialize)]
pub struct StaticsEntry {
    #[serde(rename = "static")]
    pub statics: Vec<String>,
}

/// The anoikis `wh-statics.json` shape: J-code -> its static codes.
pub type StaticsMap = HashMap<String, StaticsEntry>;

/// Bulk-upsert the wormhole-type dictionary (~150 rows). Parents of
/// `system_static`, so this runs first in the refresh transaction. Per-row
/// inserts: the array-of-array columns (`signature_level`, `source`) do not
/// UNNEST cleanly and 150 round-trips inside one transaction are trivial here.
pub async fn upsert_wormhole_types(
    tx: &mut Transaction<'_, Postgres>,
    types: &[WormholeTypeRow],
) -> anyhow::Result<()> {
    for t in types {
        sqlx::query!(
            r#"
            INSERT INTO wormhole_type (
                identifier, type_id, target_system_class, max_jump_mass,
                max_stable_mass, max_stable_time, mass_regeneration,
                possible_static, wandering_only, signature_level, source
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (identifier) DO UPDATE SET
                type_id             = excluded.type_id,
                target_system_class = excluded.target_system_class,
                max_jump_mass       = excluded.max_jump_mass,
                max_stable_mass     = excluded.max_stable_mass,
                max_stable_time     = excluded.max_stable_time,
                mass_regeneration   = excluded.mass_regeneration,
                possible_static     = excluded.possible_static,
                wandering_only      = excluded.wandering_only,
                signature_level     = excluded.signature_level,
                source              = excluded.source
            "#,
            t.identifier,
            t.type_id,
            t.target_system_class,
            t.max_jump_mass,
            t.max_stable_mass,
            t.max_stable_time,
            t.mass_regeneration,
            t.possible_static,
            t.wandering_only,
            &t.signature_level,
            &t.source,
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// Bulk-upsert the system spine (~8k rows) in a single round-trip via UNNEST.
pub async fn upsert_systems(
    tx: &mut Transaction<'_, Postgres>,
    systems: &[SystemRow],
) -> anyhow::Result<()> {
    let ids: Vec<i64> = systems.iter().map(|s| s.system_id).collect();
    let names: Vec<String> = systems.iter().map(|s| s.system_name.clone()).collect();
    let classes: Vec<String> = systems.iter().map(|s| s.system_class.clone()).collect();
    let region_ids: Vec<i64> = systems.iter().map(|s| s.region_id).collect();
    let region_names: Vec<String> = systems.iter().map(|s| s.region_name.clone()).collect();
    let secs: Vec<f32> = systems.iter().map(|s| s.security_status).collect();
    let joves: Vec<bool> = systems.iter().map(|s| s.jove_observatory).collect();

    sqlx::query!(
        r#"
        INSERT INTO eve_system (
            system_id, name, class, region_id, region_name,
            security_status, jove_observatory
        )
        SELECT * FROM UNNEST(
            $1::bigint[], $2::text[], $3::text[], $4::bigint[],
            $5::text[], $6::real[], $7::bool[]
        )
        ON CONFLICT (system_id) DO UPDATE SET
            name             = excluded.name,
            class            = excluded.class,
            region_id        = excluded.region_id,
            region_name      = excluded.region_name,
            security_status  = excluded.security_status,
            jove_observatory = excluded.jove_observatory
        "#,
        &ids,
        &names,
        &classes,
        &region_ids,
        &region_names,
        &secs,
        &joves,
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Outcome counts from a statics rebuild, for the run's summary log.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct StaticsResult {
    pub inserted: u64,
    pub skipped: u64,
}

/// Rebuild `system_static` from the anoikis map, inside the passed transaction.
///
/// Resolves each J-code to its `system_id` and validates each code against the
/// known `wormhole_type` set. For every system that appears in the map we delete
/// its existing statics and reinsert the resolved set, so a system whose statics
/// were rebalanced ends up with exactly the new set. A `(j-code, code)` pair with
/// no matching system or no matching type is skipped and counted — never fatal.
///
/// Must run AFTER [`upsert_wormhole_types`] and [`upsert_systems`] in the same
/// transaction so the name->id and code lookups see the fresh data.
pub async fn rebuild_statics(
    tx: &mut Transaction<'_, Postgres>,
    statics: &StaticsMap,
) -> anyhow::Result<StaticsResult> {
    // Resolve names -> ids and the valid type-code set in two reads, then do the
    // matching in Rust (cheap; avoids a per-row round-trip).
    let name_to_id: HashMap<String, i64> = sqlx::query!("SELECT system_id, name FROM eve_system")
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .map(|r| (r.name, r.system_id))
        .collect();

    let known_types: std::collections::HashSet<String> =
        sqlx::query!("SELECT identifier FROM wormhole_type")
            .fetch_all(&mut **tx)
            .await?
            .into_iter()
            .map(|r| r.identifier)
            .collect();

    let mut result = StaticsResult::default();

    for (j_code, entry) in statics {
        let Some(&system_id) = name_to_id.get(j_code) else {
            // J-code with no matching system: skip the whole entry, counting each
            // would-be static.
            result.skipped += entry.statics.len() as u64;
            continue;
        };

        // Resolve this system's valid static codes; drop unknown ones.
        let mut codes: Vec<String> = Vec::with_capacity(entry.statics.len());
        for code in &entry.statics {
            if known_types.contains(code) {
                codes.push(code.clone());
            } else {
                result.skipped += 1;
            }
        }

        // Rebuild this system's statics inside the txn: delete then reinsert, so
        // a rebalanced system reflects exactly the new set.
        sqlx::query!("DELETE FROM system_static WHERE system_id = $1", system_id)
            .execute(&mut **tx)
            .await?;

        if codes.is_empty() {
            continue;
        }

        let n = sqlx::query!(
            r#"
            INSERT INTO system_static (system_id, static_code)
            SELECT $1, code FROM UNNEST($2::text[]) AS code
            ON CONFLICT (system_id, static_code) DO NOTHING
            "#,
            system_id,
            &codes,
        )
        .execute(&mut **tx)
        .await?
        .rows_affected();
        result.inserted += n;
    }

    if result.skipped > 0 {
        warn!(
            skipped = result.skipped,
            "eve-system statics: skipped statics with no matching system or type"
        );
    }

    Ok(result)
}
