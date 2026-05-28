# POST /api/v1/keys  ·  GET /api/v1/keys  ·  DELETE /api/v1/keys/:id
#
# Run with:
#   hurl --test --variable base_url=http://localhost:5000 \
#               --secret    erb_api_key=$ERB_API_KEY \
#               keys.hurl

# ── 1. Create key: unauthenticated → 401 ─────────────────────────────────────

POST {{base_url}}/api/v1/keys
Content-Type: application/json
{"name": "smoke", "expires_at": null}

HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"

# ── 2. Create key: empty name → 400 ──────────────────────────────────────────

POST {{base_url}}/api/v1/keys
Content-Type: application/json
Authorization: Bearer {{erb_api_key}}
{"name": "   ", "expires_at": null}

HTTP 400
[Asserts]
jsonpath "$.error.code" isString

# ── 3. Create key via API key → 201 ──────────────────────────────────────────

POST {{base_url}}/api/v1/keys
Content-Type: application/json
Authorization: Bearer {{erb_api_key}}
{"name": "smoke", "expires_at": null}

HTTP 201
[Captures]
created_key_id: jsonpath "$.data.id"
created_key_plaintext: jsonpath "$.data.key"
[Asserts]
jsonpath "$.data.id" isString
jsonpath "$.data.key" matches "^erb_[A-Za-z0-9_-]{43}$"
jsonpath "$.data.name" == "smoke"
jsonpath "$.data.created_at" isString
# Must not expose hash
jsonpath "$.data.key_hash" not exists
jsonpath "$.error" not exists

# ── 3a. Create key with duplicate name → 409 ─────────────────────────────────

POST {{base_url}}/api/v1/keys
Content-Type: application/json
Authorization: Bearer {{erb_api_key}}
{"name": "smoke", "expires_at": null}

HTTP 409
[Asserts]
jsonpath "$.error.code" == "api_key_name_already_exists"

# ── 4. List keys via Bearer token → 200 ──────────────────────────────────────

GET {{base_url}}/api/v1/keys
Authorization: Bearer {{created_key_plaintext}}

HTTP 200
[Asserts]
jsonpath "$.data" isCollection
jsonpath "$.data[0].id" isString
jsonpath "$.data[0].name" isString
jsonpath "$.data[0].scope" == "account"
# No plaintext in list response
jsonpath "$.data[0].key" not exists
jsonpath "$.data[0].key_hash" not exists
jsonpath "$.error" not exists

# ── 5. Delete the newly-created key via Bearer token → 204 ───────────────────

DELETE {{base_url}}/api/v1/keys/{{created_key_id}}
Authorization: Bearer {{created_key_plaintext}}

HTTP 204

# ── 6. Deleted key no longer authenticates → 401 ─────────────────────────────

GET {{base_url}}/api/v1/keys
Authorization: Bearer {{created_key_plaintext}}

HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"

# ── 7. Delete non-existent key → 404 ─────────────────────────────────────────

DELETE {{base_url}}/api/v1/keys/00000000-0000-0000-0000-000000000000
Authorization: Bearer {{erb_api_key}}

HTTP 404
[Asserts]
jsonpath "$.error.code" == "not_found"
