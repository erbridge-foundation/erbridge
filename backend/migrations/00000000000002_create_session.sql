CREATE TABLE session (
    session_id         TEXT        PRIMARY KEY,
    account_id         UUID        NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    csrf_state         TEXT,
    add_character_mode BOOL        NOT NULL DEFAULT FALSE,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at         TIMESTAMPTZ NOT NULL
);

CREATE INDEX session_expires_at_idx ON session (expires_at);
CREATE INDEX session_account_id_idx ON session (account_id);
