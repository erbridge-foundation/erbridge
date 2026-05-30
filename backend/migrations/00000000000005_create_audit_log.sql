CREATE TABLE audit_log (
    id                   UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    occurred_at          TIMESTAMPTZ  NOT NULL    DEFAULT now(),

    actor_account_id     UUID         REFERENCES account(id) ON DELETE SET NULL,
    actor_character_id   BIGINT,
    actor_character_name TEXT,

    event_type           TEXT         NOT NULL,
    details              JSONB        NOT NULL    DEFAULT '{}'
);

CREATE INDEX audit_log_occurred_at_idx     ON audit_log (occurred_at DESC);
CREATE INDEX audit_log_actor_account_idx   ON audit_log (actor_account_id)
    WHERE actor_account_id IS NOT NULL;
CREATE INDEX audit_log_actor_character_idx ON audit_log (actor_character_id)
    WHERE actor_character_id IS NOT NULL;
