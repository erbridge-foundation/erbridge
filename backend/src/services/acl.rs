use reqwest_middleware::ClientWithMiddleware;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    audit::{self, AuditEvent},
    db::{
        DbError,
        acl::{self as db, Acl},
        acl_member::{self as member_db, AclMember, AclPermission, MemberType},
        characters,
    },
    error::{AppError, ConflictKind},
    esi::public_info,
};

/// Input for adding a member to an ACL. The service validates that the
/// identifier columns match the member type before touching the db.
pub struct AddMemberInput {
    pub member_type: MemberType,
    /// The member's durable EVE id — the EVE character/corporation/alliance id,
    /// uniform across all member types. Snapshotted into the audit event, and the
    /// mint key when a character member arrives without `character_id`.
    pub eve_entity_id: Option<i64>,
    /// Internal FK link to an existing `eve_character` row for character members
    /// (cascade-delete). Optional: when absent for a character member, the add
    /// mints the orphan keyed by `eve_entity_id`. `None` for corp/alliance.
    pub character_id: Option<Uuid>,
    pub name: String,
    pub permission: AclPermission,
}

/// What `add_member` needs to fetch a character's public-info affiliation snapshot
/// when it must mint an orphan (character member with no `character_id`). The ESI
/// fetch runs *before* the write transaction opens, so no outbound call is held
/// under the member-insert lock.
pub struct MintContext<'a> {
    pub http: &'a ClientWithMiddleware,
}

/// Lists the ACLs the account can manage (owner or character manager).
pub async fn list_manageable_for_account(
    pool: &PgPool,
    account_id: Uuid,
) -> Result<Vec<Acl>, AppError> {
    Ok(db::find_acls_manageable_by_account(pool, account_id).await?)
}

/// Returns a single ACL the account can manage. `NotFound` when the ACL does not
/// exist *or* the account cannot manage it — existence is not revealed, matching
/// the manageable-list visibility.
pub async fn get_manageable(
    pool: &PgPool,
    account_id: Uuid,
    acl_id: Uuid,
) -> Result<Acl, AppError> {
    db::find_manageable_acl_by_id(pool, account_id, acl_id)
        .await?
        .ok_or(AppError::NotFound)
}

/// Creates an ACL owned by `account_id` and records an audit event.
pub async fn create_acl(pool: &PgPool, account_id: Uuid, name: &str) -> Result<Acl, AppError> {
    let mut tx = pool.begin().await?;

    let acl = db::insert_acl(&mut tx, account_id, name).await?;

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

    tx.commit().await?;

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
    let mut tx = pool.begin().await?;

    // Ownership check, write, and audit all in one transaction.
    let acl = load_owned_acl_in_tx(&mut tx, account_id, acl_id).await?;
    let old_name = acl.name;

    let updated = db::update_acl_name(&mut tx, acl_id, new_name)
        .await?
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
    .await?;

    tx.commit().await?;

    Ok(updated)
}

/// Deletes an ACL the account owns (cascading members and attachments).
pub async fn delete_acl(pool: &PgPool, account_id: Uuid, acl_id: Uuid) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    let acl = load_owned_acl_in_tx(&mut tx, account_id, acl_id).await?;

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
    .await?;

    let deleted = db::delete_acl(&mut tx, acl_id).await?;
    if !deleted {
        return Err(AppError::NotFound);
    }

    tx.commit().await?;

    Ok(())
}

/// Lists an ACL's members. Caller must own the ACL.
pub async fn list_members(
    pool: &PgPool,
    account_id: Uuid,
    acl_id: Uuid,
) -> Result<Vec<AclMember>, AppError> {
    load_owned_acl(pool, account_id, acl_id).await?;
    Ok(member_db::list_members(pool, acl_id).await?)
}

