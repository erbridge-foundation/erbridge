use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    audit::{self, AuditEvent},
    db::{
        DbError, acl as acl_db, acl_member as member_db, characters as character_db,
        map::{self as db, Map, MapWithAcls},
        map_acl,
    },
    error::{AppError, ConflictKind},
    permissions::{Permission, effective_permission},
};

/// Lists the maps the account can read, each annotated with the attached ACLs
/// the account can manage.
pub async fn list_maps(pool: &PgPool, account_id: Uuid) -> Result<Vec<MapWithAcls>, AppError> {
    let maps = db::find_maps_for_account(pool, account_id).await?;
    if maps.is_empty() {
        return Ok(vec![]);
    }

    let manageable = acl_db::find_acls_manageable_by_account(pool, account_id).await?;
    let map_ids: Vec<Uuid> = maps.iter().map(|m| m.id).collect();
    let attached = map_acl::find_acl_ids_for_maps(pool, &map_ids).await?;

    Ok(maps
        .into_iter()
        .map(|m| {
            let acls = manageable_summaries(&manageable, attached.get(&m.id));
            into_map_with_acls(m, acls)
        })
        .collect())
}

/// Annotates a single map with the ACLs attached to it that `account_id` can
/// manage — the same summary shape [`list_maps`] produces per row.
async fn annotate_with_manageable_acls(
    pool: &PgPool,
    account_id: Uuid,
    map: Map,
) -> Result<MapWithAcls, AppError> {
    let manageable = acl_db::find_acls_manageable_by_account(pool, account_id).await?;
    let attached = map_acl::find_acl_ids_for_maps(pool, &[map.id]).await?;
    let acls = manageable_summaries(&manageable, attached.get(&map.id));
    Ok(into_map_with_acls(map, acls))
}

/// Picks the `(id, name)` summaries of the manageable ACLs that are attached to a
/// map (given the map's attached acl-id list, or `None` if it has none).
fn manageable_summaries(
    manageable: &[crate::db::acl::Acl],
    attached_ids: Option<&Vec<Uuid>>,
) -> Vec<(Uuid, String)> {
    manageable
        .iter()
        .filter(|a| attached_ids.is_some_and(|ids| ids.contains(&a.id)))
        .map(|a| (a.id, a.name.clone()))
        .collect()
}

/// Combines a `Map` with its computed ACL summaries into a `MapWithAcls`.
fn into_map_with_acls(m: Map, acls: Vec<(Uuid, String)>) -> MapWithAcls {
    MapWithAcls {
        id: m.id,
        name: m.name,
        slug: m.slug,
        owner_account_id: m.owner_account_id,
        description: m.description,
        status: m.status,
        delete_requested_at: m.delete_requested_at,
        created_at: m.created_at,
        updated_at: m.updated_at,
        acls,
    }
}

/// Returns a single map the account can read.
pub async fn get_map(pool: &PgPool, account_id: Uuid, map_id: Uuid) -> Result<Map, AppError> {
    let map = db::find_map_by_id(pool, map_id)
        .await?
        .filter(|m| m.status == "active")
        .ok_or(AppError::NotFound)?;
    require_map_permission(pool, map_id, account_id, Permission::Read).await?;
    Ok(map)
}

/// Returns a single active map resolved by slug, annotated with the attached
/// ACLs the caller can manage — the same shape as [`list_maps`]. An unknown or
/// soft-deleted slug, and a map the caller cannot read, all yield `NotFound`
/// (the read-permission failure is folded into 404 so existence is not leaked).
pub async fn get_map_by_slug(
    pool: &PgPool,
    account_id: Uuid,
    slug: &str,
) -> Result<MapWithAcls, AppError> {
    let map = db::find_active_map_by_slug(pool, slug)
        .await?
        .ok_or(AppError::NotFound)?;

    let effective = effective_permission(pool, account_id, map.id).await?;
    if !matches!(effective, Some(p) if p >= Permission::Read) {
        return Err(AppError::NotFound);
    }

    annotate_with_manageable_acls(pool, account_id, map).await
}

