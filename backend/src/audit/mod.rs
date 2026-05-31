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

/// How an `AuditTarget`'s `target_name` column is sourced. Lets `record_in_tx`
/// know whether a write-time lookup is required, mirroring how the actor's
/// character name is snapshotted.
#[derive(Debug, Clone)]
pub enum AuditTargetName {
    /// The event already carries the target's name; write it directly.
    Known(String),
    /// The target is an account, whose name SHALL be resolved at write time as
    /// the account's main character name (snapshot, fail-soft). Carries the
    /// target account id to look up.
    AccountMain(Uuid),
    /// No name is available; `target_name` is left NULL.
    None,
}

/// The entity an `AuditEvent` acted upon. Promoted to first-class, indexed
/// `audit_log` columns so the dominant admin query ("who did X to whom") is a
/// clean filter rather than a JSONB scan. `target_id` is stringified so a
/// single column holds both EVE character BIGINTs and account/map/acl UUIDs,
/// discriminated by `target_type`.
#[derive(Debug, Clone)]
pub struct AuditTarget {
    pub target_type: &'static str,
    pub target_id: String,
    pub name: AuditTargetName,
}

impl AuditTarget {
    fn character(eve_character_id: i64, name: Option<&str>) -> Self {
        Self {
            target_type: "character",
            target_id: eve_character_id.to_string(),
            name: match name {
                Some(n) => AuditTargetName::Known(n.to_string()),
                None => AuditTargetName::None,
            },
        }
    }

    fn account(account_id: Uuid) -> Self {
        Self {
            target_type: "account",
            target_id: account_id.to_string(),
            name: AuditTargetName::AccountMain(account_id),
        }
    }

    fn map(map_id: Uuid, name: Option<&str>) -> Self {
        Self {
            target_type: "map",
            target_id: map_id.to_string(),
            name: match name {
                Some(n) => AuditTargetName::Known(n.to_string()),
                None => AuditTargetName::None,
            },
        }
    }

