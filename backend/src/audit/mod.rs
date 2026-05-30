use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::db::characters;

/// Identifies the EVE character that an SSO-time audit event SHALL attribute to,
/// in cases where no authenticated session exists yet (e.g. the SSO callback's
/// `account_registered` / `orphan_character_claimed` / first-account bootstrap
/// events).
#[derive(Debug, Clone)]
pub struct ActingCharacter {
    pub eve_character_id: i64,
    pub name: String,
}

/// Discriminates the two paths through which an account can become a server
/// admin: the very-first-account auto-promotion at registration time, or a
/// future admin-initiated grant.
#[derive(Debug, Clone, Copy)]
pub enum ServerAdminGrantSource {
    FirstAccountBootstrap,
    AdminGrant,
}

impl ServerAdminGrantSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FirstAccountBootstrap => "first_account_bootstrap",
            Self::AdminGrant => "admin_grant",
        }
    }
}

/// Catalogue of recordable actions. Variants marked **dormant** are present
/// from day one but emitted by no production code path yet — they activate
/// when the feature that needs them lands. The catalogue is stable: existing
/// `event_type()` strings SHALL NOT be renamed once shipped.
#[derive(Debug, Clone)]
pub enum AuditEvent {
    AccountRegistered {
        account_id: Uuid,
        eve_character_id: i64,
        character_name: String,
    },
    AccountDeletionRequested {
        account_id: Uuid,
    },
    AccountReactivated {
        account_id: Uuid,
    },
    /// Dormant: emitted by a future hard-delete-after-grace sweep.
    AccountPurged {
        account_id: Uuid,
    },
    CharacterAdded {
        account_id: Uuid,
        eve_character_id: i64,
        character_name: String,
    },
    CharacterRemoved {
        account_id: Uuid,
        eve_character_id: i64,
    },
    CharacterSetMain {
        account_id: Uuid,
        eve_character_id: i64,
    },
    /// Renamed from the older iteration's `GhostCharacterClaimed` — the
    /// current codebase uses "orphan" throughout for `account_id IS NULL`
    /// character rows.
    OrphanCharacterClaimed {
        account_id: Uuid,
        eve_character_id: i64,
        character_name: String,
    },
    ApiKeyCreated {
        account_id: Uuid,
        key_id: Uuid,
        name: String,
    },
    ApiKeyRevoked {
        account_id: Uuid,
        key_id: Uuid,
    },
    ServerAdminGranted {
        account_id: Uuid,
        source: ServerAdminGrantSource,
    },
    /// Dormant: emitted by a future admin-initiated demote endpoint.
    ServerAdminRevoked {
        account_id: Uuid,
    },
    /// Dormant: emitted by a future admin-initiated block endpoint.
    EveCharacterBlocked {
        eve_character_id: i64,
        reason: Option<String>,
    },
    /// Dormant: emitted by a future admin-initiated unblock endpoint.
    EveCharacterUnblocked {
        eve_character_id: i64,
    },
    /// Dormant: emitted by a future map-create handler.
    MapCreated {
        account_id: Uuid,
        map_id: Uuid,
        name: String,
    },
    /// Dormant: emitted by a future map-delete handler.
    MapDeleted {
        account_id: Uuid,
        map_id: Uuid,
        name: String,
    },
    /// Dormant: emitted by a future ACL-create handler.
    AclCreated {
        account_id: Uuid,
        acl_id: Uuid,
        name: String,
    },
    /// Dormant: emitted by a future ACL-rename handler.
    AclRenamed {
        account_id: Uuid,
        acl_id: Uuid,
        old_name: String,
        new_name: String,
    },
    /// Dormant: emitted by a future ACL-delete handler.
    AclDeleted {
        account_id: Uuid,
        acl_id: Uuid,
        name: String,
    },
    /// Dormant: emitted by a future ACL-member-add handler.
    AclMemberAdded {
        account_id: Uuid,
        acl_id: Uuid,
        member_id: Uuid,
        member_type: String,
        permission: String,
    },
    /// Dormant: emitted by a future ACL-member-permission-change handler.
    AclMemberPermissionChanged {
        account_id: Uuid,
        acl_id: Uuid,
        member_id: Uuid,
        permission: String,
    },
    /// Dormant: emitted by a future ACL-member-remove handler.
    AclMemberRemoved {
        account_id: Uuid,
        acl_id: Uuid,
        member_id: Uuid,
    },
    /// Dormant: emitted by a future ACL-attach handler.
    AclAttachedToMap {
        account_id: Uuid,
        map_id: Uuid,
        acl_id: Uuid,
    },
    /// Dormant: emitted by a future ACL-detach handler.
    AclDetachedFromMap {
        account_id: Uuid,
        map_id: Uuid,
        acl_id: Uuid,
    },
    /// Dormant: emitted by a future admin-override map-ownership-change handler.
    AdminMapOwnershipChanged {
        map_id: Uuid,
        old_owner: Uuid,
        new_owner: Uuid,
    },
    /// Dormant: emitted by a future admin-override map-hard-delete handler.
    AdminMapHardDeleted {
        map_id: Uuid,
        name: String,
    },
    /// Dormant: emitted by a future admin-override acl-ownership-change handler.
    AdminAclOwnershipChanged {
        acl_id: Uuid,
        old_owner: Uuid,
        new_owner: Uuid,
    },
    /// Dormant: emitted by a future admin-override acl-hard-delete handler.
    AdminAclHardDeleted {
        acl_id: Uuid,
        name: String,
    },
}

