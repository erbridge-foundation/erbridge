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
