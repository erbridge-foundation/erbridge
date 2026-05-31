# HURL tests

Live-server HTTP contract tests. Run against the dev stack with `docker compose -f docker-compose.dev.yml up --build`.

## Prerequisites

- `hurl` ≥ 8.0 installed (`hurl --version`)
- Dev stack running and reachable at `BASE_URL` (default `http://localhost:5000`)
- An API key for your account — create one after SSO login:

```sh
export ERB_API_KEY=$(curl -s -X POST $BASE_URL/api/v1/keys \
  -H "Cookie: session=$SESSION" \
  -H "Content-Type: application/json" \
  -d '{"name":"hurl-testing","expires_at":null}' | jq -r '.data.key')
```

Most tests authenticate via `Authorization: Bearer $ERB_API_KEY`. The `session` variable is only needed for `session.hurl` (cookie-reissue behaviour) and `account.hurl` step 3 (asserts the `Set-Cookie` clearing header on delete).

## Run all tests

```sh
export BASE_URL=http://localhost:5000

hurl --test \
     --variable base_url=$BASE_URL \
     --secret    erb_api_key=$ERB_API_KEY \
     tests/hurl/me.hurl \
     tests/hurl/keys.hurl
```

## Per-file guide

### me.hurl

Tests `GET /api/v1/me`.

```sh
hurl --test \
     --variable base_url=$BASE_URL \
     --secret    erb_api_key=$ERB_API_KEY \
     tests/hurl/me.hurl
```

### keys.hurl

Tests the full API-key lifecycle (`POST`, `GET`, `DELETE /api/v1/keys`). Creates and deletes a temporary key during the run; `ERB_API_KEY` is used for auth on the steps that create/delete it.

```sh
hurl --test \
     --variable base_url=$BASE_URL \
     --secret    erb_api_key=$ERB_API_KEY \
     tests/hurl/keys.hurl
```

### characters.hurl

Tests `POST /api/v1/characters/:id/set-main` and `DELETE /api/v1/characters/:id`.

**Requires two characters linked to the account before running.** Add a second character via `/auth/characters/add` first.

Get the character UUIDs from `GET /api/v1/me`:

```sh
curl -s $BASE_URL/api/v1/me -H "Authorization: Bearer $ERB_API_KEY" \
  | jq '.data.characters[] | {id, name, is_main}'
```

```sh
hurl --test \
     --variable base_url=$BASE_URL \
     --secret    erb_api_key=$ERB_API_KEY \
     --variable  main_character_id=<uuid-of-main> \
     --variable  non_main_character_id=<uuid-of-non-main> \
     tests/hurl/characters.hurl
```

**Note:** steps 3–7 mutate state (set-main swaps the main; delete removes a character). Run against a disposable account or restore state afterwards.

### account.hurl

Tests `DELETE /api/v1/account` and the `account_soft_deleted` bearer rejection.

Requires both `ERB_API_KEY` (belonging to the account being deleted) and a `SESSION` cookie (for step 3, which asserts the `Set-Cookie` clearing header):

```sh
hurl --test \
     --variable base_url=$BASE_URL \
     --variable session=$SESSION \
     --secret    erb_api_key=$ERB_API_KEY \
     tests/hurl/account.hurl
```

**Note:** this file soft-deletes the account. Re-login via SSO reactivates it (status returns to `active`, `delete_requested_at` cleared).

### session.hurl

Tests that a cookie-authenticated request reissues the session cookie and that an unauthenticated request does not. Intentionally uses a session cookie — this is the only file that requires `SESSION`.

```sh
hurl --test \
     --variable base_url=$BASE_URL \
     --variable session=$SESSION \
     tests/hurl/session.hurl
```

### admin.hurl

Tests the `/api/v1/admin/*` surface. Admin endpoints are **session-cookie only** (they reject `Authorization: Bearer`, so a leaked key cannot confer admin), so this file authenticates with cookies, not `ERB_API_KEY`.

The unauthenticated (no-cookie) assertions — every endpoint → 401, and the bearer-key → 401 — run with no extra variables:

```sh
# No-credential assertions only (steps 1–2): always runnable.
hurl --test --variable base_url=$BASE_URL tests/hurl/admin.hurl
```

The full flow (non-admin 403, list/search, grant→revoke incl. the last-admin 409, block/list/unblock, audit list + `before` pagination + a `target_id` filter) needs an admin and non-admin session JWT (obtained after SSO login, like `SESSION`) plus a disposable grant target:

```sh
hurl --test \
     --variable base_url=$BASE_URL \
     --variable admin_session=$ADMIN_SESSION \
     --variable non_admin_session=$NON_ADMIN_SESSION \
     --variable admin_account_id=$ADMIN_ACCOUNT_ID \
     --variable grant_target_id=$GRANT_TARGET_ID \
     --secret    erb_api_key=$ERB_API_KEY \
     tests/hurl/admin.hurl
```

**Note:** the grant/revoke and block/unblock steps mutate state. Use a disposable `grant_target_id`. The last-admin 409 step assumes `admin_account_id` is the only admin at that point.

### blocks.hurl

Asserts block enforcement on the bearer route: once an admin blocks a character owned by an account, that account's API key is rejected with 401 `account_blocked` (the key row is not deleted). Restores state by unblocking at the end (tokens/sessions are not restored — re-SSO required).

Requires the **victim** account's `ERB_API_KEY`, an **admin** session (a different account), and an `eve_character_id` owned by the victim:

```sh
hurl --test \
     --variable base_url=$BASE_URL \
     --variable admin_session=$ADMIN_SESSION \
     --variable victim_eve_character_id=$VICTIM_CHAR_ID \
     --secret    erb_api_key=$VICTIM_API_KEY \
     tests/hurl/blocks.hurl
```
