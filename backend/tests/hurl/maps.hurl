# GET/POST /api/v1/maps  ·  GET/PATCH/DELETE /api/v1/maps/:map_id
# POST /api/v1/maps/:map_id/acls  ·  DELETE /api/v1/maps/:map_id/acls/:acl_id
#
# Run with:
#   hurl --test --variable base_url=http://localhost:5000 \
#               --secret    erb_api_key=$ERB_API_KEY \
#               maps.hurl

# ── 1. List maps: unauthenticated → 401 ──────────────────────────────────────

GET {{base_url}}/api/v1/maps

HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"

# ── 2. Create map: invalid slug → 400 ────────────────────────────────────────

POST {{base_url}}/api/v1/maps
Content-Type: application/json
Authorization: Bearer {{erb_api_key}}
{"name": "Bad", "slug": "Not A Slug"}

HTTP 400
[Asserts]
jsonpath "$.error.code" isString

# ── 3. Create map → 201 ──────────────────────────────────────────────────────

POST {{base_url}}/api/v1/maps
Content-Type: application/json
Authorization: Bearer {{erb_api_key}}
{"name": "Smoke Chain", "slug": "smoke-chain", "description": "hurl smoke test"}

HTTP 201
[Captures]
map_id: jsonpath "$.data.id"
[Asserts]
jsonpath "$.data.id" isString
jsonpath "$.data.slug" == "smoke-chain"
jsonpath "$.data.acls" isCollection
jsonpath "$.data.status" not exists
jsonpath "$.error" not exists

# ── 4. Duplicate slug → 409 ──────────────────────────────────────────────────

POST {{base_url}}/api/v1/maps
Content-Type: application/json
Authorization: Bearer {{erb_api_key}}
{"name": "Dup", "slug": "smoke-chain"}

HTTP 409
[Asserts]
jsonpath "$.error.code" == "map_slug_already_exists"

# ── 5. Get the map → 200 ─────────────────────────────────────────────────────

GET {{base_url}}/api/v1/maps/{{map_id}}
Authorization: Bearer {{erb_api_key}}

HTTP 200
[Asserts]
jsonpath "$.data.id" == "{{map_id}}"
jsonpath "$.error" not exists

# ── 6. Get a non-existent map → 404 ──────────────────────────────────────────

GET {{base_url}}/api/v1/maps/00000000-0000-0000-0000-000000000000
Authorization: Bearer {{erb_api_key}}

HTTP 404
[Asserts]
jsonpath "$.error.code" == "not_found"

# ── 7. Update the map → 200 ──────────────────────────────────────────────────

PATCH {{base_url}}/api/v1/maps/{{map_id}}
Content-Type: application/json
Authorization: Bearer {{erb_api_key}}
{"name": "Smoke Chain v2", "slug": "smoke-chain-2"}

HTTP 200
[Asserts]
jsonpath "$.data.slug" == "smoke-chain-2"

# ── 8. Create an ACL to attach → 201 ─────────────────────────────────────────

POST {{base_url}}/api/v1/acls
Content-Type: application/json
Authorization: Bearer {{erb_api_key}}
{"name": "Map Smoke ACL"}

HTTP 201
[Captures]
acl_id: jsonpath "$.data.id"

# ── 9. Attach the owned ACL to the map → 204 ─────────────────────────────────

POST {{base_url}}/api/v1/maps/{{map_id}}/acls
Content-Type: application/json
Authorization: Bearer {{erb_api_key}}
{"acl_id": "{{acl_id}}"}

HTTP 204

# ── 10. Attaching a non-existent ACL → 404 ───────────────────────────────────

POST {{base_url}}/api/v1/maps/{{map_id}}/acls
Content-Type: application/json
Authorization: Bearer {{erb_api_key}}
{"acl_id": "00000000-0000-0000-0000-000000000000"}

HTTP 404
[Asserts]
jsonpath "$.error.code" == "not_found"

# ── 11. Detach the ACL → 204 ─────────────────────────────────────────────────

DELETE {{base_url}}/api/v1/maps/{{map_id}}/acls/{{acl_id}}
Authorization: Bearer {{erb_api_key}}

HTTP 204

# ── 12. Detach again (no longer attached) → 404 ──────────────────────────────

DELETE {{base_url}}/api/v1/maps/{{map_id}}/acls/{{acl_id}}
Authorization: Bearer {{erb_api_key}}

HTTP 404
[Asserts]
jsonpath "$.error.code" == "not_found"

# ── 13. Clean up the ACL → 204 ───────────────────────────────────────────────

DELETE {{base_url}}/api/v1/acls/{{acl_id}}
Authorization: Bearer {{erb_api_key}}

HTTP 204

# ── 14. Soft-delete the map → 204 ────────────────────────────────────────────

DELETE {{base_url}}/api/v1/maps/{{map_id}}
Authorization: Bearer {{erb_api_key}}

HTTP 204

# ── 15. The soft-deleted map is gone → 404 ───────────────────────────────────

GET {{base_url}}/api/v1/maps/{{map_id}}
Authorization: Bearer {{erb_api_key}}

HTTP 404
[Asserts]
jsonpath "$.error.code" == "not_found"
