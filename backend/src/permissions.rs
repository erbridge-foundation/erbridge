//! Map access-control resolution.
//!
//! Turns "this account's characters, in these corps/alliances" into an
//! effective [`Permission`] on a given map, reading the ACLs attached to it.

use anyhow::{Context, Result};
use sqlx::PgPool;
use strum::{Display, EnumString, IntoStaticStr};
use uuid::Uuid;

/// An effective permission level on a map, ordered least-to-most permissive.
/// `deny` is deliberately **not** a variant: it is a veto handled by the
/// resolver, never an ordinary grant. The `Ord` derive backs the
/// most-permissive-wins comparison; variant order (top = least) defines it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Display, EnumString, IntoStaticStr,
)]
#[strum(serialize_all = "snake_case")]
pub enum Permission {
    Read,
    ReadWrite,
    Manage,
    Admin,
}

impl Permission {
    pub fn as_str(self) -> &'static str {
        self.into()
    }

    /// Parses a grant level. `deny` (and any unknown value) yields `None` —
    /// `deny` is not a permission level, so it never participates in the
    /// most-permissive comparison. Backed by the strum `FromStr` impl, in which
    /// `deny` is simply not a variant.
    pub fn parse(s: &str) -> Option<Self> {
        s.parse().ok()
    }
}

