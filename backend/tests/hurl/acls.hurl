# GET/POST /api/v1/acls  ·  PATCH/DELETE /api/v1/acls/:acl_id
# GET/POST /api/v1/acls/:acl_id/members  ·  PATCH/DELETE .../members/:member_id
#
# Run with:
#   hurl --test --variable base_url=http://localhost:5000 \
#               --secret    erb_api_key=$ERB_API_KEY \
#               acls.hurl

# ── 1. List ACLs: unauthenticated → 401 ──────────────────────────────────────

GET {{base_url}}/api/v1/acls

HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"

# ── 2. Create ACL: empty name → 400 ──────────────────────────────────────────

POST {{base_url}}/api/v1/acls
Content-Type: application/json
Authorization: Bearer {{erb_api_key}}
{"name": "   "}

HTTP 400
[Asserts]
jsonpath "$.error.code" isString

# ── 3. Create ACL → 201 ──────────────────────────────────────────────────────

POST {{base_url}}/api/v1/acls
Content-Type: application/json
Authorization: Bearer {{erb_api_key}}
{"name": "Smoke ACL"}

HTTP 201
[Captures]
acl_id: jsonpath "$.data.id"
[Asserts]
jsonpath "$.data.id" isString
jsonpath "$.data.name" == "Smoke ACL"
jsonpath "$.data.owner_account_id" isString
jsonpath "$.data.created_at" isString
jsonpath "$.error" not exists

# ── 4. List ACLs → 200, includes the new one ─────────────────────────────────

GET {{base_url}}/api/v1/acls
Authorization: Bearer {{erb_api_key}}

HTTP 200
[Asserts]
jsonpath "$.data" isCollection
jsonpath "$.error" not exists

# ── 4b. Get the ACL by id → 200 ──────────────────────────────────────────────

GET {{base_url}}/api/v1/acls/{{acl_id}}
Authorization: Bearer {{erb_api_key}}

HTTP 200
[Asserts]
jsonpath "$.data.id" == "{{acl_id}}"
jsonpath "$.data.name" == "Smoke ACL"
jsonpath "$.error" not exists

# ── 4c. Get an unknown ACL by id → 404 ───────────────────────────────────────

GET {{base_url}}/api/v1/acls/00000000-0000-0000-0000-000000000000
Authorization: Bearer {{erb_api_key}}

HTTP 404
[Asserts]
jsonpath "$.error.code" == "not_found"

# ── 5. Rename ACL → 200 ──────────────────────────────────────────────────────

PATCH {{base_url}}/api/v1/acls/{{acl_id}}
Content-Type: application/json
Authorization: Bearer {{erb_api_key}}
{"name": "Smoke ACL Renamed"}

HTTP 200
[Asserts]
jsonpath "$.data.name" == "Smoke ACL Renamed"

# ── 6. Add a corporation member → 201 ────────────────────────────────────────

POST {{base_url}}/api/v1/acls/{{acl_id}}/members
Content-Type: application/json
Authorization: Bearer {{erb_api_key}}
{"member_type": "corporation", "eve_entity_id": 98000001, "name": "Smoke Corp", "permission": "read"}

HTTP 201
[Captures]
member_id: jsonpath "$.data.id"
[Asserts]
jsonpath "$.data.member_type" == "corporation"
jsonpath "$.data.eve_entity_id" == 98000001
jsonpath "$.data.permission" == "read"
jsonpath "$.error" not exists

# ── 6b. Re-adding the same corporation member → 409 duplicate_acl_member ──────

POST {{base_url}}/api/v1/acls/{{acl_id}}/members
Content-Type: application/json
Authorization: Bearer {{erb_api_key}}
{"member_type": "corporation", "eve_entity_id": 98000001, "name": "Smoke Corp", "permission": "read_write"}

HTTP 409
[Asserts]
jsonpath "$.error.code" == "duplicate_acl_member"

# ── 7. Corporation member cannot be granted manage → 400 ─────────────────────

POST {{base_url}}/api/v1/acls/{{acl_id}}/members
Content-Type: application/json
Authorization: Bearer {{erb_api_key}}
{"member_type": "corporation", "eve_entity_id": 98000002, "permission": "manage"}

HTTP 400
[Asserts]
jsonpath "$.error.code" isString

# ── 7b. Add a character member with no character_id → 201 (mints the orphan) ──
#
# The picker can add a never-seen character by its EVE id alone; the add
# find-or-mints the orphan eve_character row (public-info snapshot best-effort,
# placeholder on ESI failure) and the member references its UUID. The response's
# character_id is the resolved (minted) UUID, not the request's (which is absent).
# Use a plausible EVE character id; re-running the suite reuses the same orphan.

POST {{base_url}}/api/v1/acls/{{acl_id}}/members
Content-Type: application/json
Authorization: Bearer {{erb_api_key}}
{"member_type": "character", "eve_entity_id": 2112625428, "name": "Smoke Pilot", "permission": "read"}

HTTP 201
[Asserts]
jsonpath "$.data.member_type" == "character"
jsonpath "$.data.eve_entity_id" == 2112625428
jsonpath "$.data.character_id" isString
jsonpath "$.data.permission" == "read"
jsonpath "$.error" not exists

# ── 8. List members → 200 ────────────────────────────────────────────────────

GET {{base_url}}/api/v1/acls/{{acl_id}}/members
Authorization: Bearer {{erb_api_key}}

HTTP 200
[Asserts]
jsonpath "$.data" isCollection
jsonpath "$.data[0].id" isString
jsonpath "$.error" not exists

# ── 9. Update member permission → 200 ────────────────────────────────────────

PATCH {{base_url}}/api/v1/acls/{{acl_id}}/members/{{member_id}}
Content-Type: application/json
Authorization: Bearer {{erb_api_key}}
{"permission": "read_write"}

HTTP 200
[Asserts]
jsonpath "$.data.permission" == "read_write"

# ── 10. Remove member → 204 ──────────────────────────────────────────────────

DELETE {{base_url}}/api/v1/acls/{{acl_id}}/members/{{member_id}}
Authorization: Bearer {{erb_api_key}}

HTTP 204

# ── 11. Delete ACL → 204 ─────────────────────────────────────────────────────

DELETE {{base_url}}/api/v1/acls/{{acl_id}}
Authorization: Bearer {{erb_api_key}}

HTTP 204

# ── 12. Rename a non-existent ACL → 404 ──────────────────────────────────────

PATCH {{base_url}}/api/v1/acls/00000000-0000-0000-0000-000000000000
Content-Type: application/json
Authorization: Bearer {{erb_api_key}}
{"name": "Ghost"}

HTTP 404
[Asserts]
jsonpath "$.error.code" == "not_found"