impl AuditEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::AccountRegistered { .. } => "account_registered",
            Self::AccountDeletionRequested { .. } => "account_deletion_requested",
            Self::AccountReactivated { .. } => "account_reactivated",
            Self::AccountPurged { .. } => "account_purged",
            Self::CharacterAdded { .. } => "character_added",
            Self::CharacterRemoved { .. } => "character_removed",
            Self::CharacterSetMain { .. } => "character_set_main",
            Self::OrphanCharacterClaimed { .. } => "orphan_character_claimed",
            Self::ApiKeyCreated { .. } => "api_key_created",
            Self::ApiKeyRevoked { .. } => "api_key_revoked",
            Self::ServerAdminGranted { .. } => "server_admin_granted",
            Self::ServerAdminRevoked { .. } => "server_admin_revoked",
            Self::EveCharacterBlocked { .. } => "eve_character_blocked",
            Self::EveCharacterUnblocked { .. } => "eve_character_unblocked",
            Self::MapCreated { .. } => "map_created",
            Self::MapDeleted { .. } => "map_deleted",
            Self::AclCreated { .. } => "acl_created",
            Self::AclRenamed { .. } => "acl_renamed",
            Self::AclDeleted { .. } => "acl_deleted",
            Self::AclMemberAdded { .. } => "acl_member_added",
            Self::AclMemberPermissionChanged { .. } => "acl_member_permission_changed",
            Self::AclMemberRemoved { .. } => "acl_member_removed",
            Self::AclAttachedToMap { .. } => "acl_attached_to_map",
            Self::AclDetachedFromMap { .. } => "acl_detached_from_map",
            Self::AdminMapOwnershipChanged { .. } => "admin_map_ownership_changed",
            Self::AdminMapHardDeleted { .. } => "admin_map_hard_deleted",
            Self::AdminAclOwnershipChanged { .. } => "admin_acl_ownership_changed",
            Self::AdminAclHardDeleted { .. } => "admin_acl_hard_deleted",
        }
    }

    pub fn details(&self) -> Value {
        match self {
            // actor is NULL for registration (no session yet) — account_id is
            // not in the actor column, so include it here.
            Self::AccountRegistered {
                account_id,
                eve_character_id,
                character_name,
            } => json!({
                "account_id": account_id,
                "eve_character_id": eve_character_id,
                "character_name": character_name,
            }),
            // actor == account — no need to repeat it.
            Self::AccountDeletionRequested { .. } => json!({}),
            // actor is NULL for reactivation (the session is being established
            // mid-callback) — include account_id so it's not lost.
            Self::AccountReactivated { account_id } => json!({ "account_id": account_id }),
            // actor is NULL for purge — include account_id.
            Self::AccountPurged { account_id } => json!({ "account_id": account_id }),
            Self::CharacterAdded {
                eve_character_id,
                character_name,
                ..
            } => json!({
                "eve_character_id": eve_character_id,
                "character_name": character_name,
            }),
            Self::CharacterRemoved {
                eve_character_id, ..
            } => json!({
                "eve_character_id": eve_character_id,
            }),
            // The `eve_character_id` carried here is the *new* main; the actor
            // character snapshot will be the outgoing main (resolved at write
            // time, before the is_main flip commits).
            Self::CharacterSetMain {
                eve_character_id, ..
            } => json!({
                "eve_character_id": eve_character_id,
            }),
            // actor is NULL for login-time orphan claim and Some(account) for
            // the add-character flow — include account_id for consistency so
            // the event is self-contained either way.
            Self::OrphanCharacterClaimed {
                account_id,
                eve_character_id,
                character_name,
            } => json!({
                "account_id": account_id,
                "eve_character_id": eve_character_id,
                "character_name": character_name,
            }),
            Self::ApiKeyCreated { key_id, name, .. } => json!({
                "key_id": key_id,
                "name": name,
            }),
            Self::ApiKeyRevoked { key_id, .. } => json!({
                "key_id": key_id,
            }),
            // actor is NULL for first-account bootstrap and Some(admin) for
            // future admin-initiated grant — include target account_id.
            Self::ServerAdminGranted { account_id, source } => json!({
                "account_id": account_id,
                "source": source.as_str(),
            }),
            // actor is the admin performing the demote; include target account_id.
            Self::ServerAdminRevoked { account_id } => json!({ "account_id": account_id }),
            Self::EveCharacterBlocked {
                eve_character_id,
                reason,
            } => json!({
                "eve_character_id": eve_character_id,
                "reason": reason,
            }),
            Self::EveCharacterUnblocked { eve_character_id } => json!({
                "eve_character_id": eve_character_id,
            }),
            Self::MapCreated { map_id, name, .. } => json!({
                "map_id": map_id,
                "name": name,
            }),
            Self::MapDeleted { map_id, name, .. } => json!({
                "map_id": map_id,
                "name": name,
            }),
            Self::AclCreated { acl_id, name, .. } => json!({
                "acl_id": acl_id,
                "name": name,
            }),
            Self::AclRenamed {
                acl_id,
                old_name,
                new_name,
                ..
            } => json!({
                "acl_id": acl_id,
                "old_name": old_name,
                "new_name": new_name,
            }),
            Self::AclDeleted { acl_id, name, .. } => json!({
                "acl_id": acl_id,
                "name": name,
            }),
            Self::AclMemberAdded {
                acl_id,
                member_id,
                member_type,
                permission,
                ..
            } => json!({
                "acl_id": acl_id,
                "member_id": member_id,
                "member_type": member_type,
                "permission": permission,
            }),
            Self::AclMemberPermissionChanged {
                acl_id,
                member_id,
                permission,
                ..
            } => json!({
                "acl_id": acl_id,
                "member_id": member_id,
                "permission": permission,
            }),
            Self::AclMemberRemoved {
                acl_id, member_id, ..
            } => json!({
                "acl_id": acl_id,
                "member_id": member_id,
            }),
            Self::AclAttachedToMap { map_id, acl_id, .. } => json!({
                "map_id": map_id,
                "acl_id": acl_id,
            }),
            Self::AclDetachedFromMap { map_id, acl_id, .. } => json!({
                "map_id": map_id,
                "acl_id": acl_id,
            }),
            Self::AdminMapOwnershipChanged {
                map_id,
                old_owner,
                new_owner,
            } => json!({
                "map_id": map_id,
                "old_owner": old_owner,
                "new_owner": new_owner,
            }),
            Self::AdminMapHardDeleted { map_id, name } => json!({
                "map_id": map_id,
                "name": name,
            }),
            Self::AdminAclOwnershipChanged {
                acl_id,
                old_owner,
                new_owner,
            } => json!({
                "acl_id": acl_id,
                "old_owner": old_owner,
                "new_owner": new_owner,
            }),
            Self::AdminAclHardDeleted { acl_id, name } => json!({
                "acl_id": acl_id,
                "name": name,
            }),
        }
    }
}

