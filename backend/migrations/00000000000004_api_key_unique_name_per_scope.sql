-- Enforce the api-authentication spec's "Duplicate name is rejected" rule:
--   - For scope='account': name must be unique within each account_id.
--   - For scope='server':  name must be unique across all server-scoped keys
--                          (account_id is NULL there).
-- Two partial unique indexes — one per scope — keep the constraint precise and
-- let server-scoped keys (which have account_id IS NULL) be deduplicated
-- without disturbing account-scoped semantics.
CREATE UNIQUE INDEX api_key_account_scope_name_idx
    ON api_key (account_id, name)
    WHERE scope = 'account';

CREATE UNIQUE INDEX api_key_server_scope_name_idx
    ON api_key (name)
    WHERE scope = 'server';
