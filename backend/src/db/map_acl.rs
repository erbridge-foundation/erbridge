use std::collections::HashMap;

use anyhow::{Context, Result};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// Attaches an ACL to a map. Idempotent: a duplicate `(map_id, acl_id)` is a
/// no-op (`ON CONFLICT DO NOTHING`) rather than an error.
pub async fn attach_acl(
    tx: &mut Transaction<'_, Postgres>,
    map_id: Uuid,
    acl_id: Uuid,
) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO map_acl (map_id, acl_id)
        VALUES ($1, $2)
        ON CONFLICT (map_id, acl_id) DO NOTHING
        "#,
        map_id,
        acl_id,
    )
    .execute(&mut **tx)
    .await
    .context("failed to attach acl to map")?;
    Ok(())
}

/// Detaches an ACL from a map. Returns `true` if a link was removed.
pub async fn detach_acl(
    tx: &mut Transaction<'_, Postgres>,
    map_id: Uuid,
    acl_id: Uuid,
) -> Result<bool> {
    let result = sqlx::query!(
        "DELETE FROM map_acl WHERE map_id = $1 AND acl_id = $2",
        map_id,
        acl_id,
    )
    .execute(&mut **tx)
    .await
    .context("failed to detach acl from map")?;
    Ok(result.rows_affected() > 0)
}

/// For a set of map ids, returns a map of `map_id -> [acl_id, …]` for the ACLs
/// attached to each. Maps with no attachments are absent from the result.
pub async fn find_acl_ids_for_maps(
    pool: &sqlx::PgPool,
    map_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<Uuid>>> {
    let rows = sqlx::query!(
        r#"
        SELECT map_id, acl_id
        FROM map_acl
        WHERE map_id = ANY($1)
        "#,
        map_ids,
    )
    .fetch_all(pool)
    .await
    .context("failed to fetch acl ids for maps")?;

    let mut out: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for r in rows {
        out.entry(r.map_id).or_default().push(r.acl_id);
    }
    Ok(out)
}

#[cfg(test)]
/// Attaches through a one-shot transaction — convenience for other modules'
/// tests.
pub async fn attach_acl_pool(pool: &sqlx::PgPool, map_id: Uuid, acl_id: Uuid) -> Result<()> {
    let mut tx = pool.begin().await.unwrap();
    attach_acl(&mut tx, map_id, acl_id).await?;
    tx.commit().await.unwrap();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::acl::insert_acl_for_test;
    use crate::db::map::insert_map;
    use crate::db::{accounts, map_acl};
    use sqlx::PgPool;

    async fn setup(pool: &PgPool) -> (Uuid, Uuid) {
        let owner = accounts::create_account(pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let m = insert_map(&mut tx, owner, "M", "slug-1", None)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let a = insert_acl_for_test(pool, owner, "A").await;
        (m.id, a.id)
    }

    #[sqlx::test]
    async fn attach_then_find(pool: PgPool) {
        let (map_id, acl_id) = setup(&pool).await;
        map_acl::attach_acl_pool(&pool, map_id, acl_id)
            .await
            .unwrap();

        let attached = find_acl_ids_for_maps(&pool, &[map_id]).await.unwrap();
        assert_eq!(attached.get(&map_id).unwrap(), &vec![acl_id]);
    }

    #[sqlx::test]
    async fn attach_is_idempotent(pool: PgPool) {
        let (map_id, acl_id) = setup(&pool).await;
        map_acl::attach_acl_pool(&pool, map_id, acl_id)
            .await
            .unwrap();
        // Second attach must not error on the PK collision.
        map_acl::attach_acl_pool(&pool, map_id, acl_id)
            .await
            .unwrap();

        let attached = find_acl_ids_for_maps(&pool, &[map_id]).await.unwrap();
        assert_eq!(attached.get(&map_id).unwrap().len(), 1);
    }

    #[sqlx::test]
    async fn detach_removes_link(pool: PgPool) {
        let (map_id, acl_id) = setup(&pool).await;
        map_acl::attach_acl_pool(&pool, map_id, acl_id)
            .await
            .unwrap();

        let mut tx = pool.begin().await.unwrap();
        let removed = detach_acl(&mut tx, map_id, acl_id).await.unwrap();
        tx.commit().await.unwrap();
        assert!(removed);

        let attached = find_acl_ids_for_maps(&pool, &[map_id]).await.unwrap();
        assert!(!attached.contains_key(&map_id));
    }

    #[sqlx::test]
    async fn detach_absent_returns_false(pool: PgPool) {
        let (map_id, acl_id) = setup(&pool).await;
        let mut tx = pool.begin().await.unwrap();
        let removed = detach_acl(&mut tx, map_id, acl_id).await.unwrap();
        tx.commit().await.unwrap();
        assert!(!removed);
    }
}
