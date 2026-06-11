# Inbound rate limiting (add-esi-rate-limit-backoff §6.4)
#
# Asserts the two inbound limiters against a live dev stack:
#   - a throttled /api/* request → 429 with the standard `rate_limited`
#     envelope and a Retry-After header;
#   - a throttled /auth/callback request → 302 redirect to the too-busy page
#     (NOT the JSON envelope).
#
# No auth needed: an unauthenticated /api/v1/me is a normal 401 until throttled,
# and /auth/callback with junk params never reaches a real SSO exchange.
#
# The default dev config allows a burst of 20 for /api/* and 5 for /auth/*, so
# we send enough requests to exhaust each. A distinct X-Forwarded-For per file
# isolates this run's bucket from real traffic. The number of warm-up entries
# below assumes the default bursts; if you have tuned API_RATE_BURST /
# AUTH_RATE_BURST lower you may trip earlier (still a pass) — if higher, add
# more warm-up entries.
#
# Run with:
#   hurl --test --variable base_url=http://localhost:5000 rate_limit.hurl

# ── /api/* : exhaust the burst (21 requests > burst of 20) ───────────────────────
# These warm-up entries are unauthenticated 401s until the limiter trips; we do
# not assert their status (the trip point depends on configured burst).

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

# The next request is past the burst → throttled with the standard envelope.
GET {{base_url}}/api/v1/me
X-Forwarded-For: 203.0.113.10

HTTP 429
[Asserts]
header "Retry-After" exists
jsonpath "$.error.code" == "rate_limited"
jsonpath "$.error.message" exists

# ── /auth/callback : exhaust the tighter burst (6 requests > burst of 5) ─────────
# `--location` is intentionally NOT used so we capture the 302 itself. Junk
# code/state never reach a real SSO exchange once throttled.

GET {{base_url}}/auth/callback?code=x&state=y
X-Forwarded-For: 203.0.113.20

GET {{base_url}}/auth/callback?code=x&state=y
X-Forwarded-For: 203.0.113.20

GET {{base_url}}/auth/callback?code=x&state=y
X-Forwarded-For: 203.0.113.20

GET {{base_url}}/auth/callback?code=x&state=y
X-Forwarded-For: 203.0.113.20

GET {{base_url}}/auth/callback?code=x&state=y
X-Forwarded-For: 203.0.113.20

GET {{base_url}}/auth/callback?code=x&state=y
X-Forwarded-For: 203.0.113.20

# Past the auth burst → redirect to the too-busy page, NOT a JSON envelope.
GET {{base_url}}/auth/callback?code=x&state=y
X-Forwarded-For: 203.0.113.20

HTTP 303
[Asserts]
header "Location" == "/too-busy"
