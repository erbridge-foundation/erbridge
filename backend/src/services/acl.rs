use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    audit::{self, AuditEvent},
    db::{
        DbError,
        acl::{self as db, Acl},
        acl_member::{self as member_db, AclMember, AclPermission, MemberType},
    },
    error::AppError,
};

/// Input for adding a member to an ACL. The service validates that the
/// identifier columns match the member type before touching the db.
pub struct AddMemberInput {
    pub member_type: MemberType,
    pub eve_entity_id: Option<i64>,
    pub character_id: Option<Uuid>,
    pub name: String,
    pub permission: AclPermission,
}

/// Lists the ACLs the account can manage (owner or character manager).
pub async fn list_manageable_for_account(
    pool: &PgPool,
    account_id: Uuid,
) -> Result<Vec<Acl>, AppError> {
    db::find_acls_manageable_by_account(pool, account_id)
        .await
        .map_err(AppError::Internal)
}

/// Creates an ACL owned by `account_id` and records an audit event.
pub async fn create_acl(pool: &PgPool, account_id: Uuid, name: &str) -> Result<Acl, AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let acl = db::insert_acl(&mut tx, account_id, name)
        .await
        .map_err(AppError::Internal)?;

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
    .await
    .map_err(AppError::Internal)?;

    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(acl)
}

/// Renames an ACL the account owns. Returns `Forbidden` if the caller is not the
/// owner, `NotFound` if the ACL does not exist.
pub async fn rename_acl(
    pool: &PgPool,
    account_id: Uuid,
    acl_id: Uuid,
    new_name: &str,
) -> Result<Acl, AppError> {
    let acl = load_owned_acl(pool, account_id, acl_id).await?;
    let old_name = acl.name;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let updated = db::update_acl_name(&mut tx, acl_id, new_name)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::AclRenamed {
            account_id,
            acl_id,
            old_name,
            new_name: new_name.to_string(),
        },
    )
    .await
    .map_err(AppError::Internal)?;

    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(updated)
}

/// Deletes an ACL the account owns (cascading members and attachments).
pub async fn delete_acl(pool: &PgPool, account_id: Uuid, acl_id: Uuid) -> Result<(), AppError> {
    let acl = load_owned_acl(pool, account_id, acl_id).await?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    // Audit before the delete so the name snapshot is still resolvable.
    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::AclDeleted {
            account_id,
            acl_id,
            name: acl.name,
        },
    )
    .await
    .map_err(AppError::Internal)?;

    let deleted = db::delete_acl(&mut tx, acl_id)
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

/// Lists an ACL's members. Caller must own the ACL.
pub async fn list_members(
    pool: &PgPool,
    account_id: Uuid,
    acl_id: Uuid,
) -> Result<Vec<AclMember>, AppError> {
    load_owned_acl(pool, account_id, acl_id).await?;
    member_db::list_members(pool, acl_id)
        .await
        .map_err(AppError::Internal)
}

/// Adds a member to an ACL the account owns. Validates that the identifier
/// columns match the member type before inserting.
pub async fn add_member(
    pool: &PgPool,
    account_id: Uuid,
    acl_id: Uuid,
    input: AddMemberInput,
) -> Result<AclMember, AppError> {
    load_owned_acl(pool, account_id, acl_id).await?;
    validate_member_shape(&input)?;

    let member = member_db::add_member(
        pool,
        acl_id,
        input.member_type.as_str(),
        input.eve_entity_id,
        input.character_id,
        &input.name,
        input.permission.as_str(),
    )
    .await
    .map_err(map_member_db_err)?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::AclMemberAdded {
            account_id,
            acl_id,
            member_id: member.id,
            member_type: input.member_type.as_str().to_string(),
            permission: input.permission.as_str().to_string(),
        },
    )
    .await
    .map_err(AppError::Internal)?;
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(member)
}

/// Updates a member's permission on an ACL the account owns.
pub async fn update_member_permission(
    pool: &PgPool,
    account_id: Uuid,
    acl_id: Uuid,
    member_id: Uuid,
    permission: AclPermission,
) -> Result<AclMember, AppError> {
    load_owned_acl(pool, account_id, acl_id).await?;

    let updated = member_db::update_member_permission(pool, acl_id, member_id, permission.as_str())
        .await
        .map_err(map_member_db_err)?
        .ok_or(AppError::NotFound)?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::AclMemberPermissionChanged {
            account_id,
            acl_id,
            member_id,
            permission: permission.as_str().to_string(),
        },
    )
    .await
    .map_err(AppError::Internal)?;
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(updated)
}

