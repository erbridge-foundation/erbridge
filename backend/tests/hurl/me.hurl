# GET /api/v1/me
#
# Run with:
#   hurl --test --variable base_url=http://localhost:5000 \
#               --secret    erb_api_key=$ERB_API_KEY \
#               me.hurl

# ── 1. Unauthenticated → 401 ─────────────────────────────────────────────────

GET {{base_url}}/api/v1/me

HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"
jsonpath "$.error.message" isString
jsonpath "$.data" not exists

# ── 2. Authenticated via API key → 200 with expected shape ───────────────────

GET {{base_url}}/api/v1/me
Authorization: Bearer {{erb_api_key}}

HTTP 200
[Asserts]
jsonpath "$.data" exists
jsonpath "$.data.account.id" isString
jsonpath "$.data.account.status" == "active"
jsonpath "$.data.account.is_server_admin" isBoolean
jsonpath "$.data.account.created_at" isString
# No token material in response
jsonpath "$.data.characters" isCollection
jsonpath "$.data.characters[0].id" isString
jsonpath "$.data.characters[0].eve_character_id" isInteger
jsonpath "$.data.characters[0].name" isString
jsonpath "$.data.characters[0].corporation_id" isInteger
jsonpath "$.data.characters[0].corporation_name" isString
jsonpath "$.data.characters[0].is_main" isBoolean
jsonpath "$.data.characters[0].portrait_url" matches "^https://images\\.evetech\\.net/characters/[0-9]+/portrait\\?size=128$"
jsonpath "$.data.characters[0].token_status" matches "^(active|expired)$"
# Sensitive fields must be absent
jsonpath "$.data.characters[0].encrypted_access_token" not exists
jsonpath "$.data.characters[0].encrypted_refresh_token" not exists
jsonpath "$.data.characters[0].access_token_expires_at" not exists
jsonpath "$.data.characters[0].esi_token_expires_at" not exists
jsonpath "$.data.characters[0].esi_client_id" not exists
# Envelope: no extraneous top-level keys
jsonpath "$.error" not exists