/// Adds a member to an ACL the account owns. Validates that the identifier
/// columns match the member type before inserting.
///
/// For a character member with no `character_id`, the orphan `eve_character` row
/// is find-or-minted from `eve_entity_id`: its public-info affiliation snapshot
/// is fetched (best-effort, via `mint`) *before* the transaction opens, then the
/// row is minted (or an existing row reused) inside the transaction and the
/// member is inserted referencing its UUID.
pub async fn add_member(
    pool: &PgPool,
    mint: &MintContext<'_>,
    account_id: Uuid,
    acl_id: Uuid,
    input: AddMemberInput,
) -> Result<AclMember, AppError> {
    validate_member_shape(&input)?;

    // A character member with no character_id needs an orphan minted from its
    // EVE id. Fetch the public-info snapshot now, outside the tx, so no ESI call
    // is held under the member-insert lock. `validate_member_shape` guarantees
    // eve_entity_id is present here.
    let pending_mint = match (input.member_type, input.character_id) {
        (MemberType::Character, None) => {
            let eve_character_id = input
                .eve_entity_id
                .ok_or_else(|| AppError::BadRequest("members require eve_entity_id".to_string()))?;
            Some((
                eve_character_id,
                fetch_orphan_affiliations(mint.http, eve_character_id).await,
            ))
        }
        _ => None,
    };

    let mut tx = pool.begin().await?;

    load_owned_acl_in_tx(&mut tx, account_id, acl_id).await?;

    // Resolve the character_id: the request's value if present, else the minted
    // (or reused) orphan's UUID.
    let character_id = match (input.character_id, &pending_mint) {
        (Some(id), _) => Some(id),
        (None, Some((eve_character_id, affiliations))) => {
            let (corporation_id, corporation_name, alliance_id, alliance_name) = affiliations;
            Some(
                characters::find_or_mint_orphan_in_tx(
                    &mut tx,
                    *eve_character_id,
                    &input.name,
                    *corporation_id,
                    corporation_name,
                    *alliance_id,
                    alliance_name.as_deref(),
                )
                .await?,
            )
        }
        // Corp/alliance member — no character_id.
        (None, None) => None,
    };

    let member = member_db::add_member(
        &mut *tx,
        acl_id,
        input.member_type.as_str(),
        input.eve_entity_id,
        character_id,
        &input.name,
        input.permission.as_str(),
    )
    .await
    .map_err(map_member_db_err)?;

    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::AclMemberAdded {
            account_id,
            acl_id,
            member_name: member.name.clone(),
            eve_entity_id: member.eve_entity_id,
            member_type: input.member_type.as_str().to_string(),
            permission: input.permission.as_str().to_string(),
        },
    )
    .await?;
    tx.commit().await?;

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
    let mut tx = pool.begin().await?;

    load_owned_acl_in_tx(&mut tx, account_id, acl_id).await?;

    let updated =
        member_db::update_member_permission(&mut *tx, acl_id, member_id, permission.as_str())
            .await
            .map_err(map_member_db_err)?
            .ok_or(AppError::NotFound)?;

    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::AclMemberPermissionChanged {
            account_id,
            acl_id,
            member_name: updated.name.clone(),
            eve_entity_id: updated.eve_entity_id,
            permission: permission.as_str().to_string(),
        },
    )
    .await?;
    tx.commit().await?;

    Ok(updated)
}

/// Removes a member from an ACL the account owns.
pub async fn remove_member(
    pool: &PgPool,
    account_id: Uuid,
    acl_id: Uuid,
    member_id: Uuid,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    load_owned_acl_in_tx(&mut tx, account_id, acl_id).await?;

    let removed = member_db::remove_member(&mut *tx, acl_id, member_id)
        .await?
        .ok_or(AppError::NotFound)?;

    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::AclMemberRemoved {
            account_id,
            acl_id,
            member_name: removed.name.clone(),
            eve_entity_id: removed.eve_entity_id,
        },
    )
    .await?;
    tx.commit().await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Loads an ACL and asserts the account owns it. `NotFound` if absent,
