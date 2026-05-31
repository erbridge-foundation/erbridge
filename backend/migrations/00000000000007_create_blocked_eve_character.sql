-- The character block list. Keyed on the immutable EVE character id, this table
-- is a self-contained SNAPSHOT: it deliberately has **no foreign key** to
-- `eve_character`, so an admin can pre-emptively block a griefer who has never
-- signed in (no eve_character row exists), and so the admin block list reads
-- flat without joining `eve_character`.
--
-- `character_name` / `corporation_name` are populated best-effort from ESI
-- public-info at block time and may be NULL when ESI is unavailable; enforcement
-- keys on `eve_character_id`, never the name. CCP does not permit
-- player-initiated renames and the id is immutable, so the snapshot is
-- effectively permanent-correct.
--
-- An account is "blocked" iff it owns at least one eve_character whose id is in
-- this table (a derived state — there is no per-account blocked flag).
CREATE TABLE blocked_eve_character (
    eve_character_id BIGINT PRIMARY KEY,
    character_name   TEXT,
    corporation_name TEXT,
    reason           TEXT,
    -- The admin who placed the block. SET NULL (not CASCADE) so unblocking
    -- history survives the blocking admin's account deletion.
    blocked_by       UUID REFERENCES account(id) ON DELETE SET NULL,
    blocked_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
