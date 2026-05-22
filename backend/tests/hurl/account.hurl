# DELETE /api/v1/account  (soft-delete + account_soft_deleted bearer rejection)
#
# Prerequisites:
#   - An account with at least one character (post-SSO).
#   - A session cookie for step 3 (the delete itself asserts Set-Cookie clearing).
#   - ERB_API_KEY must belong to the same account as SESSION.
#
# Run with:
#   hurl --test --variable base_url=http://localhost:5000 \
#               --variable session=<jwt-from-cookie> \
#               --secret    erb_api_key=$ERB_API_KEY \
#               account.hurl
#
# WARNING: this file soft-deletes the account.  Re-logging in via SSO reactivates it.
#
# NOTE: if this is the only server-admin account on the server, step 3 will
# return 409 `cannot_remove_last_server_admin`.  Either run against an account
# that is not a server admin, or ensure another active server admin exists
# before running this file.

# ── 1. Unauthenticated → 401 ─────────────────────────────────────────────────

DELETE {{base_url}}/api/v1/account

HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"

# ── 2. Verify bearer key works before soft-delete ────────────────────────────

GET {{base_url}}/api/v1/me
Authorization: Bearer {{erb_api_key}}

HTTP 200
[Asserts]
jsonpath "$.data.account.status" == "active"

# ── 3. Soft-delete the account → 204, Set-Cookie clears session ──────────────
# Must use session cookie here — this step asserts the cookie-clearing header.

DELETE {{base_url}}/api/v1/account
Cookie: session={{session}}

HTTP 204
[Asserts]
header "Set-Cookie" contains "session="
header "Set-Cookie" contains "Max-Age=0"

# ── 4. API key for soft-deleted account → 401 account_soft_deleted ────────────

GET {{base_url}}/api/v1/me
Authorization: Bearer {{erb_api_key}}

HTTP 401
[Asserts]
jsonpath "$.error.code" == "account_soft_deleted"
