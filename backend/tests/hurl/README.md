# HURL tests

Live-server HTTP contract tests. Run against a backend started with `cargo run` (or `docker compose up`).

## Prerequisites

- `hurl` ≥ 8.0 installed (`hurl --version`)
- Backend running and reachable at `BASE_URL`
- A valid session JWT (value of the `session` cookie after SSO login — copy from browser devtools: Application → Cookies → `session`)

## Run all tests

```sh
BASE_URL=http://localhost:3000
SESSION=<paste jwt here>

hurl --test \
     --variable base_url=$BASE_URL \
     --variable session=$SESSION \
     tests/hurl/me.hurl \
     tests/hurl/keys.hurl
```

## Per-file guide

### me.hurl

Tests `GET /api/v1/me`. No extra variables beyond `base_url` and `session`.

```sh
hurl --test \
     --variable base_url=$BASE_URL \
     --variable session=$SESSION \
     tests/hurl/me.hurl
```

### keys.hurl

Tests the full API-key lifecycle (`POST`, `GET`, `DELETE /api/v1/keys`). Auth is exercised via both session cookie and bearer token — no extra variables needed beyond `base_url` and `session`.

```sh
hurl --test \
     --variable base_url=$BASE_URL \
     --variable session=$SESSION \
     tests/hurl/keys.hurl
```

### characters.hurl

Tests `POST /api/v1/characters/:id/set-main` and `DELETE /api/v1/characters/:id`.

**Requires two characters linked to the account before running.** Add a second character via `/auth/characters/add` first.

Get the character UUIDs from `GET /api/v1/me`:

```sh
curl -s $BASE_URL/api/v1/me -H "Cookie: session=$SESSION" | jq '.data.characters[] | {id, name, is_main}'
```

```sh
hurl --test \
     --variable base_url=$BASE_URL \
     --variable session=$SESSION \
     --variable main_character_id=<uuid-of-main> \
     --variable non_main_character_id=<uuid-of-non-main> \
     tests/hurl/characters.hurl
```

**Note:** steps 3–7 mutate state (set-main swaps the main; delete removes a character). Run against a disposable account or restore state afterwards.

### account.hurl

Tests `DELETE /api/v1/account` and the `account_soft_deleted` bearer rejection.

**Requires an API key issued before running** (so step 5 can verify it is rejected after soft-delete):

```sh
API_KEY=$(curl -s -X POST $BASE_URL/api/v1/keys \
               -H "Cookie: session=$SESSION" \
               -H "Content-Type: application/json" \
               -d '{"name":"soft-delete-test","expires_at":null}' | jq -r '.data.key')

hurl --test \
     --variable base_url=$BASE_URL \
     --variable session=$SESSION \
     --secret    api_key=$API_KEY \
     tests/hurl/account.hurl
```

**Note:** this file soft-deletes the account. Re-login via SSO reactivates it (status returns to `active`, `delete_requested_at` cleared).

### session.hurl

Tests that a cookie-authenticated request reissues the session cookie and an unauthenticated request does not. No extra variables beyond `base_url` and `session`.

```sh
hurl --test \
     --variable base_url=$BASE_URL \
     --variable session=$SESSION \
     tests/hurl/session.hurl
```