/// A row read back from `audit_log` — returned by `list_audit_log`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AuditLogEntry {
    pub id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub actor_account_id: Option<Uuid>,
    pub actor_character_id: Option<i64>,
    pub actor_character_name: Option<String>,
    pub event_type: String,
    pub details: Value,
}

/// Writes a single audit event participating in the caller's transaction.
///
/// Actor-column resolution:
/// 1. If `actor_account_id` is `Some`, the account's main character is looked
///    up within `tx` and snapshotted into `actor_character_id` /
///    `actor_character_name`. If the lookup unexpectedly returns no row, a
///    `tracing::error!` fires and the function continues with NULL character
///    columns (fail-soft — the audit row is more useful than no row at all).
/// 2. Else if `acting_as` is `Some`, those values are written directly. This
///    covers SSO-callback events that fire before a session exists.
/// 3. Else all three actor columns are NULL (system events).
pub async fn record_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    actor_account_id: Option<Uuid>,
    acting_as: Option<ActingCharacter>,
    event: AuditEvent,
) -> Result<()> {
    let event_type = event.event_type();
    let details = event.details();

    let (actor_character_id, actor_character_name) = if let Some(account_id) = actor_account_id {
        match characters::get_main_for_account_tx(tx, account_id).await? {
            Some((eve_id, name)) => (Some(eve_id), Some(name)),
            None => {
                tracing::error!(
                    account_id = %account_id,
                    event_type,
                    "audit: account has no main at write time — actor character columns left NULL"
                );
                (None, None)
            }
        }
    } else if let Some(c) = acting_as {
        (Some(c.eve_character_id), Some(c.name))
    } else {
        (None, None)
    };

    sqlx::query!(
        r#"
        INSERT INTO audit_log (
            actor_account_id,
            actor_character_id,
            actor_character_name,
            event_type,
            details
        )
        VALUES ($1, $2, $3, $4, $5)
        "#,
        actor_account_id,
        actor_character_id,
        actor_character_name,
        event_type,
        details,
    )
    .execute(&mut **tx)
    .await
    .context("failed to insert audit log entry")?;

    Ok(())
}