    fn acl(acl_id: Uuid, name: Option<&str>) -> Self {
        Self {
            target_type: "acl",
            target_id: acl_id.to_string(),
            name: match name {
                Some(n) => AuditTargetName::Known(n.to_string()),
                None => AuditTargetName::None,
            },
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
    /// Emitted by the SSO callback when a blocked character is refused. Unlike
    /// every other variant this records a rejected *attempt* rather than a
    /// committed state change — a deliberate, narrow extension for a
    /// security-relevant event. `actor_account_id` is NULL (no account is
    /// authenticated); the rejected character is the subject, carried in
    /// `details` / the target columns.
    BlockedLoginRejected {
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
            Self::BlockedLoginRejected { .. } => "blocked_login_rejected",
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
            // actor is NULL (no session) — the rejected character is the subject,
            // carried here so the row is self-contained.
            Self::BlockedLoginRejected { eve_character_id } => json!({
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

    /// The entity this action targeted, used to populate the `target_*`
    /// columns. Every variant in the current catalogue has a target; the
    /// `Option` return permits a future target-less variant without a schema
    /// change. The `match` has no wildcard arm, so a new variant fails to
    /// compile until its target is declared.
    pub fn target(&self) -> Option<AuditTarget> {
        Some(match self {
            // Account targets — name resolved at write time from the account's main.
            Self::AccountRegistered { account_id, .. }
            | Self::AccountDeletionRequested { account_id }
            | Self::AccountReactivated { account_id }
            | Self::AccountPurged { account_id }
            | Self::ApiKeyCreated { account_id, .. }
            | Self::ApiKeyRevoked { account_id, .. }
            | Self::ServerAdminGranted { account_id, .. }
            | Self::ServerAdminRevoked { account_id } => AuditTarget::account(*account_id),

            // Character targets carrying a name.
            Self::OrphanCharacterClaimed {
                eve_character_id,
                character_name,
                ..
            }
            | Self::CharacterAdded {
                eve_character_id,
                character_name,
                ..
            } => AuditTarget::character(*eve_character_id, Some(character_name)),

            // Character targets with no carried name.
            Self::CharacterRemoved {
                eve_character_id, ..
            }
            | Self::CharacterSetMain {
                eve_character_id, ..
            }
            | Self::EveCharacterBlocked {
                eve_character_id, ..
            }
            | Self::EveCharacterUnblocked { eve_character_id }
            | Self::BlockedLoginRejected { eve_character_id } => {
                AuditTarget::character(*eve_character_id, None)
            }

            // Map targets carrying a name.
            Self::MapCreated { map_id, name, .. }
            | Self::MapDeleted { map_id, name, .. }
            | Self::AdminMapHardDeleted { map_id, name } => AuditTarget::map(*map_id, Some(name)),

            // Map target with no carried name.
            Self::AdminMapOwnershipChanged { map_id, .. } => AuditTarget::map(*map_id, None),

            // ACL targets carrying a name.
            Self::AclCreated { acl_id, name, .. }
            | Self::AclDeleted { acl_id, name, .. }
            | Self::AdminAclHardDeleted { acl_id, name } => AuditTarget::acl(*acl_id, Some(name)),

            // ACL rename targets the acl; its current name is the new name.
            Self::AclRenamed {
                acl_id, new_name, ..
            } => AuditTarget::acl(*acl_id, Some(new_name)),

            // ACL targets with no carried name.
            Self::AclMemberAdded { acl_id, .. }
            | Self::AclMemberPermissionChanged { acl_id, .. }
            | Self::AclMemberRemoved { acl_id, .. }
            | Self::AclAttachedToMap { acl_id, .. }
            | Self::AclDetachedFromMap { acl_id, .. }
            | Self::AdminAclOwnershipChanged { acl_id, .. } => AuditTarget::acl(*acl_id, None),
        })
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
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub target_name: Option<String>,
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
///
/// Target-column resolution (from `event.target()`):
/// - `None` → all three target columns NULL.
/// - `Some(t)` → `target_type`/`target_id` from `t`; `target_name` from `t.name`:
///   a `Known` name is written as-is; an `AccountMain` name triggers a main
///   lookup of the *target* account (reusing the actor lookup when the actor
///   account is the target account) and snapshots its name, fail-soft on miss
///   (`tracing::error!` + NULL); `None` leaves `target_name` NULL.
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

    let (target_type, target_id, target_name) = match event.target() {
        None => (None, None, None),
        Some(t) => {
            let name = match t.name {
                AuditTargetName::Known(n) => Some(n),
                AuditTargetName::None => None,
                AuditTargetName::AccountMain(target_account_id) => {
                    // Reuse the actor snapshot when the actor is the target,
                    // avoiding a redundant lookup for self-targeting events
                    // (deletion request, key create/revoke on one's own account).
                    if actor_account_id == Some(target_account_id) {
                        actor_character_name.clone()
                    } else {
                        match characters::get_main_for_account_tx(tx, target_account_id).await? {
                            Some((_, name)) => Some(name),
                            None => {
                                tracing::error!(
                                    target_account_id = %target_account_id,
                                    event_type,
                                    "audit: target account has no main at write time — target_name left NULL"
                                );
                                None
                            }
                        }
                    }
                }
            };
            (Some(t.target_type), Some(t.target_id), name)
        }
    };

    sqlx::query!(
        r#"
        INSERT INTO audit_log (
            actor_account_id,
            actor_character_id,
            actor_character_name,
            event_type,
            details,
            target_type,
            target_id,
            target_name
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
        actor_account_id,
        actor_character_id,
        actor_character_name,
        event_type,
        details,
        target_type,
        target_id,
        target_name,
    )
    .execute(&mut **tx)
    .await
    .context("failed to insert audit log entry")?;

    Ok(())
}

/// Reads audit-log entries newest-first, with optional filters and a hard
/// `limit`. Used by the admin audit-browser; `limit` is the caller's
/// responsibility to clamp.
///
/// Filter axes (all conjunctive when supplied): `event_type`,
/// `actor_account_id`, `target_type`, `target_id`, `target_name`
/// (case-insensitive *substring* match via `ILIKE '%fragment%'`, LIKE
/// metacharacters in the fragment escaped to match literally), and the
/// `before` keyset cursor.
///
/// All filters bind as parameters — no string interpolation, no SQL injection
/// surface.
#[allow(clippy::too_many_arguments)]
pub async fn list_audit_log(
    pool: &PgPool,
    event_type: Option<&str>,
    actor_account_id: Option<Uuid>,
    target_type: Option<&str>,
    target_id: Option<&str>,
    target_name: Option<&str>,
    before: Option<DateTime<Utc>>,
    limit: i64,
) -> Result<Vec<AuditLogEntry>> {
    // `target_name` is a case-insensitive *substring* search (the admin types a
    // fragment, e.g. "wasp" to find "Wasp 223"). LIKE metacharacters in the
    // fragment are escaped so they match literally, then the value is wrapped in
    // `%…%`. A leading-wildcard ILIKE cannot use the `LOWER(target_name)` index,
    // so this is a scan — acceptable at the audit log's scale.
    let target_name_pattern = target_name.map(|fragment| {
        let escaped = fragment
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        format!("%{escaped}%")
    });

    let rows = sqlx::query!(
        r#"
        SELECT id, occurred_at, actor_account_id, actor_character_id,
               actor_character_name, event_type, details,
               target_type, target_id, target_name
        FROM audit_log
        WHERE ($1::TEXT IS NULL        OR event_type       = $1)
          AND ($2::UUID IS NULL        OR actor_account_id = $2)
          AND ($3::TEXT IS NULL        OR target_type      = $3)
          AND ($4::TEXT IS NULL        OR target_id        = $4)
          AND ($5::TEXT IS NULL        OR target_name ILIKE $5 ESCAPE '\')
          AND ($6::TIMESTAMPTZ IS NULL OR occurred_at      < $6)
        ORDER BY occurred_at DESC
        LIMIT $7
        "#,
        event_type,
        actor_account_id,
        target_type,
        target_id,
        target_name_pattern,
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
            target_type: r.target_type,
            target_id: r.target_id,
            target_name: r.target_name,
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
    fn blocked_login_rejected_serialises_correctly() {
        let event = AuditEvent::BlockedLoginRejected {
            eve_character_id: 98765,
        };
        assert_eq!(event.event_type(), "blocked_login_rejected");
        let d = event.details();
        assert_eq!(d["eve_character_id"], 98765i64);
        // The subject character is the only payload — actor is NULL at emit time.
        assert_eq!(d.as_object().unwrap().len(), 1);
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
    // target() shape tests — one assertion per variant. `target_type` and
    // `target_id` are checked exactly; the name disposition is checked by kind
    // (carried value / account-lookup marker / none).
    // -----------------------------------------------------------------------

    /// Asserts a variant's target type, id, and that its name is a known
    /// (carried) string equal to `name`.
    fn assert_named_target(event: &AuditEvent, ty: &str, id: &str, name: &str) {
        let t = event.target().expect("variant should have a target");
        assert_eq!(t.target_type, ty, "target_type for {}", event.event_type());
        assert_eq!(t.target_id, id, "target_id for {}", event.event_type());
        match t.name {
            AuditTargetName::Known(n) => assert_eq!(n, name),
            other => panic!(
                "expected Known name, got {other:?} for {}",
                event.event_type()
            ),
        }
    }

    /// Asserts a variant's target type and id, and that its name is sourced by
    /// a write-time lookup of the given account's main.
    fn assert_account_target(event: &AuditEvent, id: Uuid) {
        let t = event.target().expect("variant should have a target");
        assert_eq!(t.target_type, "account");
        assert_eq!(t.target_id, id.to_string());
        match t.name {
            AuditTargetName::AccountMain(acc) => assert_eq!(acc, id),
            other => panic!(
                "expected AccountMain name, got {other:?} for {}",
                event.event_type()
            ),
        }
    }

    /// Asserts a variant's target type and id, and that it carries no name.
    fn assert_nameless_target(event: &AuditEvent, ty: &str, id: &str) {
        let t = event.target().expect("variant should have a target");
        assert_eq!(t.target_type, ty, "target_type for {}", event.event_type());
        assert_eq!(t.target_id, id, "target_id for {}", event.event_type());
        match t.name {
            AuditTargetName::None => {}
            other => panic!(
                "expected None name, got {other:?} for {}",
                event.event_type()
            ),
        }
    }

    #[test]
    fn target_account_events() {
        let id = test_uuid();
        assert_account_target(
            &AuditEvent::AccountRegistered {
                account_id: id,
                eve_character_id: 1,
                character_name: "X".into(),
            },
            id,
        );
        assert_account_target(&AuditEvent::AccountDeletionRequested { account_id: id }, id);
        assert_account_target(&AuditEvent::AccountReactivated { account_id: id }, id);
        assert_account_target(&AuditEvent::AccountPurged { account_id: id }, id);
        assert_account_target(
            &AuditEvent::ApiKeyCreated {
                account_id: id,
                key_id: other_uuid(),
                name: "k".into(),
            },
            id,
        );
        assert_account_target(
            &AuditEvent::ApiKeyRevoked {
                account_id: id,
                key_id: other_uuid(),
            },
            id,
        );
        assert_account_target(
            &AuditEvent::ServerAdminGranted {
                account_id: id,
                source: ServerAdminGrantSource::AdminGrant,
            },
            id,
        );
        assert_account_target(&AuditEvent::ServerAdminRevoked { account_id: id }, id);
    }

    #[test]
    fn target_named_character_events() {
        let id = test_uuid();
        assert_named_target(
            &AuditEvent::CharacterAdded {
                account_id: id,
                eve_character_id: 555,
                character_name: "Alt".into(),
            },
            "character",
            "555",
            "Alt",
        );
        assert_named_target(
            &AuditEvent::OrphanCharacterClaimed {
                account_id: id,
                eve_character_id: 7,
                character_name: "Orphan".into(),
            },
            "character",
            "7",
            "Orphan",
        );
    }

    #[test]
    fn target_nameless_character_events() {
        let id = test_uuid();
        assert_nameless_target(
            &AuditEvent::CharacterRemoved {
                account_id: id,
                eve_character_id: 42,
            },
            "character",
            "42",
        );
        assert_nameless_target(
            &AuditEvent::CharacterSetMain {
                account_id: id,
                eve_character_id: 99,
            },
            "character",
            "99",
        );
        assert_nameless_target(
            &AuditEvent::EveCharacterBlocked {
                eve_character_id: 12345,
                reason: None,
            },
            "character",
            "12345",
        );
        assert_nameless_target(
            &AuditEvent::EveCharacterUnblocked {
                eve_character_id: 12345,
            },
            "character",
            "12345",
        );
        assert_nameless_target(
            &AuditEvent::BlockedLoginRejected {
                eve_character_id: 98765,
            },
            "character",
            "98765",
        );
    }

    #[test]
    fn target_map_events() {
        let account_id = test_uuid();
        let map_id = other_uuid();
        assert_named_target(
            &AuditEvent::MapCreated {
                account_id,
                map_id,
                name: "Chain".into(),
            },
            "map",
            &map_id.to_string(),
            "Chain",
        );
        assert_named_target(
            &AuditEvent::MapDeleted {
                account_id,
                map_id,
                name: "Chain".into(),
            },
            "map",
            &map_id.to_string(),
            "Chain",
        );
        assert_named_target(
            &AuditEvent::AdminMapHardDeleted {
                map_id,
                name: "Chain".into(),
            },
            "map",
            &map_id.to_string(),
            "Chain",
        );
        assert_nameless_target(
            &AuditEvent::AdminMapOwnershipChanged {
                map_id,
                old_owner: account_id,
                new_owner: other_uuid(),
            },
            "map",
            &map_id.to_string(),
        );
    }

    #[test]
    fn target_acl_events() {
        let account_id = test_uuid();
        let acl_id = other_uuid();
        let member_id = Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap();
        assert_named_target(
            &AuditEvent::AclCreated {
                account_id,
                acl_id,
                name: "Corp".into(),
            },
            "acl",
            &acl_id.to_string(),
            "Corp",
        );
        assert_named_target(
            &AuditEvent::AclDeleted {
                account_id,
                acl_id,
                name: "Corp".into(),
            },
            "acl",
            &acl_id.to_string(),
            "Corp",
        );
        assert_named_target(
            &AuditEvent::AdminAclHardDeleted {
                acl_id,
                name: "Corp".into(),
            },
            "acl",
            &acl_id.to_string(),
            "Corp",
        );
        // Rename targets the acl with its *new* name.
        assert_named_target(
            &AuditEvent::AclRenamed {
                account_id,
                acl_id,
                old_name: "Old".into(),
                new_name: "New".into(),
            },
            "acl",
            &acl_id.to_string(),
            "New",
        );
        assert_nameless_target(
            &AuditEvent::AclMemberAdded {
                account_id,
                acl_id,
                member_id,
                member_type: "character".into(),
                permission: "read".into(),
            },
            "acl",
            &acl_id.to_string(),
        );
        assert_nameless_target(
            &AuditEvent::AclMemberPermissionChanged {
                account_id,
                acl_id,
                member_id,
                permission: "read_write".into(),
            },
            "acl",
            &acl_id.to_string(),
        );
        assert_nameless_target(
            &AuditEvent::AclMemberRemoved {
                account_id,
                acl_id,
                member_id,
            },
            "acl",
            &acl_id.to_string(),
        );
        assert_nameless_target(
            &AuditEvent::AclAttachedToMap {
                account_id,
                map_id: other_uuid(),
                acl_id,
            },
            "acl",
            &acl_id.to_string(),
        );
        assert_nameless_target(
            &AuditEvent::AclDetachedFromMap {
                account_id,
                map_id: other_uuid(),
                acl_id,
            },
            "acl",
            &acl_id.to_string(),
        );
        assert_nameless_target(
            &AuditEvent::AdminAclOwnershipChanged {
                acl_id,
                old_owner: account_id,
                new_owner: member_id,
            },
            "acl",
            &acl_id.to_string(),
        );
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

        let rows = list_audit_log(&pool, None, None, None, None, None, None, 10)
            .await
            .unwrap();
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

        let rows = list_audit_log(
            &pool,
            Some("account_registered"),
            None,
            None,
            None,
            None,
            None,
            10,
        )
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

        let rows = list_audit_log(&pool, None, Some(actor_a), None, None, None, None, 10)
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

        let rows = list_audit_log(&pool, None, None, None, None, None, Some(t3), 10)
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        // Newest-first: t2 then t1.
        assert!(rows[0].occurred_at < t3);
        assert!(rows[0].occurred_at > rows[1].occurred_at);
    }

    // -----------------------------------------------------------------------
    // record_in_tx target-column behaviour against the real DB.
    // -----------------------------------------------------------------------

    #[sqlx::test]
    async fn record_in_tx_writes_character_target(pool: PgPool) {
        let account_id = insert_account_with_main(&pool, 7777, "Main Pilot").await;

        let mut tx = pool.begin().await.unwrap();
        record_in_tx(
            &mut tx,
            Some(account_id),
            None,
            AuditEvent::CharacterAdded {
                account_id,
                eve_character_id: 555,
                character_name: "Alt Pilot".into(),
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let row = sqlx::query!(
            "SELECT target_type, target_id, target_name
             FROM audit_log WHERE event_type = 'character_added'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.target_type.as_deref(), Some("character"));
        assert_eq!(row.target_id.as_deref(), Some("555"));
        assert_eq!(row.target_name.as_deref(), Some("Alt Pilot"));
    }

    #[sqlx::test]
    async fn record_in_tx_snapshots_target_account_main_not_actor(pool: PgPool) {
        // Actor and target are different accounts; target_name must come from
        // the TARGET account's main, independent of the actor's main.
        let actor_id = insert_account_with_main(&pool, 1111, "Admin Actor").await;
        let target_id = insert_account_with_main(&pool, 2222, "Boss Pilot").await;

        let mut tx = pool.begin().await.unwrap();
        record_in_tx(
            &mut tx,
            Some(actor_id),
            None,
            AuditEvent::ServerAdminGranted {
                account_id: target_id,
                source: ServerAdminGrantSource::AdminGrant,
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let row = sqlx::query!(
            "SELECT actor_character_name, target_type, target_id, target_name
             FROM audit_log WHERE event_type = 'server_admin_granted'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.actor_character_name.as_deref(), Some("Admin Actor"));
        assert_eq!(row.target_type.as_deref(), Some("account"));
        assert_eq!(row.target_id.as_deref(), Some(&*target_id.to_string()));
        assert_eq!(row.target_name.as_deref(), Some("Boss Pilot"));
    }

    #[sqlx::test]
    async fn record_in_tx_target_account_missing_main_falls_soft(pool: PgPool) {
        use std::io::Write;
        use std::sync::{Arc, Mutex};
        use tracing_subscriber::fmt::MakeWriter;

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

        // Actor has a main; the TARGET account has none.
        let actor_id = insert_account_with_main(&pool, 1111, "Admin Actor").await;
        let target_id = accounts::create_account(&pool).await.unwrap();

        let buf = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::ERROR)
            .with_writer(CapturingWriter(buf.clone()))
            .with_ansi(false)
            .with_target(false)
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let mut tx = pool.begin().await.unwrap();
        record_in_tx(
            &mut tx,
            Some(actor_id),
            None,
            AuditEvent::ServerAdminGranted {
                account_id: target_id,
                source: ServerAdminGrantSource::AdminGrant,
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let row = sqlx::query!(
            "SELECT target_type, target_id, target_name
             FROM audit_log WHERE event_type = 'server_admin_granted'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        // Row still inserted; target id present, name NULL.
        assert_eq!(row.target_type.as_deref(), Some("account"));
        assert_eq!(row.target_id.as_deref(), Some(&*target_id.to_string()));
        assert!(row.target_name.is_none());

        #[allow(clippy::unwrap_used)]
        let captured = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            captured.contains("audit: target account has no main at write time"),
            "expected tracing::error! about missing target main, got: {captured}"
        );
        assert!(
            captured.contains(&target_id.to_string()),
            "expected captured log to include the target account_id, got: {captured}"
        );
    }

    #[sqlx::test]
    async fn record_in_tx_nameless_character_target(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        record_in_tx(
            &mut tx,
            None,
            None,
            AuditEvent::EveCharacterBlocked {
                eve_character_id: 314159,
                reason: Some("botting".into()),
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let row = sqlx::query!(
            "SELECT target_type, target_id, target_name
             FROM audit_log WHERE event_type = 'eve_character_blocked'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.target_type.as_deref(), Some("character"));
        assert_eq!(row.target_id.as_deref(), Some("314159"));
        assert!(row.target_name.is_none());
    }

    // -----------------------------------------------------------------------
    // list_audit_log target-filter behaviour against the real DB.
    // -----------------------------------------------------------------------

    /// Inserts a row carrying target columns directly (the list filters key off
    /// the columns, not the write path).
    async fn insert_targeted_row(
        pool: &PgPool,
        event_type: &str,
        target_type: &str,
        target_id: &str,
        target_name: Option<&str>,
    ) {
        sqlx::query!(
            "INSERT INTO audit_log (event_type, details, target_type, target_id, target_name)
             VALUES ($1, '{}'::jsonb, $2, $3, $4)",
            event_type,
            target_type,
            target_id,
            target_name,
        )
        .execute(pool)
        .await
        .unwrap();
    }

    #[sqlx::test]
    async fn list_audit_log_filter_by_target_id(pool: PgPool) {
        insert_targeted_row(&pool, "character_added", "character", "555", Some("Alt")).await;
        insert_targeted_row(&pool, "character_added", "character", "999", Some("Other")).await;
        insert_targeted_row(&pool, "map_created", "map", "555", Some("Coincidental")).await;

        let rows = list_audit_log(
            &pool,
            None,
            None,
            Some("character"),
            Some("555"),
            None,
            None,
            10,
        )
        .await
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].target_type.as_deref(), Some("character"));
        assert_eq!(rows[0].target_id.as_deref(), Some("555"));
    }

    #[sqlx::test]
    async fn list_audit_log_filter_by_target_name_is_case_insensitive_substring(pool: PgPool) {
        insert_targeted_row(
            &pool,
            "server_admin_granted",
            "account",
            &Uuid::new_v4().to_string(),
            Some("Wasp 223"),
        )
        .await;
        insert_targeted_row(
            &pool,
            "server_admin_granted",
            "account",
            &Uuid::new_v4().to_string(),
            Some("Other Pilot"),
        )
        .await;

        // A lowercase fragment ("wasp") matches "Wasp 223" — case-insensitive
        // substring, not exact match.
        let rows = list_audit_log(&pool, None, None, None, None, Some("wasp"), None, 10)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].target_name.as_deref(), Some("Wasp 223"));

        // A non-matching fragment finds nothing.
        let none = list_audit_log(&pool, None, None, None, None, Some("frigate"), None, 10)
            .await
            .unwrap();
        assert!(none.is_empty());
    }

    #[sqlx::test]
    async fn list_audit_log_target_name_like_metacharacters_are_literal(pool: PgPool) {
        insert_targeted_row(
            &pool,
            "server_admin_granted",
            "account",
            &Uuid::new_v4().to_string(),
            Some("Wasp 223"),
        )
        .await;

        // "%" must be treated literally, not as a LIKE wildcard — so it does NOT
        // match "Wasp 223".
        let rows = list_audit_log(&pool, None, None, None, None, Some("%"), None, 10)
            .await
            .unwrap();
        assert!(rows.is_empty());
    }

    #[sqlx::test]
    async fn list_audit_log_target_filters_combine_with_event_type(pool: PgPool) {
        let uuid = Uuid::new_v4().to_string();
        insert_targeted_row(
            &pool,
            "server_admin_granted",
            "account",
            &uuid,
            Some("Boss Pilot"),
        )
        .await;
        // Same target name, different event_type — excluded by the event_type filter.
        insert_targeted_row(
            &pool,
            "server_admin_revoked",
            "account",
            &uuid,
            Some("Boss Pilot"),
        )
        .await;

        let rows = list_audit_log(
            &pool,
            Some("server_admin_granted"),
            None,
            None,
            None,
            Some("boss pilot"),
            None,
            10,
        )
        .await
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].event_type, "server_admin_granted");
    }
}
