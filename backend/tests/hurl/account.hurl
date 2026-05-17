# DELETE /api/v1/account  (soft-delete + account_soft_deleted bearer rejection)
#
# Prerequisites:
#   - An account with at least one character (post-SSO).
#   - An API key issued to that account (so we can verify it is rejected after
#     soft-delete).  Create one first:
#       curl -s -X POST $BASE_URL/api/v1/keys \
#            -H "Cookie: session=$SESSION" \
#            -H "Content-Type: application/json" \
#            -d '{"name":"soft-delete-test","expires_at":null}' | jq -r '.data.key'
#
# Run with:
#   hurl --test --variable base_url=http://localhost:3000 \
#               --variable session=<jwt-from-cookie> \
#               --secret    api_key=<erb_…-plaintext-key> \
#               account.hurl
#
# WARNING: this file soft-deletes the account.  Re-logging in via SSO reactivates it.

# ── 1. Unauthenticated → 401 ─────────────────────────────────────────────────

DELETE {{base_url}}/api/v1/account

HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"

# ── 2. Verify bearer key works before soft-delete ────────────────────────────

GET {{base_url}}/api/v1/me
Authorization: Bearer {{api_key}}

HTTP 200
[Asserts]
jsonpath "$.data.account.status" == "active"

# ── 3. Soft-delete the account → 204, Set-Cookie clears session ──────────────

DELETE {{base_url}}/api/v1/account
Cookie: session={{session}}

HTTP 204
[Asserts]
header "Set-Cookie" contains "session="
header "Set-Cookie" contains "Max-Age=0"

# ── 4. Session cookie is now invalid → 401 ───────────────────────────────────

GET {{base_url}}/api/v1/me
Cookie: session={{session}}

HTTP 401

# ── 5. Bearer key for soft-deleted account → 401 account_soft_deleted ─────────

GET {{base_url}}/api/v1/me
Authorization: Bearer {{api_key}}

HTTP 401
[Asserts]
jsonpath "$.error.code" == "account_soft_deleted"
