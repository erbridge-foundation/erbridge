use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    audit::{self, AuditEvent},
    db::{
        DbError, acl as acl_db,
        map::{self as db, Map, MapWithAcls},
        map_acl,
    },
    error::{AppError, ConflictKind},
    permissions::{Permission, effective_permission},
};

/// Lists the maps the account can read, each annotated with the attached ACLs
/// the account can manage.
pub async fn list_maps(pool: &PgPool, account_id: Uuid) -> Result<Vec<MapWithAcls>, AppError> {
    let maps = db::find_maps_for_account(pool, account_id)
        .await
        .map_err(AppError::Internal)?;
    if maps.is_empty() {
        return Ok(vec![]);
    }

    let manageable = acl_db::find_acls_manageable_by_account(pool, account_id)
        .await
        .map_err(AppError::Internal)?;
    let map_ids: Vec<Uuid> = maps.iter().map(|m| m.id).collect();
    let attached = map_acl::find_acl_ids_for_maps(pool, &map_ids)
        .await
        .map_err(AppError::Internal)?;

    Ok(maps
        .into_iter()
        .map(|m| {
            let acls = manageable
                .iter()
                .filter(|a| attached.get(&m.id).is_some_and(|ids| ids.contains(&a.id)))
                .map(|a| (a.id, a.name.clone()))
                .collect();
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
        })
        .collect())
}

/// Returns a single map the account can read.
pub async fn get_map(pool: &PgPool, account_id: Uuid, map_id: Uuid) -> Result<Map, AppError> {
    let map = db::find_map_by_id(pool, map_id)
        .await
        .map_err(AppError::Internal)?
        .filter(|m| m.status == "active")
        .ok_or(AppError::NotFound)?;
    require_map_permission(pool, map_id, account_id, Permission::Read).await?;
    Ok(map)
}

/// Creates a map owned by `account_id`. If `acl_id` is supplied the map is
/// attached to that ACL in the same transaction; the caller must own the ACL.
pub async fn create_map(
    pool: &PgPool,
    account_id: Uuid,
    name: &str,
    slug: &str,
    description: Option<&str>,
    acl_id: Option<Uuid>,
) -> Result<Map, AppError> {
    // Validate ACL ownership up front, before opening the write transaction.
    if let Some(acl_id) = acl_id {
        let acl = acl_db::find_acl_by_id(pool, acl_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or(AppError::NotFound)?;
        if acl.owner_account_id != Some(account_id) {
            return Err(AppError::Forbidden);
        }
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let map = db::insert_map(&mut tx, account_id, name, slug, description)
        .await
        .map_err(map_slug_err)?;

    if let Some(acl_id) = acl_id {
        map_acl::attach_acl(&mut tx, map.id, acl_id)
            .await
            .map_err(AppError::Internal)?;
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
    .await
    .map_err(AppError::Internal)?;

    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

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
        .await
        .map_err(AppError::Internal)?
        .filter(|m| m.status == "active")
        .ok_or(AppError::NotFound)?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

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
    .await
    .map_err(AppError::Internal)?;

    let deleted = db::soft_delete_map(&mut tx, map_id)
        .await
        .map_err(AppError::Internal)?;
    if !deleted {
        return Err(AppError::NotFound);
    }

    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

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
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;
    if acl.owner_account_id != Some(account_id) {
        return Err(AppError::Forbidden);
    }

    // Snapshot the map name into the audit event so the row names both the ACL
    // (the target) and the map (the secondary entity) after either is deleted.
    let map = db::find_map_by_id(pool, map_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    map_acl::attach_acl(&mut tx, map_id, acl_id)
        .await
        .map_err(AppError::Internal)?;

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
    .await
    .map_err(AppError::Internal)?;

    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

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
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let removed = map_acl::detach_acl(&mut tx, map_id, acl_id)
        .await
        .map_err(AppError::Internal)?;
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
    .await
    .map_err(AppError::Internal)?;

    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

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
    let effective = effective_permission(pool, account_id, map_id)
        .await
        .map_err(AppError::Internal)?;

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
}
