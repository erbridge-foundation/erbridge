use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use strum::{Display, EnumString, IntoStaticStr};
use uuid::Uuid;

use crate::db::DbError;

/// The kind of entity an ACL member grants a permission to. The DB/wire form is
/// the snake_case string (`character`/`corporation`/`alliance`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum MemberType {
    Character,
    Corporation,
    Alliance,
}

impl MemberType {
    pub fn as_str(self) -> &'static str {
        self.into()
    }
}

/// A permission an ACL member may be granted. `Deny` is a veto, not an ordinary
/// level — the resolver treats it specially.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum AclPermission {
    Read,
    ReadWrite,
    Manage,
    Admin,
    Deny,
}

impl AclPermission {
    pub fn as_str(self) -> &'static str {
        self.into()
    }
}

/// A single grant within an ACL.
#[derive(Debug, Clone)]
pub struct AclMember {
    pub id: Uuid,
    pub acl_id: Uuid,
    pub member_type: String,
    pub eve_entity_id: Option<i64>,
    pub character_id: Option<Uuid>,
    pub name: String,
    pub permission: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Inserts a member into an ACL. The caller (service layer) is responsible for
/// validating the `member_type`/`permission`/id-column coherence; the database
/// CHECK and unique constraints are the backstop and surface as `DbError`.
///
/// Generic over the executor so the service can run the insert inside its
/// audit transaction (passing `&mut *tx`) while tests can seed against the pool
/// directly — one function, no pool/tx twin.
#[allow(clippy::too_many_arguments)]
pub async fn add_member<'e, E>(
    executor: E,
    acl_id: Uuid,
    member_type: &str,
    eve_entity_id: Option<i64>,
    character_id: Option<Uuid>,
    name: &str,
    permission: &str,
) -> Result<AclMember, DbError>
where
    E: sqlx::PgExecutor<'e>,
{
    let m = sqlx::query_as!(
        AclMember,
        r#"
        INSERT INTO acl_member (acl_id, member_type, eve_entity_id, character_id, name, permission)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, acl_id, member_type, eve_entity_id, character_id, name, permission,
                  created_at, updated_at
        "#,
        acl_id,
        member_type,
        eve_entity_id,
        character_id,
        name,
        permission,
    )
    .fetch_one(executor)
    .await?;

    Ok(m)
}

/// Lists the members of an ACL, oldest first.
pub async fn list_members(pool: &PgPool, acl_id: Uuid) -> Result<Vec<AclMember>> {
    sqlx::query_as!(
        AclMember,
        r#"
        SELECT id, acl_id, member_type, eve_entity_id, character_id, name, permission,
               created_at, updated_at
        FROM acl_member
        WHERE acl_id = $1
        ORDER BY created_at ASC
        "#,
        acl_id,
    )
    .fetch_all(pool)
    .await
    .context("failed to list acl members")
}

/// Updates a member's permission within an ACL. Returns the updated row, or
/// `None` if no such member exists on that ACL. A CHECK violation (e.g. raising
/// a corporation member to `admin`) surfaces as `DbError`.
pub async fn update_member_permission<'e, E>(
    executor: E,
    acl_id: Uuid,
    member_id: Uuid,
    permission: &str,
) -> Result<Option<AclMember>, DbError>
where
    E: sqlx::PgExecutor<'e>,
{
    let m = sqlx::query_as!(
        AclMember,
        r#"
        UPDATE acl_member
        SET permission = $3, updated_at = now()
        WHERE id = $2 AND acl_id = $1
        RETURNING id, acl_id, member_type, eve_entity_id, character_id, name, permission,
                  created_at, updated_at
        "#,
        acl_id,
        member_id,
        permission,
    )
    .fetch_optional(executor)
    .await?;

    Ok(m)
}

