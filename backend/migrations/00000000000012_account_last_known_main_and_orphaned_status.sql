-- Denormalized last-known-main identity snapshot + the terminal 'orphaned'
-- status (detach-transferred-character-on-bind).
--
-- An ERB account's human-readable identity is derived entirely from its
-- characters ("which account is this" = the account owning the is_main = TRUE
-- character). An account that loses all its characters — e.g. when its only
-- character is detected as transferred to a different EVE account and detached —
-- would become unnameable and unfindable. These columns snapshot the main so an
-- emptied (orphaned) account stays nameable in admin views.
--
-- `last_known_main_character_id` holds the main's BIGINT eve_character_id; it is
-- a DISPLAY/IDENTITY SNAPSHOT, deliberately NOT a foreign key and NOT a join key
-- — it must survive the referenced eve_character row being detached or removed.
-- The single source of truth for the live main remains the is_main = TRUE flag.
-- A null snapshot means only "no main has ever been observed".
ALTER TABLE account
    ADD COLUMN last_known_main_character_id   BIGINT,
    ADD COLUMN last_known_main_character_name TEXT;

-- Terminal 'orphaned' status, distinct from 'soft_deleted':
--   'active'       — normal
--   'soft_deleted' — owner-recoverable by logging back in
--   'orphaned'     — unreachable: zero characters, so no SSO login can ever
--                    resolve to it again (never reactivated by the login
--                    self-heal path). The row is kept, never auto-deleted.
ALTER TABLE account
    ADD CONSTRAINT account_status_check
        CHECK (status IN ('active', 'soft_deleted', 'orphaned'));

-- One-time backfill: seed the snapshot for existing accounts from their current
-- main, so accounts that already exist are immediately nameable. New status
-- 'orphaned' needs no backfill — it is only reached by the new detach path going
-- forward.
UPDATE account a
SET last_known_main_character_id = c.eve_character_id,
    last_known_main_character_name = c.name
FROM eve_character c
WHERE c.account_id = a.id
  AND c.is_main = TRUE;
