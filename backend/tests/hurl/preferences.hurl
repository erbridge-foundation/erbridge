# GET / PATCH /api/v1/me/preferences
#
# Prerequisites:
#   - ERB_API_KEY must belong to an existing, active account.
#
# Run with:
#   hurl --test --variable base_url=http://localhost:5000 \
#               --secret    erb_api_key=$ERB_API_KEY \
#               preferences.hurl
#
# NOTE: the PATCH steps mutate the account's stored preferences. They leave
# text_size=large and reduce_motion=auto set on the account when done.

# ── 1. Unauthenticated GET → 401 ─────────────────────────────────────────────

GET {{base_url}}/api/v1/me/preferences

HTTP 401
[Asserts]
jsonpath "$.error.code" == "unauthenticated"
jsonpath "$.data" not exists

# ── 2. Authenticated GET → 200 with the full default-filled set ──────────────

GET {{base_url}}/api/v1/me/preferences
Authorization: Bearer {{erb_api_key}}

HTTP 200
[Asserts]
jsonpath "$.data" exists
jsonpath "$.data.text_size" matches "^(auto|small|regular|large)$"
jsonpath "$.data.reduce_motion" matches "^(auto|on|off)$"
jsonpath "$.data.high_contrast" matches "^(auto|on|off)$"
jsonpath "$.data.large_targets" matches "^(off|on)$"
jsonpath "$.data.dyslexia_font" matches "^(off|on)$"
jsonpath "$.data.locale" matches "^(en)$"
jsonpath "$.error" not exists

# ── 3. PATCH sets text_size; returns the full merged set ─────────────────────

PATCH {{base_url}}/api/v1/me/preferences
Authorization: Bearer {{erb_api_key}}
Content-Type: application/json
{
  "text_size": "large"
}

HTTP 200
[Asserts]
jsonpath "$.data.text_size" == "large"
jsonpath "$.data.reduce_motion" exists

# ── 4. PATCH partial-merge: setting reduce_motion preserves text_size ────────

PATCH {{base_url}}/api/v1/me/preferences
Authorization: Bearer {{erb_api_key}}
Content-Type: application/json
{
  "reduce_motion": "auto"
}

HTTP 200
[Asserts]
jsonpath "$.data.text_size" == "large"
jsonpath "$.data.reduce_motion" == "auto"

# ── 5. PATCH sets locale; preserves text_size ───────────────────────────────

PATCH {{base_url}}/api/v1/me/preferences
Authorization: Bearer {{erb_api_key}}
Content-Type: application/json
{
  "locale": "en"
}

HTTP 200
[Asserts]
jsonpath "$.data.locale" == "en"
jsonpath "$.data.text_size" == "large"

# ── 6. PATCH with an unknown key → 400 ───────────────────────────────────────

PATCH {{base_url}}/api/v1/me/preferences
Authorization: Bearer {{erb_api_key}}
Content-Type: application/json
{
  "not_a_real_pref": "x"
}

HTTP 400
[Asserts]
jsonpath "$.error.code" == "bad_request"

# ── 7. PATCH with an invalid enum value → 400 ────────────────────────────────

PATCH {{base_url}}/api/v1/me/preferences
Authorization: Bearer {{erb_api_key}}
Content-Type: application/json
{
  "text_size": "enormous"
}

HTTP 400
[Asserts]
jsonpath "$.error.code" == "bad_request"