/// Creates a map owned by `account_id`. The map may be attached to an ACL at
/// creation time via exactly one of:
///
/// - `acl_id` — attach an existing ACL the caller owns; or
/// - `default_acl` — mint a fresh ACL named after the map, seed the caller's main
///   character as an `admin` member (when a main exists), and attach it.
///
/// Both supplied together is a 400. The whole operation runs in one transaction:
/// either the map (and any ACL/member/attachment) all exist, or none do.
pub async fn create_map(
    pool: &PgPool,
    account_id: Uuid,
    name: &str,
    slug: &str,
    description: Option<&str>,
    acl_id: Option<Uuid>,
    default_acl: bool,
) -> Result<Map, AppError> {
    if acl_id.is_some() && default_acl {
        return Err(AppError::BadRequest(
            "acl_id and default_acl are mutually exclusive".to_string(),
        ));
    }

    // Validate ACL ownership up front, before opening the write transaction.
    if let Some(acl_id) = acl_id {
        let acl = acl_db::find_acl_by_id(pool, acl_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if acl.owner_account_id != Some(account_id) {
            return Err(AppError::Forbidden);
        }
    }

    let mut tx = pool.begin().await?;

    // When requested, mint + seed the default ACL inside this transaction so a
    // later map-insert failure (e.g. slug conflict) rolls the ACL back too — no
    // orphan ACL can survive a failed map creation.
    let default_acl_id = if default_acl {
        let acl = acl_db::insert_acl(&mut tx, account_id, name).await?;
        audit::record_in_tx(
            &mut tx,
            Some(account_id),
            None,
            AuditEvent::AclCreated {
                account_id,
                acl_id: acl.id,
                name: name.to_string(),
            },
        )
        .await?;

        // Seed the caller's main as an explicit admin member, when one exists.
        if let Some((main_id, eve_character_id, char_name)) =
            character_db::get_main_for_account_tx(&mut tx, account_id).await?
        {
            // The ACL was just minted and is empty, so the member insert cannot
            // conflict and `character`+`admin` is a valid combination — any
            // DbError here is genuinely unexpected, hence Internal.
            member_db::add_member(
                &mut *tx,
                acl.id,
                "character",
                Some(eve_character_id),
                Some(main_id),
                &char_name,
                "admin",
            )
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!("failed to seed default-acl member: {e}"))
            })?;
            audit::record_in_tx(
                &mut tx,
                Some(account_id),
                None,
                AuditEvent::AclMemberAdded {
                    account_id,
                    acl_id: acl.id,
                    member_name: char_name,
                    eve_entity_id: Some(eve_character_id),
                    member_type: "character".to_string(),
                    permission: "admin".to_string(),
                },
            )
            .await?;
        }
        Some(acl.id)
    } else {
        None
    };

    let map = db::insert_map(&mut tx, account_id, name, slug, description)
        .await
        .map_err(map_slug_err)?;

    if let Some(acl_id) = acl_id.or(default_acl_id) {
        map_acl::attach_acl(&mut tx, map.id, acl_id).await?;
    }

    // The default-ACL path additionally records the attach event, naming both the
    // map and the ACL (its own target) for the audit trail.
    if let Some(acl_id) = default_acl_id {
        audit::record_in_tx(
            &mut tx,
            Some(account_id),
            None,
            AuditEvent::AclAttachedToMap {
                account_id,
                map_id: map.id,
                map_name: name.to_string(),
                acl_id,
            },
        )
        .await?;
    }

    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::MapCreated {
            account_id,
            map_id: map.id,
            name: name.to_string(),
        },
    )
    .await?;

    tx.commit().await?;

    Ok(map)
}

/// Updates a map's name/slug/description. Caller must hold `manage` or higher.
pub async fn update_map(
    pool: &PgPool,
    account_id: Uuid,
    map_id: Uuid,
    name: &str,
    slug: &str,
    description: Option<&str>,
) -> Result<Map, AppError> {
    require_map_permission(pool, map_id, account_id, Permission::Manage).await?;

    db::update_map(pool, map_id, name, slug, description)
        .await
        .map_err(map_slug_err)?
        .ok_or(AppError::NotFound)
}

/// Soft-deletes a map. Caller must hold `admin` (owner or admin-granted).
pub async fn delete_map(pool: &PgPool, account_id: Uuid, map_id: Uuid) -> Result<(), AppError> {
    require_map_permission(pool, map_id, account_id, Permission::Admin).await?;

    let map = db::find_map_by_id(pool, map_id)
        .await?
        .filter(|m| m.status == "active")
        .ok_or(AppError::NotFound)?;

    let mut tx = pool.begin().await?;

    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::MapDeleted {
            account_id,
            map_id,
            name: map.name,
        },
    )
    .await?;

    let deleted = db::soft_delete_map(&mut tx, map_id).await?;
    if !deleted {
        return Err(AppError::NotFound);
    }

    tx.commit().await?;

    Ok(())
}

