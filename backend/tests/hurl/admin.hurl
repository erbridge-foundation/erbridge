# /api/v1/admin/*  ·  server administration (admin management, block list, audit)
#
# Admin endpoints are SESSION-COOKIE ONLY — they never accept Authorization:
# Bearer (a leaked API key must not confer admin). So this file authenticates
# with cookies, not $erb_api_key:
#
#   --variable admin_session=<session JWT of a SERVER-ADMIN account>
#   --variable non_admin_session=<session JWT of a NON-admin account>
#   --variable admin_account_id=<UUID of the admin's own account>
#   --variable grant_target_id=<UUID of an account to promote/demote>
#
# The unauthenticated (no-cookie) assertions run with no extra variables. The
# admin-session flow needs the variables above; obtain a session JWT after SSO
# login (the `session` cookie value), as for session.hurl.
#
# Run with:
#   hurl --test --variable base_url=http://localhost:5000 \
#               --variable admin_session=$ADMIN_SESSION \
#               --variable non_admin_session=$NON_ADMIN_SESSION \
#               --variable admin_account_id=$ADMIN_ACCOUNT_ID \
#               --variable grant_target_id=$GRANT_TARGET_ID \
#               admin.hurl
#
# NOTE: the grant/revoke steps mutate state (promote then demote
# grant_target_id). Use a disposable target account.

# ── 1. Every admin endpoint with no credentials → 401 ────────────────────────

GET {{base_url}}/api/v1/admin/accounts
HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"

GET {{base_url}}/api/v1/admin/characters/search?q=x
HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"

GET {{base_url}}/api/v1/admin/characters/esi-search?q=wasp
HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"

POST {{base_url}}/api/v1/admin/accounts/00000000-0000-0000-0000-000000000000/grant-admin
HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"

POST {{base_url}}/api/v1/admin/accounts/00000000-0000-0000-0000-000000000000/revoke-admin
HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"

GET {{base_url}}/api/v1/admin/blocks
HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"

POST {{base_url}}/api/v1/admin/blocks
Content-Type: application/json
{"eve_character_id": 1, "reason": null}
HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"

DELETE {{base_url}}/api/v1/admin/blocks/1
HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"

GET {{base_url}}/api/v1/admin/audit
HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"

# The new search/window query params must not bypass the auth gate either.
GET {{base_url}}/api/v1/admin/audit?q=wasp&window=90d&event_type=eve_character_blocked
HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"

# ── 2. Admin endpoint rejects a bearer API key → 401 (cookie-only) ───────────
#
# Even a valid account-scoped key for an admin account must not authenticate
# here. Uses $erb_api_key if supplied; harmless 401 either way.

GET {{base_url}}/api/v1/admin/accounts
Authorization: Bearer {{erb_api_key}}
HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"

# ── 3. Non-admin session → 403 forbidden_admin_required ──────────────────────

GET {{base_url}}/api/v1/admin/accounts
Cookie: session={{non_admin_session}}
HTTP 403
[Asserts]
jsonpath "$.error.code" == "forbidden_admin_required"

GET {{base_url}}/api/v1/admin/characters/esi-search?q=wasp
Cookie: session={{non_admin_session}}
HTTP 403
[Asserts]
jsonpath "$.error.code" == "forbidden_admin_required"

# ── 4. Admin lists accounts → 200 (each with characters) ─────────────────────

