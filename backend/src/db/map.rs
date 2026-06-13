use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::db::DbError;

/// A map: an account-owned, soft-deletable container. Soft-delete is expressed
/// via `status` (`active` vs a deleted state) plus `delete_requested_at`,
/// mirroring the `account` table convention.
#[derive(Debug, Clone)]
pub struct Map {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub owner_account_id: Option<Uuid>,
    pub description: Option<String>,
    pub status: String,
    pub delete_requested_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A map annotated with the ACLs attached to it that the requesting account can
/// manage. Built by the service layer's list operation; `acls` is `(id, name)`.
#[derive(Debug, Clone)]
pub struct MapWithAcls {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub owner_account_id: Option<Uuid>,
    pub description: Option<String>,
    pub status: String,
    pub delete_requested_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub acls: Vec<(Uuid, String)>,
}

/// Inserts a map owned by `owner_account_id`. A slug collision surfaces as
/// `DbError::UniqueViolation` so the service can map it to a typed conflict.
pub async fn insert_map(
    tx: &mut Transaction<'_, Postgres>,
    owner_account_id: Uuid,
    name: &str,
    slug: &str,
    description: Option<&str>,
) -> Result<Map, DbError> {
    let m = sqlx::query_as!(
        Map,
        r#"
        INSERT INTO map (name, slug, owner_account_id, description)
        VALUES ($1, $2, $3, $4)
        RETURNING id, name, slug, owner_account_id, description,
                  status, delete_requested_at, created_at, updated_at
        "#,
        name,
        slug,
        owner_account_id,
        description,
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(m)
}

/// Looks up a map by id regardless of status. Soft-deleted maps are returned;
/// callers that must exclude them check `status`.
pub async fn find_map_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Map>> {
    sqlx::query_as!(
        Map,
        r#"
        SELECT id, name, slug, owner_account_id, description,
               status, delete_requested_at, created_at, updated_at
        FROM map
        WHERE id = $1
        "#,
        id,
    )
    .fetch_optional(pool)
    .await
    .context("failed to fetch map by id")
}

/// Looks up an active map by slug. Soft-deleted maps are excluded (a soft-deleted
/// slug is indistinguishable from an unknown one — both yield `None`).
pub async fn find_active_map_by_slug(pool: &PgPool, slug: &str) -> Result<Option<Map>> {
    sqlx::query_as!(
        Map,
        r#"
        SELECT id, name, slug, owner_account_id, description,
               status, delete_requested_at, created_at, updated_at
        FROM map
        WHERE slug = $1 AND status = 'active'
        "#,
        slug,
    )
    .fetch_optional(pool)
    .await
    .context("failed to fetch active map by slug")
}

/// Updates a map's name, slug, and description. A slug collision surfaces as
/// `DbError::UniqueViolation`. Returns `None` if no active map with that id
/// exists.
pub async fn update_map(
    pool: &PgPool,
    id: Uuid,
    name: &str,
    slug: &str,
    description: Option<&str>,
) -> Result<Option<Map>, DbError> {
    let m = sqlx::query_as!(
        Map,
        r#"
        UPDATE map
        SET name = $2, slug = $3, description = $4, updated_at = now()
        WHERE id = $1 AND status = 'active'
        RETURNING id, name, slug, owner_account_id, description,
                  status, delete_requested_at, created_at, updated_at
        "#,
        id,
        name,
        slug,
        description,
    )
    .fetch_optional(pool)
    .await?;

    Ok(m)
}

/// Soft-deletes a map: sets `status = 'deleted'` and stamps
/// `delete_requested_at`. Returns `true` if an active map was deleted.
pub async fn soft_delete_map(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<bool> {
    let result = sqlx::query!(
        r#"
        UPDATE map
        SET status = 'deleted', delete_requested_at = now(), updated_at = now()
        WHERE id = $1 AND status = 'active'
        "#,
        id,
    )
    .execute(&mut **tx)
    .await
    .context("failed to soft-delete map")?;

    Ok(result.rows_affected() > 0)
}

/// Returns every active map the account can read: maps it owns, plus maps with
/// a resolved non-`deny` grant via an attached ACL (matching the account's
/// characters by direct character / corporation / alliance membership). A
/// `deny` on a map removes it from the list even if another ACL grants access.
pub async fn find_maps_for_account(pool: &PgPool, account_id: Uuid) -> Result<Vec<Map>> {
    sqlx::query_as!(
        Map,
        r#"
        SELECT m.id, m.name, m.slug, m.owner_account_id, m.description,
               m.status, m.delete_requested_at, m.created_at, m.updated_at
        FROM map m
        WHERE m.status = 'active'
          AND (
              m.owner_account_id = $1
              OR (
                  EXISTS (
                      SELECT 1
                      FROM map_acl ma
                      JOIN acl_member am ON am.acl_id = ma.acl_id
                      JOIN eve_character ec ON ec.account_id = $1
                      WHERE ma.map_id = m.id
                        AND am.permission <> 'deny'
                        AND (
                            (am.member_type = 'character'   AND am.character_id  = ec.id)
                        OR  (am.member_type = 'corporation' AND am.eve_entity_id = ec.corporation_id)
                        OR  (am.member_type = 'alliance'    AND am.eve_entity_id = ec.alliance_id
                                                            AND ec.alliance_id IS NOT NULL)
                        )
                  )
                  AND NOT EXISTS (
                      SELECT 1
                      FROM map_acl ma
                      JOIN acl_member am ON am.acl_id = ma.acl_id
                      JOIN eve_character ec ON ec.account_id = $1
                      WHERE ma.map_id = m.id
                        AND am.permission = 'deny'
                        AND (
                            (am.member_type = 'character'   AND am.character_id  = ec.id)
                        OR  (am.member_type = 'corporation' AND am.eve_entity_id = ec.corporation_id)
                        OR  (am.member_type = 'alliance'    AND am.eve_entity_id = ec.alliance_id
                                                            AND ec.alliance_id IS NOT NULL)
                        )
                  )
              )
          )
        ORDER BY m.created_at DESC
        "#,
        account_id,
    )
    .fetch_all(pool)
    .await
    .context("failed to list maps for account")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::{insert_character, insert_character_full};
    use crate::db::{accounts, acl, acl_member, map_acl};

    async fn insert_active_map(pool: &PgPool, owner: Uuid, slug: &str) -> Map {
        let mut tx = pool.begin().await.unwrap();
        let m = insert_map(&mut tx, owner, "Test Map", slug, Some("desc"))
            .await
            .unwrap();
        tx.commit().await.unwrap();
        m
    }

    #[sqlx::test]
    async fn insert_then_find_returns_same_row(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let m = insert_active_map(&pool, owner, "alpha").await;

        let found = find_map_by_id(&pool, m.id).await.unwrap().unwrap();
        assert_eq!(found.id, m.id);
        assert_eq!(found.name, "Test Map");
        assert_eq!(found.slug, "alpha");
        assert_eq!(found.owner_account_id, Some(owner));
        assert_eq!(found.status, "active");
        assert!(found.delete_requested_at.is_none());
    }

    #[sqlx::test]
    async fn find_by_slug_returns_active_and_excludes_deleted(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let m = insert_active_map(&pool, owner, "by-slug").await;

        let found = find_active_map_by_slug(&pool, "by-slug")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.id, m.id);

        // Unknown slug is None.
        assert!(
            find_active_map_by_slug(&pool, "nope")
                .await
                .unwrap()
                .is_none()
        );

        // Soft-deleted slug is None.
        let mut tx = pool.begin().await.unwrap();
        soft_delete_map(&mut tx, m.id).await.unwrap();
        tx.commit().await.unwrap();
        assert!(
            find_active_map_by_slug(&pool, "by-slug")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[sqlx::test]
    async fn insert_duplicate_slug_returns_unique_violation(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        insert_active_map(&pool, owner, "dup").await;

        let mut tx = pool.begin().await.unwrap();
        let err = insert_map(&mut tx, owner, "Other", "dup", None)
            .await
            .unwrap_err();
        assert!(matches!(err, DbError::UniqueViolation { .. }));
    }

    #[sqlx::test]
    async fn update_changes_fields(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let m = insert_active_map(&pool, owner, "before").await;

        let updated = update_map(&pool, m.id, "Renamed", "after", None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.slug, "after");
        assert!(updated.description.is_none());
    }

    #[sqlx::test]
    async fn update_slug_collision_returns_unique_violation(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        insert_active_map(&pool, owner, "taken").await;
        let m = insert_active_map(&pool, owner, "free").await;

        let err = update_map(&pool, m.id, "x", "taken", None)
            .await
            .unwrap_err();
        assert!(matches!(err, DbError::UniqueViolation { .. }));
    }

    #[sqlx::test]
    async fn soft_delete_sets_status_and_excludes_from_update(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let m = insert_active_map(&pool, owner, "doomed").await;

        let mut tx = pool.begin().await.unwrap();
        let deleted = soft_delete_map(&mut tx, m.id).await.unwrap();
        tx.commit().await.unwrap();
        assert!(deleted);

        let found = find_map_by_id(&pool, m.id).await.unwrap().unwrap();
        assert_eq!(found.status, "deleted");
        assert!(found.delete_requested_at.is_some());

        // A soft-deleted map is no longer updatable.
        let no_update = update_map(&pool, m.id, "x", "y", None).await.unwrap();
        assert!(no_update.is_none());
    }

    #[sqlx::test]
    async fn soft_delete_already_deleted_returns_false(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let m = insert_active_map(&pool, owner, "twice").await;

        let mut tx = pool.begin().await.unwrap();
        assert!(soft_delete_map(&mut tx, m.id).await.unwrap());
        assert!(!soft_delete_map(&mut tx, m.id).await.unwrap());
        tx.commit().await.unwrap();
    }

    #[sqlx::test]
    async fn list_includes_owned_excludes_others(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let other = accounts::create_account(&pool).await.unwrap();
        let mine = insert_active_map(&pool, owner, "mine").await;
        let _theirs = insert_active_map(&pool, other, "theirs").await;

        let maps = find_maps_for_account(&pool, owner).await.unwrap();
        assert_eq!(maps.len(), 1);
        assert_eq!(maps[0].id, mine.id);
    }

    #[sqlx::test]
    async fn list_excludes_soft_deleted_owned(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let m = insert_active_map(&pool, owner, "gone").await;
        let mut tx = pool.begin().await.unwrap();
        soft_delete_map(&mut tx, m.id).await.unwrap();
        tx.commit().await.unwrap();

        assert!(
            find_maps_for_account(&pool, owner)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[sqlx::test]
    async fn list_includes_map_granted_via_corporation_acl(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let member = accounts::create_account(&pool).await.unwrap();
        // member's character is in corp 5000.
        insert_character_full(&pool, member, 200, "Grunt", 5000, None).await;

        let m = insert_active_map(&pool, owner, "shared").await;
        let a = acl::insert_acl_for_test(&pool, owner, "Corp ACL").await;
        acl_member::add_member(&pool, a.id, "corporation", Some(5000), None, "Corp", "read")
            .await
            .unwrap();
        map_acl::attach_acl_pool(&pool, m.id, a.id).await.unwrap();

        let maps = find_maps_for_account(&pool, member).await.unwrap();
        assert_eq!(maps.len(), 1);
        assert_eq!(maps[0].id, m.id);
    }

    #[sqlx::test]
    async fn list_deny_overrides_grant(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let member = accounts::create_account(&pool).await.unwrap();
        let char_id = insert_character(&pool, member, 300, "Denied").await;

        let m = insert_active_map(&pool, owner, "denied-map").await;
        let a = acl::insert_acl_for_test(&pool, owner, "Mixed ACL").await;
        acl_member::add_member(
            &pool,
            a.id,
            "corporation",
            Some(1_000_001),
            None,
            "Corp",
            "read",
        )
        .await
        .unwrap();
        acl_member::add_member(
            &pool,
            a.id,
            "character",
            None,
            Some(char_id),
            "Denied",
            "deny",
        )
        .await
        .unwrap();
        map_acl::attach_acl_pool(&pool, m.id, a.id).await.unwrap();

        assert!(
            find_maps_for_account(&pool, member)
                .await
                .unwrap()
                .is_empty()
        );
    }
}