/// Removes a member from an ACL. Returns the removed row (so the caller can
/// snapshot the member's name + EVE id into the audit event), or `None` if no
/// such member exists on that ACL.
pub async fn remove_member<'e, E>(
    executor: E,
    acl_id: Uuid,
    member_id: Uuid,
) -> Result<Option<AclMember>>
where
    E: sqlx::PgExecutor<'e>,
{
    let m = sqlx::query_as!(
        AclMember,
        r#"
        DELETE FROM acl_member
        WHERE id = $2 AND acl_id = $1
        RETURNING id, acl_id, member_type, eve_entity_id, character_id, name, permission,
                  created_at, updated_at
        "#,
        acl_id,
        member_id,
    )
    .fetch_optional(executor)
    .await
    .context("failed to remove acl member")?;

    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::acl::insert_acl_for_test;
    use crate::db::test_helpers::insert_character;
    use crate::db::{DbError, accounts};

    #[test]
    fn member_type_round_trips() {
        assert_eq!(MemberType::Character.as_str(), "character");
        assert_eq!(
            "corporation".parse::<MemberType>().unwrap(),
            MemberType::Corporation
        );
        assert_eq!(
            "alliance".parse::<MemberType>().unwrap(),
            MemberType::Alliance
        );
        assert!("fleet".parse::<MemberType>().is_err());
    }

    #[test]
    fn permission_round_trips() {
        assert_eq!(AclPermission::ReadWrite.as_str(), "read_write");
        assert_eq!(
            "deny".parse::<AclPermission>().unwrap(),
            AclPermission::Deny
        );
        assert!("bogus".parse::<AclPermission>().is_err());
    }

    #[sqlx::test]
    async fn add_and_list_member(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let a = insert_acl_for_test(&pool, owner, "ACL").await;
        let m = add_member(&pool, a.id, "corporation", Some(98), None, "Corp", "read")
            .await
            .unwrap();

        let members = list_members(&pool, a.id).await.unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].id, m.id);
        assert_eq!(members[0].member_type, "corporation");
        assert_eq!(members[0].eve_entity_id, Some(98));
        assert_eq!(members[0].permission, "read");
    }

    #[sqlx::test]
    async fn update_permission_changes_row(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let a = insert_acl_for_test(&pool, owner, "ACL").await;
        let m = add_member(&pool, a.id, "corporation", Some(98), None, "Corp", "read")
            .await
            .unwrap();

        let updated = update_member_permission(&pool, a.id, m.id, "read_write")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.permission, "read_write");
    }

    #[sqlx::test]
    async fn update_permission_wrong_acl_returns_none(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let a = insert_acl_for_test(&pool, owner, "ACL").await;
        let other = insert_acl_for_test(&pool, owner, "Other").await;
        let m = add_member(&pool, a.id, "corporation", Some(98), None, "Corp", "read")
            .await
            .unwrap();

        let result = update_member_permission(&pool, other.id, m.id, "deny")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[sqlx::test]
    async fn remove_member_deletes(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let a = insert_acl_for_test(&pool, owner, "ACL").await;
        let m = add_member(&pool, a.id, "corporation", Some(98), None, "Corp", "read")
            .await
            .unwrap();

        let removed = remove_member(&pool, a.id, m.id).await.unwrap().unwrap();
        assert_eq!(removed.id, m.id);
        assert_eq!(removed.name, "Corp");
        assert_eq!(removed.eve_entity_id, Some(98));
        assert!(list_members(&pool, a.id).await.unwrap().is_empty());
        // Removing again finds no row.
        assert!(remove_member(&pool, a.id, m.id).await.unwrap().is_none());
    }

    // ---- CHECK-constraint rejections (the database backstop) ----

    #[sqlx::test]
    async fn invalid_member_type_rejected(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let a = insert_acl_for_test(&pool, owner, "ACL").await;
        let err = add_member(&pool, a.id, "fleet", Some(1), None, "X", "read")
            .await
            .unwrap_err();
        assert!(matches!(err, DbError::CheckViolation { .. }));
    }

    #[sqlx::test]
    async fn invalid_permission_rejected(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let a = insert_acl_for_test(&pool, owner, "ACL").await;
        let err = add_member(&pool, a.id, "character", None, None, "X", "superuser")
            .await
            .unwrap_err();
        assert!(matches!(err, DbError::CheckViolation { .. }));
    }

    #[sqlx::test]
    async fn corporation_cannot_hold_manage(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let a = insert_acl_for_test(&pool, owner, "ACL").await;
        let err = add_member(&pool, a.id, "corporation", Some(5), None, "Corp", "manage")
            .await
            .unwrap_err();
        assert!(matches!(err, DbError::CheckViolation { .. }));
    }

    #[sqlx::test]
    async fn character_may_hold_admin(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let a = insert_acl_for_test(&pool, owner, "ACL").await;
        let char_id = insert_character(&pool, owner, 95465499, "Admin").await;
        // A character member carries BOTH its EVE id (eve_entity_id, the durable
        // ESI identity, symmetric with corp/alliance) and character_id (the
        // internal FK link for cascade-delete).
        let m = add_member(
            &pool,
            a.id,
            "character",
            Some(95465499),
            Some(char_id),
            "Admin",
            "admin",
        )
        .await
        .unwrap();
        assert_eq!(m.permission, "admin");
        assert_eq!(m.eve_entity_id, Some(95465499));
        assert_eq!(m.character_id, Some(char_id));
    }

    // ---- uniqueness (partial unique indexes) ----

    #[sqlx::test]
    async fn duplicate_character_member_violates_unique(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let a = insert_acl_for_test(&pool, owner, "ACL").await;
        let char_id = insert_character(&pool, owner, 12121, "Pilot").await;
        add_member(
            &pool,
            a.id,
            "character",
            Some(12121),
            Some(char_id),
            "Pilot",
            "read",
        )
        .await
        .unwrap();
        let err = add_member(
            &pool,
            a.id,
            "character",
            Some(12121),
            Some(char_id),
            "Pilot",
            "read",
        )
        .await
        .unwrap_err();
        assert!(matches!(err, DbError::UniqueViolation { .. }));
    }

    #[sqlx::test]
    async fn duplicate_corporation_member_violates_unique(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let a = insert_acl_for_test(&pool, owner, "ACL").await;
        add_member(&pool, a.id, "corporation", Some(500), None, "Corp", "read")
            .await
            .unwrap();
        let err = add_member(&pool, a.id, "corporation", Some(500), None, "Corp", "read")
            .await
            .unwrap_err();
        assert!(matches!(err, DbError::UniqueViolation { .. }));
    }

    #[sqlx::test]
    async fn duplicate_alliance_member_violates_unique(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let a = insert_acl_for_test(&pool, owner, "ACL").await;
        add_member(&pool, a.id, "alliance", Some(600), None, "Alliance", "read")
            .await
            .unwrap();
        let err = add_member(&pool, a.id, "alliance", Some(600), None, "Alliance", "read")
            .await
            .unwrap_err();
        assert!(matches!(err, DbError::UniqueViolation { .. }));
    }

    #[sqlx::test]
    async fn same_eve_id_corp_and_alliance_both_allowed(pool: PgPool) {
        // The entity index keys on member_type too, so the same eve_entity_id can
        // be both a corporation and an alliance member of one ACL.
        let owner = accounts::create_account(&pool).await.unwrap();
        let a = insert_acl_for_test(&pool, owner, "ACL").await;
        add_member(&pool, a.id, "corporation", Some(700), None, "X", "read")
            .await
            .unwrap();
        add_member(&pool, a.id, "alliance", Some(700), None, "X", "read")
            .await
            .unwrap();
        assert_eq!(list_members(&pool, a.id).await.unwrap().len(), 2);
    }

    #[sqlx::test]
    async fn migration_dedupe_keeps_oldest_and_index_recreates(pool: PgPool) {
        // Reproduce the pre-migration dirty state: drop the unique indexes,
        // insert duplicate members, then run the migration's dedupe + index
        // creation and assert one (the oldest) row of each set survives.
        let owner = accounts::create_account(&pool).await.unwrap();
        let a = insert_acl_for_test(&pool, owner, "Dirty").await;
        let char_id = insert_character(&pool, owner, 31415, "Pilot").await;

        sqlx::query("DROP INDEX acl_member_unique_character")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DROP INDEX acl_member_unique_entity")
            .execute(&pool)
            .await
            .unwrap();

        // Two duplicate corporation rows (older then newer) + two duplicate
        // character rows. The created_at ordering decides which survives.
        let older = add_member(&pool, a.id, "corporation", Some(900), None, "Old", "read")
            .await
            .unwrap();
        sqlx::query("UPDATE acl_member SET created_at = now() - interval '1 hour' WHERE id = $1")
            .bind(older.id)
            .execute(&pool)
            .await
            .unwrap();
        let _newer = add_member(&pool, a.id, "corporation", Some(900), None, "New", "read")
            .await
            .unwrap();
        add_member(
            &pool,
            a.id,
            "character",
            Some(31415),
            Some(char_id),
            "Pilot",
            "read",
        )
        .await
        .unwrap();
        add_member(
            &pool,
            a.id,
            "character",
            Some(31415),
            Some(char_id),
            "Pilot",
            "read",
        )
        .await
        .unwrap();

        // The dedupe statements from the migration.
        sqlx::query(
            r#"
            WITH ranked AS (
                SELECT id, row_number() OVER (
                    PARTITION BY acl_id, character_id
                    ORDER BY created_at ASC, id ASC) AS rn
                FROM acl_member WHERE member_type = 'character'
            )
            DELETE FROM acl_member USING ranked
            WHERE acl_member.id = ranked.id AND ranked.rn > 1
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            WITH ranked AS (
                SELECT id, row_number() OVER (
                    PARTITION BY acl_id, member_type, eve_entity_id
                    ORDER BY created_at ASC, id ASC) AS rn
                FROM acl_member WHERE member_type <> 'character'
            )
            DELETE FROM acl_member USING ranked
            WHERE acl_member.id = ranked.id AND ranked.rn > 1
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // One of each survives; the surviving corporation row is the oldest.
        let members = list_members(&pool, a.id).await.unwrap();
        assert_eq!(members.len(), 2, "one character + one corporation survive");
        let corp = members
            .iter()
            .find(|m| m.member_type == "corporation")
            .unwrap();
        assert_eq!(corp.id, older.id, "oldest corporation row is kept");

        // The indexes recreate cleanly now the data is clean.
        sqlx::query(
            "CREATE UNIQUE INDEX acl_member_unique_character
             ON acl_member (acl_id, character_id) WHERE member_type = 'character'",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE UNIQUE INDEX acl_member_unique_entity
             ON acl_member (acl_id, member_type, eve_entity_id) WHERE member_type <> 'character'",
        )
        .execute(&pool)
        .await
        .unwrap();
    }

    #[sqlx::test]
    async fn same_corp_in_different_acls_allowed(pool: PgPool) {
        // Uniqueness is scoped per-ACL: the same corporation may be a member of
        // two different ACLs.
        let owner = accounts::create_account(&pool).await.unwrap();
        let a = insert_acl_for_test(&pool, owner, "A").await;
        let b = insert_acl_for_test(&pool, owner, "B").await;
        add_member(&pool, a.id, "corporation", Some(800), None, "Corp", "read")
            .await
            .unwrap();
        add_member(&pool, b.id, "corporation", Some(800), None, "Corp", "read")
            .await
            .unwrap();
        assert_eq!(list_members(&pool, a.id).await.unwrap().len(), 1);
        assert_eq!(list_members(&pool, b.id).await.unwrap().len(), 1);
    }
}