GET {{base_url}}/api/v1/admin/accounts
Cookie: session={{admin_session}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection
jsonpath "$.data[0].id" isString
jsonpath "$.data[0].is_server_admin" isBoolean
jsonpath "$.data[0].characters" isCollection
jsonpath "$.error" not exists

# ── 5. Character search → 200, results carry owning account_id ───────────────

GET {{base_url}}/api/v1/admin/characters/search?q=a
Cookie: session={{admin_session}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection
jsonpath "$.data[0].portrait_url" exists
jsonpath "$.data[0].already_blocked" isBoolean

# ── 5b. ESI character search: short fragment → 400 ───────────────────────────
#
# The 3-char minimum is enforced before any token/ESI work, so this is
# hermetic (no live ESI needed). A full-length query exercises the real ESI
# call and is operator-run: depending on the admin's token scope/ESI health it
# returns 200 with either matches or `unavailable: true` — both are a 200, so
# the assertion only checks the envelope shape, not the match contents.

GET {{base_url}}/api/v1/admin/characters/esi-search?q=wa
Cookie: session={{admin_session}}
HTTP 400
[Asserts]
jsonpath "$.error.code" == "bad_request"

GET {{base_url}}/api/v1/admin/characters/esi-search?q=wasp
Cookie: session={{admin_session}}
HTTP 200
[Asserts]
jsonpath "$.data.results" isCollection
jsonpath "$.data.unavailable" isBoolean

# ── 6. Grant admin to the target account → 204 ───────────────────────────────

POST {{base_url}}/api/v1/admin/accounts/{{grant_target_id}}/grant-admin
Cookie: session={{admin_session}}
HTTP 204

# ── 7. Grant is idempotent → 204 ─────────────────────────────────────────────

POST {{base_url}}/api/v1/admin/accounts/{{grant_target_id}}/grant-admin
Cookie: session={{admin_session}}
HTTP 204

# ── 8. Grant a non-existent account → 404 ────────────────────────────────────

POST {{base_url}}/api/v1/admin/accounts/00000000-0000-0000-0000-000000000000/grant-admin
Cookie: session={{admin_session}}
HTTP 404
[Asserts]
jsonpath "$.error.code" == "not_found"

# ── 9. Revoke the target (two admins now exist, so this is allowed) → 204 ────

POST {{base_url}}/api/v1/admin/accounts/{{grant_target_id}}/revoke-admin
Cookie: session={{admin_session}}
HTTP 204

# ── 10. Last-admin guard: self-revoke when sole admin → 409 ──────────────────
#
# After step 9 the acting admin is the only admin again, so revoking itself
# must be rejected. (If other admins exist in your environment this returns 204;
# run against a stack where admin_account_id is the only admin to exercise 409.)

POST {{base_url}}/api/v1/admin/accounts/{{admin_account_id}}/revoke-admin
Cookie: session={{admin_session}}
HTTP 409
[Asserts]
jsonpath "$.error.code" == "cannot_remove_last_server_admin"

# ── 11. Block a character → 204; list shows it ───────────────────────────────

POST {{base_url}}/api/v1/admin/blocks
Content-Type: application/json
Cookie: session={{admin_session}}
{"eve_character_id": 90909090, "reason": "hurl-smoke"}

HTTP 204

GET {{base_url}}/api/v1/admin/blocks
Cookie: session={{admin_session}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection
jsonpath "$.data[?(@.eve_character_id == 90909090)].reason" == "hurl-smoke"

# ── 12. Block is idempotent → 204 ────────────────────────────────────────────

POST {{base_url}}/api/v1/admin/blocks
Content-Type: application/json
Cookie: session={{admin_session}}
{"eve_character_id": 90909090, "reason": "hurl-smoke"}

HTTP 204

# ── 13. Unblock → 204; unblock again → 404 ───────────────────────────────────

DELETE {{base_url}}/api/v1/admin/blocks/90909090
Cookie: session={{admin_session}}
HTTP 204

DELETE {{base_url}}/api/v1/admin/blocks/90909090
Cookie: session={{admin_session}}
HTTP 404
[Asserts]
jsonpath "$.error.code" == "not_found"

# ── 14. Audit list → 200, newest-first page with a next_before cursor ────────

GET {{base_url}}/api/v1/admin/audit
Cookie: session={{admin_session}}
HTTP 200
[Captures]
oldest_before: jsonpath "$.data.next_before"
[Asserts]
jsonpath "$.data.entries" isCollection
jsonpath "$.data.entries[0].id" isString
jsonpath "$.data.entries[0].event_type" isString

# ── 15. Audit `before` pagination → 200 (entries older than the cursor) ──────

GET {{base_url}}/api/v1/admin/audit?before={{oldest_before}}&limit=20
Cookie: session={{admin_session}}
HTTP 200
[Asserts]
jsonpath "$.data.entries" isCollection

# ── 16. Audit target_type + target_id exact-entity filter → 200 ──────────────

GET {{base_url}}/api/v1/admin/audit?event_type=eve_character_blocked&target_id=90909090
Cookie: session={{admin_session}}
HTTP 200
[Asserts]
jsonpath "$.data.entries" isCollection

# ── 17. Audit default window → 200 (recent entries within last 7 days) ───────

GET {{base_url}}/api/v1/admin/audit
Cookie: session={{admin_session}}
HTTP 200
[Asserts]
jsonpath "$.data.entries" isCollection

# ── 18. Audit widened window (90d) → 200 ─────────────────────────────────────

GET {{base_url}}/api/v1/admin/audit?window=90d
Cookie: session={{admin_session}}
HTTP 200
[Asserts]
jsonpath "$.data.entries" isCollection

# ── 19. Audit combined name search `q` (actor- and target-side) → 200 ────────

GET {{base_url}}/api/v1/admin/audit?q=pilot&window=90d
Cookie: session={{admin_session}}
HTTP 200
[Asserts]
jsonpath "$.data.entries" isCollection

# ── 20. Audit conjunction: q + event_type + window → 200 ─────────────────────

GET {{base_url}}/api/v1/admin/audit?q=pilot&event_type=eve_character_blocked&window=90d
Cookie: session={{admin_session}}
HTTP 200
[Asserts]
jsonpath "$.data.entries" isCollection

# ── 21. Audit `q` matches a value carried only in details ─────────────────────
#
# "hurl-smoke" is the block reason from steps 11–13; it is never an actor or
# target name — it lives only in the event's `details` payload. Finding it
# proves the `q` axis now searches `details::text` (self-contained search), and
# that a name/value snapshotted into details is findable. (Run after the block
# steps above, which emit eve_character_blocked rows carrying this reason.)

GET {{base_url}}/api/v1/admin/audit?q=hurl-smoke&window=90d
Cookie: session={{admin_session}}
HTTP 200
[Asserts]
jsonpath "$.data.entries" isCollection
jsonpath "$.data.entries[0].event_type" == "eve_character_blocked"
jsonpath "$.data.entries[0].details.reason" == "hurl-smoke"