/// Resolves the effective permission for `account_id` on `map_id`.
///
/// Returns `None` if the account has no access — no matching ACL member, or a
/// `deny` member overriding all grants.
///
/// Resolution rules:
/// - The owner of an **active** map always gets effective `Admin`.
/// - A `deny` member matching the account (across all attached ACLs) is a hard
///   stop.
/// - Otherwise the most-permissive matching grant wins.
pub async fn effective_permission(
    pool: &PgPool,
    account_id: Uuid,
    map_id: Uuid,
) -> Result<Option<Permission>> {
    // Owner bypass — only for an active map (a soft-deleted map grants nothing).
    let is_owner: bool = sqlx::query_scalar!(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM map
            WHERE id = $1 AND owner_account_id = $2 AND status = 'active'
        )
        "#,
        map_id,
        account_id,
    )
    .fetch_one(pool)
    .await
    .context("failed to check map ownership")?
    .unwrap_or(false);

    if is_owner {
        return Ok(Some(Permission::Admin));
    }

    // Collect every permission matching this account across all ACLs on the map,
    // matching on direct character / corporation / alliance membership.
    let rows = sqlx::query!(
        r#"
        SELECT am.permission
        FROM map_acl ma
        JOIN acl_member am ON am.acl_id = ma.acl_id
        JOIN eve_character ec ON ec.account_id = $2
        WHERE ma.map_id = $1
          AND (
              (am.member_type = 'character'   AND am.character_id  = ec.id)
          OR  (am.member_type = 'corporation' AND am.eve_entity_id = ec.corporation_id)
          OR  (am.member_type = 'alliance'    AND am.eve_entity_id = ec.alliance_id
                                              AND ec.alliance_id IS NOT NULL)
          )
        "#,
        map_id,
        account_id,
    )
    .fetch_all(pool)
    .await
    .context("failed to resolve acl permissions")?;

    if rows.is_empty() {
        return Ok(None);
    }

    // Deny is a hard stop — overrides all grants.
    if rows.iter().any(|r| r.permission == "deny") {
        return Ok(None);
    }

    // Most-permissive grant wins.
    let best = rows
        .iter()
        .filter_map(|r| Permission::parse(&r.permission))
        .max();

    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::map::insert_map;
    use crate::db::test_helpers::{insert_character, insert_character_full};
    use crate::db::{accounts, acl, acl_member, map, map_acl};

    async fn active_map(pool: &PgPool, owner: Uuid, slug: &str) -> Uuid {
        let mut tx = pool.begin().await.unwrap();
        let m = insert_map(&mut tx, owner, "M", slug, None).await.unwrap();
        tx.commit().await.unwrap();
        m.id
    }

    // ---- pure-enum behaviour ----

    #[test]
    fn permission_ordering() {
        assert!(Permission::Admin > Permission::Manage);
        assert!(Permission::Manage > Permission::ReadWrite);
        assert!(Permission::ReadWrite > Permission::Read);
    }

    #[test]
    fn permission_round_trip() {
        for (s, p) in [
            ("read", Permission::Read),
            ("read_write", Permission::ReadWrite),
            ("manage", Permission::Manage),
            ("admin", Permission::Admin),
        ] {
            assert_eq!(Permission::parse(s), Some(p));
            assert_eq!(p.as_str(), s);
        }
    }

    #[test]
    fn deny_and_unknown_parse_to_none() {
        assert_eq!(Permission::parse("deny"), None);
        assert_eq!(Permission::parse("bogus"), None);
    }

    // ---- resolver behaviour ----

    #[sqlx::test]
    async fn owner_of_active_map_gets_admin(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let map_id = active_map(&pool, owner, "own").await;

        let p = effective_permission(&pool, owner, map_id).await.unwrap();
        assert_eq!(p, Some(Permission::Admin));
    }

    #[sqlx::test]
    async fn owner_of_soft_deleted_map_gets_no_bypass(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let map_id = active_map(&pool, owner, "del").await;
        let mut tx = pool.begin().await.unwrap();
        map::soft_delete_map(&mut tx, map_id).await.unwrap();
        tx.commit().await.unwrap();

        let p = effective_permission(&pool, owner, map_id).await.unwrap();
        assert_eq!(p, None);
    }

    #[sqlx::test]
    async fn character_grant_resolves(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let member = accounts::create_account(&pool).await.unwrap();
        let char_id = insert_character(&pool, member, 1, "C").await;
        let map_id = active_map(&pool, owner, "char").await;
        let a = acl::insert_acl_for_test(&pool, owner, "A").await;
        acl_member::add_member(
            &pool,
            a.id,
            "character",
            None,
            Some(char_id),
            "C",
            "read_write",
        )
        .await
        .unwrap();
        map_acl::attach_acl_pool(&pool, map_id, a.id).await.unwrap();

        let p = effective_permission(&pool, member, map_id).await.unwrap();
        assert_eq!(p, Some(Permission::ReadWrite));
    }

    #[sqlx::test]
    async fn corporation_grant_resolves(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let member = accounts::create_account(&pool).await.unwrap();
        insert_character_full(&pool, member, 2, "C", 9000, None).await;
        let map_id = active_map(&pool, owner, "corp").await;
        let a = acl::insert_acl_for_test(&pool, owner, "A").await;
        acl_member::add_member(&pool, a.id, "corporation", Some(9000), None, "Corp", "read")
            .await
            .unwrap();
        map_acl::attach_acl_pool(&pool, map_id, a.id).await.unwrap();

        let p = effective_permission(&pool, member, map_id).await.unwrap();
        assert_eq!(p, Some(Permission::Read));
    }

    #[sqlx::test]
    async fn alliance_grant_resolves(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let member = accounts::create_account(&pool).await.unwrap();
        insert_character_full(&pool, member, 3, "C", 9000, Some(7000)).await;
        let map_id = active_map(&pool, owner, "alli").await;
        let a = acl::insert_acl_for_test(&pool, owner, "A").await;
        // Alliance members are non-character, so manage/admin are forbidden by
        // the role-for-type CHECK; read_write is the highest they may hold.
        acl_member::add_member(
            &pool,
            a.id,
            "alliance",
            Some(7000),
            None,
            "Alliance",
            "read_write",
        )
        .await
        .unwrap();
        map_acl::attach_acl_pool(&pool, map_id, a.id).await.unwrap();

        let p = effective_permission(&pool, member, map_id).await.unwrap();
        assert_eq!(p, Some(Permission::ReadWrite));
    }

    #[sqlx::test]
    async fn most_permissive_grant_wins(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let member = accounts::create_account(&pool).await.unwrap();
        let char_id = insert_character_full(&pool, member, 4, "C", 9000, None).await;
        let map_id = active_map(&pool, owner, "multi").await;
        let a = acl::insert_acl_for_test(&pool, owner, "A").await;
        acl_member::add_member(&pool, a.id, "corporation", Some(9000), None, "Corp", "read")
            .await
            .unwrap();
        acl_member::add_member(&pool, a.id, "character", None, Some(char_id), "C", "manage")
            .await
            .unwrap();
        map_acl::attach_acl_pool(&pool, map_id, a.id).await.unwrap();

        let p = effective_permission(&pool, member, map_id).await.unwrap();
        assert_eq!(p, Some(Permission::Manage));
    }

    #[sqlx::test]
    async fn deny_overrides_all_grants(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let member = accounts::create_account(&pool).await.unwrap();
        let char_id = insert_character_full(&pool, member, 5, "C", 9000, None).await;
        let map_id = active_map(&pool, owner, "deny").await;
        let a = acl::insert_acl_for_test(&pool, owner, "A").await;
        // A high character grant plus a corp-level deny: deny wins.
        acl_member::add_member(&pool, a.id, "character", None, Some(char_id), "C", "admin")
            .await
            .unwrap();
        acl_member::add_member(&pool, a.id, "corporation", Some(9000), None, "Corp", "deny")
            .await
            .unwrap();
        map_acl::attach_acl_pool(&pool, map_id, a.id).await.unwrap();

        let p = effective_permission(&pool, member, map_id).await.unwrap();
        assert_eq!(p, None);
    }

    #[sqlx::test]
    async fn no_match_means_none(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let stranger = accounts::create_account(&pool).await.unwrap();
        insert_character(&pool, stranger, 6, "S").await;
        let map_id = active_map(&pool, owner, "none").await;

        let p = effective_permission(&pool, stranger, map_id).await.unwrap();
        assert_eq!(p, None);
    }
}
