-- Promote the audit target to first-class, indexed columns. The actor side is
-- already first-class (actor_account_id + snapshotted character); the target
-- was buried in `details` JSONB. The admin audit browser queries target-first
-- ("who did X to whom"), so these columns make that the indexed axis.
--
-- All three are nullable: not every recordable action has a distinct target,
-- and they are snapshots (no FK) intended to outlive the referenced rows.
ALTER TABLE audit_log
    ADD COLUMN target_type TEXT,
    ADD COLUMN target_id   TEXT,
    ADD COLUMN target_name TEXT;

-- "All events against this entity" — partial because target-less rows do not
-- participate. Leads with target_type so (type, id) is a covering prefix.
CREATE INDEX audit_log_target_id_idx ON audit_log (target_type, target_id)
    WHERE target_id IS NOT NULL;

-- Btree on LOWER(target_name) for target-name lookups. NOTE: list_audit_log now
-- does a case-insensitive *substring* match (`target_name ILIKE '%fragment%'`),
-- which a leading wildcard prevents this index from accelerating; the index is
-- retained because it still backs equality and left-anchored prefix probes
-- (`LOWER(target_name) = LOWER($1)` / `LIKE 'fragment%'`) for any future caller.
CREATE INDEX audit_log_target_name_idx ON audit_log (LOWER(target_name))
    WHERE target_name IS NOT NULL;

-- Backfill existing rows from `details` + `event_type`. This is a one-shot
-- migration-time exception to the table's INSERT-only invariant; going forward
-- record_in_tx populates these columns at insert.

-- Character-targeted events: target_id is the EVE character id carried in details.
UPDATE audit_log SET
    target_type = 'character',
    target_id   = details ->> 'eve_character_id'
WHERE details ? 'eve_character_id'
  AND event_type IN (
    'orphan_character_claimed', 'character_added', 'character_removed',
    'character_set_main', 'eve_character_blocked', 'eve_character_unblocked'
  );

-- Account-targeted events: the affected account is either carried in details
-- (when the actor is NULL or differs) or is the actor itself.
UPDATE audit_log SET
    target_type = 'account',
    target_id   = COALESCE(details ->> 'account_id', actor_account_id::text)
WHERE event_type IN (
    'account_registered', 'account_reactivated', 'account_purged',
    'account_deletion_requested', 'api_key_created', 'api_key_revoked',
    'server_admin_granted', 'server_admin_revoked'
  )
  AND (details ? 'account_id' OR actor_account_id IS NOT NULL);

-- Map-targeted events.
UPDATE audit_log SET
    target_type = 'map',
    target_id   = details ->> 'map_id'
WHERE details ? 'map_id'
  AND event_type IN ('map_created', 'map_deleted', 'admin_map_hard_deleted', 'admin_map_ownership_changed');

-- ACL-targeted events.
UPDATE audit_log SET
    target_type = 'acl',
    target_id   = details ->> 'acl_id'
WHERE details ? 'acl_id'
  AND event_type IN (
    'acl_created', 'acl_renamed', 'acl_deleted', 'admin_acl_hard_deleted',
    'admin_acl_ownership_changed', 'acl_member_added',
    'acl_member_permission_changed', 'acl_member_removed',
    'acl_attached_to_map', 'acl_detached_from_map'
  );

-- Best-effort target_name backfill. Character targets: the name carried in
-- details (where present). Account targets: the target account's current main
-- character name. Where neither is recoverable, target_name stays NULL.
UPDATE audit_log SET target_name = details ->> 'character_name'
WHERE target_type = 'character'
  AND details ? 'character_name'
  AND target_name IS NULL;

UPDATE audit_log a SET target_name = c.name
FROM eve_character c
WHERE a.target_type = 'account'
  AND a.target_name IS NULL
  AND c.account_id = a.target_id::uuid
  AND c.is_main = TRUE;

UPDATE audit_log SET target_name = details ->> 'name'
WHERE target_type IN ('map', 'acl')
  AND details ? 'name'
  AND target_name IS NULL;

UPDATE audit_log SET target_name = details ->> 'new_name'
WHERE event_type = 'acl_renamed'
  AND details ? 'new_name'
  AND target_name IS NULL;