/// Attaches an ACL to a map. Caller must hold `admin` on the map AND own the ACL.
pub async fn attach_acl_to_map(
    pool: &PgPool,
    account_id: Uuid,
    map_id: Uuid,
    acl_id: Uuid,
) -> Result<(), AppError> {
    require_map_permission(pool, map_id, account_id, Permission::Admin).await?;

    let acl = acl_db::find_acl_by_id(pool, acl_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if acl.owner_account_id != Some(account_id) {
        return Err(AppError::Forbidden);
    }

    // Snapshot the map name into the audit event so the row names both the ACL
    // (the target) and the map (the secondary entity) after either is deleted.
    let map = db::find_map_by_id(pool, map_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let mut tx = pool.begin().await?;

    map_acl::attach_acl(&mut tx, map_id, acl_id).await?;

    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::AclAttachedToMap {
            account_id,
            map_id,
            map_name: map.name,
            acl_id,
        },
    )
    .await?;

    tx.commit().await?;

    Ok(())
}

/// Detaches an ACL from a map. Caller must hold `admin` on the map.
pub async fn detach_acl_from_map(
    pool: &PgPool,
    account_id: Uuid,
    map_id: Uuid,
    acl_id: Uuid,
) -> Result<(), AppError> {
    require_map_permission(pool, map_id, account_id, Permission::Admin).await?;

    // Snapshot the map name into the audit event (the map is the secondary
    // entity; the ACL is the target).
    let map = db::find_map_by_id(pool, map_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let mut tx = pool.begin().await?;

    let removed = map_acl::detach_acl(&mut tx, map_id, acl_id).await?;
    if !removed {
        return Err(AppError::NotFound);
    }

    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::AclDetachedFromMap {
            account_id,
            map_id,
            map_name: map.name,
            acl_id,
        },
    )
    .await?;

    tx.commit().await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Resolves the caller's effective permission on the map and refuses if it is
/// below `required` (or none).
async fn require_map_permission(
    pool: &PgPool,
    map_id: Uuid,
    account_id: Uuid,
    required: Permission,
) -> Result<(), AppError> {
    let effective = effective_permission(pool, account_id, map_id).await?;

    match effective {
        Some(p) if p >= required => Ok(()),
        _ => Err(AppError::Forbidden),
    }
}

/// Maps an insert/update `DbError` to an `AppError`, translating a slug unique
/// violation into the typed conflict.
fn map_slug_err(e: DbError) -> AppError {
    match e {
        DbError::UniqueViolation { .. } => AppError::Conflict(ConflictKind::MapSlugAlreadyExists),
        DbError::CheckViolation { constraint } => {
            AppError::Internal(anyhow::anyhow!("unexpected check violation: {constraint}"))
        }
        DbError::Other(err) => AppError::Internal(err),
    }
}

/// Validates and trims a map name (1..=100 chars after trim).
pub fn validate_map_name(name: &str) -> Result<&str, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 100 {
        return Err(AppError::BadRequest(
            "name must be 1..=100 characters".to_string(),
        ));
    }
    Ok(trimmed)
}

/// Validates a map slug: 1..=100 chars, lowercase alphanumerics in
/// hyphen-separated groups (`^[a-z0-9]+(-[a-z0-9]+)*$`), no leading/trailing or
/// doubled hyphens. Returns the slug unchanged on success.
pub fn validate_slug(slug: &str) -> Result<&str, AppError> {
    let invalid = || AppError::BadRequest("invalid slug".to_string());

    if slug.is_empty() || slug.len() > 100 {
        return Err(invalid());
    }
    if slug.starts_with('-') || slug.ends_with('-') || slug.contains("--") {
        return Err(invalid());
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(invalid());
    }
    Ok(slug)
}

/// Validates an optional description (max 500 chars). Returns the trimmed value.
pub fn validate_description(description: Option<&str>) -> Result<Option<&str>, AppError> {
    match description {
        Some(d) if d.chars().count() > 500 => Err(AppError::BadRequest(
            "description must be at most 500 characters".to_string(),
        )),
        other => Ok(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_name_rejects_empty_and_overlong() {
        assert!(validate_map_name("  ").is_err());
        assert!(validate_map_name(&"x".repeat(101)).is_err());
        assert_eq!(validate_map_name("  Home  ").unwrap(), "Home");
    }

    #[test]
    fn slug_accepts_valid() {
        for s in ["home", "wh-chain-1", "a", "abc123"] {
            assert_eq!(validate_slug(s).unwrap(), s);
        }
    }

    #[test]
    fn slug_rejects_invalid() {
        for s in ["", "-home", "home-", "wh--chain", "Home", "wh chain", "hé"] {
            assert!(validate_slug(s).is_err(), "expected {s:?} to be rejected");
        }
    }

    #[test]
    fn description_enforces_max_length() {
        assert!(validate_description(Some(&"x".repeat(501))).is_err());
        assert_eq!(validate_description(Some("ok")).unwrap(), Some("ok"));
        assert_eq!(validate_description(None).unwrap(), None);
    }

    // ---- default-ACL creation (real DB) ----

    use crate::db::accounts;
    use sqlx::PgPool;

    async fn count(pool: &PgPool, table: &str) -> i64 {
        // table is a hard-coded literal in each callsite, never user input.
        let q = format!("SELECT COUNT(*) AS n FROM {table}");
        sqlx::query_scalar::<_, i64>(&q)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn event_count(pool: &PgPool, event_type: &str) -> i64 {
        sqlx::query_scalar!(
            "SELECT COUNT(*) AS \"n!\" FROM audit_log WHERE event_type = $1",
            event_type
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[sqlx::test]
    async fn create_map_rejects_acl_id_and_default_acl_together(pool: PgPool) {
        let account = accounts::create_account(&pool).await.unwrap();
        let acl = acl_db::insert_acl_for_test(&pool, account, "X").await;

        let err = create_map(&pool, account, "Both", "both", None, Some(acl.id), true)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
        // Nothing minted beyond the pre-existing ACL; no map.
        assert_eq!(count(&pool, "map").await, 0);
    }

    #[sqlx::test]
    async fn create_map_default_acl_emits_all_events_in_one_tx(pool: PgPool) {
        let account = accounts::create_account(&pool).await.unwrap();
        sqlx::query!(
            r#"INSERT INTO eve_character (account_id, eve_character_id, name, corporation_id, corporation_name, is_main)
               VALUES ($1, 4242, 'Main', 8000, 'Corp', TRUE)"#,
            account,
        )
        .execute(&pool)
        .await
        .unwrap();

        create_map(&pool, account, "Home", "home", None, None, true)
            .await
            .unwrap();

        assert_eq!(count(&pool, "acl").await, 1);
        assert_eq!(count(&pool, "acl_member").await, 1);
        assert_eq!(count(&pool, "map_acl").await, 1);
        assert_eq!(event_count(&pool, "acl_created").await, 1);
        assert_eq!(event_count(&pool, "acl_member_added").await, 1);
        assert_eq!(event_count(&pool, "acl_attached_to_map").await, 1);
        assert_eq!(event_count(&pool, "map_created").await, 1);
    }

    #[sqlx::test]
    async fn create_map_default_acl_without_main_emits_no_member_event(pool: PgPool) {
        let account = accounts::create_account(&pool).await.unwrap();

        create_map(&pool, account, "Solo", "solo", None, None, true)
            .await
            .unwrap();

        assert_eq!(count(&pool, "acl").await, 1);
        assert_eq!(count(&pool, "acl_member").await, 0);
        assert_eq!(event_count(&pool, "acl_member_added").await, 0);
        assert_eq!(event_count(&pool, "acl_attached_to_map").await, 1);
    }

    #[sqlx::test]
    async fn create_map_default_acl_rolls_back_on_slug_conflict(pool: PgPool) {
        let account = accounts::create_account(&pool).await.unwrap();
        create_map(&pool, account, "First", "taken", None, None, false)
            .await
            .unwrap();

        let err = create_map(&pool, account, "Second", "taken", None, None, true)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            AppError::Conflict(ConflictKind::MapSlugAlreadyExists)
        ));
        // No orphan ACL survives the failed map creation.
        assert_eq!(count(&pool, "acl").await, 0);
        assert_eq!(event_count(&pool, "acl_created").await, 0);
    }
}