/// Reads audit-log entries newest-first, with three optional filters
/// (`event_type`, `actor_account_id`, `before`) and a hard `limit`. Used by
/// the admin audit-browser; `limit` is the caller's responsibility to clamp.
///
/// All filters bind as parameters — no string interpolation, no SQL injection
/// surface.
pub async fn list_audit_log(
    pool: &PgPool,
    event_type: Option<&str>,
    actor_account_id: Option<Uuid>,
    before: Option<DateTime<Utc>>,
    limit: i64,
) -> Result<Vec<AuditLogEntry>> {
    let rows = sqlx::query!(
        r#"
        SELECT id, occurred_at, actor_account_id, actor_character_id,
               actor_character_name, event_type, details
        FROM audit_log
        WHERE ($1::TEXT IS NULL        OR event_type       = $1)
          AND ($2::UUID IS NULL        OR actor_account_id = $2)
          AND ($3::TIMESTAMPTZ IS NULL OR occurred_at      < $3)
        ORDER BY occurred_at DESC
        LIMIT $4
        "#,
        event_type,
        actor_account_id,
        before,
        limit,
    )
    .fetch_all(pool)
    .await
    .context("failed to read audit_log")?;

    Ok(rows
        .into_iter()
        .map(|r| AuditLogEntry {
            id: r.id,
            occurred_at: r.occurred_at,
            actor_account_id: r.actor_account_id,
            actor_character_id: r.actor_character_id,
            actor_character_name: r.actor_character_name,
            event_type: r.event_type,
            details: r.details,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::accounts;
    use crate::db::characters as char_db;

    fn test_uuid() -> Uuid {
        Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()
    }

    fn other_uuid() -> Uuid {
        Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap()
    }

    // -----------------------------------------------------------------------
    // event_type() + details() shape tests — one per variant.
    // -----------------------------------------------------------------------

    #[test]
    fn account_registered_serialises_correctly() {
        let id = test_uuid();
        let event = AuditEvent::AccountRegistered {
            account_id: id,
            eve_character_id: 123456789,
            character_name: "Test Pilot".into(),
        };
        assert_eq!(event.event_type(), "account_registered");
        let d = event.details();
        assert_eq!(d["account_id"], id.to_string());
        assert_eq!(d["eve_character_id"], 123456789i64);
        assert_eq!(d["character_name"], "Test Pilot");
    }

    #[test]
    fn account_deletion_requested_serialises_correctly() {
        let id = test_uuid();
        let event = AuditEvent::AccountDeletionRequested { account_id: id };
        assert_eq!(event.event_type(), "account_deletion_requested");
        // account_id is carried by actor_account_id column, not repeated in details.
        assert!(event.details().as_object().unwrap().is_empty());
    }

    #[test]
    fn account_reactivated_serialises_correctly() {
        let id = test_uuid();
        let event = AuditEvent::AccountReactivated { account_id: id };
        assert_eq!(event.event_type(), "account_reactivated");
        assert_eq!(event.details()["account_id"], id.to_string());
    }

    #[test]
    fn account_purged_serialises_correctly() {
        let id = test_uuid();
        let event = AuditEvent::AccountPurged { account_id: id };
        assert_eq!(event.event_type(), "account_purged");
        assert_eq!(event.details()["account_id"], id.to_string());
    }

    #[test]
    fn character_added_serialises_correctly() {
        let id = test_uuid();
        let event = AuditEvent::CharacterAdded {
            account_id: id,
            eve_character_id: 123456789,
            character_name: "Test Character".into(),
        };
        assert_eq!(event.event_type(), "character_added");
        // account_id carried by actor column.
        assert!(event.details().get("account_id").is_none());
        assert_eq!(event.details()["eve_character_id"], 123456789i64);
        assert_eq!(event.details()["character_name"], "Test Character");
    }

    #[test]
    fn character_removed_serialises_correctly() {
        let id = test_uuid();
        let event = AuditEvent::CharacterRemoved {
            account_id: id,
            eve_character_id: 42,
        };
        assert_eq!(event.event_type(), "character_removed");
        assert_eq!(event.details()["eve_character_id"], 42i64);
        assert!(event.details().get("account_id").is_none());
    }

    #[test]
    fn character_set_main_serialises_correctly() {
        let id = test_uuid();
        let event = AuditEvent::CharacterSetMain {
            account_id: id,
            eve_character_id: 99,
        };
        assert_eq!(event.event_type(), "character_set_main");
        assert_eq!(event.details()["eve_character_id"], 99i64);
        assert!(event.details().get("account_id").is_none());
    }

    #[test]
    fn orphan_character_claimed_serialises_correctly() {
        let id = test_uuid();
        let event = AuditEvent::OrphanCharacterClaimed {
            account_id: id,
            eve_character_id: 7,
            character_name: "Orphan Pilot".into(),
        };
        // Note: the rename from "ghost" to "orphan" is asserted here so any
        // regression to the old name fails the test.
        assert_eq!(event.event_type(), "orphan_character_claimed");
        let d = event.details();
        assert_eq!(d["account_id"], id.to_string());
        assert_eq!(d["eve_character_id"], 7i64);
        assert_eq!(d["character_name"], "Orphan Pilot");
    }

    #[test]
    fn api_key_created_serialises_correctly() {
        let account_id = test_uuid();
        let key_id = other_uuid();
        let event = AuditEvent::ApiKeyCreated {
            account_id,
            key_id,
            name: "My App".into(),
        };
        assert_eq!(event.event_type(), "api_key_created");
        let d = event.details();
        assert_eq!(d["key_id"], key_id.to_string());
        assert_eq!(d["name"], "My App");
        assert!(d.get("account_id").is_none());
    }

    #[test]
    fn api_key_revoked_serialises_correctly() {
        let account_id = test_uuid();
        let key_id = other_uuid();
        let event = AuditEvent::ApiKeyRevoked { account_id, key_id };
        assert_eq!(event.event_type(), "api_key_revoked");
        assert_eq!(event.details()["key_id"], key_id.to_string());
        assert!(event.details().get("account_id").is_none());
    }

    #[test]
    fn server_admin_granted_serialises_correctly_for_bootstrap() {
        let id = test_uuid();
        let event = AuditEvent::ServerAdminGranted {
            account_id: id,
            source: ServerAdminGrantSource::FirstAccountBootstrap,
        };
        assert_eq!(event.event_type(), "server_admin_granted");
        let d = event.details();
        assert_eq!(d["account_id"], id.to_string());
        assert_eq!(d["source"], "first_account_bootstrap");
    }

    #[test]
    fn server_admin_granted_serialises_correctly_for_admin_grant() {
        let id = test_uuid();
        let event = AuditEvent::ServerAdminGranted {
            account_id: id,
            source: ServerAdminGrantSource::AdminGrant,
        };
        assert_eq!(event.details()["source"], "admin_grant");
    }

    #[test]
    fn server_admin_revoked_serialises_correctly() {
        let id = test_uuid();
        let event = AuditEvent::ServerAdminRevoked { account_id: id };
        assert_eq!(event.event_type(), "server_admin_revoked");
        assert_eq!(event.details()["account_id"], id.to_string());
    }

    #[test]
    fn eve_character_blocked_serialises_correctly_with_reason() {
        let event = AuditEvent::EveCharacterBlocked {
            eve_character_id: 12345,
            reason: Some("botting".into()),
        };
        assert_eq!(event.event_type(), "eve_character_blocked");
        let d = event.details();
        assert_eq!(d["eve_character_id"], 12345i64);
        assert_eq!(d["reason"], "botting");
    }

    #[test]
    fn eve_character_blocked_serialises_correctly_without_reason() {
        let event = AuditEvent::EveCharacterBlocked {
            eve_character_id: 12345,
            reason: None,
        };
        assert!(event.details()["reason"].is_null());
    }

    #[test]
    fn eve_character_unblocked_serialises_correctly() {
        let event = AuditEvent::EveCharacterUnblocked {
            eve_character_id: 12345,
        };
        assert_eq!(event.event_type(), "eve_character_unblocked");
        assert_eq!(event.details()["eve_character_id"], 12345i64);
    }

    #[test]
    fn map_created_serialises_correctly() {
        let account_id = test_uuid();
        let map_id = other_uuid();
        let event = AuditEvent::MapCreated {
            account_id,
            map_id,
            name: "Wormhole Chain Alpha".into(),
        };
        assert_eq!(event.event_type(), "map_created");
        let d = event.details();
        assert_eq!(d["map_id"], map_id.to_string());
        assert_eq!(d["name"], "Wormhole Chain Alpha");
        assert!(d.get("account_id").is_none());
    }

    #[test]
    fn map_deleted_serialises_correctly() {
        let account_id = test_uuid();
        let map_id = other_uuid();
        let event = AuditEvent::MapDeleted {
            account_id,
            map_id,
            name: "Test Map".into(),
        };
        assert_eq!(event.event_type(), "map_deleted");
        let d = event.details();
        assert_eq!(d["map_id"], map_id.to_string());
        assert_eq!(d["name"], "Test Map");
    }

    #[test]
    fn acl_created_serialises_correctly() {
        let account_id = test_uuid();
        let acl_id = other_uuid();
        let event = AuditEvent::AclCreated {
            account_id,
            acl_id,
            name: "Corp ACL".into(),
        };
        assert_eq!(event.event_type(), "acl_created");
        let d = event.details();
        assert_eq!(d["acl_id"], acl_id.to_string());
        assert_eq!(d["name"], "Corp ACL");
    }

    #[test]
    fn acl_renamed_serialises_correctly() {
        let account_id = test_uuid();
        let acl_id = other_uuid();
        let event = AuditEvent::AclRenamed {
            account_id,
            acl_id,
            old_name: "Old".into(),
            new_name: "New".into(),
        };
        assert_eq!(event.event_type(), "acl_renamed");
        let d = event.details();
        assert_eq!(d["old_name"], "Old");
        assert_eq!(d["new_name"], "New");
    }

    #[test]
    fn acl_deleted_serialises_correctly() {
        let account_id = test_uuid();
        let acl_id = other_uuid();
        let event = AuditEvent::AclDeleted {
            account_id,
            acl_id,
            name: "Doomed".into(),
        };
        assert_eq!(event.event_type(), "acl_deleted");
        assert_eq!(event.details()["name"], "Doomed");
    }

    #[test]
    fn acl_member_added_serialises_correctly() {
        let account_id = test_uuid();
        let acl_id = other_uuid();
        let member_id = Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap();
        let event = AuditEvent::AclMemberAdded {
            account_id,
            acl_id,
            member_id,
            member_type: "character".into(),
            permission: "read".into(),
        };
        assert_eq!(event.event_type(), "acl_member_added");
        let d = event.details();
        assert_eq!(d["member_id"], member_id.to_string());
        assert_eq!(d["member_type"], "character");
        assert_eq!(d["permission"], "read");
    }

    #[test]
    fn acl_member_permission_changed_serialises_correctly() {
        let account_id = test_uuid();
        let acl_id = other_uuid();
        let member_id = Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap();
        let event = AuditEvent::AclMemberPermissionChanged {
            account_id,
            acl_id,
            member_id,
            permission: "read_write".into(),
        };
        assert_eq!(event.event_type(), "acl_member_permission_changed");
        assert_eq!(event.details()["permission"], "read_write");
    }

    #[test]
    fn acl_member_removed_serialises_correctly() {
        let account_id = test_uuid();
        let acl_id = other_uuid();
        let member_id = Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap();
        let event = AuditEvent::AclMemberRemoved {
            account_id,
            acl_id,
            member_id,
        };
        assert_eq!(event.event_type(), "acl_member_removed");
        assert_eq!(event.details()["member_id"], member_id.to_string());
    }

    #[test]
    fn acl_attached_to_map_serialises_correctly() {
        let account_id = test_uuid();
        let map_id = other_uuid();
        let acl_id = Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap();
        let event = AuditEvent::AclAttachedToMap {
            account_id,
            map_id,
            acl_id,
        };
        assert_eq!(event.event_type(), "acl_attached_to_map");
        let d = event.details();
        assert_eq!(d["map_id"], map_id.to_string());
        assert_eq!(d["acl_id"], acl_id.to_string());
    }

    #[test]
    fn acl_detached_from_map_serialises_correctly() {
        let account_id = test_uuid();
        let map_id = other_uuid();
        let acl_id = Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap();
        let event = AuditEvent::AclDetachedFromMap {
            account_id,
            map_id,
            acl_id,
        };
        assert_eq!(event.event_type(), "acl_detached_from_map");
        let d = event.details();
        assert_eq!(d["map_id"], map_id.to_string());
        assert_eq!(d["acl_id"], acl_id.to_string());
    }

    #[test]
    fn admin_map_ownership_changed_serialises_correctly() {
        let map_id = test_uuid();
        let old = other_uuid();
        let new = Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap();
        let event = AuditEvent::AdminMapOwnershipChanged {
            map_id,
            old_owner: old,
            new_owner: new,
        };
        assert_eq!(event.event_type(), "admin_map_ownership_changed");
        let d = event.details();
        assert_eq!(d["map_id"], map_id.to_string());
        assert_eq!(d["old_owner"], old.to_string());
        assert_eq!(d["new_owner"], new.to_string());
    }

    #[test]
    fn admin_map_hard_deleted_serialises_correctly() {
        let map_id = test_uuid();
        let event = AuditEvent::AdminMapHardDeleted {
            map_id,
            name: "Doomed Map".into(),
        };
        assert_eq!(event.event_type(), "admin_map_hard_deleted");
        assert_eq!(event.details()["name"], "Doomed Map");
    }

    #[test]
    fn admin_acl_ownership_changed_serialises_correctly() {
        let acl_id = test_uuid();
        let old = other_uuid();
        let new = Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap();
        let event = AuditEvent::AdminAclOwnershipChanged {
            acl_id,
            old_owner: old,
            new_owner: new,
        };
        assert_eq!(event.event_type(), "admin_acl_ownership_changed");
    }

    #[test]
    fn admin_acl_hard_deleted_serialises_correctly() {
        let acl_id = test_uuid();
        let event = AuditEvent::AdminAclHardDeleted {
            acl_id,
            name: "Doomed ACL".into(),
        };
        assert_eq!(event.event_type(), "admin_acl_hard_deleted");
        assert_eq!(event.details()["name"], "Doomed ACL");
    }

    #[test]
    fn server_admin_grant_source_serialises() {
        assert_eq!(
            ServerAdminGrantSource::FirstAccountBootstrap.as_str(),
            "first_account_bootstrap"
        );
        assert_eq!(ServerAdminGrantSource::AdminGrant.as_str(), "admin_grant");
    }

    // -----------------------------------------------------------------------
    // record_in_tx behaviour tests against the real DB.
    // -----------------------------------------------------------------------

    fn test_key() -> Vec<u8> {
        vec![0u8; 32]
    }

    async fn insert_account_with_main(pool: &PgPool, eve_id: i64, name: &str) -> Uuid {
        let account_id = accounts::create_account(pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let char_id = char_db::upsert_tokens(
            &mut tx,
            account_id,
            eve_id,
            name,
            1_000_001,
            "Corp One",
            None,
            None,
            "client1",
            "a",
            "r",
            Utc::now() + chrono::Duration::hours(1),
            &[],
            &test_key(),
        )
        .await
        .unwrap();
        char_db::promote_if_no_main(&mut tx, account_id, char_id)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        account_id
    }

    #[sqlx::test]
    async fn record_in_tx_with_account_actor_snapshots_main(pool: PgPool) {
        let account_id = insert_account_with_main(&pool, 7777, "Main Pilot").await;

        let mut tx = pool.begin().await.unwrap();
        record_in_tx(
            &mut tx,
            Some(account_id),
            None,
            AuditEvent::AccountDeletionRequested { account_id },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let row = sqlx::query!(
            "SELECT actor_account_id, actor_character_id, actor_character_name, event_type
             FROM audit_log WHERE event_type = 'account_deletion_requested'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.actor_account_id, Some(account_id));
        assert_eq!(row.actor_character_id, Some(7777));
        assert_eq!(row.actor_character_name.as_deref(), Some("Main Pilot"));
        assert_eq!(row.event_type, "account_deletion_requested");
    }

    #[sqlx::test]
    async fn record_in_tx_with_acting_as_writes_character_columns(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        record_in_tx(
            &mut tx,
            None,
            Some(ActingCharacter {
                eve_character_id: 99999,
                name: "Signing In".to_string(),
            }),
            AuditEvent::AccountRegistered {
                account_id: Uuid::new_v4(),
                eve_character_id: 99999,
                character_name: "Signing In".to_string(),
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let row = sqlx::query!(
            "SELECT actor_account_id, actor_character_id, actor_character_name
             FROM audit_log WHERE event_type = 'account_registered'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(row.actor_account_id.is_none());
        assert_eq!(row.actor_character_id, Some(99999));
        assert_eq!(row.actor_character_name.as_deref(), Some("Signing In"));
    }

    #[sqlx::test]
    async fn record_in_tx_system_event_writes_all_nulls(pool: PgPool) {
        let account_id = Uuid::new_v4();

        let mut tx = pool.begin().await.unwrap();
        record_in_tx(
            &mut tx,
            None,
            None,
            AuditEvent::AccountPurged { account_id },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let row = sqlx::query!(
            "SELECT actor_account_id, actor_character_id, actor_character_name
             FROM audit_log WHERE event_type = 'account_purged'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(row.actor_account_id.is_none());
        assert!(row.actor_character_id.is_none());
        assert!(row.actor_character_name.is_none());
    }

    #[sqlx::test]
    async fn record_in_tx_with_account_missing_main_fails_soft(pool: PgPool) {
        // Account exists but has no characters yet — invariant violation that
        // record_in_tx should handle by writing NULL character columns.
        let account_id = accounts::create_account(&pool).await.unwrap();

        let mut tx = pool.begin().await.unwrap();
        record_in_tx(
            &mut tx,
            Some(account_id),
            None,
            AuditEvent::AccountDeletionRequested { account_id },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let row = sqlx::query!(
            "SELECT actor_account_id, actor_character_id, actor_character_name
             FROM audit_log WHERE event_type = 'account_deletion_requested'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.actor_account_id, Some(account_id));
        assert!(row.actor_character_id.is_none());
        assert!(row.actor_character_name.is_none());
    }

    #[sqlx::test]
    async fn record_in_tx_with_account_missing_main_emits_tracing_error(pool: PgPool) {
        use std::io::Write;
        use std::sync::{Arc, Mutex};
        use tracing_subscriber::fmt::MakeWriter;

        // Shared buffer that the subscriber writes into.
        #[derive(Clone)]
        struct CapturingWriter(Arc<Mutex<Vec<u8>>>);
        impl Write for CapturingWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                #[allow(clippy::unwrap_used)]
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        impl<'a> MakeWriter<'a> for CapturingWriter {
            type Writer = CapturingWriter;
            fn make_writer(&'a self) -> Self::Writer {
                self.clone()
            }
        }

        let buf = Arc::new(Mutex::new(Vec::new()));
        let writer = CapturingWriter(buf.clone());
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::ERROR)
            .with_writer(writer)
            .with_ansi(false)
            .with_target(false)
            .finish();

        let account_id = accounts::create_account(&pool).await.unwrap();

        // `set_default` returns a guard that scopes the default subscriber for
        // the rest of this function (works across .await unlike `with_default`).
        let _guard = tracing::subscriber::set_default(subscriber);

        let mut tx = pool.begin().await.unwrap();
        record_in_tx(
            &mut tx,
            Some(account_id),
            None,
            AuditEvent::AccountDeletionRequested { account_id },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        #[allow(clippy::unwrap_used)]
        let captured = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            captured.contains("audit: account has no main at write time"),
            "expected tracing::error! about missing main, got: {captured}"
        );
        assert!(
            captured.contains(&account_id.to_string()),
            "expected captured log to include the account_id, got: {captured}"
        );
        assert!(
            captured.contains("account_deletion_requested"),
            "expected captured log to include the event_type, got: {captured}"
        );
    }

    #[sqlx::test]
    async fn record_in_tx_rolls_back_with_caller_tx(pool: PgPool) {
        let account_id = Uuid::new_v4();

        let mut tx = pool.begin().await.unwrap();
        record_in_tx(
            &mut tx,
            None,
            None,
            AuditEvent::AccountPurged { account_id },
        )
        .await
        .unwrap();
        tx.rollback().await.unwrap();

        let count = sqlx::query!("SELECT COUNT(*) AS \"count!\" FROM audit_log")
            .fetch_one(&pool)
            .await
            .unwrap()
            .count;
        assert_eq!(count, 0);
    }

    // -----------------------------------------------------------------------
    // list_audit_log behaviour tests against the real DB.
    // -----------------------------------------------------------------------

    async fn insert_audit_row(
        pool: &PgPool,
        actor: Option<Uuid>,
        event_type: &str,
        occurred_at: DateTime<Utc>,
    ) {
        sqlx::query!(
            "INSERT INTO audit_log (actor_account_id, event_type, details, occurred_at)
             VALUES ($1, $2, '{}'::jsonb, $3)",
            actor,
            event_type,
            occurred_at,
        )
        .execute(pool)
        .await
        .unwrap();
    }

    #[sqlx::test]
    async fn list_audit_log_no_filters_returns_newest_first(pool: PgPool) {
        let base = Utc::now();
        insert_audit_row(
            &pool,
            None,
            "account_purged",
            base - chrono::Duration::hours(3),
        )
        .await;
        insert_audit_row(
            &pool,
            None,
            "account_purged",
            base - chrono::Duration::hours(2),
        )
        .await;
        insert_audit_row(
            &pool,
            None,
            "account_purged",
            base - chrono::Duration::hours(1),
        )
        .await;

        let rows = list_audit_log(&pool, None, None, None, 10).await.unwrap();
        assert_eq!(rows.len(), 3);
        assert!(rows[0].occurred_at > rows[1].occurred_at);
        assert!(rows[1].occurred_at > rows[2].occurred_at);
    }

    #[sqlx::test]
    async fn list_audit_log_filter_by_event_type(pool: PgPool) {
        let t = Utc::now();
        insert_audit_row(&pool, None, "account_registered", t).await;
        insert_audit_row(&pool, None, "account_purged", t).await;
        insert_audit_row(&pool, None, "account_registered", t).await;

        let rows = list_audit_log(&pool, Some("account_registered"), None, None, 10)
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.event_type == "account_registered"));
    }

    #[sqlx::test]
    async fn list_audit_log_filter_by_actor_account_id(pool: PgPool) {
        let actor_a = accounts::create_account(&pool).await.unwrap();
        let actor_b = accounts::create_account(&pool).await.unwrap();
        let t = Utc::now();
        insert_audit_row(&pool, Some(actor_a), "account_deletion_requested", t).await;
        insert_audit_row(&pool, Some(actor_b), "account_deletion_requested", t).await;
        insert_audit_row(&pool, None, "account_purged", t).await;

        let rows = list_audit_log(&pool, None, Some(actor_a), None, 10)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].actor_account_id, Some(actor_a));
    }

    #[sqlx::test]
    async fn list_audit_log_before_cursor(pool: PgPool) {
        let base = Utc::now();
        let t1 = base - chrono::Duration::hours(3);
        let t2 = base - chrono::Duration::hours(2);
        let t3 = base - chrono::Duration::hours(1);
        insert_audit_row(&pool, None, "account_purged", t1).await;
        insert_audit_row(&pool, None, "account_purged", t2).await;
        insert_audit_row(&pool, None, "account_purged", t3).await;

        let rows = list_audit_log(&pool, None, None, Some(t3), 10)
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        // Newest-first: t2 then t1.
        assert!(rows[0].occurred_at < t3);
        assert!(rows[0].occurred_at > rows[1].occurred_at);
    }
}
