CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE account (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    status              TEXT        NOT NULL DEFAULT 'active',
    delete_requested_at TIMESTAMPTZ,
    is_server_admin     BOOLEAN     NOT NULL DEFAULT FALSE
);

CREATE INDEX account_server_admin_idx ON account (id) WHERE is_server_admin = TRUE;

CREATE TABLE eve_character (
    id                      UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id              UUID        REFERENCES account(id) ON DELETE CASCADE,
    eve_character_id        BIGINT      NOT NULL UNIQUE,
    name                    TEXT        NOT NULL,
    corporation_id          BIGINT      NOT NULL,
    alliance_id             BIGINT,
    is_main                 BOOLEAN     NOT NULL DEFAULT false,
    is_online               BOOLEAN,
    esi_client_id           TEXT,
    encrypted_access_token  BYTEA,
    encrypted_refresh_token BYTEA,
    esi_token_expires_at    TIMESTAMPTZ,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX eve_character_one_main_per_account
    ON eve_character(account_id)
    WHERE is_main = true;

CREATE INDEX eve_character_account_id_idx ON eve_character (account_id);

CREATE TABLE api_key (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    scope         TEXT        NOT NULL,
    account_id    UUID        REFERENCES account(id) ON DELETE CASCADE,
    name          TEXT        NOT NULL,
    key_hash      TEXT        NOT NULL,
    expires_at    TIMESTAMPTZ NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT api_key_scope_check CHECK (
        (scope = 'account' AND account_id IS NOT NULL)
        OR (scope = 'server' AND account_id IS NULL)
    )
);

CREATE UNIQUE INDEX api_key_hash_idx ON api_key (key_hash);
CREATE INDEX api_key_account_idx ON api_key (account_id) WHERE account_id IS NOT NULL;
