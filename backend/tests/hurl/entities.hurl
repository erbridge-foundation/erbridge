# GET /api/v1/entities/search  ·  account-authenticated entity name search
#
# Run with:
#   hurl --test --variable base_url=http://localhost:5000 \
#               --secret    erb_api_key=$ERB_API_KEY \
#               entities.hurl
#
# The 3-char minimum and the auth gate are enforced before any token/ESI work,
# so those assertions are hermetic. A full-length query exercises the real ESI
# call and is operator-run: depending on the account's token scope / ESI health
# it returns 200 with either matches or `unavailable: true` — both are a 200, so
# those assertions only check the envelope shape, not the match contents.
#
# Note: a character result's `id` (the eve_character.id UUID) is NULLABLE — it is
# present only when a local row already exists; an unknown character carries
# `id: null` and the search mints nothing. The mint happens at the ACL member add
# (see acls.hurl). Name resolution is a single bulk POST /universe/names/ call.

# ── 1. Unauthenticated → 401 ─────────────────────────────────────────────────

GET {{base_url}}/api/v1/entities/search?q=wasp

HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"

# ── 2. Fragment shorter than 3 characters → 400 ──────────────────────────────

GET {{base_url}}/api/v1/entities/search?q=wa
Authorization: Bearer {{erb_api_key}}

HTTP 400
[Asserts]
jsonpath "$.error.code" == "bad_request"

# ── 3. Multi-category search (default categories) → 200 ──────────────────────
#
# Default (no `categories`) searches all three; the response groups results by
# category and carries the `unavailable` indicator.

GET {{base_url}}/api/v1/entities/search?q=wasp
Authorization: Bearer {{erb_api_key}}

HTTP 200
[Asserts]
jsonpath "$.data.characters" isCollection
jsonpath "$.data.corporations" isCollection
jsonpath "$.data.alliances" isCollection
jsonpath "$.data.unavailable" isBoolean
jsonpath "$.error" not exists

# ── 4. Categories filter restricts the search → 200 ──────────────────────────
#
# Requesting only `corporation` still returns all three group keys (the other
# two empty), so the picker can render a stable shape.

GET {{base_url}}/api/v1/entities/search?q=wasp&categories=corporation
Authorization: Bearer {{erb_api_key}}

HTTP 200
[Asserts]
jsonpath "$.data.corporations" isCollection
jsonpath "$.data.characters" isCollection
jsonpath "$.data.alliances" isCollection
jsonpath "$.data.unavailable" isBoolean
