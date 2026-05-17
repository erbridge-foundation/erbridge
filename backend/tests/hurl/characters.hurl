# POST /api/v1/characters/:id/set-main  and  DELETE /api/v1/characters/:id
#
# Prerequisites: account must have at least TWO characters linked before running
# the set-main and cannot-remove-main assertions.  Log in with a second character
# via /auth/characters/add before running this file.
#
# Run with:
#   hurl --test --variable base_url=http://localhost:3000 \
#               --variable session=<jwt-from-cookie> \
#               --variable non_main_character_id=<uuid-of-non-main-character> \
#               --variable main_character_id=<uuid-of-main-character> \
#               characters.hurl
#
# Both character UUIDs are the internal `id` field from GET /api/v1/me.

# ── 1. set-main: unknown id → 404 ────────────────────────────────────────────

POST {{base_url}}/api/v1/characters/00000000-0000-0000-0000-000000000000/set-main
Cookie: session={{session}}

HTTP 404
[Asserts]
jsonpath "$.error.code" == "not_found"

# ── 2. set-main: unauthenticated → 401 ───────────────────────────────────────

POST {{base_url}}/api/v1/characters/{{non_main_character_id}}/set-main

HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"

# ── 3. set-main: promote non-main character → 200 ────────────────────────────

POST {{base_url}}/api/v1/characters/{{non_main_character_id}}/set-main
Cookie: session={{session}}

HTTP 200
[Asserts]
jsonpath "$.data.id" == "{{non_main_character_id}}"
jsonpath "$.data.is_main" == true
jsonpath "$.data.name" isString
jsonpath "$.data.corporation_name" isString
jsonpath "$.data.portrait_url" matches "^https://images\\.evetech\\.net/characters/[0-9]+/portrait\\?size=128$"
jsonpath "$.error" not exists

# ── 4. set-main: promoting the current main is a no-op 200 ───────────────────
# (after step 3, non_main_character_id is now the main)

POST {{base_url}}/api/v1/characters/{{non_main_character_id}}/set-main
Cookie: session={{session}}

HTTP 200
[Asserts]
jsonpath "$.data.is_main" == true

# ── 5. delete: cannot remove main while siblings exist → 409 ─────────────────
# non_main_character_id is now main; main_character_id is now non-main.
# Attempt to delete the current main (non_main_character_id).

DELETE {{base_url}}/api/v1/characters/{{non_main_character_id}}
Cookie: session={{session}}

HTTP 409
[Asserts]
jsonpath "$.error.code" == "cannot_remove_main"

# ── 6. delete: remove a non-main sibling → 204 ───────────────────────────────
# main_character_id is no longer main; it is safe to remove.

DELETE {{base_url}}/api/v1/characters/{{main_character_id}}
Cookie: session={{session}}

HTTP 204

# ── 7. delete: cannot remove the only remaining character → 409 ──────────────
# Only non_main_character_id remains now.

DELETE {{base_url}}/api/v1/characters/{{non_main_character_id}}
Cookie: session={{session}}

HTTP 409
[Asserts]
jsonpath "$.error.code" == "cannot_remove_last_character"

# ── 8. delete: non-owner cannot remove another account's character → 404 ──────
# Covered by the integration test suite (requires a second session context);
# HURL cannot express two independent auth contexts in one file without a
# second session variable.  See account_management.rs integration tests.
