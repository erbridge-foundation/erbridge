-- ACL member identity uniqueness.
--
-- An ACL must not contain two members with the same identity: at most one
-- `character` member per character_id, and at most one corporation/alliance
-- member per (member_type, eve_entity_id). The constraint was assumed by the
-- service layer (a unique-violation mapping) but never created, so duplicates
-- were insertable.

-- Dedupe pre-existing duplicates first, keeping the oldest row of each set, so
-- the unique indexes below cannot fail on dirty data. RAISE NOTICE logs how many
-- rows were removed for operator visibility.
DO $$
DECLARE
    removed_chars  INTEGER;
    removed_ents   INTEGER;
BEGIN
    -- Character members: dedupe on (acl_id, character_id).
    WITH ranked AS (
        SELECT id,
               row_number() OVER (
                   PARTITION BY acl_id, character_id
                   ORDER BY created_at ASC, id ASC
               ) AS rn
        FROM acl_member
        WHERE member_type = 'character'
    )
    DELETE FROM acl_member
    USING ranked
    WHERE acl_member.id = ranked.id AND ranked.rn > 1;
    GET DIAGNOSTICS removed_chars = ROW_COUNT;

    -- Corporation/alliance members: dedupe on (acl_id, member_type, eve_entity_id).
    WITH ranked AS (
        SELECT id,
               row_number() OVER (
                   PARTITION BY acl_id, member_type, eve_entity_id
                   ORDER BY created_at ASC, id ASC
               ) AS rn
        FROM acl_member
        WHERE member_type <> 'character'
    )
    DELETE FROM acl_member
    USING ranked
    WHERE acl_member.id = ranked.id AND ranked.rn > 1;
    GET DIAGNOSTICS removed_ents = ROW_COUNT;

    RAISE NOTICE 'acl_member dedupe: removed % duplicate character members, % duplicate entity members',
        removed_chars, removed_ents;
END $$;

-- One character member per (acl_id, character_id).
CREATE UNIQUE INDEX acl_member_unique_character
    ON acl_member (acl_id, character_id)
    WHERE member_type = 'character';

-- One corporation/alliance member per (acl_id, member_type, eve_entity_id).
-- Keyed by member_type as well as eve_entity_id so the same EVE id may appear
-- once as a corporation and once as an alliance (the identities differ).
CREATE UNIQUE INDEX acl_member_unique_entity
    ON acl_member (acl_id, member_type, eve_entity_id)
    WHERE member_type <> 'character';