/// `Forbidden` if owned by someone else. Pool-based; for read-only ownership
/// checks (e.g. `list_members`).
async fn load_owned_acl(pool: &PgPool, account_id: Uuid, acl_id: Uuid) -> Result<Acl, AppError> {
    let acl = db::find_acl_by_id(pool, acl_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if acl.owner_account_id != Some(account_id) {
        return Err(AppError::Forbidden);
    }
    Ok(acl)
}

/// Transactional ownership check: the same as [`load_owned_acl`] but reading
/// inside the caller's transaction, so the authorisation and the mutation it
/// guards (and the audit row) all commit atomically — no TOCTOU window.
async fn load_owned_acl_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
    acl_id: Uuid,
) -> Result<Acl, AppError> {
    let acl = db::find_acl_by_id(&mut **tx, acl_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if acl.owner_account_id != Some(account_id) {
        return Err(AppError::Forbidden);
    }
    Ok(acl)
}

/// Best-effort fetch of a character's corporation (and alliance) affiliation for
/// a minted orphan's snapshot. Returns `(corporation_id, corporation_name,
/// alliance_id, alliance_name)`; on any failure the corp falls back to `(0, "")`
/// and the alliance to `None`, so the orphan's NOT NULL columns are always
/// satisfiable and the add still succeeds when ESI is unavailable.
async fn fetch_orphan_affiliations(
    http: &ClientWithMiddleware,
    eve_character_id: i64,
) -> (i64, String, Option<i64>, Option<String>) {
    #[derive(serde::Deserialize)]
    struct CharacterAffiliation {
        corporation_id: i64,
        #[serde(default)]
        alliance_id: Option<i64>,
    }

    let url = format!("{}/characters/{eve_character_id}/", public_info::ESI_BASE);
    let affil: Option<CharacterAffiliation> = async {
        http.get(&url)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .await
            .ok()
    }
    .await;

    let Some(affil) = affil else {
        return (0, String::new(), None, None);
    };

    let corporation_name = public_info::fetch_corporation_name(http, affil.corporation_id)
        .await
        .unwrap_or_default();

    let (alliance_id, alliance_name) = match affil.alliance_id {
        Some(aid) => (
            Some(aid),
            public_info::fetch_alliance_name(http, aid).await.ok(),
        ),
        None => (None, None),
    };

    (
        affil.corporation_id,
        corporation_name,
        alliance_id,
        alliance_name,
    )
}

/// Validates that the identifier columns match the member type. Every member
/// carries `eve_entity_id` — the durable EVE id (character/corporation/alliance)
/// — so the audit snapshot is uniform and so an unknown character can be minted
/// from it at add time. A `character` member MAY carry `character_id`, the
/// internal FK link to an existing `eve_character` row; when absent the add mints
/// the orphan keyed by `eve_entity_id`. Corporation and alliance members carry no
/// `character_id`.
pub fn validate_member_shape(input: &AddMemberInput) -> Result<(), AppError> {
    if input.eve_entity_id.is_none() {
        return Err(AppError::BadRequest(
            "members require eve_entity_id".to_string(),
        ));
    }
    match input.member_type {
        // A character member may arrive with or without character_id: present →
        // reference the existing row; absent → mint the orphan from eve_entity_id.
        MemberType::Character => {}
        MemberType::Corporation | MemberType::Alliance => {
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

/// Maps a member-insert/update `DbError` to an `AppError`.
///
/// - A unique violation means the entity is already a member of the ACL → 409
///   `duplicate_acl_member` (the partial unique indexes back this).
/// - A CHECK violation (e.g. raising a corporation member to `admin`, an invalid
///   member_type/permission value) is a malformed request → 400. Detection is by
///   SQLSTATE `23514` (`DbError::CheckViolation`), not message-substring.
fn map_member_db_err(e: DbError) -> AppError {
    match e {
        DbError::UniqueViolation { .. } => AppError::Conflict(ConflictKind::DuplicateAclMember),
        DbError::CheckViolation { .. } => {
            AppError::BadRequest("invalid acl member type/permission combination".to_string())
        }
        DbError::Other(err) => AppError::Internal(err),
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
    fn character_member_without_character_id_is_allowed() {
        // Has the EVE id but no internal FK link → valid; the add will mint the
        // orphan keyed by eve_entity_id.
        validate_member_shape(&input(MemberType::Character, Some(5), None)).unwrap();
    }

    #[test]
    fn character_member_requires_eve_entity_id() {
        // Has the FK link but no durable EVE id → rejected (the audit snapshot
        // would have no EVE id).
        let err = validate_member_shape(&input(MemberType::Character, None, Some(Uuid::new_v4())))
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
        // A character carries both its EVE id and the internal FK link.
        validate_member_shape(&input(
            MemberType::Character,
            Some(95465499),
            Some(Uuid::new_v4()),
        ))
        .unwrap();
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

    // ---- emit-site name-snapshot integration tests ----

    use crate::db::accounts;
    use sqlx::PgPool;

    fn http() -> ClientWithMiddleware {
        reqwest::Client::new().into()
    }

    /// A `MintContext` whose http client points nowhere usable — fine for the
    /// existing-row / corp / alliance paths that never fetch affiliations.
    fn mint(http: &ClientWithMiddleware) -> MintContext<'_> {
        MintContext { http }
    }

    async fn latest_details(pool: &PgPool, event_type: &str) -> serde_json::Value {
        sqlx::query_scalar!(
            "SELECT details FROM audit_log WHERE event_type = $1
             ORDER BY occurred_at DESC, id DESC LIMIT 1",
            event_type,
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[sqlx::test]
    async fn add_member_snapshots_name_and_eve_id(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let acl = db::insert_acl_for_test(&pool, owner, "Corp ACL").await;

        add_member(
            &pool,
            &mint(&http()),
            owner,
            acl.id,
            AddMemberInput {
                member_type: MemberType::Corporation,
                eve_entity_id: Some(98000001),
                character_id: None,
                name: "Wasp Industries".to_string(),
                permission: AclPermission::Read,
            },
        )
        .await
        .unwrap();

        let d = latest_details(&pool, "acl_member_added").await;
        assert_eq!(d["member_name"], "Wasp Industries");
        assert_eq!(d["eve_entity_id"], 98000001i64);
        assert!(d.get("member_id").is_none());
        assert!(d.get("acl_id").is_none());
    }

    #[sqlx::test]
    async fn add_character_member_snapshots_eve_id(pool: PgPool) {
        use crate::db::test_helpers::insert_character;

        let owner = accounts::create_account(&pool).await.unwrap();
        let acl = db::insert_acl_for_test(&pool, owner, "Corp ACL").await;
        // A character member carries its EVE id in eve_entity_id (the durable
        // ESI identity, uniform with corp/alliance) plus character_id (the
        // internal FK link). The audit snapshot uses eve_entity_id.
        let char_id = insert_character(&pool, owner, 95465499, "Tocoquadi").await;

        add_member(
            &pool,
            &mint(&http()),
            owner,
            acl.id,
            AddMemberInput {
                member_type: MemberType::Character,
                eve_entity_id: Some(95465499),
                character_id: Some(char_id),
                name: "Tocoquadi".to_string(),
                permission: AclPermission::Manage,
            },
        )
        .await
        .unwrap();

        let d = latest_details(&pool, "acl_member_added").await;
        assert_eq!(d["member_name"], "Tocoquadi");
        // The bug: this was NULL for character members. It must be the EVE id.
        assert_eq!(d["eve_entity_id"], 95465499i64);
    }

    #[sqlx::test]
    async fn remove_member_snapshots_removed_member_name(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let acl = db::insert_acl_for_test(&pool, owner, "Corp ACL").await;

        let member = add_member(
            &pool,
            &mint(&http()),
            owner,
            acl.id,
            AddMemberInput {
                member_type: MemberType::Corporation,
                eve_entity_id: Some(98000002),
                character_id: None,
                name: "Doomed Corp".to_string(),
                permission: AclPermission::Read,
            },
        )
        .await
        .unwrap();

        remove_member(&pool, owner, acl.id, member.id)
            .await
            .unwrap();

        // The member row is gone, but the audit row names it via the snapshot.
        assert!(
            member_db::list_members(&pool, acl.id)
                .await
                .unwrap()
                .is_empty()
        );
        let d = latest_details(&pool, "acl_member_removed").await;
        assert_eq!(d["member_name"], "Doomed Corp");
        assert_eq!(d["eve_entity_id"], 98000002i64);
    }

    #[sqlx::test]
    async fn remove_missing_member_is_not_found(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let acl = db::insert_acl_for_test(&pool, owner, "Corp ACL").await;
        let err = remove_member(&pool, owner, acl.id, Uuid::new_v4())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound));
    }

    #[sqlx::test]
    async fn duplicate_corporation_member_is_conflict(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let acl = db::insert_acl_for_test(&pool, owner, "Corp ACL").await;

        let http = http();
        let mint_ctx = mint(&http);
        let add = |perm: AclPermission| {
            add_member(
                &pool,
                &mint_ctx,
                owner,
                acl.id,
                AddMemberInput {
                    member_type: MemberType::Corporation,
                    eve_entity_id: Some(98000050),
                    character_id: None,
                    name: "Wasp Industries".to_string(),
                    permission: perm,
                },
            )
        };

        add(AclPermission::Read).await.unwrap();
        // Re-adding the same corporation (even with a different permission) is a
        // duplicate → 409, and no second row is inserted.
        let err = add(AclPermission::ReadWrite).await.unwrap_err();
        assert!(matches!(
            err,
            AppError::Conflict(ConflictKind::DuplicateAclMember)
        ));
        assert_eq!(
            member_db::list_members(&pool, acl.id).await.unwrap().len(),
            1
        );
    }

    #[sqlx::test]
    async fn duplicate_character_member_is_conflict(pool: PgPool) {
        use crate::db::test_helpers::insert_character;
        let owner = accounts::create_account(&pool).await.unwrap();
        let acl = db::insert_acl_for_test(&pool, owner, "Corp ACL").await;
        let char_id = insert_character(&pool, owner, 95465500, "Tocoquadi").await;

        let http = http();
        let mint_ctx = mint(&http);
        let add = || {
            add_member(
                &pool,
                &mint_ctx,
                owner,
                acl.id,
                AddMemberInput {
                    member_type: MemberType::Character,
                    eve_entity_id: Some(95465500),
                    character_id: Some(char_id),
                    name: "Tocoquadi".to_string(),
                    permission: AclPermission::Read,
                },
            )
        };
        add().await.unwrap();
        let err = add().await.unwrap_err();
        assert!(matches!(
            err,
            AppError::Conflict(ConflictKind::DuplicateAclMember)
        ));
        assert_eq!(
            member_db::list_members(&pool, acl.id).await.unwrap().len(),
            1
        );
    }

    #[sqlx::test]
    async fn same_entity_id_different_member_type_allowed(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let acl = db::insert_acl_for_test(&pool, owner, "Corp ACL").await;

        // Same eve_entity_id N as both an alliance and a corporation member: the
        // identities differ by member type, so both are permitted.
        let http = http();
        for mt in [MemberType::Alliance, MemberType::Corporation] {
            add_member(
                &pool,
                &mint(&http),
                owner,
                acl.id,
                AddMemberInput {
                    member_type: mt,
                    eve_entity_id: Some(99000001),
                    character_id: None,
                    name: "Shared Id".to_string(),
                    permission: AclPermission::Read,
                },
            )
            .await
            .unwrap();
        }
        assert_eq!(
            member_db::list_members(&pool, acl.id).await.unwrap().len(),
            2
        );
    }

    #[sqlx::test]
    async fn check_violation_still_maps_to_400(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let acl = db::insert_acl_for_test(&pool, owner, "Corp ACL").await;

        // A corporation cannot hold `manage` (acl_member_role_for_type CHECK).
        // It must surface as a 400 BadRequest, not a 409 or 500.
        let err = add_member(
            &pool,
            &mint(&http()),
            owner,
            acl.id,
            AddMemberInput {
                member_type: MemberType::Corporation,
                eve_entity_id: Some(98000060),
                character_id: None,
                name: "Overreach Corp".to_string(),
                permission: AclPermission::Manage,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
        assert!(
            member_db::list_members(&pool, acl.id)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[sqlx::test]
    async fn add_character_member_without_character_id_mints_orphan(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let acl = db::insert_acl_for_test(&pool, owner, "Corp ACL").await;

        // The affiliation fetch targets the hard-coded ESI base, unreachable in
        // tests → the mint falls back to placeholder corp columns, and the add
        // still succeeds. `name` comes from the request, not ESI.
        let member = add_member(
            &pool,
            &mint(&http()),
            owner,
            acl.id,
            AddMemberInput {
                member_type: MemberType::Character,
                eve_entity_id: Some(123456),
                character_id: None,
                name: "New Pilot".to_string(),
                permission: AclPermission::Read,
            },
        )
        .await
        .unwrap();

        // A character_id was resolved (the minted orphan's UUID).
        let minted_id = member.character_id.expect("minted orphan UUID");

        // Exactly one orphan row exists for the EVE id, holding no tokens.
        let row = sqlx::query!(
            r#"
            SELECT id, account_id, name, encrypted_refresh_token, is_main, scopes
            FROM eve_character WHERE eve_character_id = 123456
            "#
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.id, minted_id);
        assert!(row.account_id.is_none());
        assert_eq!(row.name, "New Pilot");
        assert!(row.encrypted_refresh_token.is_none());
        assert!(!row.is_main);
        assert!(row.scopes.is_empty());
    }

    #[sqlx::test]
    async fn add_character_member_without_character_id_reuses_existing_row(pool: PgPool) {
        use crate::db::characters as char_db;
        let owner = accounts::create_account(&pool).await.unwrap();
        let acl = db::insert_acl_for_test(&pool, owner, "Corp ACL").await;

        // A row already exists for this character.
        let existing = char_db::create_orphan(&pool, 222333, "Known Pilot", 1, "Corp", None, None)
            .await
            .unwrap();

        let member = add_member(
            &pool,
            &mint(&http()),
            owner,
            acl.id,
            AddMemberInput {
                member_type: MemberType::Character,
                eve_entity_id: Some(222333),
                character_id: None,
                name: "Known Pilot".to_string(),
                permission: AclPermission::Read,
            },
        )
        .await
        .unwrap();

        // No second row minted; the member references the existing UUID.
        assert_eq!(member.character_id, Some(existing));
        let count = sqlx::query!(
            "SELECT COUNT(*) AS \"c!\" FROM eve_character WHERE eve_character_id = 222333"
        )
        .fetch_one(&pool)
        .await
        .unwrap()
        .c;
        assert_eq!(count, 1);
    }

    #[sqlx::test]
    async fn add_character_member_without_eve_entity_id_is_rejected(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let acl = db::insert_acl_for_test(&pool, owner, "Corp ACL").await;

        let err = add_member(
            &pool,
            &mint(&http()),
            owner,
            acl.id,
            AddMemberInput {
                member_type: MemberType::Character,
                eve_entity_id: None,
                character_id: None,
                name: "No Id".to_string(),
                permission: AclPermission::Read,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
        assert!(
            member_db::list_members(&pool, acl.id)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[sqlx::test]
    async fn concurrent_mint_does_not_duplicate_orphan(pool: PgPool) {
        // Two adds for the same unknown character (in two ACLs) race the mint; the
        // unique eve_character_id index arbitrates and only one row results.
        let owner = accounts::create_account(&pool).await.unwrap();
        let acl_a = db::insert_acl_for_test(&pool, owner, "ACL A").await;
        let acl_b = db::insert_acl_for_test(&pool, owner, "ACL B").await;

        let http = http();
        let mint_ctx = mint(&http);
        let add = |acl_id: Uuid| {
            add_member(
                &pool,
                &mint_ctx,
                owner,
                acl_id,
                AddMemberInput {
                    member_type: MemberType::Character,
                    eve_entity_id: Some(444555),
                    character_id: None,
                    name: "Raced Pilot".to_string(),
                    permission: AclPermission::Read,
                },
            )
        };

        let (ra, rb) = tokio::join!(add(acl_a.id), add(acl_b.id));
        let ma = ra.unwrap();
        let mb = rb.unwrap();
        // Both members reference the same single orphan row.
        assert_eq!(ma.character_id, mb.character_id);
        let count = sqlx::query!(
            "SELECT COUNT(*) AS \"c!\" FROM eve_character WHERE eve_character_id = 444555"
        )
        .fetch_one(&pool)
        .await
        .unwrap()
        .c;
        assert_eq!(count, 1);
    }

    #[sqlx::test]
    async fn minted_orphan_is_claimable_after_member_add(pool: PgPool) {
        use crate::db::characters as char_db;
        // After a member-add mints an orphan, an SSO login for that pilot claims
        // the same row (sets account_id, writes tokens) without a second row.
        let owner = accounts::create_account(&pool).await.unwrap();
        let acl = db::insert_acl_for_test(&pool, owner, "Corp ACL").await;

        let member = add_member(
            &pool,
            &mint(&http()),
            owner,
            acl.id,
            AddMemberInput {
                member_type: MemberType::Character,
                eve_entity_id: Some(666777),
                character_id: None,
                name: "Future Owner".to_string(),
                permission: AclPermission::Read,
            },
        )
        .await
        .unwrap();
        let minted_id = member.character_id.unwrap();

        // Simulate the claim: upsert_tokens binds the account to the existing row.
        let claimer = accounts::create_account(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let claimed_id = char_db::upsert_tokens(
            &mut tx,
            claimer,
            666777,
            "Future Owner",
            1,
            "Corp",
            None,
            None,
            "client",
            "access",
            "refresh",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &[],
            "owner-hash",
            &[0u8; 32],
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        // Same row UUID, now account-bound — the member reference stays valid.
        assert_eq!(claimed_id, minted_id);
        let row =
            sqlx::query!("SELECT account_id FROM eve_character WHERE eve_character_id = 666777")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(row.account_id, Some(claimer));
    }

    #[sqlx::test]
    async fn member_add_and_audit_are_atomic(pool: PgPool) {
        // The member insert participates in the same transaction as the audit
        // write: rolling back the transaction (standing in for a failed audit
        // write) leaves the ACL's member list unchanged. Driving the db insert
        // through `&mut *tx` mirrors exactly what the service does.
        let owner = accounts::create_account(&pool).await.unwrap();
        let acl = db::insert_acl_for_test(&pool, owner, "Corp ACL").await;

        let mut tx = pool.begin().await.unwrap();
        member_db::add_member(
            &mut *tx,
            acl.id,
            "corporation",
            Some(98000070),
            None,
            "Rolled Back",
            "read",
        )
        .await
        .unwrap();
        tx.rollback().await.unwrap();

        assert!(
            member_db::list_members(&pool, acl.id)
                .await
                .unwrap()
                .is_empty()
        );
    }
}