/// Removes a member from an ACL the account owns.
pub async fn remove_member(
    pool: &PgPool,
    account_id: Uuid,
    acl_id: Uuid,
    member_id: Uuid,
) -> Result<(), AppError> {
    load_owned_acl(pool, account_id, acl_id).await?;

    let removed = member_db::remove_member(pool, acl_id, member_id)
        .await
        .map_err(AppError::Internal)?;
    if !removed {
        return Err(AppError::NotFound);
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::AclMemberRemoved {
            account_id,
            acl_id,
            member_id,
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

/// Loads an ACL and asserts the account owns it. `NotFound` if absent,
/// `Forbidden` if owned by someone else.
async fn load_owned_acl(pool: &PgPool, account_id: Uuid, acl_id: Uuid) -> Result<Acl, AppError> {
    let acl = db::find_acl_by_id(pool, acl_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;
    if acl.owner_account_id != Some(account_id) {
        return Err(AppError::Forbidden);
    }
    Ok(acl)
}

/// Validates that the identifier columns match the member type: a `character`
/// member must carry `character_id` (and not `eve_entity_id`); a `corporation`
/// or `alliance` member must carry `eve_entity_id` (and not `character_id`).
pub fn validate_member_shape(input: &AddMemberInput) -> Result<(), AppError> {
    match input.member_type {
        MemberType::Character => {
            if input.character_id.is_none() {
                return Err(AppError::BadRequest(
                    "character members require character_id".to_string(),
                ));
            }
            if input.eve_entity_id.is_some() {
                return Err(AppError::BadRequest(
                    "character members must not carry eve_entity_id".to_string(),
                ));
            }
        }
        MemberType::Corporation | MemberType::Alliance => {
            if input.eve_entity_id.is_none() {
                return Err(AppError::BadRequest(
                    "corporation/alliance members require eve_entity_id".to_string(),
                ));
            }
            if input.character_id.is_some() {
                return Err(AppError::BadRequest(
                    "corporation/alliance members must not carry character_id".to_string(),
                ));
            }
        }
    }
    Ok(())
}

/// Validates and trims an ACL name (1..=100 chars after trim).
pub fn validate_acl_name(name: &str) -> Result<&str, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 100 {
        return Err(AppError::BadRequest(
            "name must be 1..=100 characters".to_string(),
        ));
    }
    Ok(trimmed)
}

/// Parses a `member_type` string into the enum, erroring on an unknown value.
pub fn parse_member_type(s: &str) -> Result<MemberType, AppError> {
    s.parse()
        .map_err(|_| AppError::BadRequest(format!("invalid member_type: {s}")))
}

/// Parses a `permission` string into the enum, erroring on an unknown value.
pub fn parse_permission(s: &str) -> Result<AclPermission, AppError> {
    s.parse()
        .map_err(|_| AppError::BadRequest(format!("invalid permission: {s}")))
}

/// Maps a member-insert `DbError` to an `AppError`. A CHECK violation (e.g.
/// raising a corporation member to `admin`) is a client error, not a 500.
fn map_member_db_err(e: DbError) -> AppError {
    match e {
        DbError::UniqueViolation { .. } => AppError::BadRequest("duplicate acl member".to_string()),
        DbError::Other(err) => {
            // A CHECK-constraint violation surfaces here as Other; treat the
            // common "role for type" / invalid-value cases as bad requests.
            let msg = err.to_string();
            if msg.contains("acl_member_") && msg.contains("check") {
                AppError::BadRequest("invalid acl member type/permission combination".to_string())
            } else {
                AppError::Internal(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(
        member_type: MemberType,
        eve_entity_id: Option<i64>,
        character_id: Option<Uuid>,
    ) -> AddMemberInput {
        AddMemberInput {
            member_type,
            eve_entity_id,
            character_id,
            name: "X".to_string(),
            permission: AclPermission::Read,
        }
    }

    #[test]
    fn character_member_requires_character_id() {
        let err = validate_member_shape(&input(MemberType::Character, None, None)).unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn character_member_rejects_eve_entity_id() {
        let err =
            validate_member_shape(&input(MemberType::Character, Some(5), Some(Uuid::new_v4())))
                .unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn corporation_member_requires_eve_entity_id() {
        let err = validate_member_shape(&input(MemberType::Corporation, None, None)).unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn corporation_member_rejects_character_id() {
        let err = validate_member_shape(&input(
            MemberType::Corporation,
            Some(5),
            Some(Uuid::new_v4()),
        ))
        .unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn valid_character_member_passes() {
        validate_member_shape(&input(MemberType::Character, None, Some(Uuid::new_v4()))).unwrap();
    }

    #[test]
    fn valid_corporation_member_passes() {
        validate_member_shape(&input(MemberType::Corporation, Some(5), None)).unwrap();
    }

    #[test]
    fn acl_name_rejects_empty_and_overlong() {
        assert!(validate_acl_name("   ").is_err());
        assert!(validate_acl_name(&"x".repeat(101)).is_err());
        assert_eq!(validate_acl_name("  Corp  ").unwrap(), "Corp");
    }

    #[test]
    fn parse_member_type_round_trips_and_rejects() {
        assert_eq!(parse_member_type("alliance").unwrap(), MemberType::Alliance);
        assert!(matches!(
            parse_member_type("fleet"),
            Err(AppError::BadRequest(_))
        ));
    }

    #[test]
    fn parse_permission_round_trips_and_rejects() {
        assert_eq!(parse_permission("deny").unwrap(), AclPermission::Deny);
        assert!(matches!(
            parse_permission("root"),
            Err(AppError::BadRequest(_))
        ));
    }
}
