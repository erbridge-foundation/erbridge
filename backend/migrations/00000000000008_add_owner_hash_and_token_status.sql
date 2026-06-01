-- Character token lifecycle: owner-hash tracking + token state.
--
-- `owner_hash` is the EVE SSO `owner` claim — a hash CCP rotates when a
-- character is transferred to a different account. It is nullable: NULL means
-- "not yet observed" (legacy rows, or a character seen before this column
-- existed) and is never treated as a transfer; the next successful auth records
-- it for future comparison.
--
-- `token_status` is the advisory health of the stored credentials, driven by the
-- daily refresh sweep and reset to 'valid' by any successful auth presenting a
-- matching owner hash (no state is terminal):
--   'valid'          — usable tokens (or freshly authenticated)
--   'token_expired'  — refresh failed, or the account went idle past the floor
--   'owner_mismatch' — a successful refresh returned a DIFFERENT owner hash,
--                      i.e. the character was transferred away (proof of sale)
ALTER TABLE eve_character
    ADD COLUMN owner_hash   TEXT,
    ADD COLUMN token_status TEXT NOT NULL DEFAULT 'valid'
        CHECK (token_status IN ('valid', 'token_expired', 'owner_mismatch'));

-- Account-level freshness clock. Bumped on every successful login; read by the
-- sweep's 7-day idle waterfall. Nullable: NULL means "not yet observed" and is
-- excluded from the waterfall so legacy accounts are not mass-expired on the
-- first sweep run.
ALTER TABLE account
    ADD COLUMN last_login TIMESTAMPTZ;
