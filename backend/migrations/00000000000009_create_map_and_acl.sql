-- Maps and access-control lists.
--
-- Four foundational tables: the `map` container, the reusable named `acl`,
-- its `acl_member` grants, and the `map_acl` join attaching ACLs to maps.
-- Deliberately excludes map contents (connections/signatures/routes),
-- event-sourcing, and ACL orphan-reaping — see the add-maps-and-acls change.

-- A map: an account-owned, soft-deletable container. Soft-delete mirrors the
-- `account` convention (`status` + `delete_requested_at`). The checkpoint and
-- retention columns of the older iteration are intentionally absent.
CREATE TABLE map (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name                TEXT        NOT NULL,
    slug                TEXT        NOT NULL UNIQUE,
    owner_account_id    UUID        REFERENCES account(id) ON DELETE SET NULL,
    description         TEXT,
    status              TEXT        NOT NULL DEFAULT 'active',
    delete_requested_at TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX map_owner_idx ON map (owner_account_id);

-- A reusable, named access-control list owned by the account that created it.
-- No orphan-reaping: an ACL attached to no map simply persists.
CREATE TABLE acl (
    id               UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name             TEXT        NOT NULL,
    owner_account_id UUID        REFERENCES account(id) ON DELETE SET NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- A single grant within an ACL: a character (by character_id), corporation, or
-- alliance (by eve_entity_id) gets one permission. `name` snapshots the
-- entity's display name so reads need not re-resolve eve_entity_id via ESI.
CREATE TABLE acl_member (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    acl_id        UUID        NOT NULL REFERENCES acl(id) ON DELETE CASCADE,
    member_type   TEXT        NOT NULL,
    eve_entity_id BIGINT,
    character_id  UUID        REFERENCES eve_character(id) ON DELETE CASCADE,
    name          TEXT        NOT NULL DEFAULT '',
    permission    TEXT        NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT acl_member_type_check
        CHECK (member_type IN ('character', 'corporation', 'alliance')),
    CONSTRAINT acl_member_permission_check
        CHECK (permission IN ('read', 'read_write', 'manage', 'admin', 'deny')),
    -- manage/admin are reserved for character members; a corporation or
    -- alliance can be granted access but cannot administer the ACL.
    CONSTRAINT acl_member_role_for_type
        CHECK (member_type = 'character' OR permission NOT IN ('manage', 'admin'))
);

CREATE INDEX acl_member_acl_idx ON acl_member (acl_id);

-- Join: a map may be guarded by many ACLs; an ACL may guard many maps.
CREATE TABLE map_acl (
    map_id     UUID        NOT NULL REFERENCES map(id) ON DELETE CASCADE,
    acl_id     UUID        NOT NULL REFERENCES acl(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (map_id, acl_id)
);

CREATE INDEX map_acl_acl_idx ON map_acl (acl_id);
